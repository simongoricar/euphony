use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crossterm::style::Stylize;
use miette::{Context, miette, Result};

use crate::console::{FullValidationBackend, ValidationErrorInfo};
use crate::console::utilities::term_println_fvb;
use crate::configuration::{Config, ConfigLibrary};
use crate::filesystem as efs;

// TODO Rewrite new validation

pub trait ValidationErrorDisplay {
    /// This method should format and return the complete string that
    /// describes the implementor's error.
    fn get_error_info(&self) -> Result<ValidationErrorInfo>;
}

pub enum ValidationError<'a> {
    UnexpectedFile(UnexpectedFile<'a>),
    AlbumCollision(AlbumCollision<'a>),
}

#[allow(dead_code)]
impl<'a> ValidationError<'a> {
    pub fn new_unexpected_file<P: Into<PathBuf>>(
        file_path: P,
        library: &'a ConfigLibrary,
        reason: UnexpectedFileType,
    ) -> Self {
        Self::UnexpectedFile(UnexpectedFile::new(
            file_path,
            library,
            reason,
        ))
    }
    
    pub fn new_album_collision(
        colliding_albums: Vec<&'a ValidationAlbumEntry<'a>>,
    ) -> Result<Self> {
        Ok(Self::AlbumCollision(AlbumCollision::new(
            colliding_albums,
        )?))
    }
    
    pub fn into_validation_error_info(self) -> Result<ValidationErrorInfo> {
        match self {
            ValidationError::UnexpectedFile(unexpected_file) => unexpected_file.get_error_info(),
            ValidationError::AlbumCollision(album_collision) => album_collision.get_error_info(),
        }
    }
}

pub enum UnexpectedFileType {
    LibraryRootFile,
    ArtistDirectoryFile,
    AlbumDirectoryAudioFile,
    AlbumDirectoryOtherFile,
}

pub struct UnexpectedFile<'a> {
    file_path: PathBuf,
    library: &'a ConfigLibrary,
    reason: UnexpectedFileType,
}

impl<'a> UnexpectedFile<'a> {
    pub fn new<P: Into<PathBuf>>(
        file_path: P,
        library: &'a ConfigLibrary,
        reason: UnexpectedFileType,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            library,
            reason,
        }
    }
}

impl<'a> ValidationErrorDisplay for UnexpectedFile<'a> {
    fn get_error_info(&self) -> Result<ValidationErrorInfo> {
        // (UnexpectedFile validation error display example)
        //
        // # Unexpected (Audio)? File [in library root/in artist directory/in album directory]
        //
        // Library: Standard
        // File: Aindulmedir/some_unexpected_file.zip
        //
        // C:/StandardLibrary
        // |-- Aindulmedir (album directory)
        // |   |-> some_unexpected_file.zip
        
        // TODO File tree as in the example above.
        
        let relative_file_path = pathdiff::diff_paths(
            &self.file_path,
            &self.library.path,
        )
            .ok_or_else(|| miette!("Could not make file path relative to library base!"))?;
        
        let attributes = vec![
            ("Library".to_string(), self.library.name.clone()),
            ("File".to_string(), relative_file_path.to_string_lossy().to_string()),
        ];
        
        Ok(ValidationErrorInfo::new(
            match self.reason {
                UnexpectedFileType::LibraryRootFile => "Unexpected file in library root.",
                UnexpectedFileType::ArtistDirectoryFile => "Unexpected file in artist directory.",
                UnexpectedFileType::AlbumDirectoryAudioFile => "Unexpected audio file in album directory.",
                UnexpectedFileType::AlbumDirectoryOtherFile => "Unexpected data file in album directory.",
            },
            attributes,
        ))
    }
}


/// Represents an album belonging to a specific artist in a specific library.
/// Used by `LibraryValidator` to keep track of all available albums.
pub struct ValidationAlbumEntry<'a> {
    pub artist_name: String,
    pub album_title: String,
    pub library: &'a ConfigLibrary,
}

