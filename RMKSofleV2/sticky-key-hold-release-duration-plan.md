# Sticky Key Hold-Release Duration Plan

Status: **implemented, locally verified, and committed on `feat/sticky-mod`; not pushed**.

This plan was written against RMK branch `feat/sticky-mod` at commits:

- `0f2d0d96` — `feat: release held sticky keys after timeout`
- `17d30441` — `fix: derive defmt format for sticky modifier producers`

The second commit is only a `defmt::Format` build fix. The behavior discussed
here was introduced by `0f2d0d96`.

## Goal

Allow a Sticky Key press to be classified as a physical hold after a short,
configurable duration, independently of the normal Sticky Key timeout.

The intended interaction is:

1. A short press and release is a tap. It arms/latches the Sticky Key and gets
   the complete normal timeout after release.
2. A longer physical press is a hold. Its modifier or layer remains active
   while the key is down, but is released immediately when the physical key is
   released.
3. Holding a key must not consume or shorten the timeout used after a genuine
   tap. This preserves workflows that tap and stack multiple Sticky Keys.

The motivating example is holding Ctrl, clicking a browser link with a separate
mouse, and releasing Ctrl. RMK cannot observe the external mouse click, so the
physical duration of the Sticky Key press is the appropriate signal.

## Current Behavior and Cause

The current implementation adds:

```toml
release_on_keyup_after_timeout = true
```

When enabled, the same `timeout` controls both:

- how long a released Sticky Key remains latched and unused; and
- how long a physical press must remain down before key-up releases it rather
  than latching it.

For example, with a one-second timeout, a 500 ms Ctrl press is still treated as
a tap. On release, it latches Ctrl and starts a fresh one-second timeout. That
lingering Ctrl is undesirable for the hold workflow, but reducing the ordinary
timeout would also reduce the useful window for tapping and stacking Sticky
Keys.

In `rmk/src/keyboard/sticky_key.rs`, a new latch starts with its normal timeout
deadline. If that deadline expires while its physical producer is still down,
the timeout is deferred. With `release_on_keyup_after_timeout` enabled, the
latch entered `LatchPhase::TimedOut` in that implementation, and the later physical release removed the
effect. The core problem is not that deferred timeout behavior; it is that the
normal latch timeout is also being used as the hold-classification threshold.

## Proposed User-Facing Behavior

Replace the newly introduced boolean with a duration-valued setting:

```toml
[behavior.sticky_key]
timeout = "1s"
release_after_hold = "500ms"
```

Example results with that configuration:

| Physical press duration | Result on physical release |
| --- | --- |
| 100 ms | Treat as a tap; latch and start a fresh 1 s timeout |
| 499 ms | Treat as a tap; latch and start a fresh 1 s timeout |
| 500 ms | Treat as a hold; release immediately |
| 500 ms | Treat as a hold; release immediately |
| More than 1 s | Release immediately; never re-latch |

The threshold comparison should be inclusive: a duration equal to the
configured value is a hold.

`500ms` is a suggested personal configuration, not necessarily a library
default. It leaves separation between ordinary taps below roughly 250 ms and
intentional holds around 500 ms.

Omitting `release_after_hold` should preserve the historical behavior: an
otherwise unused physical press latches when released, even when the normal
timeout elapsed while the key was still down.

## Scope

Apply the duration-based behavior to:

- pure Sticky Key modifiers, including `OSM` compatibility actions; and
- Sticky Key layers, including `OSL` compatibility actions.

Do not apply it to modified Sticky Key tap keys. This preserves the shape
restriction introduced by `0f2d0d96`.

Do not change:

- other-key press or other-key release consumption;
- the behavior of a foreign keyboard key pressed while the Sticky Key remains
  physically down;
- layer-enter, layer-exit, or double-tap release modes;
- tap-key repeat behavior;
- modifier producer ownership and late-release handling;
- VIA/Vial timeout persistence semantics; or
- the ordinary timeout duration or when that timeout begins after a tap.

## Recommended Runtime Design

### Reuse the existing latch deadline

Do not add a second `Instant` to every latch. `StickyKeyState` can hold a
modifier latch, a layer latch, and a tap-key latch at the same time; another
timestamp in the generic latch would therefore impose an avoidable embedded
RAM cost.

Instead, let the existing `deadline` have phase-specific meaning:

- In `Pressed`, it is the configured `release_after_hold` threshold when
  that option is present. If the option is absent, it remains the normal
  timeout so historical held-timeout behavior is preserved.
