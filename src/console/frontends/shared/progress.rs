/// A small progress bar abstraction that contains just two fields: `current` out of `total` progress.
#[derive(Default, Copy, Clone, Eq, PartialEq)]
pub struct Progress {
    pub total_files: usize,

    pub audio_files_currently_processing: usize,
    pub data_files_currently_processing: usize,

    pub audio_files_finished_ok: usize,
    pub data_files_finished_ok: usize,

    pub audio_files_errored: usize,
    pub data_files_errored: usize,
}

impl Progress {
    #[inline]
    pub fn total_not_pending(&self) -> usize {
        self.audio_files_currently_processing
            + self.data_files_currently_processing
            + self.audio_files_finished_ok
            + self.data_files_finished_ok
            + self.audio_files_errored
            + self.data_files_errored
    }

    #[inline]
    pub fn total_finished_or_errored(&self) -> usize {
        self.audio_files_finished_ok
            + self.data_files_finished_ok
            + self.audio_files_errored
            + self.data_files_errored
    }

    #[inline]
    pub fn total_pending(&self) -> usize {
        self.total_files.saturating_sub(self.total_not_pending())
    }

    /// Get progress percentage.
    #[inline]
    pub fn completion_ratio(&self) -> f64 {
        if self.total_files == 0 {
            0f64
        } else {
            self.total_finished_or_errored() as f64 / self.total_files as f64
        }
    }
}
