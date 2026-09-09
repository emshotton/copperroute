pub mod footprints;
pub mod outline;
pub mod structure;

use crate::kicad::NetClassJson;

pub fn default_net_class() -> NetClassJson {
    NetClassJson {
        viaInPadAllowed: None,
        name: Some("Default".to_string()),
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        netNames: Some(Vec::new()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcbError {
    pub section: String,
    pub message: String,
}

impl PcbError {
    pub fn new(section: &str, message: &str) -> PcbError {
        PcbError {
            section: section.to_string(),
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for PcbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PcbError {}
