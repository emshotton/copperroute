use fr_board::{
    AngleRestriction, Board, BoardLibrary, BoardRules, ClearanceMatrix, Communication, Components,
    FixedState, Item, ItemClass, ItemCtx, ItemId, Layer, LayerStructure, Packages, Padstacks,
    ViaInfoId, equals_ignore_case,
};
use fr_geometry::{Area, IntBox, PolylineShapeRef, Shape, TileShape};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::IndentFileWriter;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::DsnRouterSettings;
use crate::parser::autoroute_settings::{
    read_autoroute_settings_scope, write_autoroute_settings_scope,
};
use crate::parser::dsn_file::read_string_scope;
use crate::parser::geometry::{
    self as shape, DsnPolygonPath, DsnRectangle, DsnShape, ReadAreaScopeResult,
};
use crate::parser::header::read_flip_style_rotate_first;
use crate::parser::network::{
    DsnClearanceRule, DsnRule, NetId, clearance_class_name, read_rule_scope, write_default_rule,
    write_item_clearance_class,
};
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};

pub use crate::parser::geometry::{DsnLayer, DsnLayerStructure};

const CRIT_INT: f64 = fr_geometry::CRIT_INT as f64;

#[derive(Debug, Clone, PartialEq)]
pub struct DsnPlane {
    pub area: ReadAreaScopeResult,
    pub net_name: String,
}

impl DsnPlane {
    #[must_use]
    pub fn new(area: ReadAreaScopeResult, net_name: impl Into<String>) -> DsnPlane {
        DsnPlane {
            area,
            net_name: net_name.into(),
        }
    }
}

pub fn read_plane_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let skip_window_scopes = p
        .host_cad
        .as_deref()
        .is_some_and(|host| equals_ignore_case("allegro", host));

    let Some(Token::Str(net_name)) = p.scanner.next_token()? else {
        return Ok(false);
    };
    p.scanner.set_scope_identifier(&net_name);
    let conduction_area = shape::read_area_scope(
        &mut p.scanner,
        p.layer_structure.as_ref(),
        skip_window_scopes,
    )?;
    if let Some(conduction_area) = conduction_area {
        p.plane_list.push(DsnPlane::new(conduction_area, net_name));
    }
    Ok(true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeepoutType {
    Keepout,
    ViaKeepout,
    PlaceKeepout,
}

#[derive(Debug, Default)]
struct BoardConstructionInfo {
    layer_info: Vec<DsnLayer>,
    bounding_shape: Option<DsnShape>,
    outline_shapes: Vec<DsnShape>,
    outline_clearance_class_name: Option<String>,
    found_layer_count: i32,
    default_rules: Vec<DsnRule>,
    layer_dependent_rules: Vec<StructureLayerRule>,
}

#[derive(Debug)]
struct StructureLayerRule {
    layer_name: String,
    rule: Vec<DsnRule>,
}

#[allow(clippy::too_many_lines)]
pub fn read_structure_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut info = BoardConstructionInfo::default();

    let mut flip_style_rotate_first = false;

    let mut keepout_list: Vec<Option<ReadAreaScopeResult>> = Vec::new();
    let mut via_keepout_list: Vec<Option<ReadAreaScopeResult>> = Vec::new();
    let mut place_keepout_list: Vec<Option<ReadAreaScopeResult>> = Vec::new();

    let mut prev_was_open = false;
    loop {
        let Some(next_token) = p.scanner.next_token()? else {
            return Ok(false);
        };
        if next_token == Token::Close {
            break;
        }
        let is_open = next_token == Token::Open;
        let mut read_ok = true;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Boundary) => {
                    let _ = read_boundary_scope(&mut p.scanner, &mut info)?;
                }
                Token::Kw(Keyword::Layer) => {
                    read_ok = read_layer_scope(&mut p.scanner, &mut info, &p.string_quote)?;
                    if p.layer_structure.is_some() {
                        p.layer_structure = Some(DsnLayerStructure::new(info.layer_info.clone()));
                    }
                }
                Token::Kw(Keyword::Via) => {
                    p.via_padstack_names = read_via_padstacks(&mut p.scanner)?;
                }
                Token::Kw(Keyword::Rule) => {
                    if let Some(rules) = read_rule_scope(&mut p.scanner)? {
                        info.default_rules.extend(rules);
                    }
                }
                Token::Kw(Keyword::Keepout) => {
                    ensure_layer_structure(p, &info);
                    keepout_list.push(shape::read_area_scope(
                        &mut p.scanner,
                        p.layer_structure.as_ref(),
                        false,
                    )?);
                }
                Token::Kw(Keyword::ViaKeepout) => {
                    ensure_layer_structure(p, &info);
                    via_keepout_list.push(shape::read_area_scope(
                        &mut p.scanner,
                        p.layer_structure.as_ref(),
                        false,
                    )?);
                }
                Token::Kw(Keyword::PlaceKeepout) => {
                    ensure_layer_structure(p, &info);
                    place_keepout_list.push(shape::read_area_scope(
                        &mut p.scanner,
                        p.layer_structure.as_ref(),
                        false,
                    )?);
                }
                Token::Kw(Keyword::PlaneScope) => {
                    ensure_layer_structure(p, &info);
                    let _ = read_plane_scope(p)?;
                }
                Token::Kw(Keyword::AutorouteSettings) => {
                    ensure_layer_structure(p, &info);
                    let layer_structure = p
                        .layer_structure
                        .clone()
                        .expect("ensure_layer_structure assigned it");
                    p.autoroute_settings =
                        read_autoroute_settings_scope(&mut p.scanner, &layer_structure)?;
                }
                Token::Kw(Keyword::Control) => {
                    read_ok = read_control_scope(p)?;
                }
                Token::Kw(Keyword::FlipStyle) => {
                    flip_style_rotate_first = read_flip_style_rotate_first(&mut p.scanner)?;
                }
                Token::Kw(Keyword::SnapAngle) => {
                    if let Some(snap_angle) = read_snap_angle(&mut p.scanner)? {
                        p.snap_angle = snap_angle;
                    }
                }
                _ => {
                    let _ = skip_scope(&mut p.scanner)?;
                }
            }
        }
        if !read_ok {
            return Ok(false);
        }
        prev_was_open = is_open;
    }

    let mut result = true;
    if p.board.is_none() {
        result = create_board(p, &mut info)?;
    }
    if p.board.is_none() {
        return Ok(false);
    }
    if flip_style_rotate_first {
        p.board
            .as_mut()
            .expect("checked just above")
            .set_flip_style_rotate_first(true);
    }

    for current_area in &keepout_list {
        if !insert_keepout(
            current_area.as_ref(),
            p,
            KeepoutType::Keepout,
            FixedState::SystemFixed,
        )? {
            return Ok(false);
        }
    }
    for current_area in &via_keepout_list {
        if !insert_keepout(
            current_area.as_ref(),
            p,
            KeepoutType::ViaKeepout,
            FixedState::SystemFixed,
        )? {
            return Ok(false);
        }
    }
    for current_area in &place_keepout_list {
        if !insert_keepout(
            current_area.as_ref(),
            p,
            KeepoutType::PlaceKeepout,
            FixedState::SystemFixed,
        )? {
            return Ok(false);
        }
    }

    if !insert_planes(p)? {
        return Ok(false);
    }

    insert_missing_power_planes(&info.layer_info, p);

    Ok(result)
}