impl<'a> ValidationAlbumEntry<'a> {
    /// Create a new `ValidationAlbumEntry` by providing the album's title, artist name
    /// and the library it is in.
    pub fn new<S: Into<String>>(
        artist_name: S,
        album_title: S,
        library: &'a ConfigLibrary,
    ) -> Self {
        Self {
            artist_name: artist_name.into(),
            album_title: album_title.into(),
            library,
        }
    }
}

impl<'a> PartialEq for ValidationAlbumEntry<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.artist_name.eq(&other.artist_name)
            && self.album_title.eq(&other.album_title)
    }
}

impl<'a> Eq for ValidationAlbumEntry<'a> {}

impl<'a> Hash for ValidationAlbumEntry<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.artist_name.hash(state);
        self.album_title.hash(state);
        self.library.name.hash(state);
    }
}


/// Represents a single album collision containing two or more colliding album entries
/// (each from a different library).
pub struct AlbumCollision<'a> {
    colliding_albums: Vec<&'a ValidationAlbumEntry<'a>>,
}

impl<'a> AlbumCollision<'a> {
    /// Create a new `AlbumCollision` by providing a set of colliding album entries.
    pub fn new(
        colliding_albums: Vec<&'a ValidationAlbumEntry<'a>>,
    ) -> Result<Self> {
        // Ensure the entries are actually collisions, returning Err on mismatch.
        let first_artist_name = &colliding_albums[0].artist_name;
        let first_album_name = &colliding_albums[0].album_title;
        
        for entry in colliding_albums.iter().skip(1) {
            entry.artist_name.eq(first_artist_name)
                .then_some(())
                .ok_or_else(|| miette!("Entry's artist name in colliding_albums did not match the first one."))?;
            
            entry.album_title.eq(first_album_name)
                .then_some(())
                .ok_or_else(|| miette!("Entry's album title in colliding_albums did not match the first one."))?;
        }
        
        Ok(Self {
            colliding_albums,
        })
    }
    
    /// Get the artist name of the colliding entry.
    pub fn get_artist_name(&self) -> String {
        // Because we did a sanity check that there are at least two entries and that they
        // actually collide (are the same), we can just take the first entry and return its details.
        self.colliding_albums[0].artist_name.clone()
    }
    
    /// Get the album title of the colliding entry.
    pub fn get_album_title(&self) -> String {
        // Because we did a sanity check that there are at least two entries and that they
        // actually collide (are the same), we can just take the first entry and return its details.
        self.colliding_albums[0].album_title.clone()
    }
    
    /// Returns the list of colliding libraries.
    /// The returned `Vec` is guaranteed to have at least two elements.
    pub fn get_colliding_library_names(&self) -> Vec<String> {
        self.colliding_albums
            .iter()
            .map(|entry| entry.library.name.clone())
            .collect()
    }
}

impl<'a> ValidationErrorDisplay for AlbumCollision<'a> {
    fn get_error_info(&self) -> Result<ValidationErrorInfo> {
        // (AlbumCollision validation error display example)
        //
        // # Inter-library Album Collision
        //
        // Colliding libraries: Standard + Lossless
        // Artist: Aindulmedir
        // Album: The Lunar Lexicon
        
        let colliding_libraries = self.get_colliding_library_names()
            .join(" + ");
        
        let attributes = vec![
            ("Colliding libraries".to_string(), colliding_libraries),
            ("Artist".to_string(), self.get_artist_name()),
            ("Album".to_string(), self.get_album_title()),
        ];
        
        Ok(ValidationErrorInfo::new(
            "Inter-library Album Collision",
            attributes,
        ))
    }
}


struct CollectionCollisionValidator<'a> {
    /// A nested map from artist names to album names to sets of individual (colliding) albums.
    artist_to_albums: HashMap<String, HashMap<String, HashSet<ValidationAlbumEntry<'a>>>>,
}

