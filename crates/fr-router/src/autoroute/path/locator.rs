//! Port of `autoroute.path.FoundConnectionLocator` (FoundConnectionLocator.java:30-569): the
//! backtrack walk that turns a found [`MazeResult`] into the list of items to insert.
//!
//! # The three regimes, and why there are only two implementations
//!
//! `getInstance` (`:185-207`) builds a `FoundConnectionLocator45Degree` for **both**
//! `NINETY_DEGREE` and `FORTYFIVE_DEGREE`, and a `FoundConnectionLocatorAnyAngle` for everything
//! else. The two 90°/45° runs differ only inside `calculateAdditionalCorner` (`:390-404`), which
//! dispatches on the `angleRestriction` *value* rather than on the class — so the same subclass
//! yields different corner lists for the two regimes. [`P6T14Probe`'s mode `share`] pins the
//! class dispatch and its mode `locate` pins all three corner lists over one and the same maze
//! result.
//!
//! Java's abstract class plus two subclasses is one `LocatorWalk` here with a
//! [`LocatorKind`] discriminant, because the only virtual member is
//! `calculateNextTraceCorners` (`:499`) and the walk mutates the engine's door arena while it
//! runs — a `&mut dyn` would need the arena's lifetime. The two overrides live in
//! `super::locator_45` and `super::locator_any_angle`, which is what
//! `scripts/audit-map/fr-router.map` points the two subclasses at.
//!
//! # Reference identity is load-bearing
//!
//! `calculateNextTrace` (`:432`) drops a corner with `currentNextCorner != prevCorner` — Java's
//! **reference** comparison, not `equals`. `FoundConnectionLocatorAnyAngle` returns
//! `this.currentFromPoint` *itself* at `:101` and `:184` (the "door completely passed" advance)
//! and `rightTurnNextCorner`/`leftTurnNextCorner` return their `fromCorner` argument on a null
//! tangential point (`:371`, `:397`), so the test is a real filter and a value comparison would
//! keep corners Java drops. The port models a Java reference as a `u64` token: every freshly
//! constructed `FloatPoint` gets one from `LocatorWalk::new_corner_id`, and returning
//! `currentFromPoint` reuses `LocatorWalk::current_from_id`.
//!
//! [`P6T14Probe`'s mode `share`]: `scripts/differential/java/probes/P6T14Probe.java`

use std::collections::{BTreeMap, BTreeSet};

use fr_board::structure::AngleRestriction;
use fr_board::{Board, Item, ItemId, TreeId};
use fr_geometry::{FloatPoint, IntPoint, TileShape};

use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::maze::{AutorouteControl, AutorouteEngine, MazeResult};

/// Port of the nested `FoundConnectionLocator.ResultItem` (`:542-551`): "type of a single item in
/// the result list connectionItems. Used to create a new PolylineTrace."
///
/// It is a **trace only** — a corner list and a layer. The brief's `ConnectionItem` enum with a
/// `Via` variant has no counterpart in Java: `FoundConnectionInserter` (`:47-75`) derives every
/// via from the *layer change between two consecutive `ResultItem`s*, so a via is never an entry
/// of `connectionItems`. See the module docs of [`super`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultItem {
    /// `public final IntPoint[] corners` (`:544`).
    pub corners: Vec<IntPoint>,
    /// `public final int layer` (`:545`).
    pub layer: usize,
}

impl ResultItem {
    /// Port of the constructor `ResultItem(IntPoint[], int)` (`:547-550`).
    ///
    /// renamed: `FoundConnectionLocator.ResultItem` — Java's nested-class *constructor* is
    /// `ResultItem::new` here. `audit-port.sh` reads a nested class's constructor as a public
    /// method of the enclosing class, so this is the row that closes it.
    pub fn new(corners: Vec<IntPoint>, layer: usize) -> ResultItem {
        ResultItem { corners, layer }
    }
}

/// Port of the nested `FoundConnectionLocator.BacktrackElement` (`:557-568`): "type of the
/// elements of the list returned by this.backtrack(). Next_room is the common room of the current
/// door and the next door in the backtrack list."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BacktrackElement {
    /// `public final ExpandableObject door` (`:559`).
    pub door: ExpandableRef,
    /// `public final int sectionNoOfDoor` (`:560`).
    pub section_no_of_door: i32,
    /// `public final CompleteExpansionRoom nextRoom` (`:561`); `None` is Java's `null`.
    pub next_room: Option<RoomRef>,
}

/// Which of Java's two concrete subclasses `getInstance` (`:196-205`) picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocatorKind {
    /// `FoundConnectionLocator45Degree` — **both** `NINETY_DEGREE` and `FORTYFIVE_DEGREE`.
    FortyFiveDegree,
    /// `FoundConnectionLocatorAnyAngle` — every other angle restriction.
    AnyAngle,
}

