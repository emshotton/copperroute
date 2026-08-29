//! The KiCad DRC report: the four `io/kicad/KiCadDrc*.java` DTOs, plus the two parameter objects
//! the port needs where Java reaches for a clock and for `board.communication`.
//!
//! Java: `io/kicad/{KiCadDrcReport,KiCadDrcViolation,KiCadDrcViolationItem,KiCadDrcPosition}.java`.
//! [`build`] holds `DesignRulesChecker::generate_report` and the mappers that fill these.
//!
//! # Field order is emission order
//!
//! Gson writes a class's fields in `Class.getDeclaredFields()` order, which on every JVM this port
//! targets is declaration order, so the structs below keep Java's declaration order field for
//! field. Task 8's serialiser walks them in that order and supplies ruling 2's two key spellings;
//! nothing here carries a `serde` attribute yet, deliberately — the flavor table is Task 8's.

pub mod build;

pub use build::{DrcCoordinates, DrcReportOptions, item_description};

/// Port of `io.kicad.KiCadDrcReport` (KiCadDrcReport.java:17-84).
///
/// Java's constructor takes three arguments and fills `date` from `ZonedDateTime.now()` (`:70`);
/// the port takes the formatted date too (plan-5 ruling 5 — this crate has no clock), which is
/// what makes the class testable at all.
#[derive(Debug, Clone, PartialEq)]
pub struct KiCadDrcReport {
    // renamed: KiCadDrcReport.jsonSchema -> json_schema (KiCadDrcReport.java:20-21); its
    // `@SerializedName` is `$schema`, which Task 8's key table carries.
    /// Java `jsonSchema` (KiCadDrcReport.java:20-21), a `final` initialised in place.
    pub json_schema: &'static str,
    /// Java `coordinateUnits` (KiCadDrcReport.java:24-25): the unit name the report's numbers are
    /// in — `"mm"` from the CLI (Freerouting.java:335, quirk #151).
    pub coordinate_units: String,
    /// Java `date` (KiCadDrcReport.java:28-29). **Injected** (plan-5 ruling 5), never `now()`:
    /// Java's `ZonedDateTime.now().format(ISO_OFFSET_DATE_TIME)` is a constructor side effect,
    /// and a clock in a parity crate is a permanent flake.
    pub date: String,
    /// Java `kicadVersion` (KiCadDrcReport.java:32-33), the constant `"N/A"` — Freerouting is not
    /// KiCad.
    pub kicad_version: &'static str,
    /// Java `freeroutingVersion` (KiCadDrcReport.java:36-37): `"Freerouting " +
    /// Constants.FREEROUTING_VERSION`, concatenated by `generateReport`
    /// (DesignRulesChecker.java:212-213).
    pub freerouting_version: String,
    /// Java `source` (KiCadDrcReport.java:40-41): the input file's base name
    /// (Freerouting.java:339).
    pub source: String,
    /// Java `unconnectedItems` (KiCadDrcReport.java:44-45): one entry per net that falls into two
    /// or more connected groups. **Not** the same quantity as `getIncompleteCount()`, which counts
    /// airlines (plan-5 ruling 11).
    pub unconnected_items: Vec<KiCadDrcViolation>,
    /// Java `violations` (KiCadDrcReport.java:48-49): every clearance violation, followed by every
    /// `track_dangling`/`via_dangling` entry `generateReport` moves out of the unconnected list
    /// (DesignRulesChecker.java:268-276).
    pub violations: Vec<KiCadDrcViolation>,
    /// Java `schematicParity` (KiCadDrcReport.java:52-53): declared, constructed empty (`:73`) and
    /// never added to anywhere in the Java tree. `serde_json::Value` rather than a unit type
    /// because the JSON schema's element type is an object and Plan 8 may fill it.
    pub schematic_parity: Vec<serde_json::Value>,
    /// Java `qualityScore` (KiCadDrcReport.java:56-57), the one non-`final` field: a boxed
    /// `Double` the CLI assigns after construction (Freerouting.java:349). `None` is Java's
    /// `null`, which Gson omits from the output.
    pub quality_score: Option<f64>,
}

impl KiCadDrcReport {
    /// Port of `KiCadDrcReport(String, String, String)` (KiCadDrcReport.java:66-74), plus the
    /// injected `date` that replaces Java's `ZonedDateTime.now()` (`:70`, plan-5 ruling 5).
    pub fn new(
        coordinate_units: impl Into<String>,
        source: impl Into<String>,
        version: impl Into<String>,
        date: impl Into<String>,
    ) -> KiCadDrcReport {
        KiCadDrcReport {
            json_schema: "https://schemas.kicad.org/drc.v1.json",
            coordinate_units: coordinate_units.into(),
            date: date.into(),
            kicad_version: "N/A",
            freerouting_version: version.into(),
            source: source.into(),
            unconnected_items: Vec::new(),
            violations: Vec::new(),
            schematic_parity: Vec::new(),
            quality_score: None,
        }
    }

