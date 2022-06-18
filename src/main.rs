use std::env;

use clap::Parser;

use configuration::Config;

mod configuration;
mod filesystem;
mod commands;

#[derive(Parser, Debug)]
struct CLIArgs {
    command: String,
}


fn main() {
    let config = Config::load();
    let args: CLIArgs = CLIArgs::parse();

    let current_directory = env::current_dir()
        .expect("Could not get current directory!");

    if args.command.eq("convert") {
        commands::cmd_convert(&current_directory, &config);
    } else if args.command.eq("validate") {
        commands::cmd_validate(&config);
    }
}
