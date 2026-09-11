use std::collections::BTreeMap;
use std::f64::consts::PI;

use super::PcbError;
use super::outline;
use super::structure::{Layers, NetTable};
use crate::kicad::sexpr::{Node, Value};
use crate::kicad::{ComponentJson, ConductionAreaJson, PadJson, Point2D};

const SECTION: &str = "components";

fn number(text: &str) -> Result<f64, PcbError> {
    super::numeric::number(SECTION, text)
}

fn xy(node: Option<&Node>) -> Result<(f64, f64), PcbError> {
    super::numeric::xy(SECTION, node)
}

fn point(node: &Node, key: &str) -> Result<(f64, f64), PcbError> {
    super::numeric::point(SECTION, node, key)
}

fn numeric_value(node: &Node, key: &str, fallback: f64) -> Result<f64, PcbError> {
    match node.value(key) {
        Some(text) => number(text),
        None => Ok(fallback),
    }
}

fn child_nodes(node: &Node) -> impl Iterator<Item = &Node> {
    node.values.iter().filter_map(|value| match value {
        Value::Node(child) => Some(child),
        Value::Atom(_) => None,
    })
}

fn cross(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn convex_hull_half(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for &p in points {
        while out.len() > 1 && cross(out[out.len() - 2], out[out.len() - 1], p) <= 0.0 {
            out.pop();
        }
        out.push(p);
    }
    out.pop();
    out
}

fn footprint_reference(fp: &Node, index: usize) -> String {
    fp.children("property")
        .find(|node| node.atom(1) == Some("Reference"))
        .and_then(|node| node.atom(2))
        .or_else(|| {
            fp.children("fp_text")
                .find(|node| node.atom(1) == Some("reference"))
                .and_then(|node| node.atom(2))
        })
        .map(str::to_string)
        .unwrap_or_else(|| format!("FP{index}"))
}

fn copper_rectangle(
    fp: &Node,
    node: &Node,
    layer: &str,
    layers: &Layers,
) -> Result<ConductionAreaJson, PcbError> {
    let a = point(node, "start")?;
    let b = point(node, "end")?;
    let width_source = node.child("stroke").unwrap_or(node);
    let margin = numeric_value(width_source, "width", 0.0)? / 2.0;
    let x0 = a.0.min(b.0) - margin;
    let x1 = a.0.max(b.0) + margin;
    let y0 = a.1.min(b.1) - margin;
    let y1 = a.1.max(b.1) + margin;
    let polygon = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        .into_iter()
        .map(|(x, y)| outline::footprint_point(fp, x, y))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(x, y)| Point2D { x, y })
        .collect();
    Ok(ConductionAreaJson {
        id: 0,
        netName: Some(String::new()),
        layerIndex: layers.index_of(layer)?,
        isObstacle: true,
        polygon: Some(polygon),
    })
}

