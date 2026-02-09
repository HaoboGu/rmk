# Tabber

Tabber is a special action that provides Alt+Tab-like tab/window switching behavior. When activated, it sends a `Modifier` + `Tab` key combination and holds the modifier key. Subsequent presses of Tabber send only the `Tab` key. The modifier is automatically released when you switch to a different layer.

::: warning
Tabber cannot be used in the base layer. This is a safety measure to prevent held modifiers from being stuck.
:::

## Syntax

### TOML Configuration

```toml
# Single modifier
TABBER(LCtrl)

# Multiple modifiers
TABBER(LCtrl|LGui)
TABBER(LAlt|LShift)
```

### Rust Configuration

```rust
use rmk::action::Action;
use rmk::config::tabber_action::TabberAction;

Action::Tabber(TabberAction::new(ModifierCombination::LCtrl))
```

## How It Works

When you press a Tabber key:

1. **First press**: Sends the specified modifier(s) + Tab, then holds the modifier(s)
2. **Subsequent presses**: Sends only Tab (while modifier is held)
3. **Layer switch**: Releases the held modifier(s)

This behavior mimics the operating system's Alt+Tab functionality, where holding Alt and pressing Tab cycles through windows.

## Configuration

### Available Modifiers

Any single modifier or combination of modifiers can be used with Tabber:

| Modifier | Description                 |
| -------- | --------------------------- |
| `LCtrl`  | Left Control                |
| `RCtrl`  | Right Control               |
| `LAlt`   | Left Alt (Option on macOS)  |
| `RAlt`   | Right Alt (Option on macOS) |
| `LGui`   | Left GUI (Windows/Command)  |
| `RGui`   | Right GUI (Windows/Command) |
| `LShift` | Left Shift                  |
| `RShift` | Right Shift                 |

Combine modifiers using `|` (pipe):

```toml
TABBER(LCtrl|LGui)  # Ctrl + Windows
TABBER(LAlt|LShift)  # Alt + Shift
```

::: tip
You can use any Shift key along with Tabber to reverse the direction of tab/window cycling.
:::

## Platform-Specific Usage

### Windows

- `TABBER(LCtrl)`: Cycle through browser tabs
- `TABBER(LGui)`: Bring up Task View and cycle through windows
- `TABBER(LAlt)`: Cycle through windows in the regular window switcher

### macOS

- `TABBER(LCtrl)`: Cycle through browser tabs
- `TABBER(LGui)`: Cycle through windows in Mission Control

### Linux

- `TABBER(LCtrl)`: Cycle through browser tabs
- `TABBER(LAlt)`: Cycle through windows in the regular window switcher
