use std::ffi::OsStr;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::commands::transcode::directories::DirectoryInfo;

use crate::Config;

fn get_output_file_path(
    file_name: &OsStr,
    config: &Config,
    artist: &str,
    album: &str,
    target_extension: Option<&str>,
) -> PathBuf {
    let mut output_file_path = PathBuf::from(&config.aggregated_library.path);

    output_file_path.push(artist);
    output_file_path.push(album);
    output_file_path.push(file_name);

    if target_extension.is_some() {
        output_file_path.with_extension(target_extension.unwrap())
    } else {
        output_file_path
    }
}

pub fn transcode_audio_file_into_mp3_v0(source_file_path: &Path, config: &Config) -> Result<(), Error> {
    // Compute input and output file paths.
    let file_name = source_file_path
        .file_name()
        .expect("Could not get file name for aggregation!");

    let source_album_directory = source_file_path
        .parent()
        .expect("No parent directory for source file!");
    let source_album_directory_info = DirectoryInfo::new(source_album_directory, config)?;

    let input_file_path = source_file_path
        .to_str()
        .expect("Could not convert input file path to string!");

    let output_file_path = get_output_file_path(
        file_name,
        config,
        &source_album_directory_info.artist_name,
        &source_album_directory_info.album_title,
        Some("mp3")
    );
    let output_file_path_str = output_file_path
        .to_str()
        .expect("Could not convert output file path to string!");

    // Make sure the output directory exists.
    let output_directory = output_file_path
        .parent()
        .expect("Could not get file parent directory.");
    fs::create_dir_all(output_directory)?;

    // Construct ffmpeg arguments and run ffmpeg for transcode.
    let ffmpeg_args: Vec<String> = config.tools.ffmpeg.to_mp3_v0_args
        .iter()
        .map(|item| {
            item
                .replace("{INPUT_FILE}", input_file_path)
                .replace("{OUTPUT_FILE}", output_file_path_str)
        })
        .collect();

    let command = Command::new(config.tools.ffmpeg.binary.clone())
        .args(ffmpeg_args)
        .output()?;

    if command.status.success() {
        Ok(())
    } else {
        Err(
            Error::new(
                ErrorKind::Other,
                format!(
                    "Non-zero ffmpeg exit code: {}",
                    command.status
                        .code()
                        .expect("Could not get ffmpeg exit code.")
                ),
            )
        )
    }
}

pub fn copy_data_file(source_file_path: &Path, config: &Config) -> Result<(), Error> {
    let source_file_name = source_file_path.file_name()
        .expect("Could not get file name from source file path!");

    let source_directory = source_file_path.parent()
        .expect("No parent directory for source file!");
    let source_directory_info = DirectoryInfo::new(source_directory, config)?;

    let output_file_path = get_output_file_path(
        source_file_name,
        config,
        &source_directory_info.artist_name,
        &source_directory_info.album_title,
        None,
    );
    let output_file_path_str = output_file_path
        .to_str()
        .expect("Could not convert output file path to string!");

    match fs::copy(source_file_path, output_file_path_str) {
        Ok(bytes_copied) => {
            if bytes_copied > 0 {
                Ok(())
            } else {
                Err(
                    Error::new(
                        ErrorKind::Other,
                        "Could not copy data file (0 bytes copied)."
                    )
                )
            }
        },
        Err(error) => {
            Err(error)
        }
    }
}

pub fn remove_target_file(source_file_path: &Path, config: &Config, is_audio_file: bool) -> Result<(), Error> {
    let source_file_name = source_file_path.file_name()
        .expect("Could not get file name from source file path!");

    let source_directory = source_file_path
        .parent()
        .expect("No parent directory for source file!");
    let source_directory_info = DirectoryInfo::new(source_directory, config)?;

    let output_file_pathbuf = get_output_file_path(
        source_file_name,
        config,
        &source_directory_info.artist_name,
        &source_directory_info.album_title,
        if is_audio_file { Some("mp3") } else { None },
    );

    fs::remove_file(&output_file_pathbuf)
}
