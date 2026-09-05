pub mod drc;
pub mod info;
pub mod route;

use fr_settings::sources::CliSettings;
use std::path::PathBuf;

pub(crate) fn cli_settings(settings_argv: &[String]) -> CliSettings {
    if crate::legacy::is_legacy_form(settings_argv) {
        CliSettings::new(settings_argv)
    } else {
        CliSettings::new_with_set_alias(settings_argv)
    }
}

pub(crate) fn json_settings_path(settings_argv: &[String]) -> Option<PathBuf> {
    if crate::legacy::is_legacy_form(settings_argv) {
        return None;
    }
    let mut args = settings_argv.iter();
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--settings=") {
            return Some(PathBuf::from(value));
        }
        if arg == "--settings" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}
