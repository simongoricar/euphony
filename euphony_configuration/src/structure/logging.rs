use std::path::PathBuf;

use chrono::Local;
use serde::Deserialize;

use crate::{
    paths::PathsConfiguration,
    traits::ResolvableWithPathsConfiguration,
    utilities::get_running_executable_directory,
};


#[derive(Clone)]
pub struct LoggingConfiguration {
    pub default_log_output_path: Option<PathBuf>,
}


#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedLoggingConfiguration {
    default_log_output_path: Option<PathBuf>,
}


impl ResolvableWithPathsConfiguration for UnresolvedLoggingConfiguration {
    type Resolved = LoggingConfiguration;

    fn resolve(
        self,
        paths: &PathsConfiguration,
    ) -> miette::Result<Self::Resolved> {
        let executable_directory = get_running_executable_directory()?
            .to_string_lossy()
            .to_string();

        let time_now = Local::now();
        let formatted_time_now = time_now.format("%Y-%m-%d_%H-%M-%S");

        let default_log_output_path =
            self.default_log_output_path.as_ref().map(|output_path| {
                let path_as_string = output_path
                    .to_string_lossy()
                    .to_string()
                    .replace("{LIBRARY_BASE}", &paths.base_library_path)
                    .replace("{SELF}", &executable_directory)
                    .replace("{DATETIME}", &formatted_time_now.to_string());

                PathBuf::from(path_as_string)
            });

        Ok(LoggingConfiguration {
            default_log_output_path,
        })
    }
}
