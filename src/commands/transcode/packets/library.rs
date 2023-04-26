/*
use miette::{miette, Context, Result};

use crate::commands::transcode::packets::album::AlbumWorkPacket;
use crate::configuration::{Config, ConfigLibrary};
use crate::filesystem::DirectoryScan;


 */
/*
/// Represents a music library's worth of processing work (all the albums).
///
/// NOTE: Any changes in the filesystem after this instantiation will not be visible
/// in the album packets - this is a static scan.
#[derive(Clone)]
pub struct LibraryWorkPacket<'a> {
    pub name: String,
    pub album_packets: Vec<AlbumWorkPacket<'a>>,
}

impl<'a> LibraryWorkPacket<'a> {
    /// Instantiate a new `LibraryWorkPacket` by simply providing a reference to the configuration
    /// and the library (`ConfigLibrary` instance) you want.
    ///
    /// The albums are automatically scanned given the information in the provided `ConfigLibrary`.
    pub fn from_library(
        config: &'a Config,
        library: &'a ConfigLibrary,
    ) -> Result<LibraryWorkPacket<'a>> {
        // Generate list of `AlbumWorkPacket` in this library that represent a way of processing each album individually.
        // The initially-generated list of album packets will contain all albums, even those that haven't changed.
        // Use `AlbumWorkPacket::needs_processing` to check for filesystem changes since last transcode.
        let mut album_packets: Vec<AlbumWorkPacket> = Vec::new();

        let library_directory_scan =
            DirectoryScan::from_directory_path(&library.path, 0).wrap_err_with(
                || {
                    miette!(
                        "Errored while scanning library directory: {:?}",
                        library.path
                    )
                },
            )?;

        for artist_directory in library_directory_scan.directories {
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

            let artist_directory_scan =
                DirectoryScan::from_directory_entry(&artist_directory, 0)
                    .wrap_err_with(|| {
                        miette!(
                            "Errored while scanning artist directory: {:?}",
                            artist_directory.path()
                        )
                    })?;

            for album_directory in artist_directory_scan.directories {
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


 */
