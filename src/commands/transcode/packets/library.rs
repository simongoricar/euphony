use std::io::{Error, ErrorKind};
use std::path::Path;
use crate::commands::transcode::packets::album::AlbumWorkPacket;
use crate::{Config, filesystem};

pub struct LibraryWorkPacket {
    pub name: String,
    pub album_packets: Vec<AlbumWorkPacket>,
}

impl LibraryWorkPacket {
    pub fn from_library_path(
        library_name: &str,
        library_path: &Path,
        config: &Config,
    ) -> Result<LibraryWorkPacket, Error> {
        // Make sure this is a valid library path.
        if !config.is_library(library_path) {
            return Err(
                Error::new(
                    ErrorKind::Other,
                    "Invalid library path: not registered in configuration."
                )
            );
        }

        let mut album_packets: Vec<AlbumWorkPacket> = Vec::new();

        let (_, artist_directories) = filesystem::list_directory_contents(library_path)?;

        for artist_directory in artist_directories {
            let (_, album_directories) = match filesystem::list_dir_entry_contents(&artist_directory) {
                Ok(data) => data,
                Err(error) => {
                    return Err(
                        Error::new(
                            ErrorKind::Other,
                            format!(
                                "Error while listing artist albums: {}",
                                error,
                            ),
                        )
                    )
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
            name: library_name.to_string(),
            album_packets,
        })
    }

    pub fn get_albums_in_need_of_processing(&mut self, config: &Config) -> Result<Vec<AlbumWorkPacket>, Error> {
        let mut filtered_album_packets: Vec<AlbumWorkPacket> = Vec::new();
        for album_packet in &mut self.album_packets {
            if album_packet.needs_processing(config)? {
                filtered_album_packets.push(album_packet.clone());
            }
        }

        Ok(filtered_album_packets)
    }
}