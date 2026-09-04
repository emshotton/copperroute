use crate::{HostEnvironment, RouterSettings};

pub mod priority {
    pub const DEFAULT: i32 = 0;
    pub const JSON_FILE: i32 = 10;
    pub const DSN_FILE: i32 = 20;
    pub const SES_FILE: i32 = 30;
    pub const RULES_FILE: i32 = 40;
    pub const GUI: i32 = 65;
    pub const ENVIRONMENT: i32 = 55;
    pub const CLI: i32 = 60;
    pub const API: i32 = 70;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    Default,
    JsonFile,
    DsnFile,
    SesFile,
    RulesFile,
    Gui,
    Environment,
    Cli,
    Api,
    Custom(&'static str),
}

pub trait SettingsSource {
    fn get_settings(&self) -> Option<&RouterSettings>;

    fn get_source_name(&self) -> String;

    fn get_priority(&self) -> i32;

    fn kind(&self) -> SourceKind;
}

pub struct SettingsMerger {
    sources: Vec<Box<dyn SettingsSource>>,
}

impl SettingsMerger {
    #[must_use]
    pub fn new(sources: Vec<Box<dyn SettingsSource>>) -> Self {
        let mut merger = Self {
            sources: Vec::new(),
        };
        merger.add_or_replace_sources(sources);
        merger
    }

    pub fn add_or_replace_sources(&mut self, new_sources: Vec<Box<dyn SettingsSource>>) {
        for new_source in new_sources {
            match self
                .sources
                .iter()
                .position(|existing| existing.kind() == new_source.kind())
            {
                Some(i) => self.sources[i] = new_source,
                None => self.sources.push(new_source),
            }
        }
    }

    #[must_use]
    pub fn sources(&self) -> &[Box<dyn SettingsSource>] {
        &self.sources
    }

    #[must_use]
    pub fn merge(&self, host: &HostEnvironment) -> RouterSettings {
        // :134-137 — `FRLogger.warn("No settings sources provided, using defaults")` is dropped
        if self.sources.is_empty() {
            return RouterSettings::new();
        }

        let mut sorted: Vec<&dyn SettingsSource> = self.sources.iter().map(Box::as_ref).collect();
        sorted.sort_by_key(|source| source.get_priority());

        let mut merged: Option<RouterSettings> = None;
        for source in sorted {
            let Some(settings) = source.get_settings() else {
                continue;
            };
            match merged.as_mut() {
                None => merged = Some(settings.java_clone()),
                Some(target) => {
                    let _report = target.apply_new_values_from(settings);
                }
            }
        }

        #[allow(clippy::unwrap_or_default)]
        let mut merged = merged.unwrap_or_else(RouterSettings::new);
        merged.validate(host);
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub(i32, SourceKind);

    impl SettingsSource for Stub {
        fn get_settings(&self) -> Option<&RouterSettings> {
            None
        }

        fn get_source_name(&self) -> String {
            format!("stub {}", self.0)
        }

        fn get_priority(&self) -> i32 {
            self.0
        }

        fn kind(&self) -> SourceKind {
            self.1
        }
    }

    #[test]
    fn constructor_collapses_duplicate_kinds_like_add_or_replace() {
        let merger = SettingsMerger::new(vec![
            Box::new(Stub(0, SourceKind::Default)),
            Box::new(Stub(1, SourceKind::Default)),
            Box::new(Stub(2, SourceKind::Api)),
        ]);
        assert_eq!(merger.sources().len(), 2);
        assert_eq!(merger.sources()[0].get_priority(), 1);
        assert_eq!(merger.sources()[1].get_priority(), 2);
    }

    #[test]
    fn empty_merger_returns_a_blank_without_validating() {
        let merged = SettingsMerger::new(Vec::new()).merge(&HostEnvironment::with_processors(4));
        assert_eq!(merged.max_passes, None);
    }
}
