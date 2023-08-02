//! There are three sets of functionality that can be implemented for terminal frontends,
//! each of which makes them available for different commands (e.g. a UI backend that implements
//! everything we need for transcoding might not have everything we need for validation).
//!
//! All new backend implementations must be added as variants to each of the associated enum(s) (described below).
//!
//!
//!
//! ## Enums (types of frontends)
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
//! We basically add the concrete implementations of individual frontends as one enum variant each,
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
use std::fmt::{Debug, Formatter};
use std::path::Path;
use std::thread::Scope;

pub use bare::*;

use crate::console::frontends::shared::queue::{
    AlbumQueueItem,
    AlbumQueueItemFinishedResult,
    FileQueueItem,
    FileQueueItemFinishedResult,
    QueueItemID,
};
use crate::console::frontends::terminal_ui::terminal::FancyTerminalBackend;
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
use crate::{
    enumdispatch_impl_log,
    enumdispatch_impl_log_to_file,
    enumdispatch_impl_terminal,
    enumdispatch_impl_transcode,
    enumdispatch_impl_user_controllable,
    enumdispatch_impl_validation,
    terminal_impl_direct_from,
};

mod bare;
mod macro_impls;
pub mod shared;
pub mod terminal_ui;



#[allow(clippy::large_enum_variant)]
pub enum SimpleTerminal<'config: 'thread_scope, 'thread_scope> {
    Bare(BareTerminalBackend<'config>),
    Fancy(FancyTerminalBackend<'thread_scope, 'config>),
}

terminal_impl_direct_from!(
    on
        SimpleTerminal<'config, 'scope>,
    do conversions
        BareTerminalBackend<'config> => SimpleTerminal::Bare,
        FancyTerminalBackend<'scope, 'config> => SimpleTerminal::Fancy
);

enumdispatch_impl_terminal!(
    lifetimes: 'config, 'scope, 'scope_env: 'scope,
    TerminalBackend lifetimes: 'scope, 'scope_env,
    on
        SimpleTerminal<'config, 'scope>,
    implement variants
        SimpleTerminal::Bare,
        SimpleTerminal::Fancy
);
enumdispatch_impl_log!(
    lifetimes: 'config, 'scope,
    on
        SimpleTerminal<'config, 'scope>,
    implement variants
        SimpleTerminal::Bare,
        SimpleTerminal::Fancy
);
enumdispatch_impl_log_to_file!(
    lifetimes: 'config, 'scope, 'scope_env: 'scope,
    LogToFileBackend lifetimes: 'scope, 'scope_env,
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
    lifetimes: 'config, 'scope, 'scope_env: 'scope,
    TerminalBackend lifetimes: 'scope, 'scope_env,
    on
        ValidationTerminal<'config>,
    implement variants
        ValidationTerminal::Bare
);
enumdispatch_impl_log!(
    lifetimes: 'config,
    on
        ValidationTerminal<'config>,
    implement variants
        ValidationTerminal::Bare
);
enumdispatch_impl_log_to_file!(
    lifetimes: 'config, 'scope, 'scope_env: 'scope,
    LogToFileBackend lifetimes: 'scope, 'scope_env,
    on
        ValidationTerminal<'config>,
    implement variants
        ValidationTerminal::Bare
);
enumdispatch_impl_validation!(
    lifetimes: 'config,
    on
        ValidationTerminal<'config>,
    implement variants
        ValidationTerminal::Bare
);



pub enum TranscodeTerminal<'config, 'scope> {
    Bare(BareTerminalBackend<'config>),
    Fancy(FancyTerminalBackend<'scope, 'config>),
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
        FancyTerminalBackend<'scope, 'config> => TranscodeTerminal::Fancy
);

enumdispatch_impl_terminal!(
    lifetimes: 'config: 'scope, 'scope, 'scope_env: 'scope,
    TerminalBackend lifetimes: 'scope, 'scope_env,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
enumdispatch_impl_log!(
    lifetimes: 'config, 'scope,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
enumdispatch_impl_log_to_file!(
    lifetimes: 'config: 'scope, 'scope, 'scope_env: 'scope,
    LogToFileBackend lifetimes: 'scope, 'scope_env,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
enumdispatch_impl_user_controllable!(
    lifetimes: 'config, 'scope,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
enumdispatch_impl_transcode!(
    lifetimes: 'config, 'scope,
    TranscodeBackend lifetimes: 'config,
    on
        TranscodeTerminal<'config, 'scope>,
    implement variants
        TranscodeTerminal::Bare,
        TranscodeTerminal::Fancy
);
