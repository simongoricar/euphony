use lazy_static::lazy_static;
use owo_colors::{OwoColorize, Style};
use crate::{console, utilities};
use super::super::Config;

pub fn cmd_show_config(config: &Config) {
    lazy_static! {
        static ref HEADER_STYLE: Style = Style::new().bright_cyan().bold();
        static ref SUBHEADER_STYLE: Style = Style::new().cyan().italic();
        static ref LIBRARY_NAME_STYLE: Style = Style::new().bold();
        static ref LIBRARY_PATH_STYLE: Style = Style::new().green();
    }

    console::horizontal_line(None, None);
    console::horizontal_line_with_text(
        &"Configuration".style(*HEADER_STYLE).to_string(),
        None, None, None,
    );
    console::horizontal_line(None, None);
    console::new_line();

    // Basics
    console::horizontal_line_with_text(
        &"basics".style(*SUBHEADER_STYLE).to_string(),
        None, None, None
    );
    println!(
        "  root_library_path = {}",
        config.basics.root_library_path
    );
    console::new_line();

    // Validation
    console::horizontal_line_with_text(
        &"validation".style(*SUBHEADER_STYLE).to_string(),
        None, None, None,
    );
    println!(
        "  audio_file_extensions = {:?}",
        config.validation.audio_file_extensions,
    );
    println!(
        "  ignored_file_extensions = {:?}",
        config.validation.ignored_file_extensions,
    );
    console::new_line();

    // Libraries
    console::horizontal_line_with_text(
        &"libraries".style(*SUBHEADER_STYLE).to_string(),
        None, None, None,
    );

    let library_count = config.libraries.len();
    println!("There are {} available libraries:", library_count.bold());

    for (_, library) in &config.libraries {
        let library_name_styled = library.name.style(*LIBRARY_NAME_STYLE).to_string();
        let library_path_styled = library.path.style(*LIBRARY_PATH_STYLE).to_string();

        println!(
            "  {} {}",
            utilities::string_left_align(
                &format!("{}:", library_name_styled),
                20,
            ),
            library_path_styled,
        );
    }
    console::new_line();

    // Aggregated library
    console::horizontal_line_with_text(
        &"aggregated_library".style(*SUBHEADER_STYLE).to_string(),
        None, None, None,
    );
    println!(
        "  path = {}",
        config.aggregated_library.path,
    );
}
