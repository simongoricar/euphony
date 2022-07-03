use std::fs;
use std::fs::DirEntry;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};


/// Given a path, scan its contents and return
/// a Result: when Ok it is a tuple of two Vec<DirEntry> elements,
/// the first one containing the files, the second one directories.
pub fn list_directory_contents(directory: &Path)
    -> Result<(Vec<DirEntry>, Vec<DirEntry>), Error> {
    let mut file_list: Vec<DirEntry> = Vec::new();
    let mut directory_list: Vec<DirEntry> = Vec::new();

    let dir_read = fs::read_dir(directory)?;

    for entry in dir_read {
        let entry = entry?;
        let entry_type = entry.file_type()?;

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

/// Given a `directory`, recursively scan its contents and return
/// a Result: when Ok the inner value is a vector containing DirEntry elements - files
/// in the directory (including subdirectories) up to the `maximum_recursion_depth` depth limit.
pub fn list_directory_files_recursively(
    directory: &Path,
    maximum_recursion_depth: usize,
) -> Result<Vec<DirEntry>, Error> {
    // TODO Test this.
    let mut aggregated_files: Vec<DirEntry> = Vec::new();

    // This function works non-recursively by having a stack of
    // pending directories to search, along with their depth to ensure
    // we don't exceed `maximum_recursion_depth`.
    let mut pending_directories: Vec<(PathBuf, usize)> = Vec::new();
    pending_directories.push((directory.to_path_buf(), 0));

    while !pending_directories.is_empty() {
        let (current_dir, current_depth) = pending_directories.pop()
            .expect("Could not pop directory off directory stack.");

        let (current_files, current_dirs) = list_directory_contents(current_dir.as_path())?;

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


pub fn list_directory_files_recusrively_filtered(
    directory: &Path,
    maximum_recursion_depth: usize,
    extensions: &Vec<String>,
) -> Result<Vec<DirEntry>, Error> {
    // TODO Test this.
    let mut aggregated_files: Vec<DirEntry> = Vec::new();

    // This function works non-recursively by having a stack of
    // pending directories to search, along with their depth to ensure
    // we don't exceed `maximum_recursion_depth`.
    let mut pending_directories: Vec<(PathBuf, usize)> = Vec::new();
    pending_directories.push((directory.to_path_buf(), 0));

    while !pending_directories.is_empty() {
        let (current_dir, current_depth) = pending_directories.pop()
            .expect("Could not pop directory off directory stack.");

        let (current_files, current_dirs) = list_directory_contents(current_dir.as_path())?;

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
    -> Result<(Vec<DirEntry>, Vec<DirEntry>), Error> {
    let path = dir_entry.path();

    return if !path.is_dir() {
        Err(Error::new(ErrorKind::Other, "dir_entry is not a directory."))
    } else {
        list_directory_contents(path.as_path())
    }

}

/// Check whether the file is directly in the provided directory.
pub fn is_file_directly_in_dir(file_path: &Path, directory_path: &Path) -> bool {
    match file_path.parent() {
        Some(parent) => parent.eq(directory_path),
        None => false,
    }
}

/// Compared to `is_file_directly_in_dir`, this searches subdirectories as well.
pub fn is_file_inside_directory(file_path: &Path, directory_path: &Path, recursion_limit: Option<u32>) -> bool {
    let recursion_limit = recursion_limit.unwrap_or(16);

    if recursion_limit <= 0 {
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

pub fn get_path_file_extension(path: &Path) -> Result<String, Error> {
    match path.extension() {
        Some(ext) => {
            Ok(ext
                .to_str()
                .expect("Could not extract file extension.")
                .to_string())
        },
        None => {
            Err(
                Error::new(
                    ErrorKind::Other,
                    "Could not get file extension."
                )
            )
        }
    }
}

/// Given a DirEntry, get its file extension and return it in a Result<String, ()>.
pub fn get_dir_entry_file_extension(dir_entry: &DirEntry) -> Result<String, Error> {
    match dir_entry.path().extension() {
        Some(extension) => {
            match extension.to_os_string().into_string() {
                Ok(ext) => Ok(ext.to_ascii_lowercase()),
                Err(_) => Err(Error::new(ErrorKind::Other, "Could not get file extension.")),
            }
        },
        None => Err(Error::new(ErrorKind::Other, "Could not get file extension.")),
    }
}

/// Given a DirEntry, find and return its bare name (file / directory name).
pub fn get_dir_entry_name(dir_entry: &DirEntry) -> Result<String, Error> {
    match dir_entry.path().file_name() {
        Some(name) => {
            match name.to_str() {
                Some(name) => {
                    Ok(name.to_string())
                },
                None => {
                    Err(Error::new(ErrorKind::Other, "Could not get entry name."))
                }
            }
        },
        None => {
            Err(Error::new(ErrorKind::Other, "Could not get entry name."))
        }
    }
}
