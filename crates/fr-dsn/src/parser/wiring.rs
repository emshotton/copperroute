use std::io::Write;

use fr_board::datastructures::TimeLimit;
use fr_board::rules::ItemClass;
use fr_board::{Board, BoardError, FixedState, Item, ItemId, NetClassId, PadstackId};
use fr_geometry::{Area, FloatPoint, Line, Point, Polygon, Polyline, TileShape};

use crate::error::DsnError;
use crate::format::{IdentifierType, IndentFileWriter, format_double};
use crate::keyword::Keyword;
use crate::lexer::Token;
use crate::parser::geometry::{self as shape, DsnLayer, DsnPolygonPath, DsnPolylinePath, DsnShape};
use crate::parser::network::{
    NetId, clearance_class_name, write_item_clearance_class, write_net_id,
};
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};
use crate::parser::{dsn_file, library};

pub fn read_wiring_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            return Ok(false);
        };
        if token == Token::Close {
            break;
        }
        let mut read_ok = true;
        if prev_token == Some(Token::Open) {
            match token {
                Token::Kw(Keyword::Wire) => {
                    read_wire_scope(p)?;
                }
                Token::Kw(Keyword::Via) => read_ok = read_via_scope(p)?,
                _ => {
                    skip_scope(&mut p.scanner)?;
                }
            }
        }
        if !read_ok {
            return Ok(false);
        }
    }

    let limit_ms = p.options.normalize_time_limit_ms();
    let deadline = TimeLimit::new(limit_ms);
    let stop = move || limit_ms <= 0 || deadline.is_exceeded();
    let Some(board) = p.board.as_mut() else {
        return Ok(false);
    };
    if board.normalize_all_traces_checked(&stop).is_err() {
        p.warnings
            .push("Wiring: normalization of traces failed".to_string());
    }
    Ok(true)
}

