//! regardless — Gson writes `Class.getDeclaredFields()` order — so a `#[derive(Serialize)]` with
//! `#[serde(rename)]` cannot express it. The `Serialize` impls below walk the four DTOs in
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};

use fr_dsn::format::json::to_gson_string_pretty;

use crate::checker::DesignRulesChecker;
use crate::error::DrcError;
use crate::report::build::{DrcCoordinates, DrcReportOptions};
use crate::report::{KiCadDrcPosition, KiCadDrcReport, KiCadDrcViolation, KiCadDrcViolationItem};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrcJsonFlavor {
        #[default]
    FreeroutingHead,
        KiCad,
}

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


impl KiCadDrcReport {
                                        pub fn to_json(&self, flavor: DrcJsonFlavor) -> Result<String, DrcError> {
        Ok(to_gson_string_pretty(&ReportSer {
            report: self,
            keys: flavor.keys(),
        })?)
    }
}

impl DesignRulesChecker<'_> {
                                            pub fn report_to_json(
        &mut self,
        coords: &DrcCoordinates,
        options: &DrcReportOptions,
        flavor: DrcJsonFlavor,
    ) -> Result<String, DrcError> {
        self.generate_report(coords, options).to_json(flavor)
    }
}


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
        if let Some(score) = report.quality_score {
            map.serialize_entry(self.keys.quality_score, &score)?;
        }
        map.end()
    }
}

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
        map.serialize_entry("type", self.keys.violation_type(&violation.kind))?;
        map.end()
    }
}

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

struct PositionSer<'a>(&'a KiCadDrcPosition);

impl Serialize for PositionSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("x", &self.0.x)?;
        map.serialize_entry("y", &self.0.y)?;
        map.end()
    }
}
