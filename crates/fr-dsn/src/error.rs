//! The DSN/SES I/O error type, plus the small `io/*.java` result and metadata types that ride
//! alongside it: `BoardReadResult`, `BoardMetadata`, `FileFormat`.

use fr_board::{AngleRestriction, Board, BoardError, ItemId, Unit};

use crate::coordinate_transform::CoordinateTransform;

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

    /// The input does not fit Java's fixed 16 MiB lexer buffer.
    ///
    /// Java allocates `zzBuffer` once as a `char[16 * 1024 * 1024]`
    /// (`SpecctraDsnStreamReader.java:40`) and its hand-rolled `nextString` indexes that buffer
    /// with no refill, so a larger file is mis-lexed rather than rejected; the port refuses it
    /// instead (see [`crate::lexer::DsnScanner::new`]).
    #[error("input is {units} UTF-16 code units, which exceeds the {limit}-unit lexer buffer")]
    InputTooLarge {
        /// The input's length in UTF-16 code units.
        units: usize,
        /// `ZZ_BUFFERSIZE`.
        limit: usize,
    },

    /// The scanner could not match the input, or a numeric literal did not fit its Java type.
    ///
    /// Java throws here — `Error("Error: could not match input")` from `zzScanError`
    /// (`SpecctraDsnStreamReader.java:833`), or a `NumberFormatException` out of
    /// `Integer.valueOf`/`Double.valueOf`.
    #[error("lexer: {0}")]
    Scan(String),

    /// A `fr-board` operation invoked while building or reading the board failed.
    #[error(transparent)]
    Board(#[from] BoardError),

    /// The stream did not start with a `(session <name>` header — `SesReader.read`'s
    /// `IOException` (SesReader.java:129-132), whose message this reproduces.
    ///
    /// Java raises it from `processSessionScope`'s three-token check and lets it out through
    /// `read`'s `finally`; it is the *only* content problem `SesReader` refuses outright. Every
    /// other one (an unknown net, a missing padstack, an unreadable path) bumps
    /// [`crate::SesImportSummary::errors_encountered`] and the read carries on.
    #[error(
        "SesReader: not a Specctra session file — expected '(session <name>' header, got: {got}"
    )]
    NotASessionFile {
        /// The offending token, as Java concatenates it into the message.
        got: String,
    },

    /// `DsnFile.adjustPlaneAutorouteSettings` (DsnFile.java:86-90) calls
    /// `currentConductionArea.getArea().splitToConvex()` with no null check — unlike the
    /// sibling loop over the board outline eleven lines above it (`:67-68`), which does check.
    /// Reachable: a `(plane ...)`/copper-pour polygon degenerate enough that
    /// `Area::split_to_convex` gives up NPEs here in Java, and nothing between
    /// `DsnFile.adjustPlaneAutorouteSettings` and `DsnReader.readBoard`'s unguarded call to it
    /// catches that — the crash aborts the whole read. Fix round 1 (a reachable crash must
    /// become `Err`, not a silently skipped area).
    #[error("conduction area {item:?}'s shape could not be split into convex pieces")]
    UnsplittableConductionArea {
        /// The conduction area whose `Area::split_to_convex` returned `None`.
        item: ItemId,
    },

    /// A [`CoordinateTransform`] was asked for with a scale factor that is zero, infinite or
    /// `NaN`.
    ///
    /// Java builds `new CoordinateTransform(0, 0, 0)` without complaint whenever
    /// `Structure.createBoard`'s overflow loop truncates its `int` scale factor to zero (quirks
    /// #94/#89), and the read still reports `Success` while every written coordinate is
    /// `Infinity`/`NaN`. Plan 9 Task 4 makes that state unrepresentable instead; this is the
    /// loud refusal it becomes.
    #[error("a coordinate transform needs a finite, non-zero scale factor, not {scale_factor}")]
    InvalidScaleFactor {
        /// The refused scale factor.
        scale_factor: f64,
    },

    /// `KiCadJsonReader.importSession` (KiCadJsonReader.java:757-855) threw.
    ///
    /// That method declares `throws Exception` and carries **no** `catch` of its own — unlike
    /// `readBoard`, whose `catch (Throwable)` at `:746` turns every failure into a
    /// [`BoardReadResult::ParseError`]. The throwable therefore reaches the caller, which at both
    /// call sites is a `catch (Exception e) { FRLogger.error(<message>, e); }` that does not stop
    /// the run (`Freerouting.java:326-328`, `RoutingJobScheduler.java:232-234`). The payload is
    /// the throwable's own `toString()`, i.e. the first line log4j prints for that call.
    ///
    /// Everything imported before the throw stays on the board, in Java and here alike.
    #[error("{0}")]
    KicadSession(String),
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
/// format) — `io/BoardReadResult.java`, ported as a Rust `enum` (its Java `sealed interface` +
/// four `record` permits are the four variants `Success`, `OutlineMissing`, `ParseError` and
/// `IoError`; [`BoardReadResult::Partial`] is a **fifth**, added by Plan 9 Task 4 for quirk #91
/// — Java has no way to say "this board is what a truncated file contained").
///
/// `Success.metadata` is `Option` because `DsnReader.readBoard` returns `Success` with a
/// **`null`** metadata — only `readMetadata` populates it. Both variants' `board` is `Option`
/// because Java's may be `null` or partial: `DsnReadResultTest.successAndOutlineMissingHoldNullBoard`
/// constructs `new Success(null, null, List.of())` explicitly, and `DsnReader.readBoard` reaches
/// it for real on a `(pcb name)` whose body never produced a board (Task 10 correction — Task 1
/// had `Success.board` non-optional).
///
/// `coordinate_transform` is a field the **port adds** that Java's record lacks (Plan 3 Task 10,
/// controller ruling A). Java's writers re-derive the transform from the board; this port's
/// (Tasks 11-12) take a `&CoordinateTransform` explicitly, and the one `Structure.createBoard`
/// produced is the only one that round-trips a file's coordinates unchanged. It is `None` when
/// `createBoard` never ran.
#[derive(Debug)]
pub enum BoardReadResult {
    /// Full board + metadata are available. The board is fully constructed and routable.
    Success {
        /// The constructed board; `None` when the read succeeded without producing one.
        board: Option<Box<Board>>,
        /// Header/structure metadata; `None` when the caller only wanted the board (Java's
        /// `null`).
        metadata: Option<BoardMetadata>,
        /// Non-fatal issues encountered during loading (e.g. degenerate wires, duplicate vias,
        /// missing nets). May be empty.
        warnings: Vec<String>,
        /// The transform `Structure.createBoard` built between DSN and board coordinates; `None`
        /// if it never ran. Added by the port — see the type's doc comment.
        coordinate_transform: Option<CoordinateTransform>,
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
        /// The transform `Structure.createBoard` built between DSN and board coordinates; `None`
        /// if it never ran. Added by the port — see the type's doc comment.
        coordinate_transform: Option<CoordinateTransform>,
    },
    /// The input ran out **before** a scope's closing bracket, so the board is only the part of
    /// the design the file held up to the truncation.
    ///
    // Java bug: (#91) `ScopeKeyword.skipScope` answers `false` at end of file, every caller
    // discards it, and `ScopeKeyword.readScope`'s own end-of-file check then returns `true`
    // (ScopeKeyword.java:32-33, :55-58) — so `DsnReader.readBoard` reports a truncated file as
    // `Success` with a partial board, indistinguishable from a complete read.
    //
    // fixed: T4 (#91) — this variant. Deliberately **not** a hard failure: a caller may well
    // want whatever routing data survived, and the roadmap's correction of the register's binary
    // framing says the honest answer is a third state, not a refusal. It carries the same four
    // payload fields as [`BoardReadResult::Success`] — an amendment to the task brief's
    // two-field sketch (`Partial { board, diagnostic }`), because a `Partial` with no
    // `coordinate_transform` cannot be loaded by `fr_core::load::parse_board_result` at all and
    // would be a hard failure wearing a softer name.
    Partial {
        /// The board as far as the file got; `None` when the truncation came before one existed.
        board: Option<Box<Board>>,
        /// Header/structure metadata; `None` on the [`crate::read_board`] path, as for `Success`.
        metadata: Option<BoardMetadata>,
        /// Non-fatal issues encountered during loading. May be empty; the truncation itself is
        /// in `diagnostic`, not here.
        warnings: Vec<String>,
        /// The transform `Structure.createBoard` built between DSN and board coordinates; `None`
        /// if it never ran.
        coordinate_transform: Option<CoordinateTransform>,
        /// What was truncated, and where: the scope whose closing bracket never arrived.
        diagnostic: String,
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
