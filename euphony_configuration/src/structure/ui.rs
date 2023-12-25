use serde::Deserialize;

use crate::traits::ResolvableConfiguration;

#[derive(Clone)]
pub struct UiConfiguration {
    pub transcoding: TranscodingUiConfiguration,
}

#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedUiConfiguration {
    transcoding: UnresolvedTranscodingUiConfiguration,
}

impl ResolvableConfiguration for UnresolvedUiConfiguration {
    type Resolved = UiConfiguration;

    fn resolve(self) -> miette::Result<Self::Resolved> {
        Ok(UiConfiguration {
            transcoding: self.transcoding.resolve()?,
        })
    }
}



#[derive(Clone)]
pub struct TranscodingUiConfiguration {
    pub show_logs_tab_on_exit: bool,
}


#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedTranscodingUiConfiguration {
    show_logs_tab_on_exit: bool,
}

impl ResolvableConfiguration for UnresolvedTranscodingUiConfiguration {
    type Resolved = TranscodingUiConfiguration;

    fn resolve(self) -> miette::Result<Self::Resolved> {
        Ok(TranscodingUiConfiguration {
            show_logs_tab_on_exit: self.show_logs_tab_on_exit,
        })
    }
}
