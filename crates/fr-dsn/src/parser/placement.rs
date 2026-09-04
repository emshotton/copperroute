use std::collections::BTreeMap;

use fr_board::{Board, Component, Item, equals_ignore_case};

use crate::error::DsnError;
use crate::format::{java_double_to_string, java_round_to_int};
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::read_string_scope;
use crate::parser::library::write_component_placement_scope;
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemClearanceInfo {
        pub name: String,
        pub clearance_class: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentLocation {
        pub name: String,
                                pub coor: Option<[f64; 2]>,
        pub is_front: bool,
        pub rotation: f64,
        pub position_fixed: bool,
        pub pin_infos: BTreeMap<String, ItemClearanceInfo>,
        pub keepout_infos: BTreeMap<String, ItemClearanceInfo>,
        pub via_keepout_infos: BTreeMap<String, ItemClearanceInfo>,
        pub place_keepout_infos: BTreeMap<String, ItemClearanceInfo>,
                            pub part_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPlacement {
            pub lib_name: String,
            pub locations: Vec<ComponentLocation>,
}

impl ComponentPlacement {
        #[must_use]
    pub fn new(lib_name: String) -> ComponentPlacement {
        ComponentPlacement {
            lib_name,
            locations: Vec::new(),
        }
    }
}



pub fn read_component_placement(
    scanner: &mut DsnScanner,
) -> Result<Option<ComponentPlacement>, DsnError> {
    let Some(Token::Str(name)) = scanner.next_token()? else {
        return Ok(None);
    };
    let mut component_placement = ComponentPlacement::new(name);
    let mut prev_was_open = false;
    let mut next_token = scanner.next_token()?;
    while next_token.is_some() && next_token != Some(Token::Close) {
        if prev_was_open && next_token == Some(Token::Kw(Keyword::Place)) {
            match read_place_scope(scanner)? {
                Some(next_location) => component_placement.locations.push(next_location),
                None => return Ok(None),
            }
        }
        prev_was_open = next_token == Some(Token::Open);
        next_token = scanner.next_token()?;
    }
    Ok(Some(component_placement))
}

pub fn read_component_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let Some(component_placement) = read_component_placement(&mut p.scanner)? else {
        return Ok(false);
    };
    p.placement_list.push(component_placement);
    Ok(true)
}

fn read_place_scope(scanner: &mut DsnScanner) -> Result<Option<ComponentLocation>, DsnError> {
    let mut pin_infos: BTreeMap<String, ItemClearanceInfo> = BTreeMap::new();
    let mut keepout_infos: BTreeMap<String, ItemClearanceInfo> = BTreeMap::new();
    let mut via_keepout_infos: BTreeMap<String, ItemClearanceInfo> = BTreeMap::new();
    let mut place_keepout_infos: BTreeMap<String, ItemClearanceInfo> = BTreeMap::new();

    let name = scanner.next_string_ignoring_newline(true);

    let mut location = [0.0_f64; 2];
    for slot in &mut location {
        #[allow(clippy::cast_precision_loss)]
        match scanner.next_token()? {
            Some(Token::Float(value)) => *slot = value,
            Some(Token::Int(value)) => *slot = value as f64,
            Some(Token::Close) => {
                return Ok(Some(ComponentLocation {
                    name,
                    coor: None,
                    is_front: true,
                    rotation: 0.0,
                    position_fixed: false,
                    pin_infos,
                    keepout_infos,
                    via_keepout_infos,
                    place_keepout_infos,
                    part_number: None,
                }));
            }
            _ => return Ok(None),
        }
    }

    let next_token = scanner.next_token()?;
    let mut is_front = true;
    if next_token == Some(Token::Kw(Keyword::Back)) {
        is_front = false;
    } else if next_token != Some(Token::Kw(Keyword::Front)) {
    }

    #[allow(clippy::cast_precision_loss)]
    let rotation = match scanner.next_token()? {
        Some(Token::Float(value)) => value,
        Some(Token::Int(value)) => value as f64,
        _ => return Ok(None),
    };

    let mut position_fixed = false;
    let mut part_number: Option<String> = None;
    let mut next_token = scanner.next_token()?;
    while next_token == Some(Token::Open) {
        next_token = scanner.next_token()?;
        match &next_token {
            Some(Token::Kw(Keyword::LockType)) => position_fixed = read_lock_type(scanner)?,
            Some(Token::Kw(Keyword::Pin)) => {
                let Some(info) = read_item_clearance_info(scanner)? else {
                    return Ok(None);
                };
                pin_infos.insert(info.name.clone(), info);
            }
            Some(Token::Kw(Keyword::Keepout)) => {
                let Some(info) = read_item_clearance_info(scanner)? else {
                    return Ok(None);
                };
                keepout_infos.insert(info.name.clone(), info);
            }
            Some(Token::Kw(Keyword::ViaKeepout)) => {
                let Some(info) = read_item_clearance_info(scanner)? else {
                    return Ok(None);
                };
                via_keepout_infos.insert(info.name.clone(), info);
            }
            Some(Token::Kw(Keyword::PlaceKeepout)) => {
                let Some(info) = read_item_clearance_info(scanner)? else {
                    return Ok(None);
                };
                place_keepout_infos.insert(info.name.clone(), info);
            }
            Some(Token::Str(s)) if equals_ignore_case("PN", s) => {
                part_number = Some(read_string_scope(scanner)?);
            }
            _ => {
                let _ = skip_scope(scanner)?;
            }
        }
        next_token = scanner.next_token()?;
    }
    if next_token != Some(Token::Close) {
        return Ok(None);
    }
    Ok(Some(ComponentLocation {
        name,
        coor: Some(location),
        is_front,
        rotation,
        position_fixed,
        pin_infos,
        keepout_infos,
        via_keepout_infos,
        place_keepout_infos,
        part_number,
    }))
}

fn read_item_clearance_info(
    scanner: &mut DsnScanner,
) -> Result<Option<ItemClearanceInfo>, DsnError> {
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(name)) = scanner.next_token()? else {
        return Ok(None);
    };
    let mut cl_class_name: Option<String> = None;
    let mut next_token = scanner.next_token()?;
    while next_token == Some(Token::Open) {
        next_token = scanner.next_token()?;
        let is_clearance_class = match &next_token {
            Some(Token::Kw(Keyword::ClearanceClass)) => true,
            Some(Token::Str(s)) => {
                equals_ignore_case("clearance_class", s) || equals_ignore_case("clearanceClass", s)
            }
            _ => false,
        };
        if is_clearance_class {
            cl_class_name = Some(read_string_scope(scanner)?);
        } else {
            let _ = skip_scope(scanner)?;
        }
        next_token = scanner.next_token()?;
    }
    if next_token != Some(Token::Close) {
        return Ok(None);
    }
    let Some(clearance_class) = cl_class_name else {
        return Ok(None);
    };
    Ok(Some(ItemClearanceInfo {
        name,
        clearance_class,
    }))
}

