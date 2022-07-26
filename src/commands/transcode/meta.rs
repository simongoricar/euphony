use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::fs;
use std::io::{Error, ErrorKind, Write};
use std::ops::Sub;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::{Config, filesystem};
use crate::commands::transcode::dirs::AlbumDirectoryInfo;
use crate::commands::transcode::packets::file::{FilePacketAction, FileWorkPacket};

const ALBUM_METADATA_FILE_NAME: &str = ".album.euphony";

/// Given a directory path, construct the full path to the album metadata file (.album.euphony).
/// Example: given "D:/hello/world" (as a Path), we would get "D:/hello/world/.album.euphony" (as a PathBuf).
fn get_album_metadata_filepath(directory_path: &Path) -> PathBuf {
    let mut final_path = directory_path.to_path_buf();
    final_path.push(ALBUM_METADATA_FILE_NAME);

    final_path
}

/// We store file creation and modification in 64-bit floats, but we usually compare two times
/// that should match using some tolerance. This function is useful for the mentioned task,
/// when you set the `max_distance` to a tolerance of your choice.
fn f64_approximate_eq(first: f64, second: f64, max_distance: f64) -> bool {
    let distance = first.sub(second).abs();
    distance.lt(&max_distance)
}


/// Represents a single album and its associated tracked files.
/// This is the structure that is generated/loaded from/saved into .album.euphony files.
#[derive(Serialize, Deserialize, Clone)]
pub struct AlbumMetadata {
    pub files: HashMap<String, AlbumMetadataFile>,

    #[serde(skip)]
    pub base_directory: String,
}

impl AlbumMetadata {
    /// Given a directory path, load its .album.euphony file, if it exists, into a LibraryMeta struct.
    pub fn load(directory_path: &Path) -> Result<Option<AlbumMetadata>, Error> {
        let file_path = get_album_metadata_filepath(directory_path);
        if !file_path.is_file() {
            return Ok(None);
        }

        let library_meta_string = fs::read_to_string(file_path)?;
        let library_meta: AlbumMetadata = serde_json::from_str(&library_meta_string)?;

        Ok(Some(library_meta))
    }

    /// Given a directory path, maximum directory scan depth and extensions to scan,
    /// scan the given directory recursively and generate a fresh LibraryMeta struct.
    pub fn generate(
        directory_path: &Path,
        maximum_tree_depth: Option<usize>,
        extensions: &Vec<String>,
    ) -> Result<AlbumMetadata, Error> {
        const DEFAULT_MAX_DEPTH: usize = 4;

        let maximum_tree_depth = maximum_tree_depth.unwrap_or(DEFAULT_MAX_DEPTH);

        // Enumerate files (including subdirectories up to a limit).
        let files = filesystem::list_directory_files_recusrively_filtered(
            directory_path,
            maximum_tree_depth,
            extensions,
        )?;

        // Generate info about each file (limited to relevant extensions).
        let mut file_hashmap: HashMap<String, AlbumMetadataFile> = HashMap::new();

        for file in files {
            let file_metadata = file.metadata()?;

            // Calculate size in bytes
            let file_size_bytes = file_metadata.len();

            // Get file creation and modification time
            let file_created_at_duration = match file_metadata.created()?
                .duration_since(UNIX_EPOCH) {
                    Ok(duration) => duration,
                    Err(_) => {
                        return Err(Error::new(ErrorKind::Other, "Could not get file creation time."));
                    }
            };
            let file_modified_at_duration = match file_metadata.modified()?
                .duration_since(UNIX_EPOCH) {
                    Ok(duration) => duration,
                    Err(_) => {
                        return Err(Error::new(ErrorKind::Other, "Could not get file modification time."));
                    }
            };

            let file_metadata = AlbumMetadataFile {
                size_bytes: file_size_bytes,
                time_modified: file_modified_at_duration.as_secs_f64(),
                time_created: file_created_at_duration.as_secs_f64(),
            };

            let file_path = file.path();
            let file_path_relative_to_meta_file = match pathdiff::diff_paths(file_path, directory_path) {
                Some(relative_path) => relative_path,
                None => {
                    return Err(Error::new(ErrorKind::Other, "Could not generate relative path."));
                }
            };
            let file_path_relative_to_meta_file = match file_path_relative_to_meta_file.to_str() {
                Some(str) => {
                    String::from(str)
                },
                None => {
                    return Err(Error::new(ErrorKind::Other, "Could not get string from relative path."));
                }
            };

            file_hashmap.insert(file_path_relative_to_meta_file, file_metadata);
        }

        Ok(AlbumMetadata {
            base_directory: directory_path.to_str()
                .expect("Could not get library directory.")
                .to_string(),
            files: file_hashmap,
        })
    }