fn ensure_layer_structure(p: &mut ReadScopeParameter<'_>, info: &BoardConstructionInfo) {
    if p.layer_structure.is_none() {
        p.layer_structure = Some(DsnLayerStructure::new(info.layer_info.clone()));
    }
}

fn read_boundary_scope(
    scanner: &mut DsnScanner,
    info: &mut BoardConstructionInfo,
) -> Result<bool, DsnError> {
    let current_shape = shape::read_scope(scanner, None)?;
    let mut prev_was_open = false;
    loop {
        let next_token = scanner.next_token()?;
        if next_token == Some(Token::Close) {
            break;
        }
        let Some(token) = next_token else {
            return Ok(true);
        };
        let is_open = token == Token::Open;
        if prev_was_open {
            if token == Token::Kw(Keyword::ClearanceClass) {
                info.outline_clearance_class_name = Some(read_string_scope(scanner)?);
            } else {
                let additional_shape = shape::read_scope_from_keyword(scanner, Some(token), None)?;
                add_boundary_shape(info, additional_shape);
            }
        }
        prev_was_open = is_open;
    }
    add_boundary_shape(info, current_shape);
    Ok(true)
}

fn add_boundary_shape(info: &mut BoardConstructionInfo, shape: Option<DsnShape>) {
    let Some(shape) = shape else {
        return;
    };
    if matches!(shape, DsnShape::PolylinePath(_) | DsnShape::Path(_)) {
        info.outline_shapes.push(shape);
        return;
    }
    if *shape.layer() == DsnLayer::pcb() {
        if info.bounding_shape.is_none() {
            info.bounding_shape = Some(shape);
        } else {
            info.outline_shapes.push(shape);
        }
    } else if *shape.layer() == DsnLayer::signal() {
        info.outline_shapes.push(shape);
    }
}

fn read_layer_scope(
    scanner: &mut DsnScanner,
    info: &mut BoardConstructionInfo,
    _string_quote: &str,
) -> Result<bool, DsnError> {
    let mut layer_ok = true;
    let mut is_signal = true;

    let layer_string = scanner.next_string();
    let mut net_names: Vec<String> = Vec::new();

    let mut next_token = scanner.next_token()?;
    while next_token != Some(Token::Close) {
        if next_token != Some(Token::Open) {
            return Ok(false);
        }
        next_token = scanner.next_token()?;
        match next_token {
            Some(Token::Kw(Keyword::Type)) => {
                next_token = scanner.next_token()?;
                if next_token == Some(Token::Kw(Keyword::Power)) {
                    is_signal = false;
                } else if next_token != Some(Token::Kw(Keyword::Signal))
                    && !matches!(&next_token, Some(Token::Str(s)) if s == Keyword::Jumper.name())
                {
                    layer_ok = false;
                }
                if scanner.next_token()? != Some(Token::Close) {
                    return Ok(false);
                }
            }
            Some(Token::Kw(Keyword::Rule)) => {
                let current_rules = read_rule_scope(scanner)?.unwrap_or_default();
                info.layer_dependent_rules.push(StructureLayerRule {
                    layer_name: layer_string.clone(),
                    rule: current_rules,
                });
            }
            Some(Token::Kw(Keyword::UseNet)) => loop {
                scanner.yybegin(LexicalState::Name);
                next_token = scanner.next_token()?;
                match next_token {
                    Some(Token::Close) => break,
                    Some(Token::Str(ref s)) => net_names.push(s.clone()),
                    None => return Ok(false),
                    Some(_) => {}
                }
            },
            _ => {
                let _ = skip_scope(scanner)?;
            }
        }
        next_token = scanner.next_token()?;
    }
    if layer_ok {
        let current_layer =
            DsnLayer::with_nets(layer_string, info.found_layer_count, is_signal, net_names);
        info.layer_info.push(current_layer);
        info.found_layer_count += 1;
    }
    Ok(true)
}

