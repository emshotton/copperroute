#![forbid(unsafe_code)]

pub mod coordinate_transform;
pub mod dsn_reader;
pub mod dsn_writer;
pub mod error;
pub mod format;
pub mod keyword;
pub mod kicad;
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
    to_gson_string_pretty,
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
pub use parser::structure::RuleLayerScope;
pub use ses_reader::SesImportSummary;

pub mod prelude {
    pub use crate::{
        BoardMetadata, BoardReadResult, CoordinateTransform, DSN_RESERVED, DsnCircle, DsnError,
        DsnLayer, DsnLayerStructure, DsnPolygon, DsnPolygonPath, DsnPolylinePath, DsnReadOptions,
        DsnRectangle, DsnScanner, DsnShape, FileFormat, IdentifierType, IndentFileWriter,
        JavaNumberFormatter, Keyword, LexicalState, ReadAreaScopeResult, ReadScopeParameter,
        RuleLayerScope, SES_RESERVED, ScopeKeyword, SesImportSummary, Token, WriteScopeParameter,
        format_placement_rotation, java_double_to_string, java_float_to_string, java_format_fixed,
        read_board, read_metadata, read_scope, skip_scope, to_gson_string_pretty, write,
    };
}
