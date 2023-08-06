/*
In order to allow the code to share the library, artist and album views, we wrap them
in an `Arc` (and its `Weak` reference variant, when stored).

`Shared*` types are essentially `RwLock`ed library/artist/album views under an `Arc`.
`Weak*` types are `Weak` references to the same views - call `upgrade` to obtain the corresponding `Shared*` type.
*/

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Weak};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::commands::transcode::album_state::changes::AlbumFileChangesV2;
use crate::commands::transcode::views::album::SharedAlbumView;

pub type ArcRwLock<T> = Arc<RwLock<T>>;
pub type WeakRwLock<T> = Weak<RwLock<T>>;

pub type ChangedAlbumsMap<'a> =
    HashMap<String, (SharedAlbumView<'a>, AlbumFileChangesV2<'a>)>;

/// Represents a double `HashMap`: one for audio files, the other for data files.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SortedFileMap<K: Eq + Hash, V> {
    pub audio: HashMap<K, V>,
    pub data: HashMap<K, V>,
}

impl<K: Eq + Hash, V> SortedFileMap<K, V> {
    pub fn new(audio_map: HashMap<K, V>, data_map: HashMap<K, V>) -> Self {
        Self {
            audio: audio_map,
            data: data_map,
        }
    }

    /// Get a value by key from either `audio` or `data` map.
    /// Works like the normal `get` method on `HashMap`s.
    pub fn get(&self, key: &K) -> Option<&V> {
        let value_in_audio_map = self.audio.get(key);

        if value_in_audio_map.is_some() {
            value_in_audio_map
        } else {
            self.data.get(key)
        }
    }

    /// Consumes the `SortedFileMap` and returns a flat `HashMap` with
    /// key-value pairs from both `audio` and `data`.  
    pub fn into_flattened_map(self) -> HashMap<K, V> {
        let mut flat_hashmap: HashMap<K, V> =
            HashMap::with_capacity(self.audio.len() + self.data.len());

        flat_hashmap.extend(self.audio.into_iter());
        flat_hashmap.extend(self.data.into_iter());

        flat_hashmap
    }
}

impl<K: Eq + Hash + Clone, V: Eq + Hash + Clone> SortedFileMap<K, V> {
    /// Inverts the current file map: all keys become values and values become their keys.
    pub fn to_inverted_map(&self) -> SortedFileMap<V, K> {
        let audio_inverted_map: HashMap<V, K> = self
            .audio
            .iter()
            .map(|(key, value)| (value.clone(), key.clone()))
            .collect();
        let data_inverted_map: HashMap<V, K> = self
            .data
            .iter()
            .map(|(key, value)| (value.clone(), key.clone()))
            .collect();

        SortedFileMap::new(audio_inverted_map, data_inverted_map)
    }
}
