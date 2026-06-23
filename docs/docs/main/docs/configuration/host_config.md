# Host Configuration

The `[host]` section configures host-side tools and related features.

## Configuration Example

```toml
[host]
# Enable or disable Vial support (default: true)
vial_enabled = true

# Unlock keys for Vial security (optional)
# Keys must be pressed simultaneously to unlock Vial configuration
unlock_keys = [[0, 0], [0, 1]]  # Keys at (row=0,col=0) and (row=0,col=1)

# Start Vial unlocked, bypassing the unlock-key combo (default: false)
# When true, secured operations such as the Matrix Tester are available immediately without pressing `unlock_keys`.
vial_insecure = false
```
