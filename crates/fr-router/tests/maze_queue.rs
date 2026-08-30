//! Plan 6 Task 8: the guarded queue — the anonymous `TreeSet<MazeListElement>` subclass
//! `MazeSearchEngine` installs at `MazeSearchEngine.java:84-125`.
//!
//! The guard is 35 lines of straight-line arithmetic over `ctrl` and the board's resolution, with
//! no Java-side state a probe could disagree about; every literal below is derived from the Java
//! text with its line number, and the *container* half (ordering, the tie-drop, pop-first) is
//! pinned separately in `tests/maze_list_element.rs` against `JavaTreeSet`.

use fr_board::prelude::*;
use fr_board::structure::Unit;
use fr_geometry::{FloatLine, FloatPoint, IntBox, IntPoint, Point, TileShape};
use fr_router::arena::DoorId;
use fr_router::autoroute::expansion::{ExpandableRef, RoomRef};
use fr_router::autoroute::maze::{
    AutorouteControl, AutorouteEngine, MazeAdjustment, MazeListElement, MazeQueue,
};
use fr_router::{DrillId, ExpansionDrill};
use fr_settings::RouterSettings;

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -100_000,
        y: -100_000,
    },
    ur: IntPoint {
        x: 100_000,
        y: 100_000,
    },
};

/// Two signal layers, nothing on the board, one net so `initNet` can read a half width.
fn bare_board() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    let class = rules.net_classes.append("default", &layers(), false);
    rules.nets.add("n1", 1, false, class);
    Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

/// A control block with the fanout fields the guard reads already set: `isFanout`, the start pin
/// centre and its layer. `viaRule` is left `None`, which is why this does not go through
/// `AutorouteControl::new` — the guard never touches a via.
fn fanout_control(
    settings: &RouterSettings,
    pin_center: Point,
    pin_layer: i32,
) -> AutorouteControl {
    let mut ctrl = fresh_control(settings);
    ctrl.is_fanout = true; // :87
    ctrl.fanout_start_pin_center = Some(pin_center); // :87-89
    ctrl.fanout_start_pin_layer = pin_layer; // :92
    ctrl
}

/// A control block with every field at the private constructor's default, built without a board
/// so the test can set exactly the four fields the guard reads.
fn fresh_control(settings: &RouterSettings) -> AutorouteControl {
    AutorouteControl {
        trace_costs: settings.get_trace_costs(),
        bend_costs: vec![0.0, 0.0],
        with_neckdown: false,
        layer_active: vec![true, true],
        layer_count: 2,
        trace_half_width: vec![100, 100],
        compensated_trace_half_width: vec![100, 100],
        via_radii: vec![0.0, 0.0],
        add_via_costs: vec![vec![0, 0], vec![0, 0]],
        trace_clearance_class_index: 1,
        vias_allowed: true,
        attach_smd_allowed: false,
        min_normal_via_cost: 0.0,
        ripup_allowed: false,
        ripup_costs: 1000,
        ripup_pass_no: 1,
        is_fanout: false,
        fanout_start_pin_name: None,
        fanout_start_pin_center: None,
        fanout_start_pin_layer: -1,
        remove_unconnected_vias: true,
        via_rule: None,
        net_number: 1,
        via_clearance_class: 1,
        via_infos: Vec::new(),
        via_lower_bound: 0,
        via_upper_bound: 2,
        max_via_radius: 0.0,
        tidy_region_width: i32::MAX,
        pull_tight_accuracy: 500,
        max_shove_trace_recursion_depth: 20,
        max_shove_via_recursion_depth: 5,
        max_spring_over_recursion_depth: 5,
        min_cheap_via_cost: 0.0,
        fanout_max_escape_length: 3000.0,
        fanout_min_escape_length: 500.0,
        // `RouterSettings.getStartRipupCosts`'s default (RouterSettings.java:537-548).
        start_ripup_costs: 1,
    }
}