impl<'a> CollectionCollisionValidator<'a> {
    /// Create a new empty `LibraryValidator`.
    pub fn new() -> Self {
        Self {
            artist_to_albums: HashMap::new(),
        }
    }
    
    /// Add a new album entry into the validator by providing the album's title, artist name
    /// and the library is it in. This method returns `Err` only in the case of this exact combination
    /// (including library) already existing in the validator (which is a bug, not a collision).
    pub fn add_album_entry<S: Into<String>>(
        &mut self,
        artist_name: S,
        album_title: S,
        library: &'a ConfigLibrary,
    ) -> Result<()> {
        let artist_name = artist_name.into();
        let album_title = album_title.into();
        
        let entry = ValidationAlbumEntry::new(artist_name, album_title, library);
        
        let artist_albums = self.artist_to_albums
            .entry(entry.artist_name.clone())
            .or_default();
        
        let album_set = artist_albums
            .entry(entry.album_title.clone())
            .or_default();
        
        let exact_entry_already_existed = !album_set.insert(entry);
        
        // It is possible (but would be a bug) that the exact same entry from the same library
        // would be inserted multiple times. In that case we return early.
        if exact_entry_already_existed {
            return Err(miette!("Exact entry already exists in set."));
        }
        
        // We don't generate collisions here, but instead at request (see other methods).
        Ok(())
    }
    
