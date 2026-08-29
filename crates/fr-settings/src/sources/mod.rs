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
/// **Every setter is called only where `readScope` calls it.** `(vias …)`, `(via_costs …)`,
/// `(plane_via_costs …)` and `(start_ripup_costs …)` are each optional tokens
/// (`AutorouteSettings.java:50-58`), so a file that omits one leaves the corresponding
/// `RouterSettings` field `None` and every higher-priority source's value survives the merge.
/// Getting this wrong is a wrong-output bug with no crash to find it: an earlier revision of this
/// conversion wrote the *coalesced defaults* instead, so a `.rules` file with no `(via_costs …)`
/// pushed `viaCosts = 1` over `DefaultSettings`' 50. JVM-measured on a reduced fixture —
/// `SProbe H.reduced.merged.getViaCosts = 50` where the port answered 1 (Task 6 review,
/// controller ruling L; the root fix is the four `Option` fields and the
/// `boardSpecificTraceCostsApplied` flag now carried by `DsnRouterSettings` itself).
///
/// The two per-layer cost arrays are the subtle half: `setLayerCount` seeds both with `1.0`
/// (`RouterSettings.java:466-472`), so their *values* cannot say whether the file named a cost —
/// only `boardSpecificTraceCostsApplied` can (`:776`, `:858`). The setters are therefore replayed
/// only when [`DsnRouterSettings::are_board_specific_trace_costs_applied`] is set, which leaves
/// the seeded `1.0`s and the `false` flag untouched otherwise. Replaying *both* arrays when the
/// flag is set, rather than only the array whose setter actually ran, reaches the identical end
/// state: writing a layer's seeded `1.0` back over itself changes no value, and the flag is a
/// single boolean either way.
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
        // (AutorouteSettings.java:50-58) — each setter runs only when its token was read.
        if let Some(vias_allowed) = dsn.vias_allowed_raw() {
            result.set_vias_allowed(Some(vias_allowed));
        }
        if let Some(via_costs) = dsn.via_costs_raw() {
            result.set_via_costs(via_costs);
        }
        if let Some(plane_via_costs) = dsn.plane_via_costs_raw() {
            result.set_plane_via_costs(plane_via_costs);
        }
        if let Some(start_ripup_costs) = dsn.start_ripup_costs_raw() {
            result.set_start_ripup_costs(start_ripup_costs);
        }

        // `(layer_rule …)` (:59-63, readLayerRule). `active` is unconditional because
        // `setLayerCount` seeds `routable = true` on both sides, so writing the read-back value
        // is a no-op for a layer the file never mentioned (:112-114 with :471).
        let trace_costs_were_named = dsn.are_board_specific_trace_costs_applied();
        for layer in 0..dsn.get_layer_count() {
            result.set_layer_active(layer, dsn.get_layer_active(layer));
            if let Some(horizontal) = dsn.preferred_direction_is_horizontal_raw(layer) {
                result.set_preferred_direction_is_horizontal(layer, horizontal);
            }
            // `:139-145` — and the flag they set at `RouterSettings.java:776`/`:858`.
            if trace_costs_were_named {
                result.set_preferred_direction_trace_costs(
                    layer,
                    dsn.get_preferred_direction_trace_costs(layer),
                );
                result.set_against_preferred_direction_trace_costs(
                    layer,
                    dsn.get_against_preferred_direction_trace_costs(layer),
                );
            }
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
