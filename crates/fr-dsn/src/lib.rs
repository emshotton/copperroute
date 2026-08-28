//! `fr-dsn`: Specctra DSN/SES/rules text I/O — the reader/writer layer built directly on top of
//! `fr-board`. Faithful port of `io/specctra/**` and `io/{CoordinateTransform,BoardReadResult,
//! BoardMetadata,FileFormat}.java` plus `datastructures/{IdentifierType,IndentFileWriter}.java`
//! (freerouting v2.3.0). See the Plan 3 design doc
//! (`docs/superpowers/plans/2026-08-28-plan-3-dsn.md`) for scope, architecture and rulings.
//!
//! This crate must not depend on `tracing`: `FRLogger` calls from the Java source are dropped
//! during porting, mirroring `fr-board`'s convention (plan Global Constraints).

pub mod coordinate_transform;
pub mod error;
pub mod format;
pub mod keyword;
pub mod lexer;
pub mod parser;

pub use coordinate_transform::CoordinateTransform;
pub use error::{BoardMetadata, BoardReadResult, DsnError, FileFormat};
pub use format::{
    DSN_RESERVED, IdentifierType, IndentFileWriter, SES_RESERVED, format_placement_rotation,
    java_double_to_string, java_float_to_string, java_rint, java_round, java_round_to_int,
};
pub use keyword::{Keyword, ScopeKeyword};
pub use lexer::{DsnScanner, LexicalState, Token};
pub use parser::scope_parameter::{
    DsnReadOptions, ReadScopeParameter, WriteScopeParameter, read_scope, skip_scope,
};

/// Re-exports every public type of the crate, for `use fr_dsn::prelude::*;`.
pub mod prelude {
    pub use crate::{
        BoardMetadata, BoardReadResult, CoordinateTransform, DSN_RESERVED, DsnError,
        DsnReadOptions, DsnScanner, FileFormat, IdentifierType, IndentFileWriter, Keyword,
        LexicalState, ReadScopeParameter, SES_RESERVED, ScopeKeyword, Token, WriteScopeParameter,
        format_placement_rotation, java_double_to_string, java_float_to_string, java_rint,
        java_round, java_round_to_int, read_scope, skip_scope,
    };
}
