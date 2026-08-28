//! The DSN/SES I/O error type, plus the small `io/*.java` result and metadata types that ride
//! alongside it: `BoardReadResult`, `BoardMetadata`, `FileFormat`.

use fr_board::{AngleRestriction, Board, BoardError, Unit};

/// Errors `fr-dsn`'s reading and writing operations can produce.
///
/// Per the plan's Global Constraints (`docs/superpowers/plans/2026-08-28-plan-3-dsn.md`):
/// reproduced Java bugs get a `// Java bug:` marker instead of an error variant (they are
/// reproduced, not surfaced). More variants are added as later Plan 3 tasks port the scopes
/// that can fail in ways not yet representable here (e.g. a lexer/parse failure with location
/// info).
#[derive(Debug, thiserror::Error)]
pub enum DsnError {
    /// An I/O error on the underlying stream.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// The input ended before a scope that was opened could be closed.
    #[error("unexpected end of file: expected {scope}")]
    UnexpectedEof {
        /// The name of the scope that was still open when input ran out.
        scope: String,
    },

    /// A `fr-board` operation invoked while building or reading the board failed.
    #[error(transparent)]
    Board(#[from] BoardError),
}

/// Supported board and routing file formats (`io/FileFormat.java`, verbatim).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// `UNKNOWN`.
    Unknown,
    /// `DSN`.
    Dsn,
    /// `FRB`.
    Frb,
    /// `SES`.
    Ses,
    /// `RULES`.
    Rules,
    /// `SCR`.
    Scr,
    /// `DRC_JSON`.
    DrcJson,
    /// `KICAD_DESIGN_JSON`.
    KicadDesignJson,
    /// `KICAD_SESSION_JSON`.
    KicadSessionJson,
}

/// Lightweight, immutable snapshot of the information that can be extracted from a board design
/// file's header and structure sections without constructing a full [`Board`]
/// (`io/BoardMetadata.java`).
///
/// `router_settings`'s type is a forward reference: `DsnRouterSettings`'s body arrives in Plan 3
/// Task 6 (plan ruling 5); [`crate::parser::DsnRouterSettings`] is a placeholder until then.
#[derive(Debug, Clone)]
pub struct BoardMetadata {
    /// The CAD tool that created the file (e.g. "KiCad", "EAGLE"); `null` in Java when absent.
    pub host_cad: Option<String>,
    /// Version of the CAD tool; `null` in Java when absent.
    pub host_version: Option<String>,
    /// Number of layers in the design.
    pub layer_count: usize,
    /// The design units (mm, mil, or um).
    pub unit: Unit,
    /// The resolution/scale factor for coordinate mapping.
    pub resolution: i32,
    /// Angle restriction for routing (e.g. 45°, 90°).
    pub snap_angle: AngleRestriction,
    /// Autoroute settings; only present when they were available to read.
    pub router_settings: Option<crate::parser::DsnRouterSettings>,
}

/// Sealed result type for all outcomes of a board read operation (DSN, JSON, or any other
/// format) — `io/BoardReadResult.java`, ported as a Rust `enum` verbatim (its Java `sealed
/// interface` + four `record` permits become the four variants below).
///
/// `Success.metadata` is `Option` because `DsnReader.readBoard` returns `Success` with a
/// **`null`** metadata — only `readMetadata` populates it. `OutlineMissing.board` is `Option`
/// because Java's may be `null` or partial.
///
/// Note: unlike the Java record, this variant does not yet carry a `CoordinateTransform` field
/// — that type does not exist until Plan 3 Task 5, and Task 10 is what adds it here.
#[derive(Debug)]
pub enum BoardReadResult {
    /// Full board + metadata are available. The board is fully constructed and routable.
    Success {
        /// The constructed board.
        board: Box<Board>,
        /// Header/structure metadata; `None` when the caller only wanted the board (Java's
        /// `null`).
        metadata: Option<BoardMetadata>,
        /// Non-fatal issues encountered during loading (e.g. degenerate wires, duplicate vias,
        /// missing nets). May be empty.
        warnings: Vec<String>,
    },
    /// The board was constructed but the outline (boundary) scope was absent from the input
    /// file. The board reference is still valid and may be used with caution.
    OutlineMissing {
        /// The board, if construction proceeded far enough to produce one.
        board: Option<Box<Board>>,
        /// Header/structure metadata; `None` when unavailable.
        metadata: Option<BoardMetadata>,
        /// Non-fatal issues encountered during loading. May be empty.
        warnings: Vec<String>,
    },
    /// The input did not conform to the expected grammar/format.
    ParseError {
        /// A human-readable description of where in the input the error was detected.
        location: String,
        /// A short description of the specific problem.
        detail: String,
    },
    /// An I/O error occurred while reading the underlying stream.
    IoError(std::io::Error),
}
