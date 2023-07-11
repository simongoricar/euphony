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

use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;
use std::thread::Scope;

pub use bare::*;
use crossbeam::channel::Receiver;
pub use fancy::*;
use shared::queue_v2::{
    AlbumItem,
    AlbumItemFinishedResult,
    FileItem,
    FileItemFinishedResult,
    QueueItemID,
};

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
/// The rest are variadic (but there must be at least one). Each is delimited by a comma
/// and the format is `YourStruct => EnumVariantItFitsIn`.
///
/// `'config` and `'scope` lifetimes are available.
///
/// *Read the example below.*
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
/// terminal_impl_direct_from_with_lifetime!(
///     on
///         SimpleTerminal<'config, 'scope>,
///     dispatch
///         BareTerminalBackend<'config> => SimpleTerminal::Bare,
///         TUITerminalBackend<'config, 'scope> => SimpleTerminal::Fancy
/// );
/// ```
///
/// which would expand to the following simple (but repetitive if we'd done it by hand) implementation:
///
/// ```
/// impl<'config, 'scope> From<BareTerminalBackend<'config>> for SimpleTerminal<'config, 'scope> {
///     fn from(item: BareTerminalBackend<'config>) -> Self {
///         SimpleTerminal::Bare(item)
///     }
/// }
/// impl<'config, 'scope> From<TUITerminalBackend<'config, 'scope>> for SimpleTerminal<'config, 'scope> {
///     fn from(item: TUITerminalBackend<'config, 'scope>) -> Self {
///         SimpleTerminal::Fancy
///             (item)
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
    (
        on $implementation_target: ty,
        do conversions $($backend: ty => $target_variant: path),+
    ) => {
        $(
            impl<'config, 'scope> From<$backend> for $implementation_target {
                fn from(item: $backend) -> Self {
                    $target_variant(item)
                }
            }
        )+
    };
}


/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
/// For more details, see the module documentation.
///
/// This macro implements the `TerminalBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_terminal {
    (
        lifetimes are $($lifetime: lifetime $(: $lifetime_bound: lifetime)?),+,
        TerminalBackend lifetime is $($terminal_lifetime: lifetime $(:$terminal_lifetime_bound: lifetime)?),+,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime $(: $lifetime_bound)?),+> TerminalBackend<$($terminal_lifetime $(: $terminal_lifetime_bound)?),+> for $t {
            fn setup(
                &mut self,
                scope: &'scope Scope<'scope, 'scope_env>
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.setup(scope)),+
                }
            }

            fn destroy(self) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.destroy()),+
                }
            }
        }
    };
}

/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
/// For more details, see the module documentation.
///
/// This macro implements the `LogBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_log {
    (
        lifetimes are $($lifetime: lifetime),+,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime),+> LogBackend for $t {
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
/// For more details, see the module documentation.
///
/// This macro implements the `LogToFileBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_log_to_file {
    (
        lifetimes are $($lifetime: lifetime),+,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime),+> LogToFileBackend for $t {
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
/// For more details, see the module documentation.
///
/// This macro implements the `ValidationBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_validation {
    (
        lifetimes are $($lifetime: lifetime),+,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime),+> ValidationBackend for $t {
            fn validation_add_error(&self, error: ValidationErrorInfo) {
                match self {
                    $($variant(terminal) => terminal.validation_add_error(error)),+
                }
            }
        }
    };
}

