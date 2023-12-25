use std::path::PathBuf;

use euphony_library::state::AlbumFileChangesV2;
use miette::{miette, Result};

use super::changes::{
    add_aggregated_file_deletion_job,
    add_file_copy_job,
    add_transcode_job,
    CopyProcessingReason,
    DeleteInTranscodedProcessingReason,
    FileJobContext,
    FileType,
    TranscodeProcessingReason,
};
use crate::{
    commands::transcode::jobs::{CancellableTask, FileJobMessage},
    console::frontends::shared::queue::QueueItemID,
};

#[inline]
fn sort_pathbuf_iterator<'a, I: IntoIterator<Item = &'a PathBuf>>(
    iterator: I,
) -> Vec<&'a PathBuf> {
    let mut vector: Vec<&PathBuf> = iterator.into_iter().collect();
    vector.sort_unstable();

    vector
}


pub trait GenerateChanges {
    fn generate_file_jobs<F: Fn(FileJobContext) -> Result<QueueItemID>>(
        &self,
        queue_item_id_generator: F,
    ) -> Result<Vec<CancellableTask<FileJobMessage>>>;
}

impl<'view> GenerateChanges for AlbumFileChangesV2<'view> {
    /// This method will generate and return a list of cancellable tasks.
    ///
    /// The `queue_item_id_generator` parameter should be a closure that will take two parameters:
    /// - `FileType`, which is the type of the file (audio or data) and
    /// - `&PathBuf`, which is the absolute path to the source file.
    ///
    /// The closure should return an `Ok(QueueItemID)`.
    /// If `Err` is returned, this method will exit early, propagating the error.
    fn generate_file_jobs<F: Fn(FileJobContext) -> Result<QueueItemID>>(
        &self,
        queue_item_id_generator: F,
    ) -> Result<Vec<CancellableTask<FileJobMessage>>> {
        let mut jobs: Vec<CancellableTask<FileJobMessage>> =
            Vec::with_capacity(self.number_of_changed_files());

        let absolute_source_to_target_path_map =
            self.tracked_source_files.as_ref().map(|files| {
                files.map_source_file_paths_to_transcoded_file_paths_absolute()
            });

        // Audio transcoding
        for path in sort_pathbuf_iterator(
            &self.added_in_source_since_last_transcode.audio,
        ) {
            let Some(source_to_target_path_map) =
                &absolute_source_to_target_path_map
            else {
                return Err(miette!("Can't map source paths to transcoded paths, no tracked files."));
            };

            add_transcode_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                source_to_target_path_map,
                path,
                FileType::Audio,
                TranscodeProcessingReason::AddedInSourceLibrary,
            )?;
        }

        for path in sort_pathbuf_iterator(
            &self.changed_in_source_since_last_transcode.audio,
        ) {
            let Some(source_to_target_path_map) =
                &absolute_source_to_target_path_map
            else {
                return Err(miette!("Can't map source paths to transcoded paths, no tracked files."));
            };

            add_transcode_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                source_to_target_path_map,
                path,
                FileType::Audio,
                TranscodeProcessingReason::ChangedInSourceLibrary,
            )?;
        }

        for path in sort_pathbuf_iterator(&self.missing_in_transcoded.audio) {
            let Some(source_to_target_path_map) =
                &absolute_source_to_target_path_map
            else {
                return Err(miette!("Can't map source paths to transcoded paths, no tracked files."));
            };

            add_transcode_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                source_to_target_path_map,
                path,
                FileType::Audio,
                TranscodeProcessingReason::MissingInTranscodedLibrary,
            )?;
        }


        // Data file copying
        for path in sort_pathbuf_iterator(
            &self.added_in_source_since_last_transcode.data,
        ) {
            let Some(source_to_target_path_map) =
                &absolute_source_to_target_path_map
            else {
                return Err(miette!("Can't map source paths to transcoded paths, no tracked files."));
            };

            add_file_copy_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                source_to_target_path_map,
                path,
                FileType::Data,
                CopyProcessingReason::AddedInSourceLibrary,
            )?;
        }

        for path in sort_pathbuf_iterator(
            &self.changed_in_source_since_last_transcode.data,
        ) {
            let Some(source_to_target_path_map) =
                &absolute_source_to_target_path_map
            else {
                return Err(miette!("Can't map source paths to transcoded paths, no tracked files."));
            };

            add_file_copy_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                source_to_target_path_map,
                path,
                FileType::Data,
                CopyProcessingReason::ChangedInSourceLibrary,
            )?;
        }

        for path in sort_pathbuf_iterator(&self.missing_in_transcoded.data) {
            let Some(source_to_target_path_map) =
                &absolute_source_to_target_path_map
            else {
                return Err(miette!("Can't map source paths to transcoded paths, no tracked files."));
            };

            add_file_copy_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                source_to_target_path_map,
                path,
                FileType::Data,
                CopyProcessingReason::MissingInTranscodedLibrary,
            )?;
        }


        // Transcoded library file deletion
        for target_path in sort_pathbuf_iterator(
            &self.removed_from_source_since_last_transcode.audio,
        ) {
            add_aggregated_file_deletion_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                target_path,
                FileType::Audio,
                DeleteInTranscodedProcessingReason::RemovedFromSourceLibrary,
            )?;
        }

        for target_path in sort_pathbuf_iterator(
            &self.removed_from_source_since_last_transcode.data,
        ) {
            add_aggregated_file_deletion_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                target_path,
                FileType::Data,
                DeleteInTranscodedProcessingReason::RemovedFromSourceLibrary,
            )?;
        }


        for path in sort_pathbuf_iterator(&self.excess_in_transcoded.audio) {
            add_aggregated_file_deletion_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                path,
                FileType::Audio,
                DeleteInTranscodedProcessingReason::ExcessInTranscodedLibrary,
            )?;
        }

        for path in sort_pathbuf_iterator(&self.excess_in_transcoded.data) {
            add_aggregated_file_deletion_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                path,
                FileType::Data,
                DeleteInTranscodedProcessingReason::ExcessInTranscodedLibrary,
            )?;
        }

        for path in sort_pathbuf_iterator(&self.excess_in_transcoded.unknown) {
            add_aggregated_file_deletion_job(
                &mut jobs,
                &self.album_view,
                &queue_item_id_generator,
                path,
                FileType::Unknown,
                DeleteInTranscodedProcessingReason::ExcessInTranscodedLibrary,
            )?;
        }

        Ok(jobs)
    }
}
