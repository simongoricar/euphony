use miette::Result;

use crate::configuration::ConfigEssentials;

pub trait AfterLoadInitable {
    fn after_load_init(&mut self) -> Result<()>;
}

pub trait AfterLoadWithEssentialsInitable {
    fn after_load_init(&mut self, essentials: &ConfigEssentials) -> Result<()>;
}
