use std::env::args;
use std::path::{Path, PathBuf};

use miette::{miette, Context, IntoDiagnostic, Result};

/// Inspect the first command line argument to find out the directory the program resides in.
///
/// **This contains an important escape detail:** it automatically detects whether it is running
/// inside the cargo's debug target directory (`./target/debug`) and returns the grandparent directory.
/// This only happens if the grandparent directory also contains `Cargo.toml`, signaling that it indeed
/// is the root of a project.
///
/// Visual representation of
///  <project directory>
///  |-- target
///  |   |-- debug
///  |       |- euphony(.exe)
///  |- Cargo.toml
///  |- ...
///
/// In the above case, `get_running_executable_directory` will return `<project directory>`, NOT
/// `<project directory>/target/debug`.
///
pub fn get_running_executable_directory() -> Result<PathBuf> {
    let current_args = args()
        .next()
        .ok_or_else(|| miette!("Could not get first commandline argument!"))?;

    // might be "debug"
    let executable_directory = dunce::canonicalize(current_args)
        .into_diagnostic()
        .wrap_err_with(|| {
            miette!("Could not canonicalize running executable path.")
        })?
        .parent()
        .ok_or_else(|| miette!("Could not get executable's directory."))?
        .to_path_buf();

    let executable_directory_name = executable_directory
        .file_name()
        .ok_or_else(|| {
            miette!("Could not get the name of the executable's directory.")
        })?
        .to_string_lossy()
        .to_string();

    // Attempt to detect if we're in "debug/target" and the parent directory contains Cargo.toml".
    if executable_directory_name.eq("debug") {
        let executable_parent_dir = executable_directory
            .parent()
            .ok_or_else(|| miette!("Could not get the parent directory."))?;

        let executable_parent_dir_name = executable_parent_dir
            .file_name()
            .ok_or_else(|| {
                miette!("Could not get the name of the parent directory.")
            })?
            .to_string_lossy()
            .to_string();

        // Might be "target", in which case we escape it (but only if the parent contains Cargo.toml).
        if executable_parent_dir_name.eq("target") {
            let grandparent_directory =
                executable_parent_dir.parent().ok_or_else(|| {
                    miette!("Could not get grandparent directory.")
                })?;

            let cargo_toml_path =
                Path::new(grandparent_directory).join("Cargo.toml");

            return if cargo_toml_path.exists() {
                Ok(grandparent_directory.to_path_buf())
            } else {
                Ok(executable_directory)
            };
        }
    }

    Ok(executable_directory)
}

/// Returns the default configuration filepath. This is `./data/configuration.toml`, with (potentially)
/// an additional `../../` escape if we're running inside the `./target/debug` directory of a cargo project.
pub fn get_default_configuration_file_path() -> Result<String> {
    let mut configuration_filepath = get_running_executable_directory()
        .wrap_err_with(|| miette!("Could not get the executable directory."))?;
    configuration_filepath.push("./data/configuration.toml");

    if !configuration_filepath.exists() {
        panic!("Could not find configuration.toml in data directory.");
    }

    let configuration_filepath = dunce::canonicalize(configuration_filepath)
        .into_diagnostic()
        .wrap_err_with(|| {
            miette!("Could not canonicalize the configuration.toml file path.",)
        })?;

    Ok(configuration_filepath.to_string_lossy().to_string())
}
