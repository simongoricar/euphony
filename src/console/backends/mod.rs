//! There are three sets of functionality that can be implemented for terminal backends,
//! each of which makes them available for different commands (e.g. a UI backend that implements
//! everything we need for transcoding might not have everything we need for validation).
//!
//! All new backend implementations must be added as variants to each of the associated enum(s) (described below).
//!
//!
//!
//! ## Enums (types of backends)
//!
//! **The first is `SimpleTerminal`** (`TerminalTrait` + `LogBackend` + `LogToFileBackend` traits).
//!
//! Backends that implement these three traits and are added as a variant to `SimpleTerminal` can
//! be used for the following commands:
//! - `show-config`
//! - `list-libraries`
//!
//! Both `BareTerminalBackend` and `TUITerminalBackend` are available here.
//!
//! ---
//!
//! **The second is `ValidationTerminal`** (`TerminalTrait` + `LogBackend` + `LogToFileBackend`
//! + `ValidationBackend` traits).
//!
//! Backends that implement those four traits and are added as a variant to `ValidationTerminal` can be used
//! for the following commands:
//! - `validate`
//!
//! Only the `BareTerminalBackend` is available here for the moment.
//!
//! ---
//!
//! **The third and last is `TranscodeTerminal`** (`TerminalTrait` + `LogBackend` + `LogToFileBackend` +
//! `TranscodeBackend` + `UserControllableBackend` traits).
//!
//! Backends that implement those five traits and
//! are added a variant to `TranscodeTerminal` can be used for the following commands:
//! - `transcode`
//!
//! Both `BareTerminalBackend` and `TUITerminalBackend` are available here.
//!
//!
//!
//! ## Implementation details
//!
//! > *The previous approach to this was to use `dyn` dispatching / trait objects. This is a limited
//! (can't use generics and many other things) and slow performance-wise.*
//!
//! The technique in use here is **enum dispatching**, similar to what is used in the
//! [enum_dispatch](https://docs.rs/enum_dispatch) crate.
//! We basically add the concrete implementations of individual backends as one enum variant each,
//! then implement the relevant traits they implement on the enum itself. In those implementations
//! we forward the calls to each variant by using a `match` statement.
//!
//! To reduce code repetition, a set of `enumdispatch_*` macros are available below.
//!
//! ### Usage example
//!
//! Let's say we have the following enum:
//!
//! ```
//! enum MyEnum {
//!     VariantOne(SomeBackend),
//!     VariantTwo(SomeBackendTwo),
//! }
//! ```
//!
//! where `SomeBackend` and `SomeBackendTwo` are structs that both implement `TerminalBackend`,
//! which is a base building block for the terminal backend system.
//!
//! Now `MyEnum` can only contain the implementors of `TerminalBackend`, but we can't call
//! e.g. `setup()` on the enum instance itself, because it doesn't implement it (yet). We could
//! implement each trait by hand, add all match statements, ..., but that would be a lot of repetition.
//!
//! This is where enum dispatch and the macros come in.
//!
//! Calling `enumdispatch_impl_terminal!(MyEnum, MyEnum::VariantOne, MyEnum::VariantTwo)` will
//! expand to the following:
//!
//! ```
//! impl TerminalBackend for MyEnum {
//!     fn setup(&mut self) -> Result<()> {
//!         match self {
//!              MyEnum::VariantOne(terminal) => terminal.setup(),
//!              MyEnum::VariantTwo(terminal) => terminal.setup(),
//!         }
//!     }
//!
//!     fn destroy(&mut self) -> Result<()> {
//!         match self {
//!              MyEnum::VariantOne(terminal) => terminal.destroy(),
//!              MyEnum::VariantTwo(terminal) => terminal.destroy(),
//!         }
//!     }
//! }
//! ```
//!
//! And that's it! Now we can simply do:
//!
//! ```
//! // Puts `SomeBackend` into the enum (in practice this could be one of many backend implementations
//! // being put into one of many enum variants). See `terminal_impl_direct_from` for a better approach.
//! let backend = MyEnum::VariantOne(some_backend_instance);
//!
//! backend.setup()?;
//! // ... use backend ...
//! backend.destroy()?;
//! ```
//!
//! and the `setup()` and `destroy()` calls will be passed onto the relevant struct inside
//! (and even at a performance boost, compared to `dyn` dispatch).
//!
//! *NOTE:* The macros are variadic, meaning that you can pass in any number of enum variants you have
//! (in practice, pass in all of them, otherwise the code will not compile as the match won't be exhausted).
//!

use std::fmt::Display;
use std::path::PathBuf;

pub use bare::*;
use crossbeam::channel::Receiver;
pub use fancy::*;

