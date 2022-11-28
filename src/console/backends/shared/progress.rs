/// Tiny progress bar abstraction.
#[derive(Default)]
pub struct ProgressState {
    pub current: usize,
    pub total: usize,
}

impl ProgressState {
    pub fn get_percent(&self) -> u16 {
        if self.total == 0 {
            0
        } else {
            (self.current as f32 / self.total as f32 * 100.0) as u16
        }
    }
}
