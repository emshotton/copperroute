use std::io::Read;

use fr_dsn::BoardReadResult;

use crate::{RouterSettings, SettingsSource, SourceKind, merger::priority};

#[derive(Debug, Clone)]
pub struct DsnFileSettings {
    settings: RouterSettings,
    filename: String,
}

impl DsnFileSettings {
        const PRIORITY: i32 = priority::DSN_FILE;

                                        #[must_use]
    pub fn new(dsn: impl Read, filename: &str) -> Self {
        let mut extracted: Option<RouterSettings> = None;
        let mut layer_count = 0usize;

        if let BoardReadResult::Success {
            metadata: Some(metadata),
            ..
        } = fr_dsn::read_metadata(dsn)
        {
            extracted = metadata.router_settings.map(RouterSettings::from);
            layer_count = metadata.layer_count;
        }

        #[allow(clippy::unwrap_or_default)]
        let mut settings = extracted.unwrap_or_else(RouterSettings::new);

        if layer_count > 0 && settings.get_layer_count() == 0 {
            settings.set_layer_count(layer_count);
        }

        Self {
            settings,
            filename: filename.to_string(),
        }
    }
}

impl SettingsSource for DsnFileSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        format!("DSN file: {}", self.filename)
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::DsnFile
    }
}