#[allow(clippy::too_many_lines)]
fn read_wire_scope(p: &mut ReadScopeParameter<'_>) -> Result<Option<ItemId>, DsnError> {
    let mut net_id: Option<NetId> = None;
    let mut clearance_class_name: Option<String> = None;
    let mut fixed = FixedState::Unfixed;
    let mut path: Option<DsnShape> = None;
    let mut border_shape: Option<DsnShape> = None;
    let mut hole_list: Vec<Option<DsnShape>> = Vec::new();

    let layer_structure = p.layer_structure.clone();
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            return Ok(None);
        };
        if token == Token::Close {
            break;
        }
        if prev_token != Some(Token::Open) {
            continue;
        }
        match token {
            Token::Kw(Keyword::PolygonPath) => {
                path = shape::read_polygon_path_scope(&mut p.scanner, layer_structure.as_ref())?
                    .map(DsnShape::Path);
            }
            Token::Kw(Keyword::PolylinePath) => {
                path = shape::read_polyline_path_scope(&mut p.scanner, layer_structure.as_ref())?
                    .map(DsnShape::PolylinePath);
            }
            Token::Kw(Keyword::Rectangle) => {
                border_shape =
                    shape::read_rectangle_scope(&mut p.scanner, layer_structure.as_ref())?
                        .map(DsnShape::Rect);
            }
            Token::Kw(Keyword::Polygon) => {
                border_shape = shape::read_polygon_scope(&mut p.scanner, layer_structure.as_ref())?
                    .map(DsnShape::Polygon);
            }
            Token::Kw(Keyword::Circle) => {
                border_shape = shape::read_circle_scope(&mut p.scanner, layer_structure.as_ref())?
                    .map(DsnShape::Circle);
            }
            Token::Kw(Keyword::Window) => {
                let hole_shape = shape::read_scope(&mut p.scanner, layer_structure.as_ref())?;
                hole_list.push(hole_shape);
                next_token = p.scanner.next_token()?;
                if next_token != Some(Token::Close) {
                    return Ok(None);
                }
            }
            Token::Kw(Keyword::Net) => net_id = read_net_id(p),
            Token::Kw(Keyword::ClearanceClass) => {
                clearance_class_name = Some(dsn_file::read_string_scope(&mut p.scanner)?);
            }
            Token::Kw(Keyword::Type) => fixed = calc_fixed(p)?,
            _ => {
                skip_scope(&mut p.scanner)?;
            }
        }
    }

    if path.is_none() && border_shape.is_none() {
        let msg = format!(
            "Wiring: wire has no shape at '{}'",
            p.scanner.scope_identifier()
        );
        p.warnings.push(msg);
        return Ok(None);
    }

    let Some(coordinate_transform) = p.coordinate_transform else {
        return Ok(None);
    };
    let Some(board) = p.board.as_mut() else {
        return Ok(None);
    };

    let mut net_class: NetClassId = board.rules.get_default_net_class();
    let found_nets = get_subnets(net_id.as_ref(), board);
    let mut net_numbers: Vec<i32> = Vec::with_capacity(found_nets.len());
    for (net_number, found_class) in &found_nets {
        net_numbers.push(*net_number);
        net_class = *found_class;
    }

    let mut clearance_class_index: i32 = -1;
    if let Some(name) = &clearance_class_name {
        clearance_class_index = board
            .rules
            .clearance_matrix
            .get_no(name)
            .map_or(-1, |no| no as i32);
    }

    let (layer_index, half_width) = match (&path, &border_shape) {
        (Some(path), _) => (
            path.layer().no,
            (coordinate_transform.dsn_to_board(path_width(path) / 2.0)).round() as i32,
        ),
        (None, Some(border_shape)) => (border_shape.layer().no, 0),
        (None, None) => unreachable!("the `path == null && borderShape == null` exit is above"),
    };

    if layer_index < 0 || layer_index >= board.get_layer_count() as i32 {
        let layer_name = match (&path, &border_shape) {
            (Some(path), _) => path.layer().name.clone(),
            (None, Some(border_shape)) => border_shape.layer().name.clone(),
            (None, None) => unreachable!(),
        };
        let msg = format!(
            "Wiring: wire ignored — unknown layer '{layer_name}' at '{}'",
            p.scanner.scope_identifier()
        );
        p.warnings.push(msg);
        return Ok(None);
    }
    let layer_index = layer_index as usize;

    let bounding_box = TileShape::from(board.get_bounding_box());

    let mut result: Option<ItemId> = None;
    if let Some(border_shape) = border_shape {
        if clearance_class_index < 0 {
            clearance_class_index = board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes
                .get(ItemClass::Area) as i32;
        }
        let mut area: Vec<Option<DsnShape>> = Vec::with_capacity(1 + hole_list.len());
        area.push(Some(border_shape));
        area.extend(hole_list);
        let Some(conduction_area) = shape::transform_area_to_board(&area, &coordinate_transform)
        else {
            return Ok(None);
        };
        result = Some(board.insert_conduction_area(
            conduction_area,
            layer_index,
            net_numbers.clone(),
            clearance_class_index as usize,
            false,
            fixed,
        ));
    } else if let Some(DsnShape::Path(path)) = &path {
        if clearance_class_index < 0 {
            clearance_class_index = board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes
                .get(ItemClass::Trace) as i32;
        }
        let coordinate_arr = &path.coordinate_arr;
        let mut corners: Vec<Point> = Vec::with_capacity(coordinate_arr.len() / 2);
        for i in 0..coordinate_arr.len() / 2 {
            let current_point = [coordinate_arr[2 * i], coordinate_arr[2 * i + 1]];
            let current_corner = coordinate_transform.dsn_to_board_point(&current_point);
            if !bounding_box.contains_float(&current_corner) {
                let msg = format!(
                    "Wiring: wire corner ({},{}) is outside board bounds at '{}'",
                    current_point[0] as i32,
                    current_point[1] as i32,
                    p.scanner.scope_identifier()
                );
                p.warnings.push(msg);
                return Ok(None);
            }
            corners.push(Point::Int(current_corner.round()));
        }

        let polygon = Polygon::new(corners);
        let polygon_corners = polygon.corner_array();
        let has_distinct_corner = polygon_corners
            .iter()
            .skip(1)
            .any(|corner| *corner != polygon_corners[0]);
        let is_degenerate = polygon_corners.len() < 2 || !has_distinct_corner;
        if is_degenerate {
            let msg = format!(
                "Wiring: degenerate wire trace skipped (all {} corners are identical — \
                 zero-length trace) on layer '{}'. This is likely a DSN export issue in your EDA \
                 tool.",
                polygon_corners.len(),
                path.layer.name
            );
            p.warnings.push(msg);
        } else {
            let trace_polyline = Polyline::from_polygon(&polygon);
            result = board.insert_trace_without_cleaning(
                trace_polyline,
                layer_index,
                half_width,
                net_numbers.clone(),
                clearance_class_index as usize,
                fixed,
            );
        }
    } else if let Some(DsnShape::PolylinePath(path)) = &path {
        if clearance_class_index < 0 {
            clearance_class_index = board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes
                .get(ItemClass::Trace) as i32;
        }
        let coordinate_arr = &path.coordinate_arr;
        let mut lines: Vec<Line> = Vec::with_capacity(coordinate_arr.len() / 4);
        for i in 0..coordinate_arr.len() / 4 {
            let a = coordinate_transform
                .dsn_to_board_point(&[coordinate_arr[4 * i], coordinate_arr[4 * i + 1]]);
            let b = coordinate_transform
                .dsn_to_board_point(&[coordinate_arr[4 * i + 2], coordinate_arr[4 * i + 3]]);
            lines.push(Line::new(a.round(), b.round()));
        }
        let trace_polyline = Polyline::from_lines(lines).map_err(BoardError::from)?;
        result = board.insert_trace_without_cleaning(
            trace_polyline,
            layer_index,
            half_width,
            net_numbers.clone(),
            clearance_class_index as usize,
            fixed,
        );
    } else {
        return Ok(None);
    }

    if let Some(id) = result
        && board
            .items
            .get(&id)
            .is_some_and(|item| item.header().net_count() == 0)
    {
        try_correct_net(board, id);
    }
    Ok(result)
}

