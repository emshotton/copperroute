use std::path::{Path, PathBuf};

use copper_board::Board;
use copper_core::{FileFormat, RoutingJob, SessionId};
use copper_dsn::{BoardMetadata, CoordinateTransform};
use copper_settings::sources::DsnFileSettings;
use copper_settings::{HostEnvironment, RouterSettings, SettingsSource};

use super::OpError;
use super::settings::{self, SettingsOverrides};

#[derive(Debug, Clone)]
pub enum BoardSource {
    Path(PathBuf),
    Text { text: String, name: String },
}

#[derive(Debug, Clone)]
pub struct LoadRequest {
    pub source: BoardSource,
    pub rules: Option<PathBuf>,
    pub discover_adjacent_rules: bool,
    pub session: Option<PathBuf>,
    pub kicad_project: Option<PathBuf>,
    pub settings: SettingsOverrides,
}

impl LoadRequest {
    pub fn for_board(source: BoardSource) -> LoadRequest {
        LoadRequest {
            source,
            rules: None,
            discover_adjacent_rules: false,
            session: None,
            kicad_project: None,
            settings: SettingsOverrides::default(),
        }
    }
}

#[derive(Debug)]
pub struct Loaded {
    pub job: RoutingJob,
    pub board: Board,
    pub transform: CoordinateTransform,
    pub metadata: Option<BoardMetadata>,
    pub settings: RouterSettings,
    pub warnings: Vec<String>,
}

pub fn load(request: &LoadRequest) -> Result<Loaded, OpError> {
    let mut job = RoutingJob::new(SessionId::NIL);
    match &request.source {
        BoardSource::Path(path) => job.set_input(path).map_err(|error| {
            OpError::Input(format!(
                "Couldn't load the input file '{}': {error}",
                path.display()
            ))
        })?,
        BoardSource::Text { text, name } => {
            job.set_input_bytes(Some(text.as_bytes()));
            if let Some(input) = job.input.as_mut() {
                let extension = match input.format.default_extension() {
                    "" => "dsn",
                    extension => extension,
                };
                input.set_filename(Some(&format!("{name}.{extension}")));
            }
            job.name = name.clone();
        }
    }

    let input = job.get_input().expect("the input was just set");
    if !matches!(input.format, FileFormat::Dsn | FileFormat::KicadDesignJson) {
        return Err(OpError::Input(format!(
            "'{}' is not a board: only Specctra DSN and KiCad board JSON are accepted, got {}",
            input.get_filename(),
            input.format.name()
        )));
    }
    let dsn_source = DsnFileSettings::new(input.get_data(), input.get_filename());

    if let Some(rules) = request.rules.as_deref() {
        if !rules.exists() {
            tracing::warn!("Rules file {} not found", rules.display());
        } else if let Err(error) = job.set_rules(rules) {
            tracing::warn!("Couldn't load rules file '{}': {error}", rules.display());
        }
    }
    let explicit_rules: Option<Vec<u8>> = job
        .rules
        .as_ref()
        .map(|rules| rules.get_data().to_vec())
        .filter(|data| !data.is_empty());
    let scheduler_rules = if request.discover_adjacent_rules {
        read_scheduler_rules(&job, request.rules.as_deref())
    } else {
        explicit_rules.clone()
    };

    let parsed = copper_core::parse_board_if_needed(&job)?;
    let mut board = parsed.board;
    let transform = parsed.transform;

    let host = HostEnvironment::detect();
    let mut settings = settings::resolve(
        &request.settings,
        dsn_source.get_settings(),
        explicit_rules.as_deref(),
        scheduler_rules.as_deref(),
        Some(&board),
        &host,
    )?;
    copper_core::apply_router_settings_for_loaded_board(&mut board, &mut settings);
    copper_core::apply_immediate_post_load_processing(&mut board);

    if let Some(bytes) = scheduler_rules.as_deref() {
        match copper_dsn::rules_reader::read(bytes, &job.name, &mut board, &transform, None) {
            Ok(_) => tracing::info!(
                "Rules loaded from {}",
                request.rules.as_deref().map_or_else(
                    || "the rules file beside the input".to_string(),
                    |path| path.display().to_string()
                )
            ),
            Err(error) => tracing::error!("Failed to apply rules from rules file: {error}"),
        }
    }
    apply_kicad_project(request.kicad_project.as_deref(), &mut board, &transform);
    if let Some(path) = std::env::var_os("COPPERROUTE_PAD_CLEARANCE_JSON") {
        let floors: Vec<PadFloor> = serde_json::from_slice(&std::fs::read(&path)?)
            .map_err(|error| OpError::Input(format!("Invalid pad clearance metadata: {error}")))?;
        apply_pad_clearance_metadata(&mut board, &floors)?;
    }
    import_session(request.session.as_deref(), &mut board, &transform);

    job.router_settings = settings.clone();
    job.drc_settings = copper_settings::DesignRulesCheckerSettings::default();
    Ok(Loaded {
        job,
        board,
        transform,
        metadata: parsed.metadata,
        settings,
        warnings: parsed.warnings,
    })
}

