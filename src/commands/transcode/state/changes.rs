use std::fmt::Debug;
use std::path::{Path, PathBuf};

use euphony_library::view::common::SortedFileMap;
use euphony_library::view::SharedAlbumView;
use miette::{miette, Context, Result};

// TODO Finish reorganising code into the euphony_library crate.
// TODO Try to put things in transcode::jobs into a different crate, if possible.
use crate::commands::transcode::jobs::{
    CancellableTask,
    CopyFileJob,
    DeleteProcessedFileJob,
    FileJobMessage,
    IntoCancellableTask,
    TranscodeAudioFileJob,
};
use crate::console::frontends::shared::queue::QueueItemID;



/// Describes one of three possible file types (audio, data, unknown).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum FileType {
    /// Audio files, as configured per-library.
    Audio,

    /// Data (non-audio) files, as configured per-library.
    Data,

    /// Unknown (non-audio, non-data) files.
    ///
    /// This type only appears in cases of "excess" files in the transcoded library
    /// (see `AlbumFileChangesV2::generate_from_source_and_transcoded_state`).
    Unknown,
}


#[allow(clippy::enum_variant_names)]
#[derive(Copy, Clone)]
pub enum TranscodeProcessingReason {
    AddedInSourceLibrary,
    ChangedInSourceLibrary,
    MissingInTranscodedLibrary,
}

#[allow(clippy::enum_variant_names)]
#[derive(Copy, Clone)]
pub enum CopyProcessingReason {
    AddedInSourceLibrary,
    ChangedInSourceLibrary,
    MissingInTranscodedLibrary,
}


#[allow(clippy::enum_variant_names)]
#[derive(Copy, Clone)]
pub enum DeleteInTranscodedProcessingReason {
    RemovedFromSourceLibrary,
    ExcessInTranscodedLibrary,
}


#[derive(Clone)]
pub enum FileProcessingAction {
    Transcode {
        source_path: PathBuf,
        target_path: PathBuf,
        reason: TranscodeProcessingReason,
    },
    Copy {
        source_path: PathBuf,
        target_path: PathBuf,
        reason: CopyProcessingReason,
    },
    DeleteInTranscoded {
        target_path: PathBuf,
        reason: DeleteInTranscodedProcessingReason,
    },
}

impl FileProcessingAction {
    pub fn target_path(&self) -> &Path {
        match self {
            FileProcessingAction::Transcode { target_path, .. } => {
                target_path.as_ref()
            }
            FileProcessingAction::Copy { target_path, .. } => {
                target_path.as_ref()
            }
            FileProcessingAction::DeleteInTranscoded { target_path, .. } => {
                target_path.as_ref()
            }
        }
    }
}


#[derive(Clone)]
pub struct FileJobContext {
    pub file_type: FileType,
    pub action: FileProcessingAction,
}


pub fn add_transcode_job<
    F: Fn(FileJobContext) -> Result<QueueItemID>,
    P: Into<PathBuf>,
>(
    global_job_array: &mut Vec<CancellableTask<FileJobMessage>>,
    album_view: &SharedAlbumView,
    queue_item_id_generator: &F,
    absolute_source_to_target_path_map: &SortedFileMap<PathBuf, PathBuf>,
    source_path: P,
    file_type: FileType,
    transcode_reason: TranscodeProcessingReason,
) -> Result<()> {
    let source_path = source_path.into();

    let target_path = absolute_source_to_target_path_map
        .get(&source_path)
        .ok_or_else(|| {
            miette!(
                "BUG(add_transcode_job): Map is missing audio file entry: {:?}.",
                source_path
            )
        })?;

    let queue_item_id = queue_item_id_generator(FileJobContext {
        file_type,
        action: FileProcessingAction::Transcode {
            source_path: source_path.clone(),
            target_path: target_path.to_path_buf(),
            reason: transcode_reason,
        },
    })?;

    let transcoding_job = TranscodeAudioFileJob::new(
        album_view.clone(),
        source_path,
        target_path.to_path_buf(),
        queue_item_id,
    )
    .wrap_err_with(|| miette!("Could not create TranscodeAudioFileJob."))?;

    global_job_array.push(transcoding_job.into_cancellable_task());

    Ok(())
}

pub fn add_file_copy_job<
    F: Fn(FileJobContext) -> Result<QueueItemID>,
    P: Into<PathBuf>,
>(
    global_job_array: &mut Vec<CancellableTask<FileJobMessage>>,
    album_view: &SharedAlbumView,
    queue_item_id_generator: &F,
    absolute_source_to_target_path_map: &SortedFileMap<PathBuf, PathBuf>,
    source_path: P,
    file_type: FileType,
    copy_reason: CopyProcessingReason,
) -> Result<()> {
    let source_path = source_path.into();

    let target_path = absolute_source_to_target_path_map
        .get(&source_path)
        .ok_or_else(|| {
            miette!(
                "BUG(add_file_copy_job): Map is missing audio file entry: {:?}.",
                source_path
            )
        })?;

    let queue_item_id = queue_item_id_generator(FileJobContext {
        file_type,
        action: FileProcessingAction::Copy {
            source_path: source_path.clone(),
            target_path: target_path.to_path_buf(),
            reason: copy_reason,
        },
    })?;

    let copy_job = CopyFileJob::new(
        album_view.clone(),
        source_path,
        target_path.to_path_buf(),
        queue_item_id,
    )
    .wrap_err_with(|| miette!("Could not create CopyFileJob."))?;

    global_job_array.push(copy_job.into_cancellable_task());

    Ok(())
}

pub fn add_aggregated_file_deletion_job<
    F: Fn(FileJobContext) -> Result<QueueItemID>,
    P: Into<PathBuf>,
>(
    global_job_array: &mut Vec<CancellableTask<FileJobMessage>>,
    album_view: &SharedAlbumView,
    queue_item_id_generator: &F,
    target_path: P,
    file_type: FileType,
    deletion_reason: DeleteInTranscodedProcessingReason,
) -> Result<()> {
    let target_path = target_path.into();

    let queue_item_id = queue_item_id_generator(FileJobContext {
        file_type,
        action: FileProcessingAction::DeleteInTranscoded {
            target_path: target_path.clone(),
            reason: deletion_reason,
        },
    })?;

    let transcoded_album_directory =
        album_view.read().album_directory_in_transcoded_library();

    if !target_path.starts_with(transcoded_album_directory) {
        return Err(miette!("Suspicious file deletion job (doesn't match transcoded directory): {:?}", target_path));
    }

    let copy_job = DeleteProcessedFileJob::new(
        album_view.read().euphony_configuration(),
        target_path,
        file_type,
        true,
        queue_item_id,
    )
    .wrap_err_with(|| miette!("Could not create DeleteProcessedFileJob."))?;

    global_job_array.push(copy_job.into_cancellable_task());

    Ok(())
}
