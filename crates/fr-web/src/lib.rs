//! In-memory browser entry point. Native filesystem and CLI code are not used.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn route_board(
    json: &str,
    passes: u32,
    seconds: u32,
    project: &str,
    on_progress: js_sys::Function,
    routing_layers: &str,
) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    route(json, passes, seconds, project, on_progress, routing_layers)
        .map_err(|error| JsValue::from_str(&error))
}

fn route(
    json: &str,
    passes: u32,
    seconds: u32,
    project: &str,
    on_progress: js_sys::Function,
    routing_layers: &str,
) -> Result<String, String> {
    if !(1..=100).contains(&passes) || !(1..=600).contains(&seconds) {
        return Err("Use 1–100 passes and a 1–600 second time limit".into());
    }
    let mut settings = copper_settings::resolve_headless(
        &copper_settings::SettingsInputs::default(),
        None,
        &copper_settings::HostEnvironment::with_processors(1),
    );
    settings.set_run_optimizer(false);
    settings.set_max_passes(Some(passes as i32));
    let mut loaded =
        copper_core::apply_parsed_board_result(copper_dsn::kicad::read_board(json, None), &mut settings)
            .map_err(|error| error.to_string())?;
    // A partial import must never silently become a downloadable board.
    if !loaded.warnings.is_empty() {
        return Err(loaded.warnings.join("\n"));
    }
    if !project.is_empty() {
        copper_drc::apply_kicad_project(project, &mut loaded.board, &loaded.transform)
            .map_err(|error| error.to_string())?;
    }
    if !routing_layers.is_empty() {
        let allowed: Vec<String> = serde_json::from_str(routing_layers)
            .map_err(|error| format!("Invalid routing layers: {error}"))?;
        let layers = &loaded.board.layer_structure().layers;
        if allowed.is_empty()
            || allowed
                .iter()
                .any(|name| !layers.iter().any(|l| l.name == *name && l.is_signal))
        {
            return Err("Routing layers must name existing signal copper layers".into());
        }
        for (index, layer) in layers.iter().enumerate() {
            settings.set_layer_active(index, layer.is_signal && allowed.contains(&layer.name));
        }
    }
    let initial_violations =
        copper_drc::DesignRulesChecker::new(&mut loaded.board).get_all_violations();
    let initial_drc = violation_details(&loaded.board, &loaded.transform, &initial_violations);
    let progress = copper_core::SyncProgressSink::noop();
    let mut ctx = copper_core::Ctx::new(&settings, &progress);
    ctx.cancel = ctx
        .cancel
        .with_deadline_from(copper_core::Deadline::in_seconds(seconds as i64));
    let mut observer = BrowserProgress {
        callback: on_progress,
    };
    let result =
        copper_core::RoutingPipeline::run_with_progress(&mut loaded.board, &ctx, &mut observer)
            .map_err(|error| error.to_string())?;
    let board: serde_json::Value =
        serde_json::from_str(&copper_dsn::kicad::write(&loaded.board, "browser-board"))
            .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "board": board,
        "incomplete": result.incomplete_count(),
        "violations": result.violation_count(),
        "drcDetails": violation_details(&loaded.board, &loaded.transform, &result.drc_violations),
        "initialDrcDetails": initial_drc,
        "timedOut": result.timed_out,
        "passes": result.pipeline.router_passes_completed,
    })
    .to_string())
}

/// Runs in the Web Worker, so callbacks can post frames while WASM is busy.
struct BrowserProgress {
    callback: js_sys::Function,
}

impl copper_core::ProgressSink for BrowserProgress {
    fn on_board_update(&mut self, board: &copper_board::Board, event: &copper_core::RoutingEvent) {
        let copper_core::RoutingEvent::BoardUpdated { counters } = event else {
            return;
        };
        let board: serde_json::Value = serde_json::from_str(&copper_dsn::kicad::write(board, "live"))
            .expect("the board writer produces valid JSON");
        let frame = serde_json::json!({
            "board": board,
            "pass": counters.pass_count,
            "incomplete": counters.incomplete_count,
            "routed": counters.routed_count,
            "queued": counters.queued_to_be_routed_count,
        });
        // Preview failure must not interrupt the route or change the board.
        let _ = self
            .callback
            .call1(&JsValue::NULL, &JsValue::from_str(&frame.to_string()));
    }
}

fn violation_details(
    board: &copper_board::Board,
    transform: &copper_dsn::CoordinateTransform,
    violations: &[copper_drc::DrcViolation],
) -> Vec<serde_json::Value> {
    violations.iter().map(|v| {
        let describe = |id| serde_json::json!({
            "id": format!("{id:?}"),
            "description": copper_drc::report::item_description(board, id),
            "component": board.get_item(id).map(|item| item.component_id()),
        });
        let point = transform.board_to_dsn_point(&v.position);
        serde_json::json!({
            "kind": v.kind.kicad_type(),
            "layer": v.layer.map(|l| &board.layer_structure().layers[l].name),
            "position": [point[0], -point[1]],
            "units": "mm",
            "expected": transform.board_to_dsn(v.expected),
            "actual": transform.board_to_dsn(v.actual),
            "estimated": v.estimated,
            "involvesRouting": v.involves_routing(board),
            "items": std::iter::once(v.first_item).chain(v.second_item).map(describe).collect::<Vec<_>>(),
        })
    }).collect()
}