fn element(
    door: ExpandableRef,
    next_room: Option<RoomRef>,
    entry: FloatPoint,
    sorting: f64,
) -> MazeListElement {
    MazeListElement {
        door,
        section_no_of_door: 0,
        backtrack_door: None,
        section_no_of_backtrack_door: 0,
        expansion_value: 0.0,
        sorting_value: sorting,
        next_room,
        // `shapeEntry.a.middlePoint(shapeEntry.b)` (:102-103) is the point the guard measures, so
        // a degenerate line puts it exactly at `entry`.
        shape_entry: FloatLine::new(entry, entry),
        room_ripped: false,
        adjustment: MazeAdjustment::None,
        already_checked: false,
        ripup_cost: 0,
    }
}

// =================================================================================================
// The guard
// =================================================================================================

/// `:87` — with `isFanout` false, neither window applies, however far away the element is.
#[test]
fn without_fanout_the_window_is_not_consulted_at_all() {
    let mut board = bare_board();
    let settings = RouterSettings::new();
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let room = engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
        0,
        1,
    );
    let ctrl = fresh_control(&settings);

    let mut queue = MazeQueue::new();
    assert!(queue.push(
        element(
            ExpandableRef::Door(DoorId(0)),
            Some(RoomRef::Complete(room)),
            FloatPoint::new(90_000.0, 90_000.0),
            1.0,
        ),
        &ctrl,
        &engine,
        &board,
    ));
    assert_eq!(queue.len(), 1);
}

/// `:90-106` — an element whose `nextRoom` is on the fanout start layer is refused when the
/// middle of its shape entry is further than `maxLen * resolution` from the start pin.
#[test]
fn the_max_escape_window_refuses_a_far_entry_on_the_start_layer() {
    let mut board = bare_board();
    let settings = RouterSettings::new();
    let resolution = board.communication.get_resolution(Unit::Um);
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let on_layer_0 = engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
        0,
        1,
    );
    let on_layer_1 = engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
        1,
        2,
    );
    let ctrl = fanout_control(&settings, Point::Int(IntPoint::new(0, 0)), 0);
    let limit = ctrl.fanout_max_escape_length * resolution;

    let mut queue = MazeQueue::new();
    // Just inside the window: accepted.
    assert!(queue.push(
        element(
            ExpandableRef::Door(DoorId(0)),
            Some(RoomRef::Complete(on_layer_0)),
            FloatPoint::new(limit - 1.0, 0.0),
            1.0,
        ),
        &ctrl,
        &engine,
        &board,
    ));
    // Exactly at it: `dist > maxLen * resolution` is a strict `>` (:104), so this is accepted too.
    assert!(queue.push(
        element(
            ExpandableRef::Door(DoorId(1)),
            Some(RoomRef::Complete(on_layer_0)),
            FloatPoint::new(limit, 0.0),
            2.0,
        ),
        &ctrl,
        &engine,
        &board,
    ));
    // Past it: refused.
    assert!(!queue.push(
        element(
            ExpandableRef::Door(DoorId(2)),
            Some(RoomRef::Complete(on_layer_0)),
            FloatPoint::new(limit + 1.0, 0.0),
            3.0,
        ),
        &ctrl,
        &engine,
        &board,
    ));
    // The same distance on another layer is not measured at all (:90-92).
    assert!(queue.push(
        element(
            ExpandableRef::Door(DoorId(3)),
            Some(RoomRef::Complete(on_layer_1)),
            FloatPoint::new(limit + 1.0, 0.0),
            4.0,
        ),
        &ctrl,
        &engine,
        &board,
    ));
    // And a `null` nextRoom is not on any layer (`element.nextRoom != null &&`, :91).
    assert!(queue.push(
        element(
            ExpandableRef::Door(DoorId(4)),
            None,
            FloatPoint::new(limit + 1.0, 0.0),
            5.0,
        ),
        &ctrl,
        &engine,
        &board,
    ));
    assert_eq!(queue.len(), 4);
}

