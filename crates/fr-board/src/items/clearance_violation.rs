//! `drc.ClearanceViolation`: one clearance violation between two board items.

use fr_geometry::TileShape;

use crate::ids::ItemId;

/// Port of `drc.ClearanceViolation` (ClearanceViolation.java:13-42).
///
/// Declared here, in `fr-board`, because [`Board::clearance_violations`](crate::Board::clearance_violations)
/// returns it and `fr-board` cannot depend upward on `fr-drc` (plan-5 ruling 9); `fr_drc`
/// re-exports it, so `fr_drc::ClearanceViolation` is the name callers use.
///
/// Java holds `Item` references; this holds [`ItemId`]s, per `global-constraints.md`'s
/// no-back-pointer rule.
///
/// The two headless statics of the Java class are [`Board`](crate::Board) methods, because they
/// walk a board's items and mutate them through `Item.clearanceViolations`:
///
// renamed: ClearanceViolation.aggregateSortedBySeverity (ClearanceViolation.java:64-75) -> `Board::aggregate_violations_sorted_by_severity` (`board/clearance.rs`), which supplies Java's `board.getItems()` argument itself.
// renamed: ClearanceViolation.smallestClearance (ClearanceViolation.java:86-94) -> `Board::smallest_clearance` (`board/clearance.rs`), same reason.
//
// not ported: ClearanceViolation.printInfo (ClearanceViolation.java:96-121) — it renders the violation into an `ItemInfoPrinter` through a localized `TextManager`; this crate is headless.
#[derive(Debug, Clone, PartialEq)]
pub struct ClearanceViolation {
    /// Java `firstItem` (ClearanceViolation.java:16). Always the queried item
    /// (Item.java:365-366, `:457`).
    pub first_item: ItemId,
    /// Java `secondItem` (ClearanceViolation.java:19).
    pub second_item: ItemId,
    /// Java `shape` (ClearanceViolation.java:22): the **intersection** of the two enlarged
    /// shapes (Item.java:445, `:459`), not either item's own shape.
    ///
    /// Java's field is a `ConvexShape`; every construction passes the `TileShape` that
    /// `TileShape.intersection` returns.
    pub shape: TileShape,
    /// Java `layer` (ClearanceViolation.java:25): `this.shapeLayer(i)` (Item.java:460).
    pub layer: usize,
    /// Java `expectedClearance` (ClearanceViolation.java:27): the clearance-matrix value
    /// (Item.java:424-425).
    pub expected_clearance: f64,
    /// Java `actualClearance` (ClearanceViolation.java:28): the bisection's answer
    /// (Item.java:447-449).
    pub actual_clearance: f64,
}
