1.1.0
- Added: --verbose switch (transcode commands show real-time debug logs for each file when enabled).
- Added: `max_processing_retries` and `processing_retry_delay_seconds` configuration values to allow for retrying failed files.
- Changed: transcoding UIs now look more consistent across commands.
- Fixed: album scans no longer incorrectly include subdirectories.
- Fixed: not all transcode commands were being paralellized.
- Fixed: long file names no longer break the progress bar.

1.0.0
Initial "stable-enough-for-a-personal-project" release.
Contains features for validation and transcoding of the libraries.
