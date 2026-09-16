use copper_board::Board;
use copper_drc::PlaneConnectivity;
use copper_settings::RouterSettings;

#[must_use]
pub fn plane_connectivity_of(board: &Board, settings: &RouterSettings) -> PlaneConnectivity {
    PlaneConnectivity::with_zone_clearance_um(board, settings.get_zone_clearance_um())
}
