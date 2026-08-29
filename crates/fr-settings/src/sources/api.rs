//! `settings/sources/ApiSettings.java` (38): the highest-priority source, 70.

use crate::{RouterSettings, SettingsSource, SourceKind, merger::priority};

/// `settings/sources/ApiSettings.java`: settings supplied by a REST API caller.
///
/// At priority 70 it outranks every other source. In the headless path that Plan 4 reproduces it
/// is not an API payload at all: `RoutingJobScheduler.java:163-166` registers
/// `new ApiSettings(job.routerSettings)` — the *entire result of merge #1* — so merge #2's own
/// 0-to-60 chain can only contribute where merge #1 left a field absent (plan ruling 1's Q1
/// channel). Task 8 is where that matters.
///
/// JVM-verified: `SProbe D.apiNull.maxPasses = null`, `D.api.maxPasses = 7`.
#[derive(Debug, Clone)]
pub struct ApiSettings {
    settings: RouterSettings,
}

impl ApiSettings {
    /// `ApiSettings.PRIORITY` (`:12`).
    const PRIORITY: i32 = priority::API;

    /// `ApiSettings(RouterSettings)` (`:20-22`): `null` becomes a blank `RouterSettings`.
    ///
    /// `unwrap_or_default()` (clippy's suggestion) would be wrong: `RouterSettings::default()`
    /// leaves the three nested objects `None`, while Java's `new RouterSettings()` allocates
    /// them (`RouterSettings.java:119-124`).
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
