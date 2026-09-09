use std::collections::BTreeSet;

use copper_board::{Item, Unit};
use copper_drc::DesignRulesChecker;
use copper_dsn::{BoardReadResult, DsnReadOptions};
use serde::Deserialize;

#[derive(Deserialize)]
struct Pad {
    component: String,
    pad: String,
    x_um: f64,
    y_um: f64,
    front_um: Option<f64>,
    back_um: Option<f64>,
}

fn matches(pads: &[Pad], component: &str, name: &str, x: f64, y: f64) -> Vec<usize> {
    let mut physical: Vec<_> = pads
        .iter()
        .enumerate()
        .filter(|(_, pad)| {
            pad.component == component && (pad.x_um - x).abs() <= 1.0 && (pad.y_um - y).abs() <= 1.0
        })
        .map(|(i, _)| i)
        .collect();
    let named: Vec<_> = pads
        .iter()
        .enumerate()
        .filter(|(_, pad)| pad.component == component && pad.pad == name)
        .map(|(i, _)| i)
        .collect();
    if physical.is_empty() && named.len() == 1 {
        physical = named;
    }
    physical
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        return Err(
            "usage: check_mask_session INPUT.dsn ROUTED.ses PADS.json PROJECT.kicad_pro".into(),
        );
    }
    let data = std::fs::read(&args[1])?;
    let (mut board, transform) =
        match copper_dsn::read_board(&data[..], None, None, &DsnReadOptions::default()) {
            BoardReadResult::Success {
                board: Some(board),
                coordinate_transform: Some(transform),
                ..
            }
            | BoardReadResult::OutlineMissing {
                board: Some(board),
                coordinate_transform: Some(transform),
                ..
            } => (*board, transform),
            result => return Err(format!("DSN load failed: {result:?}").into()),
        };
    let routing: Vec<_> = board
        .items_in_board_order()
        .into_iter()
        .filter(|id| matches!(board.get_item(*id), Some(Item::Trace(_) | Item::Via(_))))
        .collect();
    for id in routing {
        board.remove_item(id);
    }
    copper_dsn::ses_reader::read(std::fs::File::open(&args[2])?, &mut board, &transform)?;
    copper_drc::apply_kicad_project(&std::fs::read_to_string(&args[4])?, &mut board, &transform)?;
    let pads: Vec<Pad> = serde_json::from_slice(&std::fs::read(&args[3])?)?;
    let mut matched = BTreeSet::new();
    let unit = board.communication.unit;
    let mut unmatched_pins = Vec::new();
    for id in board.get_pins() {
        let Some(Item::Pin(pin)) = board.get_item(id) else {
            continue;
        };
        let ctx = board.ctx();
        let component = &board.components.get(pin.hdr.get_component_id()).name;
        let name = pin.name(&ctx).unwrap_or("");
        let center = transform.board_to_dsn_point(&pin.get_center(&ctx).to_float());
        let indices = matches(
            &pads,
            component,
            name,
            Unit::scale(center[0], unit, Unit::Um),
            -Unit::scale(center[1], unit, Unit::Um),
        );
        if indices.is_empty() {
            unmatched_pins.push(format!("{component}.{name}"));
        }
        let last = board.get_layer_count() - 1;
        let Some(Item::Pin(pin)) = board.items.get_mut(&id) else {
            unreachable!()
        };
        for index in indices {
            matched.insert(index);
            for (layer, value) in [(0, pads[index].front_um), (last, pads[index].back_um)] {
                if let Some(value) = value {
                    let expansion = transform
                        .dsn_to_board(Unit::scale(value, Unit::Um, unit))
                        .round() as i32;
                    pin.solder_mask_expansion
                        .entry(layer)
                        .and_modify(|existing| *existing = (*existing).max(expansion))
                        .or_insert(expansion);
                }
            }
        }
    }
    let violations = DesignRulesChecker::new(&mut board).get_all_violations();
    let rows: Vec<_> = violations.iter().filter(|v| v.kind.kicad_type() == "solder_mask_bridge").map(|v| {
        let pos = transform.board_to_dsn_point(&v.position);
        let Some(Item::Pin(pin)) = board.get_item(v.first_item) else { unreachable!() };
        let ctx = board.ctx();
        let component = &board.components.get(pin.hdr.get_component_id()).name;
        let pad = pin.name(&ctx).unwrap_or("");
        let other = board.get_item(v.second_item.unwrap()).unwrap();
        let nets: Vec<_> = other.net_nos().iter().filter_map(|n| board.rules.nets.get(*n).map(|net| &net.name)).collect();
        serde_json::json!({"first":v.first_item.0,"second":v.second_item.map(|id|id.0),"layer":v.layer,
            "component":component,"pad":pad,"other_nets":nets,
            "x_mm":Unit::scale(pos[0],unit,Unit::Mm),"y_mm":-Unit::scale(pos[1],unit,Unit::Mm),"shortfall":v.shortfall()})
    }).collect();
    println!(
        "{}",
        serde_json::json!({"matched_pads":matched.len(),"total_pads":pads.len(),"unmatched_pins":unmatched_pins,"mask_violations":rows})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_pad_names_survive_dsn_placement_rounding() {
        let pads: Vec<Pad> = serde_json::from_value(serde_json::json!([
            {"component":"U1","pad":"9","x_um":136323.029,"y_um":110241.005,"front_um":200.0},
            {"component":"U1","pad":"9","x_um":135148.029,"y_um":110241.005,"front_um":200.0}
        ]))
        .unwrap();
        assert_eq!(matches(&pads, "U1", "9@1", 136323.5, 110241.5), vec![0]);
        assert!(matches(&pads, "U2", "9@1", 136323.5, 110241.5).is_empty());
    }
}
