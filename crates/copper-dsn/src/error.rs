use copper_board::{AngleRestriction, Board, BoardError, ItemId, Unit};

use crate::coordinate_transform::CoordinateTransform;

#[derive(Debug, thiserror::Error)]
pub enum DsnError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("lexer: {0}")]
    Scan(String),

    #[error(transparent)]
    Board(#[from] BoardError),

    #[error(
        "SesReader: not a Specctra session file — expected '(session <name>' header, got: {got}"
    )]
    NotASessionFile { got: String },

    #[error("conduction area {item:?}'s shape could not be split into convex pieces")]
    UnsplittableConductionArea { item: ItemId },

    #[error("a coordinate transform needs a finite, non-zero scale factor, not {scale_factor}")]
    InvalidScaleFactor { scale_factor: f64 },

    #[error("{0}")]
    KicadSession(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Unknown,
    Dsn,
    Frb,
    Ses,
    Rules,
    Scr,
    DrcJson,
    KicadDesignJson,
    KicadSessionJson,
}

#[derive(Debug, Clone)]
pub struct BoardMetadata {
    pub host_cad: Option<String>,
    pub host_version: Option<String>,
    pub layer_count: usize,
    pub unit: Unit,
    pub resolution: i32,
    pub snap_angle: AngleRestriction,
    pub router_settings: Option<crate::parser::DsnRouterSettings>,
}

#[derive(Debug)]
pub enum BoardReadResult {
    Success {
        board: Option<Box<Board>>,
        metadata: Option<BoardMetadata>,
        warnings: Vec<String>,
        coordinate_transform: Option<CoordinateTransform>,
    },
    OutlineMissing {
        board: Option<Box<Board>>,
        metadata: Option<BoardMetadata>,
        warnings: Vec<String>,
        coordinate_transform: Option<CoordinateTransform>,
    },
    Partial {
        board: Option<Box<Board>>,
        metadata: Option<BoardMetadata>,
        warnings: Vec<String>,
        coordinate_transform: Option<CoordinateTransform>,
        diagnostic: String,
    },
    ParseError {
        location: String,
        detail: String,
    },
    IoError(std::io::Error),
}