impl LocatorKind {
    /// The class `getInstance` (`:196-205`) constructs for this angle restriction.
    pub fn of(angle_restriction: AngleRestriction) -> LocatorKind {
        // :196-197. Note this is the one place in the plan where `NINETY_DEGREE` and
        // `FORTYFIVE_DEGREE` share an arm; Task 3's search-tree dispatch keeps them apart.
        if angle_restriction == AngleRestriction::NinetyDegree
            || angle_restriction == AngleRestriction::FortyFiveDegree
        {
            LocatorKind::FortyFiveDegree
        } else {
            LocatorKind::AnyAngle
        }
    }
}

/// Port of `path.FoundConnectionLocator` (FoundConnectionLocator.java:30-569): walks the maze
/// result backwards and turns it into the list of items to insert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundConnectionLocator {
    /// `public final Collection<ResultItem> connectionItems` (`:30`): "the new items implementing
    /// the found connection."
    ///
    /// **Never `Option`.** The brief calls a `null` here "Java's SKIPPED signal", and
    /// `AutorouteEngine.autorouteConnection:230-235` does test `connectionItems == null` — but
    /// the field is assigned an empty `LinkedList` at `:101`, *before* both of the constructor's
    /// early returns, and it is `final`, so no path leaves it null. The SKIPPED arm is dead code;
    /// see `docs/java-quirks.md` #180. What the two early returns do produce is an **empty**
    /// list, which reaches `FoundConnectionInserter` and inserts nothing.
    pub connection_items: Vec<ResultItem>,

    /// `public final Item startItem` (`:33`): "the start item of the new routed connection."
    /// `None` is Java's `null`, i.e. the `:103-111` warn branch.
    pub start_item: Option<ItemId>,

    /// `public final int startLayer` (`:36`): "the layer of the connection to the start item."
    pub start_layer: usize,

    /// `public final Item targetItem` (`:39`): "the destination item of the new routed
    /// connection." `None` is Java's `null` — the fanout branch (`:126`) and both warn branches.
    pub target_item: Option<ItemId>,

    /// `public final int targetLayer` (`:42`): "the layer of the connection to the target item."
    pub target_layer: usize,

    /// `protected final BacktrackElement[] backtrackArray` (`:48`): "the array of backtrack doors
    /// from the destination to the start of a found connection of the maze search algorithm."
    ///
    /// Task 7 notes this is the only post-search holder of a page's drills; the walk that fills
    /// it runs before any drill-page invalidation, so every [`ExpandableRef`] in it is live.
    pub backtrack_array: Vec<BacktrackElement>,
}

impl FoundConnectionLocator {
    /// Port of `getInstance(Result, AutorouteControl, ShapeSearchTree, AngleRestriction,
    /// SortedSet<Item>, Map<Item,Integer>)` (`:185-207`): "returns a new Instance of
    /// FoundConnectionLocator or null, if destinationDoor is null."
    ///
    /// `None` is Java's `null`, and Java answers it for **one** reason only: a null
    /// `mazeSearchResult` (`:192-194`). The port takes an `Option<&MazeResult>` for exactly that,
    /// because [`MazeSearchEngine::find_connection`] already answers an `Option`.
    ///
    /// `ripup_costs` is Java's `Map<Item,Integer>`, which `AutorouteEngine.autorouteConnection`
    /// passes as `null` on the non-ripup path; `backtrack` (`:260`, `:319`) null-checks it.
    ///
    /// [`MazeSearchEngine::find_connection`]:
    ///     crate::autoroute::maze::MazeSearchEngine::find_connection
    pub fn get_instance(
        maze_search_result: Option<&MazeResult>,
        ctrl: &AutorouteControl,
        engine: &mut AutorouteEngine,
        board: &mut Board,
        angle_restriction: AngleRestriction,
        ripped_item_list: &mut BTreeSet<ItemId>,
        ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
    ) -> Option<FoundConnectionLocator> {
        // :192-194.
        let maze_search_result = maze_search_result?;
        // :195-205. The class choice is `LocatorKind`; the constructor body below is Java's
        // `FoundConnectionLocator(...)` (`:62-182`), which both subclasses delegate to with
        // `super(...)` and neither extends.
        Some(FoundConnectionLocator::new(
            maze_search_result,
            ctrl,
            engine,
            board,
            angle_restriction,
            ripped_item_list,
            ripup_costs,
        ))
    }