fn try_correct_net(board: &mut Board, id: ItemId) {
    let Some(fr_board::Item::Trace(trace)) = board.items.get(&id) else {
        return;
    };
    let (first_corner, last_corner) = (trace.first_corner(), trace.last_corner());
    let mut contacts = std::collections::BTreeSet::new();
    if let Some(corner) = first_corner {
        contacts.extend(board.trace_normal_contacts_at(id, &corner, true));
    }
    if let Some(corner) = last_corner {
        contacts.extend(board.trace_normal_contacts_at(id, &corner, true));
    }
    let mut corrected_net_no = 0;
    for contact in contacts.into_iter().rev() {
        let Some(item) = board.items.get(&contact) else {
            continue;
        };
        if item.header().net_count() == 1 {
            corrected_net_no = item.header().get_net_number(0);
            break;
        }
    }
    if corrected_net_no != 0 {
        let nets = board.rules.nets.clone();
        if let Some(item) = board.items.get_mut(&id) {
            item.assign_net_no(corrected_net_no, &nets);
        }
    }
}

#[allow(clippy::too_many_lines)]
fn read_via_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut fixed = FixedState::Unfixed;
    let mut next_token = p.scanner.next_token()?;
    let Some(Token::Str(padstack_name)) = next_token.clone() else {
        return Ok(false);
    };
    p.scanner.set_scope_identifier(&padstack_name);

    let mut location = [0.0f64; 2];
    for slot in &mut location {
        next_token = p.scanner.next_token()?;
        match next_token {
            Some(Token::Float(value)) => *slot = value,
            Some(Token::Int(value)) => *slot = value as f64,
            _ => {
                return Ok(false);
            }
        }
    }

    let mut net_id: Option<NetId> = None;
    let mut clearance_class_name: Option<String> = None;
    loop {
        let prev_token = next_token;
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            return Ok(false);
        };
        if token == Token::Close {
            break;
        }
        if prev_token != Some(Token::Open) {
            continue;
        }
        match token {
            Token::Kw(Keyword::Net) => net_id = read_net_id(p),
            Token::Kw(Keyword::ClearanceClass) => {
                clearance_class_name = Some(dsn_file::read_string_scope(&mut p.scanner)?);
            }
            Token::Kw(Keyword::Type) => fixed = calc_fixed(p)?,
            _ => {
                skip_scope(&mut p.scanner)?;
            }
        }
    }

    let Some(coordinate_transform) = p.coordinate_transform else {
        return Ok(false);
    };
    let Some(board) = p.board.as_mut() else {
        return Ok(false);
    };

    let cleaned_name = library::strip_dot_digits(&padstack_name);
    let Some(current_padstack) = board
        .library
        .padstacks
        .get_by_name(&cleaned_name)
        .map(|padstack| PadstackId(padstack.no))
    else {
        let msg = format!(
            "Wiring: via padstack '{padstack_name}' not found at '{}'",
            p.scanner.scope_identifier()
        );
        p.warnings.push(msg);
        return Ok(false);
    };

    let mut net_class: NetClassId = board.rules.get_default_net_class();
    let found_nets = get_subnets(net_id.as_ref(), board);
    if let Some(net_id) = &net_id
        && found_nets.is_empty()
    {
        let msg = format!(
            "Wiring: via net '{}' not found at '{}'",
            net_id.name,
            p.scanner.scope_identifier()
        );
        p.warnings.push(msg);
    }
    let mut net_numbers: Vec<i32> = vec![0; found_nets.len()];
    for (index, (net_number, found_class)) in found_nets.iter().enumerate() {
        net_numbers[index] = *net_number;
        net_class = *found_class;
    }

    let mut clearance_class_index: i32 = -1;
    if let Some(name) = &clearance_class_name {
        clearance_class_index = board
            .rules
            .clearance_matrix
            .get_no(name)
            .map_or(-1, |no| no as i32);
    }
    if clearance_class_index < 0 {
        clearance_class_index = board
            .rules
            .net_classes
            .get(net_class)
            .default_item_clearance_classes
            .get(ItemClass::Via) as i32;
    }

    let board_location = coordinate_transform.dsn_to_board_point(&location).round();
    if via_exists(board, &board_location, current_padstack, &net_numbers) {
        let msg = format!(
            "Wiring: duplicate via skipped at ({}, {})",
            board_location.x, board_location.y
        );
        p.warnings.push(msg);
    } else {
        let attach_allowed = p.via_at_smd_allowed
            && board
                .library
                .padstacks
                .get(current_padstack)
                .is_some_and(|padstack| padstack.attach_allowed);
        let limit_ms = p.options.normalize_time_limit_ms();
        let deadline = TimeLimit::new(limit_ms);
        let stop = move || limit_ms > 0 && deadline.is_exceeded();
        let board = p.board.as_mut().expect("checked above");
        board.insert_via_checked(
            current_padstack,
            Point::Int(board_location),
            net_numbers,
            clearance_class_index as usize,
            fixed,
            attach_allowed,
            &stop,
        )?;
    }
    Ok(true)
}

