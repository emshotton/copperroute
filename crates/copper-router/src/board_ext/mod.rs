mod drill_item_mover;
mod forced_pad_router;
mod forced_via_inserter;
mod routing_board_ext;
pub mod tightener;
mod trace_shover;
mod via_optimizer;

pub use drill_item_mover::DrillItemMover;
pub use forced_pad_router::{CheckDrillResult, ForcedPadRouter};
pub use forced_via_inserter::ForcedViaInserter;
pub use routing_board_ext::{
    RoutingBoardExt, combined_fallback_via_rule, sorted_unconnected_targets,
};
pub use tightener::{
    PolylineTraceExt, TraceTightener, TraceTightener45, TraceTightener90, TraceTightenerAnyAngle,
};
pub use trace_shover::{SpringOverOutcome, TraceShover};
pub use via_optimizer::ViaOptimizer;

pub(crate) fn swallow_normalize_error(
    result: Result<bool, copper_board::BoardError>,
) -> Result<(), copper_board::BoardError> {
    match result {
        Err(copper_board::BoardError::Stopped) => Err(copper_board::BoardError::Stopped),
        Ok(_) | Err(_) => Ok(()),
    }
}
