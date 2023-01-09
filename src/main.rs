use std::ops::DerefMut;
use std::path::PathBuf;
use std::process::exit;

use clap::{Args, Parser, Subcommand};
use crossterm::style::Stylize;
use miette::Result;

use crate::configuration::Config;
use crate::console::{TerminalBackend, AdvancedTranscodeTerminalBackend, LogToFileBackend};
use crate::console::backends::{BareTerminalBackend, TUITerminalBackend};
use crate::console::utilities::{term_println_attb, term_println_fvb};
use crate::globals::VERBOSE;

mod configuration;
mod filesystem;
mod commands;
mod cached;
mod globals;
mod console;


#[derive(Subcommand, PartialEq, Eq)]
enum CLICommand {
    #[command(
        name = "transcode",
        visible_aliases = ["transcode-collection"],
        about = "Transcode all libraries into the aggregated library."
    )]
    TranscodeAll(TranscodeAllArgs),

    #[command(
        name = "validate",
        visible_aliases = ["validate-collection"],
        about = "Validate all the available libraries for inconsistencies, such as \
                 forbidden files, any inter-library collisions that would cause problems \
                 when transcoding, etc."
    )]
    ValidateAll(ValidateAllArgs),

    #[command(
        name = "show-config",
        about = "Loads, validates and prints the current configuration."
    )]
    ShowConfig,

    #[command(
        name = "list-libraries",
        about = "List all the registered libraries registered in the configuration."
    )]
    ListLibraries,
}

#[derive(Args, Eq, PartialEq)]
struct TranscodeAllArgs {
    #[arg(
        long = "bare-terminal",
        help = "Whether to disable any fancy terminal UI and simply print into the console. \
                Keep in mind that this is a really bare version without any progress bars, but \
                can be useful for debugging or for cases where you simply don't want \
                a constantly-updating terminal UI (e.g. for saving logs)."
    )]
    bare_terminal: bool,
    
    #[arg(
        long = "log-to-file",
        help = "Path to the log file. If this is unset, no logs are saved."
    )]
    log_to_file: Option<String>,
}

#[derive(Args, Eq, PartialEq)]
struct ValidateAllArgs {
    #[arg(
        long = "log-to-file",
        help = "Path to the log file. If this is unset, no logs are saved."
    )]
    log_to_file: Option<String>,
}

#[derive(Parser)]
#[command(
    name = "euphony",
    author = "Simon G. <simon.peter.goricar@gmail.com>",
    about = "An opinionated music library transcode manager.",
    long_about = "Euphony is an opinionated music library transcode manager that allows the user to \
                  retain high quality audio files in multiple separate libraries while also \
                  helping to transcode their collection into a smaller format (MP3 V0). That smaller \
                  version of the library can then be used on portable devices or similar occasions where space has a larger impact. \
                  For more info, see the README file in the repository.",
    version
)]
struct CLIArgs {
    #[arg(
        short = 'c',
        long = "config",
        global = true,
        help = "Optionally a path to your configuration file. Without this option, \
                euphony tries to load ./data/configuration.toml, but understandably this \
                might not always be the most convenient location."
    )]
    config: Option<String>,

    #[arg(
        short = 'v',
        long = "verbose",
        global = true,
        help = "Increase the verbosity of output."
    )]
    verbose: bool,

    #[command(subcommand)]
    command: CLICommand,
}

/// Load and return the configuration, given the command line arguments
/// (-c/--config can override configuration filepath).
fn get_configuration(args: &CLIArgs) -> Config {
    if args.config.is_some() {
        Config::load_from_path(args.config.clone().unwrap())
    } else {
        Config::load_default_path()
    }
}

