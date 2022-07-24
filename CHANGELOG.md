1.1.0
- Fixed: not all transcode commands were being paralellized.
- Fixed: long file names no longer break the progress bar.
- Changed: transcoding UIs now look more consistent across commands.
- Added: --verbose switch (transcode commands show debug logs for each album when enabled).
- Added: `max_processing_retries` and `processing_retry_delay_seconds` configuration values to allow for retrying failed files.

1.0.0
Initial "stable-enough-for-a-personal-project" release.
Contains features for validation and transcoding of the libraries.
