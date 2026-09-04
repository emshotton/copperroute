//!    equivalent is `#[serde(skip_serializing_if = "Option::is_none")]` on every field of the five
//! `layers` — from the raw tree (`:59-64`). Task 1's `#[serde(skip)]` / `#[serde(skip_serializing)]`
//!   fields carry `#[serde(default = "…")]` for exactly that, and an explicit `null` still clears
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