    /// Port of the constructor `FoundConnectionLocator(...)` (`:62-182`).
    ///
    /// `// not ported:` `FoundConnectionLocator` — the net-33/66/67 `FRLogger.trace` block of
    /// `:78-100` and the net-98 one inside `backtrack` (`:238-250`, `:290-315`), plan-6 ruling 14.
    #[allow(clippy::too_many_lines)]
    fn new(
        maze_search_result: &MazeResult,
        ctrl: &AutorouteControl,
        engine: &mut AutorouteEngine,
        board: &mut Board,
        angle_restriction: AngleRestriction,
        ripped_item_list: &mut BTreeSet<ItemId>,
        ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
    ) -> FoundConnectionLocator {
        // :71-77.
        let backtrack_array = backtrack(maze_search_result, engine, ripped_item_list, ripup_costs);

        // :101-102. `backtrackArray` is never empty: `backtrack` pushes the destination element
        // before its loop can break (`:267-274`).
        let start_info = *backtrack_array
            .last()
            .expect("backtrack always yields at least the destination element");

        // :103-111. Java warns and leaves every field at its zero value.
        let ExpandableRef::TargetDoor(start_door) = start_info.door else {
            // FRLogger.warn("FoundConnectionLocator: ItemExpansionDoor expected for
            // startInfo.door") — dropped with every other logger payload.
            return FoundConnectionLocator {
                connection_items: Vec::new(),
                start_item: None,
                start_layer: 0,
                target_item: None,
                target_layer: 0,
                backtrack_array,
            };
        };

        // :112-114.
        let start_door_data = engine
            .rooms
            .target_door(start_door)
            .expect("the start door is live: the backtrack walk just read it");
        let start_item = start_door_data.item;
        let start_tree_entry_no = start_door_data.tree_entry_no;
        let start_room = start_door_data
            .room
            .expect("TargetItemExpansionDoor.room: Java dereferences it at :114 with no guard");
        let start_layer = engine
            .rooms
            .room_layer(board, start_room)
            .expect("the start door's room is live");

        // :116-135.
        let mut at_fanout_end = false;
        let target_item;
        let target_layer;
        let current_from_point;
        match maze_search_result.destination_door {
            // :118-123.
            ExpandableRef::TargetDoor(destination_door) => {
                let door = engine
                    .rooms
                    .target_door(destination_door)
                    .expect("the destination door is live");
                let door_item = door.item;
                let door_tree_entry_no = door.tree_entry_no;
                let door_room = door
                    .room
                    .expect("TargetItemExpansionDoor.room: dereferenced at :121 with no guard");
                target_item = Some(door_item);
                target_layer = engine
                    .rooms
                    .room_layer(board, door_room)
                    .expect("the destination door's room is live");
                current_from_point = calculate_starting_point(
                    engine,
                    board,
                    door_item,
                    door_tree_entry_no,
                    door_room,
                    engine.tree,
                );
            }
            // :124-129: "may happen only in case of fanout".
            //
            // obligation: `FoundConnectionLocator` — the fanout arm (`:124-129` here and the
            // `atFanoutEnd` short-circuit at `:142-144`) — **DISCHARGED in Plan 7 Task 17**, and
            // the prediction the marker made was right. Plan 6 measured **zero** entries over its
            // whole 369-connection corpus, because the arm fires only when the maze search's
            // destination door is an `ExpansionDrill`, which needs `ctrl.isFanout`, which only
            // `BatchFanout` sets — `autoroute/pipeline`'s, i.e. Plan 7's. Plan 7 Tasks 11 and 12
            // built it (`RoutingBoardExt::fanout` sets `ctrl.is_fanout = true`), and Task 17
            // re-ran the same counter on the committed tree: the arm is entered **32 times** by
            // `crates/fr-router/tests/batch_parity.rs`'s `the_ci_stems_climb_the_whole_ladder`,
            // in **ordinary CI** (the four `java_dir`-gated stems, no `FR_SLOW_PARITY` needed),
            // and every one of those runs is byte-identical to the HEAD jar's SES. It now has
            // ground truth.
            //
            // **Standing assertion**, added in Task 17's fix round so the discharge is a test
            // rather than a removed counter: `the_fanout_arm_is_reachable_and_ends_on_a_drill`
            // at the foot of `crates/fr-router/tests/locator.rs`. It is *directed* — the file's
            // own `probe_board()` with `ctrl.is_fanout = true`, the single change from
            // `a_layer_change_yields_two_traces_and_no_via_entry` — and exact on two
            // discriminants: a `Drill` destination is only producible by
            // `MazeSearchEngine.java:361-368`'s fanout exit, and `target_item == None` with a
            // **non-empty** `connection_items` separates this arm from the two warn branches,
            // which return early with an empty list. RED-checked with `is_fanout = false`
            // (the destination is then a `TargetDoor`).
            ExpandableRef::Drill(drill) => {
                let drill = engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .expect("the destination drill is live");
                target_item = None;
                current_from_point = drill.location.to_float();
                target_layer = drill.first_layer
                    + usize::try_from(maze_search_result.section_no_of_door)
                        .expect("a section number is non-negative");
                at_fanout_end = true;
            }
            // :130-135. Java warns and returns with `startItem`/`startLayer` already set.
            ExpandableRef::Door(_) | ExpandableRef::Page(_) => {
                // FRLogger.warn("FoundConnectionLocator: unexpected type of destinationDoor")
                return FoundConnectionLocator {
                    connection_items: Vec::new(),
                    start_item: Some(start_item),
                    start_layer,
                    target_item: None,
                    target_layer: 0,
                    backtrack_array,
                };
            }
        }

        // :136-137.
        let mut walk = LocatorWalk {
            engine,
            ctrl,
            angle_restriction,
            kind: LocatorKind::of(angle_restriction),
            current_from_point,
            current_from_id: 0,
            previous_from_point: current_from_point,
            current_trace_layer: target_layer,
            current_from_door_index: 0,
            current_to_door_index: 0,
            current_target_door_index: 0,
            // Java leaves `currentTargetShape` (`:59`) null until the loop's first
            // iteration writes it at `:159` or `:166`; nothing reads it before then.
            current_target_shape: TileShape::Simplex(fr_geometry::Simplex::EMPTY),
            next_corner_id: 1,
        };

        let mut connection_items: Vec<ResultItem> = Vec::new();
        let element_count = i32::try_from(backtrack_array.len())
            .expect("the backtrack array is far shorter than i32::MAX");

        // :139-181.
        let mut connection_done = false;
        while !connection_done {
            // :141.
            let mut layer_changed = false;
            if at_fanout_end {
                // :142-144: "do not increase this.currentTargetDoorIndex".
                layer_changed = true;
            } else {
                // :146-153.
                walk.current_target_door_index = walk.current_from_door_index + 1;
                while walk.current_target_door_index < element_count && !layer_changed {
                    let index = usize::try_from(walk.current_target_door_index)
                        .expect("the loop starts at >= 1");
                    if matches!(backtrack_array[index].door, ExpandableRef::Drill(_)) {
                        layer_changed = true;
                    } else {
                        walk.current_target_door_index += 1;
                    }
                }
            }
            if layer_changed {
                // :155-159: "the next trace leads to a via".
                let index = usize::try_from(walk.current_target_door_index)
                    .expect("a drill index is non-negative");
                let ExpandableRef::Drill(drill) = backtrack_array[index].door else {
                    // Two ways in. On the ordinary path `layerChanged` is set only by the
                    // `instanceof ExpansionDrill` test at `:148`, so the cast at `:157` is safe.
                    // On the fanout path (`:142-144`) there is no test at all — but
                    // `currentTargetDoorIndex` is still 0 there and `backtrackArray[0].door` is
                    // `mazeSearchResult.destinationDoor` itself (`:267-269`), which the arm above
                    // has already matched as a drill. Java would throw a `ClassCastException`
                    // here, which `AutorouteEngine.autorouteConnection:189-195` catches into
                    // `FAILED`; the panic is the same outcome under ruling 7's `catch_unwind`.
                    panic!(
                        "FoundConnectionLocator: backtrackArray[{index}].door is not an \
                         ExpansionDrill — Java throws a ClassCastException at \
                         FoundConnectionLocator.java:157-158"
                    )
                };
                let location = walk
                    .engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .expect("the backtrack drill is live")
                    .location
                    .clone();
                walk.current_target_shape =
                    TileShape::Box(TileShape::get_instance_from_point(&location));
            } else {
                // :160-175: "the next trace leads to the final target".
                connection_done = true;
                walk.current_target_door_index = element_count - 1;
                let target_shape = trace_connection_shape(
                    board,
                    start_item,
                    walk.engine.tree,
                    start_tree_entry_no,
                )
                .expect("Connectable.getTraceConnectionShape: the start item is connectable");
                let start_room_shape = walk
                    .engine
                    .rooms
                    .room_shape(start_room)
                    .expect("the start door's room has a shape")
                    .clone();
                walk.current_target_shape = target_shape.intersection(&start_room_shape);
                // :167-175: "the target is a conduction area, make a save connection by
                // shrinking the shape by the trace halfwidth."
                //
                // obligation: `FoundConnectionLocator` — this shrink (`:167-175`) is still
                // unreachable — **re-marked in Task 17**. It was unreachable from every fixture
                // in `crates/fr-router/tests/locator.rs` (0-dimensional on all 25 evaluations),
                // and reaching `dimension() >= 2` needs a **conduction area** as the start item.
                // Task 17 added `router-ecc83-input` to the corpus precisely because its
                // `(plane …)` net puts `ConductionArea` items on the search tree; instrumented,
                // its 14 evaluations are **all** 0-dimensional, and over the whole corpus the
                // largest dimension seen is **1** (`router-dac2020-bm01` 14 evaluations,
                // `router-j2-reference` 4). What is still missing is a connection whose *start
                // item* is the conduction area itself, which `AutoroutePassRunner`'s
                // plane-skipping item selection (`BatchAutorouter.java:383-389`) makes a Plan 7
                // question.
                if walk.current_target_shape.dimension() >= 2 {
                    let start_room_layer = walk
                        .engine
                        .rooms
                        .room_layer(board, start_room)
                        .expect("the start door's room is live");
                    let trace_half_width =
                        f64::from(walk.ctrl.compensated_trace_half_width[start_room_layer]);
                    let shrinked_shape = walk.current_target_shape.offset(-trace_half_width);
                    if !shrinked_shape.is_empty() {
                        walk.current_target_shape = shrinked_shape;
                    }
                }
            }
            // :177-180.
            walk.current_to_door_index = walk.current_from_door_index + 1;
            let next_trace =
                walk.calculate_next_trace(board, &backtrack_array, layer_changed, at_fanout_end);
            at_fanout_end = false;
            connection_items.push(next_trace);
        }

        FoundConnectionLocator {
            connection_items,
            start_item: Some(start_item),
            start_layer,
            target_item,
            target_layer,
            backtrack_array,
        }
    }
}

