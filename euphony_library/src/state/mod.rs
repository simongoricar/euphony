use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Formatter},
    ops::Sub,
    path::{Path, PathBuf},
};

use miette::{miette, Result};
use parking_lot::{RwLockReadGuard, RwLockWriteGuard};

use self::{
    common::FileTrackedMetadata,
    source::SourceAlbumState,
    transcoded::{TranscodedAlbumState, TranscodedAlbumStateLoadError},
};
use crate::{
    utilities::{ExtendedSortedFileList, SortedFileList},
    view::{AlbumSourceFileList, AlbumView, SharedAlbumView},
};

pub mod common;
pub mod source;
pub mod transcoded;

/// Given a set of snapshots from potential previous transcodes and the current filesystem state,
/// this struct processes the changes and sorts them into multiple groups of newly-added files,
/// modified files and removed files - essentially a diff with the previous folder contents.
///
/// This is part of the transcoding scanning process - this information is basically the last step.
/// If we have this, we know what files need to be transcoded, copied, removed, etc.
pub struct AlbumFileChangesV2<'view> {
    /// `AlbumView` these changes were generated from.
    pub album_view: SharedAlbumView<'view>,

    // List of tracked files.
    pub tracked_source_files: Option<AlbumSourceFileList<'view>>,

    /// Files in the source album directory that are new (haven't been processed yet).
    ///
    /// Paths are absolute and point to the source album directory.
    pub added_in_source_since_last_transcode: SortedFileList<PathBuf>,

    /// Files in the source album directory that have been previously processed,
    /// but have changed since.
    ///
    /// Paths are absolute and point to the source album directory.
    pub changed_in_source_since_last_transcode: SortedFileList<PathBuf>,

    /// Files in the source album directory that have been previously processed,
    /// but no longer exist in the source directory, meaning we should probably remove their
    /// counterparts from the transcoded album directory as well.
    ///
    /// This mostly happens when an album is transcoded and then, for example, the user runs
    /// a tagger through the audio files and applies a different naming scheme.
    ///
    /// Paths point to the transcoded library and are absolute.
    pub removed_from_source_since_last_transcode: SortedFileList<PathBuf>,

    /// Files that aren't new in the source directory, but are nevertheless missing from the
    /// transcoded album directory. We'll also transcode and/or copy these files
    /// (but we'll have to find out what their source counterparts are - see the
    /// `TranscodedAlbumState::original_file_paths` map).
    ///
    /// Paths are absolute and point to the source album directory.
    pub missing_in_transcoded: SortedFileList<PathBuf>,

    /// Files that don't belong to any transcode - essentially extra files we should probably remove.
    /// This is unlikely to happen unless the user has manually modified the transcoded album directory.
    ///
    /// Paths are absolute and point to the *transcoded album directory*.
    pub excess_in_transcoded: ExtendedSortedFileList<PathBuf>,
}

impl<'view> AlbumFileChangesV2<'view> {
    fn default_with_album_view(album: SharedAlbumView<'view>) -> Self {
        Self {
            album_view: album,
            tracked_source_files: None,
            added_in_source_since_last_transcode: SortedFileList::default(),
            changed_in_source_since_last_transcode: SortedFileList::default(),
            removed_from_source_since_last_transcode: SortedFileList::default(),
            missing_in_transcoded: SortedFileList::default(),
            excess_in_transcoded: ExtendedSortedFileList::default(),
        }
    }

