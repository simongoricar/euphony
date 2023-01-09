use std::fmt::Display;
use crate::console::{SimpleTerminalBackend, AdvancedTranscodeTerminalBackend, FullValidationBackend};

/// Utility function for converting `Display` to `String`, then wrapping it up in a `Box`
/// and calling `LogBackend::log_println`.
/// Due to rust's trait upcasting being unstable, this version of the
/// method operates on `AdvancedTranscodeTerminalBackend`.
#[inline]
pub fn term_println_attb<T: Display>(
    terminal: &dyn AdvancedTranscodeTerminalBackend,
    content: T,
) {
    let content = Box::new(content.to_string());
    terminal.log_println(content);
}

/// Utility function for converting `Display` to `String`, then wrapping it up in a `Box`
/// and calling `LogBackend::log_println`.
/// Due to rust's trait upcasting being unstable, this version of the
/// method operates on `FullValidationBackend`.
#[inline]
pub fn term_println_fvb<T: Display>(
    terminal: &dyn FullValidationBackend,
    content: T,
) {
    let content = Box::new(content.to_string());
    terminal.log_println(content);
}

/// Utility function for converting `Display` to `String`, then wrapping it up in a `Box`
/// and calling `LogBackend::log_println`.
/// Due to rust's trait upcasting being unstable, this version of the
/// method operates on `SimpleTerminalBackend`s.
#[inline]
pub fn term_println_stb<T: Display>(
    terminal: &dyn SimpleTerminalBackend,
    content: T,
) {
    let content = Box::new(content.to_string());
    terminal.log_println(content);
}