- In `Latched`, it is always the normal unused-latch `timeout`.
- In `PressDeadlineInactive` or `HoldQualified`, it stores the continuous
  modifier chord's start, but `Latch::deadline()` hides it so no wake is
  scheduled. In `Held`, it is cleared because physical key-up owns cleanup.

`LatchPhase::HoldQualified` records that the hold threshold elapsed while the
producer remained physically down. Reusing the deadline and phase avoids an
additional timestamp in every latch. The compact threshold still replaces a
boolean in profiles and policies, so representative firmware BSS/flash should
be measured when a concrete Sofle firmware crate is available.

### Handle both event orderings

If physical key-up is processed before the keyboard loop polls an elapsed hold
deadline, `on_physical_release()` compares the event time with the deadline and releases
immediately. If the deadline poll happens first, `deadline_disposition()` changes
`Pressed` to `HoldQualified`, makes the deadline inactive while retaining the
chord start, and defers actual cleanup until
physical key-up.

The two paths must be behaviorally identical:

```text
Pressed + key-up before hold deadline -> Latched; start normal timeout
Pressed + key-up at/after hold deadline -> Released
Pressed + hold-deadline poll -> HoldQualified
HoldQualified + key-up -> Released
```

The threshold poll does not send a host report or release the effect. It only
records the classification, so the modifier or layer remains active for the
entire physical hold.

After a short key-up, replace the press-phase deadline with a fresh normal
timeout measured from key-up. The normal timeout and hold threshold remain
independent even though they reuse one storage slot in different phases.

This design permits a hold threshold longer than the normal timeout without
cross-field validation: while the feature is enabled and the key is physically
down, the configured hold threshold is the only relevant deadline. The normal
timeout begins only if key-up occurs before that threshold.

### Preserve existing Held behavior

If another keyboard action is observed while the Sticky Key producer remains
down, the existing state machine transitions to `Held`. Physical release from
`Held` must continue to release immediately regardless of the duration
threshold. The new threshold fills the gap where an action, such as a click
from an external mouse, is not observable by RMK.

### Modifier visibility

The new duration does not itself make a pure modifier host-visible during the
physical press. A profile intended for conventional hold behavior must still
use:

```toml
activate_on_keypress = true
```

This is existing policy and should be documented next to the new option so a
user does not configure a hold threshold and assume that alone makes Ctrl
active while held.

Sticky layers are already activated on physical press, so this qualification
is specific to pure modifiers.

## Configuration and API Design

### Recommended schema

Assuming the boolean added by `0f2d0d96` has not become a compatibility
commitment, replace it instead of retaining two overlapping settings:

```text
release_on_keyup_after_timeout: bool
```

becomes:

```text
release_after_hold: optional duration
```

Use these representations through the configuration pipeline:

- TOML input: `Option<DurationMillis>`
- resolved `rmk-config` model: `Option<u64>` milliseconds
- concrete runtime profile: compact `StickyKeyHoldDuration`
- `StickyKeyPolicy`: compact `StickyKeyHoldDuration`

The absent value means disabled. A present value is inherited by named
profiles in the same manner as other optional profile fields. The compact
runtime domain type stores the disabled state in one machine word, avoiding
the two-word cost of `Option<Duration>` in embedded profiles and latches.

The keymap policy resolver must continue forcing this setting off for
`StickyKeyShape::TapKey`, even if the selected profile configures it.

### Compatibility alternative

If the boolean must remain backward-compatible, use both:

```toml
release_on_keyup_after_timeout = true
release_after_hold = "500ms"
```

and define the old boolean-without-duration behavior as using the normal
`timeout`. This alternative adds schema, inheritance, documentation, and test
complexity and should only be selected if real released configurations depend
on the boolean.

Do not silently reinterpret `release_on_keyup_after_timeout = true` as a
shorter hard-coded threshold. That would make old configuration text describe
behavior it no longer has.

### Named-profile inheritance limitation

With a plain optional duration, omission in a named profile means inheritance.
It does not provide a way for one named profile to explicitly disable a value
configured in the default profile.

The minimal implementation accepts this behavior, which matches ordinary
optional duration inheritance. A user who needs selective enablement can put
`release_after_hold` only on the named profiles that need it rather than on
the default profile.

If explicit child-profile disablement is required later, add an intentional
duration-or-disabled input type rather than overloading a magic duration.

## Files Expected to Change

### Configuration plumbing

- `rmk-config/src/lib.rs`
  - Replace the optional boolean in `StickyKeyConfig` and the partial named
    `StickyKeyProfile` with an optional duration.
- `rmk-config/src/resolved/behavior.rs`
  - Carry the duration as optional milliseconds.
  - Update parsing/resolution tests for the default and named profiles.
