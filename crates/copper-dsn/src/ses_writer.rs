use std::collections::BTreeSet;
use std::io::{self, Write};

use copper_board::{Board, FixedState, Item, ItemId, Padstack, Unit};
use copper_geometry::{Area, FloatPoint, Shape, ShapeOps};

use crate::coordinate_transform::CoordinateTransform;
use crate::format::double::format_fixed;
use crate::format::{IdentifierType, IndentFileWriter, SES_RESERVED, format_placement_rotation};
use crate::parser::geometry::DsnLayer;
use crate::parser::header::{write_parser_scope, write_resolution_scope};

pub fn write<W: Write>(
    board: &Board,
    ct: &CoordinateTransform,
    out: &mut W,
    design_name: &str,
) -> io::Result<()> {
    let mut output_file = IndentFileWriter::new(out);
    let session_name = design_name.replace(".dsn", ".ses");
    let identifier_type = IdentifierType::new(
        SES_RESERVED.iter().map(|s| (*s).to_string()).collect(),
        board.communication.string_quote.clone(),
    );
    write_session_scope(
        board,
        ct,
        &identifier_type,
        &mut output_file,
        &session_name,
        design_name,
    )?;
    output_file.flush()
}

fn write_session_scope<W: Write>(
    board: &Board,
    ct: &CoordinateTransform,
    identifier_type: &IdentifierType,
    file: &mut IndentFileWriter<W>,
    session_name: &str,
    design_name: &str,
) -> io::Result<()> {
    let scale_factor = ct.dsn_to_board(1.0) / f64::from(board.communication.resolution);
    let coordinate_transform =
        CoordinateTransform::new(scale_factor, 0.0, 0.0).map_err(io::Error::other)?;
    file.start_scope(false);
    file.write("session ");
    identifier_type.write(session_name, file);
    file.new_line();
    file.write("(base_design ");
    identifier_type.write(design_name, file);
    file.write(")");
    write_placement(board, identifier_type, &coordinate_transform, file);
    write_was_is(board, identifier_type, file);
    write_routes(board, identifier_type, &coordinate_transform, file);
    file.end_scope();
    Ok(())
}

fn write_placement<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("placement");
    write_resolution_scope(file, &board.communication);
    for i in 1..=board.library.packages.count() {
        write_components(board, identifier_type, ct, file, i);
    }
    file.end_scope();
}

fn write_components<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
    package_no: usize,
) {
    let mut component_found = false;
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    for i in 1..=board.components.count() as i32 {
        let current_component = board.components.get(i);
        if current_component.get_package() != package_no {
            continue;
        }
        let undeleted_item_found = board
            .get_items()
            .any(|item| item.component_id() == current_component.id);
        if !undeleted_item_found {
            continue;
        }
        if !component_found {
            let package_name = board.library.packages.get(package_no).name.clone();
            file.start_scope_nl();
            file.write("component ");
            identifier_type.write(&package_name, file);
            component_found = true;
        }
        write_component(identifier_type, ct, file, current_component);
    }
    if component_found {
        file.end_scope();
    }
}

fn write_component<W: Write>(
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
    component: &copper_board::Component,
) {
    file.new_line();
    file.write("(place ");
    identifier_type.write(&component.name, file);
    if let Some(location) = component.get_location() {
        let location = ct.board_to_dsn_point(&location.to_float());
        let xcoordinate = (location[0]).round() as i32;
        let ycoordinate = (location[1]).round() as i32;
        file.write(" ");
        file.write(&xcoordinate.to_string());
        file.write(" ");
        file.write(&ycoordinate.to_string());
        if component.placed_on_front() {
            file.write(" front ");
        } else {
            file.write(" back ");
        }
        file.write(&format_placement_rotation(
            component.get_rotation_in_degree(),
        ));
    }
    if component.position_fixed {
        file.new_line();
        file.write(" (lock_type position)");
    }
    file.write(")");
}

fn write_was_is<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("was_is");
    for pin_id in board.get_pins() {
        let Some(Item::Pin(current_pin)) = board.get_item(pin_id) else {
            continue;
        };
        let swapped_with = current_pin.get_changed_to();
        if swapped_with == pin_id {
            continue;
        }
        file.new_line();
        file.write("(pins ");
        write_swapped_pin(board, identifier_type, file, pin_id);
        file.write(" ");
        write_swapped_pin(board, identifier_type, file, swapped_with);
        file.write(")");
    }
    file.end_scope();
}

