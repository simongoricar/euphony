use std::fs;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};

use miette::{Context, IntoDiagnostic, miette, Result};

pub type DirectoryContents = (Vec<DirEntry>, Vec<DirEntry>);

/// Given a `Path`, scan its contents and return a `Result` containing a tuple of
/// two `Vec<DirEntry>` elements, the first one containing the files, the second one directories.
pub fn list_directory_contents<P: AsRef<Path>>(
    directory: P,
) -> Result<DirectoryContents> {
    let mut file_list: Vec<DirEntry> = Vec::new();
    let mut directory_list: Vec<DirEntry> = Vec::new();

    let directory_iterator = fs::read_dir(directory.as_ref())
        .into_diagnostic()
        .wrap_err_with(|| miette!("Could not read directory."))?;

    for entry in directory_iterator {
        let entry = entry
            .into_diagnostic()
            .wrap_err_with(|| miette!("Could not get directory entry."))?;
        
        let entry_type = entry.file_type()
            .into_diagnostic()
            .wrap_err_with(|| miette!("Could not get directory entry file type."))?;

        if entry_type.is_file() {
            file_list.push(entry);
        } else if entry_type.is_dir() {
            directory_list.push(entry);
        } else {
            // Skips other types (symlinks).
            continue;
        }
    }

    Ok((file_list, directory_list))
}

/// A wrapper around `list_directory_contents` that takes `DirEntry` references instead.
pub fn list_dir_entry_contents(
    dir_entry: &DirEntry,
) -> Result<DirectoryContents> {
    let path = dir_entry.path();
    
    return if !path.is_dir() {
        Err(miette!("dir_entry is not a directory."))
    } else {
        list_directory_contents(path.as_path())
            .wrap_err_with(|| miette!("Could not list DirEntry contents."))
    }
}

/// Given a directory `Path`, **recursively** scan its contents and return a `Result` containing
/// `Vec<DirEntry>` - files inside the directory, up to the `maximum_recursion_depth` depth limit.
#[allow(dead_code)]
pub fn recursively_list_directory_files(
    directory: &Path,
    maximum_recursion_depth: usize,
) -> Result<Vec<DirEntry>> {
    let mut aggregated_files: Vec<DirEntry> = Vec::new();

    // This function works non-recursively by having a stack of
    // pending directories to search, along with their depth to ensure
    // we don't exceed `maximum_recursion_depth`.
    let mut pending_directories: Vec<(PathBuf, usize)> = Vec::with_capacity(1);
    pending_directories.push((directory.to_path_buf(), 0));

    while !pending_directories.is_empty() {
        let (current_dir, current_depth) = pending_directories.pop()
            .expect("Could not pop directory off directory stack, even though !is_empty()");

        let (current_files, current_dirs) = list_directory_contents(current_dir.as_path())
            .wrap_err_with(|| miette!("Could not list directory contents."))?;

        aggregated_files.extend(current_files);

        if current_depth < maximum_recursion_depth {
            for sub_dir in current_dirs {
                let sub_dir_path = sub_dir.path();
                pending_directories.push((sub_dir_path, current_depth + 1));
            }
        }
    }

    Ok(aggregated_files)
}

/// Given a directory `Path`, **recursively** scan its contents and return a `Result` containing
/// `Vec<DirEntry>` - files inside the directory, up to the `maximum_recursion_depth` depth limit.
/// Additionally, the results are prefiltered to match a given set of `extensions`
/// (provide them without dots, e.g. "txt", "zip").
pub fn recursively_list_directory_files_filtered<P: Into<PathBuf>>(
    directory: P,
    maximum_recursion_depth: u16,
    allowed_extensions: &[String],
) -> Result<Vec<DirEntry>> {
    let mut aggregated_files: Vec<DirEntry> = Vec::new();

    // This function works non-recursively by having a stack of
    // pending directories to search, along with their depth to ensure
    // we don't exceed `maximum_recursion_depth`.
    let mut pending_directories: Vec<(PathBuf, u16)> = Vec::with_capacity(1);
    pending_directories.push((directory.into(), 0));

    while !pending_directories.is_empty() {
        let (current_dir, current_depth) = pending_directories.pop()
            .expect("Could not pop directory off directory stack, even though pending_directories was not empty.");

        let (current_files, current_dirs) = list_directory_contents(current_dir.as_path())
            .wrap_err_with(|| miette!("Could not list directory contents."))?;

        // Make sure only files with matching extensions are aggregated.
        for file in current_files {
            let file_path = file.path();
            let file_ext = match file_path.extension() {
                Some(ext) => ext
                    .to_str()
                    .ok_or_else(|| miette!("File contained invalid UTF-8."))?
                    .to_string(),
                None => continue
            };

            if allowed_extensions.contains(&file_ext) {
                aggregated_files.push(file);
            }
        }

        // Extend the search by pushing directories inside the current one
        // (that don't go too deep) onto the search stack.
        if current_depth < maximum_recursion_depth {
            for sub_directory in current_dirs {
                pending_directories.push(
                    (sub_directory.path(), current_depth + 1)
                );
            }
        }
    }

    Ok(aggregated_files)
}

/// Check whether the file is directly in the provided directory.
#[allow(dead_code)]
pub fn is_file_directly_in_dir<P1: AsRef<Path>, P2: AsRef<Path>>(
    file_path: P1,
    directory_path: P2,
) -> bool {
    match file_path.as_ref().parent() {
        Some(parent) => parent.eq(directory_path.as_ref()),
        None => false,
    }
}

/// Check whether the file is a "descendant" (either directly inside or in a subdirectory)
/// of a certain directory. If given, the `depth_limit` is respected, otherwise it defaults to 32.
#[allow(dead_code)]
pub fn is_file_inside_directory<P1: AsRef<Path>, P2: AsRef<Path>>(
    file_path: P1,
    directory_path: P2,
    depth_limit: Option<u32>,
) -> bool {
    let depth_limit = depth_limit.unwrap_or(32);
    let directory_path = directory_path.as_ref();

    if depth_limit == 0 {
        // We've reached the depth limit, give up.
        false
    } else {
        // We've got some depth to search, go up to the parent directory and try to match it.
        // Do this recursively until you reach the depth limit.
        match file_path.as_ref().parent() {
            Some(parent) => {
                if parent.eq(directory_path) {
                    true
                } else {
                    is_file_inside_directory(
                        parent,
                        directory_path,
                        Some(depth_limit - 1),
                    )
                }
            },
            None => false,
        }
    }
}

/// Given a `Path` get its file extension, if any.
pub fn get_path_file_extension(path: &Path) -> Result<String> {
    match path.extension() {
        Some(ext) => {
            Ok(ext
                .to_str()
                .ok_or_else(|| miette!("Could not extract file extension: errored while converting to str."))?
                .to_string())
        },
        None => {
            Err(miette!("Could not extract file extension."))
        }
    }
}