    /// Get a list of album collisions in this validator. A single collision represents two or more
    /// of the same album colliding in multiple different libraries.
    pub fn get_collisions(&'a self) -> Result<Vec<AlbumCollision<'a>>> {
        self.artist_to_albums
            .values()
            .flatten()
            .filter_map(|(_, album_set)| {
                if album_set.len() > 1 {
                    // This album has a collision, generate it.
                    Some(
                        AlbumCollision::new(
                            album_set.iter()
                                .collect::<Vec<&'a ValidationAlbumEntry<'a>>>()
                        )
                    )
                } else {
                    // No collision in this album.
                    None
                }
            })
            .collect::<Result<Vec<AlbumCollision<'a>>>>()
    }
}

fn validate_entire_collection(
    config: &Config,
    terminal: &mut dyn FullValidationBackend,
) -> Result<()> {
    // As explained in the README and configuration template, library structure is expected to be the following:
    // <base library directory>
    // |
    // |-- <artist directory>
    // |   |
    // |   |  [possibly some album-related README, logs, whatever else, etc.]
    // |   |  (settings for other files (see below) apply here as well)
    // |   |
    // |   |-- <album directory>
    // |   |   |
    // |   |   | ... [audio files]
    // |   |   |     (whichever types you allow inside each library's configuration, see `allowed_audio_files_by_extension`)
    // |   |   |
    // |   |   | ... [cover art]
    // |   |   | ... [some album-related README, logs, whatever else, etc.]
    // |   |   |     (settings for other files (see below) apply here as well)
    // |   |   |
    // |   |   | ... <possibly other directories that don't really matter for transcoding>
    // |   |   |     (album subdirectories are ignored by default, see `depth` in per-album configuration)
    // |
    // |-- <any directory (directly in the library directory) that has been ignored>
    // |   (it is sometimes useful to have additional directories inside your library that are
    // |    not artist directories, but instead contain some miscellaneous files (e.g. temporary files) you don't want to
    // |    transcode - these directories can be ignored for each individual library using `ignored_directories_in_base_dir`)
    // |
    // | ... [other files]
    // |     (of whatever type or name you allow in the configuration, see
    // |      `allowed_other_files_by_extension` and `allowed_other_files_by_name` - these settings
    // |      apply also to artist and album directories below)
    //
    // # Example:
    // LosslessLibrary
    // |
    // | LOSSLESS_README.txt
    // |
    // |-- Aindulmedir
    // |   |-- The Lunar Lexicon
    // |   |   | 01 Aindulmedir - Wind-Bitten.flac
    // |   |   | 02 Aindulmedir - Book of Towers.flac
    // |   |   | 03 Aindulmedir - The Librarian.flac
    // |   |   | 04 Aindulmedir - Winter and Slumber.flac
    // |   |   | 05 Aindulmedir - The Lunar Lexicon.flac
    // |   |   | 06 Aindulmedir - Snow Above Blue Fire.flac
    // |   |   | 07 Aindulmedir - Sleep-Form.flac
    // |   |   | cover.jpg
    // |
    // |-- _other
    // |   | some_other_metadata_or_something.db
    //
    // In the example above, there exists a lossless library by the name of LosslessLibrary.
    // For this to validate correctly, this library would require the following configuration:
    // - its `allowed_audio_files_by_extension` should be set to `["flac"]`,
    // - its `ignored_directories_in_base_dir` should be set to `["_other"]`,
    // - the global setting `allowed_other_files_by_extension` should also include `txt` and `jpg` (which it does by default).
    //
    // Library-specific configuration:
    // ```toml
    //   [libraries.lossless_private]
    //   name = "Lossless Private"
    //   path = "{LIBRARY_BASE}/MusicLibraryLosslessPrivate"
    //   allowed_audio_files_by_extension = ["flac"]
    //   ignored_directories_in_base_dir = []
    // ```
    //
    // NOTE: Specifying the files to transcode or copy is not directly linked to validation! See
    // `tracked_audio_extensions` and `tracked_other_extensions`, which dictate which
    // extensions are transcoded and which are copied when running the `transcode` command.
    
    let mut validation_errors: Vec<ValidationError> = Vec::new();
    let mut collision_validator = CollectionCollisionValidator::new();
    
    // Combined steps 1 - 4; for each library:
    //  1. Check for unexpected files in the root directory of each library.
    //  2. Check for unexpected files in each artist directory.
    //  3. Check for unexpected files in each album directory.
    //  4. Check for any album collisions between libraries.
    
    // This is a general chck that uses `validation.extensions_considered_audio_files` to check
    // whether the file in question is considered an audio file
    // (but this says nothing about the validity of the extension for a given library).
    let is_audio_file = |file_path: &PathBuf| {
        let file_extension = file_path.extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        
        config.validation.extensions_considered_audio_files.contains(&file_extension)
    };
    
    for library in config.libraries.values() {
        let library_root_path = Path::new(&library.path);
        
        let ignored_directories_in_base_directory = &library.ignored_directories_in_base_directory;
        let allowed_audio_file_extensions = &library.validation.allowed_audio_file_extensions;
        let allowed_other_file_extensions = &library.validation.allowed_other_file_extensions;
        let allowed_other_files_by_name = &library.validation.allowed_other_files_by_name;
        
        // Handy closures for repeated file validity checks.
        let is_valid_audio_file = |file_path: &PathBuf| {
            let file_extension = file_path.extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            
            allowed_audio_file_extensions.contains(&file_extension)
        };
        
        let is_valid_other_file = |file_path: &PathBuf| {
            let file_name = file_path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
    
            let file_extension = file_path.extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            
            allowed_other_file_extensions.contains(&file_extension)
                || allowed_other_files_by_name.contains(&file_name)
        };
        
        ////
        // Check for unexpected files in the root directory of each library.
        ////
        let (root_files, root_subdirectories) = efs::list_directory_contents(library_root_path)
            .wrap_err_with(|| miette!("Could not list files and directories in root library directory for {}", library.name))?;
        
        // All files in the root directory must match `allowed_other_file_extensions` or `allowed_other_files_by_name`.
        for root_file in root_files {
            let file_path = root_file.path();
            if !is_valid_other_file(&file_path) {
                // File did not match neither by extension nor by full name - it is invalid.
                validation_errors.push(
                    ValidationError::new_unexpected_file(
                        file_path,
                        library,
                        UnexpectedFileType::LibraryRootFile,
                    )
                )
            }
        }
        
        ////
        // Check for unexpected files in each artist directory.
        ////
        for artist_directory in root_subdirectories {
            let artist_directory_name = artist_directory.file_name()
                .to_string_lossy()
                .to_string();
            
            // If the directory has been explicitly excluded with `ignored_directories_in_base_directory`, we skip it.
            if let Some(ignored_directories) = ignored_directories_in_base_directory {
                if ignored_directories.contains(&artist_directory_name) {
                    continue;
                }
            }
    
            let (artist_dir_files, artist_dir_subdirectories) = efs::list_directory_contents(artist_directory.path())
                .wrap_err_with(|| miette!("Could not list files and directories in artist directory for {:?}", artist_directory.path()))?;
            
            // Check files directly in the artist directory.
            for artist_dir_file in artist_dir_files {
                let file_path = artist_dir_file.path();
                if !is_valid_other_file(&file_path) {
                    // File did not match neither by extension nor by full name - it is invalid.
                    validation_errors.push(
                        ValidationError::new_unexpected_file(
                            file_path,
                            library,
                            UnexpectedFileType::ArtistDirectoryFile,
                        )
                    )
                }
            }
            
            for album_directory in artist_dir_subdirectories {
                let album_directory_name = album_directory.file_name()
                    .to_string_lossy()
                    .to_string();
                
                // Add this artist-album combination into the collision validator. We'll check for colliding albums at the end.
                collision_validator.add_album_entry(
                    &artist_directory_name,
                    &album_directory_name,
                    library,
                )
                    .wrap_err_with(|| miette!("Bug: this exact library-album combination already exists!"))?;
                
                // Iterate over files in each album, checking them for validity.
                let (album_dir_files, _album_dir_subdirectories) = efs::list_directory_contents(album_directory.path())
                    .wrap_err_with(|| miette!("Could not list files and directories in album directory for {:?}", album_directory.path()))?;
                
                for album_dir_file in album_dir_files {
                    let file_path = album_dir_file.path();
                    if is_audio_file(&file_path) && !is_valid_audio_file(&file_path) {
                        // File was an audio file, but not the kind that is allowed in this library.
                        validation_errors.push(
                            ValidationError::new_unexpected_file(
                                file_path,
                                library,
                                UnexpectedFileType::AlbumDirectoryAudioFile,
                            )
                        )
                    } else if !is_valid_other_file(&file_path) {
                        // File was not an audio file, and is not a data file the user allows in this library.
                        validation_errors.push(
                            ValidationError::new_unexpected_file(
                                file_path,
                                library,
                                UnexpectedFileType::AlbumDirectoryOtherFile,
                            )
                        )
                    }
                }
                
                // TODO Implement depth config that is available per-album.
                //      At this moment it only matters in transcoding, and it is ignored during validation. This can (and will) make validation miss nested files.
                
                // TODO Refactor this code to avoid such deep nesting.
            }
        }
    }
    
    // Get the collision results from the collision validator.
    validation_errors.extend(
        collision_validator.get_collisions()?
            .into_iter()
            .map(|collision| ValidationError::AlbumCollision(collision))
    );
    
    // Validation process complete, display the results.
    let validation_error_info_array: Vec<ValidationErrorInfo> = validation_errors
        .into_iter()
        .map(|error| error.into_validation_error_info())
        .collect::<Result<Vec<ValidationErrorInfo>>>()?;
    
    if validation_error_info_array.is_empty() {
        term_println_fvb(
            terminal,
            "Entire collection validated, no validation errors.".green()
        );
    } else {
        term_println_fvb(
            terminal,
            format!(
                "Entire collection validated, found {} validation errors!",
                validation_error_info_array.len().to_string().bold()
            ).red()
        );
    
        for error in validation_error_info_array {
            terminal.validation_add_error(error);
        }
    }
    
    Ok(())
}

pub fn cmd_validate_all(
    config: &Config,
    terminal: &mut dyn FullValidationBackend,
) -> Result<()> {
    term_println_fvb(terminal, "Mode: validate all libraries.".cyan().bold());
    
    validate_entire_collection(config, terminal)?;
    Ok(())
}
