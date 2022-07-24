use std::fmt::{Debug, Formatter};
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Config, filesystem};
use crate::commands::transcode::dirs::AlbumDirectoryInfo;
use crate::globals::verbose_enabled;

#[derive(Eq, PartialEq, Debug)]
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

#[derive(Eq, PartialEq)]
pub enum FilePacketAction {
    Process,
    Remove,
}

#[derive(Clone)]
pub enum ProcessingResult {
    Ok {
        verbose_info: Option<String>,
    },
    Error {
        error: String,
        verbose_info: Option<String>,
    },
}

impl ProcessingResult {
    pub fn is_ok(&self) -> bool {
        match self {
            ProcessingResult::Ok {
                verbose_info: _verbose_info,
            } => true,
            ProcessingResult::Error {
                error: _error,
                verbose_info: _verbose_info,
            } => false,
        }
    }
}


/// Represents the smallest unit of work we can generate - a single file.
/// It contains all the information it needs to process the file.
pub struct FileWorkPacket {
    pub source_file_path: PathBuf,
    pub target_file_path: PathBuf,
    file_type: FilePacketType,
    action: FilePacketAction,
}

impl FileWorkPacket {
    pub fn new(
        file_name: &Path,
        source_album_info: &AlbumDirectoryInfo,
        config: &Config,
        action: FilePacketAction,
    ) -> Result<FileWorkPacket, Error> {
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
            action,
        })
    }

    pub fn get_file_name(&self) -> Result<String, Error> {
        Ok(self.source_file_path.file_name()
            .ok_or(Error::new(ErrorKind::Other, "Could not extract file name from source path."))?
            .to_str()
            .ok_or(Error::new(ErrorKind::Other, "Could not extract file name from source path."))?
            .to_string())
    }

    /// Run the processing for this file packet. This involves either:
    /// - transcoding if it's an audio file,
    /// - or a simple file copy if it is a data file.
    pub fn process(&self, config: &Config) -> ProcessingResult {
        match self.action {
            FilePacketAction::Process => match self.file_type {
                FilePacketType::AudioFile => self.transcode_into_mp3_v0(config),
                FilePacketType::DataFile => self.copy_data_file(),
            },
            FilePacketAction::Remove => self.remove_processed_file(true),
        }
    }

    /// Transcode the current FileWorkPacket from the source file
    /// into a MP3 V0 file in the target path. Expects the work packet to be an audio file.
    fn transcode_into_mp3_v0(&self, config: &Config) -> ProcessingResult {
        // Make sure we're actually a tracked audio file.
        if self.file_type != FilePacketType::AudioFile {
            return ProcessingResult::Error {
                error: String::from("Invalid source extension for transcode, not a tracked audio file."),
                // TODO Is this even valid?
                verbose_info: if verbose_enabled() {
                    Some(format!("FilePacket: {:?}", self))
                } else {
                    None
                },
            };
        }

        // Ensure the target directory structure exists.
        let target_directory = match self.target_file_path.parent()
            .ok_or(Error::new(ErrorKind::NotFound, "No target directory.")) {
            Ok(path) => path,
            Err(error) => {
                return ProcessingResult::Error {
                    error: error.to_string(),
                    verbose_info: if verbose_enabled() {
                        Some(format!("FilePacket: {:?}", self))
                    } else {
                        None
                    },
                }
            }
        };

        match fs::create_dir_all(target_directory) {
            Ok(()) => (),
            Err(error) =>{
                return ProcessingResult::Error {
                    error: error.to_string(),
                    verbose_info: if verbose_enabled() {
                        Some(format!("FilePacket: {:?}", self))
                    } else {
                        None
                    },
                }
            }
        };

        // Compute ffmpeg arguments.
        let source_file_path_str = match self.source_file_path.to_str()
            .ok_or(Error::new(ErrorKind::Other, "Could not convert source path to str!")) {
            Ok(string) => string,
            Err(error) => {
                return ProcessingResult::Error {
                    error: error.to_string(),
                    verbose_info: if verbose_enabled() {
                        Some(format!("FilePacket: {:?}", self))
                    } else {
                        None
                    },
                }
            }
        };

        let target_file_path_str = match self.target_file_path.to_str()
            .ok_or(Error::new(ErrorKind::Other, "Could not convert target path to str!")) {
            Ok(string) => string,
            Err(error) => {
                return ProcessingResult::Error {
                    error: error.to_string(),
                    verbose_info: if verbose_enabled() {
                        Some(format!("FilePacket: {:?}", self))
                    } else {
                        None
                    },
                }
            }
        };

        let ffmpeg_arguments: Vec<String> = config.tools.ffmpeg.to_mp3_v0_args
            .iter()
            .map(|item| item
                .replace("{INPUT_FILE}", source_file_path_str)
                .replace("{OUTPUT_FILE}", target_file_path_str)
            )
            .collect();

        // Run the actual transcode using ffmpeg.
        let ffmpeg_command = match Command::new(&config.tools.ffmpeg.binary)
            .args(ffmpeg_arguments)
            .output() {
            Ok(output) => output,
            Err(error) => {
                return ProcessingResult::Error {
                    error: error.to_string(),
                    verbose_info: if verbose_enabled() {
                        Some(format!("FilePacket: {:?}", self))
                    } else {
                        None
                    }
                };
            }
        };

        if ffmpeg_command.status.success() {
            ProcessingResult::Ok {
                verbose_info: if verbose_enabled() {
                    Some(format!("FilePacket: {:?}", self))
                } else {
                    None
                }
            }
        } else {
            let ffmpeg_exit_code = ffmpeg_command.status.code()
                .ok_or(
                    Error::new(
                        ErrorKind::Other,
                        "Could not get ffmpeg exit code!"
                    )
                );

            if ffmpeg_exit_code.is_err() {
                ProcessingResult::Error {
                    error: ffmpeg_exit_code.unwrap_err().to_string(),
                    verbose_info: if verbose_enabled() {
                        Some(format!("FilePacket: {:?}", self))
                    } else {
                        None
                    }
                }
            } else {
                let ffmpeg_exit_code = ffmpeg_exit_code.unwrap();

                if ffmpeg_exit_code == 0 {
                    ProcessingResult::Ok {
                        verbose_info: if verbose_enabled() {
                            Some(format!("FilePacket: {:?}", self))
                        } else {
                            None
                        }
                    }
                } else {
                    ProcessingResult::Error {
                        error: String::from("ffmpeg had non-zero exit code!"),
                        verbose_info: if verbose_enabled() {
                            Some(format!("FilePacket: {:?}", self))
                        } else {
                            None
                        }
                    }
                }
            }
        }
    }

    /// Perform a simple file copy from the source path to the target path.
    /// Expects the file packet to be about a data file, *not* an audio file.
    fn copy_data_file(&self) -> ProcessingResult {
        // Make sure we're actually a tracked data file.
        if self.file_type != FilePacketType::DataFile {
            return ProcessingResult::Error {
                error: String::from("Invalid source extension for copy: not a tracked data file."),
                verbose_info: if verbose_enabled() {
                    Some(format!("FilePacket: {:?}", self))
                } else {
                    None
                }
            };
        }

        match fs::copy(&self.source_file_path, &self.target_file_path) {
            Ok(bytes_copied) => {
                if bytes_copied > 0 {
                    ProcessingResult::Ok {
                        verbose_info: if verbose_enabled() {
                            Some(format!("FilePacket: {:?}", self))
                        } else {
                            None
                        }
                    }
                } else {
                    ProcessingResult::Error {
                        error: String::from("Copy operation technically complete, but 0 bytes copied."),
                        verbose_info: if verbose_enabled() {
                            Some(format!("FilePacket: {:?}", self))
                        } else {
                            None
                        }
                    }
                }
            },
            Err(error) => {
                ProcessingResult::Error {
                    error: error.to_string(),
                    verbose_info: if verbose_enabled() {
                        Some(format!("FilePacket: {:?}", self))
                    } else {
                        None
                    }
                }
            }
        }
    }

    /// Check whether the target file exists.
    pub fn target_file_exists(&self) -> bool {
        self.target_file_path.exists()
    }

    /// Remove the processed (transcoded/copied) file.
    /// TODO From where will this be called? Should it be public?
    fn remove_processed_file(&self, ignore_if_missing: bool) -> ProcessingResult {
        if !self.target_file_exists() && ignore_if_missing {
            ProcessingResult::Ok {
                verbose_info: if verbose_enabled() {
                    Some(format!("FilePacket: {:?}", self))
                } else {
                    None
                }
            }
        } else {
            match fs::remove_file(&self.target_file_path) {
                Ok(()) => {
                    ProcessingResult::Ok {
                        verbose_info: if verbose_enabled() {
                            Some(format!("FilePacket: {:?}", self))
                        } else {
                            None
                        }
                    }
                },
                Err(error) => {
                    ProcessingResult::Error {
                        error: error.to_string(),
                        verbose_info: if verbose_enabled() {
                            Some(format!("FilePacket: {:?}", self))
                        } else {
                            None
                        }
                    }
                }
            }
        }
    }
}

impl Debug for FileWorkPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FileWorkPacket ({:?}): {:?} -> {:?}",
            self.file_type,
            self.source_file_path,
            self.target_file_path,
        )
    }
}
