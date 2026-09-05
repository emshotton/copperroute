use fr_board::ItemId;
use fr_geometry::FloatPoint;

#[derive(Debug, Clone, PartialEq)]
pub struct AirLine {
    pub net_number: i32,
    pub from_item: ItemId,
    pub from_corner: FloatPoint,
    pub to_item: ItemId,
    pub to_corner: FloatPoint,
}

impl AirLine {
    pub fn new(
        net_number: i32,
        from_item: ItemId,
        from_corner: FloatPoint,
        to_item: ItemId,
        to_corner: FloatPoint,
    ) -> AirLine {
        AirLine {
            net_number,
            from_item,
            from_corner,
            to_item,
            to_corner,
        }
    }
}