fn read_scheduler_rules(job: &RoutingJob, explicit: Option<&Path>) -> Option<Vec<u8>> {
    if let Some(rules) = job.rules.as_ref() {
        let data = rules.get_data();
        if !data.is_empty() {
            return Some(data.to_vec());
        }
    }
    let dsn_path = job.get_input().and_then(|input| {
        (input.format == FileFormat::Dsn).then(|| PathBuf::from(input.get_absolute_path()))
    });
    let path = copper_settings::resolve_scheduler_rules_path(None, explicit, dsn_path.as_deref())?;
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            tracing::warn!("Failed to read rules file {}: {error}", path.display());
            None
        }
    }
}

fn apply_kicad_project(project: Option<&Path>, board: &mut Board, transform: &CoordinateTransform) {
    let Some(project) = project else {
        return;
    };
    let text = match std::fs::read_to_string(project) {
        Ok(text) => text,
        Err(error) => {
            tracing::warn!("KiCad project file {} not read: {error}", project.display());
            return;
        }
    };
    match copper_drc::apply_kicad_project(&text, board, transform) {
        Ok(()) => tracing::info!(
            "KiCad project design rules loaded from {}",
            project.display()
        ),
        Err(error) => tracing::error!("Failed to apply KiCad project design rules: {error}"),
    }
}

fn import_session(session: Option<&Path>, board: &mut Board, transform: &CoordinateTransform) {
    let Some(session) = session else {
        return;
    };
    let bytes = match std::fs::read(session) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!("Session file {} not read: {error}", session.display());
            return;
        }
    };
    if session.to_string_lossy().to_lowercase().ends_with(".json") {
        match copper_dsn::kicad::import_session(&String::from_utf8_lossy(&bytes), board) {
            Ok(()) => tracing::info!("KiCad JSON session loaded from {}", session.display()),
            Err(error) => tracing::error!("Failed to load session file: {error}"),
        }
        return;
    }
    match copper_dsn::ses_reader::read(&bytes[..], board, transform) {
        Ok(summary) => tracing::info!(
            "Session loaded from {}: {} wires, {} vias imported, {} errors",
            session.display(),
            summary.wires_imported,
            summary.vias_imported,
            summary.errors_encountered
        ),
        Err(error) => tracing::error!("Failed to load session file: {error}"),
    }
}

#[derive(serde::Deserialize)]
struct PadFloor {
    component: String,
    pad: String,
    x_um: f64,
    y_um: f64,
    front_um: f64,
    back_um: f64,
    copper_um: Option<f64>,
}