fn via_exists(
    board: &Board,
    location: &fr_geometry::IntPoint,
    padstack: PadstackId,
    net_numbers: &[i32],
) -> bool {
    let Some(padstack) = board.library.padstacks.get(padstack) else {
        return false;
    };
    let from_layer = padstack.from_layer();
    let to_layer = padstack.to_layer();
    let Ok(pick_layer) = usize::try_from(from_layer) else {
        return false;
    };
    let point = Point::Int(*location);
    let ctx = board.ctx();
    for id in board.pick_items(&point, Some(pick_layer)) {
        let Some(fr_board::Item::Via(via)) = board.items.get(&id) else {
            continue;
        };
        if via.hdr.nets_equal(net_numbers)
            && via.get_center() == point
            && via.first_layer(&ctx) as i32 == from_layer
            && via.last_layer(&ctx) as i32 == to_layer
        {
            return true;
        }
    }
    false
}

fn get_subnets(net_id: Option<&NetId>, board: &Board) -> Vec<(i32, NetClassId)> {
    let Some(net_id) = net_id else {
        return Vec::new();
    };
    if net_id.subnet_no > 0 {
        return board
            .rules
            .nets
            .get_by_name_and_subnet(&net_id.name, net_id.subnet_no)
            .map(|net| vec![(net.net_number, net.get_net_class())])
            .unwrap_or_default();
    }
    board
        .rules
        .nets
        .get_by_name(&net_id.name)
        .into_iter()
        .map(|net| (net.net_number, net.get_net_class()))
        .collect()
}

