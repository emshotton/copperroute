//! `settings/sources/JsonFileSettings.java` (79): the `freerouting.json` tier at priority 10.
//!
//! # Why this source exists at all (Plan 8 scan ruling R7)
//!
//! Plan 4 left this tier **reserved and empty**: spec §2 gives the port no persistent
//! configuration file, `merger.rs` kept the number and the identity, and
//! `scripts/differential/java/P4T1.java` proved on the JVM that the tier contributes nothing when
//! the file is absent. That was correct for Plan 4, whose CLI had no `--settings` option and no
//! working-directory lookup — but it left a **hole in the settings ladder**, because Java's
//! priority-10 source is not a GUI class and is not telemetry: it is the one tier between
//! `DefaultSettings` (0) and the DSN file (20), and `SettingsMergerTest
//! .legacyBatchModeEnablesRouterWhenJsonDisablesIt` is written against it.
//!
//! Plan 7's hand-off flagged the gap twice and named Plan 8's Task 5 as its owner; the Plan 8
//! pre-flight scan raised it as ruling **R7** and the controller ruled it **in**. So the class is
//! ported here, in full, and wired to two things by `crates/freerouting`:
//!
//! * `--settings <file>` on the native subcommand form — the port's spelling of Java's
//!   `JsonFileSettings(Path)` constructor (`:36-39`);
//! * `freerouting.json` **in the working directory** — the port's stand-in for Java's
//!   `GlobalSettings.getUserDataPath().resolve("freerouting.json")` (`:27-29`). The user-data
//!   path itself stays unported (`static` mutable path state, spec §2 — see the roster in
//!   `crate::sources::cli`), so the port looks where the process already is instead of in an
//!   OS-standard directory it does not otherwise use.
//!
//! # The leniency this reproduces, and the one it does not
//!
//! Quirk #141's strictness split governs the reader: `GsonProvider.GSON` is built with
//! `Strictness.LENIENT`, and [`RouterSettings::from_json_str`] — which `JsonFileSettings.java:51`
//! is the only headless caller of — is `serde_json`, which is strict. Where Gson reads an
//! unquoted name or coerces `"42"` into an `Integer`, this port reports an error. That divergence
//! is acceptance-only (an error where Java produced a value, never a *different* value) and is
//! recorded on [`crate::json`]'s module docs.
//!
//! One level of leniency **is** reproduced, because it is not Gson's, it is this class's: `:55-62`
//! swallows *every* exception — an I/O error, a document that is not a JSON object, a `router`
//! element that will not deserialise — into an empty [`RouterSettings`]. A `freerouting.json` full
//! of nonsense therefore changes nothing rather than aborting the run, and the port answers the
//! same empty object. The errors are kept on [`JsonFileSettings::errors`] because this crate has
//! no `FRLogger`, but no caller has to read them.

use std::path::{Path, PathBuf};

use crate::{RouterSettings, SettingsError, SettingsSource, SourceKind, merger::priority};

/// `GlobalSettings.getConfigurationFilePath` resolves this name under the user-data directory
/// (`GlobalSettings.java:178-180`); the port resolves it against the working directory instead.
pub const CONFIGURATION_FILE_NAME: &str = "freerouting.json";

/// `settings/sources/JsonFileSettings.java`: the router settings a `freerouting.json` names,
/// priority 10.
///
/// Only the keys the file's `router` object actually holds are non-`None` — that nullability is
/// the merge protocol (`SettingsMerger.java:22-31`). Everything else, including a missing file,
/// a malformed file and a file with no `router` key, is an empty `RouterSettings` that the merge
/// steps straight over.
#[derive(Debug, Clone)]
pub struct JsonFileSettings {
    json_file_path: PathBuf,
    settings: RouterSettings,
    errors: Vec<String>,
}

impl JsonFileSettings {
    /// `JsonFileSettings.PRIORITY` (`:22`).
    const PRIORITY: i32 = priority::JSON_FILE;

    /// `JsonFileSettings(Path)` (`:36-39`) — the constructor `--settings <file>` uses.
    ///
    /// renamed: JsonFileSettings -> JsonFileSettings::new (Rust has no constructors). Java runs
    /// `loadSettings()` from the constructor (`:38`), so the file is read **once**, here, and
    /// [`SettingsSource::get_settings`] is a pure accessor afterwards.
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

    /// `JsonFileSettings()` (`:27-29`), with the working directory standing in for the user-data
    /// path — see the module docs.
    ///
    /// renamed: JsonFileSettings() -> JsonFileSettings::from_working_directory — Rust has no
    /// constructor overloads, and the name says which of Java's two the caller wants.
    #[must_use]
    pub fn from_working_directory() -> Self {
        Self::new(Path::new(CONFIGURATION_FILE_NAME))
    }

