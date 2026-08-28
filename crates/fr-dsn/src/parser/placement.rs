//! `io/specctra/parser/{Placement,Component,ComponentPlacement}.java` — the `placement` scope
//! and, nested inside it, one `component` scope per library component.

use std::collections::BTreeMap;

use fr_board::{Board, Component, Item, equals_ignore_case};

use crate::error::DsnError;
use crate::format::{java_double_to_string, java_round_to_int};
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::read_string_scope;
use crate::parser::library::write_component_placement_scope;
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};

// ------------------------------------------------------------------ ComponentPlacement.java

/// `ComponentPlacement.ItemClearanceInfo` (ComponentPlacement.java:82-91): one `(pin …)`,
/// `(keepout …)`, `(via_keepout …)` or `(place_keepout …)` clearance-class override inside a
/// `place` scope.
// renamed: the nested `ComponentPlacement.ItemClearanceInfo(String, String)` constructor
// (ComponentPlacement.java:87-90) -> this struct's literal; it is package-private in Java and
// has no behaviour beyond the two assignments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemClearanceInfo {
    /// `ItemClearanceInfo.name` (ComponentPlacement.java:84) — also the map key.
    pub name: String,
    /// `ItemClearanceInfo.clearanceClass` (ComponentPlacement.java:85).
    pub clearance_class: String,
}

/// `ComponentPlacement.ComponentLocation` (ComponentPlacement.java:24-80): "the structure of an
/// entry in the list locations".
///
/// The four maps are Java `TreeMap`s (Component.java:189-192), so they are [`BTreeMap`]s here,
/// not `HashMap`s: `Network.insertComponent` walks them in key order and the order reaches the
/// board.
// renamed: the nested `ComponentPlacement.ComponentLocation(...)` constructor
// (ComponentPlacement.java:58-79) -> this struct's literal.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentLocation {
    /// `ComponentLocation.name` (ComponentPlacement.java:26).
    pub name: String,
    /// `ComponentLocation.coor` (ComponentPlacement.java:29), "the x- and y-coordinates of the
    /// location". `None` is Java's `null`: the component is not yet placed
    /// (Component.java:204-216), which `Network.insertComponent` (Network.java:949-955) passes
    /// straight through to `Components.add` as a `null` location.
    ///
    /// Still `f64` here, exactly as Java's `double[]`: the rounding to board coordinates happens
    /// in `Network.insertComponent` (Task 9), not at read time.
    pub coor: Option<[f64; 2]>,
    /// `ComponentLocation.isFront` (ComponentPlacement.java:35).
    pub is_front: bool,
    /// `ComponentLocation.rotation` (ComponentPlacement.java:38), in degrees.
    pub rotation: f64,
    /// `ComponentLocation.positionFixed` (ComponentPlacement.java:41).
    pub position_fixed: bool,
    /// `ComponentLocation.pin_infos` (ComponentPlacement.java:45), keyed by pin name.
    pub pin_infos: BTreeMap<String, ItemClearanceInfo>,
    /// `ComponentLocation.keepout_infos` (ComponentPlacement.java:48).
    pub keepout_infos: BTreeMap<String, ItemClearanceInfo>,
    /// `ComponentLocation.via_keepout_infos` (ComponentPlacement.java:51).
    pub via_keepout_infos: BTreeMap<String, ItemClearanceInfo>,
    /// `ComponentLocation.place_keepout_infos` (ComponentPlacement.java:54).
    pub place_keepout_infos: BTreeMap<String, ItemClearanceInfo>,
    /// `ComponentLocation.partNumber` (ComponentPlacement.java:56) — the `(PN …)` sub-scope.
    ///
    /// Java wins over the Task 8 brief here, which called this field `logical_part`: the field
    /// is `partNumber`, it is filled from `(PN …)` (Component.java:281-283) and it reaches
    /// `Components.add`'s `partNumber` parameter. The DSN `logical_part` scope is a different
    /// thing entirely and lives in `part_library.rs`.
    pub part_number: Option<String>,
}

/// `io/specctra/parser/ComponentPlacement.java`: placement data for one library component
/// (`libName` plus a list of `ComponentLocation`s).
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPlacement {
    /// `ComponentPlacement.libName` (ComponentPlacement.java:12): the name of the corresponding
    /// library component.
    pub lib_name: String,
    /// `ComponentPlacement.locations` (ComponentPlacement.java:15), a `LinkedList` — insertion
    /// order, so a `Vec`.
    pub locations: Vec<ComponentLocation>,
}

