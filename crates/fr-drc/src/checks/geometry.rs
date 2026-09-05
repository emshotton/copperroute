use fr_board::{Board, Item, ItemId, ItemKind, TreeObject};
use fr_geometry::{Circle, FloatPoint, TileShape, java_round};

pub struct Hole {
    pub shape: TileShape,
    pub radius: f64,
    pub estimated: bool,
    pub center: FloatPoint,
}

/// KiCad's `DRC_TEST_PROVIDER_COPPER_CLEARANCE::sub_e` (and the hole-to-hole and
/// edge-clearance providers' matching subtractions): the clearance requirement a gap test
/// actually enforces, with `constraints.epsilon` removed so a gap exactly at the nominal
/// clearance is not flagged by integer rounding.
#[must_use]
pub fn sub_epsilon(clearance: i32, epsilon: i32) -> i32 {
    (clearance - epsilon).max(0)
}

#[must_use]
pub fn gap_below(a: &TileShape, b: &TileShape, clearance: i32) -> Option<(f64, FloatPoint)> {
    let half = f64::from(clearance) / 2.0;
    let (ea, eb) = if clearance > 0 {
        (a.enlarge(half), b.enlarge(half))
    } else {
        (a.clone(), b.clone())
    };
    let overlap = ea.intersection(&eb);
    if overlap.dimension() != 2 {
        return None;
    }
    let position = overlap.centre_of_gravity();
    if a.intersection(b).dimension() == 2 {
        return Some((0.0, position));
    }
    let actual = Board::calculate_clearance_between_two_shapes(a, b, f64::from(clearance), 0, 0);
    Some((actual, position))
}

#[must_use]
pub fn is_copper(item: &Item) -> bool {
    matches!(item.kind(), ItemKind::Trace | ItemKind::Via | ItemKind::Pin)
}

#[must_use]
pub fn is_through_hole_pin(board: &Board, id: ItemId) -> bool {
    let ctx = board.ctx();
    match board.get_item(id) {
        Some(Item::Pin(pin)) => pin.first_layer(&ctx) != pin.last_layer(&ctx),
        _ => false,
    }
}

#[must_use]
pub fn is_microvia(board: &Board, id: ItemId) -> bool {
    let ctx = board.ctx();
    let Some(Item::Via(via)) = board.get_item(id) else {
        return false;
    };
    let Some(padstack) = via.get_padstack(&ctx) else {
        return false;
    };
    let last = padstack.board_layer_count() as i32 - 1;
    let (from, to) = (padstack.from_layer(), padstack.to_layer());
    to - from == 1 && (from == 0 || to == last) && last > 1
}

fn hole_from(center: FloatPoint, radius: f64, estimated: bool) -> Hole {
    let circle = Circle::new(center.round(), java_round(radius) as i32);
    Hole {
        shape: TileShape::Octagon(circle.bounding_octagon()),
        radius,
        estimated,
        center,
    }
}

#[must_use]
pub fn hole_of(board: &Board, id: ItemId) -> Option<Hole> {
    let ctx = board.ctx();
    match board.get_item(id)? {
        Item::Via(via) => {
            let padstack = via.get_padstack(&ctx)?;
            let estimated = !padstack.name.contains(':');
            Some(hole_from(
                via.get_center().to_float(),
                padstack.drill_radius(),
                estimated,
            ))
        }
        Item::Pin(pin) => {
            if pin.first_layer(&ctx) == pin.last_layer(&ctx) {
                return None;
            }
            let padstack = pin.get_padstack(&ctx)?;
            let estimated = !padstack.name.contains(':');
            Some(hole_from(
                pin.get_center(&ctx).to_float(),
                padstack.drill_radius(),
                estimated,
            ))
        }
        _ => None,
    }
}

#[must_use]
pub fn item_shapes(board: &mut Board, id: ItemId) -> Vec<(usize, TileShape)> {
    let layers: Vec<usize> = {
        let ctx = board.ctx();
        let Some(item) = board.get_item(id) else {
            return Vec::new();
        };
        (0..item.tile_shape_count(&ctx))
            .map(|i| item.shape_layer(i, &ctx))
            .collect()
    };
    layers
        .into_iter()
        .enumerate()
        .filter_map(|(i, layer)| board.item_tile_shape(id, i).map(|shape| (layer, shape)))
        .collect()
}

#[must_use]
pub fn candidates(
    board: &Board,
    shape: &TileShape,
    layer: Option<usize>,
    radius: i32,
) -> Vec<ItemId> {
    let query = if radius > 0 {
        shape.enlarge(f64::from(radius))
    } else {
        shape.clone()
    };
    board
        .overlapping_objects(&query, layer)
        .into_iter()
        .filter_map(|object| match object {
            TreeObject::Item(id) => Some(id),
            TreeObject::Room(_) => None,
        })
        .collect()
}

#[must_use]
pub fn item_position(board: &Board, id: ItemId) -> FloatPoint {
    let ctx = board.ctx();
    match board.get_item(id) {
        Some(item) => TileShape::Box(item.bounding_box(&ctx)).centre_of_gravity(),
        None => FloatPoint::new(0.0, 0.0),
    }
}
