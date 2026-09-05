//! `DsnRouterSettings` ⇄ [`RouterSettings`] conversion pair Plan 3 ruling 5 parked here.
//! |---|---|---|
pub mod api;
pub mod cli;
pub mod default_settings;
pub mod dsn_file;
pub mod env;
pub mod json_file;
pub mod rules_file;
pub mod ses_file;

pub use api::ApiSettings;
pub use cli::CliSettings;
pub use default_settings::DefaultSettings;
pub use dsn_file::DsnFileSettings;
pub use env::EnvironmentVariablesSource;
pub use json_file::{CONFIGURATION_FILE_NAME, JsonFileSettings};
pub use rules_file::RulesFileSettings;
pub use ses_file::SesFileSettings;

use fr_dsn::parser::DsnRouterSettings;

use crate::RouterSettings;

impl From<DsnRouterSettings> for RouterSettings {
    fn from(dsn: DsnRouterSettings) -> Self {
        Self::from(&dsn)
    }
}

impl From<&DsnRouterSettings> for RouterSettings {
    fn from(dsn: &DsnRouterSettings) -> Self {
        let mut result = RouterSettings::new();
        result.set_layer_count(dsn.get_layer_count());

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

        let trace_costs_were_named = dsn.are_board_specific_trace_costs_applied();
        for layer in 0..dsn.get_layer_count() {
            result.set_layer_active(layer, dsn.get_layer_active(layer));
            if let Some(horizontal) = dsn.preferred_direction_is_horizontal_raw(layer) {
                result.set_preferred_direction_is_horizontal(layer, horizontal);
            }
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

        result.set_run_router(dsn.run_router());
        result.set_run_optimizer(dsn.run_optimizer());
        result
    }
}

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
