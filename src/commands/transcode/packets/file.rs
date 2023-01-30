use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{fs, thread};

use miette::{miette, Result};

use crate::commands::transcode::dirs::AlbumDirectoryInfo;
use crate::configuration::{Config, ConfigLibrary};
use crate::filesystem;
use crate::globals::is_verbose_enabled;

const PROCESSING_THREAD_CANCELLATION_TICK_SLEEP_TIME: Duration =
    Duration::from_millis(50);

#[derive(Eq, PartialEq, Clone)]
pub enum FilePacketType {
    AudioFile,
    DataFile,
}

impl Display for FilePacketType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AudioFile => write!(f, "audio file"),
            Self::DataFile => write!(f, "data file"),
        }
    }
}

impl FilePacketType {
    pub fn from_path<P: AsRef<Path>>(
        file_path: P,
        library: &ConfigLibrary,
    ) -> Option<FilePacketType> {
        let source_file_extension =
            filesystem::get_path_file_extension(file_path.as_ref()).ok()?;

        if library
            .transcoding
            .audio_file_extensions
            .contains(&source_file_extension)
        {
            Some(FilePacketType::AudioFile)
        } else if library
            .transcoding
            .other_file_extensions
            .contains(&source_file_extension)
        {
            Some(FilePacketType::DataFile)
        } else {
            None
        }
    }
}

#[derive(Eq, PartialEq, Clone)]
pub enum FilePacketAction {
    Process,
    RemoveAtTarget,
}

#[derive(Clone, Debug)]
pub struct FileProcessingResult {
    /// Whether this instance is the last emmited one for the given FileWorkPacket.
    /// (there are cases with verbose errors where we emit *all* the intermediate errors, meanining
    /// there are multiple FileProcessingResults emmited for the same FileWorkPacket)
    pub is_final: bool,
    pub file_work_packet: FileWorkPacket,
    pub error: Option<String>,
    pub verbose_info: Option<String>,
}

impl FileProcessingResult {
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

impl FileProcessingResult {
    pub fn new_ok<S: Into<String>>(
        packet: FileWorkPacket,
        verbose_info: Option<S>,
    ) -> Self {
        FileProcessingResult {
            is_final: true,
            file_work_packet: packet,
            error: None,
            verbose_info: verbose_info.map(|info| info.into()),
        }
    }

    pub fn new_errored<S: Into<String>, T: Into<String>>(
        packet: FileWorkPacket,
        error: S,
        verbose_info: Option<T>,
    ) -> Self {
        FileProcessingResult {
            is_final: true,
            file_work_packet: packet,
            error: Some(error.into()),
            verbose_info: verbose_info.map(|info| info.into()),
        }
    }
}


/// Represents the smallest unit of work we can generate - a single file.
/// It contains all the information it needs to process the file.
#[derive(Clone)]
pub struct FileWorkPacket {
    pub source_file_path: PathBuf,
    pub target_file_path: PathBuf,
    pub file_type: FilePacketType,
    pub action: FilePacketAction,
}

impl FileWorkPacket {
    pub fn new(
        file_name: &Path,
        source_album_info: &AlbumDirectoryInfo,
        config: &Config,
        action: FilePacketAction,
    ) -> Result<FileWorkPacket> {
        let source_file_path =
            source_album_info.build_source_file_path(file_name);

        let source_file_type = FilePacketType::from_path(&source_file_path, source_album_info.library)
            .ok_or_else(|| miette!("Invalid source file extension: doesn't match any tracked extension."))?;

        let target_file_extension = match source_file_type {
            FilePacketType::AudioFile => String::from("mp3"),
            FilePacketType::DataFile => {
                filesystem::get_path_file_extension(&source_file_path)?
            }
        };

        let target_file_path = source_album_info
            .build_target_file_path(config, file_name)
            .with_extension(target_file_extension);

        Ok(FileWorkPacket {
            source_file_path,
            target_file_path,
            file_type: source_file_type,
            action,
        })
    }

    pub fn get_file_name(&self) -> Result<String> {
        Ok(self
            .source_file_path
            .file_name()
            .ok_or_else(|| {
                miette!("Could not extract file name from source path.")
            })?
            .to_str()
            .ok_or_else(|| {
                miette!("Could not extract file name: invalid utf-8!")
            })?
            .to_string())
    }

