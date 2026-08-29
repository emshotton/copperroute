//! `settings/sources/SesFileSettings.java` (52): the explicit no-op at priority 30.

use crate::{RouterSettings, SettingsSource, SourceKind, merger::priority};

/// `settings/sources/SesFileSettings.java`: the SES tier of the priority ladder.
///
/// **It does nothing, on purpose.** `SesFileSettings.loadSettings` returns
/// `new RouterSettings()` unconditionally (`:27-36`) — it never opens the file, and the `try`
/// block it wraps that single `new` in cannot throw. The class exists so the ladder documented
/// in `SettingsSource.java:35-40` has a slot at 30, and so that a SES file named on the command
/// line has *something* to be registered as. Java never registers it in the headless path at all
/// (plan ruling 1).
///
/// It is kept here, rather than dropped, for two reasons: the priority ladder is only legible
/// with all of its rungs, and spec §11 lists a SES tier — so without this type and this comment,
/// somebody would eventually re-add one believing that Java's does something. It does not:
/// SES files carry routing *results*, not router configuration.
///
/// JVM-verified: `SProbe D.ses` — `getSettings()` is not null, `maxPasses` is null, layer count
/// is 0, i.e. a plain `new RouterSettings()`.
#[derive(Debug, Clone)]
pub struct SesFileSettings {
    settings: RouterSettings,
    file_name: String,
}

impl SesFileSettings {
    /// `SesFileSettings.PRIORITY` (`:13`).
    const PRIORITY: i32 = priority::SES_FILE;

    /// `SesFileSettings(String)` (`:22-25`). The name is the only argument in Java either — the
    /// file itself is never read.
    ///
    /// not ported: loadSettings (`SesFileSettings.java:27-36`) — a private method whose whole
    /// body is `return new RouterSettings();` inside a `try` that cannot throw; inlined here.
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
