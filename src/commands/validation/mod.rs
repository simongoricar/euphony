use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crossterm::style::Stylize;
use euphony_configuration::library::LibraryConfiguration;
use euphony_configuration::{Configuration, ALBUM_OVERRIDE_FILE_NAME};
use euphony_library::state::source::SOURCE_ALBUM_STATE_FILE_NAME;
use euphony_library::view::LibraryView;
use miette::{miette, Context, Result};

use crate::commands::transcode::library_state::LIBRARY_STATE_FILE_NAME;
use crate::console::frontends::ValidationTerminal;
use crate::console::{LogBackend, ValidationBackend, ValidationErrorInfo};

/// Implemented by concrete validation errors to allow a standardised way of displaying the error.
pub trait ValidationErrorDisplay {
    /// This method should format and return the complete string that
    /// describes the implementor's error.
    fn get_error_info(&self) -> Result<ValidationErrorInfo>;
}

/// Describes all possible validation errors.
pub enum ValidationError<'a> {
    UnexpectedFile(UnexpectedFile<'a>),
    AlbumCollision(AlbumCollision<'a>),
}

impl<'a> ValidationError<'a> {
    /// Initialize a new validation error: an unexpected file.
    pub fn new_unexpected_file<P: Into<PathBuf>>(
        file_path: P,
        library: &'a LibraryConfiguration,
        reason: UnexpectedFileLocation,
    ) -> Self {
        Self::UnexpectedFile(UnexpectedFile::new(file_path, library, reason))
    }

    /// Initialize a new validation error: an album collision.
    #[allow(dead_code)]
    pub fn new_album_collision(
        colliding_albums: Vec<&'a ValidationAlbumEntry<'a>>,
    ) -> Result<Self> {
        Ok(Self::AlbumCollision(AlbumCollision::new(
            colliding_albums,
        )?))
    }

    /// Consume the enum instance and return the `ValidationErrorInfo` that its variant returns.
    pub fn into_validation_error_info(self) -> Result<ValidationErrorInfo> {
        match self {
            ValidationError::UnexpectedFile(unexpected_file) => {
                unexpected_file.get_error_info()
            }
            ValidationError::AlbumCollision(album_collision) => {
                album_collision.get_error_info()
            }
        }
    }
}

/// Describes the type of the "unexpected file type" validation error.
pub enum UnexpectedFileLocation {
    LibraryRoot,
    ArtistDirectory,
    AlbumDirectoryAudio,
    AlbumDirectoryOther,
}

/// This validation error happens when the contents of a library do not match what is configured
/// in the library configuration table in `configuration.toml`.
pub struct UnexpectedFile<'a> {
    /// Unexpected file path.
    file_path: PathBuf,

    /// What library the unexpected file is part of.
    library: &'a LibraryConfiguration,

    /// Specific reason for why this is unexpected.
    location: UnexpectedFileLocation,
}

