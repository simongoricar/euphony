use lazy_static::lazy_static;
use owo_colors::{OwoColorize, Style};

lazy_static! {
    static ref DEFAULT_LINE_CHAR: String = String::from('=');
    static ref DEFAULT_LINE_STYLE: Style = Style::new().bright_black();
    static ref DEFAULT_HEADER_STYLE: Style = Style::new().cyan().bold();
    static ref DEFAULT_WIDTH: usize = 50;
}

/// Print out a horizontal line.
/// `width` and `style` are optional arguments, specify None to get program-wide defaults.
///
/// # Example
/// ```
/// // Prints a horizontal line (50 characters wide and bright black by default).
/// horizontal_line(None, None)
/// ```
pub fn horizontal_line(width: Option<usize>, style: Option<Style>) {
    let style = style.unwrap_or(*DEFAULT_LINE_STYLE);

    match width {
        None => {
            println!("{}", (*DEFAULT_LINE_CHAR).repeat(*DEFAULT_WIDTH).style(*DEFAULT_LINE_STYLE));
        },
        Some(width) => {
            println!("{}", DEFAULT_LINE_CHAR.repeat(width).style(style));
        }
    }
}

/// Print out a centered text message with short horizontal lines on each side.
/// `header` is the text you want to print and `header_style` is its associated style.
/// `total_width` is the total width of the line (horizontal lines will adapt to fit this width).
/// `line_style` is the style for the lines on each side.
/// `margin` is the spacing between the text and horizontal lines on each side of the text.
///
/// All but `header` are optional, use None to get program-wide defaults.
pub fn horizontal_line_with_text(
    header: &str,
    header_style: Option<Style>,
    total_width: Option<usize>,
    line_style: Option<Style>,
    margin: Option<usize>,
) {
    let text_style = header_style.unwrap_or(*DEFAULT_HEADER_STYLE);
    let line_style = line_style.unwrap_or(*DEFAULT_LINE_STYLE);

    let margin = margin.unwrap_or(2);
    let total_width = total_width.unwrap_or(*DEFAULT_WIDTH);

    let wider_than_total_width = header.len() >= total_width;

    // Separate line widths ensure correctness on both odd and even lengths.
    let line_width_left = if wider_than_total_width { 0 } else {
        total_width
            .saturating_sub(header.len())
            .saturating_sub(2 * margin) / 2
    };
    let line_width_right = if wider_than_total_width { 0 } else {
        total_width
            .saturating_sub(header.len())
            .saturating_sub(line_width_left)
    };

    let line_str_left = DEFAULT_LINE_CHAR.repeat(line_width_left);
    let line_str_right = DEFAULT_LINE_CHAR.repeat(line_width_right);

    let margin_str = if wider_than_total_width {
        String::from("")
    } else {
        " ".repeat(margin)
    };

    println!(
        "{}{}{}{}{}",
        line_str_left.style(line_style),
        margin_str,
        header.style(text_style),
        margin_str,
        line_str_right.style(line_style),
    );
}

/// Simple println!() abstraction for adding new lines.
#[inline(always)]
pub fn new_line() {
    println!();
}
