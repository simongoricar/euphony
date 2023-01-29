use std::fmt::Display;
use std::path::PathBuf;

use crossbeam::channel::Receiver;

pub use bare::*;
pub use fancy::*;

use crate::console::{
    LogBackend,
    LogToFileBackend,
    TerminalBackend,
    TranscodeBackend,
    UserControllableBackend,
    UserControlMessage,
    ValidationBackend,
    ValidationErrorInfo,
};
use crate::console::backends::shared::{QueueItem, QueueItemID, QueueType};

mod fancy;
mod bare;
pub mod shared;


/// This macro implements `From` on the given enum
/// by constructing the given variant from the given terminal backend.
macro_rules! terminal_impl_direct_from {
    ($target: path, $source_backend: path, $target_variant: path) => {
        impl From<$source_backend> for $target {
            fn from(item: $source_backend) -> Self {
                $target_variant(item)
            }
        }
    };
}


/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
///
/// This macro implements the `TerminalBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_terminal {
    ($t: path, $($variant: path),+) => {
        impl TerminalBackend for $t {
            fn setup(&mut self) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.setup()),+
                }
            }
            
            fn destroy(&mut self) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.destroy()),+
                }
            }
        }
    };
}

/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
///
/// This macro implements the `LogBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_log {
    ($t: path, $($variant: path),+) => {
        impl LogBackend for $t {
            fn log_newline(&self) {
                match self {
                    $($variant(terminal) => terminal.log_newline()),+
                }
            }
            
            fn log_println<D: Display>(&self, content: D) {
                match self {
                    $($variant(terminal) => terminal.log_println(content)),+
                }
            }
        }
    }
}

/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
///
/// This macro implements the `LogToFileBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_log_to_file {
    ($t: path, $($variant: path),+) => {
        impl LogToFileBackend for $t {
            fn enable_saving_logs_to_file(&mut self, log_file_path: PathBuf) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.enable_saving_logs_to_file(log_file_path)),+
                }
            }
            
            fn disable_saving_logs_to_file(&mut self) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.disable_saving_logs_to_file()),+
                }
            }
        }
    }
}

/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
///
/// This macro implements the `ValidationBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_validation {
    ($t: path, $($variant: path),+) => {
        impl ValidationBackend for $t {
            fn validation_add_error(&self, error: ValidationErrorInfo) {
                match self {
                    $($variant(terminal) => terminal.validation_add_error(error)),+
                }
            }
        }
    };
}

/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
///
/// This macro implements the `TranscodeBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_transcode {
    ($t: path, $($variant: path),+) => {
        impl TranscodeBackend for $t {
            fn queue_begin(&mut self) {
                match self {
                    $($variant(terminal) => terminal.queue_begin()),+
                }
            }
            
            fn queue_end(&mut self) {
                match self {
                    $($variant(terminal) => terminal.queue_end()),+
                }
            }
            
            fn queue_item_add(&mut self, item: String, item_type: QueueType) -> miette::Result<QueueItemID> {
                match self {
                    $($variant(terminal) => terminal.queue_item_add(item, item_type)),+
                }
            }
            
            fn queue_item_start(&mut self, item_id: QueueItemID) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_item_start(item_id)),+
                }
            }
            
            fn queue_item_finish(&mut self, item_id: QueueItemID, was_ok: bool) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_item_finish(item_id, was_ok)),+
                }
            }
            
            fn queue_item_modify(&mut self, item_id: QueueItemID, function: Box<dyn FnOnce(&mut QueueItem)>) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_item_modify(item_id, function)),+
                }
            }
            
            fn queue_item_remove(&mut self, item_id: QueueItemID) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_item_remove(item_id)),+
                }
            }
            
            fn queue_clear(&mut self, queue_type: QueueType) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_clear(queue_type)),+
                }
            }
            
            fn progress_begin(&mut self) {
                match self {
                    $($variant(terminal) => terminal.progress_begin()),+
                }
            }
            
            fn progress_end(&mut self) {
                match self {
                    $($variant(terminal) => terminal.progress_end()),+
                }
            }
            
            fn progress_set_total(&mut self, total: usize) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.progress_set_total(total)),+
                }
            }
            
            fn progress_set_current(&mut self, finished: usize) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.progress_set_current(finished)),+
                }
            }
        }
    };
}

/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
///
/// This macro implements the `UserControllableBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_user_controllable {
    ($t: path, $($variant: path),+) => {
        impl UserControllableBackend for $t {
            fn get_user_control_receiver(&mut self) -> miette::Result<Receiver<UserControlMessage>> {
                match self {
                    $($variant(terminal) => terminal.get_user_control_receiver()),+
                }
            }
        }
    };
}

#[allow(clippy::large_enum_variant)]
pub enum SimpleTerminal {
    Bare(BareTerminalBackend),
    Fancy(TUITerminalBackend),
}

terminal_impl_direct_from!(SimpleTerminal, BareTerminalBackend, SimpleTerminal::Bare);
terminal_impl_direct_from!(SimpleTerminal, TUITerminalBackend, SimpleTerminal::Fancy);

enumdispatch_impl_terminal!(SimpleTerminal, SimpleTerminal::Bare, SimpleTerminal::Fancy);
enumdispatch_impl_log!(SimpleTerminal, SimpleTerminal::Bare, SimpleTerminal::Fancy);
enumdispatch_impl_log_to_file!(SimpleTerminal, SimpleTerminal::Bare, SimpleTerminal::Fancy);


pub enum ValidationTerminal {
    Bare(BareTerminalBackend),
}

terminal_impl_direct_from!(ValidationTerminal, BareTerminalBackend, ValidationTerminal::Bare);

enumdispatch_impl_terminal!(ValidationTerminal, ValidationTerminal::Bare);
enumdispatch_impl_log!(ValidationTerminal, ValidationTerminal::Bare);
enumdispatch_impl_log_to_file!(ValidationTerminal, ValidationTerminal::Bare);
enumdispatch_impl_validation!(ValidationTerminal, ValidationTerminal::Bare);


#[allow(clippy::large_enum_variant)]
pub enum TranscodeTerminal {
    Bare(BareTerminalBackend),
    Fancy(TUITerminalBackend),
}

terminal_impl_direct_from!(TranscodeTerminal, BareTerminalBackend, TranscodeTerminal::Bare);
terminal_impl_direct_from!(TranscodeTerminal, TUITerminalBackend, TranscodeTerminal::Fancy);

enumdispatch_impl_terminal!(TranscodeTerminal, TranscodeTerminal::Bare, TranscodeTerminal::Fancy);
enumdispatch_impl_log!(TranscodeTerminal, TranscodeTerminal::Bare, TranscodeTerminal::Fancy);
enumdispatch_impl_log_to_file!(TranscodeTerminal, TranscodeTerminal::Bare, TranscodeTerminal::Fancy);
enumdispatch_impl_user_controllable!(TranscodeTerminal, TranscodeTerminal::Bare, TranscodeTerminal::Fancy);
enumdispatch_impl_transcode!(TranscodeTerminal, TranscodeTerminal::Bare, TranscodeTerminal::Fancy);
