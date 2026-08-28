//! Specctra text-format primitives shared by every DSN/SES writer: indentation tracking
//! (`IndentFileWriter`) and legal-identifier quoting (`IdentifierType`).
//!
// added in Plan 3 Task 2: `double.rs` — `java_double_to_string`, `java_float_to_string`,
// `format_placement_rotation`.
pub mod identifier;
pub mod indent_writer;

pub use identifier::{DSN_RESERVED, IdentifierType, SES_RESERVED};
pub use indent_writer::IndentFileWriter;
