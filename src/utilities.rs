use crate::console;

/// Perform left-aligned string padding with spaces.
///
/// Uses the get_true_string_grapheme_count function to get the "true"
/// of grapheme ("char") count in the string.
///
/// Example:
/// ```
/// string_left_align("hello world", 15)
/// ```
/// would produce "hello world    " (15 chars total).
pub fn string_left_align(string: &str, width: usize) -> String {
    let true_length = console::get_true_string_grapheme_count(string);

    if true_length >= width {
        string.to_string()
    } else {
        let spaces = " ".repeat(width - true_length);
        let mut spaced_string = string.to_string();
        spaced_string.push_str(&spaces);

        spaced_string
    }
}