fn write_swapped_pin<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    file: &mut IndentFileWriter<W>,
    pin_id: ItemId,
) {
    let Some(item) = board.get_item(pin_id) else {
        return;
    };
    let Item::Pin(pin) = item else {
        return;
    };
    let component_id = item.component_id();
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    if component_id < 1 || component_id > board.components.count() as i32 {
        return;
    }
    let component = board.components.get(component_id);
    let component_name = component.name.clone();
    let package_pin_name = board
        .library
        .packages
        .get(component.get_package())
        .get_pin(pin.get_pin_index())
        .map(|p| p.name.clone());
    identifier_type.write(&component_name, file);
    file.write("-");
    if let Some(package_pin_name) = package_pin_name {
        identifier_type.write(&package_pin_name, file);
    }
}

fn write_routes<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("routes ");
    write_resolution_scope(file, &board.communication);
    write_parser_scope(file, &board.communication, identifier_type, true);
    write_library(board, identifier_type, ct, file);
    write_network(board, identifier_type, ct, file);
    file.end_scope();
}

fn write_library<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("library_out ");
    let mut written_padstack_names: BTreeSet<String> = BTreeSet::new();
    for i in 0..board.library.via_padstack_count() {
        let Some(via_padstack) = board
            .library
            .get_via_padstack(i)
            .and_then(|id| board.library.get_padstack(id))
        else {
            continue;
        };
        let session_name = via_padstack_session_name(via_padstack, board);
        if !written_padstack_names.insert(session_name.clone()) {
            continue;
        }
        write_padstack(
            via_padstack,
            &session_name,
            board,
            identifier_type,
            ct,
            file,
        );
    }
    file.end_scope();
}

fn is_kicad_net_class_via_template_name(name: &str) -> bool {
    name == "defaultVia" || name.starts_with("via_")
}

fn via_padstack_session_name(padstack: &Padstack, board: &Board) -> String {
    if !is_kicad_net_class_via_template_name(&padstack.name) {
        return padstack.name.clone();
    }
    let from_layer = padstack.from_layer();
    let to_layer = padstack.to_layer();
    let diameter_board = padstack
        .get_shape(from_layer)
        .map(|shape| {
            let bounds = shape.bounding_box();
            f64::from(bounds.width().min(bounds.height()))
        })
        .unwrap_or(0.0);
    let resolution = f64::from(board.communication.resolution);
    let to_um = |board_units: f64| {
        Unit::scale(board_units / resolution, board.communication.unit, Unit::Um)
    };
    format!(
        "Via[{from_layer}-{to_layer}]_{}:{}_um",
        format_fixed(to_um(diameter_board), 0),
        format_fixed(to_um(padstack.drill_diameter.unwrap_or(0.0)), 0)
    )
}

fn write_padstack<W: Write>(
    padstack: &Padstack,
    padstack_name: &str,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    let layer_count = board.get_layer_count();
    let mut first_layer_no = 0usize;
    while first_layer_no < layer_count
        && padstack
            .get_shape(i32::try_from(first_layer_no).unwrap_or(i32::MAX))
            .is_none()
    {
        first_layer_no += 1;
    }
    let mut last_layer_no = i64::try_from(layer_count).unwrap_or(i64::MAX) - 1;
    while last_layer_no >= 0
        && padstack
            .get_shape(i32::try_from(last_layer_no).unwrap_or(i32::MAX))
            .is_none()
    {
        last_layer_no -= 1;
    }
    if first_layer_no >= layer_count || last_layer_no < 0 {
        return;
    }

    file.start_scope_nl();
    file.write("padstack ");
    identifier_type.write(padstack_name, file);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    for i in first_layer_no..=(last_layer_no as usize) {
        let Some(current_board_shape) = padstack.get_shape(i32::try_from(i).unwrap_or(i32::MAX))
        else {
            continue;
        };
        let board_layer = &board.layer_structure().layers[i];
        let current_layer = DsnLayer::new(
            board_layer.name.clone(),
            i32::try_from(i).unwrap_or(i32::MAX),
            board_layer.is_signal,
        );
        let current_shape = ct.board_to_dsn_rel_shape(current_board_shape, current_layer);
        file.start_scope_nl();
        file.write("shape");
        if let Some(current_shape) = current_shape {
            current_shape.write_scope_int(file, identifier_type);
        }
        file.end_scope();
    }
    if !padstack.attach_allowed {
        file.new_line();
        file.write("(attach off)");
    }
    file.end_scope();
}