/// Port of the private static `calculateStartingPoint(TargetItemExpansionDoor, ShapeSearchTree)`
/// (`:213-219`): "calculates the starting point of the next trace on fromDoor.item. The
/// implementation is not yet optimal for starting points on traces or areas."
fn calculate_starting_point(
    engine: &AutorouteEngine,
    board: &mut Board,
    item: ItemId,
    tree_entry_no: usize,
    room: RoomRef,
    tree: TreeId,
) -> FloatPoint {
    // :215-216.
    let connection_shape = trace_connection_shape(board, item, tree, tree_entry_no)
        .expect("Connectable.getTraceConnectionShape: a target door's item is connectable");
    // :217.
    let room_shape = engine
        .rooms
        .room_shape(room)
        .expect("the target door's room has a shape");
    let connection_shape = connection_shape.intersection(room_shape);
    // :218.
    connection_shape.centre_of_gravity().round().to_float()
}

/// `((Connectable) item).getTraceConnectionShape(searchTree, index)`, with the cast Java performs
/// at `:165` and `:216`. `None` is Java's `ClassCastException` on a non-connectable item.
fn trace_connection_shape(
    board: &Board,
    item: ItemId,
    tree: TreeId,
    index: usize,
) -> Option<TileShape> {
    let ctx = board.ctx();
    board
        .items
        .get(&item)
        .and_then(Item::as_connectable)
        .and_then(|connectable| {
            connectable
                .as_dyn()
                .get_trace_connection_shape(tree, index, &ctx)
        })
}

