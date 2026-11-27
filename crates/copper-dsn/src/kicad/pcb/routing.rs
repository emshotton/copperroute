use std::collections::BTreeSet;
use std::f64::consts::PI;

use super::PcbError;
use super::outline;
use super::structure::{Layers, NetTable};
use crate::kicad::sexpr::{Node, Value};
use crate::kicad::{ConductionAreaJson, KeepoutJson, Point2D, TraceJson, ViaJson};

const SECTION: &str = "routing";

fn number(text: &str) -> Result<f64, PcbError> {
    super::numeric::number(SECTION, text)
}

fn point(node: &Node, key: &str) -> Result<(f64, f64), PcbError> {
    super::numeric::point(SECTION, node, key)
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

/// KiCad writes `(locked yes)` from version 7 and a bare `locked` atom before it.
pub(crate) fn is_locked(node: &Node) -> bool {
    has_atom(node, "locked") || node.value("locked").is_some_and(|value| value != "no")
}

pub fn source_id(node: &Node) -> Option<String> {
    node.value("uuid")
        .or_else(|| node.value("tstamp"))
        .map(|id| id.trim_matches('"').to_string())
}

pub fn read_traces(
    root: &Node,
    layers: &Layers,
    nets: &NetTable,
) -> Result<Vec<TraceJson>, PcbError> {
    let mut traces = Vec::new();
    for node in root.children("segment") {
        let width = number(node.value("width").unwrap_or(""))?;
        let layer_index = layers.index_of(node.value("layer").unwrap_or(""))?;
        let start = point(node, "start")?;
        let end = point(node, "end")?;
        traces.push(TraceJson {
            id: traces.len() as i32,
            netName: Some(nets.name_of(node)?),
            width,
            layerIndex: layer_index,
            points: Some(vec![
                Point2D {
                    x: start.0,
                    y: start.1,
                },
                Point2D { x: end.0, y: end.1 },
            ]),
            locked: is_locked(node),
            arc: false,
            sourceId: source_id(node),
        });
    }
    for node in root.children("arc") {
        let width = number(node.value("width").unwrap_or(""))?;
        let layer_index = layers.index_of(node.value("layer").unwrap_or(""))?;
        let points = outline::native_arc_points(node)?;
        traces.push(TraceJson {
            id: traces.len() as i32,
            netName: Some(nets.name_of(node)?),
            width: width + 2.0 * outline::OUTLINE_TOLERANCE,
            layerIndex: layer_index,
            points: Some(points.into_iter().map(|(x, y)| Point2D { x, y }).collect()),
            locked: is_locked(node),
            arc: true,
            sourceId: source_id(node),
        });
    }
    Ok(traces)
}

pub fn read_vias(root: &Node, layers: &Layers, nets: &NetTable) -> Result<Vec<ViaJson>, PcbError> {
    let mut vias = Vec::new();
    for (id, node) in root.children("via").enumerate() {
        let raw_span: &[Value] = node
            .child("layers")
            .map(|layers| &layers.values[1..])
            .unwrap_or(&[]);
        if raw_span.len() != 2 {
            return Err(PcbError::new(SECTION, "Missing via layer span."));
        }
        let start_layer = layers.index_of(atom_str(&raw_span[0]))?;
        let end_layer = layers.index_of(atom_str(&raw_span[1]))?;
        let (start_layer, end_layer) = (start_layer.min(end_layer), start_layer.max(end_layer));
        let through = start_layer == 0 && end_layer as usize + 1 == layers.entries.len();
        let position = point(node, "at")?;
        let diameter = number(node.value("size").unwrap_or(""))?;
        let drill = number(node.value("drill").unwrap_or(""))?;
        vias.push(ViaJson {
            id: id as i32,
            netName: Some(nets.name_of(node)?),
            position: Some(Point2D {
                x: position.0,
                y: position.1,
            }),
            diameter,
            drill,
            startLayerIndex: start_layer,
            endLayerIndex: end_layer,
            locked: is_locked(node) || !through,
            sourceId: source_id(node),
        });
    }
    Ok(vias)
}

pub(crate) fn zone_layers(zone: &Node, layers: &Layers) -> Result<Vec<i32>, PcbError> {
    let names: Vec<&str> = match zone.child("layers") {
        Some(node) => node.values[1..].iter().map(atom_str).collect(),
        None => zone.value("layer").into_iter().collect(),
    };
    let mut result = Vec::new();
    for name in names {
        if name == "*.Cu" || name == "F&B.Cu" {
            let mut all = layers.entries.iter().map(|layer| layer.index);
            if name == "*.Cu" {
                result.extend(all);
            } else {
                result.extend(all.next());
                result.extend(all.next_back());
            }
        } else if layers.could_be_copper(name) {
            result.push(layers.index_of(name)?);
        }
    }
    result.sort_unstable();
    result.dedup();
    Ok(result)
}

fn zone_outline(zone: &Node) -> Result<Vec<Point2D>, PcbError> {
    let polygon = zone
        .child("polygon")
        .ok_or_else(|| PcbError::new(SECTION, "Zone without an outline."))?;
    Ok(poly_points(polygon)?
        .into_iter()
        .map(|(x, y)| Point2D { x, y })
        .collect())
}

pub fn read_keepouts(root: &Node, layers: &Layers) -> Result<Vec<KeepoutJson>, PcbError> {
    let mut result = Vec::new();
    for zone in root.children("zone") {
        let Some(keepout) = zone.child("keepout") else {
            continue;
        };
        let no_tracks = keepout.value("tracks") == Some("not_allowed");
        let no_vias = keepout.value("vias") == Some("not_allowed");
        if !no_tracks && !no_vias {
            continue;
        }
        let polygon = zone_outline(zone)?;
        for layer_index in zone_layers(zone, layers)? {
            result.push(KeepoutJson {
                layerIndex: layer_index,
                noTracks: no_tracks,
                noVias: no_vias,
                polygon: polygon.clone(),
            });
        }
    }
    Ok(result)
}

pub fn check_zones(
    root: &Node,
    nets: &NetTable,
    warnings: &mut Vec<String>,
) -> Result<(), PcbError> {
    let mut copper_zones = 0u32;
    let mut preserved_keepouts = 0u32;
    let mut routing_keepouts = 0u32;
    for zone in root.children("zone") {
        if let Some(keepout) = zone.child("keepout") {
            if keepout.value("tracks") == Some("not_allowed")
                || keepout.value("vias") == Some("not_allowed")
            {
                routing_keepouts += 1;
            } else {
                preserved_keepouts += 1;
            }
            continue;
        }
        if is_teardrop(zone) {
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
    if routing_keepouts > 0 {
        warnings.push(format!(
            "{routing_keepouts} keepout areas restrict routing. A keepout that forbids tracks \
             also keeps vias out."
        ));
    }
    Ok(())
}

pub fn read_zone_fills(
    root: &Node,
    layers: &Layers,
    nets: &NetTable,
) -> Result<Vec<ConductionAreaJson>, PcbError> {
    let mut fills = Vec::new();
    for zone in root.children("zone") {
        if zone.child("keepout").is_some() || is_teardrop(zone) {
            continue;
        }
        let net_name = nets.name_of(zone)?;
        for fill in zone.children("filled_polygon") {
            let layer = fill.value("layer").unwrap_or("");
            if !layers.is_copper(layer) {
                continue;
            }
            let polygon = poly_points(fill)?
                .into_iter()
                .map(|(x, y)| Point2D { x, y })
                .collect();
            fills.push(ConductionAreaJson {
                id: fills.len() as i32,
                netName: Some(net_name.clone()),
                sourceFootprint: None,
                layerIndex: layers.index_of(layer)?,
                isObstacle: true,
                polygon: Some(polygon),
            });
        }
    }
    Ok(fills)
}

/// KiCad stores fills with holes as one outline joined to each hole by a zero-width cut. The
/// router needs that outline split into convex pieces, which fails on some cuts.
pub(crate) fn fill_is_usable(fill: &ConductionAreaJson, resolution: f64) -> bool {
    let points: Vec<copper_geometry::Point> = fill
        .polygon
        .iter()
        .flatten()
        .map(|corner| {
            copper_geometry::Point::Int(copper_geometry::IntPoint::new(
                (corner.x * resolution).round() as i32,
                (-corner.y * resolution).round() as i32,
            ))
        })
        .collect();
    points.len() >= 3
        && copper_geometry::PolygonShape::from_points(&points)
            .split_to_convex()
            .is_some()
}

/// KiCad pairs nets whose names differ only in a final `+`/`-` or `P`/`N`.
pub fn differential_pair_nets<'a>(names: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    let names: BTreeSet<&str> = names.into_iter().collect();
    let mut result = BTreeSet::new();
    for name in &names {
        for (positive, negative) in [('+', '-'), ('P', 'N')] {
            if let Some(base) = name.strip_suffix(positive) {
                let partner = format!("{base}{negative}");
                if !base.is_empty() && names.contains(partner.as_str()) {
                    result.insert((*name).to_string());
                    result.insert(partner);
                }
            }
        }
    }
    result
}

/// Nets whose tracks belong to a KiCad length-tuning pattern.
pub fn tuned_nets(root: &Node, nets: &NetTable) -> Result<BTreeSet<String>, PcbError> {
    let members: BTreeSet<String> = root
        .children("generated")
        .filter(|node| node.value("type") == Some("tuning_pattern"))
        .filter_map(|node| node.child("members"))
        .flat_map(|members| members.values[1..].iter().map(atom_str))
        .map(|id| id.trim_matches('"').to_string())
        .collect();
    let mut result = BTreeSet::new();
    for node in root.children("segment").chain(root.children("arc")) {
        if source_id(node).is_some_and(|id| members.contains(&id)) {
            let name = nets.name_of(node)?;
            if !name.is_empty() {
                result.insert(name);
            }
        }
    }
    Ok(result)
}

/// KiCad regenerates teardrops itself, so their zones are left out of routing.
pub(crate) fn is_teardrop(zone: &Node) -> bool {
    zone.child("attr")
        .is_some_and(|attr| attr.child("teardrop").is_some())
}

// A `generated` node is a length-tuning recipe whose `members` are segments and arcs that
// already appear in the file, so it carries no copper of its own.
const TOP_LEVEL_COPPER_ALLOWED: &[&str] =
    &["segment", "footprint", "module", "zone", "arc", "generated"];

pub(crate) fn text_rectangle(
    node: &Node,
    text: &str,
    at: (f64, f64),
    angle: f64,
) -> Result<Vec<Point2D>, PcbError> {
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
    Ok(corners
        .into_iter()
        .map(|(x, y)| Point2D {
            x: at.0 + mirror * x * angle.cos() - y * angle.sin(),
            y: at.1 + mirror * x * angle.sin() + y * angle.cos(),
        })
        .collect())
}

fn text_angle(node: &Node) -> Result<f64, PcbError> {
    let angle_text = node.child("at").and_then(|at| at.atom(3)).unwrap_or("0");
    Ok(-number(angle_text)? * PI / 180.0)
}

fn poly_points(node: &Node) -> Result<Vec<(f64, f64)>, PcbError> {
    let points: Vec<(f64, f64)> = match node.child("pts") {
        Some(pts) => pts
            .children("xy")
            .map(|xy_node| super::numeric::xy(SECTION, Some(xy_node)))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    if points.is_empty() {
        return Err(PcbError::new(SECTION, "Empty board polygon."));
    }
    Ok(points)
}

fn obstacle_noun(kind: &str) -> &'static str {
    match kind {
        "gr_line" => "lines",
        "gr_rect" => "rectangles",
        "gr_circle" => "circles",
        "gr_arc" => "arcs",
        "gr_poly" => "polygons",
        _ => unreachable!("obstacle_noun is only called for the kinds handled above"),
    }
}

fn bbox_area(
    points: &[(f64, f64)],
    margin: f64,
    layer: &str,
    layers: &Layers,
) -> Result<ConductionAreaJson, PcbError> {
    let polygon = outline::bounding_box(points, margin)
        .into_iter()
        .map(|(x, y)| Point2D { x, y })
        .collect();
    Ok(ConductionAreaJson {
        id: 0,
        netName: Some(String::new()),
        sourceFootprint: None,
        layerIndex: layers.index_of(layer)?,
        isObstacle: true,
        polygon: Some(polygon),
    })
}

pub fn read_copper_graphics(
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
        if !layers.could_be_copper(layer) {
            continue;
        }
        match node.name() {
            "gr_text" => {
                let at = point(node, "at")?;
                let angle = text_angle(node)?;
                let text = node.atom(1).unwrap_or("");
                let polygon = text_rectangle(node, text, at, angle)?;
                areas.push(ConductionAreaJson {
                    id: 0,
                    netName: Some(String::new()),
                    sourceFootprint: None,
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
            kind @ ("gr_line" | "gr_rect") => {
                let a = point(node, "start")?;
                let b = point(node, "end")?;
                let margin = super::numeric::stroke_margin(SECTION, node)?;
                areas.push(bbox_area(&[a, b], margin, layer, layers)?);
                warnings.push(format!(
                    "Copper {} are reserved as solid routing obstacles and preserved in \
                     downloads.",
                    obstacle_noun(kind)
                ));
            }
            "gr_circle" => {
                let center = point(node, "center")?;
                let end = point(node, "end")?;
                let radius = (end.0 - center.0).hypot(end.1 - center.1);
                let margin = super::numeric::stroke_margin(SECTION, node)?;
                let pseudo = [
                    (center.0 - radius, center.1 - radius),
                    (center.0 + radius, center.1 + radius),
                ];
                areas.push(bbox_area(&pseudo, margin, layer, layers)?);
                warnings.push(
                    "Copper circles are reserved as solid routing obstacles and preserved in \
                     downloads."
                        .to_string(),
                );
            }
            "gr_arc" => {
                let points = outline::native_arc_points(node)?;
                let margin = super::numeric::stroke_margin(SECTION, node)?;
                areas.push(bbox_area(&points, margin, layer, layers)?);
                warnings.push(
                    "Copper arcs are reserved as solid routing obstacles and preserved in \
                     downloads."
                        .to_string(),
                );
            }
            "gr_poly" => {
                let points = poly_points(node)?;
                let margin = super::numeric::stroke_margin(SECTION, node)?;
                areas.push(bbox_area(&points, margin, layer, layers)?);
                warnings.push(
                    "Copper polygons are reserved as solid routing obstacles and preserved in \
                     downloads."
                        .to_string(),
                );
            }
            other if !TOP_LEVEL_COPPER_ALLOWED.contains(&other) => {
                return Err(PcbError::new(
                    SECTION,
                    &format!("Unsupported copper object: {other}"),
                ));
            }
            _ => {}
        }
    }
    Ok(areas)
}
