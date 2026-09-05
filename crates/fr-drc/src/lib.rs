#![forbid(unsafe_code)]

pub mod airline;
pub mod checker;
pub mod constraints;
pub mod error;
pub mod net_incompletes;
pub mod report;
pub mod statistics;
pub mod unconnected;
pub mod violation;

pub use airline::AirLine;
pub use checker::DesignRulesChecker;
pub use error::DrcError;
pub use net_incompletes::NetIncompletes;
pub use report::{
    DrcCoordinates, DrcJsonFlavor, DrcReportOptions, KiCadDrcPosition, KiCadDrcReport,
    KiCadDrcViolation, KiCadDrcViolationItem,
};
pub use statistics::BoardStatisticsClearanceViolations;
pub use unconnected::{UnconnectedItems, UnconnectedKind};
pub use violation::{DrcViolation, DrcViolationKind};

pub use fr_board::ClearanceViolation;
pub use fr_board::DrcSeverity;

pub mod prelude {
    pub use crate::airline::AirLine;
    pub use crate::checker::DesignRulesChecker;
    pub use crate::error::DrcError;
    pub use crate::net_incompletes::NetIncompletes;
    pub use crate::report::{
        DrcCoordinates, DrcJsonFlavor, DrcReportOptions, KiCadDrcPosition, KiCadDrcReport,
        KiCadDrcViolation, KiCadDrcViolationItem,
    };
    pub use crate::statistics::BoardStatisticsClearanceViolations;
    pub use crate::unconnected::{UnconnectedItems, UnconnectedKind};
    pub use crate::violation::{DrcViolation, DrcViolationKind};
    pub use fr_board::ClearanceViolation;
    pub use fr_board::DrcSeverity;
}
