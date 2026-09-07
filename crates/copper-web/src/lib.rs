//! In-memory browser entry point. Native filesystem and CLI code are not used.
use wasm_bindgen::prelude::*;
mod dsn;

#[wasm_bindgen]
pub fn route_board(
    json: &str,
    passes: u32,
    seconds: u32,
    project: &str,
    on_progress: js_sys::Function,
    routing_layers: &str,
    routing_nets: &str,
) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    route(
        json,
        passes,
        seconds,
        project,
        on_progress,
        routing_layers,
        routing_nets,
    )
    .map_err(|error| JsValue::from_str(&error))
}

fn route(
    json: &str,
    passes: u32,
    seconds: u32,
    project: &str,
    on_progress: js_sys::Function,
    routing_layers: &str,
    routing_nets: &str,
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
    if !routing_nets.is_empty() {
        let filter = net_filter_from_names(&loaded.board.rules.nets, routing_nets)?;
        settings.set_net_filter(Some(filter));
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
    let airline_nets: Vec<i32> = {
        let mut drc = copper_drc::DesignRulesChecker::new(&mut loaded.board);
        drc.calculate_all_incompletes();
        drc.get_all_airlines()
            .iter()
            .map(|airline| airline.net_number)
            .collect()
    };
    let unselected_incomplete =
        incomplete_on_unselected_nets(&airline_nets, settings.net_filter.as_ref());
    let board: serde_json::Value =
        serde_json::from_str(&copper_dsn::kicad::write(&loaded.board, "browser-board"))
            .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "board": board,
        "incomplete": result.incomplete_count(),
        "incompleteOnUnselectedNets": unselected_incomplete,
        "violations": result.violation_count(),
        "drcDetails": violation_details(&loaded.board, &loaded.transform, &result.drc_violations),
        "initialDrcDetails": initial_drc,
        "timedOut": result.timed_out,
        "passes": result.pipeline.router_passes_completed,
    })
    .to_string())
}

fn incomplete_on_unselected_nets(
    airline_nets: &[i32],
    filter: Option<&std::collections::BTreeSet<i32>>,
) -> usize {
    let Some(filter) = filter else {
        return 0;
    };
    airline_nets
        .iter()
        .filter(|net| !filter.contains(net))
        .count()
}

fn net_filter_from_names(
    nets: &copper_board::Nets,
    routing_nets: &str,
) -> Result<std::collections::BTreeSet<i32>, String> {
    let selected: Vec<String> = serde_json::from_str(routing_nets)
        .map_err(|error| format!("Invalid routing nets: {error}"))?;
    if selected.is_empty() {
        return Err("Select at least one net to route".into());
    }
    let mut filter = std::collections::BTreeSet::new();
    for name in &selected {
        let matches = nets.get_by_name(name);
        if matches.is_empty() {
            return Err(format!("The board has no net named {name}"));
        }
        filter.extend(matches.iter().map(|net| net.net_number));
    }
    Ok(filter)
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

#[cfg(test)]
mod tests {
    use super::*;
    use copper_board::{NetClassId, Nets};

    fn board_nets() -> Nets {
        let mut nets = Nets::new();
        nets.add("GND", 1, false, NetClassId(0));
        nets.add("+5V", 1, false, NetClassId(0));
        nets.add("/D0", 1, false, NetClassId(0));
        nets
    }

    #[test]
    fn named_nets_resolve_to_their_board_net_numbers() {
        let filter = net_filter_from_names(&board_nets(), r#"["/D0","GND"]"#).unwrap();
        assert_eq!(filter, [1, 3].into_iter().collect());
    }

    #[test]
    fn an_unknown_net_name_is_rejected() {
        let error = net_filter_from_names(&board_nets(), r#"["GND","SPI_MISO"]"#).unwrap_err();
        assert!(error.contains("SPI_MISO"), "{error}");
    }

    #[test]
    fn incompletes_are_split_by_whether_their_net_was_selected() {
        let airlines = [1, 1, 2, 3, 3, 3];
        let filter: std::collections::BTreeSet<i32> = [1].into_iter().collect();
        assert_eq!(incomplete_on_unselected_nets(&airlines, Some(&filter)), 4);
    }

    #[test]
    fn without_a_filter_no_incomplete_belongs_to_an_unselected_net() {
        assert_eq!(incomplete_on_unselected_nets(&[1, 2, 3], None), 0);
    }

    #[test]
    fn an_empty_net_selection_is_rejected() {
        assert!(net_filter_from_names(&board_nets(), "[]").is_err());
    }
}
