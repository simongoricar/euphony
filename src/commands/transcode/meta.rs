use std::{fs, path};
use std::collections::{HashMap, HashSet};
use std::io::{Error, ErrorKind, Read, Write};
use std::ops::Sub;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::filesystem;


#[derive(Serialize, Deserialize)]
pub struct LibraryMeta {
    pub files: HashMap<String, LibraryMetaFile>,

    #[serde(skip)]
    pub base_directory: String,
}

#[derive(Serialize, Deserialize)]
pub struct LibraryMetaFile {
    // The BLAKE3 hash was removed mid-design due to likely not being
    // fast enough to scan the entire library each time we call the command.
    pub size_bytes: u64,
    pub time_modified: f64,
    pub time_created: f64,
}


pub struct FileChanges {
    pub files_removed: Vec<String>,
    pub files_new: Vec<String>,
    pub files_changed: Vec<String>,
}


fn get_librarymeta_path_from_directory(directory_path: &Path) -> PathBuf {
    let mut final_path = directory_path.to_path_buf();
    final_path.push(".librarymeta");

    final_path
}

impl LibraryMetaFile {
    pub fn matches(&self, other_meta: &LibraryMetaFile) -> bool {
        if self.size_bytes != other_meta.size_bytes {
            return false;
        }
        if self.time_created != other_meta.time_created {
            return false;
        }
        if self.time_modified != other_meta.time_modified {
            return false;
        }

        true
    }
}

impl LibraryMeta {
    /// Given a directory path, load its .librarymeta file, if it exists, into a LibraryMeta struct.
    pub fn load(directory_path: &Path) -> Option<LibraryMeta> {
        let file_path = get_librarymeta_path_from_directory(directory_path);

        if !file_path.is_file() {
            return None;
        }

        let library_meta_string = match fs::read_to_string(file_path) {
            Ok(string) => string,
            Err(_) => {
                return None;
            }
        };

        let library_meta: LibraryMeta = match serde_json::from_str(&library_meta_string) {
            Ok(meta) => meta,
            Err(error) => {
                eprintln!("Could not decode JSON file: {:?}", error);
                return None;
            }
        };

        Some(library_meta)
    }

    /// Given a directory path, maximum directory scan depth and extensions to scan,
    /// scan the given directory recursively and generate a fresh LibraryMeta struct.
    pub fn generate(
        directory_path: &Path,
        maximum_tree_depth: Option<usize>,
        extensions: &Vec<String>,
    ) -> Result<LibraryMeta, Error> {
        const DEFAULT_MAX_DEPTH: usize = 4;

        let maximum_tree_depth = maximum_tree_depth.unwrap_or(DEFAULT_MAX_DEPTH);

        // Enumerate files (including subdirectories up to a limit).
        let files = filesystem::list_directory_files_recusrively_filtered(
            directory_path,
            maximum_tree_depth,
            extensions,
        )?;

        // Generate info about each file (limited to relevant extensions).
        // TODO Test this.
        let mut file_hashmap: HashMap<String, LibraryMetaFile> = HashMap::new();

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

            let file_metadata = LibraryMetaFile {
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

        Ok(LibraryMeta {
            base_directory: directory_path.to_str()
                .expect("Could not get library directory.")
                .to_string(),
            files: file_hashmap,
        })
    }

    /// Given a directory, save the LibraryMeta struct in question into the .librarymeta file
    /// as a JSON document.
    pub fn save(&self, directory_path: &Path, allow_overwrite: bool) -> Result<(), Error> {
        let file_path = get_librarymeta_path_from_directory(directory_path);
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
    pub fn diff(&self, current_meta_state: &LibraryMeta) -> FileChanges {
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

        // TODO Test this.
        FileChanges {
            files_removed,
            files_new,
            files_changed
        }
    }
}
