use std::ops::Div;

use console::{Alignment, Color, Style};
use lazy_static::lazy_static;

// See https://www.ditig.com/256-colors-cheat-sheet for the Color256 cheat sheet.

lazy_static! {
    static ref DEFAULT_WIDTH: usize = 80;
    static ref DEFAULT_LINE_CHAR: String = String::from('=');
    static ref DEFAULT_LINE_STYLE: Style = Style::new().fg(Color::Color256(8));
}

/// Print out a horizontal line.
/// `width` and `style` are optional arguments, specify None to get program-wide defaults.
///
/// # Example
/// ```
/// // Prints a horizontal line (60 characters wide and gray by default).
/// horizontal_line(None, None)
/// ```
pub fn horizontal_line(width: Option<usize>, style: Option<Style>) {
    let width = width.unwrap_or(*DEFAULT_WIDTH);
    let style = style.unwrap_or((*DEFAULT_LINE_STYLE).clone());

    println!(
        "{}",
        style.apply_to(DEFAULT_LINE_CHAR.repeat(width))
    );
}

/// Print out a centered text message with short horizontal lines on each side.
/// `header` is the text you want to print and `apply_default_style` is whether you want to
/// style the entire header with the default text style.
/// `total_width` is the total width of the line (horizontal lines will adapt to fit this width).
/// `line_style` is the style for the lines on each side.
/// `margin` is the spacing between the text and horizontal lines on each side of the text.
///
/// All but `header` and `apply_default_style` are optional, use None to get program-wide defaults.
pub fn horizontal_line_with_text<S: AsRef<str>>(
    text: S,
    total_width: Option<usize>,
    line_style: Option<Style>,
) {
    const LINE_TO_TEXT_MARGIN: usize = 2;

    let total_width = total_width.unwrap_or(*DEFAULT_WIDTH);
    let line_style = line_style.unwrap_or((*DEFAULT_LINE_STYLE).clone());
    let text_length = console::measure_text_width(text.as_ref());

    let is_wider_than_total_width = text_length >= total_width;

    let line_width_left = if is_wider_than_total_width { 0 } else {
        total_width
            .saturating_sub(text_length)
            .saturating_sub(2 * LINE_TO_TEXT_MARGIN)
            .div(2)
    };
    let line_width_right = if is_wider_than_total_width { 0 } else {
        total_width
            .saturating_sub(text_length)
            .saturating_sub(line_width_left)
            .saturating_sub(2 * LINE_TO_TEXT_MARGIN)
    };

    let line_left = DEFAULT_LINE_CHAR.repeat(line_width_left);
    let line_right = DEFAULT_LINE_CHAR.repeat(line_width_right);

    let margin_str = if is_wider_than_total_width {
        String::from("")
    } else {
        " ".repeat(LINE_TO_TEXT_MARGIN)
    };

    println!(
        "{}{}{}{}{}",
        line_style.apply_to(line_left),
        margin_str,
        text.as_ref(),
        margin_str,
        line_style.apply_to(line_right),
    );
}

pub fn centered_print<S: AsRef<str>>(
    text: S,
    total_width: Option<usize>,
) {
    let total_width = total_width.unwrap_or(*DEFAULT_WIDTH);
    println!(
        "{}",
        console::pad_str(
            text.as_ref(),
            total_width,
            Alignment::Center,
            Some("..."),
        ),
    );
}

/// Simple println!() abstraction for adding new lines.
#[inline(always)]
pub fn new_line() {
    println!();
}