    /// Generate an `AlbumFileChangesV2` instance by comparing several saved and fresh filesystem states:
    /// - `saved_source_state` is, if previously transcoded, the source album state as saved in `.album.source-state.euphony`,
    /// - `fresh_source_state` is the fresh filesystem state of the source album directory,
    /// - `saved_transcoded_state` is, if previously transcoded, the transcoded album map as saved in `.album.transcode-state.euphony`,
    /// - `fresh_transcoded_state` is the fresh filesystem state of the transcoded album directory (album directory-relative paths).
    ///
    /// `album` is a reference to the `AlbumView` the album states are associated with and
    /// `album_file_list` is the associated source file list.
    pub fn generate_from_source_and_transcoded_state(
        saved_source_state: Option<SourceAlbumState>,
        fresh_source_state: SourceAlbumState,
        saved_transcoded_state: Option<TranscodedAlbumState>,
        fresh_transcoded_state: TranscodedAlbumState,
        album: SharedAlbumView<'view>,
        album_file_list: AlbumSourceFileList<'view>,
    ) -> Result<Self> {
        let (
            configuration,
            library_configuration,
            source_album_directory,
            transcoded_album_directory,
        ) = {
            let album_locked = album.read();

            (
                album_locked.euphony_configuration().clone(),
                album_locked.library_configuration().clone(),
                album_locked.album_directory_in_source_library(),
                album_locked.album_directory_in_transcoded_library(),
            )
        };

        // We divide the files into five groups:
        //
        // 1. *added since last transcode* (and not present in the transcoded directory)
        // 2. *changed since last transcode* (and previous transcoded version present in the transcoded directory)
        // 3. *removed since last transcode* (and previous transcoded version present in the transcoded directory)
        // 4. *not new, but missing from transcode* (likely from a manual user removal in the transcoded directory)
        // 5. *unexpected excess file in transcode directory* (likely from user intervention)
        //
        // **The groups are disjoint.**


        let saved_source_album_file_state = &saved_source_state
            .map(|state| state.tracked_files)
            .unwrap_or_default();

        // Relative paths for previously-transcoded audio and data files in the source directory
        // (loaded from `.album.transcode-state.euphony`).
        let saved_source_file_list_audio = saved_source_album_file_state
            .audio_files
            .keys()
            .cloned()
            .collect::<HashSet<String>>();
        let saved_source_file_list_data = saved_source_album_file_state
            .data_files
            .keys()
            .cloned()
            .collect::<HashSet<String>>();


        let fresh_source_album_file_state = &fresh_source_state.tracked_files;

        // Relative paths for current audio and data files in the source directory.
        let fresh_source_file_list_audio = fresh_source_album_file_state
            .audio_files
            .keys()
            .cloned()
            .collect::<HashSet<String>>();
        let fresh_source_file_list_data = fresh_source_album_file_state
            .data_files
            .keys()
            .cloned()
            .collect::<HashSet<String>>();



        let saved_transcoded_file_state = saved_transcoded_state
            .as_ref()
            .map(|state| state.transcoded_files.clone())
            .unwrap_or_default();


        // Relative paths for previously-transcoded audio and data files.
        // Note that audio extensions match the transcode output extension (e.g. MP3),
        // NOT source extension (e.g. FLAC).
        let saved_transcoded_file_list_audio = saved_transcoded_file_state
            .audio_files
            .keys()
            .cloned()
            .collect::<HashSet<String>>();
        let saved_transcoded_file_list_data = saved_transcoded_file_state
            .data_files
            .keys()
            .cloned()
            .collect::<HashSet<String>>();


        let fresh_transcoded_file_state =
            &fresh_transcoded_state.transcoded_files;

        // Relative paths for the current state of audio and data files in the transcoded album directory.
        // Note that audio extensions match the transcode output extension (e.g. MP3),
        // NOT source extension (e.g. FLAC).
        let fresh_transcoded_file_list_audio = fresh_transcoded_file_state
            .audio_files
            .keys()
            .cloned()
            .collect::<HashSet<String>>();
        let fresh_transcoded_file_list_data = fresh_transcoded_file_state
            .data_files
            .keys()
            .cloned()
            .collect::<HashSet<String>>();


        let source_to_transcode_relative_path_map = album_file_list
            .map_source_file_paths_to_transcoded_file_paths_relative();


        /*
         * Group 1: files that have been added since the last transcode
         */
        let added_in_source_since_last_transcode = {
            let audio_files_added =
                fresh_source_file_list_audio.sub(&saved_source_file_list_audio);
            let data_files_added =
                fresh_source_file_list_data.sub(&saved_source_file_list_data);

            SortedFileList::new(
                Self::convert_relative_paths_to_absolute(
                    &source_album_directory,
                    audio_files_added,
                ),
                Self::convert_relative_paths_to_absolute(
                    &source_album_directory,
                    data_files_added,
                ),
            )
        };


        /*
         * Group 2: files that have been changed in the source album directory since last transcode
         */
        let changed_in_source_since_last_transcode = {
            let audio_files_changed = Self::filter_to_changed_files(
                fresh_source_file_list_audio
                    .intersection(&saved_source_file_list_audio),
                &saved_source_album_file_state.audio_files,
                &fresh_source_album_file_state.audio_files,
            );

            let data_files_changed = Self::filter_to_changed_files(
                fresh_source_file_list_data
                    .intersection(&saved_source_file_list_data),
                &saved_source_album_file_state.data_files,
                &fresh_source_album_file_state.data_files,
            );

            SortedFileList::new(
                Self::convert_relative_paths_to_absolute(
                    &source_album_directory,
                    audio_files_changed,
                ),
                Self::convert_relative_paths_to_absolute(
                    &source_album_directory,
                    data_files_changed,
                ),
            )
        };


        /*
         * Group 3: files that have been removed from the source album directory and whose
         *          transcoded/copied versions are still present in the transcoded album directory.
         */
        let removed_from_source_since_last_transcode = {
            let audio_files_removed = saved_source_file_list_audio
                .sub(&fresh_source_file_list_audio)
                .into_iter()
                .filter_map(|audio_file| {
                    // We don't need to bother with the file if it doesn't exist
                    // in the transcoded directory.
                    let transcoded_file_path =
                        match SourceAlbumState::get_transcoded_file_path(
                            &configuration,
                            &library_configuration,
                            &audio_file,
                        ) {
                            Ok(transcoded_path) => transcoded_path,
                            Err(error) => {
                                return Some(Err(error));
                            }
                        };

                    match transcoded_file_path.exists()
                        && transcoded_file_path.is_file()
                    {
                        true => None,
                        false => Some(Ok(audio_file)),
                    }
                })
                .collect::<Result<Vec<String>>>()?;

            let data_files_removed = saved_source_file_list_data
                .sub(&fresh_source_file_list_data)
                .into_iter()
                .filter_map(|data_file| {
                    // We don't need to bother with the file if it doesn't exist
                    // in the transcoded directory.
                    let transcoded_file_path =
                        match SourceAlbumState::get_transcoded_file_path(
                            &configuration,
                            &library_configuration,
                            &data_file,
                        ) {
                            Ok(transcoded_path) => transcoded_path,
                            Err(error) => {
                                return Some(Err(error));
                            }
                        };

                    match transcoded_file_path.exists()
                        && transcoded_file_path.is_file()
                    {
                        true => None,
                        false => Some(Ok(data_file)),
                    }
                })
                .collect::<Result<Vec<String>>>()?;

            SortedFileList::new(
                Self::convert_relative_paths_to_absolute(
                    &transcoded_album_directory,
                    audio_files_removed,
                ),
                Self::convert_relative_paths_to_absolute(
                    &transcoded_album_directory,
                    data_files_removed,
                ),
            )
        };


        /*
         * Group 4: files that aren't new (exist in previous transcode file list), but are
         *          still somehow missing from the transcoded album directory.
         */
        let missing_in_transcoded = {
            // Audio files.
            let fresh_transcoded_file_list_audio_pathbuf =
                fresh_transcoded_file_list_audio
                    .iter()
                    .map(PathBuf::from)
                    .collect::<HashSet<PathBuf>>();

            let unchanged_source_audio_files = Self::filter_to_unchanged_files(
                fresh_source_file_list_audio
                    .intersection(&saved_source_file_list_audio),
                &saved_source_album_file_state.audio_files,
                &fresh_source_album_file_state.audio_files,
            )
            .into_iter()
            .map(PathBuf::from)
            .collect::<HashSet<PathBuf>>();

            let missing_audio_files = unchanged_source_audio_files
                .into_iter()
                .filter(|unchanged_audio_file_source_path| {
                    let unchanged_audio_file_transcoded_path = source_to_transcode_relative_path_map
                        .get(unchanged_audio_file_source_path)
                        .expect("BUG: Missing audio file path in source->transcode relative map.");

                    !fresh_transcoded_file_list_audio_pathbuf.contains(unchanged_audio_file_transcoded_path)
                })
                .collect::<Vec<PathBuf>>();


            // Data files.
            let unchanged_source_data_files = Self::filter_to_unchanged_files(
                fresh_source_file_list_data
                    .intersection(&saved_source_file_list_data),
                &saved_source_album_file_state.data_files,
                &fresh_source_album_file_state.data_files,
            )
            .into_iter()
            .map(PathBuf::from)
            .collect::<HashSet<PathBuf>>();

            let fresh_transcoded_file_list_data_pathbuf =
                fresh_transcoded_file_list_data
                    .iter()
                    .map(PathBuf::from)
                    .collect::<HashSet<PathBuf>>();

            let missing_data_files = unchanged_source_data_files
                .into_iter()
                .filter(|unchanged_data_file_source_path| {
                    let unchanged_data_file_transcoded_path = source_to_transcode_relative_path_map
                        .get(unchanged_data_file_source_path)
                        .expect("BUG: Missing data file path in source->transcode relative map.");

                    !fresh_transcoded_file_list_data_pathbuf.contains(
                        unchanged_data_file_transcoded_path
                    )
                })
                .collect::<Vec<PathBuf>>();


            SortedFileList::new(
                Self::convert_relative_paths_to_absolute(
                    &source_album_directory,
                    missing_audio_files,
                ),
                Self::convert_relative_paths_to_absolute(
                    &source_album_directory,
                    missing_data_files,
                ),
            )
        };


        /*
         * Group 5: unexpected excess files in the transcoded directory
         *          (not matching previous transcode)
         */
        let excess_in_transcoded = {
            let fresh_state_in_transcoded_directory =
                fresh_transcoded_file_list_audio
                    .union(&fresh_transcoded_file_list_data)
                    .map(PathBuf::from)
                    .collect::<HashSet<PathBuf>>();

            let previous_transcode_expected_files =
                saved_transcoded_file_list_audio
                    .union(&saved_transcoded_file_list_data)
                    .map(PathBuf::from)
                    .collect::<HashSet<PathBuf>>();

            let expected_transcoded_directory_files =
                source_to_transcode_relative_path_map
                    .into_flattened_map()
                    .values()
                    .cloned()
                    .collect::<HashSet<PathBuf>>();

            let excess_files = fresh_state_in_transcoded_directory
                .sub(&previous_transcode_expected_files)
                .sub(&expected_transcoded_directory_files);

            // We now sort the files based on the configuration.
            let mut excess_audio_files: Vec<PathBuf> = Vec::new();
            let mut excess_data_files: Vec<PathBuf> = Vec::new();
            let mut excess_unknown_files: Vec<PathBuf> = Vec::new();

            for excess_file in excess_files {
                if library_configuration
                    .transcoding
                    .is_path_audio_file_by_extension(&excess_file)?
                {
                    excess_audio_files.push(excess_file);
                } else if library_configuration
                    .transcoding
                    .is_path_data_file_by_extension(&excess_file)?
                {
                    excess_data_files.push(excess_file);
                } else {
                    // This can happen if the user copies some completely other file into the
                    // transcoded album directory.
                    excess_unknown_files.push(excess_file);
                }
            }

            ExtendedSortedFileList::new(
                Self::convert_relative_paths_to_absolute(
                    &transcoded_album_directory,
                    excess_audio_files,
                ),
                Self::convert_relative_paths_to_absolute(
                    &transcoded_album_directory,
                    excess_data_files,
                ),
                Self::convert_relative_paths_to_absolute(
                    &transcoded_album_directory,
                    excess_unknown_files,
                ),
            )
        };

        Ok(Self {
            album_view: album,
            tracked_source_files: Some(album_file_list),
            added_in_source_since_last_transcode,
            changed_in_source_since_last_transcode,
            removed_from_source_since_last_transcode,
            missing_in_transcoded,
            excess_in_transcoded,
        })
    }

