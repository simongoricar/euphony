/// Global boolean indicating whether we are running in verbose mode.
pub static VERBOSE: state::Storage<bool> = state::Storage::new();

pub fn verbose_enabled() -> bool {
    VERBOSE.get().eq(&true)
}
