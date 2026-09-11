use std::collections::BTreeSet;

use copper_board::{Board, DrcConstraints, DrcSeverity, Item, ItemId};
use copper_geometry::{FloatLine, FloatPoint, TileShape};

use crate::checks::geometry::{
    candidates, circle_copper_gap, circular_copper, gap_below, has_copper, hole_copper_gap,
    hole_of, is_copper, item_shapes, rounded_pad_trace_gap, sub_epsilon,
};
use crate::constraints::{pair_clearance, search_radius, severity};
use crate::{DrcViolation, DrcViolationKind};

type PairKey = (u32, u32, Option<usize>, DrcViolationKind);

fn ordered(a: ItemId, b: ItemId) -> (ItemId, ItemId) {
    if a.0 <= b.0 { (a, b) } else { (b, a) }
}

fn same_defined_net(a: &Item, b: &Item) -> bool {
    a.net_count() > 0 && a.shares_net(b)
}

fn logical_pad_name(name: &str) -> &str {
    name.split('@').next().unwrap_or(name)
}

fn same_logical_pad(board: &Board, a: &Item, b: &Item) -> bool {
    let ctx = board.ctx();
    match (a, b) {
        (Item::Pin(pa), Item::Pin(pb)) => {
            a.component_id() == b.component_id()
                && match (pa.name(&ctx), pb.name(&ctx)) {
                    (Some(na), Some(nb)) => logical_pad_name(na) == logical_pad_name(nb),
                    _ => false,
                }
        }
        _ => false,
    }
}

fn segments(item: &Item) -> Vec<FloatLine> {
    let Item::Trace(trace) = item else {
        return Vec::new();
    };
    let corners = trace.polyline().corner_approx_arr();
    corners
        .windows(2)
        .map(|pair| FloatLine::new(pair[0], pair[1]))
        .collect()
}

fn crossing_point(a: &Item, b: &Item) -> Option<FloatPoint> {
    for sa in segments(a) {
        for sb in segments(b) {
            let Some(point) = sa.intersection(&sb) else {
                continue;
            };
            if sa.segment_distance(&point) < 0.5 && sb.segment_distance(&point) < 0.5 {
                return Some(point);
            }
        }
    }
    None
}

struct Emit<'a> {
    constraints: &'a DrcConstraints,
    seen: BTreeSet<PairKey>,
    measured_holes: BTreeSet<(ItemId, ItemId, usize)>,
    out: &'a mut Vec<DrcViolation>,
}

impl Emit<'_> {
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        kind: DrcViolationKind,
        a: ItemId,
        b: ItemId,
        layer: Option<usize>,
        position: FloatPoint,
        expected: f64,
        actual: f64,
        estimated: bool,
    ) {
        let (first, second) = ordered(a, b);
        if !self.seen.insert((first.0, second.0, layer, kind)) {
            return;
        }
        let severity = severity(self.constraints, kind);
        if severity == DrcSeverity::Ignore {
            return;
        }
        self.out.push(DrcViolation {
            kind,
            severity,
            first_item: first,
            second_item: Some(second),
            layer,
            position,
            expected,
            actual,
            estimated,
        });
    }
}

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    let radius = search_radius(constraints);
    let mut emit = Emit {
        constraints,
        seen: BTreeSet::new(),
        measured_holes: BTreeSet::new(),
        out,
    };
    let ids: Vec<ItemId> = board
        .items_in_board_order()
        .into_iter()
        .filter(|id| board.get_item(*id).is_some_and(is_copper))
        .collect();

    for &id in &ids {
        let shapes = item_shapes(board, id);
        for (layer, shape) in &shapes {
            let others = candidates(board, shape, Some(*layer), radius);
            for other in others {
                if other.0 <= id.0 {
                    continue;
                }
                if !board.get_item(other).is_some_and(is_copper) {
                    continue;
                }
                check_pair(board, constraints, &mut emit, id, other, *layer, shape);
            }
        }
    }
}

fn check_pair(
    board: &mut Board,
    constraints: &DrcConstraints,
    emit: &mut Emit<'_>,
    id: ItemId,
    other: ItemId,
    layer: usize,
    shape: &TileShape,
) {
    let (same_net, same_pad, crossing, clearance) = {
        let a = &board.items[&id];
        let b = &board.items[&other];
        let same_net = same_defined_net(a, b);
        let same_pad = same_logical_pad(board, a, b);
        let crossing = if same_net { None } else { crossing_point(a, b) };
        (
            same_net,
            same_pad,
            crossing,
            pair_clearance(board, constraints, a, b),
        )
    };

    if let Some(point) = crossing {
        emit.push(
            DrcViolationKind::TracksCrossing,
            id,
            other,
            Some(layer),
            point,
            0.0,
            0.0,
            false,
        );
        return;
    }

    let other_shapes: Vec<TileShape> = item_shapes(board, other)
        .into_iter()
        .filter(|(l, _)| *l == layer)
        .map(|(_, s)| s)
        .collect();

    if !same_net
        && has_copper(board, id)
        && has_copper(board, other)
        && let Some(clearance) = clearance
        && clearance > 0
    {
        for other_shape in &other_shapes {
            let required = sub_epsilon(clearance, constraints.epsilon);
            let circular = circular_copper(board, other, layer)
                .map(|circle| (id, circle))
                .or_else(|| circular_copper(board, id, layer).map(|circle| (other, circle)));
            let gap = if let Some((copper_id, circle)) = circular {
                circle_copper_gap(
                    board,
                    copper_id,
                    layer,
                    circle.center.to_float(),
                    f64::from(circle.radius),
                    required,
                )
            } else {
                gap_below(shape, other_shape, required)
            };
            if let Some((actual, position)) = gap {
                let actual = if let Some(gap) = rounded_pad_trace_gap(board, id, other, layer) {
                    if gap >= f64::from(sub_epsilon(clearance, constraints.epsilon)) {
                        continue;
                    }
                    gap.max(0.0)
                } else {
                    actual
                };
                let both_netted =
                    board.items[&id].net_count() > 0 && board.items[&other].net_count() > 0;
                let kind = if actual == 0.0 && both_netted {
                    DrcViolationKind::ShortingItems
                } else {
                    DrcViolationKind::Clearance
                };
                emit.push(
                    kind,
                    id,
                    other,
                    Some(layer),
                    position,
                    f64::from(clearance),
                    actual,
                    false,
                );
                break;
            }
        }
    }

    if same_net || same_pad {
        return;
    }
    let Some(hole_clearance) = constraints.hole_clearance else {
        return;
    };
    for (copper_id, hole_id) in [(id, other), (other, id)] {
        if !has_copper(board, copper_id) {
            continue;
        }
        let Some(hole) = hole_of(board, hole_id) else {
            continue;
        };
        if !emit.measured_holes.insert((copper_id, hole_id, layer)) {
            continue;
        }
        if let Some((actual, position)) = hole_copper_gap(
            board,
            copper_id,
            layer,
            &hole,
            sub_epsilon(hole_clearance, constraints.epsilon),
        ) {
            emit.push(
                DrcViolationKind::HoleClearance,
                copper_id,
                hole_id,
                Some(layer),
                position,
                f64::from(hole_clearance),
                actual,
                hole.estimated,
            );
        }
    }
}
