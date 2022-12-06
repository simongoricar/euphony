use crossterm::style::Stylize;

use crate::configuration::Config;
use crate::console::SimpleTerminalBackend;
use crate::console::utilities::term_println_ltb;

/// Generic abstraction over `LogBackend::log_println` for printing headers.
#[inline]
fn terminal_print_group_header<S: AsRef<str>>(
    terminal: &dyn SimpleTerminalBackend,
    header: S,
) {
    const PAD_TO_WIDTH: usize = 10;
    
    let total_padding = PAD_TO_WIDTH.saturating_sub(header.as_ref().len());
    let left_padding = total_padding / 2;
    let right_padding = total_padding - left_padding;
    
    term_println_ltb(
        terminal,
        format!(
            "|----- {}{:^12}{} -----|",
            " ".repeat(left_padding),
            header.as_ref().bold(),
            " ".repeat(right_padding)
        ),
    );
}

pub fn cmd_show_config(
    config: &Config,
    terminal: &mut dyn SimpleTerminalBackend,
) {
    // Binds a few short functions to the current terminal,
    // allowing for a zero- or single-argument calls that print the header, a simple log line, etc.
    let term_print_header = |content: &str| terminal_print_group_header(terminal, content);
    let term_println = |content: &str| term_println_ltb(terminal, content);
    let term_newline = || terminal.log_newline();
    
    term_println(
        &format!(
            "Configuration file: {}",
            config.configuration_file_path.to_string_lossy(),
        )
    );
    term_newline();
    
    // Essentials
    term_print_header("essentials");
    term_println(
        &format!(
            "  base_library_path = {}",
            config.essentials.base_library_path,
        )
    );
    term_println(
        &format!(
            "  base_tools_path = {}",
            config.essentials.base_tools_path,
        )
    );
    term_newline();
    
    
    // Tools
    term_print_header("tools");
    term_println(
        &format!(
            " => {}",
            "ffmpeg".bold()
        )
    );
    term_println(
        &format!(
            "    binary = {}",
            config.tools.ffmpeg.binary,
        )
    );
    term_println(
        &format!(
            "    to_mp3_v0_args = {:?}",
            config.tools.ffmpeg.to_mp3_v0_args,
        )
    );
    term_newline();
    
    
    // Validation
    term_print_header("validation");
    term_println(
        &format!(
            "  allowed_other_files_by_extension = {:?}",
            config.validation.allowed_other_files_by_extension,
        )
    );
    term_println(
        &format!(
            "  allowed_other_files_by_name = {:?}",
            config.validation.allowed_other_files_by_name,
        )
    );
    term_newline();
    
    
    // Libraries
    term_print_header("libraries");
    
    for (library_key, library) in &config.libraries {
        term_println(
            &format!(
                " => {} ({})",
                library.name.clone().bold(),
                library_key,
            )
        );
    
        term_println(
            &format!(
                "    path = \"{}\"",
                library.path,
            )
        );
        term_println(
            &format!(
                "    allowed_audio_files_by_extension = {:?}",
                &library.allowed_audio_files_by_extension,
            )
        );
        term_println(
            &format!(
                "    ignored_directories_in_base_dir = {}",
                match &library.ignored_directories_in_base_dir {
                    Some(ignores) => format!("{:?}", ignores),
                    None => String::from("[]"),
                },
            )
        );
        term_newline();
    }
    
    
    // Aggregated library
    term_print_header("aggregated_library");
    term_println(
        &format!(
            "  path = {}",
            config.aggregated_library.path,
        )
    );
}

pub fn cmd_list_libraries(
    config: &Config,
    terminal: &mut dyn SimpleTerminalBackend,
) {
    // Binds a few short functions to the current terminal,
    // allowing for a zero- or single-argument calls that print the header, a simple log line, etc.
    let term_println = |content: &str| term_println_ltb(terminal, content);
    let term_newline = || terminal.log_newline();
    
    term_println(
        &format!(
            "Configuration file: {}",
            config.configuration_file_path.to_string_lossy(),
        )
    );
    term_newline();
    
    term_println(
        &format!(
            "{} libraries are available:",
            config.libraries.len()
                .to_string()
                .bold()
        )
    );
    
    for (library_key, library) in &config.libraries {
        term_println(
            &format!(
                " - {} ({})",
                library.name.clone().bold(),
                library_key,
            )
        );
    
        term_println(
            &format!(
                "    path = \"{}\"",
                library.path,
            )
        );
        term_println(
            &format!(
                "    allowed_audio_files_by_extension = {:?}",
                &library.allowed_audio_files_by_extension,
            )
        );
        term_println(
            &format!(
                "    ignored_directories_in_base_dir = {}",
                match &library.ignored_directories_in_base_dir {
                    Some(ignores) => format!("{:?}", ignores),
                    None => String::from("[]"),
                },
            )
        );
        term_newline();
    }
}
