//! Routes a KiCad board given as its file text and returns the edits to apply to it, so a KiCad
//! plugin can write them back as one undoable change.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use copper_dsn::kicad::changes::{RoutingChanges, routing_changes};
use copper_dsn::kicad::pcb::{self, ImportOptions};
use copper_router::pipeline::{ProgressSink, RouterBudget, RouterStop, RoutingEvent};
use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RouteRequest {
    pub boardText: String,
    pub boardName: String,
    pub projectText: Option<String>,
    pub rulesText: Option<String>,
    pub ripUp: bool,
    pub ripUpNets: Option<Vec<String>>,
    /// Route only these nets; every net when absent.
    pub onlyNets: Option<Vec<String>>,
    pub respectZones: bool,
    pub routeTunedNets: bool,
    pub maxPasses: u32,
    pub timeoutSeconds: u32,
    pub optimize: bool,
}

impl Default for RouteRequest {
    fn default() -> RouteRequest {
        RouteRequest {
            boardText: String::new(),
            boardName: "board".to_string(),
            projectText: None,
            rulesText: None,
            ripUp: false,
            ripUpNets: None,
            onlyNets: None,
            respectZones: true,
            routeTunedNets: false,
            maxPasses: 20,
            timeoutSeconds: 300,
            optimize: true,
        }
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub pass: i32,
    pub routed: i32,
    pub incomplete: i32,
    pub phase: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteResult {
    pub changes: RoutingChanges,
    pub incomplete: i32,
    pub violations: usize,
    pub passes: i32,
    pub timedOut: bool,
    pub cancelled: bool,
    pub skippedNets: Vec<String>,
    pub warnings: Vec<String>,
}

struct Observer<'a> {
    on_progress: &'a mut dyn FnMut(&Progress) -> bool,
    cancel: Rc<Cell<bool>>,
}

impl ProgressSink for Observer<'_> {
    fn on_event(&mut self, event: &RoutingEvent) {
        let RoutingEvent::BoardUpdated { counters } = event else {
            return;
        };
        let progress = Progress {
            pass: counters.pass_count.unwrap_or(0),
            routed: counters.routed_count.unwrap_or(0),
            incomplete: counters.incomplete_count.unwrap_or(0),
            phase: counters.phase.clone().unwrap_or_default(),
        };
        if (self.on_progress)(&progress) {
            self.cancel.set(true);
        }
    }
}

/// `on_progress` returns true to stop routing; what has been routed so far is still returned.
pub fn route(
    request: &RouteRequest,
    on_progress: &mut dyn FnMut(&Progress) -> bool,
) -> Result<RouteResult, String> {
    let mut imported = pcb::read_pcb(
        &request.boardText,
        &request.boardName,
        &pcb::default_net_class(),
    )
    .map_err(|error| error.to_string())?;
    if let Some(project) = request.projectText.as_deref() {
        copper_dsn::kicad::apply_net_classes(&mut imported.board, project).map_err(|error| {
            format!("The KiCad project's net classes could not be read: {error}")
        })?;
    }
    if let Some(rules) = request.rulesText.as_deref() {
        let warnings = copper_dsn::kicad::dru::apply_custom_rules(&mut imported.board, rules)
            .map_err(|error| error.to_string())?;
        imported.warnings.extend(warnings);
    }
    let original = imported.board.clone();
    pcb::apply_import_options(
        &mut imported,
        &ImportOptions {
            rip_up: request.ripUp,
            rip_up_nets: request
                .ripUpNets
                .as_ref()
                .map(|nets| nets.iter().cloned().collect()),
            respect_zones: request.respectZones,
            route_tuned_nets: request.routeTunedNets,
        },
    );
    let kept = imported.board.clone();
    let skipped = imported.skipped_nets.clone();
    let mut warnings = imported.warnings.clone();

    let mut settings = copper_settings::resolve_headless(
        &copper_settings::SettingsInputs::default(),
        None,
        &copper_settings::HostEnvironment::with_processors(1),
    );
    settings.set_max_passes(Some(request.maxPasses.max(1) as i32));
    settings.set_run_optimizer(request.optimize);
    let mut loaded = copper_core::apply_parsed_board_result(
        copper_dsn::kicad::read_board_json(imported.board, None),
        &mut settings,
    )
    .map_err(|error| error.to_string())?;
    warnings.extend(loaded.warnings.iter().cloned());
    pcb::ignore_nets(&mut loaded.board, &skipped);
    if let Some(project) = request.projectText.as_deref() {
        copper_drc::apply_kicad_project(project, &mut loaded.board, &loaded.transform).map_err(
            |error| format!("The KiCad project's design rules could not be read: {error}"),
        )?;
    }
    if let Some(names) = request.onlyNets.as_ref() {
        let wanted: BTreeSet<&str> = names.iter().map(String::as_str).collect();
        let filter: BTreeSet<i32> = loaded
            .board
            .rules
            .nets
            .iter()
            .filter(|net| wanted.contains(net.name.as_str()))
            .map(|net| net.net_number)
            .collect();
        if filter.is_empty() {
            return Err("None of the selected nets are on this board.".to_string());
        }
        settings.set_net_filter(Some(filter));
    }

    let cancel = Rc::new(Cell::new(false));
    let poll = Rc::clone(&cancel);
    let timeout_ms =
        i32::try_from(u64::from(request.timeoutSeconds.max(1)) * 1000).unwrap_or(i32::MAX);
    let stop = RouterStop::with_deadline(timeout_ms).with_cancel_poll(Box::new(move |stop| {
        if poll.get() {
            stop.request_stop();
        }
    }));
    let mut observer = Observer {
        on_progress,
        cancel: Rc::clone(&cancel),
    };
    let pipeline = copper_router::pipeline::run_pipeline(
        &mut loaded.board,
        &settings,
        &stop,
        RouterBudget::default(),
        &mut observer,
    )
    .map_err(|error| error.to_string())?;
    let violations = copper_drc::DesignRulesChecker::new(&mut loaded.board)
        .get_all_violations()
        .len();
    stop.poll_deadline();

    Ok(RouteResult {
        changes: routing_changes(&original, &kept, &loaded.board),
        incomplete: pipeline
            .final_statistics
            .connections
            .incomplete_count
            .unwrap_or(0),
        violations,
        passes: pipeline.router_passes_completed,
        timedOut: stop.is_timed_out(),
        cancelled: cancel.get(),
        skippedNets: skipped.into_iter().collect(),
        warnings,
    })
}

/// The JSON form of [`route`], for bindings in other languages.
pub fn route_json(
    request_json: &str,
    on_progress: &mut dyn FnMut(&Progress) -> bool,
) -> Result<String, String> {
    let request: RouteRequest = serde_json::from_str(request_json)
        .map_err(|error| format!("Invalid routing request: {error}"))?;
    let result = route(&request, on_progress)?;
    serde_json::to_string(&result).map_err(|error| error.to_string())
}

pub fn progress_json(progress: &Progress) -> Result<String, String> {
    serde_json::to_string(progress).map_err(|error| error.to_string())
}