pub(crate) fn read_via_padstacks(
    scanner: &mut DsnScanner,
) -> Result<Option<Vec<String>>, DsnError> {
    let mut normal_vias: Vec<String> = Vec::new();
    let mut spare_vias: Vec<String> = Vec::new();
    loop {
        let next_token = scanner.next_token()?;
        match next_token {
            Some(Token::Close) => break,
            Some(Token::Open) => {
                if scanner.next_token()? == Some(Token::Kw(Keyword::Spare)) {
                    spare_vias = read_via_padstacks(scanner)?.unwrap_or_default();
                } else {
                    let _ = skip_scope(scanner)?;
                }
            }
            Some(Token::Str(s)) => normal_vias.push(s),
            _ => return Ok(None),
        }
    }
    normal_vias.extend(spare_vias);
    Ok(Some(normal_vias))
}

fn read_control_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = p.scanner.next_token()? else {
            return Ok(false);
        };
        if next_token == Token::Close {
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            if next_token == Token::Kw(Keyword::ViaAtSmd) {
                p.via_at_smd_allowed = crate::parser::dsn_file::read_on_off_scope(&mut p.scanner)?;
            } else {
                let _ = skip_scope(&mut p.scanner)?;
            }
        }
        prev_was_open = is_open;
    }
    Ok(true)
}

pub fn read_snap_angle(
    scanner: &mut DsnScanner,
) -> Result<Option<fr_board::AngleRestriction>, DsnError> {
    use fr_board::AngleRestriction;
    let snap_angle = match scanner.next_token()? {
        Some(Token::Kw(Keyword::NinetyDegree)) => AngleRestriction::NinetyDegree,
        Some(Token::Kw(Keyword::FortyfiveDegree)) => AngleRestriction::FortyFiveDegree,
        Some(Token::Kw(Keyword::None)) => AngleRestriction::None,
        _ => return Ok(None),
    };
    if scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    Ok(Some(snap_angle))
}

fn insert_keepout(
    area: Option<&ReadAreaScopeResult>,
    p: &mut ReadScopeParameter<'_>,
    keepout_type: KeepoutType,
    fixed_state: FixedState,
) -> Result<bool, DsnError> {
    let Some(area) = area else {
        return Ok(true);
    };
    let Some(coordinate_transform) = p.coordinate_transform else {
        return Ok(true);
    };
    let Some(keepout_area) =
        shape::transform_area_to_board(&area.shape_list, &coordinate_transform)
    else {
        return Ok(true);
    };
    if keepout_area.dimension() < 2 {
        return Ok(true);
    }
    let Some(current_layer) = area
        .shape_list
        .first()
        .and_then(Option::as_ref)
        .map(DsnShape::layer)
    else {
        return Ok(true);
    };
    let current_layer = current_layer.clone();
    let clearance_class_name = area.clearance_class_name.clone();

    let layer_structure = p.layer_structure.clone().unwrap_or_default();
    let Some(board) = p.board.as_mut() else {
        return Ok(false);
    };
    if current_layer == DsnLayer::signal() {
        for i in 0..board.get_layer_count() {
            if layer_structure.layers.get(i).is_some_and(|l| l.is_signal) {
                insert_keepout_on_layer(
                    board,
                    keepout_area.clone(),
                    i,
                    clearance_class_name.as_deref(),
                    keepout_type,
                    fixed_state,
                );
            }
        }
    } else if current_layer.no >= 0 {
        insert_keepout_on_layer(
            board,
            keepout_area,
            current_layer.no as usize,
            clearance_class_name.as_deref(),
            keepout_type,
            fixed_state,
        );
    } else {
        return Ok(false);
    }
    Ok(true)
}

fn insert_keepout_on_layer(
    board: &mut Board,
    area: Area,
    layer: usize,
    clearance_class_name: Option<&str>,
    keepout_type: KeepoutType,
    fixed_state: FixedState,
) {
    let clearance_class_index = match clearance_class_name {
        None => {
            let default_class = board.rules.get_default_net_class();
            board
                .rules
                .net_classes
                .get(default_class)
                .default_item_clearance_classes
                .get(ItemClass::Area)
        }
        Some(name) => board
            .rules
            .clearance_matrix
            .get_no(name)
            .unwrap_or_else(BoardRules::clearance_class_none),
    };
    match keepout_type {
        KeepoutType::ViaKeepout => {
            board.insert_via_obstacle(area, layer, clearance_class_index, fixed_state);
        }
        KeepoutType::PlaceKeepout => {
            board.insert_component_obstacle(area, layer, clearance_class_index, fixed_state);
        }
        KeepoutType::Keepout => {
            board.insert_obstacle(area, layer, clearance_class_index, fixed_state);
        }
    }
}

fn insert_planes(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let plane_list = std::mem::take(&mut p.plane_list);
    let result = insert_planes_inner(p, &plane_list);
    p.plane_list = plane_list;
    result
}

fn insert_planes_inner(
    p: &mut ReadScopeParameter<'_>,
    plane_list: &[DsnPlane],
) -> Result<bool, DsnError> {
    let Some(coordinate_transform) = p.coordinate_transform else {
        return Ok(true);
    };
    for plane_info in plane_list {
        let net_id = NetId {
            name: plane_info.net_name.clone(),
            subnet_no: 1,
        };
        if !p.netlist.contains(&net_id) {
            p.netlist.add_net(net_id.clone());
            let Some(board) = p.board.as_mut() else {
                return Ok(false);
            };
            let default_class = board.rules.get_default_net_class();
            board
                .rules
                .nets
                .add(net_id.name.clone(), net_id.subnet_no, true, default_class);
        }
        let Some(board) = p.board.as_mut() else {
            return Ok(false);
        };
        let Some(current_net) = board
            .rules
            .nets
            .get_by_name_and_subnet(&plane_info.net_name, 1)
        else {
            continue;
        };
        let net_number = current_net.net_number;
        let net_class = current_net.get_net_class();

        let Some(plane_area) =
            shape::transform_area_to_board(&plane_info.area.shape_list, &coordinate_transform)
        else {
            continue;
        };
        let Some(current_layer) = plane_info
            .area
            .shape_list
            .first()
            .and_then(Option::as_ref)
            .map(DsnShape::layer)
        else {
            continue;
        };
        if current_layer.no < 0 {
            return Ok(false);
        }
        let layer_no = current_layer.no as usize;
        let clearance_class_index = match &plane_info.area.clearance_class_name {
            Some(name) => board
                .rules
                .clearance_matrix
                .get_no(name)
                .unwrap_or_else(BoardRules::clearance_class_none),
            None => board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes
                .get(ItemClass::Area),
        };
        board.insert_conduction_area(
            plane_area,
            layer_no,
            vec![net_number],
            clearance_class_index,
            false,
            FixedState::SystemFixed,
        );
    }
    Ok(true)
}

