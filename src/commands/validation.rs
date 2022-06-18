use std::collections::{HashMap, HashSet, LinkedList};
use std::hash::{Hash, Hasher};
use std::io::Error;
use std::path::Path;

use owo_colors::{OwoColorize, Style};

use super::super::configuration::{Config, ConfigLibrary};
use super::super::filesystem as mfs;
use super::super::console;


// Collision checker code
#[derive(Eq)]
struct AlbumEntry {
    // (note that this struct is hashed/EQed only based on the name attribute, nothing else)
    name: String,
    source_library_name: String,
}

impl AlbumEntry {
    fn new_without_source(name: String) -> AlbumEntry {
        AlbumEntry {
            name,
            source_library_name: String::new(),
        }
    }

    fn new(name: String, source_library_name: String) -> AlbumEntry {
        AlbumEntry {
            name,
            source_library_name,
        }
    }
}

impl PartialEq<Self> for AlbumEntry {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
    }
}

impl Hash for AlbumEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state)
    }
}


struct Collision {
    artist: String,
    album: String,
    // Library names.
    already_exists_in: String,
    collision_with: String,
}


struct CollisionChecker {
    /// Keys are artists, values are a set of albums we know about.
    albums_per_artist: HashMap<String, HashSet<AlbumEntry>>,
    collisions: LinkedList<Collision>,
}

impl CollisionChecker {
    fn new() -> CollisionChecker {
        CollisionChecker {
            albums_per_artist: HashMap::new(),
            collisions: LinkedList::new(),
        }
    }

    fn would_collide(&self, artist: &str, album: &str) -> bool {
        if !self.albums_per_artist.contains_key(artist) {
            // If we don't even know the artist yet, there can't be a collision.
            false
        } else {
            // Let's check the album set for a collision as we have seen this artist before.
            self.albums_per_artist[artist]
                .contains(&AlbumEntry::new_without_source(album.to_string().clone()))
        }
    }

    /// Enter an album into the database, making note of a collision if it happens.
    fn add_album(&mut self, artist: &str, album: &str, source: &str) -> bool {
        if self.would_collide(artist, album) {
            let existing_entry = self.albums_per_artist[artist]
                .get(&AlbumEntry::new_without_source(album.to_string()))
                .unwrap();

            let collision = Collision {
                artist: artist.to_string(),
                album: album.to_string(),
                already_exists_in: existing_entry.source_library_name.clone(),
                collision_with: source.to_string(),
            };

            self.collisions.push_back(collision);

            return false;
        }

        // If we don't know the artist, add an empty ArtistAlbumSet for them.
        if !self.albums_per_artist.contains_key(artist) {
            self.albums_per_artist.insert(artist.to_string(), HashSet::new());
        }

        let album_set = match self.albums_per_artist.get_mut(artist) {
            Some(value) => value,
            None => {
                return false;
            }
        };

        let did_collide = !album_set.insert(
            AlbumEntry::new(album.to_string().clone(), source.to_string().clone())
        );
        if did_collide {
            panic!("Album set somehow collided anyway!");
        }

        true
    }

    // TODO
}


// Validation
enum Validation {
    Valid,
    Invalid(Vec<String>)
}


