//! `nets`, `outline`, `traces`, `vias`, `conductionAreas` — and leaves `hostCad`, `hostVersion`

use copper_board::{Board, Item, Unit};
use copper_geometry::ShapeOps;

use super::dto::{
    ConductionAreaJson, KiCadBoardJson, LayerJson, NetClassJson, NetJson, OutlineJson, Point2D,
    TraceJson, UnitJson, ViaJson,
};
use crate::format::json::to_gson_string_pretty;

pub const DEFAULT_DESIGN_NAME: &str = "KiCad_Design";

#[must_use]
pub fn write(board: &Board, design_name: &str) -> String {
    let scale_factor = match board.communication.unit {
        Unit::Mil => 254.0,
        Unit::Um => 10.0,
        _ => 10_000.0,
    };

    let mut board_json = KiCadBoardJson {
        allowSolderMaskBridgesInFootprints: board.rules.allow_solder_mask_bridges_in_footprints,
        solderMaskMinWidth: board
            .rules
            .solder_mask_min_width
            .map(|v| f64::from(v) / scale_factor),
        viaInPadAllowed: board
            .rules
            .net_classes
            .iter()
            .next()
            .is_some_and(class_via_attachment),
        designName: Some(design_name.to_string()),
        resolution: scale_factor,
        unit: Some(match board.communication.unit {
            Unit::Mil => UnitJson::MIL,
            Unit::Um => UnitJson::UM,
            _ => UnitJson::MM,
        }),
        ..KiCadBoardJson::default()
    };
    let layers = board_json.layers.as_mut().expect("`new ArrayList<>()`");

    for i in 0..board.get_layer_count() {
        let layer = &board.layer_structure().layers[i];
        layers.push(LayerJson {
            index: i32::try_from(i).unwrap_or(i32::MAX),
            name: Some(layer.name.clone()),
            r#type: Some(if layer.is_signal { "signal" } else { "plane" }.to_string()),
        });
    }

    for i in 1..=board.rules.nets.max_net_number() {
        let Some(net) = board.rules.nets.get(i) else {
            continue;
        };
        board_json
            .nets
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(NetJson {
                id: net.net_number,
                name: Some(net.name.clone()),
                className: Some(
                    board
                        .rules
                        .net_classes
                        .get(net.get_net_class())
                        .get_name()
                        .to_string(),
                ),
                containsPlane: net.contains_plane(),
            });
    }

    for i in 0..board.rules.net_classes.count() {
        let net_class = board.rules.net_classes.get(copper_board::NetClassId(i));
        let clearance_class_index = net_class.get_trace_clearance_class();
        let clearance = f64::from(board.rules.clearance_matrix.get_value(
            clearance_class_index,
            clearance_class_index,
            0,
            false,
        )) / scale_factor;
        let trace_width =
            f64::from(net_class.get_trace_half_width(0).wrapping_mul(2)) / scale_factor;

        let mut via_diameter = 0.8;
        let mut explicit_drill = None;
        if let Some(via_rule) = net_class.get_via_rule()
            && via_rule.via_count() > 0
        {
            let via_info = via_rule.get_via(0);
            if let Some(via_pad) = board.library.padstacks.get(via_info.get_padstack())
                && let Some(shape) = via_pad.get_shape(0)
            {
                via_diameter = f64::from(shape.bounding_box().width()) / scale_factor;
                explicit_drill = via_pad.drill_diameter.map(|d| d / scale_factor);
            }
        }
        let via_drill = explicit_drill.unwrap_or(via_diameter * 0.5);

        let mut net_names: Vec<String> = Vec::new();
        for n in 1..=board.rules.nets.max_net_number() {
            if let Some(net) = board.rules.nets.get(n)
                && net.get_net_class() == copper_board::NetClassId(i)
            {
                net_names.push(net.name.clone());
            }
        }

        board_json
            .netClasses
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(NetClassJson {
                viaInPadAllowed: (class_via_attachment(net_class) != board_json.viaInPadAllowed)
                    .then_some(class_via_attachment(net_class)),
                name: Some(net_class.get_name().to_string()),
                clearance,
                traceWidth: trace_width,
                viaDiameter: via_diameter,
                viaDrill: via_drill,
                netNames: Some(net_names),
            });
    }

    if let Some(Item::BoardOutline(outline)) = board.get_outline().and_then(|id| board.get_item(id))
    {
        let clearance_class_index = outline.hdr.clearance_class();
        let mut corners: Vec<Point2D> = Vec::new();
        for i in 0..outline.shape_count() {
            let Some(poly_shape) = outline.get_shape(i) else {
                continue;
            };
            for pt in poly_shape.as_ops().bounded_corners() {
                corners.push(Point2D {
                    x: pt.to_float().x / scale_factor,
                    y: -pt.to_float().y / scale_factor,
                });
            }
        }
        board_json.outline = Some(OutlineJson {
            ordered: false,
            cutouts: None,
            corners: Some(corners),
            clearance: f64::from(board.rules.clearance_matrix.get_value(
                clearance_class_index,
                clearance_class_index,
                0,
                false,
            )) / scale_factor,
        });
    }

    let mut trace_id = 1;
    for id in board.get_traces() {
        let Some(Item::Trace(poly_trace)) = board.get_item(id) else {
            continue;
        };
        let net_name = (poly_trace.hdr.net_count() > 0)
            .then(|| board.rules.nets.get(poly_trace.hdr.get_net_number(0)))
            .flatten()
            .map(|net| net.name.clone());
        let points = poly_trace
            .polyline()
            .corners()
            .iter()
            .map(|pt| Point2D {
                x: pt.to_float().x / scale_factor,
                y: -pt.to_float().y / scale_factor,
            })
            .collect();
        board_json
            .traces
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(TraceJson {
                id: trace_id,
                netName: net_name,
                width: f64::from(poly_trace.get_half_width().wrapping_mul(2)) / scale_factor,
                layerIndex: i32::try_from(poly_trace.get_layer()).unwrap_or(i32::MAX),
                points: Some(points),
            });
        trace_id += 1;
    }

    let layer_count = board.get_layer_count();
    let mut via_id = 1;
    let ctx = board.ctx();
    for id in board.get_vias() {
        let Some(Item::Via(via)) = board.get_item(id) else {
            continue;
        };
        let net_name = (via.hdr.net_count() > 0)
            .then(|| board.rules.nets.get(via.hdr.get_net_number(0)))
            .flatten()
            .map(|net| net.name.clone());
        let center = via.get_center().to_float();
        let position = Point2D {
            x: center.x / scale_factor,
            y: -center.y / scale_factor,
        };

        let padstack = via.get_padstack(&ctx);
        let mut first_layer = 0usize;
        while first_layer < layer_count
            && padstack
                .and_then(|p| p.get_shape(i32::try_from(first_layer).unwrap_or(i32::MAX)))
                .is_none()
        {
            first_layer += 1;
        }
        let mut last_layer = i64::try_from(layer_count).unwrap_or(i64::MAX) - 1;
        while last_layer >= 0
            && padstack
                .and_then(|p| p.get_shape(i32::try_from(last_layer).unwrap_or(i32::MAX)))
                .is_none()
        {
            last_layer -= 1;
        }

        let diameter = padstack
            .and_then(|p| p.get_shape(i32::try_from(first_layer).unwrap_or(i32::MAX)))
            .map_or(0.8, |shape| {
                f64::from(shape.bounding_box().width()) / scale_factor
            });

        board_json
            .vias
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(ViaJson {
                id: via_id,
                netName: net_name,
                position: Some(position),
                diameter,
                drill: padstack
                    .and_then(|p| p.drill_diameter)
                    .map_or(diameter * 0.5, |d| d / scale_factor),
                startLayerIndex: i32::try_from(first_layer).unwrap_or(i32::MAX),
                endLayerIndex: i32::try_from(last_layer).unwrap_or(i32::MAX),
            });
        via_id += 1;
    }

    let mut area_id = 1;
    for id in board.get_conduction_areas() {
        let Some(Item::ConductionArea(area)) = board.get_item(id) else {
            continue;
        };
        let net_name = (area.hdr.net_count() > 0)
            .then(|| board.rules.nets.get(area.hdr.get_net_number(0)))
            .flatten()
            .map(|net| net.name.clone());
        let polygon = area
            .get_area(&ctx)
            .corner_approx_arr()
            .into_iter()
            .map(|pt| Point2D {
                x: pt.x / scale_factor,
                y: -pt.y / scale_factor,
            })
            .collect();
        board_json
            .conductionAreas
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(ConductionAreaJson {
                id: area_id,
                netName: net_name,
                layerIndex: i32::try_from(area.get_layer()).unwrap_or(i32::MAX),
                isObstacle: area.get_is_obstacle(),
                polygon: Some(polygon),
            });
        area_id += 1;
    }

    to_gson_string_pretty(&board_json).expect("every double the writer builds is finite")
}

fn class_via_attachment(class: &copper_board::rules::NetClass) -> bool {
    class
        .get_via_rule()
        .and_then(|r| r.iter().next())
        .is_some_and(|via| via.attach_smd_allowed())
}
