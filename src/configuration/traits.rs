use miette::Result;

use crate::configuration::ConfigPaths;

pub trait AfterLoadInitable {
    fn after_load_init(&mut self) -> Result<()>;
}

pub trait AfterLoadWithEssentialsInitable {
    fn after_load_init(&mut self, essentials: &ConfigPaths) -> Result<()>;
}
