use fr_geometry::PolylineError;
use thiserror::Error;

use crate::ids::ItemId;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BoardError {
            #[error("trace normalisation failed: {0}")]
    Normalization(#[from] PolylineError),

        #[error("invalid layer index {0}")]
    InvalidLayer(usize),

        #[error("unknown item id {0}")]
    UnknownItem(ItemId),

                                            #[error("normalisation was stopped by the caller's stop check")]
    Stopped,
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