    /// Generate an `AlbumFileChangesV2` instance that contains changes required
    /// to fully remove a transcoded album.
    ///
    /// This is useful in cases where the source album is completely removed between transcodes
    /// and otherwise isn't picked up by the diffing code (and needs to be manually added).
    ///
    /// `album` is a reference to the `AlbumView` the album states are associated with and
    /// `album_file_list` is the associated source file list.
    pub fn generate_entire_transcoded_album_deletion<P: AsRef<Path>>(
        album: SharedAlbumView<'view>,
        album_path_relative_to_library: P,
    ) -> Result<Self> {
        let transcoded_album_directory = {
            let album_path_relative_to_library =
                album_path_relative_to_library.as_ref();
            if !album_path_relative_to_library.is_relative() {
                return Err(miette!(
                    "Invalid album_path_relative_to_library: not relative."
                ));
            }

            let album_view = album.read();
            let artist_view = album_view.read_lock_artist();
            let library_view = artist_view.read_lock_library();

            let configuration = library_view.euphony_configuration.clone();

            let transcoded_library_directory =
                Path::new(&configuration.aggregated_library.path)
                    .join(album_path_relative_to_library);


            transcoded_library_directory
        };

        if !transcoded_album_directory.exists() {
            // No files to remove.
            return Ok(Self::default_with_album_view(album));
        }

        // We don't want to delete the entire directory, so we'll carefully delete the files
        // that we consider to have transcoded/copied ourselves.
        // This means loading the `.album.transcode-state.euphony` file and deleting just tracked files.

        let saved_transcoded_state =
            match TranscodedAlbumState::load_from_directory(
                &transcoded_album_directory,
            ) {
                Ok(state) => state,
                Err(error) => {
                    return match error {
                        TranscodedAlbumStateLoadError::NotFound => {
                            Ok(Self::default_with_album_view(album))
                        }
                        TranscodedAlbumStateLoadError::SchemaVersionMismatch(
                            _,
                        ) => Ok(Self::default_with_album_view(album)),
                        _ => Err(error.into()),
                    }
                }
            };

        let audio_file_list: Vec<&String> = saved_transcoded_state
            .transcoded_files
            .audio_files
            .keys()
            .filter(|path| {
                let absolute_transcoded_file_path =
                    transcoded_album_directory.join(path);

                absolute_transcoded_file_path.exists()
                    && absolute_transcoded_file_path.is_file()
            })
            .collect();

        let data_file_list: Vec<&String> = saved_transcoded_state
            .transcoded_files
            .data_files
            .keys()
            .filter(|path| {
                let absolute_transcoded_file_path =
                    transcoded_album_directory.join(path);

                absolute_transcoded_file_path.exists()
                    && absolute_transcoded_file_path.is_file()
            })
            .collect();


        let removed_from_source_since_last_transcode = {
            SortedFileList::new(
                Self::convert_relative_paths_to_absolute(
                    &transcoded_album_directory,
                    audio_file_list,
                ),
                Self::convert_relative_paths_to_absolute(
                    &transcoded_album_directory,
                    data_file_list,
                ),
            )
        };

        Ok(Self {
            album_view: album,
            tracked_source_files: None,
            added_in_source_since_last_transcode: SortedFileList::default(),
            changed_in_source_since_last_transcode: SortedFileList::default(),
            removed_from_source_since_last_transcode,
            missing_in_transcoded: SortedFileList::default(),
            excess_in_transcoded: ExtendedSortedFileList::default(),
        })
    }

