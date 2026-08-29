//! `settings/sources/RulesFileSettings.java` (109): a `.rules` file's `(autoroute_settings …)`
//! block, priority 40.

use std::io::Read;
use std::path::Path;

use crate::{RouterSettings, SettingsSource, SourceKind, merger::priority};

/// `settings/sources/RulesFileSettings.java`: the explicit routing-rule overrides.
///
/// This is the tier that carries per-layer directions and per-layer trace costs into the merge,
/// and — because of quirk Q18 — the tier whose trace costs the DSN source at priority 20 has
/// usually already blocked. Its per-layer `preferredDirectionHorizontal` still lands, because
/// `layers` is an *object* array and merges element-wise (plan ruling 1's Q1 channel).
///
/// renamed: RulesFileSettings -> RulesFileSettings::new / RulesFileSettings::from_path.
/// Java has four constructors (`:28-69`) — `InputStream`, `File`, `Path`, `String` — that differ
/// only in how they reach the bytes and what they call the source. They collapse into two:
/// [`Self::new`] for an already-open reader and [`Self::from_path`] for the three
/// filesystem-facing ones, which share Java's "file missing → blank settings" arm (`:40`,
/// `:57-69`).
///
/// **The two naming rules Java's overloads disagree on are preserved**: the `File`/`Path`
/// constructors name the source with `file.getName()` — the *basename* (`:39`) — while the
/// `String` constructor keeps the string verbatim (`:58`). [`Self::from_path`] takes the
/// basename, [`Self::new`] takes whatever the caller passes. JVM-verified: `SProbe
/// D.rulesMissing.getSourceName = RULES file: dummy.rules` (String ctor) and `E.hw48na` =
/// `RULES file: Issue029-hw48na_valid.rules` for a path with directories in it (File ctor).
#[derive(Debug, Clone)]
pub struct RulesFileSettings {
    settings: RouterSettings,
    file_name: String,
}

impl RulesFileSettings {
    /// `RulesFileSettings.PRIORITY` (`:18`).
    const PRIORITY: i32 = priority::RULES_FILE;

    /// `RulesFileSettings(InputStream, String)` + `loadSettings` (`:28-31`, `:81-93`).
    ///
    /// `RulesReader.readRouterSettings` returning `null` — an empty stream, a bad header, or no
    /// `(autoroute_settings …)` scope — and **any** exception both give a blank
    /// `RouterSettings`; this never fails (`:84-92`). The two `FRLogger` calls are dropped
    /// (plan Global Constraints); a `MergeReport` entry would have no merge to attach to,
    /// because the failure happens before any field is copied.
    #[must_use]
    pub fn new(rules: impl Read, file_name: &str) -> Self {
        let settings = match fr_dsn::rules_reader::read_router_settings(rules) {
            Ok(Some(extracted)) => RouterSettings::from(extracted),
            Ok(None) | Err(_) => RouterSettings::new(),
        };
        Self {
            settings,
            file_name: file_name.to_string(),
        }
    }

    /// `RulesFileSettings(File)` / `(Path)` / `(String)` (`:38-69`, `loadFromFile` `:71-79`).
    ///
    /// A path that does not exist, or that cannot be opened, gives a blank `RouterSettings`
    /// rather than an error (`:40`, `:63-65`, `:76-78`) — `SProbe D.rulesMissing`. The source
    /// name is the path's **basename**, matching the `File` constructor (`:39`); a path with no
    /// final component at all falls back to its full display form, where Java's
    /// `file.getName()` would answer the empty string.
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        let file_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        match std::fs::File::open(path) {
            Ok(file) => Self::new(file, &file_name),
            Err(_) => Self {
                settings: RouterSettings::new(),
                file_name,
            },
        }
    }
}

impl SettingsSource for RulesFileSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        format!("RULES file: {}", self.file_name)
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::RulesFile
    }
}