impl ComponentPlacement {
    /// `ComponentPlacement(String)` (ComponentPlacement.java:18-21).
    #[must_use]
    pub fn new(lib_name: String) -> ComponentPlacement {
        ComponentPlacement {
            lib_name,
            locations: Vec::new(),
        }
    }
}

// ------------------------------------------------------------------------- Component.java

// Fix round 1 (Task 6): there is no `read_placement_scope` here, and none is dispatched to —
// verified by reading `Placement.java` in full: it has only a constructor and `writeScope`, no
// `readScope` override at all, so `ScopeKeyword::Placement` dispatches straight to the generic
// `read_scope_generic` loop (`parser/scope_parameter.rs`), the same as `ScopeKeyword::Pcb`, and
// that loop is what finds `Component`'s own override for each nested `(component …)`.

/// `Component.readScope(IJFlexScanner)` (Component.java:29-54) — the overload that does the
/// actual parsing, and the one `SesReader` reuses ("used also when reading a session file").
/// `None` is Java's `null`.
// renamed: Component.readScope(IJFlexScanner) -> read_component_placement (Rust has no
// overloading, and the sibling `ReadScopeParameter` override below keeps the dispatch-table
// name `read_component_scope`).
pub fn read_component_placement(
    scanner: &mut DsnScanner,
) -> Result<Option<ComponentPlacement>, DsnError> {
    let Some(Token::Str(name)) = scanner.next_token()? else {
        // "component name expected" (Component.java:31-37).
        return Ok(None);
    };
    let mut component_placement = ComponentPlacement::new(name);
    // Java's `prevToken` starts as the component name, which is never `(`.
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

/// `Component.readScope(ReadScopeParameter)` (Component.java:366-380), the `ScopeKeyword`
/// override: parse one `(component …)` scope and append it to `ReadScopeParameter.placementList`.
/// **Nothing reaches the board here** — `Network.insertComponents` (Task 9) drains the list.
// renamed: Component.readScope(ReadScopeParameter) -> read_component_scope.
pub fn read_component_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let Some(component_placement) = read_component_placement(&mut p.scanner)? else {
        return Ok(false);
    };
    p.placement_list.push(component_placement);
    Ok(true)
}

/// `Component.readPlaceScope` (Component.java:187-309): one `(place <name> [x y side rotation]
/// …)` scope. `None` is Java's `null`.
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
                // component is not yet placed (Component.java:204-216)
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
            // "Double was expected as the second and third parameter of the component/place
            // command" (Component.java:217-224).
            _ => return Ok(None),
        }
    }

    let next_token = scanner.next_token()?;
    let mut is_front = true;
    if next_token == Some(Token::Kw(Keyword::Back)) {
        is_front = false;
    } else if next_token != Some(Token::Kw(Keyword::Front)) {
        // "Keyword.FRONT expected" (Component.java:231-236) — a warning only: Java carries on
        // with `isFront` still `true` and does *not* consume another token.
    }

    #[allow(clippy::cast_precision_loss)]
    let rotation = match scanner.next_token()? {
        Some(Token::Float(value)) => value,
        Some(Token::Int(value)) => value as f64,
        // "number expected" (Component.java:243-249).
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
            // Java bug: Component.readPlaceScope tests `nextToken == Keyword.PN ||
            // (nextToken instanceof String && "PN".equalsIgnoreCase(...))` (Component.java:
            // 281-282). The first half is dead: `Keyword.PN` exists as a singleton but `"PN"`
            // is not a lexeme of the JFlex DFA, so `nextToken` can never *be* that instance —
            // the string half is the only one that ever fires, and it is what this port keeps
            // (see the `Keyword::PN` note in `keyword.rs`, which is why there is no `Pn`
            // variant to compare against). Reproduced as written: same accepted inputs, same
            // rejected ones. See `docs/java-quirks.md`.
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
        // ") expected" (Component.java:289-293).
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

/// `Component.readItemClearanceInfo` (Component.java:311-350): the `<name> (clearance_class …)`
/// body shared by the `pin`, `keepout`, `via_keepout` and `place_keepout` sub-scopes.
fn read_item_clearance_info(
    scanner: &mut DsnScanner,
) -> Result<Option<ItemClearanceInfo>, DsnError> {
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(name)) = scanner.next_token()? else {
        // "String expected" (Component.java:316-321).
        return Ok(None);
    };
    let mut cl_class_name: Option<String> = None;
    let mut next_token = scanner.next_token()?;
    while next_token == Some(Token::Open) {
        next_token = scanner.next_token()?;
        // Component.java:326-328 accepts the keyword **and** both string spellings: the DFA
        // lexes `clearance_class` to `Keyword.CLEARANCE_CLASS`, but HEAD's own writer emitted
        // `clearanceClass` for a while (plan ruling 1), so Java added a compatibility shim on
        // read. Ported as written — the *writer* still emits only `clearance_class`.
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
        // ") expected" (Component.java:335-341).
        return Ok(None);
    }
    let Some(clearance_class) = cl_class_name else {
        // "clearance class name not found" (Component.java:342-348).
        return Ok(None);
    };
    Ok(Some(ItemClearanceInfo {
        name,
        clearance_class,
    }))
}