    /// Port of `addViolation` (KiCadDrcReport.java:77-79).
    pub fn add_violation(&mut self, violation: KiCadDrcViolation) {
        self.violations.push(violation);
    }

    /// Port of `addUnconnectedItem` (KiCadDrcReport.java:82-84).
    pub fn add_unconnected_item(&mut self, item: KiCadDrcViolation) {
        self.unconnected_items.push(item);
    }
}

/// Port of `io.kicad.KiCadDrcViolation` (KiCadDrcViolation.java:7-50). Field order is Java's
/// declaration order — `description`, `items`, `severity`, `type` — not the constructor's.
#[derive(Debug, Clone, PartialEq)]
pub struct KiCadDrcViolation {
    /// Java `description` (KiCadDrcViolation.java:10-11).
    pub description: String,
    /// Java `items` (KiCadDrcViolation.java:14-15): two entries for a clearance violation, one for
    /// a dangling item, and the whole of `allItems` for an unconnected net.
    pub items: Vec<KiCadDrcViolationItem>,
    /// Java `severity` (KiCadDrcViolation.java:18-19): `"error"` for both clearance types
    /// (DesignRulesChecker.java:355), `"warning"` for all three unconnected types (`:382`,
    /// `:420`). Java's javadoc also names `"ignore"`, which nothing produces.
    pub severity: &'static str,
    // renamed: KiCadDrcViolation.type -> kind, because `type` is a Rust keyword. The
    // `@SerializedName("type")` (KiCadDrcViolation.java:33) is what Task 8's key table emits.
    /// Java `type` (KiCadDrcViolation.java:33-34): `"clearance"`, `"holeClearance"`,
    /// `"unconnectedItems"`, `"track_dangling"` or `"via_dangling"` — the HEAD spellings
    /// (plan-5 ruling 1). Ruling 2's KiCad flavor renames two of them at serialisation time; this
    /// field always carries HEAD's, because Java stores exactly what `UnconnectedItems.type` and
    /// `convertToDrcViolation` computed.
    pub kind: String,
}

impl KiCadDrcViolation {
    /// Port of `KiCadDrcViolation(String, String, String, List)` (KiCadDrcViolation.java:44-50),
    /// parameter order included.
    pub fn new(
        kind: impl Into<String>,
        description: impl Into<String>,
        severity: &'static str,
        items: Vec<KiCadDrcViolationItem>,
    ) -> KiCadDrcViolation {
        KiCadDrcViolation {
            description: description.into(),
            items,
            severity,
            kind: kind.into(),
        }
    }
}

/// Port of `io.kicad.KiCadDrcViolationItem` (KiCadDrcViolationItem.java:6-31).
#[derive(Debug, Clone, PartialEq)]
pub struct KiCadDrcViolationItem {
    /// Java `description` (KiCadDrcViolationItem.java:9-10).
    pub description: String,
    /// Java `pos` (KiCadDrcViolationItem.java:13-14).
    pub pos: KiCadDrcPosition,
    /// Java `uuid` (KiCadDrcViolationItem.java:17-18): `String.valueOf(item.getId())`
    /// (DesignRulesChecker.java:321-322) — the board-unique item id, not a UUID.
    pub uuid: String,
}

impl KiCadDrcViolationItem {
    /// Port of `KiCadDrcViolationItem(String, KiCadDrcPosition, String)`
    /// (KiCadDrcViolationItem.java:27-31).
    pub fn new(
        description: impl Into<String>,
        pos: KiCadDrcPosition,
        uuid: impl Into<String>,
    ) -> KiCadDrcViolationItem {
        KiCadDrcViolationItem {
            description: description.into(),
            pos,
            uuid: uuid.into(),
        }
    }
}

/// Port of `io.kicad.KiCadDrcPosition` (KiCadDrcPosition.java:6-25).
///
// renamed: KiCadDrcPosition.coordX/coordY -> x/y (KiCadDrcPosition.java:10-13). Java's fields are
// named `coordX`/`coordY` and `@SerializedName`d back to `x`/`y`; the port skips the detour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KiCadDrcPosition {
    /// Java `coordX` (KiCadDrcPosition.java:9-10), a raw `double` in the report's unit.
    pub x: f64,
    /// Java `coordY` (KiCadDrcPosition.java:12-13).
    pub y: f64,
}

impl KiCadDrcPosition {
    /// Port of `KiCadDrcPosition(double, double)` (KiCadDrcPosition.java:22-25).
    pub fn new(x: f64, y: f64) -> KiCadDrcPosition {
        KiCadDrcPosition { x, y }
    }
}
