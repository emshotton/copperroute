//! Small, closely related header/global-settings scopes with no dedicated file of their own in
//! the plan's file structure: `io/specctra/parser/{Parser,Resolution,PlaceControl}.java`.
//!
//! `Parser` reads the `parser` scope (`host_cad`/`host_version`/`string_quote`/
//! `write_resolution`, i.e. the file's own metadata about the tool that wrote it); `Resolution`
//! reads the `resolution` scope (`unit`, `resolution`); `PlaceControl` reads the `place_control`
//! scope (`flip_style`/`rotate_first`). All three are tiny compared to `structure`/`network`/
//! `wiring`/`library`/`placement`, hence the shared file.

use crate::error::DsnError;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

// added in Plan 3: Parser.readScope
/// Stub for `Parser.readScope` (Parser.java) — replaced with the real reader by a later task;
/// for now this just discards the scope's body.
pub fn read_parser_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}

// added in Plan 3: Resolution.readScope
/// Stub for `Resolution.readScope` (Resolution.java) — replaced with the real reader by a later
/// task; for now this just discards the scope's body.
pub fn read_resolution_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}

// added in Plan 3: PlaceControl.readScope
/// Stub for `PlaceControl.readScope` (PlaceControl.java) — replaced with the real reader by a
/// later task; for now this just discards the scope's body.
pub fn read_place_control_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}
