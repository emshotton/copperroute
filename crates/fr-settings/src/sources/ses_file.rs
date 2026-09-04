use crate::{RouterSettings, SettingsSource, SourceKind, merger::priority};

#[derive(Debug, Clone)]
pub struct SesFileSettings {
    settings: RouterSettings,
    file_name: String,
}

impl SesFileSettings {
        const PRIORITY: i32 = priority::SES_FILE;

                        #[must_use]
    pub fn new(file_name: &str) -> Self {
        Self {
            settings: RouterSettings::new(),
            file_name: file_name.to_string(),
        }
    }
}

impl SettingsSource for SesFileSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        format!("SES file: {}", self.file_name)
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::SesFile
    }
}
