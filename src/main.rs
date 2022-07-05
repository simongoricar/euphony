use std::{env, path};
use std::path::PathBuf;
use std::process::exit;
use ::console::style;

use clap::{Parser, Args, Subcommand};

use configuration::Config;

mod configuration;
mod filesystem;
mod commands;
mod console;
mod cached;


#[derive(Subcommand, PartialEq, Eq)]
enum CLICommand {
    TranscodeAll,
    TranscodeLibrary(TranscodeLibraryArgs),
    TranscodeAlbum(TranscodeAlbumArgs),
    Validate,
    ShowConfig,

    // TODO
    Download,
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

#[derive(Parser)]
struct CLIArgs {
    #[clap(subcommand)]
    command: CLICommand,
}


fn main() {
    let config = Config::load();
    let args: CLIArgs = CLIArgs::parse();

    if args.command == CLICommand::TranscodeAll {
        match commands::cmd_transcode_all(&config) {
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

    } else if let CLICommand::TranscodeAlbum(args) = args.command {
        let selected_directory = match args.directory {
            Some(dir) => path::PathBuf::from(dir),
            None => {
                env::current_dir()
                    .expect("Could not get current directory!")
            }
        };

        match commands::cmd_transcode_album(&selected_directory, &config) {
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

    } else if let CLICommand::TranscodeLibrary(args) = args.command {
        let selected_library = match config.libraries.get(&args.library_name) {
            Some(library) => library,
            None => {
                eprintln!(
                    "{} {}",
                    style("No such library:")
                        .red(),
                    args.library_name,
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

    } else if args.command == CLICommand::Validate {
        match commands::cmd_validate(&config) {
            true => exit(0),
            false => exit(1),
        }

    } else if args.command == CLICommand::ShowConfig {
        commands::cmd_show_config(&config);

    } else {
        panic!("Unexpected/unimplemented command!");
    }
}
