use copper_board::{Board, Item};
use copper_geometry::{FloatPoint, Shape, ShapeOps};
use copper_settings::SettingsSource;
use wasm_bindgen::prelude::*;

fn load(text: &str, name: &str) -> Result<copper_core::ParsedBoard, String> {
    copper_core::parse_board_result(copper_dsn::read_board(
        text.as_bytes(),
        None,
        Some(name),
        &copper_dsn::DsnReadOptions::default(),
    ))
    .map_err(|e| e.to_string())
}

#[wasm_bindgen]
pub fn preview_dsn(text: &str, name: &str) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    let loaded = load(text, name).map_err(|e| JsValue::from_str(&e))?;
    Ok(serde_json::json!({"svg": svg(&loaded.board), "layers": layers(&loaded.board), "warnings": loaded.warnings}).to_string())
}

#[wasm_bindgen]
pub fn route_dsn(
    text: &str,
    name: &str,
    passes: u32,
    seconds: u32,
    callback: js_sys::Function,
) -> Result<String, JsValue> {
    route(text, name, passes, seconds, callback).map_err(|e| JsValue::from_str(&e))
}

fn route(
    text: &str,
    name: &str,
    passes: u32,
    seconds: u32,
    callback: js_sys::Function,
) -> Result<String, String> {
    console_error_panic_hook::set_once();
    if !(1..=100).contains(&passes) || !(1..=600).contains(&seconds) {
        return Err("Use 1–100 passes and a 1–600 second time limit".into());
    }
    let mut loaded = load(text, name)?;
    if !loaded.warnings.is_empty() {
        return Err(loaded.warnings.join("\n"));
    }
    let source = copper_settings::sources::DsnFileSettings::new(text.as_bytes(), name);
    let mut settings = copper_settings::resolve_headless(
        &copper_settings::SettingsInputs {
            dsn: source.get_settings(),
            ..Default::default()
        },
        Some(&loaded.board),
        &copper_settings::HostEnvironment::with_processors(1),
    );
    settings.set_run_router(true);
    settings.set_run_optimizer(false);
    settings.set_max_passes(Some(passes as i32));
    // Match the PCB route-from-scratch workflow, including protected routing.
    let routing = loaded
        .board
        .get_traces()
        .into_iter()
        .chain(loaded.board.get_vias())
        .collect::<Vec<_>>();
    for id in &routing {
        if let Some(item) = loaded.board.get_item_mut(*id) {
            item.set_fixed_state(copper_board::FixedState::Unfixed);
        }
    }
    if !loaded.board.remove_items(routing) {
        return Err("Could not remove all existing routing".into());
    }
    let mut observer = DsnProgress(callback);
    observer.send(serde_json::json!({"type":"preview", "svg":svg(&loaded.board), "warnings":[]}));
    let sink = copper_core::SyncProgressSink::noop();
    let mut ctx = copper_core::Ctx::new(&settings, &sink);
    ctx.cancel = ctx
        .cancel
        .with_deadline_from(copper_core::Deadline::in_seconds(seconds as i64));
    let result =
        copper_core::RoutingPipeline::run_with_progress(&mut loaded.board, &ctx, &mut observer)
            .map_err(|e| e.to_string())?;
    let mut dsn = Vec::new();
    copper_dsn::dsn_writer::write_with_settings(
        &loaded.board,
        &loaded.transform,
        &mut dsn,
        name,
        false,
        Some(&copper_dsn::parser::DsnRouterSettings::from(&settings)),
    )
    .map_err(|e| e.to_string())?;
    let mut ses = Vec::new();
    copper_dsn::ses_writer::write(&loaded.board, &loaded.transform, &mut ses, name)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "svg":svg(&loaded.board), "dsn":String::from_utf8(dsn).map_err(|e|e.to_string())?,
        "ses":String::from_utf8(ses).map_err(|e|e.to_string())?,
        "incomplete":result.incomplete_count(), "violations":result.violation_count(),
        "passes":result.pipeline.router_passes_completed, "timedOut":result.timed_out,
        "warnings":["DSN rules and layer settings are imported. Import the SES into the source PCB editor and run its DRC; source-editor metadata is not part of DSN."]
    }).to_string())
}
struct DsnProgress(js_sys::Function);
impl DsnProgress {
    fn send(&mut self, value: serde_json::Value) {
        let _ = self
            .0
            .call1(&JsValue::NULL, &JsValue::from_str(&value.to_string()));
    }
}
impl copper_core::ProgressSink for DsnProgress {
    fn on_board_update(&mut self, board: &Board, event: &copper_core::RoutingEvent) {
        if let copper_core::RoutingEvent::BoardUpdated { counters } = event {
            self.send(serde_json::json!({"type":"progress", "svg":svg(board), "pass":counters.pass_count, "incomplete":counters.incomplete_count,"routed":counters.routed_count}));
        }
    }
}
fn color(board: &Board, layer: usize) -> &'static str {
    let name = &board.layer_structure().layers[layer].name;
    if name == "F.Cu" || layer == 0 {
        "#f26d78"
    } else if name == "B.Cu" || layer + 1 == board.get_layer_count() {
        "#68b8ff"
    } else {
        ["#ce9fff", "#ffa657", "#83c5af", "#ff8fc8"][(layer - 1) % 4]
    }
}
fn layers(board: &Board) -> Vec<serde_json::Value> {
    board.layer_structure().layers.iter().enumerate().map(|(i,l)| serde_json::json!({"name":l.name,"label":l.name,"type":if l.is_signal {"signal"} else {"power"},"color":color(board,i)})).collect()
}
fn points(ps: &[FloatPoint]) -> String {
    ps.iter()
        .map(|p| format!("{},{}", p.x, -p.y))
        .collect::<Vec<_>>()
        .join(" ")
}
fn shape_svg(shape: &Shape, fill: &str, opacity: f64) -> String {
    match shape {
        Shape::Circle(c) => format!(
            r#"<circle cx="{}" cy="{}" r="{}" fill="{fill}" opacity="{opacity}"/>"#,
            c.center.x,
            -f64::from(c.center.y),
            c.radius
        ),
        _ => format!(
            r#"<polygon points="{}" fill="{fill}" opacity="{opacity}"/>"#,
            points(&shape.corner_approx_arr())
        ),
    }
}
fn svg(board: &Board) -> String {
    let bbox = board.get_bounding_box();
    let margin = f64::from(bbox.width().max(bbox.height()).max(1)) * 0.03;
    let x = f64::from(bbox.ll.x) - margin;
    let y = -f64::from(bbox.ur.y) - margin;
    let w = f64::from(bbox.width().max(1)) + margin * 2.0;
    let h = f64::from(bbox.height().max(1)) + margin * 2.0;
    let mut out = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x} {y} {w} {h}" role="img" aria-label="DSN board preview"><rect x="{x}" y="{y}" width="{w}" height="{h}" fill="#101c25"/>"##
    );
    let ctx = board.ctx();
    for item in board.get_items() {
        match item {
            Item::BoardOutline(outline) => {
                for i in 0..outline.shape_count() {
                    if let Some(shape) = outline.get_shape(i) {
                        out.push_str(&format!(
                            r##"<polygon points="{}" fill="none" stroke="#83c5af" stroke-width="{}"/>"##,
                            points(&shape.as_ops().corner_approx_arr()), margin / 12.0
                        ));
                    }
                }
            }
            Item::Trace(trace) => {
                out.push_str(&format!(
                    r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                    points(&trace.polyline().corner_approx_arr()),
                    color(board, trace.get_layer()), 2 * trace.get_half_width()
                ));
            }
            Item::Pin(pin) => {
                for layer in pin.first_layer(&ctx)..=pin.last_layer(&ctx) {
                    if let Some(shape) = pin.get_shape_on_layer(layer, &ctx) {
                        out.push_str(&shape_svg(&shape, "#edcf86", 1.0));
                    }
                }
                if pin.first_layer(&ctx) != pin.last_layer(&ctx) {
                    if let Some(stack) = pin.get_padstack(&ctx) {
                        out.push_str(&drill_svg(
                            pin.get_center(&ctx).to_float(),
                            stack.drill_radius(),
                        ));
                    }
                }
            }
            Item::Via(via) => {
                if let Some(shape) = via.get_shape_on_layer(via.first_layer(&ctx), &ctx) {
                    out.push_str(&shape_svg(&shape, "#edcf86", 1.0));
                }
                if let Some(stack) = via.get_padstack(&ctx) {
                    out.push_str(&drill_svg(
                        via.get_center().to_float(),
                        stack.drill_radius(),
                    ));
                }
            }
            _ => {
                for i in 0..item.tile_shape_count(&ctx) {
                    if let Some(shape) = board.item_tile_shape_ref(item.id(), i) {
                        out.push_str(&format!(
                            r##"<polygon points="{}" fill="#83c5af" opacity="0.15"/>"##,
                            points(&shape.corner_approx_arr())
                        ));
                    }
                }
            }
        }
    }
    out.push_str(&airline_svg(
        &copper_drc::all_airlines(board),
        margin / 20.0,
    ));
    out.push_str("</svg>");
    out
}

