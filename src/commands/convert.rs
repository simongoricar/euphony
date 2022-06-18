use std::fs;
use std::path::PathBuf;
use std::process::exit;

use super::super::configuration::Config;

// TODO
pub fn cmd_convert(directory: &PathBuf, _config: &Config) {
    if !directory.is_dir() {
        println!("Current directory is invalid.");
        exit(1);
    }

    let dir_read = fs::read_dir(directory)
        .expect("Could not list files in current directory!");

    for file in dir_read {
        let _file = file.expect("Could not list file!");
    }

    // TODO
}
