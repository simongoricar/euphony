use std::fs;
use std::fs::DirEntry;
use std::io::{Error, ErrorKind};
use std::path::Path;


/// Given a PathBuf scan its contents and
/// return a Result: when Ok it is a tuple of two Vec<DirEntry> elements,
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

pub fn list_dir_entry_contents(dir_entry: &DirEntry)
    -> Result<(Vec<DirEntry>, Vec<DirEntry>), Error> {
    let path = dir_entry.path();

    return if !path.is_dir() {
        Err(Error::new(ErrorKind::Other, "dir_entry is not a directory."))
    } else {
        list_directory_contents(path.as_path())
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
