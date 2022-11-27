use std::fmt::Display;
use crate::console::{LogTerminalBackend, TranscodeLogTerminalBackend};

/// Utility function for converting `Display` to `String`, then wrapping it up in a `Box`
/// and calling `LogBackend::log_println`.
/// Due to rust's trait upcasting being unstable, this version of the
/// method operates on `TranscodeLogTerminalBackend`s.
#[inline]
pub fn term_println_tlt<T: Display>(
    terminal: &mut dyn TranscodeLogTerminalBackend,
    content: T,
) {
    let content = Box::new(content.to_string());
    terminal.log_println(content);
}

/// Utility function for converting `Display` to `String`, then wrapping it up in a `Box`
/// and calling `LogBackend::log_println`.
/// Due to rust's trait upcasting being unstable, this version of the
/// method operates on `LogTerminalBackend`s.
#[inline]
pub fn term_println_lt<T: Display>(
    terminal: &mut dyn LogTerminalBackend,
    content: T,
) {
    let content = Box::new(content.to_string());
    terminal.log_println(content);
}
