use std::fs;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};
use miette::{Result, miette, IntoDiagnostic, Context};

/// Given a path, scan its contents and return
/// a Result: when Ok it is a tuple of two Vec<DirEntry> elements,
/// the first one containing the files, the second one directories.
pub fn list_directory_contents<P: AsRef<Path>>(
    directory: P,
) -> Result<(Vec<DirEntry>, Vec<DirEntry>)> {
    let mut file_list: Vec<DirEntry> = Vec::new();
    let mut directory_list: Vec<DirEntry> = Vec::new();

    let dir_read = fs::read_dir(directory.as_ref())
        .into_diagnostic()
        .wrap_err_with(|| miette!("Could not read directory."))?;

    for entry in dir_read {
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
            // Skip other types.
            continue;
        }
    }

    Ok((file_list, directory_list))
}

/// Given a `directory`, recursively scan its contents and return a Result:
/// - when `Ok`, the inner value is a vector containing DirEntry elements - files
///   in the directory (including subdirectories) up to the `maximum_recursion_depth` depth limit.
/// - when `Err`, it contains the `Error` encountered while
#[allow(dead_code)]
pub fn list_directory_files_recursively(
    directory: &Path,
    maximum_recursion_depth: usize,
) -> Result<Vec<DirEntry>> {
    let mut aggregated_files: Vec<DirEntry> = Vec::new();

    // This function works non-recursively by having a stack of
    // pending directories to search, along with their depth to ensure
    // we don't exceed `maximum_recursion_depth`.
    let mut pending_directories: Vec<(PathBuf, usize)> = Vec::new();
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


pub fn list_directory_files_recusrively_filtered<P: Into<PathBuf>>(
    directory: P,
    maximum_recursion_depth: u16,
    extensions: &[String],
) -> Result<Vec<DirEntry>> {
    let mut aggregated_files: Vec<DirEntry> = Vec::new();

    // This function works non-recursively by having a stack of
    // pending directories to search, along with their depth to ensure
    // we don't exceed `maximum_recursion_depth`.
    let mut pending_directories: Vec<(PathBuf, u16)> = Vec::new();
    pending_directories.push((directory.into(), 0));

    while !pending_directories.is_empty() {
        let (current_dir, current_depth) = pending_directories.pop()
            .expect("Could not pop directory off directory stack.");

        let (current_files, current_dirs) = list_directory_contents(current_dir.as_path())
            .wrap_err_with(|| miette!("Could not list directory contents."))?;

        // Make sure only files with matching extensions are aggregated.
        for file in current_files {
            let file_path = file.path();
            let file_ext = match file_path.extension() {
                Some(ext) => {
                    match ext.to_str() {
                        Some(extstr) => extstr,
                        None => {
                            continue;
                        }
                    }
                },
                None => {
                    continue;
                }
            };

            let file_ext_owned = &file_ext.to_owned();
            if extensions.contains(file_ext_owned) {
                aggregated_files.push(file);
            }
        }
        // aggregated_files.extend(current_files);

        if current_depth < maximum_recursion_depth {
            for sub_dir in current_dirs {
                let sub_dir_path = sub_dir.path();
                pending_directories.push((sub_dir_path, current_depth + 1));
            }
        }
    }

    Ok(aggregated_files)
}

pub fn list_dir_entry_contents(dir_entry: &DirEntry)
    -> Result<(Vec<DirEntry>, Vec<DirEntry>)> {
    let path = dir_entry.path();

    return if !path.is_dir() {
        Err(miette!("dir_entry is not a directory."))
    } else {
        list_directory_contents(path.as_path())
            .wrap_err_with(|| miette!("Could not list directory contents."))
    }

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

/// Compared to `is_file_directly_in_dir`, this searches subdirectories as well.
#[allow(dead_code)]
pub fn is_file_inside_directory<P1: AsRef<Path>, P2: AsRef<Path>>(
    file_path: P1,
    directory_path: P2,
    recursion_limit: Option<u32>,
) -> bool {
    let file_path = file_path.as_ref();
    let directory_path = directory_path.as_ref();
    
    let recursion_limit = recursion_limit.unwrap_or(16);

    if recursion_limit == 0 {
        false
    } else {
        match file_path.parent() {
            Some(parent) => {
                if parent.eq(directory_path) {
                    true
                } else {
                    is_file_inside_directory(
                        parent,
                        directory_path,
                        Some(recursion_limit - 1),
                    )
                }
            },
            None => false,
        }
    }
}

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
