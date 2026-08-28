//! `io/specctra/parser/{Placement,Component,ComponentPlacement}.java` — the `placement` scope
//! and, nested inside it, one `component` scope per library component.

use crate::error::DsnError;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

/// `io/specctra/parser/ComponentPlacement.java`: placement data for one library component
/// (`libName` plus a list of `ComponentLocation`s).
///
/// Placeholder — body (`locations: Vec<ComponentLocation>` and its fields: name, coordinates,
/// front/back side, rotation, fixed flag, per-pin clearance-class overrides, part number) arrives
/// with `Placement.readScope`/`Component.readScope`.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPlacement {
    /// `ComponentPlacement.libName` (ComponentPlacement.java:11): the name of the corresponding
    /// library component.
    pub lib_name: String,
}

impl ComponentPlacement {
    /// `ComponentPlacement(String)` (ComponentPlacement.java:18-21).
    #[must_use]
    pub fn new(lib_name: String) -> ComponentPlacement {
        ComponentPlacement { lib_name }
    }
}

// added in Plan 3: Placement.readScope
/// Stub for `Placement.readScope` (Placement.java) — replaced with the real reader by a later
/// task; for now this just discards the scope's body.
pub fn read_placement_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}

// added in Plan 3: Component.readScope
/// Stub for `Component.readScope` (Component.java) — note this is *not* actually a
/// `ScopeKeyword.readScope` override in Java (`Component.readScope(IJFlexScanner)` is a `static`
/// method with a different signature, returning `ComponentPlacement` rather than `boolean`,
/// called directly by `Placement.readScope` rather than through the generic dispatch); this stub
/// keeps the uniform `fn read_xxx_scope(p) -> Result<bool, DsnError>` shape the dispatch table
/// wants for now, and whichever later task ports `Component` should revisit that shape too.
pub fn read_component_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}
