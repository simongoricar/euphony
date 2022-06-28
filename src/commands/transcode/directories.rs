use std::io::{Error, ErrorKind};
use std::path::Path;
use crate::Config;

pub fn directory_is_artist(config: &Config, directory_path: &Path) -> bool {
    match directory_path.parent() {
        Some(root) => {
            // Check the path matches any of the libraries
            for (_, library) in &config.libraries {
                if Path::new(&library.path).eq(root) {
                    return true;
                }
            }

            false
        },
        None => false
    }
}

pub fn directory_is_album(config: &Config, directory_path: &Path) -> bool {
    match directory_path.parent() {
        Some(parent) => {
            directory_is_artist(config, parent)
        },
        None => false
    }
}


pub struct DirectoryInfo {
    pub artist: String,
    pub album: String,
}

pub fn get_directory_artist_and_album(
    config: &Config,
    directory_path: &Path,
) -> Result<DirectoryInfo, Error> {
    if !directory_is_album(config, directory_path) {
        return Err(
            Error::new(ErrorKind::Other, "Target is not album directory.")
        );
    }

    let album_title = directory_path
        .file_name()
        .expect("Could not get album directory name!");

    let parent = directory_path
        .parent()
        .expect("Could not get path parent!");
    let artist_name = parent
        .file_name()
        .expect("Could not get artist directory name!");

    Ok(DirectoryInfo {
        artist: artist_name
            .to_str()
            .expect("Could not convert artist directory name to string!")
            .to_string(),
        album: album_title
            .to_str()
            .expect("Could not convert album directory name to string!")
            .to_string()
    })
}