fn insert_missing_power_planes(layer_info: &[DsnLayer], p: &mut ReadScopeParameter<'_>) {
    let Some(board) = p.board.as_mut() else {
        return;
    };
    let conduction_area_layers: Vec<i32> = board
        .get_conduction_areas()
        .into_iter()
        .filter_map(|id| match board.get_item(id) {
            Some(fr_board::Item::ConductionArea(area)) => i32::try_from(area.area.get_layer()).ok(),
            _ => None,
        })
        .collect();

    for current_layer in layer_info {
        if current_layer.is_signal {
            continue;
        }
        let conduction_area_found = conduction_area_layers.contains(&current_layer.no);
        if conduction_area_found || current_layer.net_names.is_empty() {
            continue;
        }
        let current_net_name = current_layer.net_names[0].clone();
        let current_net_id = NetId {
            name: current_net_name.clone(),
            subnet_no: 1,
        };
        if !p.netlist.contains(&current_net_id) {
            p.netlist.add_net(current_net_id.clone());
            let Some(board) = p.board.as_mut() else {
                return;
            };
            let default_class = board.rules.get_default_net_class();
            board.rules.nets.add(
                current_net_id.name.clone(),
                current_net_id.subnet_no,
                true,
                default_class,
            );
        }
        let Some(board) = p.board.as_mut() else {
            return;
        };
        let Some(current_net) = board
            .rules
            .nets
            .get_by_name_and_subnet(&current_net_id.name, current_net_id.subnet_no)
        else {
            continue;
        };
        let net_number = current_net.net_number;
        if current_layer.no < 0 {
            continue;
        }
        let bounding_box = board.bounding_box;
        board.insert_conduction_area(
            Area::Shape(Shape::Tile(TileShape::Box(bounding_box))),
            current_layer.no as usize,
            vec![net_number],
            BoardRules::clearance_class_none(),
            false,
            FixedState::SystemFixed,
        );
    }
}

