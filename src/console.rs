use lazy_static::lazy_static;
use owo_colors::{OwoColorize, Style};

lazy_static! {
    static ref DEFAULT_LINE_CHAR: String = String::from('=');
    static ref DEFAULT_LINE_STYLE: Style = Style::new().bright_black();
    static ref DEFAULT_HEADER_STYLE: Style = Style::new().cyan().bold();
    static ref DEFAULT_WIDTH: usize = 50;
}

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

    let line_width_per_side = if wider_than_total_width {
        0
    } else {
        total_width.saturating_sub(header.len()).saturating_sub(2 * margin) / 2
    };

    let line_str = DEFAULT_LINE_CHAR.repeat(line_width_per_side);

    let margin_str = if wider_than_total_width {
        "".to_string()
    } else {
        " ".repeat(margin)
    };

    println!(
        "{}{}{}{}{}",
        line_str.style(line_style), margin_str,
        header.style(text_style),
        margin_str, line_str.style(line_style)
    );
}

#[inline(always)]
pub fn new_line() {
    println!();
}
