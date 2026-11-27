pub mod footprints;
mod numeric;
pub mod outline;
pub mod routing;
pub mod structure;

use crate::kicad::sexpr::{self, Node};
use crate::kicad::{
    ConductionAreaJson, KiCadBoardJson, NetClassJson, OutlineJson, Point2D, UnitJson,
};
use structure::{Layers, NetTable};

const SECTION: &str = "pcb";

fn number(text: &str) -> Result<f64, PcbError> {
    numeric::number(SECTION, text)
}

/// What to do with routing already on a KiCad board before autorouting it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportOptions {
    /// Remove unlocked tracks, arcs and through vias so they are routed again.
    pub rip_up: bool,
    /// Limits rip-up to these nets; every net when absent.
    pub rip_up_nets: Option<std::collections::BTreeSet<String>>,
    /// Treat existing zone fills as copper: their own net connects through them and every
    /// other net routes around them.
    pub respect_zones: bool,
    /// Also autoroute differential pairs and length-tuned nets, which are skipped by default.
    pub route_tuned_nets: bool,
}

/// Applies `options` to a freshly read board.
pub fn apply_import_options(imported: &mut ImportedPcb, options: &ImportOptions) {
    rip_up(&mut imported.board, options);
    if !options.route_tuned_nets && !imported.tuned_nets.is_empty() {
        imported.skipped_nets.clone_from(&imported.tuned_nets);
        imported.warnings.push(format!(
            "{} differential pair and length-tuned nets are left for KiCad's own tools: {}.",
            imported.tuned_nets.len(),
            imported
                .tuned_nets
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if options.respect_zones {
        let fills = std::mem::take(&mut imported.zone_fills);
        let usable: Vec<ConductionAreaJson> = fills
            .into_iter()
            .filter(|fill| routing::fill_is_usable(fill, imported.board.resolution))
            .collect();
        if !usable.is_empty() {
            imported.warnings.push(format!(
                "{} zone fills are routed around as copper; refill zones in KiCad after routing.",
                usable.len()
            ));
        }
        imported
            .board
            .conductionAreas
            .get_or_insert_with(Vec::new)
            .extend(usable);
    }
}

/// Locked items, blind or buried vias, and nets outside the selection are always kept.
pub fn rip_up(board: &mut crate::kicad::KiCadBoardJson, options: &ImportOptions) {
    if !options.rip_up {
        return;
    }
    let ripped = |locked: bool, net: Option<&str>| {
        !locked
            && options
                .rip_up_nets
                .as_ref()
                .is_none_or(|nets| nets.contains(net.unwrap_or_default()))
    };
    if let Some(traces) = board.traces.as_mut() {
        traces.retain(|trace| !ripped(trace.locked, trace.netName.as_deref()));
        for (id, trace) in traces.iter_mut().enumerate() {
            trace.id = id as i32;
        }
    }
    if let Some(vias) = board.vias.as_mut() {
        vias.retain(|via| !ripped(via.locked, via.netName.as_deref()));
    }
}

/// Moves each named net into a copy of its net class that the autorouter ignores.
pub fn ignore_nets(board: &mut copper_board::Board, names: &std::collections::BTreeSet<String>) {
    let numbers: Vec<i32> = board
        .rules
        .nets
        .iter()
        .filter(|net| names.contains(&net.name))
        .map(|net| net.net_number)
        .collect();
    for number in numbers {
        let Some(class) = board
            .rules
            .nets
            .get(number)
            .map(copper_board::Net::get_net_class)
        else {
            continue;
        };
        let ignored = board.rules.net_classes.ignored_copy_of(class);
        if let Some(net) = board.rules.nets.get_mut(number) {
            net.set_class(ignored);
        }
    }
}

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
    pub embedded_net_classes: usize,
    /// Filled copper of every zone, used only with [`ImportOptions::respect_zones`].
    pub zone_fills: Vec<ConductionAreaJson>,
    /// Differential pair and length-tuned nets.
    pub tuned_nets: std::collections::BTreeSet<String>,
    /// Nets the autorouter must leave alone; see [`ignore_nets`].
    pub skipped_nets: std::collections::BTreeSet<String>,
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
    let mut conduction_areas = routing::read_copper_graphics(&root, &layers, &mut warnings)?;
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
    let keepouts = routing::read_keepouts(&root, &layers)?;
    let zone_fills = routing::read_zone_fills(&root, &layers, &nets)?;
    let mut tuned_nets = routing::tuned_nets(&root, &nets)?;

    if traces.iter().any(|trace| trace.arc) {
        warnings.push(
            "Curved tracks are kept as fixed copper, approximated within 0.005 mm for clearance."
                .to_string(),
        );
    }

    let embedded_classes = root.children("net_class").count();
    let classes = structure::read_net_classes(&root, defaults)?;
    let (board_nets, net_classes) = nets.finish(classes);
    tuned_nets.extend(routing::differential_pair_nets(
        board_nets.iter().filter_map(|net| net.name.as_deref()),
    ));
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

    let setup = root.child("setup");
    let allow_solder_mask_bridges_in_footprints = setup
        .map(|setup| {
            setup
                .value("allow_soldermask_bridges_in_footprints")
                .unwrap_or("no")
                == "yes"
        })
        .unwrap_or(false);
    let solder_mask_min_width = match setup {
        Some(setup) if setup.child("solder_mask_min_width").is_some() => {
            Some(number(setup.value("solder_mask_min_width").unwrap_or("0"))?)
        }
        _ => None,
    };

    let board = KiCadBoardJson {
        designName: Some(name.to_string()),
        unit: Some(UnitJson::MM),
        resolution: 10000.0,
        allowSolderMaskBridgesInFootprints: allow_solder_mask_bridges_in_footprints,
        traceEndsAnywhereInPads: true,
        solderMaskMinWidth: solder_mask_min_width,
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
        keepouts,
        ..Default::default()
    };

    let mut seen = std::collections::HashSet::new();
    warnings.retain(|warning| seen.insert(warning.clone()));

    Ok(ImportedPcb {
        board,
        warnings,
        embedded_net_classes: embedded_classes,
        zone_fills,
        tuned_nets,
        skipped_nets: std::collections::BTreeSet::new(),
    })
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
