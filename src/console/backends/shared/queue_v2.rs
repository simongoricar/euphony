use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;

use linked_hash_map::LinkedHashMap;
use miette::{miette, Result};

use crate::commands::transcode::views::SharedAlbumView;

/// Queue item ID, which is just a `u32` behind the scenes.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct QueueItemID(pub u32);

impl QueueItemID {
    /// Generate a new random 32-bit `QueueItemID`.
    pub fn new_random() -> Self {
        Self(rand::random::<u32>())
    }
}

impl Deref for QueueItemID {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A bare version of `AlbumItemState` and `FileItemState` that has only the bare additional context.
/// Useful as a general state enum in `QueueItem` so we
/// don't have to parametrize another type.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum QueueItemGenericState {
    /// Initialized, but not even queued yet.
    Pending,

    /// Queued and waiting to be started.
    Queued,

    /// Queued and in progress.
    InProgress,

    /// Queued and finished.
    Finished { ok: bool },
}


pub trait QueueItem<FinishResult: Debug> {
    /// Get the `QueueItem`s ID.
    fn get_id(&self) -> QueueItemID;

    /// Get the state the queue item is currently in.
    /// The returned type is a `QueueItemGenericState`, which is
    /// a bare, context-less version of `AlbumItemState` and `FileItemState`.
    ///
    /// This was done to avoid having another generic, but it means implementations
    /// must do one more state conversion.
    fn get_state(&self) -> QueueItemGenericState;

    /// Called by the queue when the item is put into it.
    fn on_item_enqueued(&mut self);

    /// Called by the queue when the item is marked as started.
    fn on_item_started(&mut self);

    /// Called by the queue when the item is marked as finished.
    fn on_item_finished(&mut self, result: FinishResult);
}

pub trait RenderableQueueItem<RenderOutput> {
    fn render(&self) -> RenderOutput;
}


/// Shortcut trait to help with queue item state queries.
pub trait QueueItemStateQuery<R: Debug> {
    fn is_pending(&self) -> bool;
    fn is_queued(&self) -> bool;
    fn is_in_progress(&self) -> bool;
    fn is_finished(&self) -> bool;
}

// Blanket implementation on basically all `QueueItem`s to help with querying for states.
impl<I: QueueItem<R>, R: Debug> QueueItemStateQuery<R> for I {
    fn is_pending(&self) -> bool {
        match self.get_state() {
            QueueItemGenericState::Pending => true,
            _ => false,
        }
    }

    fn is_queued(&self) -> bool {
        match self.get_state() {
            QueueItemGenericState::Queued => true,
            _ => false,
        }
    }

    fn is_in_progress(&self) -> bool {
        match self.get_state() {
            QueueItemGenericState::InProgress => true,
            _ => false,
        }
    }

    fn is_finished(&self) -> bool {
        match self.get_state() {
            QueueItemGenericState::Finished { .. } => true,
            _ => false,
        }
    }
}


/*
 * ALBUM QUEUE ITEM implementation
 */
#[derive(Copy, Clone, Debug)]
pub struct AlbumItemFinishedResult {
    pub ok: bool,
}

#[derive(Copy, Clone)]
pub enum AlbumItemState {
    /// Initialized, but not even queued yet.
    Pending,

    /// Queued and waiting to be started.
    Queued,

    /// Queued and in progress.
    InProgress,

    /// Queued and finished.
    Finished { ok: bool },
}

pub struct AlbumItem<'a> {
    pub id: QueueItemID,

    pub album_view: SharedAlbumView<'a>,

    pub num_changed_files: usize,

    pub state: AlbumItemState,
}

impl<'a> AlbumItem<'a> {
    pub fn new(album: SharedAlbumView<'a>, num_changed_files: usize) -> Self {
        let random_id = QueueItemID::new_random();

        Self {
            id: random_id,
            album_view: album,
            num_changed_files,
            state: AlbumItemState::Pending,
        }
    }
}

