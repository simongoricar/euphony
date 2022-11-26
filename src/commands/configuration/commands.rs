use crossterm::style::Stylize;
use crate::configuration::Config;
use crate::console::{LogBackend, TerminalBackend};


pub fn cmd_show_config<T: TerminalBackend + LogBackend>(
    config: &Config,
    terminal: &mut T,
) {
    terminal.log_println("⚙ Configuration ⚙");
    
    let configuration_file_path_str = config.configuration_file_path
        .to_string_lossy();
    
    terminal.log_println(format!(
        "(using {})",
        configuration_file_path_str
            .as_ref()
            .yellow()
            .italic()
    ));
    terminal.log_newline();
    terminal.log_newline();
    
    
    // Essentials
    terminal.log_println("-- essentials --");
    terminal.log_println(format!(
        "  base_library_path = {}",
        config.essentials.base_library_path,
    ));
    terminal.log_println(format!(
        "  base_tools_path = {}",
        config.essentials.base_tools_path,
    ));
    terminal.log_newline();
    
    
    // Tools
    terminal.log_println("-- tools --");
    terminal.log_println(format!(
        "  {}",
        "ffmpeg".italic()
    ));
    terminal.log_println(format!(
        "    binary = {}",
        config.tools.ffmpeg.binary,
    ));
    terminal.log_println(format!(
        "    to_mp3_v0_args = {:?}",
        config.tools.ffmpeg.to_mp3_v0_args,
    ));
    terminal.log_newline();
    
    
    // Validation
    terminal.log_println("-- validation --");
    
    terminal.log_println(format!(
        "  allowed_other_files_by_extension = {:?}",
        config.validation.allowed_other_files_by_extension,
    ));
    terminal.log_println(format!(
        "  allowed_other_files_by_name = {:?}",
        config.validation.allowed_other_files_by_name,
    ));
    terminal.log_newline();
    
    
    // Libraries
    terminal.log_println("-- libraries --");
    
    let library_count = config.libraries.len();
    terminal.log_println(format!(
        "({} available)",
        library_count
            .to_string()
            .bold()
    ));
    
    for (library_key, library) in &config.libraries {
        terminal.log_println(format!(
            "  {} ({}):",
            library.name,
            library_key,
        ));
        
        terminal.log_println(format!(
            "    path = {}",
            library.path,
        ));
        terminal.log_println(format!(
            "    allowed_audio_files_by_extension = {:?}",
            &library.allowed_audio_files_by_extension,
        ));
        terminal.log_println(format!(
            "    ignored_directories_in_base_dir = {}",
            match &library.ignored_directories_in_base_dir {
                Some(ignores) => format!("{:?}", ignores),
                None => String::from("[]"),
            },
        ));
        terminal.log_newline();
    }
    
    
    // Aggregated library
    terminal.log_println("-- aggregated_library --");
    terminal.log_println(format!(
        "  path = {}",
        config.aggregated_library.path,
    ));
}

pub fn cmd_list_libraries<T: TerminalBackend + LogBackend>(
    config: &Config,
    terminal: &mut T,
) {
    terminal.log_println("📔 Libraries 📔");
    
    terminal.log_println(format!(
        "(using {})",
        config.configuration_file_path
            .to_string_lossy()
            .as_ref()
            .yellow()
            .italic()
    ));
    terminal.log_newline();
    
    terminal.log_println(format!(
        "{} libraries are available:",
        config.libraries.len()
            .to_string()
            .bold()
    ));
    
    for (library_key, library) in &config.libraries {
        terminal.log_println(format!(
            "  {:>22} {}",
            format!("({})", library_key),
            library.name,
        ));
    }
}