    /// Run the processing for this file packet. This involves either:
    /// - transcoding if it's an audio file
    /// - or a simple file copy if it is a data file.
    pub fn process(
        &self,
        config: &Config,
        cancellation_flag: &AtomicBool,
    ) -> FileProcessingResult {
        match self.action {
            FilePacketAction::Process => match self.file_type {
                FilePacketType::AudioFile => {
                    self.transcode_into_mp3_v0(config, cancellation_flag)
                }
                FilePacketType::DataFile => self.copy_data_file(),
            },
            FilePacketAction::RemoveAtTarget => self.remove_processed_file(true),
        }
    }

    /// Transcode the current FileWorkPacket from the source file
    /// into a MP3 V0 file in the target path. Expects the work packet to be an audio file.
    fn transcode_into_mp3_v0(
        &self,
        config: &Config,
        cancellation_flag: &AtomicBool,
    ) -> FileProcessingResult {
        // Make sure we're actually a tracked audio file.
        if self.file_type != FilePacketType::AudioFile {
            return FileProcessingResult::new_errored(
                self.clone(),
                "Invalid source extension for transcode, not a tracked audio file.",
                is_verbose_enabled().then_some(format!("Not an audio file. {self:?}")),
            );
        }

        // Ensure the target directory structure exists.
        let target_directory = match self.target_file_path.parent() {
            Some(parent) => parent,
            None => {
                return FileProcessingResult::new_errored(
                    self.clone(),
                    "File target path had no parent directory?!",
                    is_verbose_enabled().then_some(format!(
                        "Couldn't construct target directory. {self:?}",
                    )),
                );
            }
        };

        match fs::create_dir_all(target_directory) {
            Ok(()) => (),
            Err(error) => {
                return FileProcessingResult::new_errored(
                    self.clone(),
                    error.to_string(),
                    is_verbose_enabled().then_some(format!(
                        "Couldn't create parent directories. {self:?}",
                    )),
                );
            }
        };

        // Compute ffmpeg arguments.
        let source_file_path_str = match self.source_file_path.to_str() {
            Some(str) => str,
            None => {
                return FileProcessingResult::new_errored(
                    self.clone(),
                    "Could not convert source path to str!",
                    is_verbose_enabled().then_some(format!(
                        "Couldn't construct source file path. {self:?}",
                    )),
                );
            }
        };

        let target_file_path_str = match self.target_file_path.to_str() {
            Some(str) => str,
            None => {
                return FileProcessingResult::new_errored(
                    self.clone(),
                    "Could not convert target path to str!",
                    is_verbose_enabled().then_some(format!(
                        "Couldn't construct target file path. {self:?}",
                    )),
                );
            }
        };

        let ffmpeg_arguments: Vec<String> = config
            .tools
            .ffmpeg
            .to_mp3_v0_args
            .iter()
            .map(|item| {
                item.replace("{INPUT_FILE}", source_file_path_str)
                    .replace("{OUTPUT_FILE}", target_file_path_str)
            })
            .collect();

        // Run the actual transcode using ffmpeg.
        let mut ffmpeg_child = match Command::new(&config.tools.ffmpeg.binary)
            .args(ffmpeg_arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return FileProcessingResult::new_errored(
                    self.clone(),
                    "Could not spawn ffmpeg to transcode audio.",
                    is_verbose_enabled()
                        .then_some(format!("Couldn't spawn ffmpeg: {error}")),
                );
            }
        };

        while ffmpeg_child
            .try_wait()
            .expect("Could not wait for ffmpeg child.")
            .is_none()
        {
            // Keep checking whether the user requested cancellation.
            let flag_value = cancellation_flag.load(Ordering::Acquire);
            if flag_value {
                // Cancellation requested, kill ffmpeg and exit.
                ffmpeg_child.kill().expect("Could not kill ffmpeg process.");
                break;
            }

            thread::sleep(PROCESSING_THREAD_CANCELLATION_TICK_SLEEP_TIME);
        }