/// Validate each individual music library (e.g. one for lossless, one for private, etc.)
/// Validation is done in multiple steps:
///  1. each library is checked for unusual or forbidden files (see configuration file),
///  2. validate that there are no collisions between any of the libraries.
pub fn cmd_validate(config: &Config) {
    let whiter_line = Style::new().white();

    console::horizontal_line(None, None);
    console::horizontal_line_with_text("LIBRARY VALIDATION (1/2: file types)", None, None, None, None);
    console::new_line();

    let mut collision_checker = CollisionChecker::new();

    // 1. check for forbidden or unusual files.
    for (_, library) in &config.libraries {
        console::horizontal_line_with_text(&format!("Library: \"{}\"", library.name), None, None, Some(whiter_line), None);

        let validation = match validate_library(config, &library, &mut collision_checker) {
            Ok(validation) => validation,
            Err(err) => {
                println!("{}", format!("Could not validate library because of an error: {}", err).red().bold());
                continue
            }
        };

        match validation {
            Validation::Valid => {
                println!("{}", "Library files are valid!".green().bold());
            },
            Validation::Invalid(errors) => {
                println!("{}", "Library files are not completely valid. Here are the errors:".red().bold());
                for (index, err) in errors.iter().enumerate() {
                    println!("  {}. {}", index + 1, err);
                }
            }
        }

        console::new_line();
    }

    // 2. check if there were any errors during collision checks.
    console::new_line();
    console::horizontal_line_with_text("LIBRARY VALIDATION (2/2: collisions)", None, None, None, None);

    if collision_checker.collisions.len() == 0 {
        println!("{}", "No collisions found!".green().bold())
    } else {
        println!("{}", "Found some collisions: ".red().bold());
        for (index, collision) in collision_checker.collisions.iter().enumerate() {
            let collision_title = format!(
                "{} - {}",
                collision.artist, collision.album
            );
            let collision_title = collision_title.bright_cyan();

            let collision_description = format!(
                "{} {} {} {}",
                "Libraries:".bright_black(),
                collision.already_exists_in.yellow().bold(),
                "and".bright_black(),
                collision.collision_with.yellow().bold()
            );

            println!("  {}. {}", index + 1, collision_title);
            println!("  {}  {}", " ".repeat((index + 1).to_string().len()), collision_description);
        }
    }

    console::new_line();
    console::horizontal_line_with_text("All libraries processed!", None, None, None, None);
}

fn validate_library(
    config: &Config, library: &ConfigLibrary, collision_checker: &mut CollisionChecker
) -> Result<Validation, Error> {
    // TODO Find a way to make paths look easily readable (currently mixed \\ and /, etc.).

    let mut invalid_list: Vec<String> = Vec::new();

    let audio_file_extensions = &library.audio_file_extensions;
    let must_not_contain_ext = &library.must_not_contain_extensions;

    let base_path = &library.path;
    let (files, directories) = mfs::list_directory_contents(Path::new(base_path))?;

    // Library structure should be:
    //  <library directory>
    //  |-- <artist>
    //  |   |-- <album>
    //  |   |   |-- <... audio files>
    //  |   |   |-- <... cover art>
    //  |   |   |-- <... possibly some album-related README, etc.>
    //  |   |   |-- <... possibly other directory that don't matter>
    //  |   |-- <... possibly some artist-related README, etc.>
    //  | ...
    //  |--

    // There should not be any audio files in the base directory.
    for file in &files {
        let file_path = file.path();
        let extension = mfs::get_dir_entry_file_extension(file)?;

        if config.validation.audio_file_extensions.contains(&extension) {
            invalid_list.push(format!("Unexpected audio file in base directory: {:?}", file_path))
        }
    }

    for artist_dir in &directories {
        let artist_name = mfs::get_dir_entry_name(artist_dir).unwrap();
        let (artist_files, artist_directories) = mfs::list_dir_entry_contents(artist_dir)?;

        // There should not be any audio files in the artist directory.
        for artist_dir_file in &artist_files {
            let file_path = artist_dir_file.path();
            let extension = mfs::get_dir_entry_file_extension(artist_dir_file)?;

            if config.validation.audio_file_extensions.contains(&extension) {
                invalid_list.push(format!("Unexpected audio file in artist directory: {:?}", file_path));
            }
        }

        for album_dir in &artist_directories {
            let album_name = mfs::get_dir_entry_name(album_dir).unwrap();
            let (album_files, _) = mfs::list_dir_entry_contents(album_dir)?;

            collision_checker.add_album(&artist_name, &album_name, &library.name);

            // This directory can contain audio files and any files specified in the ignored_file_extensions config value.
            for track_file in album_files {
                let file_path = track_file.path();
                let extension = mfs::get_dir_entry_file_extension(&track_file)?;

                let is_ok_audio_file = audio_file_extensions.contains(&extension);
                let is_ignored = config.validation.ignored_file_extensions.contains(&extension);
                let is_specifically_forbidden = must_not_contain_ext.contains(&extension);

                if is_specifically_forbidden {
                    invalid_list.push(format!("File with forbidden extension {}: {:?}", extension, file_path));
                } else if is_ok_audio_file || is_ignored {
                    continue
                } else {
                    invalid_list.push(format!("Unexpected file: {:?}", file_path));
                }
            }
        }
    }

    if invalid_list.len() == 0 {
        Ok(Validation::Valid)
    } else {
        Ok(Validation::Invalid(invalid_list))
    }
}
