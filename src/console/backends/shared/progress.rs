/// A small progress bar abstraction that contains just two fields: `current` out of `total` progress.
#[derive(Default)]
pub struct ProgressState {
    pub current: usize,
    pub total: usize,
}

impl ProgressState {
    /// Get the progress bar completion percentage.
    pub fn get_percent(&self) -> u16 {
        if self.total == 0 {
            0
        } else {
            (self.current as f32 / self.total as f32 * 100.0) as u16
        }
    }
}
