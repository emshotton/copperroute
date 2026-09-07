use std::cmp::Ordering;
use std::collections::HashMap;

use copper_geometry::{FloatLine, FloatPoint};
use copper_router::arena::DoorId;
use copper_router::autoroute::expansion::ExpandableRef;
use copper_router::autoroute::maze::{MazeAdjustment, MazeListElement};

fn test_doors(ids: &[i32]) -> impl Fn(ExpandableRef) -> i32 + use<> {
    let map: HashMap<ExpandableRef, i32> = ids
        .iter()
        .map(|id| (ExpandableRef::Door(DoorId(*id as u32)), *id))
        .collect();
    move |door| *map.get(&door).expect("a TestDoor the test registered")
}

fn element(door: i32, section_no: i32, expansion: f64, sorting: f64) -> MazeListElement {
    MazeListElement {
        door: ExpandableRef::Door(DoorId(door as u32)),
        section_no_of_door: section_no,
        backtrack_door: None,
        section_no_of_backtrack_door: 0,
        expansion_value: expansion,
        sorting_value: sorting,
        next_room: None,
        shape_entry: FloatLine::new(FloatPoint::new(0.0, 0.0), FloatPoint::new(0.0, 0.0)),
        room_ripped: false,
        adjustment: MazeAdjustment::None,
        already_checked: false,
        ripup_cost: 0,
    }
}

/// Reproduces a `TreeSet`'s `add`-with-comparator semantics on a `Vec`: an
/// element that compares equal to one already present is dropped, and the
/// survivors are kept in comparator order (so draining from the front is
/// `poll_first`).
fn insert_by<F>(queue: &mut Vec<MazeListElement>, element: MazeListElement, ids: F) -> bool
where
    F: Fn(ExpandableRef) -> i32 + Copy,
{
    if queue
        .iter()
        .any(|existing| existing.compare_to(&element, ids) == Ordering::Equal)
    {
        return false;
    }
    queue.push(element);
    queue.sort_by(|a, b| a.compare_to(b, ids));
    true
}

fn push_for_test(element: MazeListElement) -> bool {
    use copper_board::prelude::*;
    use copper_geometry::{IntBox, TileShape};
    use copper_router::autoroute::expansion::RoomRef;
    use copper_router::autoroute::maze::{AutorouteControl, AutorouteEngine, MazeQueue};
    use copper_settings::RouterSettings;

    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    let class = rules.net_classes.append("default", &layers(), false);
    rules.nets.add("n1", 1, false, class);
    rules
        .net_classes
        .get_mut(class)
        .set_via_rule(Some(copper_board::ViaRule::new("empty")));
    let mut board = Board::new(
        Vec::new(),
        0,
        IntBox::from_coords(-100_000, -100_000, 100_000, 100_000),
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let settings = RouterSettings::new();
    let mut ctrl = AutorouteControl::from_settings(&board, 1, &settings);
    ctrl.bend_costs = vec![0.0, 0.0];
    ctrl.with_neckdown = false;
    ctrl.trace_half_width = vec![100, 100];
    ctrl.compensated_trace_half_width = vec![100, 100];
    ctrl.via_radii = vec![0.0, 0.0];
    ctrl.trace_clearance_class_index = 1;
    ctrl.min_normal_via_cost = 0.0;
    ctrl.via_rule = None;
    ctrl.max_via_radius = 0.0;
    ctrl.min_cheap_via_cost = 0.0;
    ctrl.start_ripup_costs = 1;
    ctrl.units_per_mm = 1.0;
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let room = engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
        0,
        1,
    );
    let mut element = element;
    element.next_room = Some(RoomRef::Complete(room));
    let mut queue = MazeQueue::new();
    queue.push(element, &ctrl, &engine, &board)
}

#[test]
fn the_four_tie_breaks_are_taken_in_javas_order() {
    let ids = test_doors(&[1, 2]);

    assert_eq!(
        element(1, 0, 9.0, 1.0).compare_to(&element(2, 5, 0.0, 2.0), &ids),
        Ordering::Less
    );
    assert_eq!(
        element(1, 0, 5.0, 1.0).compare_to(&element(2, 5, 4.0, 1.0), &ids),
        Ordering::Greater
    );
    assert_eq!(
        element(1, 7, 1.0, 1.0).compare_to(&element(2, 0, 1.0, 1.0), &ids),
        Ordering::Less
    );
    assert_eq!(
        element(1, 7, 1.0, 1.0).compare_to(&element(1, 0, 1.0, 1.0), &ids),
        Ordering::Greater
    );
    assert_eq!(
        element(1, 3, 1.0, 1.0).compare_to(&element(1, 3, 1.0, 1.0), &ids),
        Ordering::Equal
    );
}

