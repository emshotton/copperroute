//! `io/specctra/parser/{Structure,LayerStructure,Layer}.java` — the `structure` scope, and
//! (nested inside it, Structure.java:1001-1005) the `plane` scope.

use crate::error::DsnError;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

/// `io/specctra/parser/LayerStructure.java` (the DSN-parser's own layer structure, distinct from
/// `fr_board::LayerStructure`): the ordered list of `Layer`s read from a `structure` scope,
/// before it is turned into a board layer structure.
///
/// Placeholder — body (`layers: Vec<Layer>`, `getNo`) arrives with `Structure.readScope`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DsnLayerStructure;

/// `io/specctra/parser/ReadScopeParameter.java`'s nested `PlaneInfo` (2.3.0) / `ReadScopeParameter.PlaneInfo`
/// (HEAD): a plane read from a `plane` scope, held until the library scope has been read fully
/// enough to insert it into the board (`ReadScopeParameter.planeList`'s element type).
///
/// Placeholder — body (`area: Shape::ReadAreaScopeResult`, `net_name: String`) arrives with
/// `Plane.readScope`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DsnPlane;

// added in Plan 3: Structure.readScope
/// Stub for `Structure.readScope` (Structure.java) — replaced with the real reader by a later
/// task; for now this just discards the scope's body.
pub fn read_structure_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}

// added in Plan 3: Plane.readScope
/// Stub for `Plane.readScope` (Plane.java) — replaced with the real reader by a later task; for
/// now this just discards the scope's body. Called only from within `Structure.readScope`
/// (Structure.java:1001-1005), not from the top-level `pcb` scope.
pub fn read_plane_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}
