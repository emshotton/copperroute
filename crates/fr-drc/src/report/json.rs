//! `GsonProvider.GSON.toJson(report)` for the KiCad DRC report — `generateReportJson`
//! (`drc/DesignRulesChecker.java:817-820`) — in plan-5 ruling 2's two key flavors.
//!
//! # One table, two rows
//!
//! Java has no flavors: each jar hard-codes one spelling in its DTOs' `@SerializedName`s, and the
//! two jars disagree (ruling 1). The port carries both as [`HEAD`] and [`KICAD`], the single place
//! any flavored string is *chosen* — the six keys, and the two `type` **values** that move with
//! them. The DTO itself always holds HEAD's `type` string, because that is what Java stores in the
//! field (`report/build.rs`'s `head_kind_string`); [`FlavorKeys::violation_type`] is where the
//! rename happens, and the [`HEAD`] row is what it matches against.
//!
//! # Why the serialisation is hand-written
//!
//! The key set is flavor-dependent while the field order must be Java's declaration order
//! regardless — Gson writes `Class.getDeclaredFields()` order — so a `#[derive(Serialize)]` with
//! `#[serde(rename)]` cannot express it. The `Serialize` impls below walk the four DTOs in
//! declaration order and take their key strings from the table. Everything *below* the key is
//! `serde_json`'s, driven by [`fr_dsn::format::json::JavaNumberFormatter`], which is the write half of
//! `GsonProvider.GSON` (`util/gson/GsonProvider.java:12-20`, moved into `fr-dsn` by ruling 7):
//!
//! - two-space indent, `": "` after every key, no trailing newline;
//! - every `f64` through `Double.toString`, so a `pos` of `1.0e7` is `1.0E7` and a `qualityScore`
//!   keeps its `.078369140625` tail — Rust's `{}` agrees on neither;
//! - `disableHtmlEscaping()` (`GsonProvider.java:15`): `<`, `>`, `&`, `=`, `'` are written raw,
//!   while `U+2028`/`U+2029` are escaped anyway;
//! - a `None` `quality_score` **omits the key**, Gson's default `serializeNulls = false` — which
//!   is the only shape `generateReportJson` itself can produce, because the CLI assigns the score
//!   after the call (`Freerouting.java:349`).
//!
//! All four are JVM-measured by `crates/fr-drc/tests/data/JsonProbe.java`.

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};

use fr_dsn::format::json::to_gson_string_pretty;

use crate::checker::DesignRulesChecker;
use crate::error::DrcError;
use crate::report::build::{DrcCoordinates, DrcReportOptions};
use crate::report::{KiCadDrcPosition, KiCadDrcReport, KiCadDrcViolation, KiCadDrcViolationItem};

// -------------------------------------------------------------------------------------------
// The flavor table
// -------------------------------------------------------------------------------------------

/// Which spelling of the KiCad DRC schema to emit (plan-5 ruling 2).
///
/// [`DrcJsonFlavor::keys`] maps a variant to its row by `match`, not by discriminant value —
/// adding a third flavor is then a compile error rather than an out-of-bounds index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrcJsonFlavor {
    /// The clone's HEAD: camelCase keys, `holeClearance`, `unconnectedItems`. The parity default.
    //
    // Java bug: the `@SerializedName`s of io/kicad/KiCadDrcReport.java:20-57 and KiCadDrcViolation.java:33 drifted away from the schema those very files name. `jsonSchema` is `https://schemas.kicad.org/drc.v1.json` (KiCadDrcReport.java:21) and KiCad 9.0.1 writes `coordinate_units`/`kicad_version`/`unconnected_items`/`schematic_parity` and the types `hole_clearance`/`unconnected_items` (`../freerouting/fixtures/*-kicad_drc.json`); freerouting 2.3.0 matched it (`javap -p app/freerouting/drc/DrcReport.class` on `tools/freerouting-2.3.0.jar`), and HEAD does not. Reproduced as the default because HEAD is what this port ports (ruling 1) and HEAD's own DesignRulesCheckerTest.java:88-96 asserts camelCase; [`DrcJsonFlavor::KiCad`] is the way out. Quirks row #154.
    //
    // obligation: **Plan 8** wires the `-drc` CLI's flavor default. The product decision ruling 2
    // deferred is now made — **ruling W: the CLI defaults to `KiCad`** (the user's stated focus,
    // and what the document's own `$schema` promises), with HEAD's spelling behind a flag. This
    // enum's `Default` stays `FreeroutingHead`: it is the *parity* choice the crate's tests pin
    // against the jar, not the CLI's.
    #[default]
    FreeroutingHead,
    /// KiCad's own spelling, which is also freerouting 2.3.0's.
    KiCad,
}

