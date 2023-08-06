use std::fmt::Debug;

/// Represents a double `Vec`: one for audio files, the other for data files.
/// If you want to deal with unknown files as well, see `ExtendedSortedFileList`.
#[derive(Default, Debug, Clone)]
pub struct SortedFileList<T> {
    pub audio: Vec<T>,
    pub data: Vec<T>,
}

impl<T> SortedFileList<T> {
    /// Initialize a new `SortedFileList` by providing its audio and data vector.
    pub fn new(audio_list: Vec<T>, data_list: Vec<T>) -> Self {
        Self {
            audio: audio_list,
            data: data_list,
        }
    }

    /// Returns `true` if both `audio` and `data` lists are empty.
    pub fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.data.is_empty()
    }
}


/// Unlike `SortedFileList`, `ExtendedSortedFileList` includes `unknown` types of files.
/// That is the only difference.
#[derive(Default, Debug, Clone)]
pub struct ExtendedSortedFileList<T> {
    pub audio: Vec<T>,
    pub data: Vec<T>,
    pub unknown: Vec<T>,
}

impl<T> ExtendedSortedFileList<T> {
    /// Initialize a new `ExtendedSortedFileList` by providing its audio, data and unknown file vector.
    pub fn new(
        audio_list: Vec<T>,
        data_list: Vec<T>,
        unknown_list: Vec<T>,
    ) -> Self {
        Self {
            audio: audio_list,
            data: data_list,
            unknown: unknown_list,
        }
    }

    /// Returns `true` if `audio`, `data` and `unknown` lists are empty.
    pub fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.data.is_empty() && self.unknown.is_empty()
    }
}

/// We store file creation and modification in 64-bit floats, but we usually compare two times
/// that should match using some tolerance (usually to avoid rounding errors).
///
/// Set the `max_distance` to a tolerance of your choice. If the difference is larger,
/// this function returns `true`.
#[inline]
pub fn f64_approximate_eq(first: f64, second: f64, max_distance: f64) -> bool {
    (first - second).abs() < max_distance
}
