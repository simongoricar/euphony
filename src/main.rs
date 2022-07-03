use std::{env, path};
use std::path::PathBuf;
use std::process::exit;

use clap::{Parser, Args, Subcommand};
use owo_colors::OwoColorize;

use configuration::Config;

mod configuration;
mod filesystem;
mod commands;
mod console;
mod utilities;


#[derive(Subcommand, PartialEq, Eq)]
enum CLICommand {
    // TODO
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

    if let CLICommand::TranscodeAlbum(args) = args.command {
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
                console::horizontal_line(None, None);
                console::horizontal_line_with_text(
                    &format!(
                        "{}",
                        "Album aggregation complete"
                            .green()
                            .italic()
                            .bold()
                    ),
                    None, None, None,
                );
                console::horizontal_line(None, None);
            },
            Err(error) => {
                console::new_line();
                console::horizontal_line(None, None);
                console::horizontal_line_with_text(
                    &format!(
                        "{}",
                        "Errors in album aggregation"
                            .red()
                            .italic()
                            .bold()
                    ),
                    None, None, None,
                );
                eprintln!("Error: {}", error);
                console::horizontal_line(None, None);

                exit(1);
            }
        };

    } else if let CLICommand::TranscodeLibrary(args) = args.command {
        let selected_library = config.libraries.get(&args.library_name);
        if selected_library.is_none() {
            eprintln!(
                "{}{}",
                "No such library: ".red(),
                args.library_name,
            );
            exit(1);
        }

        let selected_library = selected_library.unwrap();
        let selected_library_path = PathBuf::from(&selected_library.path);

        let library_transcode = commands::cmd_transcode_library(
            &selected_library_path,
            &config,
        );

        if library_transcode.is_ok() {
            exit(0);
        } else {
            exit(1);
        }

    } else if args.command == CLICommand::Validate {
        let is_completely_valid = commands::cmd_validate(&config);
        if is_completely_valid {
            exit(0);
        } else {
            exit(1);
        }

    } else if args.command == CLICommand::ShowConfig {
        commands::cmd_show_config(&config);

    } else {
        panic!("Unexpected command!");
    }
}
