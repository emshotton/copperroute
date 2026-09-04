//! flavor-dependent, which no `#[serde(rename)]` can express.

pub mod build;
pub mod json;

pub use build::{DrcCoordinates, DrcReportOptions, item_description};
pub use json::DrcJsonFlavor;

#[derive(Debug, Clone, PartialEq)]
pub struct KiCadDrcReport {
        pub json_schema: &'static str,
            pub coordinate_units: String,
                pub date: String,
            pub kicad_version: &'static str,
                pub freerouting_version: String,
            pub source: String,
                pub unconnected_items: Vec<KiCadDrcViolation>,
                pub violations: Vec<KiCadDrcViolation>,
                pub schematic_parity: Vec<serde_json::Value>,
                pub quality_score: Option<f64>,
}

impl KiCadDrcReport {
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

        pub fn add_violation(&mut self, violation: KiCadDrcViolation) {
        self.violations.push(violation);
    }

        pub fn add_unconnected_item(&mut self, item: KiCadDrcViolation) {
        self.unconnected_items.push(item);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KiCadDrcViolation {
        pub description: String,
            pub items: Vec<KiCadDrcViolationItem>,
                pub severity: &'static str,
                        pub kind: String,
}

impl KiCadDrcViolation {
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

#[derive(Debug, Clone, PartialEq)]
pub struct KiCadDrcViolationItem {
        pub description: String,
        pub pos: KiCadDrcPosition,
            pub uuid: String,
}

impl KiCadDrcViolationItem {
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KiCadDrcPosition {
        pub x: f64,
        pub y: f64,
}

impl KiCadDrcPosition {
        pub fn new(x: f64, y: f64) -> KiCadDrcPosition {
        KiCadDrcPosition { x, y }
    }
}