fn write_network<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("network_out ");
    for i in 1..=board.rules.nets.max_net_number() {
        write_net(i, board, identifier_type, ct, file);
    }
    file.end_scope();
}

fn write_net<W: Write>(
    net_number: i32,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    let ctx = board.ctx();
    let mut header_written = false;
    for item_id in board.get_connectable_items(net_number) {
        let Some(current_item) = board.get_item(item_id) else {
            continue;
        };
        if current_item.get_fixed_state() == FixedState::SystemFixed {
            continue;
        }
        let is_wire = matches!(current_item, Item::Trace(_));
        let is_via = matches!(current_item, Item::Via(_));
        let is_conduction_area = matches!(current_item, Item::ConductionArea(_))
            && board.layer_structure().layers[current_item.first_layer(&ctx)].is_signal;
        if !header_written && (is_wire || is_via || is_conduction_area) {
            file.start_scope_nl();
            file.write("net ");
            match board.rules.nets.get(net_number) {
                None => {}
                Some(current_net) => {
                    let name = current_net.name.clone();
                    identifier_type.write(&name, file);
                }
            }
            header_written = true;
        }
        if is_wire {
            write_wire(item_id, board, identifier_type, ct, file);
        } else if is_via {
            write_via(item_id, board, identifier_type, ct, file);
        } else if is_conduction_area {
            write_conduction_area(item_id, board, identifier_type, ct, file);
        }
    }
    if header_written {
        file.end_scope();
    }
}

fn write_wire<W: Write>(
    wire_id: ItemId,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    let Some(item) = board.get_item(wire_id) else {
        return;
    };
    let Item::Trace(wire) = item else {
        return;
    };
    let layer_index = wire.get_layer();
    let board_layer_name = board.layer_structure().layers[layer_index].name.clone();
    let wire_width = (ct.board_to_dsn(f64::from(2 * wire.get_half_width()))).round() as i32;

    let corners = wire.polyline().corners();
    let mut coors: Vec<i32> = Vec::with_capacity(2 * corners.len());
    let mut prev_coors: Option<[i32; 2]> = None;
    for (i, corner) in corners.iter().enumerate() {
        let mut corner_point = corner.to_float();
        if (i == 0 || i == corners.len() - 1)
            && let Some(snapped) = snapped_endpoint(board, wire_id, i == 0)
        {
            corner_point = snapped;
        }
        let current_float_coors = ct.board_to_dsn_point(&corner_point);
        let current_coors = [
            (current_float_coors[0]).round() as i32,
            (current_float_coors[1]).round() as i32,
        ];
        if prev_coors != Some(current_coors) {
            coors.push(current_coors[0]);
            coors.push(current_coors[1]);
            prev_coors = Some(current_coors);
        }
    }

    file.start_scope_nl();
    file.write("wire");
    write_path(&board_layer_name, wire_width, &coors, identifier_type, file);
    write_fixed_state(file, item.get_fixed_state());
    file.end_scope();
}

pub fn snapped_endpoint(board: &Board, wire_id: ItemId, start_side: bool) -> Option<FloatPoint> {
    let ctx = board.ctx();
    let Some(Item::Trace(wire)) = board.get_item(wire_id) else {
        return None;
    };
    let corner = if start_side {
        wire.first_corner()
    } else {
        wire.last_corner()
    };
    let corner_float = corner?.to_float();
    let contacts = if start_side {
        board.trace_start_contacts(wire_id)
    } else {
        board.trace_end_contacts(wire_id)
    };
    let layer = wire.get_layer();
    for contact_id in contacts.into_iter().rev() {
        let Some(contact) = board.get_item(contact_id) else {
            continue;
        };
        let pad_shape = match contact {
            Item::Via(via) => {
                if layer < contact.first_layer(&ctx) || layer > contact.last_layer(&ctx) {
                    continue;
                }
                via.get_shape(layer - contact.first_layer(&ctx), &ctx)
            }
            Item::Pin(pin) => {
                if layer < contact.first_layer(&ctx) || layer > contact.last_layer(&ctx) {
                    continue;
                }
                pin.get_shape(layer - contact.first_layer(&ctx), &ctx)
            }
            _ => continue,
        };
        let Some(pad_shape) = pad_shape else {
            continue;
        };
        let Some(center) = board.drill_center(contact_id) else {
            continue;
        };
        let center = center.to_float();
        let center_distance = corner_float.distance(&center);
        if center_distance <= 0.5 {
            return None;
        }
        if center_distance <= pad_shape.border_distance(&center) {
            return Some(center);
        }
    }
    None
}

