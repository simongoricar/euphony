use std::path::Path;

use miette::{miette, Result};

use crate::commands::transcode::packets::album::AlbumWorkPacket;
use crate::configuration::Config;
use crate::filesystem;

#[derive(Clone)]
pub struct LibraryWorkPacket {
    pub name: String,
    pub album_packets: Vec<AlbumWorkPacket>,
}

impl LibraryWorkPacket {
    pub fn from_library_path<P: AsRef<Path>>(
        library_key: &str,
        library_path: P,
        config: &Config,
    ) -> Result<LibraryWorkPacket> {
        let library_path = library_path.as_ref();
        
        // Make sure this is a valid library path.
        if !config.is_library(library_path) {
            return Err(miette!("Invalid library path: not registered in configuration."));
        }

        let library = config.libraries.get(library_key)
            .ok_or_else(|| miette!("No such library: {}", library_key))?;

        let mut album_packets: Vec<AlbumWorkPacket> = Vec::new();

        let (_, artist_directories) = filesystem::list_directory_contents(library_path)?;

        for artist_directory in artist_directories {
            let directory_name = artist_directory.file_name().to_string_lossy().to_string();
            if library.ignored_directories_in_base_dir.is_some()
                && library.ignored_directories_in_base_dir.as_ref().unwrap().contains(&directory_name) {
                continue
            }

            let (_, album_directories) = match filesystem::list_dir_entry_contents(&artist_directory) {
                Ok(data) => data,
                Err(error) => {
                    return Err(miette!("Error while listing artist albums: {}", error));
                },
            };

            for album_directory in album_directories {
                let album_directory_path = album_directory.path();
                let album_packet = AlbumWorkPacket::from_album_path(
                    album_directory_path, config,
                )?;

                album_packets.push(album_packet);
            }
        }

        Ok(LibraryWorkPacket {
            name: library.name.clone(),
            album_packets,
        })
    }

    pub fn get_albums_in_need_of_processing(&mut self, config: &Config) -> Result<Vec<AlbumWorkPacket>> {
        let mut filtered_album_packets: Vec<AlbumWorkPacket> = Vec::new();
        for album_packet in &mut self.album_packets {
            if album_packet.needs_processing(config)? {
                filtered_album_packets.push(album_packet.clone());
            }
        }

        Ok(filtered_album_packets)
    }
}
