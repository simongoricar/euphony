use std::collections::HashMap;
use std::{fs, io, path};
use std::io::{ErrorKind, Write};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct LibraryMeta {
    files: HashMap<String, LibraryMetaFile>,
}

#[derive(Serialize, Deserialize)]
struct LibraryMetaFile {
    hash_blake2: String,
    size_bytes: u64,
    time_modified: u64,
    time_created: u64,
}

impl LibraryMeta {
    pub fn load(file_path: &str) -> Option<LibraryMeta> {
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

    pub fn save(&self, file_path: &str, allow_overwrite: bool) -> Result<(), io::Error> {
        let path = path::Path::new(file_path);
        if !path.exists() && !allow_overwrite {
            return Err(
                io::Error::new(
                    ErrorKind::AlreadyExists,
                    "Could not save .librarymeta, file already exists.",
                )
            );
        }

        let serialized_meta = serde_json::to_string(self)?;

        let mut file = fs::File::create(path)?;
        file.write_all(serialized_meta.as_bytes())?;

        Ok(())
    }
}
