use std::path::Path;

use miette::Result;
use serde::Deserialize;

use crate::{
    filesystem::get_path_extension_or_empty,
    paths::PathsConfiguration,
    traits::ResolvableWithPathsConfiguration,
};



#[derive(Clone)]
pub struct ToolsConfiguration {
    pub ffmpeg: FfmpegToolsConfiguration,
}

#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedToolsConfiguration {
    ffmpeg: UnresolvedFfmpegToolsConfiguration,
}

impl ResolvableWithPathsConfiguration for UnresolvedToolsConfiguration {
    type Resolved = ToolsConfiguration;

    fn resolve(
        self,
        paths: &PathsConfiguration,
    ) -> miette::Result<Self::Resolved> {
        Ok(ToolsConfiguration {
            ffmpeg: self.ffmpeg.resolve(paths)?,
        })
    }
}



#[derive(Clone)]
pub struct FfmpegToolsConfiguration {
    /// Configures the ffmpeg binary location.
    /// The {TOOLS_BASE} placeholder is available (see `base_tools_path` in the `essentials` table)
    pub binary: String,

    /// These are the arguments passed to ffmpeg when converting an audio file into MP3 V0.
    /// The placeholders {INPUT_FILE} and {OUTPUT_FILE} will be replaced with the absolute path to those files.
    pub audio_transcoding_args: Vec<String>,

    /// This setting should be the extension of the audio files after transcoding.
    /// The default conversion is to MP3, but the user may set any ffmpeg conversion above, which is why this exists.
    pub audio_transcoding_output_extension: String,
}

impl FfmpegToolsConfiguration {
    /// Returns `Ok(true)` if the given path's extension matches
    /// the ffmpeg transcoding output path.
    ///
    /// Returns `Err` if the extension is not valid UTF-8.
    pub fn is_path_transcoding_output_by_extension<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<bool> {
        let extension = get_path_extension_or_empty(file_path)?;

        Ok(self.audio_transcoding_output_extension.eq(&extension))
    }
}

#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedFfmpegToolsConfiguration {
    binary: String,

    audio_transcoding_args: Vec<String>,

    audio_transcoding_output_extension: String,
}

impl ResolvableWithPathsConfiguration for UnresolvedFfmpegToolsConfiguration {
    type Resolved = FfmpegToolsConfiguration;

    fn resolve(
        self,
        paths: &PathsConfiguration,
    ) -> miette::Result<Self::Resolved> {
        let ffmpeg = self.binary.replace("{TOOLS_BASE}", &paths.base_tools_path);

        let canonicalized_ffmpeg = dunce::canonicalize(ffmpeg.clone())
            .unwrap_or_else(|_| panic!(
                "Could not canonicalize ffmpeg binary path: \"{ffmpeg}\", make sure the path is valid.",
            ));

        let binary = canonicalized_ffmpeg.to_string_lossy().to_string();

        if !canonicalized_ffmpeg.is_file() {
            panic!("No file exists at this path: {}", self.binary);
        }

        let audio_transcoding_output_extension =
            self.audio_transcoding_output_extension.to_ascii_lowercase();

        Ok(FfmpegToolsConfiguration {
            binary,
            audio_transcoding_args: self.audio_transcoding_args,
            audio_transcoding_output_extension,
        })
    }
}