fn airline_svg(airlines: &[copper_drc::AirLine], width: f64) -> String {
    if airlines.is_empty() {
        return String::new();
    }
    let d = airlines
        .iter()
        .map(|a| {
            format!(
                "M {} {} L {} {}",
                a.from_corner.x, -a.from_corner.y, a.to_corner.x, -a.to_corner.y
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        r##"<g class="ratsnest" fill="none" stroke="#e8f0f2" stroke-width="{width}" stroke-dasharray="{} {}" opacity="0.4"><path d="{d}"/></g>"##,
        width * 6.0,
        width * 4.0
    )
}

fn drill_svg(center: FloatPoint, radius: f64) -> String {
    format!(
        r##"<circle cx="{}" cy="{}" r="{radius}" fill="#101c25"/>"##,
        center.x, -center.y
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use copper_board::ItemId;
    use copper_drc::AirLine;

    fn airline(from: (f64, f64), to: (f64, f64)) -> AirLine {
        AirLine::new(
            1,
            ItemId(0),
            FloatPoint {
                x: from.0,
                y: from.1,
            },
            ItemId(1),
            FloatPoint { x: to.0, y: to.1 },
        )
    }

    #[test]
    fn airlines_are_drawn_as_one_path_with_the_y_flip_the_rest_of_the_svg_uses() {
        let svg = airline_svg(&[airline((0.0, 5.0), (10.0, 20.0))], 1.0);
        assert!(svg.contains(r#"<path d="M 0 -5 L 10 -20"/>"#), "{svg}");
        assert!(svg.contains(r#"class="ratsnest""#), "{svg}");
    }

    #[test]
    fn several_airlines_share_the_one_path() {
        let svg = airline_svg(
            &[
                airline((0.0, 4.0), (1.0, 1.0)),
                airline((2.0, 2.0), (3.0, 3.0)),
            ],
            1.0,
        );
        assert!(svg.contains(r#"d="M 0 -4 L 1 -1 M 2 -2 L 3 -3""#), "{svg}");
    }

    #[test]
    fn a_fully_routed_board_draws_no_ratsnest_group_at_all() {
        assert_eq!(airline_svg(&[], 1.0), "");
    }
}