use crate::console::backends::shared::{QueueItem, QueueItemID, QueueType};
use crate::console::{
    LogBackend,
    LogToFileBackend,
    TerminalBackend,
    TranscodeBackend,
    UserControlMessage,
    UserControllableBackend,
    ValidationBackend,
    ValidationErrorInfo,
};

mod bare;
mod fancy;
pub mod shared;


/// This macro implements `From` on the given enum
/// by directly constructing the given variant(s) from the given terminal backend.
///
/// ## Usage
///
/// The first argument is the enum you want to implement this for.
/// The rest are variadic (must be at least one). Each is delimited by a comma
/// and the format is `YourStruct => EnumVariantItFitsIn`.
///
/// *I'd recommend reading the example below.*
///
/// ## Example
///
/// Let's say we have the following enum:
///
/// ```
/// enum SimpleTerminal {
///     Bare(BareTerminalBackend),
///     Fancy(TUITerminalBackend),
/// }
/// ```
///
/// And we want to use this macro to be able to call `.into()` on a `BareTerminalBackend` and have it
/// convert into a `SimpleTerminal`. We would call the macro like this:
///
/// ```
/// terminal_impl_direct_from!(
///     SimpleTerminal,
///     BareTerminalBackend => SimpleTerminal::Bare,
///     TUITerminalBackend => SimpleTerminal::Fancy
/// );
/// ```
///
/// which would expand to the following simple (but repetitive) implementation.
///
/// ```
/// impl From<BareTerminalBackend> for SimpleTerminal {
///     fn from(item: BareTerminalBackend) -> Self {
///         SimpleTerminal::Bare(item)
///     }
/// }
/// impl From<TUITerminalBackend> for SimpleTerminal {
///     fn from(item: TUITerminalBackend) -> Self {
///         SimpleTerminal::Fancy(item)
///     }
/// }
/// ```
///
/// We can now perform simple `.into()`s in our code instead of manual conversion:
///
/// ```
/// let simple_terminal: SimpleTerminal = BareTerminalBackend::new().into();
/// ```
///
///
macro_rules! terminal_impl_direct_from {
    ($target: path, $($source_backend: path => $target_variant: path),+) => {
        $(
            impl From<$source_backend> for $target {
                fn from(item: $source_backend) -> Self {
                    $target_variant(item)
                }
            }
        )+
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

terminal_impl_direct_from!(
    SimpleTerminal,
    BareTerminalBackend => SimpleTerminal::Bare,
    TUITerminalBackend => SimpleTerminal::Fancy
);

enumdispatch_impl_terminal!(
    SimpleTerminal,
    SimpleTerminal::Bare,
    SimpleTerminal::Fancy
);
enumdispatch_impl_log!(
    SimpleTerminal,
    SimpleTerminal::Bare,
    SimpleTerminal::Fancy
);
enumdispatch_impl_log_to_file!(
    SimpleTerminal,
    SimpleTerminal::Bare,
    SimpleTerminal::Fancy
);


pub enum ValidationTerminal {
    Bare(BareTerminalBackend),
}

terminal_impl_direct_from!(
    ValidationTerminal,
    BareTerminalBackend => ValidationTerminal::Bare
);

enumdispatch_impl_terminal!(ValidationTerminal, ValidationTerminal::Bare);
enumdispatch_impl_log!(ValidationTerminal, ValidationTerminal::Bare);
enumdispatch_impl_log_to_file!(ValidationTerminal, ValidationTerminal::Bare);
enumdispatch_impl_validation!(ValidationTerminal, ValidationTerminal::Bare);


#[allow(clippy::large_enum_variant)]
pub enum TranscodeTerminal {
    Bare(BareTerminalBackend),
    Fancy(TUITerminalBackend),
}

terminal_impl_direct_from!(
    TranscodeTerminal,
    BareTerminalBackend => TranscodeTerminal::Bare,
    TUITerminalBackend => TranscodeTerminal::Fancy
);

enumdispatch_impl_terminal!(
    TranscodeTerminal,
    TranscodeTerminal::Bare,
    TranscodeTerminal::Fancy
);
enumdispatch_impl_log!(
    TranscodeTerminal,
    TranscodeTerminal::Bare,
    TranscodeTerminal::Fancy
);
enumdispatch_impl_log_to_file!(
    TranscodeTerminal,
    TranscodeTerminal::Bare,
    TranscodeTerminal::Fancy
);
enumdispatch_impl_user_controllable!(
    TranscodeTerminal,
    TranscodeTerminal::Bare,
    TranscodeTerminal::Fancy
);
enumdispatch_impl_transcode!(
    TranscodeTerminal,
    TranscodeTerminal::Bare,
    TranscodeTerminal::Fancy
);
