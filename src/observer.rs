use std::rc::Weak;

pub trait Observable<E> {
    /// Register a new observer that will receive all Observable events.
    fn register_observer(&mut self, observer: Weak<dyn Observer<E>>);
}

pub trait Observer<E> {
    /// Emit the event to the observer (called by the Observable).
    fn emit(&self, event: E);
}
