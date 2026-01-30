//! Unified error types for rmk-config
//!
//! This module provides a centralized error type for all configuration-related errors,
//! replacing scattered panic! calls and string-based errors throughout the crate.

use std::fmt;

/// Unified error type for rmk-config
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// File I/O error
    FileRead { path: String, message: String },
    /// TOML parsing error
    TomlParse { path: String, message: String },
    /// Validation error with context
    Validation { field: String, message: String },
    /// Missing required field
    MissingField { field: String },
    /// Invalid value
    InvalidValue {
        field: String,
        value: String,
        expected: String,
    },
    /// Chip/board not supported
    UnsupportedHardware { kind: String, name: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::FileRead { path, message } => {
                write!(f, "Failed to read config file '{}': {}", path, message)
            }
            ConfigError::TomlParse { path, message } => {
                write!(f, "Failed to parse '{}': {}", path, message)
            }
            ConfigError::Validation { field, message } => {
                write!(f, "Validation error in '{}': {}", field, message)
            }
            ConfigError::MissingField { field } => {
                write!(f, "Missing required field: {}", field)
            }
            ConfigError::InvalidValue {
                field,
                value,
                expected,
            } => {
                write!(
                    f,
                    "Invalid value '{}' for '{}', expected: {}",
                    value, field, expected
                )
            }
            ConfigError::UnsupportedHardware { kind, name } => {
                write!(f, "Unsupported {}: {}", kind, name)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Result type alias for configuration operations
pub type ConfigResult<T> = Result<T, ConfigError>;
