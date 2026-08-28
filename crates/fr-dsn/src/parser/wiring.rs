//! `io/specctra/parser/Wiring.java` — the `wiring` scope (`wire`/`via`/`fromto` entries).

use crate::error::DsnError;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

// added in Plan 3: Wiring.readScope
/// Stub for `Wiring.readScope` (Wiring.java) — replaced with the real reader by a later task
/// (which also owns plan ruling 4's `board.normalizeAllTraces()` call at the end of the scope,
/// Wiring.java:346); for now this just discards the scope's body.
pub fn read_wiring_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}
