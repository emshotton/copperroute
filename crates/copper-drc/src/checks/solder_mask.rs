use std::collections::{BTreeMap, BTreeSet};

use copper_board::{Board, DrcConstraints, DrcSeverity, Item, ItemId};
use copper_geometry::{FloatLine, Shape, ShapeOps, TileShape};

use crate::checks::geometry::{candidates, gap_below, has_copper, item_shapes};
use crate::constraints::severity;
use crate::{DrcViolation, DrcViolationKind};

fn mask_base_shape(board: &Board, id: ItemId, layer: usize) -> Option<Shape> {
    let Item::Pin(pin) = board.get_item(id)? else {
        return None;
    };
    let ctx = board.ctx();
    pin.get_shape_on_layer(layer, &ctx).or_else(|| {
        (pin.solder_mask_expansion.contains_key(&layer)
            && pin.first_layer(&ctx) == pin.last_layer(&ctx))
        .then(|| pin.get_shape_on_layer(pin.first_layer(&ctx), &ctx))
        .flatten()
    })
}

fn mask_item_shapes(board: &mut Board, id: ItemId) -> Vec<(usize, TileShape)> {
    let mut shapes = item_shapes(board, id);
    if let Some(Item::Pin(pin)) = board.get_item(id) {
        for &layer in pin.solder_mask_expansion.keys() {
            if !shapes.iter().any(|(existing, _)| *existing == layer)
                && let Some(shape) = mask_base_shape(board, id, layer)
            {
                shapes.push((layer, shape.bounding_tile()));
            }
        }
    }
    shapes
}

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
    let shape = mask_base_shape(board, pad, layer)?;
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

