use console::{Style, style};
use console::Color::Color256;
use lazy_static::lazy_static;
use super::super::Config;
use crate::console as c;

lazy_static! {
    static ref HEADER_STYLE: Style = Style::new().fg(Color256(96)).bold().underlined();
    static ref SUBHEADER_STYLE: Style = Style::new().cyan();

    static ref LIBRARY_NAME_STYLE: Style = Style::new().bold();
    static ref LIBRARY_KEY_STYLE: Style = Style::new().bright().black();
    static ref LIBRARY_PATH_STYLE: Style = Style::new().fg(Color256(107));
}


pub fn cmd_show_config(config: &Config) {
    c::horizontal_line_with_text(
        HEADER_STYLE.apply_to("⚙ CONFIGURATION ⚙").to_string(),
        None, None,
    );

    let configuration_file_path_str = config.configuration_file_path.to_string_lossy();
    c::centered_print(
        format!(
            "(using {})",
            style(configuration_file_path_str)
                .yellow()
                .bright()
                .italic(),
        ),
        None,
    );
    c::new_line();
    c::new_line();

    // Basics
    c::centered_print(
        SUBHEADER_STYLE.apply_to("- basics -").to_string(),
        None,
    );
    println!(
        "  root_library_path = {}",
        config.essentials.root_library_path,
    );
    c::new_line();

    // Validation
    c::centered_print(
        SUBHEADER_STYLE.apply_to("- validation -").to_string(),
        None,
    );
    println!(
        "  allowed_other_files_by_extension = {:?}",
        config.validation.allowed_other_files_by_extension,
    );
    println!(
        "  allowed_other_files_by_name = {:?}",
        config.validation.allowed_other_files_by_name,
    );
    c::new_line();

    // Libraries
    c::centered_print(
        SUBHEADER_STYLE.apply_to("- libraries -").to_string(),
        None,
    );

    let library_count = config.libraries.len();
    c::centered_print(
        format!(
            "({} available)",
            style(library_count)
                .bold(),
        ),
        None,
    );

    for (library_key, library) in &config.libraries {
        println!(
            "  {}",
            format!(
                "{} {}:",
                LIBRARY_NAME_STYLE.apply_to(&library.name).to_string(),
                LIBRARY_KEY_STYLE.apply_to(format!("({})", library_key)).to_string(),
            ),
        );

        println!(
            "    path = {}",
            LIBRARY_PATH_STYLE.apply_to(&library.path).to_string(),
        );
        println!(
            "    allowed_audio_files_by_extension = {:?}",
            &library.allowed_audio_files_by_extension,
        );
        c::new_line();
    }

    // Aggregated library
    c::centered_print(
        SUBHEADER_STYLE.apply_to("- aggregated_library -").to_string(),
        None,
    );
    println!(
        "  path = {}",
        config.aggregated_library.path,
    );
}

pub fn cmd_list_libraries(config: &Config) {
    c::horizontal_line_with_text(
        HEADER_STYLE.apply_to("Libraries").to_string(),
        None, None,
    );
    let configuration_file_path_str = config.configuration_file_path.to_string_lossy();
    c::centered_print(
        format!(
            "(using {})",
            style(configuration_file_path_str)
                .yellow()
                .bright()
                .italic(),
        ),
        None,
    );
    c::new_line();

    println!(
        "There are {} libraries available:",
        style(config.libraries.len())
            .bold(),
    );

    for (library_key, library) in &config.libraries {
        println!(
            "  {} {}",
            LIBRARY_KEY_STYLE
                .apply_to(
                    format!(
                        "{:>22}",
                        format!("({})", library_key),
                    )
                )
                .to_string(),
            LIBRARY_NAME_STYLE
                .apply_to(&library.name)
                .to_string(),
        );
    }
}
