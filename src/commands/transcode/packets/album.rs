use std::path::{Path, PathBuf};

use miette::Result;

use crate::cached::CachedValue;
use crate::commands::transcode::directories::AlbumDirectoryInfo;
use crate::commands::transcode::metadata::AlbumMetadata;
use crate::commands::transcode::packets::file::{FilePacketAction, FileWorkPacket};
use crate::configuration::{Config, ConfigLibrary};

/// Represents a grouping of file packets into a single album.
/// Using this struct we can generate a list of file work packets in the album.
#[derive(Clone)]
pub struct AlbumWorkPacket<'a> {
    /// Album information (artist name, album title, source library).
    pub album_info: AlbumDirectoryInfo<'a>,

    /// Contains a cached version of the metadata available on disk (if any).
    /// Generated on first access.
    cached_saved_meta: CachedValue<Option<AlbumMetadata>>,

    /// Contains a cached version of the fresh file metadata.
    /// Generated on first access.
    cached_fresh_meta: CachedValue<AlbumMetadata>,
}

impl<'a> AlbumWorkPacket<'a> {
    pub fn from_album_path<P: AsRef<Path>>(
        album_directory_path: P,
        config: &'a Config,
        library: &'a ConfigLibrary,
    ) -> Result<AlbumWorkPacket<'a>> {
        let directory_info = AlbumDirectoryInfo::new(
            album_directory_path.as_ref(),
            config,
            library,
        )?;
        
        Ok(AlbumWorkPacket::from_album_info(directory_info))
    }

    pub fn from_album_info(
        album_directory_info: AlbumDirectoryInfo<'a>,
    ) -> AlbumWorkPacket<'a> {
        AlbumWorkPacket {
            album_info: album_directory_info,
            cached_saved_meta: CachedValue::new(),
            cached_fresh_meta: CachedValue::new(),
        }
    }

    fn get_album_directory_path(&self) -> PathBuf {
        let mut path = PathBuf::from(&self.album_info.library.path);
        
        path.push(&self.album_info.artist_name);
        path.push(&self.album_info.album_title);

        path
    }

    pub fn get_saved_meta(&mut self) -> Result<Option<AlbumMetadata>> {
        if self.cached_saved_meta.is_cached() {
            return match self.cached_saved_meta.get() {
                Some(meta) => Ok(Some(meta.clone())),
                None => Ok(None),
            }
        }

        let full_album_directory_path = self.get_album_directory_path();

        let saved_meta = AlbumMetadata::load(&full_album_directory_path)?;
        self.cached_saved_meta.set(saved_meta.clone());

        Ok(saved_meta)
    }

    pub fn get_fresh_meta(&mut self) -> Result<AlbumMetadata> {
        if self.cached_fresh_meta.is_cached() {
            return Ok(self.cached_fresh_meta.get().clone());
        }

        let full_album_directory_path = self.get_album_directory_path();

        let fresh_meta = AlbumMetadata::generate(
            &full_album_directory_path,
            &self.album_info.library.transcoding.all_tracked_extensions
        )?;
        self.cached_fresh_meta.set(fresh_meta.clone());

        Ok(fresh_meta)
    }

    pub fn needs_processing(&mut self, config: &Config) -> Result<bool> {
        let saved_meta = self.get_saved_meta()?;
        
        if let Some(saved_meta) = saved_meta {
            let fresh_meta = self.get_fresh_meta()?;
    
            let meta_diff = saved_meta.diff_with_fresh_or_missing_from_target_dir(
                &fresh_meta,
                &self.album_info,
                config,
            )?;
    
            Ok(meta_diff.has_any_changes())
        } else {
            Ok(true)
        }
    }

    pub fn get_work_packets(&mut self, config: &Config) -> Result<Vec<FileWorkPacket>> {
        let needs_processing = self.needs_processing(config)?;
        if !needs_processing {
            return Ok(Vec::new());
        }

        // Generate a fresh look at the files and generate a list of file packets from that.
        let saved_meta = self.get_saved_meta()?;
        let fresh_meta = self.get_fresh_meta()?;

        let mut file_packets: Vec<FileWorkPacket> = Vec::new();
        
        if saved_meta.is_some() {
            let diff = saved_meta.unwrap().diff_with_fresh_or_missing_from_target_dir(
                &fresh_meta,
                &self.album_info,
                config,
            )?;

            for new_file_name in diff.files_new
                .iter()
                .chain(diff.files_untranscoded.iter())
            {
                file_packets.push(
                    FileWorkPacket::new(
                        Path::new(&new_file_name),
                        &self.album_info,
                        config,
                        FilePacketAction::Process,
                    )?,
                );
            }

            for changed_file_name in diff.files_changed {
                file_packets.push(
                    FileWorkPacket::new(
                        Path::new(&changed_file_name),
                        &self.album_info,
                        config,
                        FilePacketAction::Process,
                    )?,
                );
            }

            for removed_file_name in diff.files_removed {
                file_packets.push(
                    FileWorkPacket::new(
                        Path::new(&removed_file_name),
                        &self.album_info,
                        config,
                        FilePacketAction::RemoveAtTarget,
                    )?,
                );
            }

        } else {
            for (fresh_file, _) in fresh_meta.files {
                let file_packet = FileWorkPacket::new(
                    Path::new(&fresh_file),
                    &self.album_info,
                    config,
                    FilePacketAction::Process,
                )?;

                file_packets.push(file_packet);
            }
        }

        // Sort file packets by name.
        file_packets.sort_unstable_by(
            |first, second| {
                let first_name = first.source_file_path
                    .file_name()
                    .expect("Could not convert file path to string while sorting.")
                    .to_str()
                    .expect("Could not convert file path to string while sorting.");

                let second_name = second.source_file_path
                    .file_name()
                    .expect("Could not convert file path to string while sorting.")
                    .to_str()
                    .expect("Could not convert file path to string while sorting.");

                first_name.cmp(second_name)
            }
        );

        Ok(file_packets)
    }

    pub fn save_fresh_meta(&mut self, allow_overwrite: bool) -> Result<()> {
        let fresh_meta = self.get_fresh_meta()?;
        fresh_meta.save(&self.get_album_directory_path(), allow_overwrite)?;

        Ok(())
    }
}
