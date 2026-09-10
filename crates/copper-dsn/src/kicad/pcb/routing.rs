use std::f64::consts::PI;

use super::PcbError;
use super::structure::{Layers, NetTable};
use crate::kicad::sexpr::{Node, Value};
use crate::kicad::{ConductionAreaJson, Point2D, TraceJson, ViaJson};

const SECTION: &str = "routing";

fn number(text: &str) -> Result<f64, PcbError> {
    super::numeric::number(SECTION, text)
}

fn point(node: &Node, key: &str) -> Result<(f64, f64), PcbError> {
    super::numeric::point(SECTION, node, key)
}

fn has_flag(node: &Node, key: &str) -> bool {
    has_atom(node, key) || node.child(key).is_some()
}

fn has_atom(node: &Node, key: &str) -> bool {
    node.values
        .iter()
        .any(|value| matches!(value, Value::Atom(text) if text == key))
}

fn atom_str(value: &Value) -> &str {
    match value {
        Value::Atom(text) => text.as_str(),
        Value::Node(_) => "",
    }
}

pub fn read_traces(
    root: &Node,
    layers: &Layers,
    nets: &NetTable,
) -> Result<Vec<TraceJson>, PcbError> {
    let mut traces = Vec::new();
    for (id, node) in root.children("segment").enumerate() {
        if has_flag(node, "locked") {
            return Err(PcbError::new(
                SECTION,
                "Locked tracks are not supported yet.",
            ));
        }
        let net_name = nets.name_of(node)?;
        if net_name.is_empty() {
            return Err(PcbError::new(
                SECTION,
                "Tracks without a net are not supported yet.",
            ));
        }
        let width = number(node.value("width").unwrap_or(""))?;
        let layer_index = layers.index_of(node.value("layer").unwrap_or(""))?;
        let start = point(node, "start")?;
        let end = point(node, "end")?;
        traces.push(TraceJson {
            id: id as i32,
            netName: Some(net_name),
            width,
            layerIndex: layer_index,
            points: Some(vec![
                Point2D {
                    x: start.0,
                    y: start.1,
                },
                Point2D { x: end.0, y: end.1 },
            ]),
        });
    }
    Ok(traces)
}

pub fn read_vias(root: &Node, layers: &Layers, nets: &NetTable) -> Result<Vec<ViaJson>, PcbError> {
    let mut vias = Vec::new();
    for (id, node) in root.children("via").enumerate() {
        if has_flag(node, "locked") || has_atom(node, "blind") || has_atom(node, "micro") {
            return Err(PcbError::new(
                SECTION,
                "Locked, blind, and micro vias are not supported yet.",
            ));
        }
        let raw_span: &[Value] = node
            .child("layers")
            .map(|layers| &layers.values[1..])
            .unwrap_or(&[]);
        if raw_span.len() != 2 {
            return Err(PcbError::new(SECTION, "Missing via layer span."));
        }
        let start_name = atom_str(&raw_span[0]);
        let end_name = atom_str(&raw_span[1]);
        let net_name = nets.name_of(node)?;
        let first_layer = layers
            .entries
            .first()
            .and_then(|layer| layer.name.as_deref())
            .unwrap_or("");
        let last_layer = layers
            .entries
            .last()
            .and_then(|layer| layer.name.as_deref())
            .unwrap_or("");
        if net_name.is_empty() || start_name != first_layer || end_name != last_layer {
            return Err(PcbError::new(
                SECTION,
                "Only through vias assigned to a net are supported.",
            ));
        }
        let position = point(node, "at")?;
        let diameter = number(node.value("size").unwrap_or(""))?;
        let drill = number(node.value("drill").unwrap_or(""))?;
        vias.push(ViaJson {
            id: id as i32,
            netName: Some(net_name),
            position: Some(Point2D {
                x: position.0,
                y: position.1,
            }),
            diameter,
            drill,
            startLayerIndex: layers.index_of(start_name)?,
            endLayerIndex: layers.index_of(end_name)?,
        });
    }
    Ok(vias)
}

