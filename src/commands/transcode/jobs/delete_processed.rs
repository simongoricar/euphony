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
use crate::console::frontends::shared::queue::QueueItemID;
use crate::globals::is_verbose_enabled;

/// One of multiple file jobs.
///
/// `DeleteProcessedFileJob` removes a transcoded audio file or copied data file
/// from the aggregated library.
pub struct DeleteProcessedFileJob {
    /// Path to the file to delete.
    target_file_path: PathBuf,

    file_type: FileType,

    /// If `true` we should ignore the error if `target_file_path` does not exist.
    ignore_if_missing: bool,

    /// `QueueItemID` this job belongs to.
    queue_item: QueueItemID,
}

impl DeleteProcessedFileJob {
    /// Initialize a new `DeleteProcessedFileJob` from the given target path to remove.
    /// If the file is missing
    pub fn new(
        target_file_path: PathBuf,
        file_type: FileType,
        ignore_if_missing: bool,
        queue_item: QueueItemID,
    ) -> Result<Self> {
        /*
         * 1. Sanity checks
         */
        if target_file_path.exists() && !target_file_path.is_file() {
            return Err(miette!("Given path exists, but is not a file!"));
        }

        if !target_file_path.exists() && !ignore_if_missing {
            return Err(miette!("Given path doesn't exist."));
        }

        Ok(Self {
            target_file_path,
            file_type,
            ignore_if_missing,
            queue_item,
        })
    }
}

impl FileJob for DeleteProcessedFileJob {
    fn run(
        &mut self,
        _cancellation_flag: &AtomicBool,
        message_sender: &Sender<FileJobMessage>,
    ) -> Result<()> {
        message_sender
            .send(FileJobMessage::new_starting(
                self.queue_item,
                self.file_type,
                self.target_file_path.to_string_lossy(),
            ))
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not send FileJobMessage::Starting.")
            })?;

        let processing_result = if !self.target_file_path.is_file() {
            if self.ignore_if_missing {
                let verbose_info = is_verbose_enabled()
                    .then(|| "File did not exist, but ignore_if_missing==true - skipping.".to_string());

                FileJobResult::Okay { verbose_info }
            } else {
                FileJobResult::Errored {
                    error: "File did not exist and ignore_if_missing != true!"
                        .to_string(),
                    verbose_info: None,
                }
            }
        } else {
            let removal_result = fs::remove_file(&self.target_file_path);

            match removal_result {
                Ok(_) => FileJobResult::Okay { verbose_info: None },
                Err(error) => FileJobResult::Errored {
                    error: error.to_string(),
                    verbose_info: None,
                },
            }
        };

        message_sender
            .send(FileJobMessage::new_finished(
                self.queue_item,
                self.file_type,
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