    /// Returns `true` if any changes were detected since last transcode
    /// (essentially always `true` if no previous transcoding has been done
    /// and the directory has some audio/data files).
    pub fn has_changes(&self) -> bool {
        !self.added_in_source_since_last_transcode.is_empty()
            || !self.changed_in_source_since_last_transcode.is_empty()
            || !self.removed_from_source_since_last_transcode.is_empty()
            || !self.missing_in_transcoded.is_empty()
            || !self.excess_in_transcoded.is_empty()
    }

    /// Return the total number of changed files.
    #[inline]
    pub fn number_of_changed_files(&self) -> usize {
        self.number_of_changed_audio_files()
            + self.number_of_changed_data_files()
    }

    pub fn number_of_changed_audio_files(&self) -> usize {
        self.added_in_source_since_last_transcode.audio.len()
            + self.changed_in_source_since_last_transcode.audio.len()
            + self.removed_from_source_since_last_transcode.audio.len()
            + self.missing_in_transcoded.audio.len()
            + self.excess_in_transcoded.audio.len()
    }

    pub fn number_of_changed_data_files(&self) -> usize {
        self.added_in_source_since_last_transcode.data.len()
            + self.changed_in_source_since_last_transcode.data.len()
            + self.removed_from_source_since_last_transcode.data.len()
            + self.missing_in_transcoded.data.len()
            + self.excess_in_transcoded.data.len()
            + self.excess_in_transcoded.unknown.len()
    }

