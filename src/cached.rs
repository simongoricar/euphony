/// Represents a value that is initially empty, but can become cached at some point.
/// This is just a generic wrapper around this concept.
pub struct CachedValue<T> {
    value: Option<T>,
}

impl<T> CachedValue<T> {
    pub fn new_empty() -> CachedValue<T> {
        CachedValue {
            value: None,
        }
    }

    pub fn set(&mut self, value: T) {
        self.value = Some(value);
    }

    pub fn is_cached(&self) -> bool {
        self.value.is_some()
    }

    /// Extracts the refernce to the inner cached value.
    /// **NOTE: This method panics if the inner value is not cached.**
    /// Use `is_cached` beforehand.
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("Could not get value.")
    }
}