fn read_lock_type(scanner: &mut DsnScanner) -> Result<bool, DsnError> {
    let mut result = false;
    loop {
        match scanner.next_token()? {
            Some(Token::Close) | None => break,
            Some(Token::Kw(Keyword::Position)) => result = true,
            _ => {}
        }
    }
    Ok(result)
}


pub fn write_placement_scope(p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("placement");
    if p.board.components.get_flip_style_rotate_first() {
        p.file.new_line();
        p.file.write("(place_control (flip_style rotate_first))");
    }
    for i in 1..=p.board.library.packages.count() {
        write_component_placement_scope(p, i);
    }
    p.file.end_scope();
}

pub fn write_component_scope(p: &mut WriteScopeParameter<'_>, component: &Component) {
    let board = p.board;
    p.file.start_scope_nl();
    p.file.write("place ");
    p.file.new_line();
    p.identifier_type.write(&component.name, &mut p.file);
    if component.is_placed() {
        let location = component.get_location().expect("isPlaced").to_float();
        let coor = p.coordinate_transform.board_to_dsn_point(&location);
        for value in coor {
            p.file.write(" ");
            p.file.write(&java_double_to_string(value));
        }
        if component.placed_on_front() {
            p.file.write(" front ");
        } else {
            p.file.write(" back ");
        }
        let rotation = java_round_to_int(component.get_rotation_in_degree());
        p.file.write(&rotation.to_string());
    }
    if component.position_fixed {
        p.file.new_line();
        p.file.write(" (lock_type position)");
    }
    let pin_count = board
        .library
        .packages
        .get(component.get_package())
        .pin_count();
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    for i in 0..pin_count as i32 {
        write_pin_info(p, component, i);
    }
    write_keepout_infos(p, component);
    p.file.end_scope();
}