- `rmk-macro/src/codegen/behavior.rs`
  - Inherit the optional duration.
  - Emit `StickyKeyHoldDuration` in generated runtime profiles.
  - Update code-generation inheritance tests.
- `rmk/src/config/behavior.rs`
  - Replace the concrete boolean with the compact duration domain type.
  - Default it to `StickyKeyHoldDuration::DISABLED`.
- `rmk/src/keymap.rs`
  - Carry the compact duration in `StickyKeyPolicy`.
  - Suppress it for tap-key shapes.
  - Update policy-resolution tests.

### Runtime behavior

- `rmk/src/keyboard/sticky_key.rs`
  - Use the hold duration for the deadline while physically `Pressed`.
  - Preserve `HoldQualified` to record a threshold-first wake-up until key-up.
  - Replace the deadline with a fresh normal timeout after a short key-up.
  - Update focused latch unit tests.
- `rmk/src/keyboard.rs`
  - Thread the already-captured physical event time into Sticky Key dispatch,
    including delayed combo-buffer dispatch.
  - Update or add the event-first key-up integration test.

### Scenario tests and documentation

- `rmk/tests/scenarios/sticky_key.toml`
- `rmk/tests/scenarios/one_shot.toml`
- `docs/docs/main/docs/configuration/behavior.md`
- `rmk/CHANGELOG.md`

Protocol or persisted-storage formats should not change because this is a
profile configuration field rather than a key action field. Verify this
assumption during implementation before deciding that snapshot updates are
unnecessary.

## Implementation Sequence

### 1. Change the configuration model

1. Replace the TOML boolean with `Option<DurationMillis>` under both the
   default Sticky Key configuration and partial named profiles.
2. Rename it to `release_after_hold` throughout resolved configuration.
3. Resolve/inherit it as optional milliseconds during macro expansion.
4. Emit `StickyKeyHoldDuration::from_duration(...)` or
   `StickyKeyHoldDuration::DISABLED` in generated profiles.
5. Change the runtime profile default to `StickyKeyHoldDuration::DISABLED`.
6. Change `StickyKeyPolicy` to carry the compact duration and preserve the
   tap-key exclusion.

Complete this layer first so compile errors identify every configuration
consumer that needs migration.

### 2. Give the existing deadline phase-specific meaning

1. In `Latch::new()` and `Latch::begin_press()`, use the enabled hold duration
   or fall back to `timeout` for the press-phase deadline.
2. Do not add a second timestamp or enlarge the latch's timer storage.
3. On short key-up, replace the press-phase deadline with a normal timeout
   measured from the supplied event time.
4. Continue using the normal timeout for all already-latched and buffered-claim
   paths.

### 3. Update physical-release transitions

1. In `Pressed`, return `Released` when the configured threshold has elapsed.
2. Otherwise transition to `Latched`, discard the old press deadline, and
   start a fresh normal timeout from release.
3. Keep `Held -> Released` unchanged.
4. Keep owner/source validation before any duration decision.
5. Ensure accumulated modifier producers invoke the decision only when the
   last physical producer is released, matching current ownership behavior.

### 4. Update timeout transitions

1. When a configured hold deadline expires in `Pressed`, transition to
   `HoldQualified`, retain the chord start in the inactive deadline slot, and
   defer physical cleanup.
2. Preserve `HoldQualified -> Released` on physical key-up.
3. Preserve foreign-key handling from `Pressed`, `PressDeadlineInactive`, and
   `HoldQualified`.
4. When the duration is absent, preserve the historical timeout deferral in
   `PressDeadlineInactive` while retaining the chord start.
5. Confirm that an event-first key-up at or after a deadline produces the same
   result as a timeout-first loop wake-up.

### 5. Update user documentation and migration text

1. Replace the boolean example and description with the duration setting.
2. Clearly distinguish the hold threshold from the normal latch timeout.
3. State that the normal timeout begins fresh after a tap is released.
4. State that pure modifiers need `activate_on_keypress = true` to behave as
   host-visible modifiers while physically held.
5. Mention that external mouse activity is not observed or required.
6. If compatibility is intentionally broken, include the direct migration:

   ```text
   release_on_keyup_after_timeout = true
   ```

   becomes, for example:

   ```text
   release_after_hold = "500ms"
   ```

## Required Test Matrix

### Focused latch tests

Use deterministic instants rather than relying on wall-clock timing.

1. No configured threshold:
   - release before normal timeout latches;
   - release after the normal timeout was polled still latches and starts a
     fresh timeout.
2. Configured 500 ms threshold:
   - release at 499 ms latches;
   - release at exactly 500 ms releases;
   - release after 500 ms releases.