/// Port of the private static `backtrack(Result, SortedSet<Item>, Map<Item,Integer>, int)`
/// (`:225-327`): "creates a list of doors by backtracking from destinationDoor to the start door."
///
/// Java's "Returns null, if destinationDoor is null" (`:230-232`) is unreachable from the port:
/// [`FoundConnectionLocator::get_instance`] has already turned a null result into `None`, and a
/// `MazeResult` cannot hold a null door.
fn backtrack(
    maze_search_result: &MazeResult,
    engine: &AutorouteEngine,
    ripped_item_list: &mut BTreeSet<ItemId>,
    mut ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
) -> Vec<BacktrackElement> {
    let mut result: Vec<BacktrackElement> = Vec::new();
    // :234-237.
    let mut current_next_room: Option<RoomRef> = None;
    let mut current_backtrack_door = maze_search_result.destination_door;
    let mut current_element = engine
        .maze_search_element(
            current_backtrack_door,
            maze_search_result.section_no_of_door,
        )
        .expect("the destination door's section is live")
        .clone();

    match current_backtrack_door {
        // :251-252.
        ExpandableRef::TargetDoor(door) => {
            current_next_room = engine
                .rooms
                .target_door(door)
                .expect("the destination door is live")
                .room;
        }
        // :253-266.
        ExpandableRef::Drill(drill) => {
            let drill = engine
                .rooms
                .drills
                .get(drill.0)
                .expect("the destination drill is live");
            let index = drill.first_layer
                + usize::try_from(maze_search_result.section_no_of_door)
                    .expect("a section number is non-negative");
            current_next_room = drill.rooms[index];
            // :256-265.
            if current_element.room_ripped {
                let rooms = drill.rooms.clone();
                for room in rooms.into_iter().flatten() {
                    if let RoomRef::Obstacle(obstacle) = room {
                        let item = engine
                            .rooms
                            .obstacle_room(obstacle)
                            .expect("the drill's obstacle room is live")
                            .get_item();
                        ripped_item_list.insert(item);
                        if let Some(costs) = ripup_costs.as_deref_mut() {
                            costs.insert(item, current_element.ripup_cost);
                        }
                    }
                }
            }
        }
        ExpandableRef::Door(_) | ExpandableRef::Page(_) => {}
    }

    // :267-269.
    let mut current_backtrack_element = BacktrackElement {
        door: current_backtrack_door,
        section_no_of_door: maze_search_result.section_no_of_door,
        next_room: current_next_room,
    };
    // :271-325.
    loop {
        // :272.
        result.push(current_backtrack_element);
        // :273-276.
        let Some(next_door) = current_element.backtrack_door else {
            break;
        };
        current_backtrack_door = next_door;
        // :277-281.
        let mut current_section_no = current_element.section_no_of_backtrack_door;
        let section_count = i32::try_from(
            engine
                .maze_search_element_count(current_backtrack_door)
                .expect("a backtrack door always has its section array allocated"),
        )
        .expect("a door has far fewer sections than i32::MAX");
        if current_section_no >= section_count {
            // FRLogger.warn("FoundConnectionLocator: currentSectionNo to big")
            current_section_no = section_count - 1;
        }
        // :282-286.
        current_next_room = match current_backtrack_door {
            ExpandableRef::Drill(drill) => {
                let index =
                    usize::try_from(current_section_no).expect("a section number is non-negative");
                engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .expect("the backtrack drill is live")
                    .rooms[index]
            }
            _ => match current_next_room {
                Some(room) => engine.expandable_other_room(current_backtrack_door, room),
                // Java's `otherRoom(null)` matches neither room and answers `null` (`:62-71`).
                None => None,
            },
        };
        // :287-289.
        current_element = engine
            .maze_search_element(current_backtrack_door, current_section_no)
            .expect("the backtrack door's section is live")
            .clone();
        current_backtrack_element = BacktrackElement {
            door: current_backtrack_door,
            section_no_of_door: current_section_no,
            next_room: current_next_room,
        };
        // :316-323.
        if current_element.room_ripped
            && let Some(RoomRef::Obstacle(obstacle)) = current_next_room
        {
            let item = engine
                .rooms
                .obstacle_room(obstacle)
                .expect("the backtrack obstacle room is live")
                .get_item();
            ripped_item_list.insert(item);
            if let Some(costs) = ripup_costs.as_deref_mut() {
                costs.insert(item, current_element.ripup_cost);
            }
        }
    }
    result
}