impl<'a> QueueItem<AlbumItemFinishedResult> for AlbumItem<'a> {
    #[inline]
    fn get_id(&self) -> QueueItemID {
        self.id
    }

    #[inline]
    fn get_state(&self) -> QueueItemGenericState {
        match self.state {
            AlbumItemState::Pending => QueueItemGenericState::Pending,
            AlbumItemState::Queued => QueueItemGenericState::Queued,
            AlbumItemState::InProgress => QueueItemGenericState::InProgress,
            AlbumItemState::Finished { ok } => {
                QueueItemGenericState::Finished { ok }
            }
        }
    }

    fn on_item_enqueued(&mut self) {
        self.state = AlbumItemState::Queued;
    }

    fn on_item_started(&mut self) {
        self.state = AlbumItemState::InProgress;
    }

    fn on_item_finished(&mut self, result: AlbumItemFinishedResult) {
        self.state = AlbumItemState::Finished { ok: result.ok }
    }
}

impl<'a> RenderableQueueItem<String> for AlbumItem<'a> {
    fn render(&self) -> String {
        // This is just a placeholder implementation as concrete backend implementations
        // are expected to "subclass" (enclose) this struct with their specific implementation.
        let album_locked = self
            .album_view
            .read()
            .expect("AlbumView RwLock has been poisoned!");

        format!(
            "{} - {}",
            album_locked.read_lock_artist().name,
            album_locked.title,
        )
    }
}


/*
 * FILE QUEUE ITEM implementation
 */
#[derive(Copy, Clone)]
pub enum FileItemType {
    /// Audio files, as configured per-library.
    Audio,

    /// Data (non-audio) files, as configured per-library.
    Data,

    /// Unknown (non-audio, non-data) files.
    ///
    /// This type only appears in cases of "excess" files in the transcoded library
    /// (see `AlbumFileChangesV2::generate_from_source_and_transcoded_state`).
    Unknown,
}

#[derive(Clone, Eq, PartialEq)]
pub enum FileItemState {
    /// Initialized, but not even queued yet.
    Pending,

    /// Queued and waiting to be started.
    Queued,

    /// Queued and in progress.
    InProgress,

