use fr_board::{Board, Item, ItemId, ItemKind, TreeObject};
use fr_geometry::{Circle, FloatPoint, Shape, ShapeOps, TileShape, java_round};

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
pub fn has_copper(board: &Board, id: ItemId) -> bool {
    let ctx = board.ctx();
    match board.get_item(id) {
        Some(Item::Pin(pin)) => !pin.get_padstack(&ctx).is_some_and(|p| p.hole_only),
        Some(item) => is_copper(item),
        None => false,
    }
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
            let estimated = padstack
                .drill_diameter
                .map_or(!padstack.name.contains(':'), |_| padstack.drill_estimated);
            Some(hole_from(
                via.get_center().to_float(),
                padstack.drill_radius(),
                estimated,
            ))
        }
        Item::Pin(pin) => {
            let padstack = pin.get_padstack(&ctx)?;
            if padstack.drill_diameter == Some(0.0)
                || (padstack.drill_diameter.is_none()
                    && pin.first_layer(&ctx) == pin.last_layer(&ctx))
            {
                return None;
            }
            let estimated = padstack
                .drill_diameter
                .map_or(!padstack.name.contains(':'), |_| padstack.drill_estimated);
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
    // The search tree enlarges pin and via shapes by the routing hole clearance;
    // DRC measures the copper itself.
    {
        let ctx = board.ctx();
        let copper_on = |layer: usize| match board.get_item(id) {
            Some(Item::Pin(pin)) => pin.get_shape_on_layer(layer, &ctx),
            Some(Item::Via(via)) => via.get_shape_on_layer(layer, &ctx),
            _ => None,
        };
        if matches!(board.get_item(id), Some(Item::Pin(_) | Item::Via(_))) {
            return layers
                .into_iter()
                .filter_map(|layer| Some((layer, copper_on(layer)?.bounding_tile())))
                .collect();
        }
    }
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

/// Physical point-to-copper distance, without routing margins or polygon inflation.
fn segment_nearest(p: FloatPoint, a: FloatPoint, b: FloatPoint) -> FloatPoint {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length2 = dx * dx + dy * dy;
    let t = if length2 == 0.0 {
        0.0
    } else {
        ((p.x - a.x) * dx + (p.y - a.y) * dy) / length2
    }
    .clamp(0.0, 1.0);
    FloatPoint::new(a.x + t * dx, a.y + t * dy)
}

fn rounded_rectangle_nearest(shape: &Shape, radius: f64, p: FloatPoint) -> Option<FloatPoint> {
    let corners = shape.corner_approx_arr();
    if corners.len() != 4 {
        return None;
    }
    let center = FloatPoint::new(
        corners.iter().map(|p| p.x).sum::<f64>() / 4.0,
        corners.iter().map(|p| p.y).sum::<f64>() / 4.0,
    );
    let width = corners[0].distance(&corners[1]);
    let height = corners[1].distance(&corners[2]);
    if width == 0.0 || height == 0.0 {
        return None;
    }
    let ux = (corners[1].x - corners[0].x) / width;
    let uy = (corners[1].y - corners[0].y) / width;
    let vx = (corners[2].x - corners[1].x) / height;
    let vy = (corners[2].y - corners[1].y) / height;
    let x = (p.x - center.x) * ux + (p.y - center.y) * uy;
    let y = (p.x - center.x) * vx + (p.y - center.y) * vy;
    let hx = (width / 2.0 - radius).max(0.0);
    let hy = (height / 2.0 - radius).max(0.0);
    let qx = x.clamp(-hx, hx);
    let qy = y.clamp(-hy, hy);
    let q = FloatPoint::new(center.x + ux * qx + vx * qy, center.y + uy * qx + vy * qy);
    let distance = p.distance(&q);
    if distance <= radius {
        return Some(p);
    }
    Some(FloatPoint::new(
        q.x + (p.x - q.x) * radius / distance,
        q.y + (p.y - q.y) * radius / distance,
    ))
}

pub fn hole_copper_gap(
    board: &Board,
    copper_id: ItemId,
    layer: usize,
    hole: &Hole,
    clearance: i32,
) -> Option<(f64, FloatPoint)> {
    let ctx = board.ctx();
    let item = board.get_item(copper_id)?;
    let p = hole.center;
    let (nearest, copper_radius) = match item {
        Item::Trace(trace) => {
            let corners = trace.polyline().corner_approx_arr();
            let nearest = corners
                .windows(2)
                .map(|s| segment_nearest(p, s[0], s[1]))
                .min_by(|a, b| a.distance(&p).total_cmp(&b.distance(&p)))?;
            (nearest, f64::from(trace.get_half_width()))
        }
        _ => {
            let (shape, radius) = match item {
                Item::Pin(pin) => (
                    pin.get_shape_on_layer(layer, &ctx)?,
                    pin.get_padstack(&ctx)?.round_rect_radius,
                ),
                Item::Via(via) => (via.get_shape_on_layer(layer, &ctx)?, None),
                _ => return None,
            };
            if let Shape::Circle(circle) = &shape {
                (circle.center.to_float(), f64::from(circle.radius))
            } else if let Some(radius) = radius {
                (rounded_rectangle_nearest(&shape, radius, p)?, 0.0)
            } else if shape.contains_float(&p) {
                (p, 0.0)
            } else {
                let corners = shape.corner_approx_arr();
                let nearest = (0..corners.len())
                    .map(|i| segment_nearest(p, corners[i], corners[(i + 1) % corners.len()]))
                    .min_by(|a, b| a.distance(&p).total_cmp(&b.distance(&p)))?;
                (nearest, 0.0)
            }
        }
    };
    let distance = p.distance(&nearest);
    let signed_gap = distance - copper_radius - hole.radius;
    let actual = signed_gap.max(0.0);
    if signed_gap >= f64::from(clearance) {
        return None;
    }
    let position = if distance > 0.0 {
        let along = (hole.radius + actual / 2.0) / distance;
        FloatPoint::new(
            p.x + (nearest.x - p.x) * along,
            p.y + (nearest.y - p.y) * along,
        )
    } else {
        p
    };
    Some((actual, position))
}
