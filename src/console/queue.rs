use std::ops::Deref;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct QueueItemID(pub u32);

impl Deref for QueueItemID {
    type Target = u32;
    
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct QueueItem {
    pub content: String,
    pub id: QueueItemID,
    pub is_active: bool,
    pub is_ok: bool,
}

impl QueueItem {
    pub fn new<S: Into<String>>(content: S) -> Self {
        let random_id = QueueItemID(rand::random::<u32>());
        
        Self {
            content: content.into(),
            id: random_id,
            is_active: false,
            is_ok: true,
        }
    }
}