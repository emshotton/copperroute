//! `fr-dsn`: Specctra DSN/SES/rules text I/O — the reader/writer layer built directly on top of
//! `fr-board`. Faithful port of `io/specctra/**` and `io/{CoordinateTransform,BoardReadResult,
//! BoardMetadata,FileFormat}.java` plus `datastructures/{IdentifierType,IndentFileWriter}.java`
//! (freerouting v2.3.0). See the Plan 3 design doc
//! (`docs/superpowers/plans/2026-08-28-plan-3-dsn.md`) for scope, architecture and rulings.
//!
//! This crate must not depend on `tracing`: `FRLogger` calls from the Java source are dropped
//! during porting, mirroring `fr-board`'s convention (plan Global Constraints).
//
// added in Plan 8: SessionToEagle.getInstance — `io/specctra/parser/SessionToEagle.java` (627
// lines) converts a Specctra session file into an Eagle CAD command script. It is deliberately
// outside Plan 3 (the plan's Architecture paragraph defers it, with `io/kicad/**`, to Plan 8
// "fr-core + surfaces"): it neither reads the DSN grammar nor touches a `Board`, so nothing in
// this crate calls it and nothing in it is needed to reach byte parity. Its only caller anywhere
// in the Java tree is `SesReader.saveSpecctraSessionSesAsEagleScriptScr` (SesReader.java:105-109),
// a one-line delegate that `ses_reader.rs` already carries a `// not ported:` marker for, so the
// deferral is closed on both sides. See `scripts/audit-map/fr-dsn.map`.

pub mod coordinate_transform;
pub mod dsn_reader;
pub mod dsn_writer;
pub mod error;
pub mod format;
pub mod keyword;
pub mod lexer;
pub mod parser;
pub mod rules_reader;
pub mod rules_writer;
pub mod ses_reader;
pub mod ses_writer;

pub use coordinate_transform::CoordinateTransform;
pub use dsn_reader::{read_board, read_metadata};
pub use dsn_writer::write;
pub use error::{BoardMetadata, BoardReadResult, DsnError, FileFormat};
pub use format::{
    DSN_RESERVED, IdentifierType, IndentFileWriter, JavaNumberFormatter, SES_RESERVED,
    format_placement_rotation, java_double_to_string, java_float_to_string, java_format_fixed,
    java_rint, java_round, java_round_to_int, to_gson_string_pretty,
};
pub use keyword::{Keyword, ScopeKeyword};
pub use lexer::{DsnScanner, LexicalState, Token};
pub use parser::geometry::{
    DsnCircle, DsnLayer, DsnLayerStructure, DsnPolygon, DsnPolygonPath, DsnPolylinePath,
    DsnRectangle, DsnShape, ReadAreaScopeResult,
};
pub use parser::scope_parameter::{
    DsnReadOptions, ReadScopeParameter, WriteScopeParameter, read_scope, skip_scope,
};
pub use ses_reader::SesImportSummary;

/// Re-exports every public type of the crate, for `use fr_dsn::prelude::*;`.
pub mod prelude {
    pub use crate::{
        BoardMetadata, BoardReadResult, CoordinateTransform, DSN_RESERVED, DsnCircle, DsnError,
        DsnLayer, DsnLayerStructure, DsnPolygon, DsnPolygonPath, DsnPolylinePath, DsnReadOptions,
        DsnRectangle, DsnScanner, DsnShape, FileFormat, IdentifierType, IndentFileWriter,
        JavaNumberFormatter, Keyword, LexicalState, ReadAreaScopeResult, ReadScopeParameter,
        SES_RESERVED, ScopeKeyword, SesImportSummary, Token, WriteScopeParameter,
        format_placement_rotation, java_double_to_string, java_float_to_string, java_format_fixed,
        java_rint, java_round, java_round_to_int, read_board, read_metadata, read_scope,
        skip_scope, to_gson_string_pretty, write,
    };
}