/// Port of the private static `ninetyDegreeCorner(FloatPoint, FloatPoint, boolean)` (`:329-342`).
fn ninety_degree_corner(
    from_point: FloatPoint,
    to_point: FloatPoint,
    horizontal_first: bool,
) -> FloatPoint {
    // :333-339.
    let (x, y) = if horizontal_first {
        (to_point.x, from_point.y)
    } else {
        (from_point.x, to_point.y)
    };
    // :340.
    FloatPoint::new(x, y)
}

/// Port of the private static `fortyfiveDegreeCorner(FloatPoint, FloatPoint, boolean)`
/// (`:343-384`).
///
/// The four sign tests are **not** symmetric: `:353` is `toPoint.y >= fromPoint.y` while `:360`,
/// `:369` and `:376` are strict `>`. `P6T14Probe`'s mode `corner` pins both sides of each.
fn fortyfive_degree_corner(
    from_point: FloatPoint,
    to_point: FloatPoint,
    horizontal_first: bool,
) -> FloatPoint {
    // :345-346.
    let abs_dx = (to_point.x - from_point.x).abs();
    let abs_dy = (to_point.y - from_point.y).abs();
    let x;
    let y;

    // :350-382.
    if abs_dx <= abs_dy {
        if horizontal_first {
            x = to_point.x;
            // :353: the only non-strict comparison of the four.
            y = if to_point.y >= from_point.y {
                from_point.y + abs_dx
            } else {
                from_point.y - abs_dx
            };
        } else {
            x = from_point.x;
            y = if to_point.y > from_point.y {
                to_point.y - abs_dx
            } else {
                to_point.y + abs_dx
            };
        }
    } else if horizontal_first {
        y = from_point.y;
        x = if to_point.x > from_point.x {
            to_point.x - abs_dy
        } else {
            to_point.x + abs_dy
        };
    } else {
        y = to_point.y;
        x = if to_point.x > from_point.x {
            from_point.x + abs_dy
        } else {
            from_point.x - abs_dy
        };
    }
    // :383.
    FloatPoint::new(x, y)
}

