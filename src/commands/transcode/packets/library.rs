use miette::{miette, Context, Result};

use crate::commands::transcode::packets::album::AlbumWorkPacket;
use crate::configuration::{Config, ConfigLibrary};
use crate::filesystem;

#[derive(Clone)]
pub struct LibraryWorkPacket<'a> {
    pub name: String,
    pub album_packets: Vec<AlbumWorkPacket<'a>>,
}

impl<'a> LibraryWorkPacket<'a> {
    pub fn from_library(
        config: &'a Config,
        library: &'a ConfigLibrary,
    ) -> Result<LibraryWorkPacket<'a>> {
        // Generate list of `AlbumWorkPacket` in this library that represent a way of processing each album individually.
        let mut album_packets: Vec<AlbumWorkPacket> = Vec::new();

        // Iterate over artist directories, then album directories.
        let (_, artist_directories) = filesystem::list_directory_contents(
            &library.path,
        )
        .wrap_err_with(|| {
            miette!(
                "Error while listing artists for library: {}!",
                library.name
            )
        })?;

        for artist_directory in artist_directories {
            let directory_name =
                artist_directory.file_name().to_string_lossy().to_string();

            // Skip scanning of manually ignored directories in the base folder of the library.
            if let Some(ignored_directories) =
                &library.ignored_directories_in_base_directory
            {
                if ignored_directories.contains(&directory_name) {
                    continue;
                }
            }

            let (_, album_directories) =
                filesystem::list_dir_entry_contents(&artist_directory)
                    .wrap_err_with(|| {
                        miette!(
                            "Error while listing albums for artist: {}!",
                            directory_name
                        )
                    })?;

            for album_directory in album_directories {
                let album_packet = AlbumWorkPacket::from_album_path(
                    album_directory.path(),
                    config,
                    library,
                )?;

                album_packets.push(album_packet);
            }
        }

        Ok(LibraryWorkPacket {
            name: library.name.clone(),
            album_packets,
        })
    }

    pub fn get_albums_in_need_of_processing(
        &mut self,
        config: &Config,
    ) -> Result<Vec<AlbumWorkPacket<'a>>> {
        let mut filtered_album_packets: Vec<AlbumWorkPacket<'a>> = Vec::new();
        for album_packet in &mut self.album_packets {
            if album_packet.needs_processing(config)? {
                filtered_album_packets.push(album_packet.clone());
            }
        }

        Ok(filtered_album_packets)
    }
}
