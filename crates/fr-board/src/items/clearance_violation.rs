use fr_geometry::TileShape;

use crate::ids::ItemId;

#[derive(Debug, Clone, PartialEq)]
pub struct ClearanceViolation {
            pub first_item: ItemId,
        pub second_item: ItemId,
                        pub shape: TileShape,
        pub layer: usize,
            pub expected_clearance: f64,
            pub actual_clearance: f64,
}
