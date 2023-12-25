use std::path::PathBuf;
use std::process::exit;
use std::thread;
use std::thread::Scope;

use clap::{Args, Parser, Subcommand};
use crossterm::style::Stylize;
use euphony_configuration::Configuration;
use miette::{miette, Context, Result};

use crate::console::frontends::terminal_ui::terminal::FancyTerminalBackend;
use crate::console::frontends::{
    BareTerminalBackend,
    SimpleTerminal,
    TranscodeTerminal,
    ValidationTerminal,
};
use crate::console::{LogBackend, LogToFileBackend, TerminalBackend};
use crate::globals::VERBOSE;

mod cancellation;
mod commands;
mod console;
mod globals;

pub const EUPHONY_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(PartialEq, Eq)]
#[derive(Subcommand)]
enum CLICommand {
    #[command(
        name = "transcode",
        visible_aliases(["transcode-collection"]),
        about = "Transcode all libraries into the aggregated library."
    )]
    TranscodeAll(TranscodeAllArgs),

    #[command(
        name = "validate",
        visible_aliases(["validate-collection"]),
        about = "Validate all the available libraries for inconsistencies, such as forbidden files, \
                 any inter-library collisions that would cause problems when transcoding, etc."
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
    log_to_file: Option<PathBuf>,
}

#[derive(Args, Eq, PartialEq)]
struct ValidateAllArgs {
    #[arg(
        long = "log-to-file",
        help = "Path to the log file. If this is unset, no logs are saved."
    )]
    log_to_file: Option<PathBuf>,
}

#[derive(Parser)]
#[command(
    name = "euphony",
    author = "Simon G. <simon.peter.goricar@gmail.com>",
    about = "An opinionated music library transcode manager.",
    long_about = "Euphony is a music library transcode manager that allows the user to \
                  retain high quality audio files in one or more libraries while also \
                  helping to transcode their collection into a smaller format. That smaller \
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
                euphony tries to load ./data/configuration.toml (relative to the binary), \
                but understandably this might not always be the most convenient location."
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
/// (`-c`/`--config` can override the load path).
fn get_configuration(args: &CLIArgs) -> Result<Configuration> {
    if args.config.is_some() {
        Configuration::load_from_path(args.config.clone().unwrap())
    } else {
        Configuration::load_default_path()
    }
}

/// Initializes and returns a terminal backend for transcoding.
/// If `use_bare` is true, this will return `BareConsoleBackend`, otherwise `TUITerminalBackend`.
///
/// `FancyTerminalBackend` has a better and dynamic terminal UI, but is harder to debug non-UI bugs.
///
/// `BareConsoleBackend` is a bare-bones backend that simply linearly logs all activity to the console,
/// making it much easier to track down bugs or parse output in some other program.
fn get_transcode_terminal<'scope>(
    config: &Configuration,
    use_bare_terminal: bool,
) -> TranscodeTerminal<'_, 'scope> {
    if use_bare_terminal {
        BareTerminalBackend::new().into()
    } else {
        FancyTerminalBackend::new(config)
            .expect("Could not create fancy terminal UI backend.")
            .into()
    }
}

/// Initializes the required terminal backend and executes the given CLI command.
fn run_requested_cli_command<'config: 'scope, 'scope, 'scope_env: 'scope>(
    args: CLIArgs,
    config: &'config Configuration,
    scope: &'scope Scope<'scope, 'scope_env>,
) -> Result<()> {
    if let CLICommand::TranscodeAll(transcode_args) = args.command {
        // `transcode`/`transcode-all` has two available terminal frontends:
        // - the fancy one uses `ratatui` for a full-fledged terminal UI with progress bars and multiple "windows",
        // - the bare one (enabled with --bare-terminal) is a simple console echo implementation (no progress bars, etc.).
        let terminal =
            get_transcode_terminal(config, transcode_args.bare_terminal);

        if let Some(log_file_path) = transcode_args
            .log_to_file
            .or_else(|| config.logging.default_log_output_path.clone())
        {
            terminal
                .enable_saving_logs_to_file(log_file_path, scope)
                .wrap_err_with(|| {
                    miette!("Failed to enable logging to disk.")
                })?;
        }

        terminal.setup(scope).wrap_err_with(|| {
            miette!("Failed to set up terminal UI backend.")
        })?;


        let result = commands::cmd_transcode_all(config, &terminal)
            .wrap_err_with(|| {
                miette!("Failed to execute transcode command to completion.")
            });
        if let Err(error) = result {
            terminal.log_println(format!("{error}").dark_red());
        }


        terminal.destroy().wrap_err_with(|| {
            miette!("Failed to destroy terminal UI backend.")
        })?;

        Ok(())
    } else if let CLICommand::ValidateAll(args) = args.command {
        let mut terminal: ValidationTerminal = BareTerminalBackend::new().into();

        if let Some(log_file_path) = args
            .log_to_file
            .or_else(|| config.logging.default_log_output_path.clone())
        {
            terminal
                .enable_saving_logs_to_file(log_file_path, scope)
                .wrap_err_with(|| {
                    miette!("Failed to enable logging to disk.")
                })?;
        }

        terminal.setup(scope).wrap_err_with(|| {
            miette!("Failed to set up terminal UI backend.")
        })?;



        let result = commands::cmd_validate(config, &mut terminal)
            .wrap_err_with(|| {
                miette!("Failed to execute transcode command to completion.")
            });

        match result {
            Ok(_) => {}
            Err(error) => {
                terminal.log_println(format!(
                    "{}: {}",
                    "Something went wrong while validating:".red(),
                    error,
                ));
            }
        };


        terminal.destroy().wrap_err_with(|| {
            miette!("Failed to destroy terminal UI backend.")
        })?;

        Ok(())
    } else if args.command == CLICommand::ShowConfig {
        let mut terminal: SimpleTerminal = BareTerminalBackend::new().into();

        terminal.setup(scope).wrap_err_with(|| {
            miette!("Failed to set up terminal UI backend.")
        })?;


        commands::cmd_show_config(config, &mut terminal);


        terminal.destroy().wrap_err_with(|| {
            miette!("Failed to destroy terminal UI backend.")
        })?;

        Ok(())
    } else if args.command == CLICommand::ListLibraries {
        let mut terminal: SimpleTerminal = BareTerminalBackend::new().into();

        terminal.setup(scope).wrap_err_with(|| {
            miette!("Failed to set up terminal UI backend.")
        })?;


        commands::cmd_list_libraries(config, &mut terminal);


        terminal.destroy().wrap_err_with(|| {
            miette!("Failed to destroy terminal UI backend.")
        })?;

        Ok(())
    } else {
        panic!("Unrecognized command!");
    }
}

/// Entry function for `euphony`.
///
/// Parses CLI arguments, loads the configuration file and starts executing the requested command.
fn main() -> Result<()> {
    let args = CLIArgs::parse();
    VERBOSE.set(args.verbose);

    let configuration = get_configuration(&args)
        .wrap_err_with(|| miette!("Could not load configuration."))?;

    thread::scope(|scope| {
        let command_result =
            run_requested_cli_command(args, &configuration, scope);

        match command_result {
            Ok(_) => exit(0),
            Err(error) => {
                eprintln!("{:?}", error);
                exit(1);
            }
        };
    });

    Ok(())
}
