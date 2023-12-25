use miette::Result;

use crate::paths::PathsConfiguration;

pub trait ResolvableConfiguration {
    type Resolved;

    fn resolve(self) -> Result<Self::Resolved>;
}

pub trait ResolvableWithPathsConfiguration {
    type Resolved;

    fn resolve(self, paths: &PathsConfiguration) -> Result<Self::Resolved>;
}

pub trait ResolvableWithContextConfiguration {
    type Resolved;
    type Context;

    fn resolve(self, context: Self::Context) -> Result<Self::Resolved>;
}
