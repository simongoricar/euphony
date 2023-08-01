use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct CancellationToken {
    token: Arc<AtomicBool>,
}

impl CancellationToken {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            token: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.token.store(true, Ordering::SeqCst);
    }
}
