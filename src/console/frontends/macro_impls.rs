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
#[macro_export]
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
#[macro_export]
macro_rules! enumdispatch_impl_terminal {
    (
        lifetimes: $($lifetime: lifetime $(: $lifetime_bound: lifetime)?),+,
        TerminalBackend lifetimes: $($terminal_lifetime: lifetime $(:$terminal_lifetime_bound: lifetime)?),+,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime $(: $lifetime_bound)?),+> TerminalBackend<$($terminal_lifetime $(: $terminal_lifetime_bound)?),+> for $t {
            fn setup(
                &self,
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
#[macro_export]
macro_rules! enumdispatch_impl_log {
    (
        lifetimes: $($lifetime: lifetime),+,
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
#[macro_export]
macro_rules! enumdispatch_impl_log_to_file {
    (
        lifetimes: $($lifetime: lifetime $(: $lifetime_bound: lifetime)?),+,
        LogToFileBackend lifetimes: $($ltfb_lifetime: lifetime $(:$ltfb_lifetime_bound: lifetime)?),+,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime $(: $lifetime_bound)?),+> LogToFileBackend<$($ltfb_lifetime $(: $ltfb_lifetime_bound)?),+> for $t {
            fn enable_saving_logs_to_file<P: AsRef<Path>>(
                &self,
                log_file_path: P,
                scope: &'scope Scope<'scope, 'scope_env>
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.enable_saving_logs_to_file(log_file_path, scope)),+
                }
            }

            fn disable_saving_logs_to_file(&self) -> miette::Result<()> {
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
#[macro_export]
macro_rules! enumdispatch_impl_validation {
    (
        lifetimes: $($lifetime: lifetime),+,
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
#[macro_export]
macro_rules! enumdispatch_impl_transcode {
    (
        lifetimes: $($lifetime: lifetime),+,
        TranscodeBackend lifetimes: $transcode_lifetime: lifetime,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime),+> TranscodeBackend<$transcode_lifetime> for $t {
            /*
             * Album queue
             */
            fn queue_album_enable(&self) {
                match self {
                    $($variant(terminal) => terminal.queue_album_enable()),+
                }
            }

            fn queue_album_disable(&self) {
                match self {
                    $($variant(terminal) => terminal.queue_album_disable()),+
                }
            }

            fn queue_album_clear(&self) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_album_clear()),+
                }
            }

            fn queue_album_item_add(&self, item: AlbumQueueItem<$transcode_lifetime>) -> miette::Result<QueueItemID> {
                match self {
                    $($variant(terminal) => terminal.queue_album_item_add(item)),+
                }
            }

            fn queue_album_item_start(&self, item_id: QueueItemID) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_album_item_start(item_id)),+
                }
            }

            fn queue_album_item_finish(
                &self,
                item_id: QueueItemID,
                result: AlbumQueueItemFinishedResult,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_album_item_finish(item_id, result)),+
                }
            }

            fn queue_album_item_remove(&self, item_id: QueueItemID) -> miette::Result<AlbumQueueItem<$transcode_lifetime>> {
                match self {
                    $($variant(terminal) => terminal.queue_album_item_remove(item_id)),+
                }
            }

            /*
             * File queue
             */
            fn queue_file_enable(&self) {
                match self {
                    $($variant(terminal) => terminal.queue_file_enable()),+
                }
            }

            fn queue_file_disable(&self) {
                match self {
                    $($variant(terminal) => terminal.queue_file_disable()),+
                }
            }

            fn queue_file_clear(&self) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_file_clear()),+
                }
            }

            fn queue_file_item_add(&self, item: FileQueueItem<$transcode_lifetime>) -> miette::Result<QueueItemID> {
                match self {
                    $($variant(terminal) => terminal.queue_file_item_add(item)),+
                }
            }

            fn queue_file_item_start(&self, item_id: QueueItemID) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_file_item_start(item_id)),+
                }
            }

            fn queue_file_item_finish(
                &self,
                item_id: QueueItemID,
                result: FileQueueItemFinishedResult,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.queue_file_item_finish(item_id, result)),+
                }
            }

            fn queue_file_item_remove(&self, item_id: QueueItemID) -> miette::Result<FileQueueItem<$transcode_lifetime>> {
                match self {
                    $($variant(terminal) => terminal.queue_file_item_remove(item_id)),+
                }
            }

            /*
             * Progress
             */
            fn progress_enable(&self) {
                match self {
                    $($variant(terminal) => terminal.progress_enable()),+
                }
            }

            fn progress_disable(&self) {
                match self {
                    $($variant(terminal) => terminal.progress_disable()),+
                }
            }

            fn progress_set_total(&self, num_total: usize) -> miette::Result<()> {
                match self {
                    $($variant(terminal) => terminal.progress_set_total(num_total)),+
                }
            }

            fn progress_set_audio_files_currently_processing(
                &self,
                num_audio_files_currently_processing: usize,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) =>
                        terminal.progress_set_audio_files_currently_processing(num_audio_files_currently_processing)),+
                }
            }

            fn progress_set_data_files_currently_processing(
                &self,
                num_data_files_currently_processing: usize,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) =>
                        terminal.progress_set_data_files_currently_processing(num_data_files_currently_processing)),+
                }
            }

            fn progress_set_audio_files_finished_ok(
                &self,
                num_audio_files_finished_ok: usize,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) =>
                        terminal.progress_set_audio_files_finished_ok(num_audio_files_finished_ok)),+
                }
            }

            fn progress_set_data_files_finished_ok(
                &self,
                num_data_files_finished_ok: usize,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) =>
                        terminal.progress_set_data_files_finished_ok(num_data_files_finished_ok)),+
                }
            }

            fn progress_set_audio_files_errored(
                &self,
                num_audio_files_errored: usize,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) =>
                        terminal.progress_set_audio_files_errored(num_audio_files_errored)),+
                }
            }

            fn progress_set_data_files_errored(
                &self,
                num_data_files_errored: usize,
            ) -> miette::Result<()> {
                match self {
                    $($variant(terminal) =>
                        terminal.progress_set_data_files_errored(num_data_files_errored)),+
                }
            }
        }
    };
}

/// This macro implements [enum dispatching](https://docs.rs/enum_dispatch/latest/enum_dispatch/) behavior.
/// For more details, see the module documentation.
///
/// This macro implements the `UserControllableBackend` trait on the given enum's variants.
#[macro_export]
macro_rules! enumdispatch_impl_user_controllable {
    (
        lifetimes: $($lifetime: lifetime),+,
        on $t: ty,
        implement variants $($variant: path),+
    ) => {
        impl<$($lifetime),+> UserControllableBackend for $t {
            fn get_user_control_receiver(&self) -> miette::Result<tokio::sync::broadcast::Receiver<UserControlMessage>> {
                match self {
                    $($variant(terminal) => terminal.get_user_control_receiver()),+
                }
            }
        }
    };
}