fn read_net_id(p: &mut ReadScopeParameter<'_>) -> Option<NetId> {
    let mut subnet_number = 0;
    let net_name = p.scanner.next_string();
    p.scanner.set_scope_identifier(&net_name);
    let mut next_token = p.scanner.next_token().ok()?;
    if let Some(Token::Int(value)) = next_token {
        subnet_number = value as i32;
        next_token = p.scanner.next_token().ok()?;
    }
    let _ = next_token;
    Some(NetId::new(net_name, subnet_number))
}

fn calc_fixed(p: &mut ReadScopeParameter<'_>) -> Result<FixedState, DsnError> {
    let mut result = FixedState::Unfixed;
    let next_token = p.scanner.next_token()?;
    match next_token {
        Some(Token::Kw(Keyword::ShoveFixed)) => result = FixedState::ShoveFixed,
        Some(Token::Kw(Keyword::Fix)) => result = FixedState::SystemFixed,
        Some(Token::Kw(Keyword::Normal)) => {}
        _ => result = FixedState::UserFixed,
    }
    let next_token = p.scanner.next_token()?;
    if next_token != Some(Token::Close) {
        return Ok(FixedState::Unfixed);
    }
    Ok(result)
}

fn path_width(shape: &DsnShape) -> f64 {
    match shape {
        DsnShape::Path(path) => path.width,
        DsnShape::PolylinePath(path) => path.width,
        _ => 0.0,
    }
}

pub fn write_wiring_scope(p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("wiring");
    let board_wires = p.board.get_traces();
    for current_board_wire in board_wires {
        write_wire_scope(p, current_board_wire);
    }
    let board_vias = p.board.get_vias();
    for current_via in board_vias {
        write_via_scope(p, current_via);
    }
    let board = p.board;
    for id in board.items_in_board_order() {
        let Some(Item::ConductionArea(area)) = board.items.get(&id) else {
            continue;
        };
        if !board.layer_structure().layers[area.get_layer()].is_signal {
            continue;
        }
        write_conduction_area_scope(p, id);
    }
    p.file.end_scope();
}

fn write_via_scope(p: &mut WriteScopeParameter<'_>, via_id: ItemId) {
    let board = p.board;
    let ctx = board.ctx();
    let Some(item) = board.items.get(&via_id) else {
        return;
    };
    let Item::Via(via) = item else {
        return;
    };
    let Some(via_padstack) = via.get_padstack(&ctx) else {
        return;
    };
    let via_padstack_name = via_padstack.name.clone();
    let via_location = via.get_center().to_float();
    let via_coor = p.coordinate_transform.board_to_dsn_point(&via_location);
    let via_net = if item.net_count() > 0 {
        board.rules.nets.get(item.get_net_number(0)).cloned()
    } else {
        None
    };
    p.file.start_scope_nl();
    p.file.write("via ");
    p.identifier_type.write(&via_padstack_name, &mut p.file);
    for coor in via_coor {
        p.file.write(" ");
        p.file.write(&format_double(coor));
    }
    if let Some(via_net) = &via_net {
        write_net(via_net, &mut p.file, &p.identifier_type);
    }
    let clearance_name = clearance_class_name(&board.rules, item.clearance_class()).to_string();
    write_item_clearance_class(&clearance_name, &mut p.file, &p.identifier_type);
    write_fixed_state(&mut p.file, item.get_fixed_state());
    p.file.end_scope();
}