    /// Generate a `SourceAlbumState` (deserialized version of `.album.source-state.euphony` file),
    /// usually with the intent to save a fresh version of it to disk.
    ///
    /// This method does no further disk lookups, all information is already in the memory.
    pub fn generate_source_album_state(&self) -> Result<SourceAlbumState> {
        SourceAlbumState::generate_from_tracked_files(
            self.tracked_source_files.as_ref().ok_or_else(|| {
                miette!("Can't generate source album state, no tracked files.")
            })?,
            self.read_lock_album().album_directory_in_source_library(),
        )
    }

    /// Generate a `TranscodedAlbumState`
    /// (deserialized version of `.album.transcode-state.euphony` file),
    /// usually with the intent to save a fresh version of it to disk.
    ///
    /// This method **does further disk lookups**.
    pub fn generate_transcoded_album_state(
        &self,
    ) -> Result<TranscodedAlbumState> {
        TranscodedAlbumState::generate_from_tracked_files(
            self.tracked_source_files.as_ref().ok_or_else(|| {
                miette!(
                    "Can't generate transcoded album state, no tracked files."
                )
            })?,
            self.read_lock_album()
                .album_directory_in_transcoded_library(),
        )
    }


    fn filter_to_changed_files<'s, I: Iterator<Item = &'s String>>(
        map_key_iterator: I,
        first_metadata_map: &HashMap<String, FileTrackedMetadata>,
        second_metadata_map: &HashMap<String, FileTrackedMetadata>,
    ) -> Vec<String> {
        map_key_iterator
            .filter_map(|file_name| {
                let first_metadata = first_metadata_map
                    .get(file_name.as_str())
                    .expect("BUG: Could not find intersecting key in first metadata map.");

                let second_metadata = second_metadata_map
                    .get(file_name.as_str())
                    .expect("BUG: Could not find intersecting key in second metadata map.");

                match first_metadata.matches(second_metadata) {
                    true => {
                        None
                    }
                    false => Some(file_name.to_string())
                }
            })
            .collect()
    }

