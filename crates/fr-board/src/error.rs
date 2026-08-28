//! The board-level error type.

use fr_geometry::PolylineError;
use thiserror::Error;

use crate::ids::ItemId;

/// Errors `fr-board`'s operations can produce.
///
/// Per `global-constraints.md`: reproduced Java bugs get a `// Java bug:` marker instead of an
/// error variant (they are reproduced, not surfaced), and a crash-to-value totalization is only
/// allowed where no reachable Java caller observes the difference — otherwise, as here, a
/// `Result`.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BoardError {
    /// Trace normalisation hit `Polyline::from_lines`'s failure mode. Per `global-constraints.md`
    /// this must propagate out of trace normalisation, never be swallowed into an empty trace.
    #[error("trace normalisation failed: {0}")]
    Normalization(#[from] PolylineError),

    /// A layer index outside `0..LayerStructure::count()`.
    #[error("invalid layer index {0}")]
    InvalidLayer(usize),

    /// An [`ItemId`] not present in `Board::items`.
    #[error("unknown item id {0}")]
    UnknownItem(ItemId),
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::PolylineError;

    #[test]
    fn normalization_wraps_polyline_error_via_from() {
        let err: BoardError = PolylineError::NormalizationIndexUnderflow.into();
        assert_eq!(
            err,
            BoardError::Normalization(PolylineError::NormalizationIndexUnderflow)
        );
    }

    #[test]
    fn invalid_layer_displays_index() {
        assert_eq!(
            BoardError::InvalidLayer(7).to_string(),
            "invalid layer index 7"
        );
    }

    #[test]
    fn unknown_item_displays_id() {
        assert_eq!(
            BoardError::UnknownItem(ItemId(3)).to_string(),
            "unknown item id 3"
        );
    }
}