fn write_wire_scope(p: &mut WriteScopeParameter<'_>, wire_id: ItemId) {
    let board = p.board;
    let Some(item) = board.items.get(&wire_id) else {
        return;
    };
    let Item::Trace(current_wire) = item else {
        return;
    };
    let layer_index = current_wire.get_layer();
    let board_layer = &board.layer_structure().layers[layer_index];
    let current_layer = DsnLayer::new(
        board_layer.name.clone(),
        i32::try_from(layer_index).unwrap_or(i32::MAX),
        board_layer.is_signal,
    );
    let wire_width = p
        .coordinate_transform
        .board_to_dsn(f64::from(2 * current_wire.get_half_width()));
    let wire_net = if item.net_count() > 0 {
        board.rules.nets.get(item.get_net_number(0))
    } else {
        None
    };
    let Some(wire_net) = wire_net else {
        return;
    };
    p.file.start_scope_nl();
    p.file.write("wire");

    if p.compat_mode {
        let corners = current_wire.polyline().corners();
        let float_corner_arr: Vec<FloatPoint> = corners.iter().map(Point::to_float).collect();
        let coors = p
            .coordinate_transform
            .board_to_dsn_points(&float_corner_arr);
        let current_path = DsnPolygonPath::new(current_layer, wire_width, coors);
        current_path.write_scope(&mut p.file, &p.identifier_type);
    } else {
        let coors = p
            .coordinate_transform
            .board_to_dsn_lines(current_wire.polyline().lines());
        let current_path = DsnPolylinePath::new(current_layer, wire_width, coors);
        current_path.write_scope(&mut p.file, &p.identifier_type);
    }
    write_net(wire_net, &mut p.file, &p.identifier_type);
    let clearance_name = clearance_class_name(&board.rules, item.clearance_class()).to_string();
    write_item_clearance_class(&clearance_name, &mut p.file, &p.identifier_type);
    write_fixed_state(&mut p.file, item.get_fixed_state());
    p.file.end_scope();
}

fn write_conduction_area_scope(p: &mut WriteScopeParameter<'_>, conduction_id: ItemId) {
    let board = p.board;
    let ctx = board.ctx();
    let Some(item) = board.items.get(&conduction_id) else {
        return;
    };
    let Item::ConductionArea(conduction_area) = item else {
        return;
    };
    let net_count = item.net_count();
    if net_count != 1 {
        return;
    }
    let Some(current_net) = board.rules.nets.get(item.get_net_number(0)).cloned() else {
        return;
    };
    let current_area = conduction_area.get_area(&ctx);
    let layer_index = conduction_area.get_layer();
    let board_layer = &board.layer_structure().layers[layer_index];
    let conduction_layer = DsnLayer::new(
        board_layer.name.clone(),
        i32::try_from(layer_index).unwrap_or(i32::MAX),
        board_layer.is_signal,
    );
    let (boundary_shape, holes) = match current_area {
        Area::Shape(s) => (s.clone(), Vec::new()),
        area => (area.get_border(), area.get_holes()),
    };
    p.file.start_scope_nl();
    p.file.write("wire ");
    if let Some(dsn_shape) = p
        .coordinate_transform
        .board_to_dsn_shape(&boundary_shape, conduction_layer.clone())
    {
        dsn_shape.write_scope(&mut p.file, &p.identifier_type);
    }
    for hole in &holes {
        if let Some(dsn_hole) = p
            .coordinate_transform
            .board_to_dsn_shape(hole, conduction_layer.clone())
        {
            dsn_hole.write_hole_scope(&mut p.file, &p.identifier_type);
        }
    }
    write_net(&current_net, &mut p.file, &p.identifier_type);
    let clearance_name = clearance_class_name(&board.rules, item.clearance_class()).to_string();
    write_item_clearance_class(&clearance_name, &mut p.file, &p.identifier_type);
    p.file.end_scope();
}

fn write_net<W: Write>(
    net: &fr_board::Net,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    file.new_line();
    file.write("(");
    write_net_id(net, file, identifier_type);
    file.write(")");
}

fn write_fixed_state<W: Write>(file: &mut IndentFileWriter<W>, fixed_state: FixedState) {
    if fixed_state == FixedState::Unfixed {
        return;
    }
    file.new_line();
    file.write("(type ");
    match fixed_state {
        FixedState::ShoveFixed => file.write("shove_fixed)"),
        FixedState::SystemFixed => file.write("fix)"),
        _ => file.write("protect)"),
    }
}