    fn filter_to_unchanged_files<'s, I: Iterator<Item = &'s String>>(
        map_key_iterator: I,
        first_metadata_map: &HashMap<String, FileTrackedMetadata>,
        second_metadata_map: &HashMap<String, FileTrackedMetadata>,
    ) -> Vec<String> {
        map_key_iterator
            .filter_map(|file_name| {
                let first_metadata = first_metadata_map
                    .get(file_name.as_str())
                    .expect("BUG: Could not find intersecting key in first metadata map.");

                let second_metadata = second_metadata_map
                    .get(file_name.as_str())
                    .expect("BUG: Could not find intersecting key in second metadata map.");

                match first_metadata.matches(second_metadata) {
                    true => {
                        Some(file_name.to_string())
                    }
                    false => None
                }
            })
            .collect()
    }

    /// Given an iterator over relative paths (can be `String`, `str`),
    /// construct a vector that contains absolute paths.
    fn convert_relative_paths_to_absolute<
        D: AsRef<Path>,
        E: AsRef<Path>,
        I: IntoIterator<Item = E>,
    >(
        base_directory: D,
        paths: I,
    ) -> Vec<PathBuf> {
        let base_directory = base_directory.as_ref();

        paths
            .into_iter()
            .map(|item| base_directory.join(item))
            .collect()
    }

    pub fn read_lock_album(&self) -> RwLockReadGuard<'_, AlbumView<'view>> {
        self.album_view.read()
    }

    #[allow(dead_code)]
    pub fn write_lock_library(&self) -> RwLockWriteGuard<'_, AlbumView<'view>> {
        self.album_view.write()
    }
}

impl<'a> Debug for AlbumFileChangesV2<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AlbumFileChangesV2 {{\n\
            \tadded_in_source_since_last_transcode={:?}\n\
            \tchanged_in_source_since_last_transcode={:?}\n\
            \tremoved_in_source_since_last_transcode={:?}\n\
            \tmissing_in_transcoded={:?}\n\
            \texcess_in_transcoded={:?}\n\
            }}",
            self.added_in_source_since_last_transcode,
            self.changed_in_source_since_last_transcode,
            self.removed_from_source_since_last_transcode,
            self.missing_in_transcoded,
            self.excess_in_transcoded,
        )
    }
}