fn custom_pad_polygon(
    pad: &Node,
    size: (f64, f64),
    warnings: &mut Vec<String>,
) -> Result<Vec<Point2D>, PcbError> {
    let primitives = pad
        .child("primitives")
        .map(|node| &node.values[1..])
        .unwrap_or(&[]);
    let primitive = match primitives {
        [Value::Node(node)] if node.name() == "gr_poly" && node.value("fill") == Some("yes") => {
            node
        }
        _ => {
            return Err(PcbError::new(
                SECTION,
                "Custom pads require one filled convex polygon.",
            ));
        }
    };
    let poly: Vec<(f64, f64)> = match primitive.child("pts") {
        Some(pts) => pts
            .children("xy")
            .map(|node| xy(Some(node)))
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    let raw_signs: Vec<i32> = poly
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let b = poly[(i + 1) % poly.len()];
            let c = poly[(i + 2) % poly.len()];
            let value = cross(p, b, c);
            if value > 0.0 {
                1
            } else if value < 0.0 {
                -1
            } else {
                0
            }
        })
        .collect();
    let signs: Vec<i32> = raw_signs.into_iter().filter(|&s| s != 0).collect();
    if poly.len() < 3 || (!signs.is_empty() && signs.iter().any(|&s| s != signs[0])) {
        return Err(PcbError::new(
            SECTION,
            "Concave custom pads are not supported yet.",
        ));
    }
    let radius = numeric_value(primitive, "width", 0.0)? / 2.0;
    let anchor = primitive_anchor(pad);
    if anchor.as_deref() != Some("circle") || size.0 != size.1 || radius < 0.0 {
        return Err(PcbError::new(
            SECTION,
            "Custom polygon pads require a circular anchor and nonnegative stroke.",
        ));
    }
    let sign0 = f64::from(*signs.first().unwrap_or(&0));
    let uncovered = poly.iter().enumerate().any(|(i, &a)| {
        let b = poly[(i + 1) % poly.len()];
        let span = (b.0 - a.0).hypot(b.1 - a.1);
        sign0 * cross(a, b, (0.0, 0.0)) / span + radius < size.0 / 2.0
    });
    if uncovered {
        return Err(PcbError::new(
            SECTION,
            "Custom pad polygon must cover its circular anchor.",
        ));
    }
    let count: usize = if radius != 0.0 {
        let raw = (PI / (1.0 / (1.0 + outline::OUTLINE_TOLERANCE / radius)).acos()).ceil();
        (raw as usize).max(24)
    } else {
        1
    };
    let mut samples: Vec<(f64, f64)> = poly
        .iter()
        .flat_map(|&p| {
            (0..count).map(move |i| {
                let r = if radius != 0.0 {
                    radius / (PI / count as f64).cos()
                } else {
                    0.0
                };
                let angle = i as f64 * 2.0 * PI / count as f64;
                (p.0 + r * angle.cos(), p.1 + r * angle.sin())
            })
        })
        .collect();
    samples.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
    let mut reversed = samples.clone();
    reversed.reverse();
    let mut hull = convex_hull_half(&samples);
    hull.extend(convex_hull_half(&reversed));
    warnings.push(
        "Convex custom pad outlines include their stroke, approximated within 0.005 mm; \
         original pad definitions are preserved."
            .to_string(),
    );
    Ok(hull.into_iter().map(|(x, y)| Point2D { x, y }).collect())
}

fn primitive_anchor(pad: &Node) -> Option<String> {
    pad.child("options")
        .and_then(|options| options.value("anchor"))
        .map(str::to_string)
}

const MASK_LAYERS: [&str; 2] = ["F.Mask", "B.Mask"];

fn has_atom(values: &[Value], atom: &str) -> bool {
    values
        .iter()
        .any(|value| matches!(value, Value::Atom(name) if name == atom))
}

fn solder_mask_expansion(raw_layers: &[Value], margin: f64) -> BTreeMap<String, f64> {
    MASK_LAYERS
        .into_iter()
        .filter(|&layer| has_atom(raw_layers, layer) || has_atom(raw_layers, "*.Mask"))
        .map(|layer| (layer.to_string(), margin))
        .collect()
}

fn effective_solder_mask_expansion(margin: f64) -> BTreeMap<String, f64> {
    MASK_LAYERS
        .into_iter()
        .map(|layer| (layer.to_string(), margin))
        .collect()
}

fn allows_solder_mask_bridges(fp: &Node) -> bool {
    fp.child("attr")
        .is_some_and(|attr| has_atom(&attr.values, "allow_soldermask_bridges"))
}

const CLEARANCE_ZERO_OVERRIDE_VERSION: f64 = 20_240_201.0;

