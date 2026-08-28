//! `io/specctra/parser/{Placement,Component,ComponentPlacement}.java` — the `placement` scope
//! and, nested inside it, one `component` scope per library component.

use crate::error::DsnError;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

/// `io/specctra/parser/ComponentPlacement.java`: placement data for one library component
/// (`libName` plus a list of `ComponentLocation`s).
///
/// Placeholder — body (`locations: Vec<ComponentLocation>` and its fields: name, coordinates,
/// front/back side, rotation, fixed flag, per-pin clearance-class overrides, part number) arrives
/// with `Component.readScope`.
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

// Fix round 1: there is no `read_placement_scope` here, and none is dispatched to — verified by
// reading `Placement.java` in full: it has only a constructor and `writeScope`, no `readScope`
// override at all, so `ScopeKeyword::Placement` dispatches straight to the generic
// `read_scope_generic` loop (`parser/scope_parameter.rs`), the same as `ScopeKeyword::Pcb`.

// added in Plan 3: Component.readScope
/// Stub for `Component.readScope(ReadScopeParameter)` — this **is** a real `@Override` of
/// `ScopeKeyword.readScope` (`Component.java:367-379`; fix round 1 corrects an earlier, wrong
/// claim here that it wasn't). Its Java body delegates to the *other*
/// `Component.readScope(IJFlexScanner)` overload (`Component.java:29`, called at `:370`) to do
/// the actual parsing, then appends the resulting `ComponentPlacement` to
/// `scopeParameter.placementList`. This stub keeps the uniform `fn read_xxx_scope(p) ->
/// Result<bool, DsnError>` shape the dispatch table wants for now; the real body (once ported)
/// keeps that same shape too, since it is a genuine `ReadScopeParameter`-taking override.
pub fn read_component_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}
