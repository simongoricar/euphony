use std::env;
use std::process::exit;

use clap::{ArgEnum, Parser};

use configuration::Config;

mod configuration;
mod filesystem;
mod commands;
mod console;
mod utilities;


#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum CLICommand {
    Aggregate,
    Validate,
    ShowConfig
}

#[derive(Parser)]
struct CLIArgs {
    #[clap(arg_enum)]
    command: CLICommand,
}


fn main() {
    let config = Config::load();
    let args: CLIArgs = CLIArgs::parse();

    let current_directory = env::current_dir()
        .expect("Could not get current directory!");

    if args.command == CLICommand::Aggregate {
        commands::cmd_aggregate(&current_directory, &config);

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
