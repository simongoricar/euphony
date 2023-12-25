use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{fs, thread};

use crossbeam::channel::Sender;
use euphony_configuration::get_path_extension_or_empty;
use euphony_library::view::SharedAlbumView;
use miette::{miette, Context, IntoDiagnostic, Result};

use crate::commands::transcode::jobs::common::{
    FileJob,
    FileJobMessage,
    FileJobResult,
};
use crate::commands::transcode::state::changes::FileType;
use crate::console::frontends::shared::queue::QueueItemID;
use crate::globals::is_verbose_enabled;

const FFMPEG_TASK_CANCELLATION_CHECK_INTERVAL: Duration =
    Duration::from_millis(50);
const PARTIAL_TRANSCODED_FILE_DELETE_ATTEMPT_INTERVAL: Duration =
    Duration::from_millis(200);

/*
 * Specific job implementations
 */

/// One of multiple file jobs.
///
/// `TranscodeAudioFileJob` uses ffmpeg to transcode an audio file. The resulting file location
/// is in the album directory of the aggregated library.
pub struct TranscodeAudioFileJob {
    /// Path to the target file's directory (for missing directory creation purposes).
    target_file_directory_path: PathBuf,

    /// Path to the target file that will be created.
    target_file_path: PathBuf,

    /// Path to the ffmpeg binary.
    ffmpeg_binary_path: String,

    /// List of arguments to ffmpeg that will transcode the audio as configured.
    ffmpeg_arguments: Vec<String>,

    /// `QueueItemID` this job belongs to.
    queue_item: QueueItemID,
}

impl TranscodeAudioFileJob {
    /// Initialize a new `TranscodeAudioFileJob`.
    pub fn new(
        album: SharedAlbumView,
        source_file_path: PathBuf,
        target_file_path: PathBuf,
        queue_item: QueueItemID,
    ) -> Result<Self> {
        let album_locked = album.read();

        let config = album_locked.euphony_configuration();

        /*
         * 1. Sanity and error checking before we begin, as these jobs should not operate on
         *    unusual cases that are not matching the configuration.
         */
        let transcoding_config =
            &album_locked.library_configuration().transcoding;
        let ffmpeg_config = &config.tools.ffmpeg;

        if !transcoding_config
            .is_path_audio_file_by_extension(&source_file_path)?
        {
            return Err(miette!(
                "Invalid source file extension \"{}\": \
                expected a tracked audio extension for this library (one of \"{:?}\").",
                get_path_extension_or_empty(source_file_path)?,
                transcoding_config.audio_file_extensions,
            ));
        }

        if !ffmpeg_config
            .is_path_transcoding_output_by_extension(&target_file_path)?
        {
            let ffmpeg_output_extension =
                &config.tools.ffmpeg.audio_transcoding_output_extension;

            return Err(miette!(
                "Invalid ffmpeg output file extension \"{}\": expected \"{}\".",
                get_path_extension_or_empty(target_file_path)?,
                ffmpeg_output_extension
            ));
        };

        let target_file_directory = target_file_path
            .parent()
            .ok_or_else(|| miette!("Could not get target file directory."))?;

        let source_file_path_str = source_file_path
            .to_str()
            .ok_or_else(|| miette!("Source file path is not valid UTF-8."))?;
        let target_file_path_str = target_file_path
            .to_str()
            .ok_or_else(|| miette!("Target file path is not valid UTF-8."))?;

        let ffmpeg_arguments: Vec<String> = config
            .tools
            .ffmpeg
            .audio_transcoding_args
            .iter()
            .map(|arg| {
                arg.replace("{INPUT_FILE}", source_file_path_str)
                    .replace("{OUTPUT_FILE}", target_file_path_str)
            })
            .collect();


        // We have owned versions of data here because we want to be able to send this
        // job across threads easily.
        Ok(Self {
            target_file_directory_path: target_file_directory.to_path_buf(),
            target_file_path: PathBuf::from(target_file_path_str),
            ffmpeg_binary_path: config.tools.ffmpeg.binary.clone(),
            ffmpeg_arguments,
            queue_item,
        })
    }
}

