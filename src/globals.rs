/// A global boolean indicating whether we are running in verbose mode.
pub static VERBOSE: state::Storage<bool> = state::Storage::new();

/// Shorthand to get the global flag value for verbosity.
#[inline]
pub fn is_verbose_enabled() -> bool {
    VERBOSE.get().eq(&true)
}