fn apply_pad_clearance_metadata(board: &mut Board, floors: &[PadFloor]) -> Result<(), OpError> {
    let copper_floors = floors
        .iter()
        .map(|floor| {
            let value = floor.copper_um.unwrap_or(0.0);
            let distance = copper_board::Unit::scale(
                value * f64::from(board.communication.resolution.max(1)),
                copper_board::Unit::Um,
                board.communication.unit,
            );
            if !distance.is_finite() || distance < 0.0 || distance > f64::from(i32::MAX) {
                return Err(OpError::Input(format!(
                    "Invalid copper clearance for {}.{}",
                    floor.component, floor.pad
                )));
            }
            Ok(distance.ceil() as i32)
        })
        .collect::<Result<Vec<_>, OpError>>()?;
    let mut matched = std::collections::BTreeSet::new();
    for id in board.get_pins() {
        let Some(copper_board::items::Item::Pin(pin)) = board.get_item(id) else {
            continue;
        };
        let component = &board.components.get(pin.hdr.get_component_id()).name;
        let ctx = board.ctx();
        let center = pin.get_center(&ctx).to_float();
        let Some(name) = pin.name(&ctx) else { continue };
        if let Some((index, floor)) = floors.iter().enumerate().find(|(_, f)| {
            f.component == *component
                && (((center.x - f64::from(board.clearance_override_board_units(f.x_um))).abs()
                    <= 2.0
                    && (center.y + f64::from(board.clearance_override_board_units(f.y_um))).abs()
                        <= 2.0)
                    || (f.pad == name
                        && floors
                            .iter()
                            .filter(|other| other.component == *component && other.pad == name)
                            .count()
                            == 1))
        }) {
            matched.insert(index);
            let mut values = vec![0; board.get_layer_count()];
            values[0] = board.clearance_override_board_units(floor.front_um);
            let last = values.len() - 1;
            values[last] = board.clearance_override_board_units(floor.back_um);
            for (layer, value) in values.iter_mut().enumerate() {
                *value = board
                    .solder_mask_clearance_limit(id, layer, *value)
                    .max(copper_floors[index]);
            }
            board.raise_pin_clearance(id, &values);
        }
    }
    for (index, floor) in floors.iter().enumerate() {
        if !matched.contains(&index) {
            tracing::warn!(
                "Pad clearance metadata did not match {}.{}",
                floor.component,
                floor.pad
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod pad_clearance_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn copper_metadata_is_not_limited_by_the_mask_gap_cap() {
        let input = json!({"resolution":10000,"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
          "nets":[{"id":1,"name":"A"},{"id":2,"name":"B"}],
          "components":[{"reference":"J1","position":{"x":0,"y":0},"layer":"F.Cu","pads":[{"name":"1","netName":"A","shape":"rect","size":{"x":1,"y":1},"layers":["F.Cu","B.Cu"]}]},
          {"reference":"J2","position":{"x":1.2,"y":0},"layer":"F.Cu","pads":[{"name":"1","netName":"B","shape":"rect","size":{"x":1,"y":1},"layers":["F.Cu","B.Cu"]}]}]});
        let copper_dsn::BoardReadResult::Success {
            board: Some(mut board),
            ..
        } = copper_dsn::kicad::read_board(&input.to_string(), None)
        else {
            panic!("fixture must load")
        };
        let floors:Vec<PadFloor>=serde_json::from_value(json!([{"component":"J1","pad":"1","x_um":0,"y_um":0,"front_um":400,"back_um":400,"copper_um":300}])).unwrap();
        let pin = board
            .get_pins()
            .into_iter()
            .find(|id| {
                let item = board.get_item(*id).unwrap();
                board.components.get(item.component_id()).name == "J1"
            })
            .unwrap();
        assert_eq!(board.solder_mask_clearance_limit(pin, 0, 4000), 2000);
        apply_pad_clearance_metadata(&mut board, &floors).unwrap();
        let class = board.get_item(pin).unwrap().clearance_class();
        for layer in 0..2 {
            assert!(
                board
                    .rules
                    .clearance_matrix
                    .get_value(class, 1, layer, false)
                    >= 3000
            );
        }
    }
    #[test]
    fn invalid_copper_metadata_is_rejected_even_without_matching_pads() {
        for value in [-1.0, f64::INFINITY, f64::NAN, 1e30] {
            let input = json!({"resolution":10000,"layers":[{"name":"F.Cu"},{"name":"B.Cu"}]});
            let copper_dsn::BoardReadResult::Success {
                board: Some(mut board),
                ..
            } = copper_dsn::kicad::read_board(&input.to_string(), None)
            else {
                panic!("fixture must load")
            };
            let floor = PadFloor {
                component: "absent".into(),
                pad: "1".into(),
                x_um: 0.0,
                y_um: 0.0,
                front_um: 0.0,
                back_um: 0.0,
                copper_um: Some(value),
            };
            assert!(apply_pad_clearance_metadata(&mut board, &[floor]).is_err());
        }
    }
}
