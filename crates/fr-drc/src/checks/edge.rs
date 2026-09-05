use fr_board::{Board, DrcConstraints, DrcSeverity, Item, ItemId};
use fr_geometry::TileShape;

use crate::checks::geometry::{gap_below, is_copper, item_shapes};
use crate::constraints::severity;
use crate::{DrcViolation, DrcViolationKind};

fn keepout_pieces(board: &Board, outline: ItemId) -> Vec<TileShape> {
    let ctx = board.ctx();
    match board.get_item(outline) {
        Some(Item::BoardOutline(item)) => {
            item.get_keepout_area(&ctx);
            item.keepout_convex_pieces(&ctx)
                .map(<[TileShape]>::to_vec)
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

pub fn run(board: &mut Board, constraints: &DrcConstraints, out: &mut Vec<DrcViolation>) {
    let Some(minimum) = constraints.copper_edge_clearance else {
        return;
    };
    let severity = severity(constraints, DrcViolationKind::CopperEdgeClearance);
    if severity == DrcSeverity::Ignore {
        return;
    }
    let Some(outline) = board.get_outline() else {
        return;
    };
    let pieces = keepout_pieces(board, outline);
    if pieces.is_empty() {
        return;
    }
    let ids: Vec<ItemId> = board
        .items_in_board_order()
        .into_iter()
        .filter(|id| board.get_item(*id).is_some_and(is_copper))
        .collect();
    for id in ids {
        for (layer, shape) in item_shapes(board, id) {
            let mut worst: Option<(f64, fr_geometry::FloatPoint)> = None;
            for piece in &pieces {
                if let Some((actual, position)) = gap_below(&shape, piece, minimum)
                    && worst.is_none_or(|(best, _)| actual < best)
                {
                    worst = Some((actual, position));
                }
            }
            if let Some((actual, position)) = worst {
                out.push(DrcViolation {
                    kind: DrcViolationKind::CopperEdgeClearance,
                    severity,
                    first_item: id,
                    second_item: Some(outline),
                    layer: Some(layer),
                    position,
                    expected: f64::from(minimum),
                    actual,
                    estimated: false,
                });
            }
        }
    }
}