    /// The path this source was built from — `JsonFileSettings.jsonFilePath` (`:23`), which Java
    /// keeps only to name it in its four log lines.
    #[must_use]
    pub fn json_file_path(&self) -> &Path {
        &self.json_file_path
    }

    /// Everything `:55-62` swallowed, in the order encountered — this port's stand-in for the two
    /// `FRLogger.warn` arms. Empty for a missing file, which is `:42-45`'s silent `debug` arm.
    ///
    // added in Plan 4: (no Java counterpart — the failures only reach FRLogger)
    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }
}

impl SettingsSource for JsonFileSettings {
    /// `JsonFileSettings.getSettings` (`:65-68`).
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    /// `JsonFileSettings.getSourceName` (`:70-73`) — the literal `"freerouting.json"`, whatever
    /// path the source was actually built from.
    fn get_source_name(&self) -> String {
        CONFIGURATION_FILE_NAME.to_string()
    }

    /// `JsonFileSettings.getPriority` (`:75-78`).
    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::JsonFile
    }
}

/// `JsonFileSettings.loadSettings` (`:41-63`), in Java's own order.
///
/// 1. `:42-45` — the file does not exist: `FRLogger.debug` and an empty `RouterSettings`. **Not**
///    an error, and not recorded as one.
/// 2. `:47-48` — read the whole document and take it as a JSON *object*. `getAsJsonObject()`
///    throws `IllegalStateException` for an array or a scalar, which `:58` catches.
/// 3. `:49-50` — `root.get("router")`, applied only when it is present **and** an object. A
///    document with no `router` key falls through to `:62` without a log line.
/// 4. `:51-53` — the object goes through `GsonProvider.GSON`; here,
///    [`RouterSettings::from_json_str`].
/// 5. `:55-62` — every failure is a warning and an empty `RouterSettings`.
fn load_settings(json_file_path: &Path, errors: &mut Vec<String>) -> RouterSettings {
    // :42-45 — `Files.exists` is a *check*, not the read, so a file that disappears between the
    // two lands in the `catch` below exactly as it does in Java.
    if !json_file_path.exists() {
        return RouterSettings::new();
    }

    match read_router_object(json_file_path) {
        Ok(Some(settings)) => settings,
        // :49-50's `routerElement == null || !isJsonObject()` — silent, and :62's empty object.
        Ok(None) => RouterSettings::new(),
        // :55-60 — `catch (IOException)` and `catch (Exception)` differ only in their message.
        Err(error) => {
            errors.push(format!(
                "Failed to load settings from JSON file: {}: {error}",
                json_file_path.display()
            ));
            RouterSettings::new()
        }
    }
}

/// Steps 2-4 above, with the failures Java catches expressed as a `Result`.
fn read_router_object(json_file_path: &Path) -> Result<Option<RouterSettings>, SettingsError> {
    // `Files.newBufferedReader` is UTF-8; so is `std::fs::read_to_string`.
    let text = std::fs::read_to_string(json_file_path)?;
    // `JsonParser.parseReader(reader).getAsJsonObject()` (:48) as one step: `getAsJsonObject`
    // throws `IllegalStateException` for an array or a scalar, and deserialising straight into a
    // JSON *map* fails on exactly the same documents. Both land in the `catch` at :58.
    let root: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text)?;

    // :49-50
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

    /// A scratch directory that removes itself, so the tests never touch the working directory.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "fr-settings-json-file-{tag}-{}",
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
        assert_eq!(source.get_source_name(), "freerouting.json");
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
            "freerouting.json",
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
        let path = write(&scratch.0, "freerouting.json", r#"{"version":"x"}"#);
        let source = JsonFileSettings::new(&path);
        assert!(
            source
                .get_settings()
                .expect("never null")
                .max_passes
                .is_none()
        );
        // :49-50 falls through to :62 without a log line.
        assert!(source.errors().is_empty());
    }

    #[test]
    fn a_router_key_that_is_not_an_object_is_empty_and_silent() {
        let scratch = Scratch::new("scalarrouter");
        let path = write(&scratch.0, "freerouting.json", r#"{"router":42}"#);
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
        let path = write(&scratch.0, "freerouting.json", "{not json at all");
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
        let path = write(&scratch.0, "freerouting.json", "[1, 2, 3]");
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
        // Quirk #141: Gson would coerce `"42"` into `max_passes`; `serde_json` refuses. Java's
        // own `catch` at :58 is what makes the two agree on the *outcome* — an empty object.
        let scratch = Scratch::new("coercion");
        let path = write(
            &scratch.0,
            "freerouting.json",
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