impl<'a> UnexpectedFile<'a> {
    pub fn new<P: Into<PathBuf>>(
        file_path: P,
        library: &'a LibraryConfiguration,
        reason: UnexpectedFileLocation,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            library,
            location: reason,
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

        // TODO Render a shortened file tree as in the example above.

        let relative_file_path =
            pathdiff::diff_paths(&self.file_path, &self.library.path)
                .ok_or_else(|| {
                    miette!("Could not make file path relative to library base!")
                })?;

        let attributes = vec![
            ("Library".to_string(), self.library.name.clone()),
            (
                "File".to_string(),
                relative_file_path.to_string_lossy().to_string(),
            ),
        ];

        Ok(ValidationErrorInfo::new(
            match self.location {
                UnexpectedFileLocation::LibraryRoot => {
                    "Unexpected file in library root."
                }
                UnexpectedFileLocation::ArtistDirectory => {
                    "Unexpected file in artist directory."
                }
                UnexpectedFileLocation::AlbumDirectoryAudio => {
                    "Unexpected audio file in album directory."
                }
                UnexpectedFileLocation::AlbumDirectoryOther => {
                    "Unexpected data file in album directory."
                }
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

    pub library: &'a LibraryConfiguration,
}

impl<'a> ValidationAlbumEntry<'a> {
    /// Create a new `ValidationAlbumEntry` by providing the album's title, artist name
    /// and the library it is in.
    pub fn new<S: Into<String>>(
        artist_name: S,
        album_title: S,
        library: &'a LibraryConfiguration,
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
/// (each from a different library) - were the user to try and `transcode`, this would cause issues
/// with overwriting existing files in the transcoded collection.
pub struct AlbumCollision<'a> {
    colliding_albums: Vec<&'a ValidationAlbumEntry<'a>>,
}

impl<'a> AlbumCollision<'a> {
    /// Initialize a new `AlbumCollision` by providing a set of colliding album entries.
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

        Ok(Self { colliding_albums })
    }

    /// Get the artist name of the colliding entry.
    pub fn artist_name(&self) -> String {
        // Because we did a sanity check that there are at least two entries and that they
        // actually collide (are the same), we can just take the first entry and return its details.
        self.colliding_albums[0].artist_name.clone()
    }

    /// Get the album title of the colliding entry.
    pub fn album_title(&self) -> String {
        // Because we did a sanity check that there are at least two entries and that they
        // actually collide (are the same), we can just take the first entry and return its details.
        self.colliding_albums[0].album_title.clone()
    }

    /// Returns the list of colliding libraries.
    /// The returned `Vec` is guaranteed to have at least two elements.
    pub fn colliding_library_names(&self) -> Vec<String> {
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

        let colliding_libraries = self.colliding_library_names().join(", ");

        let attributes = vec![
            (
                "Colliding libraries".to_string(),
                colliding_libraries,
            ),
            ("Artist".to_string(), self.artist_name()),
            ("Album".to_string(), self.album_title()),
        ];

        Ok(ValidationErrorInfo::new(
            "Inter-library Album Collision",
            attributes,
        ))
    }
}


/// A high-level validator for inter-library album collisions.
///
/// The process is as follows:
/// - instantiate an empty `CollectionCollisionValidator`,
/// - call `add_album_entry` with all your albums from all your libraries,
/// - when finished, call `find_collisions` to receive information about potential collisions.
struct CollectionCollisionValidator<'a> {
    /// A nested map from artist names to album names to sets of individual (colliding) albums.
    artist_to_albums:
        HashMap<String, HashMap<String, HashSet<ValidationAlbumEntry<'a>>>>,
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
        library: &'a LibraryConfiguration,
    ) -> Result<()> {
        let artist_name = artist_name.into();
        let album_title = album_title.into();

        let entry = ValidationAlbumEntry::new(artist_name, album_title, library);

        let artist_albums = self
            .artist_to_albums
            .entry(entry.artist_name.clone())
            .or_default();

        let album_set =
            artist_albums.entry(entry.album_title.clone()).or_default();

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
    pub fn find_collisions(&'a self) -> Result<Vec<AlbumCollision<'a>>> {
        self.artist_to_albums
            .values()
            .flatten()
            .filter_map(|(_, album_set)| {
                if album_set.len() > 1 {
                    // This album has a collision, generate it.
                    Some(AlbumCollision::new(
                        album_set
                            .iter()
                            .collect::<Vec<&'a ValidationAlbumEntry<'a>>>(),
                    ))
                } else {
                    // No collision in this album.
                    None
                }
            })
            .collect::<Result<Vec<AlbumCollision<'a>>>>()
    }
}

/// Runs the validation process over the entire collection (all registered libraries).
fn validate_entire_collection(
    config: &Configuration,
    terminal: &mut ValidationTerminal,
) -> Result<()> {
    // As explained in the README and configuration template, library structure
    // is expected to be the following:
    //
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

    // For each library, check the following:
    //  1. Unexpected files in the root library directory,
    //  2. Unexpected files in any artist directory,
    //  3. Unexpected files in any album directory.

    // As we're validating albums we're also performing an artist-album collision check
    // between all registered libraries.

    let is_any_audio_file = |file_path: &Path| {
        let file_extension = file_path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();

        config
            .validation
            .extensions_considered_audio_files
            .contains(&file_extension)
    };

    for library_config in config.libraries.values() {
        let library_view =
            LibraryView::from_library_configuration(config, library_config)?;
        let library_view_locked = library_view.read();

        let ignored_directories_in_base_directory: HashSet<&String> =
            match &library_config.ignored_directories_in_base_directory {
                Some(ignored_dirs) => HashSet::from_iter(ignored_dirs.iter()),
                None => HashSet::new(),
            };

        let allowed_audio_file_extensions =
            &library_config.validation.allowed_audio_file_extensions;
        let allowed_other_file_extensions =
            &library_config.validation.allowed_other_file_extensions;
        let allowed_other_files_by_name =
            &library_config.validation.allowed_other_files_by_name;

        // Handy closures for repeated file validity checks.
        let is_valid_library_audio_file = |file_path: &Path| {
            let file_extension = file_path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();

            allowed_audio_file_extensions.contains(&file_extension)
        };

        let is_valid_library_non_audio_file = |file_path: &Path| {
            let file_name = file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let file_extension = file_path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();

            allowed_other_file_extensions.contains(&file_extension)
                || allowed_other_files_by_name.contains(&file_name)
        };

        // Check for unexpected files in the root library directory.
        let root_library_files_to_check =
            library_view_locked.library_root_validation_files()?;
        for root_file in root_library_files_to_check {
            let root_file_name = root_file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Allow `.library.state.euphony`.
            if root_file_name.eq(LIBRARY_STATE_FILE_NAME) {
                continue;
            }

            if !is_valid_library_non_audio_file(root_file.as_path()) {
                validation_errors.push(ValidationError::new_unexpected_file(
                    root_file,
                    library_config,
                    UnexpectedFileLocation::LibraryRoot,
                ))
            }
        }

        // Check for unexpected files in each artist directory.
        for (artist_name, artist_view) in library_view_locked.artists()? {
            if ignored_directories_in_base_directory.contains(&artist_name) {
                continue;
            }

            let artist_view_locked = artist_view.read();

            let artist_files =
                artist_view_locked.artist_directory_validation_files()?;
            for artist_dir_file_path in artist_files {
                if !is_valid_library_non_audio_file(
                    artist_dir_file_path.as_path(),
                ) {
                    validation_errors.push(ValidationError::new_unexpected_file(
                        artist_dir_file_path,
                        library_config,
                        UnexpectedFileLocation::ArtistDirectory,
                    ))
                }
            }

            // Iterate over each of their albums and validate those as well.
            for (album_title, album_view) in artist_view_locked.albums()? {
                collision_validator
                    .add_album_entry(&artist_name, &album_title, library_config)
                    .wrap_err_with(|| miette!("BUG: Duplicate album entry."))?;

                let album_view_locked = album_view.read();

                let album_files = album_view_locked.album_validation_files()?;
                for album_dir_file_path in album_files {
                    let album_dir_file_name = album_dir_file_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    if album_dir_file_name.eq(SOURCE_ALBUM_STATE_FILE_NAME)
                        || album_dir_file_name.eq(ALBUM_OVERRIDE_FILE_NAME)
                    {
                        continue;
                    }

                    let is_any_audio =
                        is_any_audio_file(album_dir_file_path.as_path());
                    let is_valid_audio = is_valid_library_audio_file(
                        album_dir_file_path.as_path(),
                    );
                    let is_valid_non_audio = is_valid_library_non_audio_file(
                        album_dir_file_path.as_path(),
                    );

                    if is_any_audio && !is_valid_audio {
                        // File was an audio file, but not the kind that we allow in this library.
                        validation_errors.push(
                            ValidationError::new_unexpected_file(
                                &album_dir_file_path,
                                library_config,
                                UnexpectedFileLocation::AlbumDirectoryAudio,
                            ),
                        );
                    } else if !is_any_audio && !is_valid_non_audio {
                        // File was not an audio file nor a valid non-audio (data) file in this library.
                        validation_errors.push(
                            ValidationError::new_unexpected_file(
                                &album_dir_file_path,
                                library_config,
                                UnexpectedFileLocation::AlbumDirectoryOther,
                            ),
                        );
                    }
                }
            }
        }
    }

    // Get the artist-album collision results.
    validation_errors.extend(
        collision_validator
            .find_collisions()?
            .into_iter()
            .map(ValidationError::AlbumCollision),
    );


    // We've completed the validation process, we'll now display the results.
    let validation_errors_vec: Vec<ValidationErrorInfo> = validation_errors
        .into_iter()
        .map(|error| error.into_validation_error_info())
        .collect::<Result<Vec<ValidationErrorInfo>>>()?;

    if validation_errors_vec.is_empty() {
        terminal.log_println("All libraries validated, no errors.".green());
    } else {
        terminal.log_println(
            format!(
                "{} validation errors!",
                validation_errors_vec.len()
            )
            .red(),
        );

        for error in validation_errors_vec {
            terminal.validation_add_error(error);
        }
    }

    Ok(())
}

/// Associated with the `validate` command.
///
/// Validates the entire collection for unexpected files and album collisions.
pub fn cmd_validate(
    config: &Configuration,
    terminal: &mut ValidationTerminal,
) -> Result<()> {
    terminal.log_println("Command: validate entire collection.".cyan().bold());

    validate_entire_collection(config, terminal)?;
    Ok(())
}
