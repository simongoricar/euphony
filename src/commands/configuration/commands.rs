use crossterm::style::Stylize;

use crate::configuration::Config;
use crate::console::SimpleTerminalBackend;
use crate::console::utilities::term_println_stb;

/// Generic abstraction over `LogBackend::log_println` for printing headers.
fn terminal_print_group_header<S: AsRef<str>>(
    terminal: &dyn SimpleTerminalBackend,
    header: S,
) {
    const PAD_TO_WIDTH: usize = 10;
    
    let total_padding = PAD_TO_WIDTH.saturating_sub(header.as_ref().len());
    let left_padding = total_padding / 2;
    let right_padding = total_padding - left_padding;
    
    term_println_stb(
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
    let term_println = |content: &str| term_println_stb(terminal, content);
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
            "    base_library_path = {}",
            config.essentials.base_library_path,
        )
    );
    term_println(
        &format!(
            "    base_tools_path = {}",
            config.essentials.base_tools_path,
        )
    );
    term_newline();
    
    
    // Validation (basics)
    term_print_header("validation");
    term_println(
        &format!(
            "    extensions_considered_audio_files = {:?}",
            config.validation.extensions_considered_audio_files,
        )
    );
    
    
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
    
    
    // Libraries
    term_print_header("libraries");
    
    for (library_key, library) in &config.libraries {
        term_println(
            &format!(
                "{} ({})",
                format!(" => {}", library.name).bold(),
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
                "    ignored_directories_in_base_directory = {:?}",
                library.ignored_directories_in_base_directory
                    .as_ref()
                    .unwrap_or(&Vec::new())
            )
        );
        
        // `validation` sub-table
        term_println(
            &format!(
                "     => {}",
                "validation".italic()
            )
        );
        term_println(
            &format!(
                "        allowed_audio_file_extensions = {:?}",
                library.validation.allowed_audio_file_extensions,
            )
        );
        term_println(
            &format!(
                "        allowed_other_file_extensions = {:?}",
                library.validation.allowed_other_file_extensions,
            )
        );
        term_println(
            &format!(
                "        allowed_other_files_by_name = {:?}",
                library.validation.allowed_other_files_by_name,
            )
        );
        
        // `transcoding` sub-table
        term_println(
            &format!(
                "     => {}",
                "transcoding".italic()
            )
        );
        term_println(
            &format!(
                "        audio_file_extensions = {:?}",
                library.transcoding.audio_file_extensions,
            )
        );
        term_println(
            &format!(
                "        other_file_extensions = {:?}",
                library.transcoding.other_file_extensions,
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
    term_println(
        &format!(
            "  transcode_threads = {}",
            config.aggregated_library.transcode_threads,
        )
    );
    term_println(
        &format!(
            "  failure_max_retries = {}",
            config.aggregated_library.failure_max_retries,
        )
    );
    term_println(
        &format!(
            "  failure_delay_seconds = {}",
            config.aggregated_library.failure_delay_seconds,
        )
    );
}

pub fn cmd_list_libraries(
    config: &Config,
    terminal: &mut dyn SimpleTerminalBackend,
) {
    // Binds a few short functions to the current terminal,
    // allowing for a zero- or single-argument calls that print the header, a simple log line, etc.
    let term_println = |content: &str| term_println_stb(terminal, content);
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
                "{} ({})",
                format!(" => {}", library.name).bold(),
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
                "    ignored_directories_in_base_directory = {:?}",
                library.ignored_directories_in_base_directory
                    .as_ref()
                    .unwrap_or(&Vec::new())
            )
        );
        
        // `validation` sub-table
        term_println(
            &format!(
                "     => {}",
                "validation".italic()
            )
        );
        term_println(
            &format!(
                "        allowed_audio_file_extensions = {:?}",
                library.validation.allowed_audio_file_extensions,
            )
        );
        term_println(
            &format!(
                "        allowed_other_file_extensions = {:?}",
                library.validation.allowed_other_file_extensions,
            )
        );
        term_println(
            &format!(
                "        allowed_other_files_by_name = {:?}",
                library.validation.allowed_other_files_by_name,
            )
        );
        
        // `transcoding` sub-table
        term_println(
            &format!(
                "     => {}",
                "transcoding".italic()
            )
        );
        term_println(
            &format!(
                "        audio_file_extensions = {:?}",
                library.transcoding.audio_file_extensions,
            )
        );
        term_println(
            &format!(
                "        other_file_extensions = {:?}",
                library.transcoding.other_file_extensions,
            )
        );
        
        term_newline();
    }
}