impl FileJob for TranscodeAudioFileJob {
    fn run(
        &mut self,
        cancellation_flag: &AtomicBool,
        message_sender: &Sender<FileJobMessage>,
    ) -> Result<()> {
        message_sender
            .send(FileJobMessage::new_starting(
                self.queue_item,
                FileType::Audio,
                self.target_file_path.to_string_lossy(),
            ))
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not send FileJobMessage::Starting.")
            })?;

        /*
         * Step 1: create missing directories
         */
        let create_dir_result =
            fs::create_dir_all(&self.target_file_directory_path);

        if let Err(error) = create_dir_result {
            let verbose_info = is_verbose_enabled()
                .then(|| format!("fs::create_dir_all error: {error}"));

            message_sender.send(FileJobMessage::new_finished(self.queue_item, FileType::Audio, self.target_file_path.to_string_lossy(), FileJobResult::Errored {
                error: "Could not create target file's missing parent directory.".to_string(),
                verbose_info
            }))
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not send FileJobMessage::Finished"))?;

            return Ok(());
        }

        /*
         * Step 2: run ffmpeg (transcodes audio)
         */
        let mut ffmpeg_child_process = Command::new(&self.ffmpeg_binary_path)
            .args(&self.ffmpeg_arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not spawn ffmpeg for transcoding.")
            })?;

        // Keep checking for cancellation
        while ffmpeg_child_process
            .try_wait()
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not wait or get process exit code.")
            })?
            .is_none()
        {
            let cancellation_flag_value =
                cancellation_flag.load(Ordering::SeqCst);
            if cancellation_flag_value {
                // Cancellation flag is set to true, we should kill ffmpeg and exit as soon as possible.
                ffmpeg_child_process
                    .kill()
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        miette!("Could not kill ffmpeg process.")
                    })?;

                break;
            }

            thread::sleep(FFMPEG_TASK_CANCELLATION_CHECK_INTERVAL);
        }

        ffmpeg_child_process.wait().into_diagnostic()?;

        // ffmpeg process is finished at this point, we should just check what the reason was.
        let final_cancellation_flag = cancellation_flag.load(Ordering::SeqCst);
        if final_cancellation_flag {
            // Process was killed because of cancellation.

            // Delete the partial file.
            if self.target_file_path.exists() && self.target_file_path.is_file()
            {
                let mut retries: usize = 0;
                while retries <= 4 {
                    match fs::remove_file(&self.target_file_path) {
                        Ok(_) => {
                            break;
                        }
                        Err(error) => {
                            if retries == 4 {
                                return Err(error).into_diagnostic();
                            }

                            retries += 1;
                            thread::sleep(
                                PARTIAL_TRANSCODED_FILE_DELETE_ATTEMPT_INTERVAL,
                            );
                        }
                    };
                }
            }

            message_sender
                .send(FileJobMessage::new_cancelled(
                    self.queue_item,
                    FileType::Audio,
                    self.target_file_path.to_string_lossy(),
                ))
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!("Could not send FileJobMessage::Cancelled.")
                })?;

            Ok(())
        } else {
            // Everything was normal.
            let ffmpeg_output = ffmpeg_child_process
                .wait_with_output()
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not get ffmpeg output."))?;

            let ffmpeg_exit_code = ffmpeg_output
                .status
                .code()
                .ok_or_else(|| miette!("No ffmpeg exit code?!"))?;

            // Extract ffmpeg stdout/stderr/exit code if necessary.
            let processing_result = if ffmpeg_exit_code == 0 {
                let verbose_info: Option<String> = is_verbose_enabled()
                    .then(|| {
                        format!(
                            "ffmpeg exited (exit code 0). Binary={:?} Arguments={:?}",
                            &self.ffmpeg_binary_path, &self.ffmpeg_arguments
                        )
                    });

                FileJobResult::Okay { verbose_info }
            } else {
                let ffmpeg_stdout = String::from_utf8(ffmpeg_output.stdout)
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        miette!("Could not parse ffmpeg stdout.")
                    })?;

                let ffmpeg_stderr = String::from_utf8(ffmpeg_output.stderr)
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        miette!("could not parse ffmpeg stderr.")
                    })?;

                let error = format!(
                    "ffmpeg exited with non-zero exit code.\nStdout: {}\nStderr: {}",
                    ffmpeg_stdout, ffmpeg_stderr
                );

                let verbose_info: Option<String> = is_verbose_enabled()
                    .then(|| {
                        format!(
                            "ffmpeg exited (exit code {}). Binary={:?} Arguments={:?}",
                            ffmpeg_exit_code,
                            &self.ffmpeg_binary_path, &self.ffmpeg_arguments
                        )
                    });

                FileJobResult::Errored {
                    error,
                    verbose_info,
                }
            };

            message_sender
                .send(FileJobMessage::new_finished(
                    self.queue_item,
                    FileType::Audio,
                    self.target_file_path.to_string_lossy(),
                    processing_result,
                ))
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!("Could not send FileJobMessage::Finished.")
                })?;

            Ok(())
        }
    }
}
