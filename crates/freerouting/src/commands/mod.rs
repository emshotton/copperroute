pub mod drc;
pub mod info;
pub mod route;

use crate::cli::Cli;
use crate::ops::SettingsOverrides;

pub fn overrides(cli: &Cli, set: &[String]) -> SettingsOverrides {
    SettingsOverrides {
        settings_file: cli.settings.clone(),
        set: set.to_vec(),
        sparse: None,
    }
}
