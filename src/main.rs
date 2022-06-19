extern crate core;

use std::env;

use clap::{ArgEnum, Parser};

use configuration::Config;

mod configuration;
mod filesystem;
mod commands;
mod console;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum CLICommand {
    Convert,
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

    if args.command == CLICommand::Convert {
        commands::cmd_convert(&current_directory, &config);
    } else if args.command == CLICommand::Validate {
        commands::cmd_validate(&config);
    } else if args.command == CLICommand::ShowConfig {
        todo!();
    } else {
        panic!("Unexpected command!");
    }
}
