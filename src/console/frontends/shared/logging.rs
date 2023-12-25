use std::{
    env::args,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
};

use chrono::Local;
use miette::{miette, Context, IntoDiagnostic, Result};
use strip_ansi_escapes::Writer as StripAnsiWriter;

use crate::EUPHONY_VERSION;

// TODO Extract code from enable_saving_logs_to_file.
/// Prepares the log file for log output.
/// This involves opening the file for writing
/// (creating it if necessary). If the file already exists,
/// is is opened in append mode.
///
/// A small invocation header is written to the log file before the writer
/// handle is returned.
pub fn initialize_log_file_for_log_output(
    log_output_file_path: &Path,
) -> Result<BufWriter<StripAnsiWriter<File>>> {
    let log_output_directory_path = log_output_file_path
        .parent()
        .ok_or_else(|| miette!("No log file parent directory?!"))?;

    if log_output_directory_path.exists() && !log_output_directory_path.is_dir()
    {
        return Err(miette!("Invalid log file path: parent directory path is actually not a directory."));
    }
    if !log_output_directory_path.exists() {
        fs::create_dir_all(log_output_directory_path)
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Failed to create log file parent directory.")
            })?;
    }

    let output_file = match log_output_file_path.exists() {
        true => OpenOptions::new()
            .append(true)
            .open(log_output_file_path)
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!(
                    "Failed to open log output file for appending: {:?}",
                    log_output_file_path
                )
            })
            .wrap_err_with(|| {
                miette!("Failed to open existing log file for writing.")
            })?,
        false => OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(log_output_file_path)
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!(
                    "Failed to create log output file: {:?}",
                    log_output_file_path
                )
            })
            .wrap_err_with(|| miette!("Failed to create and open log file."))?,
    };

    let ansi_escaping_writer = strip_ansi_escapes::Writer::new(output_file);
    let mut buf_writer = BufWriter::with_capacity(1024, ansi_escaping_writer);

    // Write an "invocation header", marking the start of euphony.
    let time_now = Local::now();
    let formatted_time_now = time_now.format("%Y-%m-%d %H:%M:%S%.3f");

    buf_writer
        .write_all(
            format!(
                "{} Hello from euphony {}. Started with arguments: {:?}",
                formatted_time_now,
                EUPHONY_VERSION,
                args()
            )
            .as_bytes(),
        )
        .into_diagnostic()
        .wrap_err_with(|| {
            miette!("Could not write invocation header to file.")
        })?;

    Ok(buf_writer)
}