/// The strings that move between the two flavors: six keys and the two `type` values, one row per
/// [`DrcJsonFlavor`] ([`HEAD`] and [`KICAD`]). (Ruling 2 calls it "nine strings"; there are eight fields and seven distinct
/// rewrites, because `unconnectedItems` is both a key and one of the two `type` values. The count
/// is the plan's arithmetic, not a measurement — see
/// `tests/report_json.rs::flavors_differ_only_in_the_key_tables_eight_strings`.) Every other key — `$schema`, `date`, `source`, `violations`, and
/// every per-violation (`description`, `items`, `severity`, `type`) and per-item (`description`,
/// `pos`, `uuid`, `x`, `y`) key — is identical in both, as are the `type` values `clearance`,
/// `track_dangling` and `via_dangling`.
struct FlavorKeys {
    coordinate_units: &'static str,
    kicad_version: &'static str,
    freerouting_version: &'static str,
    unconnected_items: &'static str,
    schematic_parity: &'static str,
    quality_score: &'static str,
    hole_clearance_type: &'static str,
    unconnected_items_type: &'static str,
}

/// The HEAD jar's `@SerializedName`s (`io/kicad/KiCadDrc*.java`).
///
/// This is also the spelling the DTOs always *store* in `KiCadDrcViolation::kind`: Java puts
/// whatever `UnconnectedItems.type` (UnconnectedItems.java:31, `:36`) and `convertToDrcViolation`
/// (DesignRulesChecker.java:326-330) computed into the field, and only Gson's `@SerializedName`s
/// differ between the two jars — so `report/build.rs` keeps its own `head_kind_string` and its
/// `"holeClearance"` literal, and [`FlavorKeys::violation_type`] renames what it wrote.
const HEAD: &FlavorKeys = &FlavorKeys {
    coordinate_units: "coordinateUnits",
    kicad_version: "kicadVersion",
    freerouting_version: "freeroutingVersion",
    unconnected_items: "unconnectedItems",
    schematic_parity: "schematicParity",
    quality_score: "qualityScore",
    hole_clearance_type: "holeClearance",
    unconnected_items_type: "unconnectedItems",
};

/// `tools/freerouting-2.3.0.jar`'s field names (`javap -p app/freerouting/drc/DrcReport.class`),
/// which are KiCad 9.0.1's too.
const KICAD: &FlavorKeys = &FlavorKeys {
    coordinate_units: "coordinate_units",
    kicad_version: "kicad_version",
    freerouting_version: "freerouting_version",
    unconnected_items: "unconnected_items",
    schematic_parity: "schematic_parity",
    quality_score: "quality_score",
    hole_clearance_type: "hole_clearance",
    unconnected_items_type: "unconnected_items",
};

impl DrcJsonFlavor {
    fn keys(self) -> &'static FlavorKeys {
        match self {
            DrcJsonFlavor::FreeroutingHead => HEAD,
            DrcJsonFlavor::KiCad => KICAD,
        }
    }
}

impl FlavorKeys {
    /// The `type` value for a violation whose DTO carries `stored`. Only the two strings the
    /// table names move; `clearance`, `track_dangling` and `via_dangling` pass through.
    fn violation_type<'a>(&'static self, stored: &'a str) -> &'a str {
        if stored == HEAD.hole_clearance_type {
            self.hole_clearance_type
        } else if stored == HEAD.unconnected_items_type {
            self.unconnected_items_type
        } else {
            stored
        }
    }
}

// -------------------------------------------------------------------------------------------
// The two entry points
// -------------------------------------------------------------------------------------------

impl KiCadDrcReport {
    /// `GsonProvider.GSON.toJson(report)` (DesignRulesChecker.java:819), byte for byte, in
    /// `flavor`'s key spelling.
    ///
    /// # Errors
    ///
    /// [`DrcError::Json`] if any coordinate or the quality score is NaN or ±Infinity — the point
    /// where Gson throws `IllegalArgumentException`, because `GsonProvider` never calls
    /// `serializeSpecialFloatingPointValues()` (`fr_dsn::format::json`'s module docs, point 3).
    /// Nothing `generate_report` produces can be non-finite; a hand-built report can.
    pub fn to_json(&self, flavor: DrcJsonFlavor) -> Result<String, DrcError> {
        // `fr_dsn::format::json::to_gson_string_pretty` *is* `GsonProvider.GSON.toJson` (ruling 7
        // moved it there); rebuilding its three lines here would be a second copy of a byte-parity
        // entry point, which is what that move existed to prevent.
        Ok(to_gson_string_pretty(&ReportSer {
            report: self,
            keys: flavor.keys(),
        })?)
    }
}

