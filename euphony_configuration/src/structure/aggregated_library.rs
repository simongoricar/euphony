use serde::Deserialize;

use crate::{
    paths::PathsConfiguration,
    traits::ResolvableWithPathsConfiguration,
};

#[derive(Clone)]
pub struct AggregatedLibraryConfiguration {
    pub path: String,

    pub transcode_threads: usize,

    pub failure_max_retries: u16,

    pub failure_delay_seconds: u16,
}

#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedAggregatedLibraryConfiguration {
    path: String,

    transcode_threads: usize,

    failure_max_retries: u16,

    failure_delay_seconds: u16,
}

impl ResolvableWithPathsConfiguration
    for UnresolvedAggregatedLibraryConfiguration
{
    type Resolved = AggregatedLibraryConfiguration;

    fn resolve(
        self,
        paths: &PathsConfiguration,
    ) -> miette::Result<Self::Resolved> {
        let path = self
            .path
            .replace("{LIBRARY_BASE}", &paths.base_library_path);

        if self.transcode_threads == 0 {
            panic!("transcode_threads is set to 0! The minimum value is 1.");
        }


        Ok(AggregatedLibraryConfiguration {
            path,
            transcode_threads: self.transcode_threads,
            failure_max_retries: self.failure_max_retries,
            failure_delay_seconds: self.failure_delay_seconds,
        })
    }
}
