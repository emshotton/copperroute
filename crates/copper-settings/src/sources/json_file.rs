use std::path::{Path, PathBuf};

use crate::{RouterSettings, SettingsError, SettingsSource, SourceKind, merger::priority};

pub const CONFIGURATION_FILE_NAME: &str = "copperroute.json";

#[derive(Debug, Clone)]
pub struct JsonFileSettings {
    json_file_path: PathBuf,
    settings: RouterSettings,
    errors: Vec<String>,
}

impl JsonFileSettings {
    const PRIORITY: i32 = priority::JSON_FILE;

    #[must_use]
    pub fn new(json_file_path: &Path) -> Self {
        let mut errors = Vec::new();
        let settings = load_settings(json_file_path, &mut errors);
        Self {
            json_file_path: json_file_path.to_path_buf(),
            settings,
            errors,
        }
    }

    #[must_use]
    pub fn from_working_directory() -> Self {
        Self::new(Path::new(CONFIGURATION_FILE_NAME))
    }

    #[must_use]
    pub fn json_file_path(&self) -> &Path {
        &self.json_file_path
    }

    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }
}

impl SettingsSource for JsonFileSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        CONFIGURATION_FILE_NAME.to_string()
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::JsonFile
    }
}

fn load_settings(json_file_path: &Path, errors: &mut Vec<String>) -> RouterSettings {
    if !json_file_path.exists() {
        return RouterSettings::new();
    }

    match read_router_object(json_file_path) {
        Ok(Some(settings)) => settings,
        Ok(None) => RouterSettings::new(),
        Err(error) => {
            errors.push(format!(
                "Failed to load settings from JSON file: {}: {error}",
                json_file_path.display()
            ));
            RouterSettings::new()
        }
    }
}

fn read_router_object(json_file_path: &Path) -> Result<Option<RouterSettings>, SettingsError> {
    let text = std::fs::read_to_string(json_file_path)?;
    let root: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text)?;

    match root.get("router") {
        Some(router) if router.is_object() => {
            Ok(Some(RouterSettings::from_json_str(&router.to_string())?))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).expect("write fixture");
        path
    }

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "copper-settings-json-file-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create scratch");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn priority_and_source_name_are_javas() {
        let scratch = Scratch::new("names");
        let source = JsonFileSettings::new(&scratch.0.join("absent.json"));
        assert_eq!(source.get_priority(), 10);
        assert_eq!(source.get_source_name(), "copperroute.json");
        assert_eq!(source.kind(), SourceKind::JsonFile);
    }

    #[test]
    fn a_missing_file_is_an_empty_router_settings_and_not_an_error() {
        let scratch = Scratch::new("missing");
        let source = JsonFileSettings::new(&scratch.0.join("absent.json"));
        let settings = source.get_settings().expect("never null");
        assert!(settings.max_passes.is_none());
        assert!(
            source.errors().is_empty(),
            "JsonFileSettings.java:43 debugs"
        );
    }

    #[test]
    fn only_the_keys_the_router_object_names_are_set() {
        let scratch = Scratch::new("keys");
        let path = write(
            &scratch.0,
            "copperroute.json",
            r#"{"version":"x","router":{"max_passes":7,"enabled":false}}"#,
        );
        let source = JsonFileSettings::new(&path);
        let settings = source.get_settings().expect("never null");
        assert_eq!(settings.max_passes, Some(7));
        assert_eq!(settings.enabled, Some(false));
        assert!(settings.max_threads.is_none());
        assert!(source.errors().is_empty());
    }

    #[test]
    fn a_document_with_no_router_key_is_empty_and_silent() {
        let scratch = Scratch::new("norouter");
        let path = write(&scratch.0, "copperroute.json", r#"{"version":"x"}"#);
        let source = JsonFileSettings::new(&path);
        assert!(
            source
                .get_settings()
                .expect("never null")
                .max_passes
                .is_none()
        );
        assert!(source.errors().is_empty());
    }

    #[test]
    fn a_router_key_that_is_not_an_object_is_empty_and_silent() {
        let scratch = Scratch::new("scalarrouter");
        let path = write(&scratch.0, "copperroute.json", r#"{"router":42}"#);
        let source = JsonFileSettings::new(&path);
        assert!(
            source
                .get_settings()
                .expect("never null")
                .max_passes
                .is_none()
        );
        assert!(source.errors().is_empty());
    }

    #[test]
    fn a_parse_failure_is_swallowed_into_an_empty_router_settings() {
        let scratch = Scratch::new("broken");
        let path = write(&scratch.0, "copperroute.json", "{not json at all");
        let source = JsonFileSettings::new(&path);
        assert!(
            source
                .get_settings()
                .expect("never null")
                .max_passes
                .is_none()
        );
        assert_eq!(source.errors().len(), 1, "JsonFileSettings.java:59");
    }

    #[test]
    fn a_document_that_is_not_an_object_is_swallowed_too() {
        let scratch = Scratch::new("array");
        let path = write(&scratch.0, "copperroute.json", "[1, 2, 3]");
        let source = JsonFileSettings::new(&path);
        assert!(
            source
                .get_settings()
                .expect("never null")
                .max_passes
                .is_none()
        );
        assert_eq!(source.errors().len(), 1, "getAsJsonObject at :48 throws");
    }

    #[test]
    fn a_router_object_the_reader_rejects_is_swallowed() {
        let scratch = Scratch::new("coercion");
        let path = write(
            &scratch.0,
            "copperroute.json",
            r#"{"router":{"max_passes":"42"}}"#,
        );
        let source = JsonFileSettings::new(&path);
        assert!(
            source
                .get_settings()
                .expect("never null")
                .max_passes
                .is_none()
        );
        assert_eq!(source.errors().len(), 1);
    }
}