pub fn check_zones(
    root: &Node,
    nets: &NetTable,
    warnings: &mut Vec<String>,
) -> Result<(), PcbError> {
    let mut copper_zones = 0u32;
    let mut preserved_keepouts = 0u32;
    for zone in root.children("zone") {
        if let Some(keepout) = zone.child("keepout") {
            if keepout.value("tracks") != Some("allowed")
                || keepout.value("vias") != Some("allowed")
            {
                return Err(PcbError::new(
                    SECTION,
                    "Zone keepouts that restrict tracks or vias are not supported for routing yet.",
                ));
            }
            preserved_keepouts += 1;
            continue;
        }
        if nets.name_of(zone)?.is_empty() {
            return Err(PcbError::new(
                SECTION,
                "Netless copper zones are not supported for routing yet.",
            ));
        }
        copper_zones += 1;
    }
    if copper_zones > 0 {
        warnings.push(format!(
            "{copper_zones} copper zones will be preserved with their fill cache removed. \
             Routing uses tracks only; refill zones in KiCad (B), then run DRC."
        ));
    }
    if preserved_keepouts > 0 {
        warnings.push(format!(
            "{preserved_keepouts} keepout areas allow tracks and vias and are preserved for \
             KiCad zone refill."
        ));
    }
    Ok(())
}

pub fn read_copper_text(
    root: &Node,
    layers: &Layers,
    warnings: &mut Vec<String>,
) -> Result<Vec<ConductionAreaJson>, PcbError> {
    let mut areas = Vec::new();
    for value in &root.values {
        let Value::Node(node) = value else {
            continue;
        };
        let layer = node.value("layer").unwrap_or("");
        if !layers.is_copper(layer) || node.name() != "gr_text" {
            continue;
        }
        let effects = node.child("effects");
        let font = effects.and_then(|effects| effects.child("font"));
        let font = match font {
            Some(font) if font.child("face").is_none() => font,
            _ => {
                return Err(PcbError::new(
                    SECTION,
                    "Custom copper text fonts are not supported yet.",
                ));
            }
        };
        let size = point(font, "size")?;
        let at = point(node, "at")?;
        let angle_text = node.child("at").and_then(|at| at.atom(3)).unwrap_or("0");
        let angle = -number(angle_text)? * PI / 180.0;
        let text = node.atom(1).unwrap_or("");
        let lines: Vec<&str> = text.split('\n').collect();
        let longest = lines
            .iter()
            .map(|line| line.encode_utf16().count())
            .max()
            .unwrap_or(0) as f64;
        let width = longest * size.0 * 1.5 + size.1;
        let height = lines.len() as f64 * size.1 * 2.0;
        let justify: Vec<&str> = effects
            .and_then(|effects| effects.child("justify"))
            .map(|justify| {
                justify
                    .values
                    .iter()
                    .filter_map(|value| match value {
                        Value::Atom(text) => Some(text.as_str()),
                        Value::Node(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let left = if justify.contains(&"left") {
            -size.1 / 2.0
        } else if justify.contains(&"right") {
            -width
        } else {
            -width / 2.0
        };
        let top = if justify.contains(&"top") {
            -size.1 / 2.0
        } else if justify.contains(&"bottom") {
            -height
        } else {
            -height / 2.0
        };
        let mirror = if justify.contains(&"mirror") {
            -1.0
        } else {
            1.0
        };
        let corners = [
            (left, top),
            (left + width, top),
            (left + width, top + height),
            (left, top + height),
        ];
        let polygon = corners
            .into_iter()
            .map(|(x, y)| Point2D {
                x: at.0 + mirror * x * angle.cos() - y * angle.sin(),
                y: at.1 + mirror * x * angle.sin() + y * angle.cos(),
            })
            .collect();
        areas.push(ConductionAreaJson {
            id: 0,
            netName: Some(String::new()),
            layerIndex: layers.index_of(layer)?,
            isObstacle: true,
            polygon: Some(polygon),
        });
        warnings.push(
            "Copper text is preserved and reserved as conservative rectangular routing \
             obstacles; check text clearances in KiCad."
                .to_string(),
        );
    }
    Ok(areas)
}
