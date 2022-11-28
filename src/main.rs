use std::ops::DerefMut;
use std::process::exit;

use clap::{Args, Parser, Subcommand};
use crossterm::style::Stylize;
use miette::Result;

use crate::configuration::Config;
use crate::console::{TerminalBackend, TranscodeLogTerminalBackend};
use crate::console::backends::{BareConsoleBackend, TUITerminalBackend};
use crate::console::utilities::term_println_tltb;
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
        visible_aliases = ["transcode-all"],
        about = "Transcode all registered libraries into the aggregated (transcoded) library."
    )]
    TranscodeAll(TranscodeAllArgs),
    
    // TODO Reimplement transcode-library and transcode-album with the new terminal backend.

    #[command(
        name = "validate",
        visible_aliases = ["validate-all"],
        about = "Validate all the available (sub)libraries for inconsistencies, such as \
                 forbidden files, any inter-library collisions that would cause problems \
                 when aggregating (transcoding), etc."
    )]
    ValidateAll,

    #[command(
        name = "validate-library",
        about = "Validate a specific library for inconsistencies, such as forbidden files."
    )]
    ValidateLibrary(ValidateLibraryArgs),

    #[command(
        name = "show-config",
        about = "Loads, validates and prints the current configuration from `./data/configuration.toml`."
    )]
    ShowConfig,

    #[command(
        name = "list-libraries",
        about = "List all the registered libraries."
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
}

#[derive(Args, Eq, PartialEq)]
struct ValidateLibraryArgs {
    #[arg(
        help = "Library to process (by full name)."
    )]
    library_name: String,
}

#[derive(Parser)]
#[command(
    name = "euphony",
    author = "Simon G. <simon.peter.goricar@gmail.com>",
    about = "An opinionated music library transcode manager.",
    long_about = "Euphony is an opinionated music library transcode manager that allows the user to \
                  retain high quality audio files in multiple separate libraries while also enabling \
                  the listener to transcode their library with ease into a smaller format (MP3 V0) \
                  to take with them on the go. For more info, see the README file in the repository.",
    version
)]
struct CLIArgs {
    #[arg(
        short = 'c',
        long = "config",
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
fn get_terminal_backend(
    use_bare: bool
) -> Box<dyn TranscodeLogTerminalBackend> {
    if use_bare {
        Box::new(BareConsoleBackend::new())
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
        
        match commands::cmd_transcode_all(config, terminal.deref_mut()) {
            Ok(_) => {
                terminal.log_newline();
                term_println_tltb(terminal.deref_mut(), "Transcoding finished.".green().italic());
    
                terminal
                    .destroy()
                    .expect("Could not destroy tui terminal backend.");
                
                Ok(())
            },
            Err(error) => {
                terminal.log_newline();
                term_println_tltb(
                    terminal.deref_mut(),
                    format!(
                        "{} {}",
                        "Errored while transcoding:".red(),
                        error,
                    )
                );
    
                terminal
                    .destroy()
                    .expect("Could not destroy tui terminal backend.");
                
                Err(1)
            }
        }
    } else if args.command == CLICommand::ValidateAll {
        let mut bare_terminal = BareConsoleBackend::new();
    
        bare_terminal.setup()
            .expect("Could not set up bare console backend.");
        commands::cmd_validate_all(config, &mut bare_terminal);
        bare_terminal.destroy()
            .expect("Could not destroy bare console backend.");
    
        Ok(())
        
    } else if let CLICommand::ValidateLibrary(validation_args) = args.command {
        let mut bare_terminal = BareConsoleBackend::new();
    
        bare_terminal.setup()
            .expect("Could not set up bare console backend.");
        commands::cmd_validate_library(config, validation_args.library_name, &mut bare_terminal);
        bare_terminal.destroy()
            .expect("Could not destroy bare console backend.");
    
        Ok(())
        
    } else if args.command == CLICommand::ShowConfig {
        let mut bare_terminal = BareConsoleBackend::new();
        
        bare_terminal.setup()
            .expect("Could not set up bare console backend.");
        commands::cmd_show_config(config, &mut bare_terminal);
        bare_terminal.destroy()
            .expect("Could not destroy bare console backend.");
        
        Ok(())
        
    } else if args.command == CLICommand::ListLibraries {
        let mut bare_terminal = BareConsoleBackend::new();
    
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