/// Initializes and returns a boxed terminal backend.
/// If `use_bare` is true, this will return `BareConsoleBackend`, otherwise `TUITerminalBackend`.
///
/// `TUITerminalBackend` has a better and dynamic terminal UI, but is harder to debug or properly log still down.
/// `BareConsoleBackend` is a bare-bones backend that simply linearly logs all activity to the console.
fn get_terminal_backend(
    use_bare: bool
) -> Box<dyn AdvancedTranscodeTerminalBackend> {
    if use_bare {
        Box::new(BareTerminalBackend::new())
    } else {
        Box::new(TUITerminalBackend::new().expect("Could not create TUI terminal backend."))
    }
}

/// Initializes the required terminal backend and executes the given CLI subcommand.
fn process_cli_command(
    args: CLIArgs,
    config: &Config,
) -> std::result::Result<(), i32> {
    if let CLICommand::TranscodeAll(transcode_args) = args.command {
        // `transcode`/`transcode-all` has two available terminal backends:
        // - the fancy one uses `tui` for a full-fledged terminal UI with progress bars and multiple "windows",
        // - the bare one (enabled with --bare-terminal) is a simple console echo implementation (no progress bars, etc.).
        let mut terminal = get_terminal_backend(transcode_args.bare_terminal);
        terminal.setup()
            .expect("Could not set up tui terminal backend.");
        
        if let Some(log_file_path) = transcode_args.log_to_file {
            terminal.enable_saving_logs_to_file(PathBuf::from(log_file_path))
                .map_err(|_| 1)?;
        }
        
        match commands::cmd_transcode_all(config, terminal.deref_mut()) {
            Ok(final_message) => {
                term_println_attb(
                    terminal.deref_mut(),
                    final_message,
                );
    
                terminal
                    .destroy()
                    .expect("Could not destroy tui terminal backend.");
                
                Ok(())
            },
            Err(error) => {
                term_println_attb(
                    terminal.deref_mut(),
                    error.to_string().red(),
                );
    
                terminal
                    .destroy()
                    .expect("Could not destroy tui terminal backend.");
                
                Err(1)
            }
        }
    } else if let CLICommand::ValidateAll(args) = args.command {
        let mut bare_terminal = BareTerminalBackend::new();
    
        bare_terminal.setup()
            .expect("Could not set up bare console backend.");
        
        if let Some(log_file_path) = args.log_to_file {
            bare_terminal.enable_saving_logs_to_file(PathBuf::from(log_file_path))
                .map_err(|_| 1)?;
        }
        
        match commands::cmd_validate_all(config, &mut bare_terminal) {
            Ok(_) => {}
            Err(error) => {
                term_println_fvb(
                    &bare_terminal,
                    format!(
                        "{}: {}",
                        "Something went wrong while validating:".red(),
                        error,
                    ),
                );
            }
        };
        bare_terminal.destroy()
            .expect("Could not destroy bare console backend.");
    
        Ok(())
        
    } else if args.command == CLICommand::ShowConfig {
        let mut bare_terminal = BareTerminalBackend::new();
        
        bare_terminal.setup()
            .expect("Could not set up bare console backend.");
        commands::cmd_show_config(config, &mut bare_terminal);
        bare_terminal.destroy()
            .expect("Could not destroy bare console backend.");
        
        Ok(())
        
    } else if args.command == CLICommand::ListLibraries {
        let mut bare_terminal = BareTerminalBackend::new();
    
        bare_terminal.setup()
            .expect("Could not set up bare console backend.");
        commands::cmd_list_libraries(config, &mut bare_terminal);
        bare_terminal.destroy()
            .expect("Could not destroy bare console backend.");
    
        Ok(())
        
    } else {
        panic!("Unrecognized command!");
    }
}

/// Entry function for `euphony`. Parses CLI arguments,
/// loads the configuration file and starts executing the given subcommand.
fn main() -> Result<()> {
    // TODO .album.euphony should have a version lock inside it
    let args: CLIArgs = CLIArgs::parse();
    VERBOSE.set(args.verbose);
    
    let configuration = get_configuration(&args);
    
    match process_cli_command(args, &configuration) {
        Ok(_) => {
            exit(0)
        },
        Err(exit_code) => {
            exit(exit_code)
        }
    };
}