/// `Component.readLockType` (Component.java:352-364): `true` iff the `(lock_type …)` scope
/// mentions `position`.
///
// totalized: Component.readLockType — Java's `for (;;)` breaks only on `CLOSED_BRACKET`, and
// `nextToken()` answers `null` for ever once the input is exhausted, so a file truncated inside
// a `(lock_type` scope spins the reader thread for ever. The port breaks on end of file and
// returns whatever it has seen, which is the value Java would have returned had the bracket been
// there. No reachable caller observes the difference on a well-formed file.
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

// ------------------------------------------------------------------------- Placement.java

/// `Placement.writeScope` (Placement.java:14-28): the `(placement …)` scope — the optional
/// `place_control` line, then one `(component …)` scope per library package that has components.
// renamed: Placement.writeScope -> write_placement_scope.
pub fn write_placement_scope(p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("placement");
    if p.board.components.get_flip_style_rotate_first() {
        p.file.new_line();
        p.file.write("(place_control (flip_style rotate_first))");
    }
    // Java guards the loop with `library.packages != null`, on a field this port makes
    // non-nullable (as `Library.writeScope` does).
    for i in 1..=p.board.library.packages.count() {
        write_component_placement_scope(p, i);
    }
    p.file.end_scope();
}

/// `Component.writeScope` (Component.java:56-88): one `(place …)` scope inside a `(component …)`.
// renamed: Component.writeScope -> write_component_scope.
pub fn write_component_scope(p: &mut WriteScopeParameter<'_>, component: &Component) {
    let board = p.board;
    p.file.start_scope_nl();
    p.file.write("place ");
    p.file.new_line();
    p.identifier_type.write(&component.name, &mut p.file);
    if component.is_placed() {
        // `isPlaced()` is exactly `location != null`, so the `unwrap` below cannot fire.
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

/// `Component.writePinInfo` (Component.java:90-122): one `(pin <name> (clearance_class <name>))`
/// line per package pin, skipped entirely for an unplaced component.
fn write_pin_info(p: &mut WriteScopeParameter<'_>, component: &Component, pin_index: i32) {
    if !component.is_placed() {
        return;
    }
    let board = p.board;
    // "package pin not found" (Component.java:99-102).
    let Some(package_pin) = board
        .library
        .packages
        .get(component.get_package())
        .get_pin(pin_index)
    else {
        return;
    };
    // "component pin not found" (Component.java:103-108).
    let Some(component_pin) = board
        .get_pin(component.id, pin_index)
        .and_then(|id| board.get_item(id))
    else {
        return;
    };
    // "clearance class name not found" (Component.java:109-115).
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

/// `Component.writeKeepoutInfos` (Component.java:124-168): the `(keepout …)`, `(via_keepout …)`
/// and `(place_keepout …)` clearance-class overrides, in that order (Java's `j` loop).
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
            // "clearance class name not found" (Component.java:153-159) — Java `return`s here,
            // abandoning the remaining keepouts of *both* remaining kinds, not just this one.
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

/// `Component.getKeepout` (Component.java:170-185): the first obstacle area in board-item order
/// that belongs to `component_id` and carries `name`.
///
/// `instanceof ObstacleArea` covers the four area variants Java derives from it
/// ([`Item::is_obstacle_area`]) — `ComponentOutline` and `BoardOutline` extend `Item` directly
/// and are therefore *not* matched. `Board::get_items` iterates in Java's `itemList` order
/// (descending id, quirk #63), which is the order Java's `startReadObject` walk uses.
fn get_keepout<'a>(board: &'a Board, component_id: i32, name: &str) -> Option<&'a Item> {
    board.get_items().find(|item| {
        item.component_id() == component_id
            && item.is_obstacle_area()
            && obstacle_area_name(item) == Some(name)
    })
}

/// `ObstacleArea.name` reached through the `Item` enum — Java gets it from the shared base class.
fn obstacle_area_name(item: &Item) -> Option<&str> {
    match item {
        Item::ObstacleArea(a) => a.name(),
        Item::ConductionArea(a) => a.name(),
        Item::ViaObstacleArea(a) => a.name(),
        Item::ComponentObstacleArea(a) => a.name(),
        _ => None,
    }
}