/// `:108-122` — a drill element is refused when it is **closer** than `minLen * resolution` to
/// the start pin, on any layer, and the test is a strict `<` (:119).
#[test]
fn the_min_escape_window_refuses_a_near_drill() {
    let mut board = bare_board();
    let settings = RouterSettings::new();
    let resolution = board.communication.get_resolution(Unit::Um);
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let ctrl = fanout_control(&settings, Point::Int(IntPoint::new(0, 0)), 0);
    let floor = ctrl.fanout_min_escape_length * resolution;

    let near = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(-10, -10, 10, 10)),
        Point::Int(IntPoint::new(floor as i32 - 1, 0)),
        0,
        1,
    );
    let far = ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(-10, -10, 10, 10)),
        Point::Int(IntPoint::new(floor as i32 + 1, 0)),
        0,
        1,
    );
    let near = engine.rooms.drills.insert(near);
    let far = engine.rooms.drills.insert(far);

    let mut queue = MazeQueue::new();
    assert!(!queue.push(
        element(
            ExpandableRef::Drill(DrillId(near)),
            None,
            FloatPoint::new(0.0, 0.0),
            1.0,
        ),
        &ctrl,
        &engine,
        &board,
    ));
    assert!(queue.push(
        element(
            ExpandableRef::Drill(DrillId(far)),
            None,
            FloatPoint::new(0.0, 0.0),
            2.0,
        ),
        &ctrl,
        &engine,
        &board,
    ));
    assert_eq!(queue.len(), 1);
}

/// The two windows are **both** applied to the same element (`:93-107` then `:108-122`, not an
/// `else`): a drill on the start layer has to clear the max window and the min window.
#[test]
fn both_windows_apply_to_a_drill_on_the_start_layer() {
    let mut board = bare_board();
    let settings = RouterSettings::new();
    let resolution = board.communication.get_resolution(Unit::Um);
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let room = engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
        0,
        1,
    );
    let ctrl = fanout_control(&settings, Point::Int(IntPoint::new(0, 0)), 0);
    let floor = ctrl.fanout_min_escape_length * resolution;

    // A drill comfortably inside the max window but under the min one.
    let drill = engine.rooms.drills.insert(ExpansionDrill::new(
        TileShape::Box(IntBox::from_coords(-10, -10, 10, 10)),
        Point::Int(IntPoint::new(floor as i32 - 1, 0)),
        0,
        1,
    ));
    let mut queue = MazeQueue::new();
    assert!(!queue.push(
        element(
            ExpandableRef::Drill(DrillId(drill)),
            Some(RoomRef::Complete(room)),
            FloatPoint::new(1.0, 0.0),
            1.0,
        ),
        &ctrl,
        &engine,
        &board,
    ));
    assert!(queue.is_empty());
}

// =================================================================================================
// The container half, through the queue's own API
// =================================================================================================

/// `push` resolves `door.getId()` through the engine, so two doors between different rooms sort
/// by the ids the engine answers — and `pop_first` walks them in that order.
#[test]
fn the_queue_sorts_and_pops_through_the_engines_door_ids() {
    let mut board = bare_board();
    let settings = RouterSettings::new();
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let ctrl = fresh_control(&settings);

    let a = engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
        0,
        1,
    );
    let b = engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(10, 0, 20, 10))),
        0,
        2,
    );
    let c = engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(20, 0, 30, 10))),
        0,
        3,
    );
    let ab = engine
        .rooms
        .new_door(RoomRef::Complete(a), RoomRef::Complete(b), 1);
    let bc = engine
        .rooms
        .new_door(RoomRef::Complete(b), RoomRef::Complete(c), 1);
    // `ExpansionDoor.getId` = `min * 31 + max` over the two room ids: 1*31+2 = 33, 2*31+3 = 65.
    assert_eq!(engine.expandable_id_no(ExpandableRef::Door(ab)), 33);
    assert_eq!(engine.expandable_id_no(ExpandableRef::Door(bc)), 65);

    let mut queue = MazeQueue::new();
    // Equal costs, so the door id is the whole order — and the *larger* id goes in first.
    assert!(queue.push(
        element(
            ExpandableRef::Door(bc),
            None,
            FloatPoint::new(0.0, 0.0),
            1.0
        ),
        &ctrl,
        &engine,
        &board,
    ));
    assert!(queue.push(
        element(
            ExpandableRef::Door(ab),
            None,
            FloatPoint::new(0.0, 0.0),
            1.0
        ),
        &ctrl,
        &engine,
        &board,
    ));

    let first = queue.pop_first().expect("two elements");
    assert_eq!(first.door, ExpandableRef::Door(ab));
    let second = queue.pop_first().expect("one element");
    assert_eq!(second.door, ExpandableRef::Door(bc));
    assert!(queue.pop_first().is_none());
    assert!(queue.is_empty());
}
