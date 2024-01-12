use std::path::Path;

use crossterm::style::Stylize;
use euphony_configuration::Configuration;

use crate::console::frontends::SimpleTerminal;
use crate::console::LogBackend;

/// Prints a configuration group header, for example: `|----- your header here -----|`.
fn terminal_print_group_header<S: AsRef<str>>(
    terminal: &SimpleTerminal,
    header: S,
) {
    const PAD_TO_WIDTH: usize = 10;

    let total_padding = PAD_TO_WIDTH.saturating_sub(header.as_ref().len());
    let left_padding = total_padding / 2;
    let right_padding = total_padding - left_padding;

    terminal.log_println(format!(
        "|----- {}{:^12}{} -----|",
        " ".repeat(left_padding),
        header.as_ref().bold(),
        " ".repeat(right_padding)
    ));
}

/// Associated with the `show-config` command.
///
/// Prints the entire configuration.
pub fn cmd_show_config(config: &Configuration, terminal: &mut SimpleTerminal) {
    terminal.log_println(format!(
        "Configuration file: {}",
        config.configuration_file_path.to_string_lossy(),
    ));
    terminal.log_newline();


    // Essentials
    terminal_print_group_header(terminal, "paths");
    terminal.log_println(format!(
        "    base_library_path = {}",
        config.paths.base_library_path,
    ));
    terminal.log_println(format!(
        "    base_tools_path = {}",
        config.paths.base_tools_path,
    ));
    terminal.log_newline();


    // Logging
    terminal_print_group_header(terminal, "logging");
    terminal.log_println(format!(
        "    default_log_output_path = {:?}",
        config.logging.default_log_output_path
    ));


    // Validation (basics)
    terminal_print_group_header(terminal, "validation");
    terminal.log_println(format!(
        "    extensions_considered_audio_files = {:?}",
        config.validation.extensions_considered_audio_files,
    ));


    // Tools
    terminal_print_group_header(terminal, "tools");
    terminal.log_println(format!(" => {}", "ffmpeg".bold()));
    terminal.log_println(format!(
        "    binary = {}",
        config.tools.ffmpeg.binary,
    ));
    terminal.log_println(format!(
        "    audio_transcoding_args = {:?}",
        config.tools.ffmpeg.audio_transcoding_args,
    ));
    terminal.log_println(format!(
        "    audio_transcoding_output_extension = {:?}",
        config.tools.ffmpeg.audio_transcoding_output_extension,
    ));
    terminal.log_newline();


    // Libraries
    terminal_print_group_header(terminal, "libraries");

    for (library_key, library) in &config.libraries {
        terminal.log_println(&format!(
            "{} ({})",
            format!(" => {}", library.name).bold(),
            library_key,
        ));

        let library_path = Path::new(&library.path);
        let library_path_exists = library_path.exists() && library_path.is_dir();

        terminal.log_println(format!(
            "    path = \"{}\"{}",
            library.path,
            match library_path_exists {
                true => {
                    " (exists)".green()
                }
                false => {
                    " (not found)".red()
                }
            }
        ));
        terminal.log_println(format!(
            "    ignored_directories_in_base_directory = {:?}",
            library
                .ignored_directories_in_base_directory
                .as_ref()
                .unwrap_or(&Vec::new())
        ));

        // `validation` sub-table
        terminal.log_println(format!("     => {}", "validation".italic()));
        terminal.log_println(format!(
            "        allowed_audio_file_extensions = {:?}",
            library.validation.allowed_audio_file_extensions,
        ));
        terminal.log_println(format!(
            "        allowed_other_file_extensions = {:?}",
            library.validation.allowed_other_file_extensions,
        ));
        terminal.log_println(format!(
            "        allowed_other_files_by_name = {:?}",
            library.validation.allowed_other_files_by_name,
        ));

        // `transcoding` sub-table
        terminal.log_println(format!("     => {}", "transcoding".italic()));
        terminal.log_println(format!(
            "        audio_file_extensions = {:?}",
            library.transcoding.audio_file_extensions,
        ));
        terminal.log_println(format!(
            "        other_file_extensions = {:?}",
            library.transcoding.other_file_extensions,
        ));

        terminal.log_newline();
    }


    // Aggregated library
    terminal_print_group_header(terminal, "aggregated_library");
    terminal.log_println(format!(
        "  path = {}",
        config.aggregated_library.path,
    ));
    terminal.log_println(format!(
        "  transcode_threads = {}",
        config.aggregated_library.transcode_threads,
    ));
    terminal.log_println(format!(
        "  failure_max_retries = {}",
        config.aggregated_library.failure_max_retries,
    ));
    terminal.log_println(format!(
        "  failure_delay_seconds = {}",
        config.aggregated_library.failure_delay_seconds,
    ));
}

/// Associated with the `list-libraries` command.
///
/// Prints the registered music libraries from the current configuration.
pub fn cmd_list_libraries(
    config: &Configuration,
    terminal: &mut SimpleTerminal,
) {
    terminal.log_println(format!(
        "Configuration file: {}",
        config.configuration_file_path.to_string_lossy(),
    ));
    terminal.log_newline();

    terminal.log_println(format!(
        "{} libraries are available:",
        config.libraries.len().to_string().bold()
    ));

    for (library_key, library) in &config.libraries {
        terminal.log_println(format!(
            "{} ({})",
            format!(" => {}", library.name).bold(),
            library_key,
        ));

        terminal.log_println(format!("    path = \"{}\"", library.path,));
        terminal.log_println(format!(
            "    ignored_directories_in_base_directory = {:?}",
            library
                .ignored_directories_in_base_directory
                .as_ref()
                .unwrap_or(&Vec::new())
        ));

        // `validation` sub-table
        terminal.log_println(format!("     => {}", "validation".italic()));
        terminal.log_println(format!(
            "        allowed_audio_file_extensions = {:?}",
            library.validation.allowed_audio_file_extensions,
        ));
        terminal.log_println(format!(
            "        allowed_other_file_extensions = {:?}",
            library.validation.allowed_other_file_extensions,
        ));
        terminal.log_println(format!(
            "        allowed_other_files_by_name = {:?}",
            library.validation.allowed_other_files_by_name,
        ));

        // `transcoding` sub-table
        terminal.log_println(format!("     => {}", "transcoding".italic()));
        terminal.log_println(format!(
            "        audio_file_extensions = {:?}",
            library.transcoding.audio_file_extensions,
        ));
        terminal.log_println(format!(
            "        other_file_extensions = {:?}",
            library.transcoding.other_file_extensions,
        ));

        terminal.log_newline();
    }
}