fn pad_pair_gap(board: &Board, a: ItemId, b: ItemId, layer: usize) -> Option<f64> {
    let ctx = board.ctx();
    let core = |id| {
        let Item::Pin(pin) = board.get_item(id)? else {
            return None;
        };
        let shape = mask_base_shape(board, id, layer)?;
        if let Shape::Circle(circle) = shape {
            return Some((
                TileShape::Box(copper_geometry::IntBox::from_coords(
                    circle.center.x,
                    circle.center.y,
                    circle.center.x,
                    circle.center.y,
                )),
                f64::from(circle.radius),
            ));
        }
        let radius = pin.get_padstack(&ctx)?.round_rect_radius.unwrap_or(0.0);
        Some((shape.bounding_tile().offset(-radius), radius))
    };
    let (a, ar) = core(a)?;
    let (b, br) = core(b)?;
    let corners = b.corner_approx_arr();
    let gap = (0..corners.len())
        .map(|i| {
            segment_gap(
                &a,
                &FloatLine::new(corners[i], corners[(i + 1) % corners.len()]),
            )
        })
        .fold(f64::INFINITY, f64::min);
    Some(gap - ar - br)
}

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    let kind = DrcViolationKind::SolderMaskBridge;
    let severity = severity(constraints, kind);
    if severity == DrcSeverity::Ignore {
        return;
    }
    let web_width = constraints.solder_mask_min_width.unwrap_or(0).max(0);
    let mut seen = BTreeSet::new();
    let mut largest_margin: BTreeMap<usize, i32> = BTreeMap::new();
    let mut mask_only_pins: BTreeMap<usize, Vec<ItemId>> = BTreeMap::new();
    for id in board.get_pins() {
        if let Some(Item::Pin(pin)) = board.get_item(id) {
            for (&layer, &margin) in &pin.effective_solder_mask_expansion {
                let largest = largest_margin.entry(layer).or_default();
                *largest = (*largest).max(margin);
            }
            for (&layer, &margin) in &pin.solder_mask_expansion {
                let largest = largest_margin.entry(layer).or_default();
                *largest = (*largest).max(margin);
                if pin.get_shape_on_layer(layer, &board.ctx()).is_none()
                    && mask_base_shape(board, id, layer).is_some()
                {
                    mask_only_pins.entry(layer).or_default().push(id);
                }
            }
        }
    }
    let clearance = constraints
        .solder_mask_to_copper_clearance
        .unwrap_or(0)
        .max(0);
    for pin_id in board.get_pins() {
        let Some(Item::Pin(pin)) = board.get_item(pin_id) else {
            continue;
        };
        if pin.allow_solder_mask_bridges
            || pin.solder_mask_expansion.is_empty()
            || !has_copper(board, pin_id)
        {
            continue;
        }
        let expansion = pin.solder_mask_expansion.clone();
        for (layer, copper) in mask_item_shapes(board, pin_id) {
            let Some(margin) = expansion.get(&layer) else {
                continue;
            };
            let aperture = copper.enlarge(f64::from(*margin));
            let mut nearby = candidates(
                board,
                &aperture,
                Some(layer),
                clearance.max(
                    largest_margin
                        .get(&layer)
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(web_width),
                ),
            );
            nearby.extend(mask_only_pins.get(&layer).into_iter().flatten().copied());
            for other_id in nearby {
                let Some(other) = board.get_item(other_id) else {
                    continue;
                };
                if other_id == pin_id
                    || !matches!(other, Item::Trace(_) | Item::Via(_) | Item::Pin(_))
                    || other.shares_net(board.get_item(pin_id).unwrap())
                {
                    continue;
                }
                let hole_only_source = !has_copper(board, other_id);
                if hole_only_source && !matches!(other, Item::Pin(_)) {
                    continue;
                }
                let mut other_has_aperture = false;
                let other_margin = if let Item::Pin(other_pin) = other {
                    if other_pin.allow_solder_mask_bridges {
                        continue;
                    }
                    let ctx = board.ctx();
                    let Item::Pin(pin) = board.get_item(pin_id).unwrap() else {
                        unreachable!()
                    };
                    let same_footprint = (pin.hdr.get_component_id() != 0
                        && board.get_item(pin_id).unwrap().component_id() == other.component_id())
                        || pin.source_footprint.as_ref().is_some_and(|source| {
                            Some(source) == other_pin.source_footprint.as_ref()
                        });
                    let number = pin.source_pad_number.as_deref().or_else(|| pin.name(&ctx));
                    let other_number = other_pin
                        .source_pad_number
                        .as_deref()
                        .or_else(|| other_pin.name(&ctx));
                    if same_footprint
                        && (board.rules.allow_solder_mask_bridges_in_footprints
                            || number
                                .is_some_and(|name| !name.is_empty() && Some(name) == other_number))
                    {
                        continue;
                    }
                    let aperture_margin = other_pin.solder_mask_expansion.get(&layer);
                    other_has_aperture = aperture_margin.is_some() && !hole_only_source;
                    if other_has_aperture {
                        aperture_margin.copied()
                    } else {
                        other_pin
                            .effective_solder_mask_expansion
                            .get(&layer)
                            .or(aperture_margin)
                            .copied()
                    }
                } else {
                    None
                };
                // KiCad excludes hole-only pads as apertures, but checks their
                // copper-layer shape against other pads' apertures.
                let required = if other_has_aperture {
                    web_width
                } else {
                    clearance
                };
                let expected = f64::from(
                    margin
                        .saturating_add(other_margin.unwrap_or(0))
                        .saturating_add(required),
                );
                let exact_gap = if matches!(other, Item::Pin(_)) {
                    pad_pair_gap(board, pin_id, other_id, layer)
                } else {
                    pad_routing_gap(board, pin_id, other_id, layer)
                };
                if exact_gap.is_some_and(|gap| gap >= expected && !(gap == 0.0 && expected == 0.0))
                {
                    continue;
                }
                for (other_layer, shape) in mask_item_shapes(board, other_id) {
                    if other_layer != layer {
                        continue;
                    }
                    let other_aperture = shape.enlarge(f64::from(other_margin.unwrap_or(0)));
                    let Some((_, position)) = gap_below(&aperture, &other_aperture, required)
                        .or_else(|| {
                            if required != 0 || expected != 0.0 || exact_gap != Some(0.0) {
                                return None;
                            }
                            let overlap = aperture.intersection(&other_aperture);
                            (overlap.dimension() >= 0).then(|| {
                                (
                                    0.0,
                                    TileShape::Box(overlap.bounding_box()).centre_of_gravity(),
                                )
                            })
                        })
                    else {
                        continue;
                    };
                    if !seen.insert((pin_id.min(other_id), pin_id.max(other_id), layer)) {
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