impl DesignRulesChecker<'_> {
    /// Port of `generateReportJson` (DesignRulesChecker.java:817-820): `generateReport` followed
    /// by `GsonProvider.GSON.toJson`, with ruling 2's flavor and ruling 5's injected fields.
    ///
    /// `&mut self` for [`DesignRulesChecker::generate_report`]'s reason (ruling 8): the clearance
    /// queries it runs lower each item's `smallestClearance` and advance the search tree's entry
    /// counter.
    ///
    /// # Errors
    ///
    /// [`KiCadDrcReport::to_json`]'s.
    pub fn report_to_json(
        &mut self,
        coords: &DrcCoordinates,
        options: &DrcReportOptions,
        flavor: DrcJsonFlavor,
    ) -> Result<String, DrcError> {
        self.generate_report(coords, options).to_json(flavor)
    }
}

// -------------------------------------------------------------------------------------------
// The six `Serialize` impls: one per DTO, in Java's declaration order, plus two list wrappers
// that thread the flavor down to each element
// -------------------------------------------------------------------------------------------

/// `io.kicad.KiCadDrcReport`, in `getDeclaredFields()` order (KiCadDrcReport.java:20-57).
struct ReportSer<'a> {
    report: &'a KiCadDrcReport,
    keys: &'static FlavorKeys,
}

impl Serialize for ReportSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let report = self.report;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("$schema", report.json_schema)?;
        map.serialize_entry(self.keys.coordinate_units, &report.coordinate_units)?;
        map.serialize_entry("date", &report.date)?;
        map.serialize_entry(self.keys.kicad_version, report.kicad_version)?;
        map.serialize_entry(self.keys.freerouting_version, &report.freerouting_version)?;
        map.serialize_entry("source", &report.source)?;
        map.serialize_entry(
            self.keys.unconnected_items,
            &ViolationsSer {
                violations: &report.unconnected_items,
                keys: self.keys,
            },
        )?;
        map.serialize_entry(
            "violations",
            &ViolationsSer {
                violations: &report.violations,
                keys: self.keys,
            },
        )?;
        map.serialize_entry(self.keys.schematic_parity, &report.schematic_parity)?;
        // Gson's default `serializeNulls = false`: a `null` `Double` writes no key at all
        // (KiCadDrcReport.java:56-57), which is every document `generateReportJson` produces.
        if let Some(score) = report.quality_score {
            map.serialize_entry(self.keys.quality_score, &score)?;
        }
        map.end()
    }
}

/// A `List<KiCadDrcViolation>`, threading the flavor through to each element.
struct ViolationsSer<'a> {
    violations: &'a [KiCadDrcViolation],
    keys: &'static FlavorKeys,
}

impl Serialize for ViolationsSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.violations.len()))?;
        for violation in self.violations {
            seq.serialize_element(&ViolationSer {
                violation,
                keys: self.keys,
            })?;
        }
        seq.end()
    }
}

/// `io.kicad.KiCadDrcViolation` (KiCadDrcViolation.java:10-34): `description`, `items`,
/// `severity`, `type` — declaration order, not the constructor's parameter order.
struct ViolationSer<'a> {
    violation: &'a KiCadDrcViolation,
    keys: &'static FlavorKeys,
}

impl Serialize for ViolationSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let violation = self.violation;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("description", &violation.description)?;
        map.serialize_entry("items", &ItemsSer(&violation.items))?;
        map.serialize_entry("severity", violation.severity)?;
        // The one *value* the flavor rewrites (ruling 2).
        map.serialize_entry("type", self.keys.violation_type(&violation.kind))?;
        map.end()
    }
}

/// A `List<KiCadDrcViolationItem>`. Nothing below this point is flavored.
struct ItemsSer<'a>(&'a [KiCadDrcViolationItem]);

impl Serialize for ItemsSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for item in self.0 {
            seq.serialize_element(&ItemSer(item))?;
        }
        seq.end()
    }
}

/// `io.kicad.KiCadDrcViolationItem` (KiCadDrcViolationItem.java:9-18).
struct ItemSer<'a>(&'a KiCadDrcViolationItem);

impl Serialize for ItemSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("description", &self.0.description)?;
        map.serialize_entry("pos", &PositionSer(&self.0.pos))?;
        map.serialize_entry("uuid", &self.0.uuid)?;
        map.end()
    }
}

/// `io.kicad.KiCadDrcPosition` (KiCadDrcPosition.java:9-13), whose `coordX`/`coordY` are
/// `@SerializedName`d back to `x`/`y` on HEAD; 2.3.0's `drc.DrcPosition` simply names the fields
/// `x`/`y` (`javap -p`), so this is one pair of keys the two jars already agree on. The two `f64`s
/// go through
/// [`fr_dsn::format::json::JavaNumberFormatter`], i.e. `Double.toString`.
struct PositionSer<'a>(&'a KiCadDrcPosition);

impl Serialize for PositionSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("x", &self.0.x)?;
        map.serialize_entry("y", &self.0.y)?;
        map.end()
    }
}
