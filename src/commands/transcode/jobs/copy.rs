use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crossbeam::channel::Sender;
use miette::{miette, Context, IntoDiagnostic, Result};

use crate::commands::transcode::album_state::changes::FileType;
use crate::commands::transcode::jobs::common::{
    FileJob,
    FileJobMessage,
    FileJobResult,
};
use crate::commands::transcode::views::SharedAlbumView;
use crate::console::frontends::shared::queue::QueueItemID;
use crate::filesystem::get_path_extension_or_empty;
use crate::globals::is_verbose_enabled;

/// One of multiple file jobs.
///
/// `CopyFileJob` simply copies a file (usually data/other files, not audio files) into the
/// album directory in the aggregated library.
pub struct CopyFileJob {
    /// File to copy from.
    source_file_path: PathBuf,

    /// File to copy to.
    target_file_path: PathBuf,

    /// For missing directory creation purposes, the directory `target_file_path` is in.
    target_file_directory_path: PathBuf,

    /// `QueueItemID` this job belongs to.
    queue_item: QueueItemID,
}

impl CopyFileJob {
    /// Initialize a new `CopyFileJob`.
    pub fn new(
        album: SharedAlbumView,
        source_file_path: PathBuf,
        target_file_path: PathBuf,
        queue_item: QueueItemID,
    ) -> Result<Self> {
        let album_locked = album.read();

        let transcoding_config =
            &album_locked.library_configuration().transcoding;

        /*
         * 1. Sanity checks
         */
        if !transcoding_config
            .is_path_data_file_by_extension(&source_file_path)?
        {
            return Err(miette!(
                "Invalid source file extension: \"{}\": \
                expected a tracked data file extension for this library (one of \"{:?}\").",
                get_path_extension_or_empty(source_file_path)?,
                transcoding_config.audio_file_extensions,
            ));
        }


        let target_file_directory = target_file_path
            .parent()
            .ok_or_else(|| miette!("Could not get target file directory."))?;

        Ok(Self {
            target_file_directory_path: target_file_directory.to_path_buf(),
            source_file_path,
            target_file_path,
            queue_item,
        })
    }
}

impl FileJob for CopyFileJob {
    fn run(
        &mut self,
        _cancellation_flag: &AtomicBool,
        message_sender: &Sender<FileJobMessage>,
    ) -> Result<()> {
        message_sender
            .send(FileJobMessage::new_starting(
                self.queue_item,
                FileType::Data,
                self.target_file_path.to_string_lossy(),
            ))
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not send FileJobMessage::Starting.")
            })?;

        /*
         * Step 1: create parent directories if missing.
         */
        let create_dir_result =
            fs::create_dir_all(&self.target_file_directory_path);

        if let Err(error) = create_dir_result {
            let verbose_info = is_verbose_enabled()
                .then(|| format!("fs::create_dir_all error: {error}"));

            message_sender.send(FileJobMessage::new_finished(self.queue_item, FileType::Data, self.target_file_path.to_string_lossy(), FileJobResult::Errored {
                error: "Could not create target file's missing parent directory.".to_string(),
                verbose_info
            }))
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not send FileJobMessage::Finished"))?;

            return Ok(());
        }

        /*
         * Step 2: copy the file.
         */
        // TODO Find out a way to create cancellable file copies.
        //      (Make sure to handle the half-copied edge-case - we should delete such a file)
        let copy_result =
            fs::copy(&self.source_file_path, &self.target_file_path);

        let processing_result = match copy_result {
            Ok(bytes_copied) => {
                let verbose_info = is_verbose_enabled().then(|| {
                    format!(
                        "Copy operation OK. Copied {} bytes.",
                        bytes_copied
                    )
                });

                FileJobResult::Okay { verbose_info }
            }
            Err(error) => {
                let verbose_info = is_verbose_enabled().then(|| {
                    format!(
                        "Copy operation from {:?} to {:?} failed.",
                        &self.source_file_path, &self.target_file_path
                    )
                });

                FileJobResult::Errored {
                    error: error.to_string(),
                    verbose_info,
                }
            }
        };

        message_sender
            .send(FileJobMessage::new_finished(
                self.queue_item,
                FileType::Data,
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
