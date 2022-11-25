use std::process::exit;

use clap::{Args, Parser, Subcommand};
use crossterm::style::Stylize;
use miette::Result;

use configuration::Config;
use crate::console_backends::{BareConsoleBackend, LogBackend, TerminalBackend, TUITerminalBackend};
use crate::globals::VERBOSE;

mod configuration;
mod filesystem;
mod commands;
mod cached;
mod globals;
mod observer;
mod console_backends;


#[derive(Subcommand, PartialEq, Eq)]
enum CLICommand {
    #[command(
        name = "transcode",
        visible_aliases = ["transcode-all"],
        about = "Transcode all registered libraries into the aggregated (transcoded) library."
    )]
    TranscodeAll,

    #[command(
        name = "transcode-library",
        about = "Transcode only the specified library into the aggregated (transcoded) library. \
                 Requires a single positional parameter: the library name (by full name), \
                 as configured in the configuration file."
    )]
    TranscodeLibrary(TranscodeLibraryArgs),

    #[command(
        name = "transcode-album",
        about = "Transcode only the specified album into the aggregated (transcoded) library. \
                 The current directory is used by default, but you may pass a different one \
                 using \"--dir <path>\"."
    )]
    TranscodeAlbum(TranscodeAlbumArgs),

    #[command(
        name = "validate-all",
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

#[derive(Args, PartialEq, Eq)]
struct TranscodeAlbumArgs {
    #[arg(
        long = "dir",
        help = "Directory to process, defaults to current directory."
    )]
    directory: Option<String>,
}

#[derive(Args, PartialEq, Eq)]
struct TranscodeLibraryArgs {
    #[arg(
        help = "Library to process (by full name)."
    )]
    library_name: String,
}

#[derive(Args, PartialEq, Eq)]
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

fn get_configuration(args: &CLIArgs) -> Config {
    if args.config.is_some() {
        Config::load_from_path(args.config.clone().unwrap())
    } else {
        Config::load_default_path()
    }
}

fn process_cli_command(
    args: CLIArgs,
    config: &Config,
) -> std::result::Result<(), i32> {
    if args.command == CLICommand::TranscodeAll {
        let mut tui_terminal = TUITerminalBackend::new()
            .expect("Could not create tui terminal backend.");
        
        tui_terminal.setup()
            .expect("Could not set up tui terminal backend.");
        
        match commands::cmd_transcode_all(config, &mut tui_terminal) {
            Ok(_) => {
                tui_terminal.log_newline();
                tui_terminal.log_println("Transcoding finished.".green().italic());
                
                tui_terminal.destroy()
                    .expect("Could not destroy tui terminal backend.");
                
                Ok(())
            },
            Err(error) => {
                tui_terminal.log_newline();
                tui_terminal.log_println(format!(
                    "{} {}",
                    "Errored while transcoding:".red(),
                    error,
                ));
    
                tui_terminal.destroy()
                    .expect("Could not destroy tui terminal backend.");
                
                Err(1)
            }
        }
    } else if args.command == CLICommand::ShowConfig {
        let mut bare_terminal = BareConsoleBackend::new();
        
        bare_terminal.setup()
            .expect("Could not set up bare console backend.");
        
        commands::cmd_show_config(config, &mut bare_terminal);
        
        bare_terminal.destroy()
            .expect("Could not destroy bare console backend.");
        
        Ok(())
        
    } else {
        // TODO Other commands.
        todo!("Unimplemented!");
    }
}

fn main() -> Result<()> {
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
    
    /*
    if args.command == CLICommand::TranscodeAll {
        match commands::cmd_transcode_all(&configuration, &mut terminal_backend) {
            Ok(_) => {
                console.newline()?;
                console.println_styled("Transcoding completed.".green().italic())?;
                
                Ok(())
                
                // console::new_line();
                // console::horizontal_line_with_text(
                //     format!(
                //         "{}",
                //         style("Full aggregation complete.")
                //             .green()
                //             .italic()
                //             .bold()
                //     ),
                //     None, None,
                // );
            },
            Err(error) => {
                console.newline()?;
                console.println_styled("Errored while transcoding:".red().bold())?;
                console.println(error)?;
                
                // console::new_line();
                // console::horizontal_line_with_text(
                //     format!(
                //         "{}",
                //         style("Errors in full aggregation!")
                //             .red()
                //             .italic()
                //             .bold()
                //     ),
                //     None, None,
                // );
                // console::centered_print(
                //     format!(
                //         "{}",
                //         style(error)
                //             .red()
                //     ),
                //     None,
                // );
                
                Ok(())
            }
        }

    } else if let CLICommand::TranscodeAlbum(ta_args) = &args.command {
        let selected_directory = match &ta_args.directory {
            Some(dir) => path::PathBuf::from(dir),
            None => {
                env::current_dir()
                    .expect("Could not get current directory!")
            }
        };

        match commands::cmd_transcode_album(&selected_directory, &get_configuration(&args)) {
            Ok(_) => {
                // console::new_line();
                // console::horizontal_line_with_text(
                //     format!(
                //         "{}",
                //         style("Album aggregation complete.")
                //             .green()
                //             .italic()
                //             .bold()
                //     ),
                //     None, None,
                // );
                
                Ok(())
            },
            Err(error) => {
                // console::new_line();
                // console::horizontal_line_with_text(
                //     format!(
                //         "{}",
                //         style("Errors in album aggregation!")
                //             .red()
                //             .italic()
                //             .bold()
                //     ),
                //     None, None,
                // );
                // console::centered_print(
                //     format!(
                //         "{}",
                //         style(error)
                //             .red()
                //     ),
                //     None,
                // );

                exit(1);
            }
        }

    } else if let CLICommand::TranscodeLibrary(tl_args) = &args.command {
        let config = get_configuration(&args);

        let selected_library = match config.get_library_by_full_name(&tl_args.library_name) {
            Some(library) => library,
            None => {
                // eprintln!(
                //     "{} {}",
                //     style("No such library:")
                //         .red(),
                //     tl_args.library_name,
                // );
                exit(1);
            }
        };

        let selected_library_path = PathBuf::from(&selected_library.path);

        match commands::cmd_transcode_library(&selected_library_path, &config) {
            Ok(_) => {
                // console::new_line();
                // console::horizontal_line_with_text(
                //     format!(
                //         "{}",
                //         style("Library aggregation complete.")
                //             .green()
                //             .italic()
                //             .bold()
                //     ),
                //     None, None,
                // );
                
                Ok(())
            },
            Err(error) => {
                // eprintln!(
                //     "{} {}",
                //     style("Error while transcoding library:")
                //         .red(),
                //     error,
                // );
                exit(1);
            }
        }

    } else if args.command == CLICommand::ValidateAll {
        match commands::cmd_validate_all(&get_configuration(&args)) {
            true => exit(0),
            false => exit(1),
        }

    } else if let CLICommand::ValidateLibrary(vl_args) = &args.command {
        match commands::cmd_validate_library(&get_configuration(&args), &vl_args.library_name) {
            true => exit(0),
            false => exit(1),
        };

    } else if args.command == CLICommand::ShowConfig {
        commands::cmd_show_config(&get_configuration(&args));
        Ok(())

    } else if args.command == CLICommand::ListLibraries {
        commands::cmd_list_libraries(&get_configuration(&args));
        Ok(())

    } else {
        panic!("Unexpected/unimplemented command!");
    }
    
     */
}
