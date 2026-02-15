//! Security utilities for RustDesk
//!
//! This module provides security-focused utilities for:
//! - Input validation
//! - Command execution safety
//! - Path sanitization
//! - Hostname validation

pub mod validation;

pub use validation::{
    validate_hostname,
    validate_port,
    validate_path,
    sanitize_command_arg,
    ValidationError,
};
