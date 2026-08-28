//! `io/specctra/parser/Library.java` — the `library` scope (`image`/`padstack` definitions).

use crate::error::DsnError;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

// added in Plan 3: Library.readScope
/// Stub for `Library.readScope` (Library.java) — replaced with the real reader by a later task;
/// for now this just discards the scope's body.
pub fn read_library_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}
