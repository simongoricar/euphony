use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;

use euphony_library::view::SharedAlbumView;
use linked_hash_map::{Iter, LinkedHashMap};
use miette::{miette, Result};

use crate::commands::transcode::state::changes::{FileJobContext, FileType};


/// Unique queue item ID.
///
/// Behind the scenes, this is represented with a `u32`
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct QueueItemID(u32);

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

/// A bare version of `AlbumItemState` and `FileItemState` that only has
/// the essential additional context.
///
/// Useful as a general state enum in `QueueItem` so we
/// don't have to parametrize another type.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum GenericQueueItemState {
    /// Item has been initialized, but not queued yet.
    Pending,

    /// Item has been queued and is waiting to be started.
    Queued,

    /// Item has been queued and is currently in progress
    /// (transcoding, copying, etc. - depending on context).
    InProgress,

    /// Item has been queued and has finished.
    ///
    /// The `ok` field indicates whether the operation this item represents
    /// has completed successfully.
    Finished { ok: bool },
}


/// `QueueItem` includes general methods of accessing queue items state and information.
///
/// Implement on specific queue items (e.g. `AlbumItem<'_>`, see below).
pub trait QueueItem<FinishResult: Debug> {
    /// Get the `QueueItem`s ID.
    fn get_id(&self) -> QueueItemID;

    /// Get the state the queue item is currently in.
    /// The returned type is a `QueueItemGenericState`, which is
    /// a simply, context-less version of `AlbumItemState` and `FileItemState`.
    ///
    /// This was done to avoid having another generic, but it means implementations
    /// must do one more state conversion.
    fn get_state(&self) -> GenericQueueItemState;

    /// Called by the queue when the item is queued.
    fn on_item_enqueued(&mut self);

    /// Called by the queue when the item is marked as started.
    fn on_item_started(&mut self);

    /// Called by the queue when the item is marked as finished.
    fn on_item_finished(&mut self, result: FinishResult);
}

/// To be implemented by queue items that can be rendered to the screen.
pub trait RenderableQueueItem<RenderOutput> {
    fn render(&self) -> RenderOutput;
}


/// Ease of use trait to help with queue item state queries.
///
/// *Not intended to be implemented manually, see blanket implementation below.*
pub trait QueueItemStateQuery<R: Debug> {
    fn is_pending(&self) -> bool;
    fn is_queued(&self) -> bool;
    fn is_in_progress(&self) -> bool;
    fn is_finished(&self) -> bool;
}

// Blanket implementation on basically all `QueueItem`s to help with querying for states.
impl<I: QueueItem<R>, R: Debug> QueueItemStateQuery<R> for I {
    fn is_pending(&self) -> bool {
        matches!(self.get_state(), GenericQueueItemState::Pending)
    }

    fn is_queued(&self) -> bool {
        matches!(self.get_state(), GenericQueueItemState::Queued)
    }

    fn is_in_progress(&self) -> bool {
        matches!(
            self.get_state(),
            GenericQueueItemState::InProgress
        )
    }

    fn is_finished(&self) -> bool {
        matches!(
            self.get_state(),
            GenericQueueItemState::Finished { .. }
        )
    }
}


/*
 * ALBUM QUEUE ITEM implementation
 */
#[derive(Copy, Clone, Debug)]
pub struct AlbumQueueItemFinishedResult {
    pub ok: bool,
}

impl AlbumQueueItemFinishedResult {
    pub fn new_ok() -> Self {
        Self { ok: true }
    }
}


#[derive(Copy, Clone)]
pub enum AlbumQueueItemState {
    /// Initialized, but not even queued yet.
    Pending,

    /// Queued and waiting to be started.
    Queued,

    /// Queued and in progress.
    InProgress,

    /// Queued and finished.
    Finished { ok: bool },
}

pub struct AlbumQueueItem<'config> {
    pub id: QueueItemID,

    pub album_view: SharedAlbumView<'config>,

    pub num_changed_audio_files: usize,
    pub num_changed_data_files: usize,

    pub state: AlbumQueueItemState,
}

impl<'a> AlbumQueueItem<'a> {
    pub fn new(
        album: SharedAlbumView<'a>,
        num_changed_audio_files: usize,
        num_changed_data_files: usize,
    ) -> Self {
        let random_id = QueueItemID::new_random();

        Self {
            id: random_id,
            album_view: album,
            num_changed_audio_files,
            num_changed_data_files,
            state: AlbumQueueItemState::Pending,
        }
    }
}

impl<'a> QueueItem<AlbumQueueItemFinishedResult> for AlbumQueueItem<'a> {
    #[inline]
    fn get_id(&self) -> QueueItemID {
        self.id
    }

    #[inline]
    fn get_state(&self) -> GenericQueueItemState {
        match self.state {
            AlbumQueueItemState::Pending => GenericQueueItemState::Pending,
            AlbumQueueItemState::Queued => GenericQueueItemState::Queued,
            AlbumQueueItemState::InProgress => GenericQueueItemState::InProgress,
            AlbumQueueItemState::Finished { ok } => {
                GenericQueueItemState::Finished { ok }
            }
        }
    }

    fn on_item_enqueued(&mut self) {
        self.state = AlbumQueueItemState::Queued;
    }

    fn on_item_started(&mut self) {
        self.state = AlbumQueueItemState::InProgress;
    }

    fn on_item_finished(&mut self, result: AlbumQueueItemFinishedResult) {
        self.state = AlbumQueueItemState::Finished { ok: result.ok }
    }
}

