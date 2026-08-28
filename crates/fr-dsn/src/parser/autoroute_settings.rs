//! `io/specctra/parser/AutorouteSettings.java` — the `autoroute_settings` scope.

// added in Plan 3 Task 6: full `DsnRouterSettings`, holding exactly the fields the DSN/rules
// scopes read and write (`run_router`, `run_optimizer`, `vias_allowed`, `via_costs`,
// `plane_via_costs`, `start_ripup_costs`, and per-layer `active`/
// `preferred_direction_is_horizontal`/`preferred_direction_trace_costs`/
// `against_preferred_direction_trace_costs`), per plan ruling 5. `AutorouteSettings.readScope`
// and `.writeScope` also land in Task 6.
//
// This is a placeholder so `BoardMetadata::router_settings` (Task 1, `crate::error`) compiles
// before Task 6 fills in the real type.
/// Placeholder for the DSN/rules `autoroute_settings` scope's router settings. Body added in
/// Plan 3 Task 6.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DsnRouterSettings;
