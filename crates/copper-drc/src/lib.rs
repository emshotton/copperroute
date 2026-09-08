#![forbid(unsafe_code)]

pub mod airline;
pub mod checker;
pub mod checks;
pub mod constraints;
pub mod error;
pub mod net_incompletes;
pub mod plane_connectivity;
pub mod report;
pub mod statistics;
pub mod unconnected;
pub mod violation;

pub use airline::AirLine;
pub use checker::DesignRulesChecker;
pub use constraints::apply_kicad_project;
pub use error::DrcError;
pub use net_incompletes::{NetIncompletes, all_airlines};
pub use plane_connectivity::PlaneConnectivity;
pub use report::{
    DrcCoordinates, DrcReportOptions, KiCadDrcPosition, KiCadDrcReport, KiCadDrcViolation,
    KiCadDrcViolationItem,
};
pub use statistics::BoardStatisticsClearanceViolations;
pub use unconnected::{UnconnectedItems, UnconnectedKind};
pub use violation::{DrcViolation, DrcViolationKind};

pub use copper_board::DrcSeverity;

pub mod prelude {
    pub use crate::airline::AirLine;
    pub use crate::checker::DesignRulesChecker;
    pub use crate::error::DrcError;
    pub use crate::net_incompletes::NetIncompletes;
    pub use crate::plane_connectivity::PlaneConnectivity;
    pub use crate::report::{
        DrcCoordinates, DrcReportOptions, KiCadDrcPosition, KiCadDrcReport, KiCadDrcViolation,
        KiCadDrcViolationItem,
    };
    pub use crate::statistics::BoardStatisticsClearanceViolations;
    pub use crate::unconnected::{UnconnectedItems, UnconnectedKind};
    pub use crate::violation::{DrcViolation, DrcViolationKind};
    pub use copper_board::DrcSeverity;
}
