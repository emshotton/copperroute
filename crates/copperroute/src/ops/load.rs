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
