//! `io/specctra/parser/PartLibrary.java` — the `part_library` scope (`logical_part`/
//! `logical_part_mapping` entries used for pin/gate swap). Plan ruling 7: ported only as far as
//! DSN read/write needs (`BoardLibrary::logical_parts`); the rest is `// not ported:`.

use crate::error::DsnError;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

// added in Plan 3: PartLibrary.readScope
/// Stub for `PartLibrary.readScope` (PartLibrary.java) — replaced with the real reader by a
/// later task; for now this just discards the scope's body.
pub fn read_part_library_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}