    /// Queued and finished.
    Finished { result: FileItemFinishedResult },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileItemErrorType {
    Cancelled,
    Errored { error: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileItemFinishedResult {
    Ok,
    Failed(FileItemErrorType),
}

pub struct FileItem<'a> {
    pub id: QueueItemID,

    pub album_view: SharedAlbumView<'a>,

    pub file_type: FileItemType,

    pub file_name: String,

    pub state: FileItemState,
}

impl<'a> FileItem<'a> {
    pub fn new(
        album: SharedAlbumView<'a>,
        file_type: FileItemType,
        file_name: String,
    ) -> Self {
        let random_id = QueueItemID::new_random();

        Self {
            id: random_id,
            album_view: album,
            file_type,
            file_name,
            state: FileItemState::Pending,
        }
    }
}

impl<'a> QueueItem<FileItemFinishedResult> for FileItem<'a> {
    #[inline]
    fn get_id(&self) -> QueueItemID {
        self.id
    }

    #[inline]
    fn get_state(&self) -> QueueItemGenericState {
        match &self.state {
            FileItemState::Pending => QueueItemGenericState::Pending,
            FileItemState::Queued => QueueItemGenericState::Queued,
            FileItemState::InProgress => QueueItemGenericState::InProgress,
            FileItemState::Finished { result } => {
                QueueItemGenericState::Finished {
                    ok: result == &FileItemFinishedResult::Ok,
                }
            }
        }
    }

    fn on_item_enqueued(&mut self) {
        self.state = FileItemState::Queued;
    }

    fn on_item_started(&mut self) {
        self.state = FileItemState::InProgress;
    }

    fn on_item_finished(&mut self, result: FileItemFinishedResult) {
        self.state = FileItemState::Finished { result }
    }
}

impl<'a> RenderableQueueItem<String> for FileItem<'a> {
    fn render(&self) -> String {
        // This is just a placeholder implementation as concrete backend implementations
        // are expected to "subclass" (enclose) this struct with their specific implementation.
        format!(
            "{} {}",
            match self.file_type {
                FileItemType::Audio => "[audio]",
                FileItemType::Data => " [data]",
                FileItemType::Unknown => "   [??]",
            },
            self.file_name,
        )
    }
}


/*
 * QUEUE implementation
 */

/// A generic queue. Its items must implement `QueueItem`,
/// making the queue items identifiable by their `QueueItemID`
/// as well as having several event handlers.
pub struct Queue<I: QueueItem<R>, R: Debug> {
    pub items: LinkedHashMap<QueueItemID, I>,

    /// Couldn't make the compiler ignore that `R` is unused, so I added `PhantomData`.
    /// Maybe I'm just stupid? We need that R in `impl` below.
    _phantom_data: PhantomData<R>,
}

impl<I: QueueItem<R>, R: Debug> Queue<I, R> {
    /// Instantiate a new empty `Queue`.
    pub fn new() -> Self {
        Self {
            items: LinkedHashMap::new(),
            _phantom_data: PhantomData::default(),
        }
    }

    /// Adds an item to the queue.
    pub fn add_item(&mut self, mut item: I) -> Result<()> {
        let item_id = item.get_id();
        if self.items.contains_key(&item_id) {
            return Err(miette!("This queue item already exists."));
        }

        item.on_item_enqueued();
        self.items.insert(item_id, item);
        Ok(())
    }

    /// Get a reference to the item with the given `QueueItemID`.
    /// If no such item exists, `Err` is returned. If an item with the same ID exists,
    /// `Ok(&mut queue_item)` is returned.
    pub fn item(&mut self, item_id: QueueItemID) -> Result<&I> {
        let item = self
            .items
            .get(&item_id)
            .ok_or_else(|| miette!("No such queue item."))?;

        Ok(item)
    }

    /// Get a mutable reference to the item with the given `QueueItemID`.
    /// If no such item exists, `Err` is returned. If an item with the same ID exists,
    /// `Ok(&mut queue_item)` is returned.
    pub fn item_mut(&mut self, item_id: QueueItemID) -> Result<&mut I> {
        let item = self
            .items
            .get_mut(&item_id)
            .ok_or_else(|| miette!("No such queue item."))?;

        Ok(item)
    }

    /// Remove a queue item by its `QueueItemID`. If no such item exists,
    /// this method returns `Err`, otherwise `Ok(removed_item)`.
    pub fn remove_item(&mut self, item_id: QueueItemID) -> Result<I> {
        if !self.items.contains_key(&item_id) {
            return Err(miette!("No such queue item."));
        }

        let removed_item = self.items.remove(&item_id).expect(
            "Can't remove item even though contains_key assured us it exists.",
        );

        Ok(removed_item)
    }

    /// Put the given item into its "in-progress" state by calling its `start` method.
    pub fn start_item(&mut self, item_id: QueueItemID) -> Result<()> {
        let item = self.item_mut(item_id)?;

        item.on_item_started();
        Ok(())
    }

    /// Put the given item into its "finished" state by calling its `finish` method.
    /// The parameter `result` should contain the result, the type of which depends on the
    /// items in the queue (see `QueueItem`'s `FinishResult` generic).
    pub fn finish_item(
        &mut self,
        item_id: QueueItemID,
        result: R,
    ) -> Result<()> {
        let item = self.item_mut(item_id)?;

        item.on_item_finished(result);
        Ok(())
    }

    /// Clear the queue.
    ///
    /// Note that this does not free up the existing allocated memory
    /// of the `Vec` backing this queue (same behaviour as `Vec` - capacity remains).
    pub fn clear(&mut self) {
        self.items.clear();
    }
}
