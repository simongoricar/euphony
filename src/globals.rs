/// Global boolean indicating whether we are running in verbose mode.
pub static VERBOSE: state::Storage<bool> = state::Storage::new();

/// Get the global flag value for verbosity.
pub fn is_verbose_enabled() -> bool {
    VERBOSE.get().eq(&true)
}
