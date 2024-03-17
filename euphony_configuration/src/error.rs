use std::{io, path::PathBuf};

use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum ConfigurationError {
    #[error("Failed to load configuration file.")]
    FileLoadError {
        file_path: PathBuf,
        error: io::Error,
    },

    #[error(
        "Failed to parse configuration file \
        \"{file_path}\" as TOML: {error}."
    )]
    FileFormatError {
        file_path: PathBuf,
        error: Box<toml::de::Error>,
    },
    // TODO
}
