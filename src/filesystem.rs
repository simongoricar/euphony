use std::fs;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};

use miette::{miette, Context, IntoDiagnostic, Result};

/// A directory scan containing `files` and `directories`.
///
/// Depending on the initialization, the scan can contain just direct children (`scan_depth == 0`)
/// or files and directories deeper in the tree (`scan_depth >= 1`).
pub struct DirectoryScan {
    pub files: Vec<DirEntry>,
    pub directories: Vec<DirEntry>,
}

impl DirectoryScan {
    /// Scan the given directory.
    ///
    /// If the `scan_depth` parameter equals `0`, only the immediate files and directories will be listed.
    /// Any non-zero number will scan up to that subdirectory depth (e.g. `1` will result in the scan
    /// containing direct files and all files directly in the directories one level down).
    pub fn from_directory_path<P: AsRef<Path>>(
        directory_path: P,
        directory_scan_depth: u16,
    ) -> Result<Self> {
        let directory_path = directory_path.as_ref();

        let mut file_list: Vec<DirEntry> = Vec::new();
        let mut directory_list: Vec<DirEntry> = Vec::new();

        // The scanning works by maintaining a queue of directories to search

        // Meaning: Vec<(directory_to_search, directory's depth)>
        let mut search_queue: Vec<(PathBuf, u16)> = Vec::new();
        search_queue.push((directory_path.to_path_buf(), 0));

        while !search_queue.is_empty() {
            let (directory_to_scan, directory_depth) = search_queue.pop()
                .expect("BUG: Could not pop directory off search queue, even though is had elements.");

            let directory_iterator = fs::read_dir(directory_to_scan)
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not read directory."))?;

            // Split the directory iterator elements into files and directories.
            for entry in directory_iterator {
                let entry = entry.into_diagnostic().wrap_err_with(|| {
                    miette!("Could not get directory entry.")
                })?;

                let entry_type = entry
                    .file_type()
                    .into_diagnostic()
                    .wrap_err_with(|| miette!("Could not get file type."))?;

                if entry_type.is_file() {
                    file_list.push(entry);
                } else if entry_type.is_dir() {
                    // If we can go deeper, queue the directory we found for further search.
                    if directory_depth < directory_scan_depth {
                        search_queue.push((entry.path(), directory_depth + 1));
                    }

                    // But always store the directories we have found so far.
                    directory_list.push(entry);
                } else {
                    // FIXME: Implement a solution for symlinks (which are currently simply ignored).
                    continue;
                }
            }
        }

        Ok(Self {
            files: file_list,
            directories: directory_list,
        })
    }

    /// Equal to `Self::from_directory_path`, but accepts a reference to `DirEntry` instead of the path.
    pub fn from_directory_entry(
        directory_entry: &DirEntry,
        scan_depth: u16,
    ) -> Result<Self> {
        let directory_path = directory_entry.path();

        if directory_path.is_dir() {
            Self::from_directory_path(directory_path, scan_depth)
        } else {
            Err(miette!(
                "Provided directory_entry is not a directory."
            ))
        }
    }

    /// Retrieve the list of scanned files, additionally filtered to specific extensions.
    /// The extension list should contain lowercase names without dots (e.g. "txt").
    #[allow(dead_code)]
    pub fn files_with_extensions(
        &self,
        extensions: &[String],
    ) -> Vec<&DirEntry> {
        self.files
            .iter()
            .filter(|item| {
                let extension = item
                    .path()
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                extensions.contains(&extension)
            })
            .collect::<Vec<&DirEntry>>()
    }
}

/// Get a file's extension (or an empty string if none).
/// Returns `Err` if the extension is not valid UTF-8.
#[inline]
pub fn get_path_extension_or_empty<P: AsRef<Path>>(path: P) -> Result<String> {
    Ok(path
        .as_ref()
        .extension()
        .unwrap_or_default()
        .to_str()
        .ok_or_else(|| miette!("Could not convert extension to UTF-8."))?
        .to_ascii_lowercase())
}
