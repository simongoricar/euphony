use std::{env, path};
use std::path::PathBuf;
use std::process::exit;

use ::console::style;
use clap::{Args, Parser, Subcommand};

use configuration::Config;
use crate::globals::VERBOSE;

mod configuration;
mod filesystem;
mod commands;
mod console;
mod cached;
mod globals;
mod observer;


#[derive(Subcommand, PartialEq, Eq)]
enum CLICommand {
    #[clap(
        name = "transcode-all",
        about = "Transcode all registered libraries into the aggregated (transcoded) library."
    )]
    TranscodeAll,

    #[clap(
        name = "transcode-library",
        about = "Transcode only the specified library into the aggregated (transcoded) library. \
                 Requires a single positional parameter: the library name (by full name), \
                 as configured in the configuration file."
    )]
    TranscodeLibrary(TranscodeLibraryArgs),

    #[clap(
        name = "transcode-album",
        about = "Transcode only the specified album into the aggregated (transcoded) library. \
                 The current directory is used by default, but you may pass a different one \
                 using \"--dir <path>\"."
    )]
    TranscodeAlbum(TranscodeAlbumArgs),

    #[clap(
        name = "validate-all",
        about = "Validate all the available (sub)libraries for inconsistencies, such as \
                 forbidden files, any inter-library collisions that would cause problems \
                 when aggregating (transcoding), etc."
    )]
    ValidateAll,

    #[clap(
        name = "validate-library",
        about = "Validate a specific library for inconsistencies, such as forbidden files."
    )]
    ValidateLibrary(ValidateLibraryArgs),

    #[clap(
        name = "show-config",
        about = "Loads, validates and prints the current configuration from `./data/configuration.toml`."
    )]
    ShowConfig,

    #[clap(
        name = "list-libraries",
        about = "List all the registered libraries."
    )]
    ListLibraries,
}

#[derive(Args, PartialEq, Eq)]
struct TranscodeAlbumArgs {
    #[clap(
        long = "dir",
        help = "Directory to process, defaults to current directory."
    )]
    directory: Option<String>,
}

#[derive(Args, PartialEq, Eq)]
struct TranscodeLibraryArgs {
    #[clap(
        help = "Library to process (by full name)."
    )]
    library_name: String,
}

#[derive(Args, PartialEq, Eq)]
struct ValidateLibraryArgs {
    #[clap(
        help = "Library to process (by full name)."
    )]
    library_name: String,
}

#[derive(Parser)]
#[clap(
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
    #[clap(
        short = 'c',
        long = "config",
        help = "Optionally a path to your configuration file. Without this option, \
                euphony tries to load ./data/configuration.toml, but understandably this \
                might not always be the most convinient location."
    )]
    config: Option<String>,

    #[clap(
        short = 'v',
        long = "verbose",
        help = "Increase the verbosity of output."
    )]
    verbose: bool,

    #[clap(subcommand)]
    command: CLICommand,
}

fn get_configuration(args: &CLIArgs) -> Config {
    if args.config.is_some() {
        Config::load_from_path(args.config.clone().unwrap())
    } else {
        Config::load_default_path()
    }
}

fn main() {
    let args: CLIArgs = CLIArgs::parse();

    VERBOSE.set(args.verbose);

    if args.command == CLICommand::TranscodeAll {
        match commands::cmd_transcode_all(&get_configuration(&args)) {
            Ok(_) => {
                console::new_line();
                console::horizontal_line_with_text(
                    format!(
                        "{}",
                        style("Full aggregation complete.")
                            .green()
                            .italic()
                            .bold()
                    ),
                    None, None,
                );
            },
            Err(error) => {
                console::new_line();
                console::horizontal_line_with_text(
                    format!(
                        "{}",
                        style("Errors in full aggregation!")
                            .red()
                            .italic()
                            .bold()
                    ),
                    None, None,
                );
                console::centered_print(
                    format!(
                        "{}",
                        style(error)
                            .red()
                    ),
                    None,
                );
            }
        };

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
                console::new_line();
                console::horizontal_line_with_text(
                    format!(
                        "{}",
                        style("Album aggregation complete.")
                            .green()
                            .italic()
                            .bold()
                    ),
                    None, None,
                );
            },
            Err(error) => {
                console::new_line();
                console::horizontal_line_with_text(
                    format!(
                        "{}",
                        style("Errors in album aggregation!")
                            .red()
                            .italic()
                            .bold()
                    ),
                    None, None,
                );
                console::centered_print(
                    format!(
                        "{}",
                        style(error)
                            .red()
                    ),
                    None,
                );

                exit(1);
            }
        };

    } else if let CLICommand::TranscodeLibrary(tl_args) = &args.command {
        let config = get_configuration(&args);

        let selected_library = match config.get_library_by_full_name(&tl_args.library_name) {
            Some(library) => library,
            None => {
                eprintln!(
                    "{} {}",
                    style("No such library:")
                        .red(),
                    tl_args.library_name,
                );
                exit(1);
            }
        };

        let selected_library_path = PathBuf::from(&selected_library.path);

        match commands::cmd_transcode_library(&selected_library_path, &config) {
            Ok(_) => {
                console::new_line();
                console::horizontal_line_with_text(
                    format!(
                        "{}",
                        style("Library aggregation complete.")
                            .green()
                            .italic()
                            .bold()
                    ),
                    None, None,
                );
            },
            Err(error) => {
                eprintln!(
                    "{} {}",
                    style("Error while transcoding library:")
                        .red(),
                    error,
                );
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

    } else if args.command == CLICommand::ListLibraries {
        commands::cmd_list_libraries(&get_configuration(&args));

    } else {
        panic!("Unexpected/unimplemented command!");
    }
}
