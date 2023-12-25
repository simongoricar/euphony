//! Module containing the entire configuration structure for
//! the main euphony configuration.

pub mod aggregated_library;
pub mod library;
pub mod logging;
pub mod paths;
pub mod tools;
pub mod ui;
pub mod validation;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use miette::{miette, Context, Result};
use serde::Deserialize;

use crate::aggregated_library::{
    AggregatedLibraryConfiguration,
    UnresolvedAggregatedLibraryConfiguration,
};
use crate::library::{LibraryConfiguration, UnresolvedLibraryConfiguration};
use crate::logging::{LoggingConfiguration, UnresolvedLoggingConfiguration};
use crate::paths::{PathsConfiguration, UnresolvedPathsConfiguration};
use crate::tools::{ToolsConfiguration, UnresolvedToolsConfiguration};
use crate::traits::{
    ResolvableConfiguration,
    ResolvableWithContextConfiguration,
    ResolvableWithPathsConfiguration,
};
use crate::ui::{UiConfiguration, UnresolvedUiConfiguration};
use crate::utilities::get_default_configuration_file_path;
use crate::validation::{
    UnresolvedValidationConfiguration,
    ValidationConfiguration,
};

/// This struct contains the entire `euphony` configuration,
/// from tool paths to libraries and so forth.
#[derive(Clone)]
pub struct Configuration {
    pub paths: PathsConfiguration,

    pub logging: LoggingConfiguration,

    pub ui: UiConfiguration,

    pub validation: ValidationConfiguration,

    pub tools: ToolsConfiguration,

    pub libraries: BTreeMap<String, LibraryConfiguration>,

    // TODO Should I rename "aggregated library" to something else, like "transcoded library"?
    pub aggregated_library: AggregatedLibraryConfiguration,

    pub configuration_file_path: PathBuf,
}

#[derive(Deserialize, Clone)]
struct UnresolvedConfiguration {
    paths: UnresolvedPathsConfiguration,

    logging: UnresolvedLoggingConfiguration,

    ui: UnresolvedUiConfiguration,

    validation: UnresolvedValidationConfiguration,

    tools: UnresolvedToolsConfiguration,

    libraries: BTreeMap<String, UnresolvedLibraryConfiguration>,

    aggregated_library: UnresolvedAggregatedLibraryConfiguration,
}

#[allow(dead_code)]
impl Configuration {
    pub fn load_from_path<S: Into<PathBuf>>(
        configuration_filepath: S,
    ) -> Result<Configuration> {
        let configuration_filepath = configuration_filepath.into();

        // Read the configuration file into memory.
        let configuration_string = fs::read_to_string(&configuration_filepath)
            .expect("Could not read configuration file!");

        // Parse the string into the `Config` structure.
        let unresolved_configuration: UnresolvedConfiguration =
            toml::from_str(&configuration_string)
                .expect("Could not load configuration file!");

        let configuration_file_path = dunce::canonicalize(configuration_filepath)
            .expect("Could not canonicalize configuration file path even though it has loaded!");


        // Resolve the configuration into its final state.
        let resolved_configuration =
            unresolved_configuration.resolve(configuration_file_path)?;

        Ok(resolved_configuration)
    }

    pub fn load_default_path() -> Result<Configuration> {
        Configuration::load_from_path(
            get_default_configuration_file_path().wrap_err_with(|| {
                miette!("Could not get default configuration file path.")
            })?,
        )
    }

    pub fn is_library<P: AsRef<Path>>(&self, library_path: P) -> bool {
        for library in self.libraries.values() {
            let current_path = Path::new(&library.path);
            if current_path.eq(library_path.as_ref()) {
                return true;
            }
        }

        false
    }

    pub fn get_library_name_from_path<P: AsRef<Path>>(
        &self,
        library_path: P,
    ) -> Option<String> {
        for library in self.libraries.values() {
            let current_path = Path::new(&library.path);
            if current_path.eq(library_path.as_ref()) {
                return Some(library.name.clone());
            }
        }

        None
    }

    pub fn get_library_by_full_name<S: AsRef<str>>(
        &self,
        library_name: S,
    ) -> Option<&LibraryConfiguration> {
        self.libraries
            .values()
            .find(|library| library.name.eq(library_name.as_ref()))
    }
}

impl ResolvableWithContextConfiguration for UnresolvedConfiguration {
    type Resolved = Configuration;
    type Context = PathBuf;

    fn resolve(
        self,
        configuration_file_path: PathBuf,
    ) -> Result<Self::Resolved> {
        let paths = self.paths.resolve()?;
        let logging = self.logging.resolve(&paths)?;
        let ui = self.ui.resolve()?;
        let validation = self.validation.resolve()?;
        let tools = self.tools.resolve(&paths)?;

        let libraries: BTreeMap<String, LibraryConfiguration> = self
            .libraries
            .into_iter()
            .map(|(key, value)| {
                Ok::<_, miette::Report>((key, value.resolve(&paths)?))
            })
            .collect::<Result<_, _>>()?;

        let aggregated_library = self.aggregated_library.resolve(&paths)?;

        Ok(Configuration {
            paths,
            logging,
            ui,
            validation,
            tools,
            libraries,
            aggregated_library,
            configuration_file_path,
        })
    }
}
