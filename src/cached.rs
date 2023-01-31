/// Represents a value that is initially empty, but might become cached at some point
/// after it is requested once. This is a generic wrapper around a "cached value" concept.
///
/// TODO Perhaps it's time to re-evaluate how things are cached, panicking on `get` seems like a bad idea.
pub struct CachedValue<T> {
    value: Option<T>,
}

impl<T> CachedValue<T> {
    /// Creates a new, empty, `CachedValue`.
    pub fn new() -> CachedValue<T> {
        CachedValue { value: None }
    }

    /// Sets the cached value.
    pub fn set(&mut self, value: T) {
        self.value = Some(value);
    }

    /// Returns a `bool` indicating whether the value is cached.
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

impl<T> Clone for CachedValue<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
        }
    }
}