3. Normal timeout interaction:
   - normal timeout shorter than the physical press is deferred;
   - later qualifying key-up releases;
   - no stale deadline remains.
4. Foreign keyboard action:
   - a foreign key changes `Pressed` to `Held` before the threshold;
   - physical key-up releases immediately as it did previously.
5. Re-press and replacement:
   - ordinary `begin_press()` resets the physical start time;
   - accumulated modifier producers preserve the chord's first press time;
   - an old producer's late release cannot consume a newer latch.
6. Modifier aggregation:
   - releasing a non-final producer does not classify or release the complete
     modifier latch;
   - releasing the final producer applies the latest profile's threshold from
     the first press in the continuous modifier chord;
   - both physical release orders are covered.

### Keyboard integration tests

1. Event-first wake-up:
   - advance mock time beyond the hold threshold without polling a Sticky Key
     timer;
   - process key-up;
   - verify immediate modifier/layer removal.
2. Combo-buffered press:
   - delay Sticky Key dispatch beyond the hold threshold;
   - verify classification still uses the original physical press time.
3. Timeout-first wake-up:
   - process the normal timeout while the key remains down;
   - process key-up later;
   - verify immediate removal when the hold threshold was exceeded.
4. Host-visible modifier:
   - with `activate_on_keypress = true`, observe modifier-down on press and
     modifier-up on qualifying key-up;
   - verify the next ordinary key is unmodified.
5. Short modifier tap:
   - release below threshold;
   - verify the modifier applies to the next key;
   - verify the full normal timeout is measured from physical release.
6. Sticky layer:
   - short release leaves the layer latched;
   - qualifying release removes it immediately.
7. Tap-key exclusion:
   - a profile containing the duration does not change modified sticky tap-key
     behavior.

### Configuration/code-generation tests

1. Parse milliseconds and seconds.
2. Reject malformed duration text.
3. Inherit the default duration into a named profile.
4. Override the default with a different named-profile duration.
5. Emit `None` when absent.
6. Confirm tap-key policy resolution disables the feature while modifier and
   layer policy resolution retain it.

### Existing regressions that must stay green

- Default held-timeout behavior when the new option is absent.
- Other-key press and other-key release policies.
- Double-tap cancellation and late physical releases.
- Layer-enter and layer-exit policies.
- Sticky modifier and sticky layer coexistence.
- Modifier producer ownership and combo releases.
- Modified tap-key repeat and replacement behavior.
- OSM/OSL aliases and VIA/Vial default-profile behavior.

## Verification Commands

Use the repository's established toolchain and CI commands at implementation
time. At minimum, run the narrow suites for:

- `rmk-config` parsing/resolution;
- `rmk-macro` code generation;
- `rmk` Sticky Key unit tests;
- the `sticky_key.toml` scenarios;
- the `one_shot.toml` scenarios; and
- formatting plus the normal workspace compile/check used by this branch.

Then inspect the diff to ensure no unrelated files or generated artifacts were
changed.

## Estimated Change Size

The runtime behavior should remain small: two initialization/reset sites, one
key-up predicate, and reuse of the existing deadline and timeout-specific phase.

The number of files is larger because RMK deliberately separates TOML input,
resolved configuration, generated code, runtime configuration, policy
resolution, tests, and documentation. Expect roughly 9–11 touched files, with
most changes being mechanical field plumbing and tests rather than new runtime
complexity.

No keyboard-loop redesign, external mouse integration, new event type, or
additional runtime timestamp should be necessary.

## Acceptance Criteria

The work is complete only when all of the following are true:

- The normal Sticky Key timeout and physical hold threshold are independently
  configurable.
- A short tap receives the full normal timeout beginning at key-up.
- A qualifying hold releases its modifier/layer immediately on key-up.
- A hold longer than the normal timeout never re-latches when the duration
  feature is enabled.
- Omitting the new duration preserves historical behavior.
- Pure modifiers remain host-visible throughout a qualifying hold when
  `activate_on_keypress = true`, then produce a balancing modifier-up report.
- Sticky layers follow the same duration distinction.
- Modified sticky tap keys are unaffected.
- Existing ownership, release-mode, timeout, profile, OSM/OSL, and protocol
  tests remain green.

## Decision to Confirm Before Any Future Implementation

Confirm whether `release_on_keyup_after_timeout` has any compatibility
obligation outside this feature branch.

- If **no**, replace it with `release_after_hold = "..."` as recommended.
- If **yes**, retain a compatibility path using the old boolean and document
  exactly how it selects the legacy normal-timeout threshold.

Unless new information establishes such a compatibility requirement, the
implementation should use the simpler replacement design.
