use console::{Alignment, Style, style};
use console::Color::Color256;
use lazy_static::lazy_static;
use super::super::Config;
use crate::console as c;

lazy_static! {
    static ref HEADER_STYLE: Style = Style::new().fg(Color256(152)).bold();
    static ref SUBHEADER_STYLE: Style = Style::new().cyan().italic();

    static ref LIBRARY_NAME_STYLE: Style = Style::new().bold();
    static ref LIBRARY_PATH_STYLE: Style = Style::new().green();
}


pub fn cmd_show_config(config: &Config) {
    c::horizontal_line(None, None);
    c::horizontal_line_with_text(
        HEADER_STYLE.apply_to("Configuration").to_string(),
        None, None,
    );
    c::horizontal_line(None, None);
    c::new_line();

    // Basics
    c::horizontal_line_with_text(
        SUBHEADER_STYLE.apply_to("basics").to_string(),
        None, None,
    );
    println!(
        "  root_library_path = {}",
        config.basics.root_library_path,
    );
    c::new_line();

    // Validation
    c::horizontal_line_with_text(
        SUBHEADER_STYLE.apply_to("validation").to_string(),
        None, None,
    );
    println!(
        "  audio_file_extensions = {:?}",
        config.validation.audio_file_extensions,
    );
    println!(
        "  ignored_file_extensions = {:?}",
        config.validation.ignored_file_extensions,
    );
    c::new_line();

    // Libraries
    c::horizontal_line_with_text(
        SUBHEADER_STYLE.apply_to("libraries").to_string(),
        None, None,
    );

    let library_count = config.libraries.len();
    println!(
        "There are {} available libraries:",
        style(library_count)
            .bold(),
    );

    for (_, library) in &config.libraries {
        println!(
            "  {} {}",
            console::pad_str(
                &format!(
                    "{}:",
                    LIBRARY_NAME_STYLE.apply_to(&library.name).to_string(),
                ),
                20,
                Alignment::Left,
                None,
            ),
            LIBRARY_PATH_STYLE.apply_to(&library.path)
                .to_string(),
        );
    }
    c::new_line();

    // Aggregated library
    c::horizontal_line_with_text(
        SUBHEADER_STYLE.apply_to("aggregated_library").to_string(),
        None, None,
    );
    println!(
        "  path = {}",
        config.aggregated_library.path,
    );
}
