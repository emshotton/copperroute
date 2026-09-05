mod connection;
pub mod inserter;
pub mod locator;
pub(crate) mod locator_45;
pub(crate) mod locator_any_angle;

pub use connection::Connection;
pub use inserter::FoundConnectionInserter;
pub use locator::{
    BacktrackElement, FoundConnectionLocator, LocatorKind, ResultItem, calculate_additional_corner,
};
