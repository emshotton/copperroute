use std::collections::BTreeSet;

use copper_board::{Board, DrcConstraints, DrcSeverity, Item, ItemId};
use copper_geometry::{FloatLine, Shape, ShapeOps, TileShape};

use crate::checks::geometry::{candidates, gap_below, item_shapes};
use crate::constraints::severity;
use crate::{DrcViolation, DrcViolationKind};

fn segment_gap(tile: &TileShape, segment: &FloatLine) -> f64 {
    if tile.contains_float(&segment.a) || tile.contains_float(&segment.b) {
        return 0.0;
    }
    let corners = tile.corner_approx_arr();
    let mut distance = f64::INFINITY;
    for index in 0..corners.len() {
        let edge = FloatLine::new(corners[index], corners[(index + 1) % corners.len()]);
        if let Some(point) = edge.intersection(segment)
            && edge.segment_distance(&point) < 1e-6
            && segment.segment_distance(&point) < 1e-6
        {
            return 0.0;
        }
        distance = distance
            .min(edge.segment_distance(&segment.a))
            .min(edge.segment_distance(&segment.b))
            .min(segment.segment_distance(&edge.a))
            .min(segment.segment_distance(&edge.b));
    }
    distance
}

fn pad_routing_gap(board: &Board, pad: ItemId, other: ItemId, layer: usize) -> Option<f64> {
    let Some(Item::Pin(pin)) = board.get_item(pad) else {
        return None;
    };
    let ctx = board.ctx();
    let shape = pin.get_shape_on_layer(layer, &ctx)?;
    let (segments, half_width) = match board.get_item(other)? {
        Item::Trace(trace) => (
            trace
                .polyline()
                .corner_approx_arr()
                .windows(2)
                .map(|p| FloatLine::new(p[0], p[1]))
                .collect::<Vec<_>>(),
            f64::from(trace.get_half_width()),
        ),
        Item::Via(via) => {
            let Shape::Circle(circle) = via.get_shape_on_layer(layer, &ctx)? else {
                return None;
            };
            let center = circle.center.to_float();
            (
                vec![FloatLine::new(center, center)],
                f64::from(circle.radius),
            )
        }
        _ => return None,
    };
    if let Shape::Circle(circle) = shape {
        let center = circle.center.to_float();
        let distance = segments
            .iter()
            .map(|s| s.segment_distance(&center))
            .fold(f64::INFINITY, f64::min);
        return Some(distance - f64::from(circle.radius) - half_width);
    }
    let radius = pin.get_padstack(&ctx)?.round_rect_radius.unwrap_or(0.0);
    let tile = shape.bounding_tile().offset(-radius);
    let distance = segments
        .iter()
        .map(|segment| segment_gap(&tile, segment))
        .fold(f64::INFINITY, f64::min);
    Some(distance - radius - half_width)
}

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    let kind = DrcViolationKind::SolderMaskBridge;
    let severity = severity(constraints, kind);
    if severity == DrcSeverity::Ignore {
        return;
    }
    let mut seen = BTreeSet::new();
    let clearance = constraints
        .solder_mask_to_copper_clearance
        .unwrap_or(0)
        .max(0);
    for pin_id in board.get_pins() {
        let Some(Item::Pin(pin)) = board.get_item(pin_id) else {
            continue;
        };
        if pin.allow_solder_mask_bridges
            || pin.hdr.net_count() == 0
            || pin.solder_mask_expansion.is_empty()
        {
            continue;
        }
        let expansion = pin.solder_mask_expansion.clone();
        for (layer, copper) in item_shapes(board, pin_id) {
            let Some(margin) = expansion.get(&layer) else {
                continue;
            };
            let aperture = copper.enlarge(f64::from(*margin));
            for other_id in candidates(board, &aperture, Some(layer), clearance) {
                let Some(other) = board.get_item(other_id) else {
                    continue;
                };
                if !matches!(other, Item::Trace(_) | Item::Via(_))
                    || other.net_count() == 0
                    || other.shares_net(board.get_item(pin_id).unwrap())
                {
                    continue;
                }
                let expected = f64::from(margin.saturating_add(clearance));
                let exact_gap = pad_routing_gap(board, pin_id, other_id, layer);
                if exact_gap.is_some_and(|gap| gap >= expected) {
                    continue;
                }
                for (other_layer, shape) in item_shapes(board, other_id) {
                    if other_layer != layer {
                        continue;
                    }
                    let Some((_, position)) = gap_below(&aperture, &shape, clearance) else {
                        continue;
                    };
                    if !seen.insert((pin_id, other_id, layer)) {
                        continue;
                    }
                    let actual = exact_gap.unwrap_or_else(|| {
                        Board::calculate_clearance_between_two_shapes(
                            &copper,
                            &shape,
                            expected.max(0.0),
                            0,
                            0,
                        )
                    });
                    out.push(DrcViolation {
                        kind,
                        severity,
                        first_item: pin_id,
                        second_item: Some(other_id),
                        layer: Some(layer),
                        position,
                        expected,
                        actual,
                        estimated: false,
                    });
                }
            }
        }
    }
}
