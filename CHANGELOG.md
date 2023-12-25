# Changelog
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).



## [Unreleased]


---

## [2.1.1] - 2023-12-25

### Changed
- A significant amount of the code has been rearranged into two new crates 
  (`euphony_configuration` and `euphony_library`) 
  to allow for reusability and integration with other projects.

### Fixed
- Fixed log file creation error when using the bare terminal backend.



## [2.1.0] - 2023-08-07

### Added
- Ability to create distinct (timestamped) log files by using the `{DATETIME}` placeholder in the `logging.default_log_output_path` configuration option.
- When the user cancels a transcoding operation, partially transcoded files will be deleted and the user will be warned that the album didn't complete.
- Display validity of library paths in the `show-config` command.

### Changed
- Improved placeholder (`{LIBRARY_BASE}`, `{DATETIME}`, etc.) documentation in the configuration file template (`data/configuration.TEMPLATE.toml`).

### Fixed
- Album and file queue now properly collapse leading finished items when there is not enough space to display all queue items.
- Create any missing log file path parent directories (`default_log_output_path` / `--log-to-file`).



## [2.0.0] - 2023-08-06
**This version includes breaking changes.**

### Added
- Now using [`rustfmt`](https://github.com/rust-lang/rustfmt) as the code formatter for this project. Not perfect, but a good way to introduce deterministic formatting for now.
- `euphony` is now able to detect fully removed albums or even entire artists. This was previously a not insignificant limitation, but with this change `euphony transcode` should now replicate any changes you make in your source libraries, including album and artist deletion.

### Changed
- Configuration file structure has been rewritten and is **incompatible with previous versions**.
- Now using [`ratatui`](https://github.com/ratatui-org/ratatui) as the terminal UI library as `tui` is currently unmaintained.
- The fancy terminal UI has received some UI improvements, including a separate view for transcoding progress and log output.
- Log output is now timestamped both in the live log view and the configurable log file output.
- Internals:
  - Libraries, artists and individual albums have been abstracted into *views*, making code cleaner and more modular. This will also make adding support for library structures other than `<Library>/<Artist>/<Album>` easier to implement in the future, if that comes up.
  - A lot of other internal systems have been rewritten to be more efficient, easier to reason about and maintain.


## [1.3.1] - 2023-01-29

### Changed
- Under-the-hood optimizations: instead of [trait objects](https://doc.rust-lang.org/book/ch17-02-trait-objects.html) 
  the backends now use *enum dispatching* (like the [enum_dispatch](https://docs.rs/enum_dispatch/latest/enum_dispatch/) crate does it, but in-house). 
  This gives better performance, but more importantly we are no longer limited to [object safety](https://doc.rust-lang.org/reference/items/traits.html#object-safety), 
  meaning we can now use generics and other features.
- Project dependencies have been updated to the latest versions.


## [1.3.0] - 2023-01-10

### Added
- `validate` command has been rewritten and will now work properly again. Its visual elements have been overhauled as well 
   and errors will now display as a proper flattened error list (instead of error reporting being done in multiple confusing stages).

### Changed
- `validate` and `transcode` commands have been aliased into `validate-collection` and `transcode-collection`.
- Several code optimizations that now pass around references instead of clones where possible.


## [1.2.0] - 2022-12-12

### Added
- Implemented a new "terminal backend" system that allows for multiple UI implementations on the same feature set (without the individual commands having to know the backend details).
- As part of the new terminal backend system: 
  - Transcoding: a completely new dynamic terminal UI (default - no option needed), built with [`tui`](https://docs.rs/tui/latest/tui/) - a much better and fancier visualization of the processing being done.
  - Transcoding: a bare-bones simple terminal output mode is available (`euphony transcode --bare-terminal`) for environments in which a constantly-updating terminal UI is unwelcome.
  - Other commands: rewritten display layout and colours.

### Changed
- Renamed `transcode-all` to `transcode` and `validate-all` to `validate` for simplicity now that we don't have multiple transcoding commands.

### Removed
- As part of the UI rewrite, the barely-used transcoding commands are gone (`transcode-library`, `transcode-album`, `validate-library`).

### Fixed
- `transcode` command now respects the `ignored_directories_in_base_dir` option.
- `show-config` now also shows the previously-missing `ignored_directories_in_base_dir` option for each library.


## [1.1.0] - 2022-07-26

### Added
- --verbose switch (transcode commands show real-time debug logs for each file when enabled).
- `max_processing_retries` and `processing_retry_delay_seconds` configuration values to allow for retrying failed files.
- Optional album-specific configuration file named `.album.override.euphony` that can influence an album's scanning and transcoding.

### Changed
- Transcoding UIs now look more consistent across commands.

### Fixed
- Album scans now no longer incorrectly include subdirectories.
- All transcode commands are now properly paralellized.
- Long file names no longer break the progress bar.


## [1.0.0] - 2022-07-23

*Initial "stable-enough-for-a-personal-project" release.*

Contains features for validation and transcoding of the libraries 
(more general info available in the [README.md](https://github.com/DefaultSimon/euphony/blob/0cb64bc5864b89e52c2d5e7ee474bb6ccf2141e2/README.md) at this tag).



[Unreleased]: https://github.com/DefaultSimon/euphony/compare/v2.1.0...HEAD
[2.1.1]: https://github.com/DefaultSimon/euphony/compare/v2.1.0...v2.1.1
[2.1.0]: https://github.com/DefaultSimon/euphony/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/DefaultSimon/euphony/compare/v1.3.1...v2.0.0
[1.3.1]: https://github.com/DefaultSimon/euphony/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/DefaultSimon/euphony/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/DefaultSimon/euphony/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/DefaultSimon/euphony/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/DefaultSimon/euphony/compare/93d88c4fdbbdf40697cc50e97c92366e02d84e15...v1.0.0
