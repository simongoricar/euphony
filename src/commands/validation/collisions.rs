use std::collections::{HashMap, HashSet, LinkedList};
use std::collections::hash_map::Entry;
use std::hash::{Hash, Hasher};

/// ArtistAlbumEntry is a way of identifying a single artist-album combination in the library.
/// It contains only the basic information: the artist name, the album title and the library it comes from.
/// Note that this struct is hashed and compared by both the artist name and album title, but NOT the source library name.
#[derive(Eq)]
pub struct ArtistAlbumEntry {
    pub artist_name: String,
    pub album_title: String,
    pub source_library_name: String,
}

impl ArtistAlbumEntry {
    fn new_without_source<S: Into<String>>(artist_name: S, album_title: S) -> Self {
        ArtistAlbumEntry {
            artist_name: artist_name.into(),
            album_title: album_title.into(),
            source_library_name: String::from(""),
        }
    }

    fn new<S: Into<String>>(artist_name: S, album_title: S, source_library_name: S) -> Self {
        ArtistAlbumEntry {
            artist_name: artist_name.into(),
            album_title: album_title.into(),
            source_library_name: source_library_name.into(),
        }
    }
}

impl PartialEq<Self> for ArtistAlbumEntry {
    fn eq(&self, other: &Self) -> bool {
        self.artist_name.eq(&other.artist_name)
            && self.album_title.eq(&other.album_title)
    }
}

impl Hash for ArtistAlbumEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.artist_name.hash(state);
        self.album_title.hash(state);
    }
}

pub struct Collision {
    pub artist_name: String,
    pub album_title: String,
    pub colliding_libraries_by_name: (String, String),
}


pub struct CollisionAudit {
    artist_name_to_album_entry_set: HashMap<String, HashSet<ArtistAlbumEntry>>,
    pub collisions: LinkedList<Collision>,
}

impl CollisionAudit {
    pub fn new() -> Self {
        CollisionAudit {
            artist_name_to_album_entry_set: HashMap::new(),
            collisions: LinkedList::new(),
        }
    }

    /// Computes whether adding the specified artist and associated album into the collision auditor
    /// would introduce an album collision.
    fn would_collide<R: AsRef<str>>(&self, artist_name: R, album_title: R) -> bool {
        if self.artist_name_to_album_entry_set.contains_key(artist_name.as_ref()) {
            // Could be a collision, check the album list for this artist.
            let placeholder_entry = ArtistAlbumEntry::new_without_source(
                artist_name.as_ref(),
                album_title.as_ref(),
            );
            self.artist_name_to_album_entry_set[artist_name.as_ref()].contains(&placeholder_entry)

        } else {
            // We don't even know the artist yet, no chance for a collision.
            false
        }
    }

    /// Attempts to retrieve a known ArtistAlbumEntry by its artist name and album title.
    fn get_entry<S: AsRef<str>>(&self, artist_name: S, album_title: S) -> Option<&ArtistAlbumEntry> {
        let placeholder_entry = ArtistAlbumEntry::new_without_source(
            artist_name.as_ref(),
            album_title.as_ref(),
        );

        self.artist_name_to_album_entry_set
            .get(artist_name.as_ref())?
            .get(&placeholder_entry)
    }

    /// Adds the specified artist-album entry to the collision auditer. If a collision is found,
    /// its details are saved into the instance's `collisions` attribute and `false` is returned from this method.
    pub fn add_album<S: Into<String>>(&mut self, artist_name: S, album_title: S, source_library_name: S) -> bool {
        let artist_name = artist_name.into();
        let album_title = album_title.into();
        let source_library_name = source_library_name.into();

        if self.would_collide(&artist_name, &album_title) {
            let existing_album_entry = self.get_entry(artist_name, album_title)
                .expect("Artist-album combination is supposed to collide, but no entry could be found.");

            let collision = Collision {
                artist_name: existing_album_entry.artist_name.clone(),
                album_title: existing_album_entry.album_title.clone(),
                colliding_libraries_by_name: (
                    existing_album_entry.source_library_name.clone(),
                    source_library_name,
                )
            };

            self.collisions.push_back(collision);

            return false;
        }

        if let Entry::Vacant(e) =
            self.artist_name_to_album_entry_set.entry(artist_name.clone())
        {
            // First album for this artist
            let mut album_set = HashSet::new();
            album_set.insert(
                ArtistAlbumEntry::new(&artist_name, &album_title, &source_library_name)
            );

            e.insert(album_set);

        } else {
            // This artist already has some albums
            let artist_album_set = self.artist_name_to_album_entry_set
                .get_mut(&artist_name)
                .expect("Key was found but get_mut returned None?");

            artist_album_set.insert(
                ArtistAlbumEntry::new(artist_name, album_title, source_library_name),
            );
        }

        true
    }

    pub fn has_collisions(&self) -> bool {
        !self.collisions.is_empty()
    }
}
