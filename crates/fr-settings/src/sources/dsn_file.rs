//! `settings/sources/DsnFileSettings.java` (69): the DSN file's `(autoroute_settings …)` block
//! and its layer count, priority 20.

use std::io::Read;

use fr_dsn::BoardReadResult;

use crate::{RouterSettings, SettingsSource, SourceKind, merger::priority};

/// `settings/sources/DsnFileSettings.java`: router settings extracted from a Specctra design
/// file by a metadata-only parse.
///
/// # Quirk Q18 — the single most consequential quirk in this plan
///
/// `DsnFileSettings.java:46-48` calls `setLayerCount` whenever the extracted settings carry no
/// layers, which covers **both** "the file has no `(autoroute_settings)` block" and "it has one
/// that named no `layer_rule`". `RouterSettings.setLayerCount` does not merely size `layers`: it
/// seeds every element with `routable = Some(true)` and replaces *both* `scoring` cost arrays
/// with all-`1.0` vectors (`RouterSettings.java:466-477`).
///
/// So for essentially every board, the priority-20 DSN source contributes a full `layers[]` and
/// two fully populated `double[]`s. `copy_fields` rule 5 (`ReflectionUtil.java:269-290`) copies a
/// primitive array **only** into a target that is null or empty — first writer wins — so from
/// priority 20 onward `scoring.preferredDirectionTraceCost` and
/// `scoring.undesiredDirectionTraceCost` are frozen at `1.0`, and no `.rules` file, environment
/// variable or CLI flag at any higher priority can replace them. See the array arm of
/// [`crate::copy_fields`] and `docs/java-quirks.md`.
///
/// JVM-verified over three fixtures with no `(autoroute_settings)` block at all — `SProbe
/// F.Issue413-test.dsn`, `F.Issue066-Project_GP8B.dsn`, `F.Issue026-J2_reference.dsn` — and its
/// merged consequence at `SProbe G.merged`.
///
/// Java bug: DsnFileSettings (DsnFileSettings.java:43-48) — the comment there calls the seeding
/// a fix for wrongly-sized layer arrays, and for `layers` it is one. The collateral damage to
/// the two cost arrays is not intended by anything: it silently disables per-layer trace costs
/// for every board whose `.rules` file is read after the DSN.
#[derive(Debug, Clone)]
pub struct DsnFileSettings {
    settings: RouterSettings,
    filename: String,
}

impl DsnFileSettings {
    /// `DsnFileSettings.PRIORITY` (`:16`).
    const PRIORITY: i32 = priority::DSN_FILE;

    /// `DsnFileSettings(InputStream, String)` (`:28-53`).
    ///
    /// The parse is `DsnReader.readMetadata` (`fr_dsn::read_metadata`), which stops after the
    /// `(structure …)` scope. Only a `Success` **with** metadata contributes anything (`:35`) —
    /// an `OutlineMissing`, a parse error or an I/O error all leave the source blank, which is
    /// Java's behaviour too: its `instanceof BoardReadResult.Success s && s.metadata() != null`
    /// matches neither the other variants nor a null metadata.
    ///
    /// `FRLogger.debug` at `:51-52` is dropped (plan Global Constraints).
    #[must_use]
    pub fn new(dsn: impl Read, filename: &str) -> Self {
        let mut extracted: Option<RouterSettings> = None;
        let mut layer_count = 0usize;

        if let BoardReadResult::Success {
            metadata: Some(metadata),
            ..
        } = fr_dsn::read_metadata(dsn)
        {
            // `null` when the file has no `(autorouteSettings …)` block (:36).
            extracted = metadata.router_settings.map(RouterSettings::from);
            layer_count = metadata.layer_count;
        }

        // Start with whatever the DSN's autoroute block provided, or a blank slate (:41).
        // Not `unwrap_or_default()`: `RouterSettings::default()` leaves the three nested objects
        // `None`, while Java's `new RouterSettings()` allocates them (`RouterSettings.java:119-124`).
        #[allow(clippy::unwrap_or_default)]
        let mut settings = extracted.unwrap_or_else(RouterSettings::new);

        // :46-48 — conditional, not unconditional: a block that *did* name layer rules already
        // has its layers, and re-running `setLayerCount` there would discard the costs it just
        // parsed (quirk #126).
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
