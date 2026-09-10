pub mod footprints;
mod numeric;
pub mod outline;
pub mod routing;
pub mod structure;

use crate::kicad::sexpr::{self, Node, Value};
use crate::kicad::{KiCadBoardJson, NetClassJson, OutlineJson, Point2D, UnitJson};
use structure::{Layers, NetTable};

const SECTION: &str = "pcb";

const TOP_LEVEL_COPPER_ALLOWED: &[&str] = &["segment", "footprint", "module", "zone", "arc"];

pub fn default_net_class() -> NetClassJson {
    NetClassJson {
        viaInPadAllowed: None,
        name: Some("Default".to_string()),
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        netNames: Some(Vec::new()),
    }
}

#[derive(Debug)]
pub struct ImportedPcb {
    pub board: KiCadBoardJson,
    pub warnings: Vec<String>,
}

pub fn read_pcb(text: &str, name: &str, defaults: &NetClassJson) -> Result<ImportedPcb, PcbError> {
    let root = sexpr::parse(text).map_err(|error| PcbError::new(SECTION, &error.to_string()))?;
    if root.name() != "kicad_pcb" {
        return Err(PcbError::new(SECTION, "Expected one kicad_pcb board."));
    }
    validate_defaults(&root, defaults)?;

    let mut warnings = Vec::new();

    let layers = Layers::read(&root)?;
    let nets = NetTable::read(&root)?;
    routing::check_zones(&root, &nets, &mut warnings)?;

    let paths = outline::outline_paths(&root)?;
    reject_unsupported_copper_objects(&root, &layers)?;
    let mut conduction_areas = routing::read_copper_text(&root, &layers, &mut warnings)?;
    if paths.curved {
        warnings.push(
            "Curved board edges are approximated within 0.005 mm for routing; the original \
             outline is preserved in downloads."
                .to_string(),
        );
    }
    let outline = outline::assemble_outline(&paths, &mut warnings)?;
    if !outline.cutouts.is_empty() {
        warnings.push(format!(
            "{} internal cutouts are reserved on every copper layer.",
            outline.cutouts.len()
        ));
    }

    let (components, footprint_areas) =
        footprints::read_components(&root, &layers, &nets, &mut warnings)?;
    conduction_areas.extend(footprint_areas);
    if components.is_empty() {
        return Err(PcbError::new(SECTION, "No pads were found."));
    }

    let traces = routing::read_traces(&root, &layers, &nets)?;
    let vias = routing::read_vias(&root, &layers, &nets)?;

    if root.children("arc").next().is_some() {
        return Err(PcbError::new(
            SECTION,
            "Curved tracks are not supported yet.",
        ));
    }

    let embedded_classes = root.children("net_class").count();
    let classes = structure::read_net_classes(&root, defaults)?;
    let (board_nets, net_classes) = nets.finish(classes);
    if embedded_classes > 0 {
        warnings.push(format!(
            "Imported {embedded_classes} embedded KiCad net classes, including trace widths, \
             clearances and via dimensions."
        ));
    }

    let clearance = net_classes
        .first()
        .map(|class| class.clearance)
        .unwrap_or(0.0)
        + if paths.curved {
            outline::OUTLINE_TOLERANCE
        } else {
            0.0
        };

    let board = KiCadBoardJson {
        designName: Some(name.to_string()),
        unit: Some(UnitJson::MM),
        resolution: 10000.0,
        layers: Some(layers.entries),
        nets: Some(board_nets),
        netClasses: Some(net_classes),
        components: Some(components),
        outline: Some(OutlineJson {
            ordered: true,
            corners: Some(
                outline
                    .boundary
                    .into_iter()
                    .map(|(x, y)| Point2D { x, y })
                    .collect(),
            ),
            cutouts: Some(
                outline
                    .cutouts
                    .into_iter()
                    .map(|points| points.into_iter().map(|(x, y)| Point2D { x, y }).collect())
                    .collect(),
            ),
            clearance,
        }),
        traces: Some(traces),
        vias: Some(vias),
        conductionAreas: Some(conduction_areas),
        ..Default::default()
    };

    let mut seen = std::collections::HashSet::new();
    warnings.retain(|warning| seen.insert(warning.clone()));

    Ok(ImportedPcb { board, warnings })
}

fn validate_defaults(root: &Node, defaults: &NetClassJson) -> Result<(), PcbError> {
    if root
        .children("net_class")
        .any(|node| node.atom(1) == Some("Default"))
    {
        return Ok(());
    }
    let in_range = |value: f64| value > 0.0 && value <= 5.0;
    if !in_range(defaults.traceWidth)
        || !in_range(defaults.clearance)
        || !in_range(defaults.viaDiameter)
        || !in_range(defaults.viaDrill)
    {
        return Err(PcbError::new(SECTION, "Invalid routing rules."));
    }
    if defaults.viaDrill >= defaults.viaDiameter {
        return Err(PcbError::new(
            SECTION,
            "Via drill must be smaller than diameter.",
        ));
    }
    Ok(())
}

fn reject_unsupported_copper_objects(root: &Node, layers: &Layers) -> Result<(), PcbError> {
    for value in &root.values {
        let Value::Node(node) = value else {
            continue;
        };
        let layer = node.value("layer").unwrap_or("");
        if !layers.is_copper(layer) || node.name() == "gr_text" {
            continue;
        }
        if !TOP_LEVEL_COPPER_ALLOWED.contains(&node.name()) {
            return Err(PcbError::new(
                SECTION,
                &format!("Unsupported copper object: {}", node.name()),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcbError {
    pub section: String,
    pub message: String,
}

impl PcbError {
    pub fn new(section: &str, message: &str) -> PcbError {
        PcbError {
            section: section.to_string(),
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for PcbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PcbError {}
