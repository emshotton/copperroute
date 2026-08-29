//! `settings/sources/**`: the concrete [`crate::SettingsSource`] implementations, plus the
//! `DsnRouterSettings` ⇄ [`RouterSettings`] conversion pair Plan 3 ruling 5 parked here.
//!
//! Five of Java's nine source classes are in scope for this crate:
//!
//! | Java | Rust | priority |
//! |---|---|---|
//! | `DefaultSettings` | [`DefaultSettings`] | 0 |
//! | `DsnFileSettings` | [`DsnFileSettings`] | 20 |
//! | `SesFileSettings` | [`SesFileSettings`] | 30 |
//! | `RulesFileSettings` | [`RulesFileSettings`] | 40 |
//! | `EnvironmentVariablesSource` | [`EnvironmentVariablesSource`] | 55 |
//! | `CliSettings` | [`CliSettings`] | 60 |
//! | `ApiSettings` | [`ApiSettings`] | 70 |
//!
//! [`cli`] also carries the two pieces of `settings/GlobalSettings.applyCommandLineArguments`
//! that belong beside `CliSettings`: the dead [`cli::LegacyBridge`] (plan ruling 8) and the `-de`
//! file classifier [`cli::classify_de_arguments`] (plan ruling 10).
//!
//! `JsonFileSettings` (10) and `GuiSettingsSource` (50) are out of scope — no persistent config
//! file (spec §2) and no GUI — and Task 11 writes their `// not ported:` roster.

pub mod api;
pub mod cli;
pub mod default_settings;
pub mod dsn_file;
pub mod env;
pub mod rules_file;
pub mod ses_file;

pub use api::ApiSettings;
pub use cli::CliSettings;
pub use default_settings::DefaultSettings;
pub use dsn_file::DsnFileSettings;
pub use env::EnvironmentVariablesSource;
pub use rules_file::RulesFileSettings;
pub use ses_file::SesFileSettings;

use fr_dsn::parser::DsnRouterSettings;

use crate::RouterSettings;

// ---------------------------------------------------------------------------------------------
// Plan 3 ruling 5: the DsnRouterSettings <-> RouterSettings conversion
// ---------------------------------------------------------------------------------------------

/// `io/specctra/parser/AutorouteSettings.readScope` returns an
/// `app.freerouting.settings.RouterSettings`; `fr-dsn` cannot depend on `fr-settings` without
/// inverting the build order, so Plan 3 gave it the field-for-field subset
/// [`DsnRouterSettings`] and parked this conversion here (plan ruling 3 and Plan 3 ruling 5).
///
/// The body is `AutorouteSettings.readScope`'s own construction sequence, in its own order:
/// `new RouterSettings()` (:19), `setLayerCount(layerStructure.layers.length)` (:20), then one
/// setter per value the scope named (:50-68). Building it that way rather than assigning fields
/// is what makes the result *identical* to the object Java's `RulesReader` hands
/// `RulesFileSettings` — including the details a field-by-field mapping would get wrong:
///
/// - `setLayerCount` seeds `layers[i].routable = Some(true)` and both `scoring` cost arrays with
///   `1.0` (`RouterSettings.java:466-477`), so a layer the file never mentioned still ends up
///   routable with neutral costs;
/// - `layers[i].preferredDirectionHorizontal` stays **`None`** where the file named no
///   direction — Plan 3 kept `Option<bool>` on both sides precisely so this survives, and it is
///   the one nullable field a later `.rules` source can still contribute through the merge
///   (plan ruling 1's Q1 channel);
/// - `fanout`, `optimizer` and `scoring` come back as the *present but empty* objects
///   `new RouterSettings()` allocates, not as `None` — JVM-verified, `SProbe E.processorRaw`.
///
/// Every field the scope does not name is left absent.
impl From<DsnRouterSettings> for RouterSettings {
    fn from(dsn: DsnRouterSettings) -> Self {
        Self::from(&dsn)
    }
}

impl From<&DsnRouterSettings> for RouterSettings {
    fn from(dsn: &DsnRouterSettings) -> Self {
        let mut result = RouterSettings::new();
        result.set_layer_count(dsn.get_layer_count());

        // `(vias …)`, `(via_costs …)`, `(plane_via_costs …)`, `(start_ripup_costs …)`
        // (AutorouteSettings.java:50-58).
        result.set_vias_allowed(Some(dsn.vias_allowed()));
        result.set_via_costs(dsn.via_costs());
        result.set_plane_via_costs(dsn.plane_via_costs());
        result.set_start_ripup_costs(dsn.start_ripup_costs());

        // `(layer_rule …)` (:59-63, readLayerRule).
        for layer in 0..dsn.get_layer_count() {
            result.set_layer_active(layer, dsn.get_layer_active(layer));
            if let Some(horizontal) = dsn.preferred_direction_is_horizontal_raw(layer) {
                result.set_preferred_direction_is_horizontal(layer, horizontal);
            }
            result.set_preferred_direction_trace_costs(
                layer,
                dsn.get_preferred_direction_trace_costs(layer),
            );
            result.set_against_preferred_direction_trace_costs(
                layer,
                dsn.get_against_preferred_direction_trace_costs(layer),
            );
        }

        // `withAutoroute`/`withPostroute` are applied after the loop (:67-68).
        result.set_run_router(dsn.run_router());
        result.set_run_optimizer(dsn.run_optimizer());
        result
    }
}

/// The reverse: what `io/specctra/RulesWriter` and `DsnWriter` see when they are handed a
/// [`RouterSettings`] (`AutorouteSettings.writeScope` reads it through the same getters).
///
/// It goes through [`RouterSettings`]' null-coalescing accessors, so it **applies their
/// defaults** — `getLayerActive`'s `true`, `getPreferredDirectionIsHorizontal`'s alternating
/// `layer % 2 == 1` (`RouterSettings.java:743,747`), and both trace-cost getters' `1.0`
/// (`:786,806`). That is lossy in exactly one direction: a `None`
/// `preferredDirectionHorizontal` cannot come back as `None`, because `DsnRouterSettings`'
/// setters only write `Some`. It is nonetheless faithful — those defaults are precisely what
/// Java's writers put in the file — and `reverse_conversion_applies_the_getters_defaults` pins
/// both halves.
impl From<&RouterSettings> for DsnRouterSettings {
    fn from(settings: &RouterSettings) -> Self {
        let mut dsn = DsnRouterSettings::new();
        dsn.set_layer_count(settings.get_layer_count());

        dsn.set_run_router(settings.get_run_router());
        dsn.set_run_optimizer(settings.get_run_optimizer());
        dsn.set_vias_allowed(settings.get_vias_allowed());
        dsn.set_via_costs(settings.get_via_costs());
        dsn.set_plane_via_costs(settings.get_plane_via_costs());
        dsn.set_start_ripup_costs(settings.get_start_ripup_costs());

        for layer in 0..settings.get_layer_count() {
            dsn.set_layer_active(layer, settings.get_layer_active(layer));
            dsn.set_preferred_direction_is_horizontal(
                layer,
                settings.get_preferred_direction_is_horizontal(layer),
            );
            dsn.set_preferred_direction_trace_costs(
                layer,
                settings.get_preferred_direction_trace_costs(layer),
            );
            dsn.set_against_preferred_direction_trace_costs(
                layer,
                settings.get_against_preferred_direction_trace_costs(layer),
            );
        }
        dsn
    }
}