impl<'a> RenderableQueueItem<String> for AlbumQueueItem<'a> {
    fn render(&self) -> String {
        // This is just a placeholder implementation as concrete backend implementations
        // are expected to "subclass" (enclose) this struct with their specific implementation.
        let album_locked = self.album_view.read();

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
#[deprecated]
#[allow(dead_code)]
#[derive(Copy, Clone)]
pub enum FileQueueItemType {
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
pub enum FileQueueItemState {
    /// Initialized, but not even queued yet.
    Pending,

    /// Queued and waiting to be started.
    Queued,

    /// Queued and in progress.
    InProgress,

    /// Queued and finished.
    Finished { result: FileQueueItemFinishedResult },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileQueueItemErrorType {
    Cancelled,
    Errored { error: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileQueueItemFinishedResult {
    Ok,
    Failed(FileQueueItemErrorType),
}

pub struct FileQueueItem<'config> {
    pub id: QueueItemID,

    pub album_view: SharedAlbumView<'config>,

    pub file_name: String,

    pub context: FileJobContext,

    pub state: FileQueueItemState,
}

impl<'config> FileQueueItem<'config> {
    pub fn new(
        album: SharedAlbumView<'config>,
        file_name: String,
        context: FileJobContext,
    ) -> Self {
        let random_id = QueueItemID::new_random();

        Self {
            id: random_id,
            album_view: album,
            file_name,
            context,
            state: FileQueueItemState::Pending,
        }
    }
}

impl<'config> QueueItem<FileQueueItemFinishedResult> for FileQueueItem<'config> {
    #[inline]
    fn get_id(&self) -> QueueItemID {
        self.id
    }

    #[inline]
    fn get_state(&self) -> GenericQueueItemState {
        match &self.state {
            FileQueueItemState::Pending => GenericQueueItemState::Pending,
            FileQueueItemState::Queued => GenericQueueItemState::Queued,
            FileQueueItemState::InProgress => GenericQueueItemState::InProgress,
            FileQueueItemState::Finished { result } => {
                GenericQueueItemState::Finished {
                    ok: result == &FileQueueItemFinishedResult::Ok,
                }
            }
        }
    }

    fn on_item_enqueued(&mut self) {
        self.state = FileQueueItemState::Queued;
    }

    fn on_item_started(&mut self) {
        self.state = FileQueueItemState::InProgress;
    }

    fn on_item_finished(&mut self, result: FileQueueItemFinishedResult) {
        self.state = FileQueueItemState::Finished { result }
    }
}

impl<'config> RenderableQueueItem<String> for FileQueueItem<'config> {
    fn render(&self) -> String {
        // This is just a placeholder implementation as concrete backend implementations
        // are expected to "subclass" (enclose) this struct with their specific implementation.
        format!(
            "{} {}",
            match self.context.file_type {
                FileType::Audio => "[audio]",
                FileType::Data => "[data]",
                FileType::Unknown => "    [??]",
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
pub struct Queue<Item: QueueItem<FinishedResult>, FinishedResult: Debug> {
    /// We use `LinkedHashMap` because we need to preserve the insertion order
    /// and also be able to quickly get specific items by their keys.
    items: LinkedHashMap<QueueItemID, Item>,

    /// Couldn't make the compiler ignore that `R` is unused, so I added `PhantomData`.
    /// Maybe I'm just stupid? We need that R in `impl` below.
    _phantom_data: PhantomData<FinishedResult>,
}

impl<I: QueueItem<R>, R: Debug> Queue<I, R> {
    /// Instantiate a new empty `Queue`.
    pub fn new() -> Self {
        Self {
            items: LinkedHashMap::new(),
            _phantom_data: PhantomData,
        }
    }

    /// Get a reference to the item with the given `QueueItemID`.
    ///
    /// If an item with the given ID exists, `Some(&mut queue_item)` is returned.
    ///
    /// If no such item exists, `None` is returned.
    pub fn item(&mut self, item_id: QueueItemID) -> Option<&I> {
        self.items.get(&item_id)
    }

    /// Get a mutable reference to the item with the given `QueueItemID`.
    ///
    /// If an item with the given ID exists, `Some(&mut queue_item)` is returned.
    ///
    /// If no such item exists, `None` is returned.
    pub fn item_mut(&mut self, item_id: QueueItemID) -> Option<&mut I> {
        self.items.get_mut(&item_id)
    }

    pub fn items(&self) -> Iter<QueueItemID, I> {
        self.items.iter()
    }

    /// Adds an item to the queue.
    pub fn queue_item(&mut self, mut item: I) -> Result<()> {
        let item_id = item.get_id();
        if self.items.contains_key(&item_id) {
            return Err(miette!("This queue item already exists."));
        }

        item.on_item_enqueued();
        self.items.insert(item_id, item);
        Ok(())
    }

    /// Remove a queue item by its `QueueItemID`. If no such item exists,
    /// this method returns `Err`, otherwise `Ok(removed_item)`.
    pub fn remove_item(&mut self, item_id: QueueItemID) -> Result<I> {
        let removed_item = self
            .items
            .remove(&item_id)
            .ok_or_else(|| miette!("No such queue item."))?;

        Ok(removed_item)
    }

    /// Put the given item into its "in-progress" state by calling its `start` method.
    pub fn start_item(&mut self, item_id: QueueItemID) -> Result<()> {
        let item = self
            .item_mut(item_id)
            .ok_or_else(|| miette!("No such queue item."))?;

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
        let item = self
            .item_mut(item_id)
            .ok_or_else(|| miette!("No such queue item."))?;

        item.on_item_finished(result);

        Ok(())
    }

    /// Clear the queue.
    ///
    /// Note that this does not free up the existing allocated memory of the `Vec` backing this queue
    /// (same behaviour as `Vec` - the existing capacity remains).
    pub fn clear(&mut self) {
        self.items.clear();
    }
}