        let final_flag_value = cancellation_flag.load(Ordering::Acquire);
        if final_flag_value {
            // Cancellation occured.
            FileProcessingResult::new_errored(
                self.clone(),
                "User cancelled task.",
                is_verbose_enabled().then_some("User cancelled task."),
            )
        } else {
            // No cancellation occured.
            let final_output = ffmpeg_child
                .wait_with_output()
                .expect("Could not wait for ffmpeg to finish");

            if let Some(error_code) = final_output.status.code() {
                if error_code == 0 {
                    FileProcessingResult::new_ok(
                        self.clone(),
                        is_verbose_enabled()
                            .then_some(format!("ffmpeg exited (0). {self:?}")),
                    )
                } else {
                    let ffmpeg_stdout =
                        match String::from_utf8(final_output.stdout) {
                            Ok(stdout) => stdout,
                            Err(error) => {
                                return FileProcessingResult::new_errored(
                                    self.clone(),
                                    format!(
                                        "Couldn't get ffmpeg stdout! {error}",
                                    ),
                                    is_verbose_enabled().then_some(format!(
                                    "from_utf8(ffmpeg.stdout) failed! {self:?}",
                                )),
                                );
                            }
                        };

                    let ffmpeg_stderr =
                        match String::from_utf8(final_output.stderr) {
                            Ok(stderr) => stderr,
                            Err(error) => {
                                return FileProcessingResult::new_errored(
                                    self.clone(),
                                    format!(
                                        "Couldn't get ffmpeg stderr! {error}",
                                    ),
                                    is_verbose_enabled().then_some(format!(
                                    "from_utf8(ffmpeg.stderr) failed! {self:?}",
                                )),
                                );
                            }
                        };

                    FileProcessingResult::new_errored(
                        self.clone(),
                        format!("Non-zero ffmpeg exit code: {error_code}"),
                        is_verbose_enabled().then_some(format!(
                            "ffmpeg exited ({error_code}): {self:?}\n\
                                ffmpeg stdout: {ffmpeg_stdout}\n\
                                ffmpeg stderr: {ffmpeg_stderr}",
                        )),
                    )
                }
            } else {
                FileProcessingResult::new_errored(
                    self.clone(),
                    "Could not get ffmpeg exit code!",
                    is_verbose_enabled().then_some(format!(
                        "Couldn't get ffmpeg exit code. {self:?}",
                    )),
                )
            }
        }
    }

    /// Perform a simple file copy from the source path to the target path.
    /// Expects the file packet to be about a data file, *not* an audio file.
    fn copy_data_file(&self) -> FileProcessingResult {
        // Make sure we're actually a tracked data file.
        if self.file_type != FilePacketType::DataFile {
            return FileProcessingResult::new_errored(
                self.clone(),
                "Invalid source extension for copy: not a tracked data file.",
                is_verbose_enabled()
                    .then_some(format!("Not a data file. {self:?}")),
            );
        }

        let target_directory = match self.target_file_path.parent() {
            Some(path) => path,
            None => {
                return FileProcessingResult::new_errored(
                    self.clone(),
                    "No target directory.",
                    is_verbose_enabled().then_some(format!(
                        "Couldn't construct target directory. {self:?}",
                    )),
                );
            }
        };

        match fs::create_dir_all(target_directory) {
            Ok(()) => (),
            Err(error) => {
                return FileProcessingResult::new_errored(
                    self.clone(),
                    error.to_string(),
                    is_verbose_enabled().then_some(format!(
                        "Couldn't create parent directories. {self:?}",
                    )),
                );
            }
        }

        match fs::copy(&self.source_file_path, &self.target_file_path) {
            Ok(bytes_copied) => {
                if bytes_copied > 0 {
                    FileProcessingResult::new_ok(
                        self.clone(),
                        is_verbose_enabled().then_some(format!(
                            "Copy operation complete. {self:?}",
                        )),
                    )
                } else {
                    FileProcessingResult::new_errored(
                        self.clone(),
                        "Copy operation technically complete, but 0 bytes copied?!",
                        is_verbose_enabled().then_some(format!("Copy complete, but 0 bytes copied. {self:?}")),
                    )
                }
            }
            Err(error) => FileProcessingResult::new_errored(
                self.clone(),
                error.to_string(),
                is_verbose_enabled()
                    .then_some(format!("Error while copying file. {self:?}")),
            ),
        }
    }

    /// Check whether the target file exists.
    pub fn target_file_exists(&self) -> bool {
        self.target_file_path.exists()
    }

    /// Remove the processed (transcoded/copied) file.
    fn remove_processed_file(
        &self,
        ignore_if_missing: bool,
    ) -> FileProcessingResult {
        if !self.target_file_exists() && ignore_if_missing {
            FileProcessingResult::new_ok(
                self.clone(),
                is_verbose_enabled().then_some(format!(
                    "File didn't exist, ignoring. {self:?}",
                )),
            )
        } else {
            match fs::remove_file(&self.target_file_path) {
                Ok(()) => FileProcessingResult::new_ok(
                    self.clone(),
                    is_verbose_enabled()
                        .then_some(format!("File removed. {self:?}")),
                ),
                Err(error) => FileProcessingResult::new_errored(
                    self.clone(),
                    error.to_string(),
                    is_verbose_enabled()
                        .then_some(format!("Could not remove file. {self:?}")),
                ),
            }
        }
    }
}

impl Debug for FileWorkPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<FileWorkPacket({}) {:?}=>{:?}>",
            self.file_type, self.source_file_path, self.target_file_path,
        )
    }
}