fn write_via<W: Write>(
    via_id: ItemId,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    let ctx = board.ctx();
    let Some(item) = board.get_item(via_id) else {
        return;
    };
    let Item::Via(via) = item else {
        return;
    };
    let Some(via_padstack) = via.get_padstack(&ctx) else {
        return;
    };
    let via_padstack_name = via_padstack_session_name(via_padstack, board);
    let via_location = via.get_center().to_float();
    file.start_scope_nl();
    file.write("via ");
    identifier_type.write(&via_padstack_name, file);
    file.write(" ");
    let location = ct.board_to_dsn_point(&via_location);
    file.write(&((location[0]).round() as i32).to_string());
    file.write(" ");
    file.write(&((location[1]).round() as i32).to_string());
    write_fixed_state(file, item.get_fixed_state());
    file.end_scope();
}

fn write_fixed_state<W: Write>(file: &mut IndentFileWriter<W>, fixed_state: FixedState) {
    if fixed_state <= FixedState::ShoveFixed {
        return;
    }
    file.new_line();
    file.write("(type ");
    if fixed_state == FixedState::SystemFixed {
        file.write("fix)");
    } else {
        file.write("protect)");
    }
}

fn write_path<W: Write>(
    layer_name: &str,
    width: i32,
    coors: &[i32],
    identifier_type: &IdentifierType,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("path ");
    identifier_type.write(layer_name, file);
    file.write(" ");
    file.write(&width.to_string());
    for pair in coors.chunks_exact(2) {
        file.new_line();
        file.write(&pair[0].to_string());
        file.write(" ");
        file.write(&pair[1].to_string());
    }
    file.end_scope();
}

fn write_conduction_area<W: Write>(
    conduction_id: ItemId,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    let ctx = board.ctx();
    let Some(item) = board.get_item(conduction_id) else {
        return;
    };
    let Item::ConductionArea(conduction_area) = item else {
        return;
    };
    if item.net_count() != 1 {
        return;
    }
    let current_area = conduction_area.get_area(&ctx);
    let layer_index = conduction_area.get_layer();
    let board_layer = &board.layer_structure().layers[layer_index];
    let conduction_layer = DsnLayer::new(
        board_layer.name.clone(),
        i32::try_from(layer_index).unwrap_or(i32::MAX),
        board_layer.is_signal,
    );
    let (boundary_shape, holes): (Shape, Vec<Shape>) = match current_area {
        Area::Shape(s) => (s.clone(), Vec::new()),
        area => (area.get_border(), area.get_holes()),
    };
    file.start_scope_nl();
    file.write("wire ");
    if let Some(dsn_shape) = ct.board_to_dsn_shape(&boundary_shape, conduction_layer.clone()) {
        dsn_shape.write_scope_int(file, identifier_type);
    }
    for hole in &holes {
        if let Some(dsn_hole) = ct.board_to_dsn_shape(hole, conduction_layer.clone()) {
            dsn_hole.write_hole_scope(file, identifier_type);
        }
    }
    file.end_scope();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_name_replaces_every_dsn_occurrence() {
        assert_eq!("board.ses", "board.dsn".replace(".dsn", ".ses"));
        assert_eq!("a.ses.b.ses", "a.dsn.b.dsn".replace(".dsn", ".ses"));
        assert_eq!("tutorial_board", "tutorial_board".replace(".dsn", ".ses"));
    }

    #[test]
    fn fixed_state_is_written_only_above_shove_fixed() {
        fn rendered(state: FixedState) -> String {
            let mut out: Vec<u8> = Vec::new();
            let mut file = IndentFileWriter::new(&mut out);
            write_fixed_state(&mut file, state);
            file.flush().expect("Vec never fails");
            String::from_utf8(out).expect("UTF-8")
        }
        assert_eq!(rendered(FixedState::Unfixed), "");
        assert_eq!(rendered(FixedState::ShoveFixed), "");
        assert_eq!(rendered(FixedState::UserFixed), "\n(type protect)");
        assert_eq!(rendered(FixedState::SystemFixed), "\n(type fix)");
    }
}