fn write_pin_info(p: &mut WriteScopeParameter<'_>, component: &Component, pin_index: i32) {
    if !component.is_placed() {
        return;
    }
    let board = p.board;
    let Some(package_pin) = board
        .library
        .packages
        .get(component.get_package())
        .get_pin(pin_index)
    else {
        return;
    };
    let Some(component_pin) = board
        .get_pin(component.id, pin_index)
        .and_then(|id| board.get_item(id))
    else {
        return;
    };
    let Some(cl_class_name) = board
        .rules
        .clearance_matrix
        .get_name(component_pin.clearance_class())
    else {
        return;
    };
    let pin_name = package_pin.name.clone();
    let cl_class_name = cl_class_name.to_string();
    p.file.new_line();
    p.file.write("(pin ");
    p.identifier_type.write(&pin_name, &mut p.file);
    p.file.write(" (clearance_class ");
    p.identifier_type.write(&cl_class_name, &mut p.file);
    p.file.write("))");
}

fn write_keepout_infos(p: &mut WriteScopeParameter<'_>, component: &Component) {
    if !component.is_placed() {
        return;
    }
    let board = p.board;
    let board_package = board.library.packages.get(component.get_package());
    for j in 0..3 {
        let (current_keepout_arr, keepout_type) = match j {
            0 => (&board_package.keepouts, "(keepout "),
            1 => (&board_package.via_keepouts, "(via_keepout "),
            _ => (&board_package.place_keepouts, "(place_keepout "),
        };
        for current_keepout in current_keepout_arr {
            let Some(current_obstacle_area) =
                get_keepout(board, component.id, &current_keepout.name)
            else {
                continue;
            };
            if current_obstacle_area.clearance_class() == 0 {
                continue;
            }
            let Some(cl_class_name) = board
                .rules
                .clearance_matrix
                .get_name(current_obstacle_area.clearance_class())
            else {
                return;
            };
            let keepout_name = current_keepout.name.clone();
            let cl_class_name = cl_class_name.to_string();
            p.file.new_line();
            p.file.write(keepout_type);
            p.identifier_type.write(&keepout_name, &mut p.file);
            p.file.write(" (clearance_class ");
            p.identifier_type.write(&cl_class_name, &mut p.file);
            p.file.write("))");
        }
    }
}

fn get_keepout<'a>(board: &'a Board, component_id: i32, name: &str) -> Option<&'a Item> {
    board.get_items().find(|item| {
        item.component_id() == component_id
            && item.is_obstacle_area()
            && obstacle_area_name(item) == Some(name)
    })
}

fn obstacle_area_name(item: &Item) -> Option<&str> {
    match item {
        Item::ObstacleArea(a) => a.name(),
        Item::ConductionArea(a) => a.name(),
        Item::ViaObstacleArea(a) => a.name(),
        Item::ComponentObstacleArea(a) => a.name(),
        _ => {
            debug_assert!(
                !item.is_obstacle_area(),
                "an ObstacleArea variant is missing from obstacle_area_name; get_keepout would \
                 silently stop finding its keepouts"
            );
            None
        }
    }
}
