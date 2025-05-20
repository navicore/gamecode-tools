//! Logging utilities for the gamecode-tools library
//!
//! This module provides logging initialization and convenient re-exports
//! of the log crate macros for consistent logging across the library.

/// Initialize logging with the specified level
pub fn init(_level: log::LevelFilter) {
    // Implementation depends on what logging framework is used
    // This is left as a stub for applications to implement
    // as they may choose different logging backends
}

// Re-export the log crate and its macros
pub use log::{debug, error, info, trace, warn, LevelFilter};