/// Port of the package-private static `calculateAdditionalCorner(FloatPoint, FloatPoint, boolean,
/// AngleRestriction)` (`:390-404`): "calculates an additional corner, so that for the lines from
/// fromPoint to the result corner and from the result corner to toPoint angleRestriction is
/// fulfilled."
///
/// Note that the free-angle arm (`:401`) returns **`toPoint` itself**, not a copy — the object
/// identity `calculateNextTrace`'s dedup (`:432`) tests with `!=`.
pub fn calculate_additional_corner(
    from_point: FloatPoint,
    to_point: FloatPoint,
    horizontal_first: bool,
    angle_restriction: AngleRestriction,
) -> FloatPoint {
    // :396-402.
    match angle_restriction {
        AngleRestriction::NinetyDegree => {
            ninety_degree_corner(from_point, to_point, horizontal_first)
        }
        AngleRestriction::FortyFiveDegree => {
            fortyfive_degree_corner(from_point, to_point, horizontal_first)
        }
        AngleRestriction::None => to_point,
    }
}

// =================================================================================================
// The walk itself
// =================================================================================================

/// One corner produced by `calculateNextTraceCorners` (`:499`), together with the identity of the
/// Java object it is — see the module docs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LocatedCorner {
    pub(crate) point: FloatPoint,
    /// The `u64` standing in for Java's reference. Two corners with the same token *are* the same
    /// Java object; two with different tokens are different objects even when their coordinates
    /// agree.
    pub(crate) id: u64,
}

/// The mutable half of `FoundConnectionLocator`: Java's `currentFromPoint`, `previousFromPoint`,
/// `currentTraceLayer` and the three door cursors (`:53-59`), plus the engine and control the
/// walk reads.
///
/// It is a separate struct because the walk borrows the engine mutably (the 45° regime's
/// `ExpansionDoor.getSectionSegments` allocates the door's section array,
/// ExpansionDoor.java:141) while the finished [`FoundConnectionLocator`] must not.
pub(crate) struct LocatorWalk<'a> {
    pub(crate) engine: &'a mut AutorouteEngine,
    pub(crate) ctrl: &'a AutorouteControl,
    /// `protected final AngleRestriction angleRestriction` (`:51`).
    pub(crate) angle_restriction: AngleRestriction,
    /// Which override of `calculateNextTraceCorners` (`:499`) to call.
    pub(crate) kind: LocatorKind,
    /// `protected FloatPoint currentFromPoint` (`:53`).
    pub(crate) current_from_point: FloatPoint,
    /// The identity token of [`Self::current_from_point`] — see the module docs.
    pub(crate) current_from_id: u64,
    /// `protected FloatPoint previousFromPoint` (`:54`).
    pub(crate) previous_from_point: FloatPoint,
    /// `protected int currentTraceLayer` (`:55`).
    pub(crate) current_trace_layer: usize,
    /// `protected int currentFromDoorIndex` (`:56`).
    pub(crate) current_from_door_index: i32,
    /// `protected int currentToDoorIndex` (`:57`).
    pub(crate) current_to_door_index: i32,
    /// `protected int currentTargetDoorIndex` (`:58`).
    pub(crate) current_target_door_index: i32,
    /// `protected TileShape currentTargetShape` (`:59`).
    pub(crate) current_target_shape: TileShape,
    next_corner_id: u64,
}

