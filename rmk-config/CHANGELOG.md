# Changelog

All notable changes to rmk-config will be documented in this file.

## [Unreleased]

### Changed

- **[BREAKING]** Reorganized internal module structure into `types/`, `config/`, and `api/` directories
- **[BREAKING]** Renamed internal types:
  - `KeyboardInfo` → `KeyboardMetadata` (internal TOML structure)
  - `Basic` → `KeyboardInfo` (public API type)
  - `LayoutTomlConfig` → `LayoutDefinition`
  - `LayoutConfig` → `Layout`
  - `LayerTomlConfig` → `LayerDefinition`
- Renamed API methods for better clarity (old names still work):
  - `get_device_config()` → `keyboard_info()`
  - `get_chip_model()` → `chip()`
  - `get_chip_config()` → `chip_settings()`
  - `get_layout_config()` → `layout()`
  - `get_board_config()` → `board()`
  - `get_communication_config()` → `communication()`
  - `get_behavior_config()` → `behavior()`
  - `get_storage_config()` → `storage()`
  - `get_light_config()` → `light()`
  - `get_host_config()` → `host()`
  - `get_output_config()` → `outputs()`
  - `get_dependency_config()` → `dependencies()`
- Renamed `layout.rs` to `layout_parser.rs` to better reflect its purpose

### Added

- Created `ConfigLoader` with documented two-pass loading mechanism
- Created centralized `Validator` for all configuration validation
- Added comprehensive module documentation

### Improved

- Reduced `lib.rs` from 1055 lines to ~150 lines
- Simplified `EventConfig` from 272 lines to ~200 lines
- Better separation of concerns between types, configuration logic, and API
- Clearer documentation of default configuration handling

### Fixed

- Consolidated scattered validation logic into single `Validator`

## Notes

This refactoring maintains backward compatibility at the API level. All existing code using rmk-config will continue to work, though using the new method names is recommended for future code.

See [REFACTORING.md](REFACTORING.md) for detailed migration guide and implementation details.