    /// Given a directory, save the LibraryMeta struct in question into the .album.euphony file
    /// as a JSON document.
    pub fn save(&self, directory_path: &Path, allow_overwrite: bool) -> Result<(), Error> {
        let file_path = get_album_metadata_filepath(directory_path);
        if file_path.exists() && !allow_overwrite {
            return Err(
                Error::new(
                    ErrorKind::AlreadyExists,
                    "File already exists.",
                )
            );
        }

        let serialized_meta = serde_json::to_string(self)?;

        let mut file = fs::File::create(file_path)?;
        file.write_all(serialized_meta.as_bytes())?;

        Ok(())
    }

    /// Given another instance of the LibraryMeta struct (expected to be the fresh one),
    /// compare them and generate a list of new, changed and removed files between the snapshots.
    pub fn diff(&self, current_meta_state: &AlbumMetadata) -> FileChanges {
        let saved_file_paths: HashSet<&String> = self.files.keys().collect();
        let current_file_paths: HashSet<&String> = current_meta_state.files.keys().collect();

        // Compute new files.
        let mut files_new: Vec<String> = current_file_paths
            .sub(&saved_file_paths)
            .iter()
            .map(|item| item.to_owned().clone())
            .collect();
        // We don't need stable sorting anyways (there should be no equal elements).
        files_new.sort_unstable();

        // Compute removed files.
        let mut files_removed: Vec<String> = saved_file_paths
            .sub(&current_file_paths)
            .iter()
            .map(|item| item.to_owned().clone())
            .collect();
        files_removed.sort_unstable();

        // Compute changed files.
        let mut files_changed: Vec<String> = Vec::new();

        let matching_files = saved_file_paths
            .intersection(&current_file_paths);

        for matching_file_name in matching_files {
            let saved_file_meta = self.files.get(*matching_file_name)
                .expect("No matching file meta in self.files, even though it was in the intersection!");
            let current_file_meta = current_meta_state.files.get(*matching_file_name)
                .expect("No matching file meta in current_meta_state.files, even though it was in the intersection!");

            if !saved_file_meta.matches(current_file_meta) {
                files_changed.push(matching_file_name.to_owned().clone());
            }
        }

        FileChanges {
            files_removed,
            files_new,
            files_changed
        }
    }

    /// Improved version of the diff algorithm, which first does a diff and then adds any
    /// files that are not present in the aggregated library (otherwise we can remove files
    /// from the aggregated library, but no changes will be detected on a library transcode).
    pub fn diff_or_missing(
        &self,
        current_meta_state: &AlbumMetadata,
        album_info: &AlbumDirectoryInfo,
        config: &Config,
    ) -> Result<FileChanges, Error> {
        // TODO Test this code.
        let mut files_missing_in_target: Vec<String> = Vec::new();

        for (file_name, _) in &self.files {
            // Check if this file exists in the target directory.
            // If it doesn't, add it to the missing file list.
            let file_packet = FileWorkPacket::new(
                Path::new(file_name),
                album_info,
                config,
                FilePacketAction::Process,
            )?;

            if !file_packet.target_file_exists() {
                files_missing_in_target.push(file_name.clone());
            }
        }

        let mut diff = self.diff(current_meta_state);
        diff.files_new.extend(files_missing_in_target);

        Ok(diff)
    }
}


/// This struct holds information about a single tracked file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AlbumMetadataFile {
    // The BLAKE3 hash was removed mid-design due to likely not being
    // fast enough to scan the entire library each time we call the command.
    pub size_bytes: u64,
    pub time_modified: f64,
    pub time_created: f64,
}

impl AlbumMetadataFile {
    pub fn matches(&self, other_meta: &AlbumMetadataFile) -> bool {
        if self.size_bytes != other_meta.size_bytes {
            return false;
        }

        static DEFAULT_MAX_DISTANCE: f64 = 0.01;

        if !f64_approximate_eq(self.time_created, other_meta.time_created, DEFAULT_MAX_DISTANCE) {
            return false;
        }

        if !f64_approximate_eq(self.time_modified, other_meta.time_modified, DEFAULT_MAX_DISTANCE) {
            return false;
        }

        true
    }
}


#[derive(Debug)]
pub struct FileChanges {
    pub files_new: Vec<String>,
    pub files_changed: Vec<String>,
    pub files_removed: Vec<String>,
}

impl FileChanges {
    pub fn has_any_changes(&self) -> bool {
        self.files_new.len() > 0
            || self.files_changed.len() > 0
            || self.files_removed.len() > 0
    }
}
