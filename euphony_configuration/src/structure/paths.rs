use serde::Deserialize;

use crate::{
    traits::ResolvableConfiguration,
    utilities::get_running_executable_directory,
};

/// Base paths - reusable values such as the base library path and base tools path.
#[derive(Clone)]
pub struct PathsConfiguration {
    pub base_library_path: String,
    pub base_tools_path: String,
}

#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedPathsConfiguration {
    base_library_path: String,
    base_tools_path: String,
}


impl ResolvableConfiguration for UnresolvedPathsConfiguration {
    type Resolved = PathsConfiguration;

    fn resolve(self) -> miette::Result<Self::Resolved> {
        // Replaces any placeholders and validates the paths.
        let executable_directory = get_running_executable_directory()?
            .to_string_lossy()
            .to_string();

        let base_library_path = self
            .base_library_path
            .replace("{SELF}", &executable_directory);
        let base_tools_path = self
            .base_tools_path
            .replace("{SELF}", &executable_directory);

        let base_library_path = dunce::canonicalize(base_library_path)
            .unwrap_or_else(|_| panic!(
                "Could not canonicalize base_library_path \"{}\", make sure it exists.",
                self.base_library_path,
            ))
            .to_string_lossy()
            .to_string();

        let base_tools_path = dunce::canonicalize(base_tools_path)
            .unwrap_or_else(|_| panic!(
                "Could not canonicalize base_tools_path \"{}\", make sure it exists.",
                self.base_tools_path,
            ))
            .to_string_lossy()
            .to_string();


        Ok(PathsConfiguration {
            base_library_path,
            base_tools_path,
        })
    }
}
