//! The DRC report as KiCad's `drc.v1.json` document, keys in the order `kicad-cli pcb drc`
//! writes them.

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};

use copper_dsn::format::json::to_gson_string_pretty;

use crate::checker::DesignRulesChecker;
use crate::error::DrcError;
use crate::report::build::{DrcCoordinates, DrcReportOptions};
use crate::report::{KiCadDrcPosition, KiCadDrcReport, KiCadDrcViolation, KiCadDrcViolationItem};

impl KiCadDrcReport {
    pub fn to_json(&self) -> Result<String, DrcError> {
        Ok(to_gson_string_pretty(&ReportSer(self))?)
    }
}

impl DesignRulesChecker<'_> {
    pub fn report_to_json(
        &mut self,
        coords: &DrcCoordinates,
        options: &DrcReportOptions,
    ) -> Result<String, DrcError> {
        self.generate_report(coords, options).to_json()
    }
}

struct ReportSer<'a>(&'a KiCadDrcReport);

impl Serialize for ReportSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let report = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("$schema", report.json_schema)?;
        map.serialize_entry("coordinate_units", &report.coordinate_units)?;
        map.serialize_entry("date", &report.date)?;
        map.serialize_entry("kicad_version", report.kicad_version)?;
        map.serialize_entry("copperroute_version", &report.router_version)?;
        map.serialize_entry("source", &report.source)?;
        map.serialize_entry(
            "unconnected_items",
            &ViolationsSer(&report.unconnected_items),
        )?;
        map.serialize_entry("violations", &ViolationsSer(&report.violations))?;
        map.serialize_entry("schematic_parity", &report.schematic_parity)?;
        if let Some(score) = report.quality_score {
            map.serialize_entry("quality_score", &score)?;
        }
        map.end()
    }
}

struct ViolationsSer<'a>(&'a [KiCadDrcViolation]);

impl Serialize for ViolationsSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for violation in self.0 {
            seq.serialize_element(&ViolationSer(violation))?;
        }
        seq.end()
    }
}

struct ViolationSer<'a>(&'a KiCadDrcViolation);

impl Serialize for ViolationSer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let violation = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("description", &violation.description)?;
        map.serialize_entry("items", &ItemsSer(&violation.items))?;
        map.serialize_entry("severity", violation.severity)?;
        map.serialize_entry("type", &violation.kind)?;
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
