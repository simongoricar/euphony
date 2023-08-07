use miette::Result;

use crate::configuration::ConfigPaths;

pub trait AfterLoadInitable {
    fn after_load_init(&mut self) -> Result<()>;
}

pub trait AfterLoadWithPathsInitable {
    fn after_load_init(&mut self, paths: &ConfigPaths) -> Result<()>;
}
