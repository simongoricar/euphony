use std::collections::{LinkedList, HashSet, HashMap};
use std::hash::{Hash, Hasher};


#[derive(Eq)]
pub struct AlbumEntry {
    // (note that this struct is hashed/EQed only based on the name attribute, nothing else)
    pub name: String,
    pub source_library_name: String,
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


pub struct Collision {
    pub artist: String,
    pub album: String,
    // Library names.
    pub already_exists_in: String,
    pub collision_with: String,
}


pub struct CollisionChecker {
    /// Keys are artists, values are a set of albums we know about.
    pub albums_per_artist: HashMap<String, HashSet<AlbumEntry>>,
    pub collisions: LinkedList<Collision>,
}

impl CollisionChecker {
    pub fn new() -> CollisionChecker {
        CollisionChecker {
            albums_per_artist: HashMap::new(),
            collisions: LinkedList::new(),
        }
    }

    pub fn would_collide(&self, artist: &str, album: &str) -> bool {
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
    pub fn add_album(&mut self, artist: &str, album: &str, source: &str) -> bool {
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
}