fn update_board_rules(
    p: &ReadScopeParameter<'_>,
    info: &BoardConstructionInfo,
    board_rules: &mut BoardRules,
) {
    let Some(coordinate_transform) = p.coordinate_transform else {
        return;
    };
    let mut smd_to_turn_gap_found = false;
    for current_object in &info.default_rules {
        if let DsnRule::Clearance(current_rule) = current_object
            && set_clearance_rule(
                current_rule,
                RuleLayerScope::AllLayers,
                &coordinate_transform,
                board_rules,
                &p.string_quote,
            )
        {
            smd_to_turn_gap_found = true;
        }
    }
    for current_object in &info.default_rules {
        if let DsnRule::Width(wire_width) = current_object {
            let trace_halfwidth =
                (coordinate_transform.dsn_to_board(*wire_width) / 2.0).round() as i32;
            board_rules.set_default_trace_half_widths(trace_halfwidth);
        }
    }
    let layer_structure = p.layer_structure.clone().unwrap_or_default();
    for layer_rule in &info.layer_dependent_rules {
        let Some(layer_index) = layer_structure.get_no(&layer_rule.layer_name) else {
            continue;
        };
        for current_object in &layer_rule.rule {
            match current_object {
                DsnRule::Width(wire_width) => {
                    let trace_halfwidth =
                        (coordinate_transform.dsn_to_board(*wire_width) / 2.0).round() as i32;
                    board_rules.set_default_trace_half_width(layer_index, trace_halfwidth);
                }
                DsnRule::Clearance(current_rule) => {
                    set_clearance_rule(
                        current_rule,
                        RuleLayerScope::One(layer_index),
                        &coordinate_transform,
                        board_rules,
                        &p.string_quote,
                    );
                }
            }
        }
    }
    if !smd_to_turn_gap_found {
        board_rules.set_pin_edge_to_turn_dist(f64::from(board_rules.get_min_trace_half_width()));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleLayerScope {
    AllLayers,
    One(usize),
}

pub fn set_clearance_rule(
    rule: &DsnClearanceRule,
    scope: RuleLayerScope,
    coordinate_transform: &CoordinateTransform,
    board_rules: &mut BoardRules,
    string_quote: &str,
) -> bool {
    let mut result = false;
    let current_clearance = (coordinate_transform.dsn_to_board(rule.value)).round() as i32;
    if rule.clearance_class_pairs.is_empty() {
        match scope {
            RuleLayerScope::AllLayers => board_rules
                .clearance_matrix
                .set_default_value(current_clearance),
            RuleLayerScope::One(layer) => board_rules
                .clearance_matrix
                .set_default_value_on_layer(layer, current_clearance),
        }
        return result;
    }
    if contains_wire_clearance_pair(&rule.clearance_class_pairs) {
        create_default_clearance_classes(board_rules);
    }

    for current_string in &rule.clearance_class_pairs {
        if equals_ignore_case("smd_to_turn_gap", current_string) {
            board_rules.set_pin_edge_to_turn_dist(f64::from(current_clearance));
            result = true;
            continue;
        }
        let current_pair: [String; 2] = if rule.clearance_class_pairs.len() == 2 {
            let mut pair = [
                rule.clearance_class_pairs[0].clone(),
                rule.clearance_class_pairs[1].clone(),
            ];
            for i in 0..2 {
                pair[i] = pair[i].replace('"', "");
                if let Some(stripped) = pair[1].strip_prefix('_') {
                    pair[1] = stripped.to_string();
                }
            }
            pair
        } else if let Some(rest) = current_string.strip_prefix(string_quote) {
            let mut parts = rest.splitn(2, string_quote);
            let first = parts.next().unwrap_or_default().to_string();
            let Some(second) = parts.next() else {
                continue;
            };
            let Some(second) = second.strip_prefix('_') else {
                continue;
            };
            [first, second.to_string()]
        } else {
            let mut parts = current_string.splitn(2, '_');
            let first = parts.next().unwrap_or_default().to_string();
            let Some(second) = parts.next() else {
                continue;
            };
            [first, second.to_string()]
        };

        let mut first_class_no = if current_pair[0] == "wire" {
            Some(1)
        } else {
            board_rules.clearance_matrix.get_no(&current_pair[0])
        };
        if first_class_no.is_none() {
            first_class_no = Some(append_clearance_class(board_rules, &current_pair[0]));
        }
        let mut second_class_no = if current_pair[1] == "wire" {
            Some(1)
        } else {
            board_rules.clearance_matrix.get_no(&current_pair[1])
        };
        if second_class_no.is_none() {
            second_class_no = Some(append_clearance_class(board_rules, &current_pair[1]));
        }
        let first_class_no = first_class_no.expect("assigned above");
        let second_class_no = second_class_no.expect("assigned above");

        match scope {
            RuleLayerScope::AllLayers => {
                board_rules.clearance_matrix.set_value_on_all_layers(
                    first_class_no,
                    second_class_no,
                    current_clearance,
                );
                board_rules.clearance_matrix.set_value_on_all_layers(
                    second_class_no,
                    first_class_no,
                    current_clearance,
                );
            }
            RuleLayerScope::One(layer) => {
                board_rules.clearance_matrix.set_value(
                    first_class_no,
                    second_class_no,
                    layer,
                    current_clearance,
                );
                board_rules.clearance_matrix.set_value(
                    second_class_no,
                    first_class_no,
                    layer,
                    current_clearance,
                );
            }
        }
    }
    result
}

pub(crate) fn contains_wire_clearance_pair(clearance_pairs: &[String]) -> bool {
    clearance_pairs
        .iter()
        .any(|p| p.starts_with("wire_") || p.ends_with("_wire"))
}

fn create_default_clearance_classes(board_rules: &mut BoardRules) {
    append_clearance_class(board_rules, "via");
    append_clearance_class(board_rules, "smd");
    append_clearance_class(board_rules, "pin");
    append_clearance_class(board_rules, "area");
}

fn append_clearance_class(board_rules: &mut BoardRules, name: &str) -> usize {
    board_rules.clearance_matrix.append_class(name);
    let result = board_rules
        .clearance_matrix
        .get_no(name)
        .expect("appendClass leaves the class present");
    let default_net_class = board_rules.get_default_net_class();
    let net_class = board_rules.net_classes.get_mut(default_net_class);
    match name {
        "via" => net_class
            .default_item_clearance_classes
            .set(ItemClass::Via, result),
        "pin" => net_class
            .default_item_clearance_classes
            .set(ItemClass::Pin, result),
        "smd" => net_class
            .default_item_clearance_classes
            .set(ItemClass::Smd, result),
        "area" => net_class
            .default_item_clearance_classes
            .set(ItemClass::Area, result),
        _ => {}
    }
    result
}

struct OutlineShape {
    shape: PolylineShapeRef,
    bounding_box: IntBox,
    convex_shapes: Option<Vec<TileShape>>,
    is_hole: bool,
}

impl OutlineShape {
    fn new(shape: PolylineShapeRef) -> OutlineShape {
        let bounding_box = shape.as_ops().bounding_box();
        let convex_shapes = shape.split_to_convex();
        OutlineShape {
            shape,
            bounding_box,
            convex_shapes,
            is_hole: false,
        }
    }

    fn contains_all_corners(&self, other_shape: &OutlineShape) -> bool {
        let Some(convex_shapes) = &self.convex_shapes else {
            return false;
        };
        let corner_count = other_shape.shape.as_ops().border_line_count();
        for i in 0..corner_count {
            let current_corner = other_shape.shape.as_ops().corner(i);
            if !convex_shapes.iter().any(|s| s.contains(&current_corner)) {
                return false;
            }
        }
        true
    }
}

fn separate_holes(outline_shapes: &mut Vec<PolylineShapeRef>) -> Vec<PolylineShapeRef> {
    let mut shapes: Vec<OutlineShape> = outline_shapes
        .iter()
        .cloned()
        .map(OutlineShape::new)
        .collect();
    for i in 0..shapes.len() {
        for j in 0..shapes.len() {
            if i == j || shapes[j].is_hole {
                continue;
            }
            if !shapes[j].bounding_box.contains(&shapes[i].bounding_box) {
                continue;
            }
            shapes[i].is_hole = shapes[j].contains_all_corners(&shapes[i]);
        }
    }
    let mut hole_list: Vec<PolylineShapeRef> = Vec::new();
    let mut kept: Vec<PolylineShapeRef> = Vec::new();
    for entry in shapes {
        if entry.is_hole {
            hole_list.push(entry.shape);
        } else {
            kept.push(entry.shape);
        }
    }
    *outline_shapes = kept;
    hole_list
}

#[allow(clippy::too_many_lines)]
fn create_board(
    p: &mut ReadScopeParameter<'_>,
    info: &mut BoardConstructionInfo,
) -> Result<bool, DsnError> {
    let layer_count = info.layer_info.len();
    if layer_count == 0 {
        return Ok(false);
    }
    if info.bounding_shape.is_none() {
        if info.outline_shapes.is_empty() {
            p.board_outline_ok = false;
            return Ok(false);
        }
        let mut boxes = info
            .outline_shapes
            .iter()
            .filter_map(DsnShape::bounding_box);
        let Some(mut bounding_box) = boxes.next() else {
            p.board_outline_ok = false;
            return Ok(false);
        };
        for other in boxes {
            bounding_box = bounding_box.union(&other);
        }
        info.bounding_shape = Some(DsnShape::Rect(bounding_box));
    }
    let bounding_shape = info
        .bounding_shape
        .as_ref()
        .expect("assigned just above when it was None");
    let Some(bounding_box) = bounding_shape.bounding_box() else {
        p.board_outline_ok = false;
        return Ok(false);
    };

    let mut board_layer_arr: Vec<Layer> = Vec::with_capacity(layer_count);
    for current_layer in &info.layer_info {
        if current_layer.no < 0 || current_layer.no as usize >= layer_count {
            return Ok(false);
        }
        board_layer_arr.push(Layer::new(
            current_layer.name.clone(),
            current_layer.is_signal,
        ));
    }
    let board_layer_structure = LayerStructure::new(board_layer_arr);
    p.layer_structure = Some(DsnLayerStructure::new(info.layer_info.clone()));

    let mut scale_factor = f64::from(p.resolution.max(1));

    let mut max_coor = 0.0_f64;
    for coordinate in bounding_box.coor {
        max_coor = max_coor.max((coordinate * f64::from(p.resolution)).abs());
    }
    if max_coor == 0.0 {
        p.board_outline_ok = false;
        return Ok(false);
    }
    while 5.0 * max_coor >= CRIT_INT {
        scale_factor /= 10.0;
        max_coor /= 10.0;
    }

    let coordinate_transform = CoordinateTransform::new(scale_factor, 0.0, 0.0)?;
    p.coordinate_transform = Some(coordinate_transform);

    let Shape::Tile(TileShape::Box(bounds)) =
        bounding_box.transform_to_board(&coordinate_transform)
    else {
        unreachable!("Rectangle.transformToBoard returns an IntBox");
    };
    let bounds = bounds.offset(1000.0);

    let mut board_outline_shapes: Vec<PolylineShapeRef> = Vec::new();
    for current_shape in &info.outline_shapes {
        let mut current_shape = current_shape.clone();
        if let DsnShape::Path(current_path) = &current_shape
            && current_path.width != 0.0
        {
            current_shape = DsnShape::Path(DsnPolygonPath::new(
                current_path.layer.clone(),
                0.0,
                current_path.coordinate_arr.clone(),
            ));
        }
        let Some(current_board_shape) = current_shape
            .transform_to_board(&coordinate_transform)
            .and_then(to_polyline_shape)
        else {
            continue;
        };
        if current_board_shape.as_ops().dimension() > 0 {
            board_outline_shapes.push(current_board_shape);
        }
    }
    if board_outline_shapes.is_empty() {
        let bounding_shape = info.bounding_shape.as_ref().expect("set above");
        if let Some(current_board_shape) = bounding_shape
            .transform_to_board(&coordinate_transform)
            .and_then(to_polyline_shape)
        {
            board_outline_shapes.push(current_board_shape);
        }
    }
    let hole_shapes = separate_holes(&mut board_outline_shapes);

    let clearance_matrix = ClearanceMatrix::get_default_instance(&board_layer_structure, 0);
    let mut board_rules = BoardRules::new(board_layer_structure.clone(), clearance_matrix);
    let board_communication = Communication {
        string_quote: p.string_quote.clone(),
        constants: p.constants.clone(),
        write_resolution: p.write_resolution.clone(),
        dsn_file_generated_by_host: p.dsn_file_generated_by_host,
        ..Communication::new(
            p.unit,
            p.resolution,
            p.id_generator,
            p.host_cad.clone(),
            p.host_version.clone(),
        )
    };

    update_board_rules(p, info, &mut board_rules);
    board_rules.trace_angle_restriction = p.snap_angle;

    let outline_clearance_no = match &info.outline_clearance_class_name {
        Some(name) => board_rules.clearance_matrix.get_no(name).unwrap_or(0),
        None => {
            let default_class = board_rules.get_default_net_class();
            board_rules
                .net_classes
                .get(default_class)
                .default_item_clearance_classes
                .get(ItemClass::Area)
        }
    };

    let mut library = BoardLibrary::new(
        Padstacks::new(board_layer_structure.clone()),
        Packages::new(),
    );
    library.set_via_padstacks(Vec::new());

    let board = Board::new(
        board_outline_shapes,
        outline_clearance_no,
        bounds,
        board_rules,
        library,
        Components::new(),
        board_communication,
    );
    p.board = Some(board);

    let board = p.board.as_mut().expect("just assigned");
    for current_outline_hole in &hole_shapes {
        for i in 0..board_layer_structure.layers.len() {
            board.insert_obstacle(
                Area::Shape(current_outline_hole.to_shape()),
                i,
                0,
                FixedState::SystemFixed,
            );
        }
    }

    Ok(true)
}

fn to_polyline_shape(shape: Shape) -> Option<PolylineShapeRef> {
    match shape {
        Shape::Tile(t) => Some(PolylineShapeRef::Tile(t)),
        Shape::Polygon(p) => Some(PolylineShapeRef::Polygon(p)),
        Shape::Circle(_) => None,
    }
}

pub fn write_structure_scope(
    p: &mut WriteScopeParameter<'_>,
    autoroute_settings: Option<&DsnRouterSettings>,
) {
    let board = p.board;
    p.file.start_scope_nl();
    p.file.write("structure");

    write_layers(p);

    write_boundaries(p);

    write_via_padstacks(p);

    write_default_rules(p);

    write_snap_angle(&mut p.file, p.board.rules.trace_angle_restriction);

    write_control_scope(p);

    if let Some(settings) = autoroute_settings {
        write_autoroute_settings_scope(
            &mut p.file,
            settings,
            board.layer_structure(),
            &p.identifier_type,
        );
    }

    write_conduction_areas(p);

    write_keepouts(p);

    p.file.end_scope();
}

fn write_conduction_areas(p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    for id in board.items_in_board_order() {
        let Some(Item::ConductionArea(area)) = board.items.get(&id) else {
            continue;
        };
        if board.layer_structure().layers[area.get_layer()].is_signal {
            continue;
        }
        write_plane_scope(p, id);
    }
}

fn write_keepouts(p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    for id in board.items_in_board_order() {
        let Some(item) = board.items.get(&id) else {
            continue;
        };
        if !item.is_obstacle_area() {
            continue;
        }
        if item.component_id() != 0 {
            continue;
        }
        if matches!(item, Item::ConductionArea(_)) {
            continue;
        }
        write_keepout_scope(p, id);
    }
}

fn write_boundaries(p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("boundary");
    let bounds = p.board.get_bounding_box();
    let rect_coor = p.coordinate_transform.board_to_dsn_box(&bounds);
    let bounding_rectangle = DsnRectangle::new(DsnLayer::pcb(), rect_coor);
    bounding_rectangle.write_scope(&mut p.file, &p.identifier_type);
    p.file.end_scope();

    let board = p.board;
    let Some(outline_id) = board.get_outline() else {
        return;
    };
    let Some(Item::BoardOutline(outline)) = board.items.get(&outline_id) else {
        return;
    };

    for i in 0..outline.shape_count() {
        let Some(shape) = outline.get_shape(i) else {
            continue;
        };
        let shape = shape.to_shape();
        let Some(outline_shape) = p
            .coordinate_transform
            .board_to_dsn_shape(&shape, DsnLayer::signal())
        else {
            continue;
        };
        p.file.start_scope_nl();
        p.file.write("boundary");
        outline_shape.write_scope(&mut p.file, &p.identifier_type);
        p.file.end_scope();
    }
}

pub fn write_layers(p: &mut WriteScopeParameter<'_>) {
    for i in 0..p.board.layer_structure().count() {
        let write_layer_rule = default_net_class_trace_half_width(&p.board.rules, i)
            != default_net_class_trace_half_width(&p.board.rules, 0)
            || !clearance_equals(&p.board.rules.clearance_matrix, i, 0);
        write_layer_scope(p, i, write_layer_rule);
    }
}

pub fn write_default_rules(p: &mut WriteScopeParameter<'_>) {
    write_default_rule(p, 0);
}

fn write_via_padstacks(p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    p.file.new_line();
    p.file.write("(via");
    for i in 0..board.library.via_padstack_count() {
        let Some(padstack) = board
            .library
            .get_via_padstack(i)
            .and_then(|id| board.library.padstacks.get(id))
        else {
            continue;
        };
        let name = padstack.name.clone();
        p.file.write(" ");
        p.identifier_type.write(&name, &mut p.file);
    }
    p.file.write(")");
}

fn write_control_scope(p: &mut WriteScopeParameter<'_>) {
    let rules = &p.board.rules;
    let mut via_at_smd_allowed = false;
    for i in 0..rules.via_infos.count() {
        if rules.via_infos.get(ViaInfoId(i)).attach_smd_allowed() {
            via_at_smd_allowed = true;
            break;
        }
    }
    p.file.start_scope_nl();
    p.file.write("control");
    p.file.new_line();
    p.file.write("(via_at_smd ");
    if via_at_smd_allowed {
        p.file.write("on)");
    } else {
        p.file.write("off)");
    }
    p.file.end_scope();
}

fn write_keepout_scope(p: &mut WriteScopeParameter<'_>, keepout_id: ItemId) {
    let board = p.board;
    let ctx = board.ctx();
    let Some(keepout) = board.items.get(&keepout_id) else {
        return;
    };
    let Some((keepout_area, layer_index)) = obstacle_area_of(keepout, &ctx) else {
        return;
    };
    let board_layer = &board.layer_structure().layers[layer_index];
    let keepout_layer = DsnLayer::new(
        board_layer.name.clone(),
        i32::try_from(layer_index).unwrap_or(i32::MAX),
        board_layer.is_signal,
    );
    let (boundary_shape, holes) = match keepout_area {
        Area::Shape(s) => (s.clone(), Vec::new()),
        area => (area.get_border(), area.get_holes()),
    };
    p.file.start_scope_nl();
    if matches!(keepout, Item::ViaObstacleArea(_)) {
        p.file.write("via_keepout");
    } else {
        p.file.write("keepout");
    }
    if let Some(dsn_shape) = p
        .coordinate_transform
        .board_to_dsn_shape(&boundary_shape, keepout_layer.clone())
    {
        dsn_shape.write_scope(&mut p.file, &p.identifier_type);
    }
    for hole in &holes {
        if let Some(dsn_hole) = p
            .coordinate_transform
            .board_to_dsn_shape(hole, keepout_layer.clone())
        {
            dsn_hole.write_hole_scope(&mut p.file, &p.identifier_type);
        }
    }
    if keepout.clearance_class() > 0 {
        let clearance_name = clearance_class_name(&board.rules, keepout.clearance_class());
        if clearance_name != "default" {
            let clearance_name = clearance_name.to_string();
            write_item_clearance_class(&clearance_name, &mut p.file, &p.identifier_type);
        }
    }
    p.file.end_scope();
}

pub fn write_snap_angle<W: std::io::Write>(
    file: &mut IndentFileWriter<W>,
    angle_restriction: AngleRestriction,
) {
    file.start_scope_nl();
    file.write("snap_angle ");
    file.new_line();
    match angle_restriction {
        AngleRestriction::NinetyDegree => file.write("ninety_degree"),
        AngleRestriction::FortyFiveDegree => file.write("fortyfive_degree"),
        AngleRestriction::None => file.write("none"),
    }
    file.end_scope();
}

fn clearance_equals(cl_matrix: &ClearanceMatrix, layer1: usize, layer2: usize) -> bool {
    if layer1 == layer2 {
        return true;
    }
    for i in 1..cl_matrix.get_class_count() {
        for j in i..cl_matrix.get_class_count() {
            if cl_matrix.get_value(i, j, layer1, false) != cl_matrix.get_value(i, j, layer2, false)
            {
                return false;
            }
        }
    }
    true
}

pub fn write_layer_scope(p: &mut WriteScopeParameter<'_>, layer_index: usize, write_rule: bool) {
    let board = p.board;
    p.file.start_scope_nl();
    p.file.write("layer ");
    let board_layer = &board.layer_structure().layers[layer_index];
    p.identifier_type.write(&board_layer.name, &mut p.file);
    p.file.new_line();
    p.file.write("(type ");
    if board_layer.is_signal {
        p.file.write("signal)");
    } else {
        p.file.write("power)");
    }
    if write_rule {
        write_default_rule(p, layer_index);
    }
    p.file.end_scope();
}

pub fn write_plane_scope(p: &mut WriteScopeParameter<'_>, conduction_id: ItemId) {
    let board = p.board;
    let ctx = board.ctx();
    let Some(conduction) = board.items.get(&conduction_id) else {
        return;
    };
    if conduction.net_count() != 1 {
        return;
    }
    let Some(net) = board.rules.nets.get(conduction.get_net_number(0)) else {
        return;
    };
    let net_name = net.name.clone();
    let Some((current_area, layer_index)) = obstacle_area_of(conduction, &ctx) else {
        return;
    };
    let board_layer = &board.layer_structure().layers[layer_index];
    let plane_layer = DsnLayer::new(
        board_layer.name.clone(),
        i32::try_from(layer_index).unwrap_or(i32::MAX),
        board_layer.is_signal,
    );
    let (boundary_shape, holes) = match current_area {
        Area::Shape(s) => (s.clone(), Vec::new()),
        area => (area.get_border(), area.get_holes()),
    };
    p.file.start_scope_nl();
    p.file.write("plane ");
    p.identifier_type.write(&net_name, &mut p.file);
    if let Some(dsn_shape) = p
        .coordinate_transform
        .board_to_dsn_shape(&boundary_shape, plane_layer.clone())
    {
        dsn_shape.write_scope(&mut p.file, &p.identifier_type);
    }
    for hole in &holes {
        if let Some(dsn_hole) = p
            .coordinate_transform
            .board_to_dsn_shape(hole, plane_layer.clone())
        {
            dsn_hole.write_hole_scope(&mut p.file, &p.identifier_type);
        }
    }
    p.file.end_scope();
}

fn obstacle_area_of<'b>(item: &'b Item, ctx: &ItemCtx<'_>) -> Option<(&'b Area, usize)> {
    match item {
        Item::ObstacleArea(a) => Some((a.get_area(ctx), a.get_layer())),
        Item::ConductionArea(a) => Some((a.get_area(ctx), a.get_layer())),
        Item::ViaObstacleArea(a) => Some((a.get_area(ctx), a.get_layer())),
        Item::ComponentObstacleArea(a) => Some((a.get_area(ctx), a.get_layer())),
        _ => None,
    }
}

pub(crate) fn default_net_class_trace_half_width(rules: &BoardRules, layer: usize) -> i32 {
    if rules.net_classes.count() == 0 {
        return 1500;
    }
    rules
        .net_classes
        .get(fr_board::NetClassId(0))
        .get_trace_half_width(layer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::DsnScanner;

    fn scan(input: &str) -> DsnScanner {
        DsnScanner::new(input)
    }

    #[test]
    fn read_snap_angle_reads_the_three_keywords() {
        assert_eq!(
            read_snap_angle(&mut scan("ninety_degree)")).expect("scan"),
            Some(fr_board::AngleRestriction::NinetyDegree)
        );
        assert_eq!(
            read_snap_angle(&mut scan("fortyfive_degree)")).expect("scan"),
            Some(fr_board::AngleRestriction::FortyFiveDegree)
        );
        assert_eq!(
            read_snap_angle(&mut scan("none)")).expect("scan"),
            Some(fr_board::AngleRestriction::None)
        );
        assert_eq!(read_snap_angle(&mut scan("on)")).expect("scan"), None);
        assert_eq!(
            read_snap_angle(&mut scan("none none)")).expect("scan"),
            None
        );
    }

    #[test]
    fn read_via_padstacks_appends_the_spare_vias_last() {
        let mut scanner = scan("Via_1 Via_2 (spare Spare_1))");
        let vias = read_via_padstacks(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(vias, vec!["Via_1", "Via_2", "Spare_1"]);
    }

    #[test]
    fn contains_wire_clearance_pair_matches_either_end() {
        assert!(contains_wire_clearance_pair(&["wire_via".to_string()]));
        assert!(contains_wire_clearance_pair(&["via_wire".to_string()]));
        assert!(!contains_wire_clearance_pair(&["via_smd".to_string()]));
    }
}