/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
/// For more details, see the module documentation.
///
/// This macro implements the `TranscodeBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_transcode {
    (
        lifetimes are $($lifetime: lifetime),+,
        TranscodeBackend lifetime is $transcode_lifetime: lifetime,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime),+> TranscodeBackend<$transcode_lifetime> for $t {
            /*
             * Album queue
             */
            fn queue_album_enable(&mut self) {
                match self {
                    $($variant(terminal) => terminal.queue_album_enable()),+
                }
            }

            fn queue_album_disable(&mut self) {
                match self {
                    $($variant(terminal) => terminal.queue_album_disable()),+
                }
            }

            fn queue_album_clear(&mut self) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_album_clear()),+
                }
            }

            fn queue_album_item_add(&mut self, item: AlbumItem<$transcode_lifetime>) -> miette::Result<QueueItemID> {
                match self {
                    $($variant(terminal) => terminal.queue_album_item_add(item)),+
                }
            }

            fn queue_album_item_start(&mut self, item_id: QueueItemID) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_album_item_start(item_id)),+
                }
            }

            fn queue_album_item_finish(
                &mut self,
                item_id: QueueItemID,
                result: AlbumItemFinishedResult,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_album_item_finish(item_id, result)),+
                }
            }

            fn queue_album_item_remove(&mut self, item_id: QueueItemID) -> miette::Result<AlbumItem<$transcode_lifetime>> {
                match self {
                    $($variant(terminal) => terminal.queue_album_item_remove(item_id)),+
                }
            }

            /*
             * File queue
             */
            fn queue_file_enable(&mut self) {
                match self {
                    $($variant(terminal) => terminal.queue_file_enable()),+
                }
            }

            fn queue_file_disable(&mut self) {
                match self {
                    $($variant(terminal) => terminal.queue_file_disable()),+
                }
            }

            fn queue_file_clear(&mut self) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_file_clear()),+
                }
            }

            fn queue_file_item_add(&mut self, item: FileItem<$transcode_lifetime>) -> miette::Result<QueueItemID> {
                match self {
                    $($variant(terminal) => terminal.queue_file_item_add(item)),+
                }
            }

            fn queue_file_item_start(&mut self, item_id: QueueItemID) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_file_item_start(item_id)),+
                }
            }

            fn queue_file_item_finish(
                &mut self,
                item_id: QueueItemID,
                result: FileItemFinishedResult,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_file_item_finish(item_id, result)),+
                }
            }

            fn queue_file_item_remove(&mut self, item_id: QueueItemID) -> miette::Result<FileItem<$transcode_lifetime>> {
                match self {
                    $($variant(terminal) => terminal.queue_file_item_remove(item_id)),+
                }
            }

            /*
             * Progress
             */
            fn progress_enable(&mut self) {
                match self {
                    $($variant(terminal) => terminal.progress_enable()),+
                }
            }

            fn progress_disable(&mut self) {
                match self {
                    $($variant(terminal) => terminal.progress_disable()),+
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
/// For more details, see the module documentation.
///
/// This macro implements the `UserControllableBackend` trait on the given enum's variants.
macro_rules! enumdispatch_impl_user_controllable {
    (
        lifetimes are $($lifetime: lifetime),+,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime),+> UserControllableBackend for $t {
            fn get_user_control_receiver(&mut self) -> miette::Result<Receiver<UserControlMessage>> {
                match self {
                    $($variant(terminal) => terminal.get_user_control_receiver()),+
                }
            }
        }
    };
}

#[allow(clippy::large_enum_variant)]
pub enum SimpleTerminal<'config: 'scope, 'scope> {
    Bare(BareTerminalBackend<'config>),
    Fancy(TUITerminalBackend<'config, 'scope>),
}

terminal_impl_direct_from!(
    on
        SimpleTerminal<'config, 'scope>,
    do conversions
        BareTerminalBackend<'config> => SimpleTerminal::Bare,
        TUITerminalBackend<'config, 'scope> => SimpleTerminal::Fancy
);

enumdispatch_impl_terminal!(
    lifetimes are 'config, 'scope, 'scope_env: 'scope,
    TerminalBackend lifetime is 'scope, 'scope_env,
    on
        SimpleTerminal<'config, 'scope>,
    implement variants
        SimpleTerminal::Bare,
        SimpleTerminal::Fancy
);
enumdispatch_impl_log!(
    lifetimes are 'config, 'scope,
    on
        SimpleTerminal<'config, 'scope>,
    implement variants
        SimpleTerminal::Bare,
        SimpleTerminal::Fancy
);
enumdispatch_impl_log_to_file!(
    lifetimes are 'config, 'scope,
    on
        SimpleTerminal<'config, 'scope>,
    implement variants
        SimpleTerminal::Bare,
        SimpleTerminal::Fancy
);


pub enum ValidationTerminal<'config> {
    Bare(BareTerminalBackend<'config>),
}

terminal_impl_direct_from!(
    on
        ValidationTerminal<'config>,
    do conversions
        BareTerminalBackend<'config> => ValidationTerminal::Bare
);

enumdispatch_impl_terminal!(
    lifetimes are 'config, 'scope, 'scope_env: 'scope,
    TerminalBackend lifetime is 'scope, 'scope_env,
    on
        ValidationTerminal<'config>,
    implement variants
        ValidationTerminal::Bare
);
enumdispatch_impl_log!(
    lifetimes are 'config,
    on
        ValidationTerminal<'config>,
    implement variants
        ValidationTerminal::Bare
);
enumdispatch_impl_log_to_file!(
    lifetimes are 'config,
    on
        ValidationTerminal<'config>,
    implement variants
        ValidationTerminal::Bare
);
enumdispatch_impl_validation!(
    lifetimes are 'config,
    on
        ValidationTerminal<'config>,
    implement variants
        ValidationTerminal::Bare
);


pub enum TranscodeTerminal<'config, 'scope> {
    Bare(BareTerminalBackend<'config>),
    Fancy(TUITerminalBackend<'config, 'scope>),
}

impl<'config: 'scope, 'scope> Debug for TranscodeTerminal<'config, 'scope> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "TranscodeTerminal")
    }
}

terminal_impl_direct_from!(
    on
        TranscodeTerminal<'config, 'scope>,
    do conversions
        BareTerminalBackend<'config> => TranscodeTerminal::Bare,
        TUITerminalBackend<'config, 'scope> => TranscodeTerminal::Fancy
);

enumdispatch_impl_terminal!(
    lifetimes are 'config, 'scope, 'scope_env: 'scope,
    TerminalBackend lifetime is 'scope, 'scope_env,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
enumdispatch_impl_log!(
    lifetimes are 'config, 'scope,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
enumdispatch_impl_log_to_file!(
    lifetimes are 'config, 'scope,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
enumdispatch_impl_user_controllable!(
    lifetimes are 'config, 'scope,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
enumdispatch_impl_transcode!(
    lifetimes are 'config, 'scope,
    TranscodeBackend lifetime is 'config,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