#[test]
fn a_non_finite_sorting_value_is_refused_at_add() {
    let ids = test_doors(&[1, 2]);

    let nan = element(1, 0, 1.0, f64::NAN);
    let number = element(2, 0, 5.0, 1.0);
    assert_eq!(nan.compare_to(&number, &ids), Ordering::Less);
    assert_eq!(number.compare_to(&nan, &ids), Ordering::Greater);

    let a = element(1, 0, 1.0, f64::NAN);
    let b = element(2, 0, 2.0, 5.0);
    let c = element(1, 1, 9.0, 3.0);
    assert_eq!(a.compare_to(&b, &ids), Ordering::Less);
    assert_eq!(b.compare_to(&c, &ids), Ordering::Greater);
    assert_eq!(a.compare_to(&c, &ids), Ordering::Less);
    assert_eq!(
        c.compare_to(&b, &ids),
        Ordering::Less,
        "so A < B, C < B and A < C — and nothing says where A and C sit relative to each other \
         in a way B agrees with"
    );

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            !push_for_test(element(1, 0, 1.0, bad)),
            "a sorting value of {bad} is an upstream bug and is refused"
        );
    }
}

#[test]
fn two_paths_at_the_same_cost_are_both_kept() {
    let ids = test_doors(&[1]);

    let first = element(1, 2, 3.0, 4.0);
    let mut second = element(1, 2, 3.0, 4.0);
    second.room_ripped = true;
    second.ripup_cost = 999;
    second.adjustment = MazeAdjustment::Left;
    second.backtrack_door = Some(ExpandableRef::Door(DoorId(1)));

    assert_eq!(first.sorting_value, second.sorting_value);
    assert_eq!(first.expansion_value, second.expansion_value);
    assert_eq!(first.door, second.door);
    assert_eq!(first.section_no_of_door, second.section_no_of_door);
    assert_ne!(first.compare_to(&second, &ids), Ordering::Equal);
    assert_eq!(
        first.compare_to(&second, &ids).reverse(),
        second.compare_to(&first, &ids)
    );

    let mut queue: Vec<MazeListElement> = Vec::new();
    assert!(insert_by(&mut queue, first.clone(), &ids));
    assert!(
        insert_by(&mut queue, second.clone(), &ids),
        "the jar's TreeSet answers false here and drops the element whole, ripupCost and all"
    );
    assert_eq!(queue.len(), 2, "both paths are in the queue");
    let held: Vec<&MazeListElement> = queue.iter().collect();
    assert!(held.contains(&&first) && held.contains(&&second));

    assert_eq!(first.compare_to(&first, &ids), Ordering::Equal);
    assert!(
        !insert_by(&mut queue, first.clone(), &ids),
        "a genuine duplicate is still a duplicate"
    );
    assert_eq!(queue.len(), 2);

    assert_eq!(
        copper_router::autoroute::expansion::ExpansionDoor::id(1, 63),
        copper_router::autoroute::expansion::ExpansionDoor::id(2, 32),
        "1 * 31 + 63 == 2 * 31 + 32 == 94"
    );
    let ids2 = |door: ExpandableRef| {
        let _ = door;
        94
    };
    let door_a = element(7, 0, 1.0, 1.0);
    let mut door_b = element(8, 0, 1.0, 1.0);
    door_b.section_no_of_door = 0;
    assert_ne!(
        door_a.compare_to(&door_b, ids2),
        Ordering::Equal,
        "two different doors that collide on the hash are still two doors"
    );
}

#[test]
fn pop_first_walks_the_queue_in_sorting_value_order() {
    let ids = test_doors(&[1, 2, 3, 4, 5]);
    let mut queue: Vec<MazeListElement> = Vec::new();
    for (door, sorting) in [(3, 30.0), (1, 10.0), (5, 50.0), (2, 20.0), (4, 40.0)] {
        assert!(insert_by(&mut queue, element(door, 0, 0.0, sorting), &ids));
    }
    assert_eq!(queue.len(), 5);

    let mut popped = Vec::new();
    while !queue.is_empty() {
        popped.push(queue.remove(0).sorting_value);
    }
    assert_eq!(popped, vec![10.0, 20.0, 30.0, 40.0, 50.0]);
    assert_eq!(queue.len(), 0);
    assert!(queue.is_empty());
}

#[test]
fn interleaved_pops_and_pushes_keep_the_tree_sorted() {
    let ids: HashMap<ExpandableRef, i32> = (0..2000)
        .map(|i| (ExpandableRef::Door(DoorId(i)), i as i32))
        .collect();
    let resolve = |door: ExpandableRef| *ids.get(&door).expect("registered");

    let mut queue: Vec<MazeListElement> = Vec::new();
    let mut door = 0u32;
    let mut value = 0.0f64;
    for _ in 0..200 {
        for _ in 0..3 {
            value = (value * 7.0 + 13.0) % 1000.0;
            insert_by(&mut queue, element(door as i32, 0, 0.0, value), resolve);
            door += 1;
        }
        for _ in 0..2 {
            if !queue.is_empty() {
                queue.remove(0);
            }
        }
    }
    let remaining: Vec<f64> = queue.into_iter().map(|e| e.sorting_value).collect();
    assert!(!remaining.is_empty());
    assert!(
        remaining.windows(2).all(|w| w[0] <= w[1]),
        "the survivors come out in ascending sortingValue order: {remaining:?}"
    );
}
