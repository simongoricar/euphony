use std::fs;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::{Config, filesystem};
use crate::commands::transcode::dirs::AlbumDirectoryInfo;
use crate::commands::transcode::packets::album::AlbumWorkPacket;

#[derive(Eq, PartialEq)]
enum FilePacketType {
    AudioFile,
    DataFile,
}

impl FilePacketType {
    pub fn from_path<P: AsRef<Path>>(file_path: P, config: &Config) -> Option<FilePacketType> {
        let source_file_extension = filesystem::get_path_file_extension(file_path.as_ref())
            .ok()?;

        if config.file_metadata.matches_audio_extension(&source_file_extension) {
            Some(FilePacketType::AudioFile)
        } else if config.file_metadata.matches_data_extension(&source_file_extension) {
            Some(FilePacketType::DataFile)
        } else {
            None
        }
    }
}


/// Represents the smallest unit of work we can generate - a single file.
/// It contains all the information it needs to process the file.
pub struct FileWorkPacket<'a> {
    pub source_file_path: PathBuf,
    pub target_file_path: PathBuf,

    file_type: FilePacketType,

    /// Owning AlbumWorkPacket reference.
    owner: &'a AlbumWorkPacket,
}

impl<'a> FileWorkPacket<'a> {
    pub fn new(
        file_name: &Path,
        source_album_info: &AlbumDirectoryInfo,
        album_work_packet: &'a AlbumWorkPacket,
        config: &Config,
    ) -> Result<FileWorkPacket<'a>, Error> {
        let source_file_path = source_album_info
            .build_full_file_path(file_name);

        let source_file_type = FilePacketType::from_path(&source_file_path, config)
            .ok_or(
                Error::new(
                    ErrorKind::Other,
                    "Invalid source file extension: doesn't match any tracked extension."
                )
            )?;

        let target_file_extension = match source_file_type {
            FilePacketType::AudioFile => String::from("mp3"),
            FilePacketType::DataFile => filesystem::get_path_file_extension(&source_file_path)?,
        };

        let target_file_path = source_album_info
            .as_aggregated_directory(config)
            .build_full_file_path(file_name)
            .with_extension(target_file_extension);

        Ok(FileWorkPacket {
            source_file_path,
            target_file_path,
            file_type: source_file_type,
            owner: album_work_packet,
        })
    }

    /// Run the processing for this file packet. This involves either:
    /// - transcoding if it's an audio file,
    /// - or a simple file copy if it is a data file.
    pub fn process(&self, config: &Config) -> Result<(), Error> {
        match self.file_type {
            FilePacketType::AudioFile => self.transcode_into_mp3_v0(config),
            FilePacketType::DataFile => self.copy_data_file(),
        }
    }

    /// Transcode the current FileWorkPacket from the source file
    /// into a MP3 V0 file in the target path. Expects the work packet to be an audio file.
    fn transcode_into_mp3_v0(&self, config: &Config) -> Result<(), Error> {
        // Make sure we're actually a tracked audio file.
        if self.file_type != FilePacketType::AudioFile {
            return Err(
                Error::new(
                    ErrorKind::Other,
                    "Invalid source extension for transcode: not a tracked audio file."
                )
            );
        }

        // Ensure the target directory structure exists.
        let target_directory = self.target_file_path.parent()
            .ok_or(Error::new(ErrorKind::NotFound, "No target directory."))?;
        fs::create_dir_all(target_directory)?;

        // Compute ffmpeg arguments.
        let source_file_path_str = self.source_file_path.to_str()
            .ok_or(Error::new(ErrorKind::Other, "Could not convert source path to str!"))?;
        let target_file_path_str = self.target_file_path.to_str()
            .ok_or(Error::new(ErrorKind::Other, "Could not convert target path to str!"))?;

        let ffmpeg_arguments: Vec<String> = config.tools.ffmpeg.to_mp3_v0_args
            .iter()
            .map(|item| item
                .replace("{INPUT_FILE}", source_file_path_str)
                .replace("{OUTPUT_FILE}", target_file_path_str)
            )
            .collect();

        // Run ffmpeg
        let ffmpeg_command = Command::new(&config.tools.ffmpeg.binary)
            .args(ffmpeg_arguments)
            .output()?;

        if ffmpeg_command.status.success() {
            Ok(())
        } else {
            let ffmpeg_exit_code = ffmpeg_command.status.code()
                .ok_or(
                    Error::new(
                        ErrorKind::Other,
                        "Could not get ffmpeg exit code!"
                    )
                )?;

            Err(
                Error::new(
                    ErrorKind::Other,
                    format!(
                        "Non-zero ffmpeg exit code: {}",
                        ffmpeg_exit_code,
                    ),
                )
            )
        }
    }

    /// Perform a simple file copy from the source path to the target path.
    /// Expects the file packet to be about a data file, *not* an audio file.
    fn copy_data_file(&self) -> Result<(), Error> {
        // Make sure we're actually a tracked data file.
        if self.file_type != FilePacketType::DataFile {
            return Err(
                Error::new(
                    ErrorKind::Other,
                    "Invalid source extension for copy: not a tracked data file."
                )
            );
        }

        match fs::copy(&self.source_file_path, &self.target_file_path) {
            Ok(bytes_copied) => {
                if bytes_copied > 0 {
                    Ok(())
                } else {
                    Err(
                        Error::new(
                            ErrorKind::Other,
                            "Copy operation technically complete, but 0 bytes copied."
                        )
                    )
                }
            },
            Err(error) => Err(error)
        }
    }

    /// Remove the processed (transcoded/copied) file.
    /// TODO From where will this be called? Should it be public?
    fn remove_processed_file(&self) -> Result<(), Error> {
        fs::remove_file(&self.target_file_path)
    }
}
