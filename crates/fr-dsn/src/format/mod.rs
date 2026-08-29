//! Specctra text-format primitives shared by every DSN/SES writer: indentation tracking
//! (`IndentFileWriter`), legal-identifier quoting (`IdentifierType`) and Java-identical number
//! rendering (`double`).
pub mod double;
pub mod identifier;
pub mod indent_writer;

pub use double::{
    format_placement_rotation, java_double_to_string, java_float_to_string, java_format_fixed,
    java_rint, java_round, java_round_to_int,
};
pub use identifier::{DSN_RESERVED, IdentifierType, SES_RESERVED};
pub use indent_writer::IndentFileWriter;
