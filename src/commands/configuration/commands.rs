use crossterm::style::Stylize;

use crate::configuration::Config;
use crate::console::LogTerminalBackend;
use crate::console::utilities::term_println_lt;

pub fn cmd_show_config(
    config: &Config,
    terminal: &mut dyn LogTerminalBackend,
) {
    term_println_lt(terminal, "⚙ Configuration ⚙");
    
    let configuration_file_path_str = config.configuration_file_path
        .to_string_lossy();
    
    term_println_lt(
        terminal,
        format!(
            "(using {})",
            configuration_file_path_str
                .as_ref()
                .yellow()
                .italic()
        )
    );
    terminal.log_newline();
    terminal.log_newline();
    
    
    // Essentials
    term_println_lt(terminal, "-- essentials --");
    term_println_lt(
        terminal,
        format!(
            "  base_library_path = {}",
            config.essentials.base_library_path,
        )
    );
    term_println_lt(
        terminal,
        format!(
            "  base_tools_path = {}",
            config.essentials.base_tools_path,
        )
    );
    terminal.log_newline();
    
    
    // Tools
    term_println_lt(terminal, "-- tools --");
    term_println_lt(
        terminal,
        format!(
            "  {}",
            "ffmpeg".italic()
        )
    );
    term_println_lt(
        terminal,
        format!(
            "    binary = {}",
            config.tools.ffmpeg.binary,
        )
    );
    term_println_lt(
        terminal,
        format!(
            "    to_mp3_v0_args = {:?}",
            config.tools.ffmpeg.to_mp3_v0_args,
        )
    );
    terminal.log_newline();
    
    
    // Validation
    term_println_lt(terminal, "-- validation --");
    
    term_println_lt(
        terminal,
        format!(
            "  allowed_other_files_by_extension = {:?}",
            config.validation.allowed_other_files_by_extension,
        )
    );
    term_println_lt(
        terminal,
        format!(
            "  allowed_other_files_by_name = {:?}",
            config.validation.allowed_other_files_by_name,
        )
    );
    terminal.log_newline();
    
    
    // Libraries
    term_println_lt(terminal, "-- libraries --");
    
    let library_count = config.libraries.len();
    term_println_lt(
        terminal,
        format!(
            "({} available)",
            library_count
                .to_string()
                .bold()
        )
    );
    
    for (library_key, library) in &config.libraries {
        term_println_lt(
            terminal,
            format!(
                "  {} ({}):",
                library.name,
                library_key,
            )
        );
    
        term_println_lt(
            terminal,
            format!(
                "    path = {}",
                library.path,
            )
        );
        term_println_lt(
            terminal,
            format!(
                "    allowed_audio_files_by_extension = {:?}",
                &library.allowed_audio_files_by_extension,
            )
        );
        term_println_lt(
            terminal,
            format!(
                "    ignored_directories_in_base_dir = {}",
                match &library.ignored_directories_in_base_dir {
                    Some(ignores) => format!("{:?}", ignores),
                    None => String::from("[]"),
                },
            )
        );
        terminal.log_newline();
    }
    
    
    // Aggregated library
    term_println_lt(terminal, "-- aggregated_library --");
    term_println_lt(
        terminal,
        format!(
            "  path = {}",
            config.aggregated_library.path,
        )
    );
}

pub fn cmd_list_libraries(
    config: &Config,
    terminal: &mut dyn LogTerminalBackend,
) {
    term_println_lt(terminal, "📔 Libraries 📔");
    
    term_println_lt(
        terminal,
        format!(
            "(using {})",
            config.configuration_file_path
                .to_string_lossy()
                .as_ref()
                .yellow()
                .italic()
        )
    );
    terminal.log_newline();
    
    term_println_lt(
        terminal,
        format!(
            "{} libraries are available:",
            config.libraries.len()
                .to_string()
                .bold()
        )
    );
    
    for (library_key, library) in &config.libraries {
        term_println_lt(
            terminal,
            format!(
                "  {:>22} {}",
                format!("({})", library_key),
                library.name,
            )
        );
    }
}