fn local_clearance(node: &Node, version: f64) -> Result<Option<f64>, PcbError> {
    let Some(field) = node.child("clearance") else {
        return Ok(None);
    };
    let value = number(field.atom(1).unwrap_or(""))?;
    if value == 0.0 && version <= CLEARANCE_ZERO_OVERRIDE_VERSION {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

pub fn read_components(
    root: &Node,
    layers: &Layers,
    nets: &NetTable,
    warnings: &mut Vec<String>,
) -> Result<(Vec<ComponentJson>, Vec<ConductionAreaJson>), PcbError> {
    let mut components = Vec::new();
    let mut conduction_areas = Vec::new();

    let board_mask_margin = match root.child("setup") {
        Some(setup) => numeric_value(setup, "pad_to_mask_clearance", 0.0)?,
        None => 0.0,
    };
    let version = root.number("version").unwrap_or(0.0);

    for (fi, fp) in root
        .children("footprint")
        .chain(root.children("module"))
        .enumerate()
    {
        if fp.children("net_tie_pad_groups").next().is_some() {
            return Err(PcbError::new(SECTION, "Net ties are not supported yet."));
        }
        if fp.children("zone").next().is_some() {
            return Err(PcbError::new(
                SECTION,
                "Footprint zones are not supported yet.",
            ));
        }

        for node in child_nodes(fp) {
            let layer = node.value("layer").unwrap_or("");
            if layers.could_be_copper(layer) && node.name() == "fp_rect" {
                conduction_areas.push(copper_rectangle(fp, node, layer, layers)?);
                warnings.push(
                    "Footprint copper rectangles are reserved as solid routing obstacles and \
                     preserved in downloads."
                        .to_string(),
                );
                continue;
            }
            if layers.could_be_copper(layer) && node.name() != "pad" && node.name() != "layer" {
                return Err(PcbError::new(
                    SECTION,
                    "Footprint copper graphics are not supported yet.",
                ));
            }
        }

        let origin = point(fp, "at")?;
        let rotation = match fp.child("at").and_then(|at| at.atom(3)) {
            Some(text) => number(text)?,
            None => 0.0,
        } * PI
            / 180.0;
        let reference = footprint_reference(fp, fi);
        let allow_solder_mask_bridges = allows_solder_mask_bridges(fp);

        for (pi, pad) in fp.children("pad").enumerate() {
            let pad_type = pad.atom(2);
            let shape = pad.atom(3);
            let type_ok = matches!(
                pad_type,
                Some("smd" | "connect" | "thru_hole" | "np_thru_hole")
            );
            let shape_ok = matches!(
                shape,
                Some("circle" | "rect" | "oval" | "roundrect" | "custom")
            );
            if !type_ok || !shape_ok {
                return Err(PcbError::new(
                    SECTION,
                    &format!(
                        "Unsupported pad {reference}.{}: {}/{}",
                        pad.atom(1).unwrap_or("undefined"),
                        pad_type.unwrap_or("undefined"),
                        shape.unwrap_or("undefined"),
                    ),
                ));
            }
            let shape = shape.expect("validated above");
            let pad_type = pad_type.expect("validated above");

            let mut copper_polygon = None;
            if shape == "custom" {
                let size = point(pad, "size")?;
                copper_polygon = Some(custom_pad_polygon(pad, size, warnings)?);
            } else if pad.child("primitives").is_some() {
                return Err(PcbError::new(SECTION, "Unexpected custom pad primitives."));
            }

            let drill_node = pad.child("drill");
            let slotted = drill_node.and_then(|node| node.atom(1)) == Some("oval");
            let drill = if slotted {
                let node = drill_node.expect("slotted implies a drill node");
                let width = number(node.atom(2).unwrap_or(""))?;
                let height = number(node.atom(3).unwrap_or(""))?;
                if pad_type != "thru_hole" || width <= 0.0 || height <= 0.0 {
                    return Err(PcbError::new(
                        SECTION,
                        "Only plated slots with positive dimensions are supported.",
                    ));
                }
                warnings.push(
                    "Plated slots retain their copper pad geometry; slot-specific drill checks \
                     require KiCad DRC. Original slots are preserved in downloads."
                        .to_string(),
                );
                width.min(height)
            } else {
                numeric_value(pad, "drill", 0.0)?
            };

            let local = point(pad, "at")?;
            let size = point(pad, "size")?;
            if shape == "circle" && size.0 != size.1 {
                return Err(PcbError::new(
                    SECTION,
                    "Circular pads must have equal dimensions.",
                ));
            }
            if size.0 <= 0.0 || size.1 <= 0.0 {
                return Err(PcbError::new(SECTION, "Pad sizes must be positive."));
            }

            let position = (
                origin.0 + local.0 * rotation.cos() + local.1 * rotation.sin(),
                origin.1 - local.0 * rotation.sin() + local.1 * rotation.cos(),
            );
            let angle = match pad.child("at").and_then(|at| at.atom(3)) {
                Some(text) => number(text)?,
                None => 0.0,
            };

            let raw_layers: &[Value] = pad
                .child("layers")
                .map(|node| &node.values[1..])
                .unwrap_or(&[]);
            let mut pad_layers: Vec<String> = Vec::new();
            let mut unknown_copper_layer: Option<&str> = None;
            for value in raw_layers {
                let Value::Atom(name) = value else {
                    continue;
                };
                if name == "*.Cu" || name == "F&B.Cu" {
                    pad_layers.extend(
                        layers
                            .entries
                            .iter()
                            .map(|entry| entry.name.clone().unwrap_or_default()),
                    );
                } else if layers.is_copper(name) {
                    pad_layers.push(name.clone());
                } else if name.ends_with(".Cu") {
                    unknown_copper_layer.get_or_insert(name);
                }
            }
            if pad_layers.is_empty() {
                if let Some(name) = unknown_copper_layer {
                    return Err(PcbError::new(
                        SECTION,
                        &format!("Unknown copper layer: {name}"),
                    ));
                }
                continue;
            }

            let round_rect_ratio = if shape == "roundrect" {
                Some(numeric_value(pad, "roundrect_rratio", 0.25)?)
            } else {
                None
            };
            let shape_offset = match drill_node.and_then(|node| node.child("offset")) {
                Some(offset) => {
                    let (x, y) = xy(Some(offset))?;
                    Some(Point2D { x, y })
                }
                None => None,
            };
            let mask_margin = numeric_value(
                pad,
                "solder_mask_margin",
                numeric_value(fp, "solder_mask_margin", board_mask_margin)?,
            )?;
            let copper_clearance = match local_clearance(pad, version)? {
                Some(value) => Some(value),
                None => local_clearance(fp, version)?,
            };

            let pad_json = PadJson {
                sourceFootprint: Some(fi.to_string()),
                sourcePadNumber: Some(pad.atom(1).unwrap_or("undefined").to_string()),
                copperClearance: copper_clearance,
                allowSolderMaskBridges: allow_solder_mask_bridges,
                solderMaskExpansion: Some(solder_mask_expansion(raw_layers, mask_margin)),
                effectiveSolderMaskExpansion: Some(effective_solder_mask_expansion(mask_margin)),
                name: Some(pi.to_string()),
                netName: Some(nets.name_of(pad)?),
                shape: Some(shape.to_string()),
                roundRectRatio: round_rect_ratio,
                size: Some(Point2D {
                    x: size.0,
                    y: size.1,
                }),
                copperPolygon: copper_polygon,
                shapeOffset: shape_offset,
                offset: Some(Point2D { x: 0.0, y: 0.0 }),
                position: Some(Point2D {
                    x: position.0,
                    y: position.1,
                }),
                drill,
                nonPlated: pad_type == "np_thru_hole",
                drillEstimated: slotted,
                layers: Some(pad_layers.into_iter().map(Some).collect()),
            };

            components.push(ComponentJson {
                reference: Some(format!("{reference}:{fi}:{pi}")),
                value: None,
                footprint: None,
                position: Some(Point2D {
                    x: position.0,
                    y: position.1,
                }),
                rotation: -angle,
                layer: Some("F.Cu".to_string()),
                pads: Some(vec![pad_json]),
            });

            if shape == "roundrect" {
                warnings.push(
                    "Rounded pads retain their corner radius for hole DRC; routing uses \
                     enclosing rectangles."
                        .to_string(),
                );
            }
        }
    }

    Ok((components, conduction_areas))
}
