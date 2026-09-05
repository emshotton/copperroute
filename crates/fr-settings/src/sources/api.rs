use crate::{RouterSettings, SettingsSource, SourceKind, merger::priority};

#[derive(Debug, Clone)]
pub struct ApiSettings {
    settings: RouterSettings,
}

impl ApiSettings {
    const PRIORITY: i32 = priority::API;

    #[allow(clippy::unwrap_or_default)]
    #[must_use]
    pub fn new(settings: Option<RouterSettings>) -> Self {
        Self {
            settings: settings.unwrap_or_else(RouterSettings::new),
        }
    }
}

impl SettingsSource for ApiSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        "API Settings".to_string()
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Api
    }
}
