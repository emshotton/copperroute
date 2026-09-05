use fr_dsn::format::json::to_gson_string_pretty;

use crate::{FanoutSettings, OptimizerSettings, RouterSettings, ScoringSettings, SettingsError};

pub(crate) fn constructed_fanout() -> Option<FanoutSettings> {
    Some(FanoutSettings::default())
}

pub(crate) fn constructed_optimizer() -> Option<OptimizerSettings> {
    Some(OptimizerSettings::default())
}

pub(crate) fn constructed_scoring() -> Option<ScoringSettings> {
    Some(ScoringSettings::default())
}

impl RouterSettings {
    pub fn from_json_str(s: &str) -> Result<Self, SettingsError> {
        Ok(serde_json::from_str(s)?)
    }

    pub fn to_json_string_pretty(&self) -> Result<String, SettingsError> {
        Ok(to_gson_string_pretty(self)?)
    }
}