impl LocatorWalk<'_> {
    /// A token for a freshly constructed `FloatPoint` — a Java `new FloatPoint(...)`.
    pub(crate) fn new_corner_id(&mut self) -> u64 {
        self.next_corner_id += 1;
        self.next_corner_id
    }

    /// A corner that is a brand new Java object.
    pub(crate) fn fresh(&mut self, point: FloatPoint) -> LocatedCorner {
        LocatedCorner {
            point,
            id: self.new_corner_id(),
        }
    }

    /// A corner that **is** `this.currentFromPoint` — the object identity `:101`, `:184`, `:371`
    /// and `:397` hand back.
    pub(crate) fn same_as_current_from_point(&self) -> LocatedCorner {
        LocatedCorner {
            point: self.current_from_point,
            id: self.current_from_id,
        }
    }

    /// Java's `this.currentFromPoint = point` where `previousFromPoint` is **not** touched — the
    /// 45° override's `:186`.
    pub(crate) fn set_current_from_point(&mut self, corner: LocatedCorner) {
        self.current_from_point = corner.point;
        self.current_from_id = corner.id;
    }

    /// Port of the private `calculateNextTrace(boolean, boolean)` (`:410-493`): "calculates the
    /// next trace of the connection under construction."
    ///
    /// Java's doc comment says "Returns null, if all traces are returned"; the body has no `null`
    /// return, so the port's is total.
    ///
    /// `// not ported:` `calculateNextTrace` — the net-33/66/67 `FRLogger.trace` of `:468-490`.
    fn calculate_next_trace(
        &mut self,
        board: &mut Board,
        backtrack_array: &[BacktrackElement],
        layer_changed: bool,
        at_fanout_end: bool,
    ) -> ResultItem {
        // :411-412.
        let mut corner_list: Vec<FloatPoint> = vec![self.current_from_point];
        // :413-424.
        if !at_fanout_end
            && let Some(adjusted_start_corner) = self.adjust_start_corner(backtrack_array)
        {
            // :416-419.
            let add_corner = calculate_additional_corner(
                self.current_from_point,
                adjusted_start_corner,
                true,
                self.angle_restriction,
            );
            corner_list.push(add_corner);
            corner_list.push(adjusted_start_corner);
            // :421-422.
            self.previous_from_point = self.current_from_point;
            let adjusted = self.fresh(adjusted_start_corner);
            self.set_current_from_point(adjusted);
        }
        // :425.
        let mut prev_corner_id = self.current_from_id;
        // :426-439.
        loop {
            let next_corners = self.calculate_next_trace_corners(backtrack_array);
            // :428-430.
            if next_corners.is_empty() {
                break;
            }
            for corner in next_corners {
                // :432: Java's reference comparison.
                if corner.id != prev_corner_id {
                    corner_list.push(corner.point);
                    self.previous_from_point = self.current_from_point;
                    self.set_current_from_point(corner);
                    prev_corner_id = corner.id;
                }
            }
        }

        // :441-448.
        let mut next_layer = self.current_trace_layer;
        if layer_changed {
            self.current_from_door_index = self.current_target_door_index + 1;
            let index = usize::try_from(self.current_from_door_index)
                .expect("the door index is non-negative");
            if let Some(next_room) = backtrack_array[index].next_room {
                next_layer = self
                    .engine
                    .rooms
                    .room_layer(board, next_room)
                    .expect("the backtrack room is live");
            }
        }

        // :450-459: "round the new trace corners to Integer", dropping only *consecutive*
        // duplicates — so the spikes of the 90° regime survive.
        let mut rounded_corner_list: Vec<IntPoint> = Vec::new();
        let mut prev_point: Option<IntPoint> = None;
        for corner in corner_list {
            let current_point = corner.round();
            if Some(current_point) != prev_point {
                rounded_corner_list.push(current_point);
                prev_point = Some(current_point);
            }
        }

        // :461-467.
        let result = ResultItem::new(rounded_corner_list, self.current_trace_layer);
        // :491.
        self.current_trace_layer = next_layer;
        result
    }

    /// Port of the private `adjustStartCorner()` (`:522-537`): "adjusts the start corner, so that
    /// a trace starting at this corner is completely contained in the start room."
    ///
    /// `None` is each of Java's three `return this.currentFromPoint` arms (`:524`, `:528`,
    /// `:534`) — the identity `:415` tests for with `!=`, i.e. "nothing to adjust".
    fn adjust_start_corner(&self, backtrack_array: &[BacktrackElement]) -> Option<FloatPoint> {
        // :523-525.
        if self.current_from_door_index < 0 {
            return None;
        }
        let index =
            usize::try_from(self.current_from_door_index).expect("checked non-negative above");
        // :526-529.
        let next_room = backtrack_array[index].next_room?;
        // :530-532.
        let trace_half_width =
            f64::from(self.ctrl.compensated_trace_half_width[self.current_trace_layer]);
        let room_shape = self
            .engine
            .rooms
            .room_shape(next_room)
            .expect("the backtrack room has a shape");
        let shrinked_room_shape = room_shape.offset(-trace_half_width);
        // :533-535.
        if shrinked_room_shape.is_empty()
            || shrinked_room_shape.contains_float(&self.current_from_point)
        {
            return None;
        }
        // :536.
        Some(
            shrinked_room_shape
                .nearest_point_approx(&self.current_from_point)
                .expect("a non-empty shape has a nearest point")
                .round()
                .to_float(),
        )
    }

    /// Port of the abstract `calculateNextTraceCorners()` (`:499`), dispatched over the two
    /// concrete subclasses — Java's virtual call at `:427`.
    fn calculate_next_trace_corners(
        &mut self,
        backtrack_array: &[BacktrackElement],
    ) -> Vec<LocatedCorner> {
        match self.kind {
            LocatorKind::FortyFiveDegree => {
                super::locator_45::calculate_next_trace_corners(self, backtrack_array)
            }
            LocatorKind::AnyAngle => {
                super::locator_any_angle::calculate_next_trace_corners(self, backtrack_array)
            }
        }
    }
}

// =================================================================================================
// The deferral roster for `autoroute/path/FoundConnectionLocator.java`
// =================================================================================================

// not ported: `FoundConnectionLocator.emitDiagnostics` (`:502-516`) — an `AutorouteDiagnostic.Sink`
// walk, and `AutorouteDiagnostic` is on plan-6 ruling 13's not-ported roster (a GUI overlay sink).
// Task 1 recorded that its `sink == null` guard (`:503`) is dead: `AutorouteEngine` never calls it.
