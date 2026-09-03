//! Port of `autoroute.maze.MazeSearchEngine` (MazeSearchEngine.java:42-1258) — the A\* over
//! expansion doors — as far as Task 11 carries it: construction, `getInstance`, `init`, the pop
//! loop and the four helpers those need.
//!
//! # What is here and what is not
//!
//! Task 11 owns the *frame*: the struct, `getInstance` (`:135-152`), `init` (`:969-1103`),
//! `findConnection` (`:300-312`), `occupyNextElement` (`:314-384`), `doorIsSmall` (`:763-789`),
//! `reduceTraceShapesAtTiePins` (`:154-167`), `segmentProjection` (`:173-204`),
//! `toImpactedPoints` (`:287-292`) and the two nested result types (`:1218-1227`, `:1233-1257`).
//! Task 12 landed the third of the expanders `occupyNextElement` dispatches to,
//! `expandToRoomDoors`, in `expand.rs`. The two drill expanders are Task 13's
//! `MazeExpansionEngine` (`expansion_engine.rs`) — Java's own split, where they are methods of a
//! separate class the search engine constructs at `:82` — so `occupyNextElement` dispatches to
//! `MazeExpansionEngine::expand_to_drills_of_page` and `::expand_to_other_layers` rather than to
//! members of this struct. With those two in place there is no stub left anywhere under
//! `occupyNextElement`, and [`MazeSearchEngine::find_connection`] runs end to end.
//!
//! # Why `engine` is a field and `board` is a parameter
//!
//! Java's `MazeSearchEngine` holds `autorouteEngine`, and reaches the board through
//! `autorouteEngine.board`. The port cannot: `AutorouteEngine` borrows the board per call
//! (Task 6's module docs), and `MazeQueue::push` needs `&AutorouteEngine` *and* `&Board` at the
//! same time as `&mut MazeQueue` (Task 8 §8.1). Keeping the engine as a `&mut` **field** and the
//! board as a **parameter** makes those three borrows disjoint — `&mut self.queue`,
//! `&*self.engine`, `&*board` — which is why [`MazeSearchEngine::push`] can exist at all.
//!
//! # Visibility
//!
//! Four of Java's members here are `private` or `private static` and are `pub` in the port:
//! [`MazeSearchEngine::init`], [`MazeSearchEngine::door_is_small`],
//! [`MazeSearchEngine::reduce_trace_shapes_at_tie_pins`] and [`segment_projection`]. Java's own
//! test for them is a reflective probe (`scripts/differential/java/probes/P6T11Probe.java` uses
//! `setAccessible(true)` for exactly these); Rust integration tests have no reflection, and
//! plan-6 ruling 6 requires a test per cancellation site — all four of `init`'s sites are
//! unobservable through `getInstance`, which collapses them into one `None`.

use std::collections::BTreeSet;

use fr_board::datastructures::StopCheck;
use fr_board::structure::AngleRestriction;
use fr_board::{Board, Item, ItemId, TreeId};
use fr_geometry::{FloatLine, FloatPoint, JavaRandom, Point};

use crate::arena::DoorId;
use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::item_info;
use crate::autoroute::maze::queue::p7t14b_maze_ledger;
use crate::autoroute::maze::{
    AutorouteControl, AutorouteEngine, DestinationDistance, MazeAdjustment, MazeExpansionEngine,
    MazeListElement, MazeQueue,
};

/// `MazeSearchEngine.ALREADY_RIPPED_COSTS` (`:44`): `static final int ALREADY_RIPPED_COSTS = 1`.
///
/// Read by the ripup resolver (Task 13); it lives here because it is a member of this class.
pub const ALREADY_RIPPED_COSTS: i32 = 1;

/// Port of `maze.MazeSearchEngine` (MazeSearchEngine.java:42-1258).
///
/// # Java-vs-brief
///
/// * The brief's field list omits `searchTree` (`:61`) and `ALREADY_RIPPED_COSTS` (`:44`). Both
///   are HEAD's and both are carried.
/// * `MazeFanoutDiagnostics` (`:65`) is on the plan's not-ported roster (a diagnostics sink), so
///   there is no field for it.
/// * `expansionEngine` (`:66`) and `ripupResolver` (`:67`) are Task 13's
///   [`MazeExpansionEngine`] and
///   [`crate::autoroute::maze::MazeRipupResolver`]. Both are unit structs
///   whose functions take this engine as their leading parameter, so there is no field to hold:
///   Java's two are `final` and constructed once from `this` (`:82-83`).
#[derive(Debug)]
pub struct MazeSearchEngine<'a> {
    /// `public final AutorouteEngine autorouteEngine` (`:47`) — see the module docs for why it is
    /// a `&mut` field rather than an owned value.
    pub engine: &'a mut AutorouteEngine,

    /// `final AutorouteControl ctrl` (`:49`).
    pub ctrl: &'a AutorouteControl,

    /// `final SortedSet<MazeListElement> mazeExpansionList` (`:52`): "the queue of expanded
    /// elements used in this search algorithm" — the anonymous guarded `TreeSet` of `:84-125`,
    /// which is [`MazeQueue`] (Task 8).
    pub queue: MazeQueue,

    /// `final DestinationDistance destinationDistance` (`:58`): "used for calculating of a good
    /// lower bound for the distance between a new MazeExpansionElement and the destination set of
    /// the expansion."
    pub destination_distance: DestinationDistance,

    /// `final ShapeSearchTree searchTree` (`:61`): "the search tree for expanding. It is the tree
    /// compensated for the current net" — `autorouteEngine.autorouteSearchTree` (`:81`), so a
    /// [`TreeId`] rather than a reference.
    pub search_tree: TreeId,

    /// `final Random randomGenerator` (`:63`), seeded at `:79-80` with `ctrl.ripupCosts`
    /// "to keep v1.9 deterministic randomization across passes". Its only draw anywhere in the
    /// plan is `MazeRipupResolver.java:158-163`, i.e.
    /// [`MazeRipupResolver::check_ripup`](crate::autoroute::maze::MazeRipupResolver::check_ripup)
    /// on a pass that randomises.
    pub random_generator: JavaRandom,

    /// `private ExpandableObject destinationDoor` (`:70`): "the destination door found by the
    /// expanding algorithm". Private like Java's, with
    /// [`destination_door`](Self::destination_door) as the accessor Java's own probe needs
    /// reflection for.
    destination_door: Option<ExpandableRef>,

    /// `private int sectionNoOfDestinationDoor` (`:72`).
    section_no_of_destination_door: i32,
}

impl<'a> MazeSearchEngine<'a> {
    /// Port of the constructor `MazeSearchEngine(AutorouteEngine, AutorouteControl)`
    /// (`:75-133`).
    ///
    /// `new MazeFanoutDiagnostics(ctrl)` (`:78`) is dropped (not-ported roster), and so are the
    /// `MazeExpansionEngine`/`MazeRipupResolver` constructions of `:82-83` — both are unit structs
    /// in the port and take this engine as a parameter instead.
    pub fn new(
        engine: &'a mut AutorouteEngine,
        ctrl: &'a AutorouteControl,
    ) -> MazeSearchEngine<'a> {
        // :79-80. `ctrl.ripupCosts` is an `int`, widened to Java's `long` seed.
        let mut random_generator = JavaRandom::new(0);
        random_generator.set_seed(i64::from(ctrl.ripup_costs));
        MazeSearchEngine {
            search_tree: engine.tree, // :81
            engine,
            // :126-129.
            destination_distance: DestinationDistance::new(
                &ctrl.trace_costs,
                &ctrl.layer_active,
                ctrl.min_normal_via_cost,
                ctrl.min_cheap_via_cost,
            ),
            ctrl,
            queue: MazeQueue::new(), // :84-125
            random_generator,
            destination_door: None,
            section_no_of_destination_door: 0,
        }
    }

    /// Port of `getInstance(Set<Item>, Set<Item>, AutorouteEngine, AutorouteControl)`
    /// (`:135-152`): "initializes a new instance of MazeSearchEngine for searching a connection
    /// between startItems and destinationItems. Returns null, if the initialisation failed."
    ///
    /// `None` is Java's `null`, i.e. an [`init`](Self::init) that found no start door **or** was
    /// cancelled — the two are indistinguishable to the caller in Java too.
    ///
    /// The two sets are `BTreeSet<ItemId>` where Java has `Set<Item>`; every production caller
    /// passes a `TreeSet<Item>` (`Item.getConnectedSet`/`getUnconnectedSet`, Item.java:612/677),
    /// whose order is `Item.compareTo` = `other.id - this.id`, i.e. **descending id**. `init`
    /// therefore walks both sets backwards — see the loops.
    pub fn get_instance(
        start_items: &BTreeSet<ItemId>,
        destination_items: &BTreeSet<ItemId>,
        engine: &'a mut AutorouteEngine,
        board: &mut Board,
        ctrl: &'a AutorouteControl,
        stop: StopCheck<'_>,
    ) -> Option<MazeSearchEngine<'a>> {
        // :140.
        let mut new_instance = MazeSearchEngine::new(engine, ctrl);
        // :142-147.
        if new_instance.init(board, start_items, destination_items, stop) {
            Some(new_instance)
        } else {
            None
        }
    }

    /// `private ExpandableObject destinationDoor` (`:70`) — Java reads the field directly.
    pub fn destination_door(&self) -> Option<ExpandableRef> {
        self.destination_door
    }

    /// `private int sectionNoOfDestinationDoor` (`:72`).
    pub fn section_no_of_destination_door(&self) -> i32 {
        self.section_no_of_destination_door
    }

    /// `mazeExpansionList.add(element)` (`:84-125` through its six call sites) — the guarded
    /// `add`, never a bare insert.
    ///
    /// The three borrows are disjoint fields of `self`, which is the whole reason the engine is
    /// a field and the board a parameter (module docs).
    pub fn push(&mut self, element: MazeListElement, board: &Board) -> bool {
        self.queue.push(element, self.ctrl, self.engine, board)
    }

    // =============================================================================================
    // init (:969-1103)
    // =============================================================================================

    /// Port of `init(Set<Item>, Set<Item>)` (`:969-1103`): "initializes the maze search
    /// algorithm. Returns false if the initialisation failed."
    ///
    /// `pub` where Java's is `private` — see the module docs' Visibility note.
    ///
    /// Four of plan-6 ruling 6's six cancellation sites are in here: `:975` (the destination
    /// loop), `:1009` (the start loop), `:1035` (the room-completion loop) and `:1051` (the
    /// target-door loop, which fires **once per door**, including the destination doors it is
    /// about to skip). Every one answers `false` with the work done so far left in place.
    ///
    /// The two `FRLogger.debug` failure reports (`:996-1006`, `:1085-1101`) are dropped, and with
    /// them the `expansionDoorsFound`/`expansionDoorsDestination` counters that exist only to
    /// fill them in.
    pub fn init(
        &mut self,
        board: &mut Board,
        start_items: &BTreeSet<ItemId>,
        destination_items: &BTreeSet<ItemId>,
        stop: StopCheck<'_>,
    ) -> bool {
        // :970-971.
        MazeSearchEngine::reduce_trace_shapes_at_tie_pins(
            board,
            start_items,
            self.ctrl.net_number,
            self.search_tree,
        );
        MazeSearchEngine::reduce_trace_shapes_at_tie_pins(
            board,
            destination_items,
            self.ctrl.net_number,
            self.search_tree,
        );

        // :973-986. "process the destination items".
        let mut destination_ok = false;
        for current_item in destination_items.iter().rev().copied() {
            // :975-977.
            if self.engine.is_stop_requested(stop) {
                return false;
            }
            // :978-979. `getAutorouteInfo()` creates the info on demand; `set_start_info` is the
            // same creating accessor.
            item_info::set_start_info(board, current_item, false);
            // :980-985. The loop bound is re-read on **every** iteration, as Java's
            // `i < currentItem.treeShapeCount(this.searchTree)` is: `getTreeShape` can call
            // `clearDerivedData()` (plan-6 ruling 10) and refill the cache from scratch, so the
            // count is not a loop invariant in Java and must not become one here.
            let mut i = 0;
            while i < board.item_tree_shape_count(current_item, self.search_tree) {
                if let Some(current_tree_shape) =
                    board.item_tree_shape(current_item, self.search_tree, i)
                {
                    let layer = board.item_shape_layer(current_item, i).unwrap_or_else(|| {
                        panic!(
                            "MazeSearchEngine.init: item {current_item:?} has a tree shape but no \
                             layer — Java would have NPE'd at MazeSearchEngine.java:983"
                        )
                    });
                    self.destination_distance
                        .join(&current_tree_shape.bounding_box(), layer);
                }
                i += 1;
            }
            destination_ok = true;
        }

        // :988-994. "destination set is not needed for fanout".
        if !destination_ok && self.ctrl.is_fanout {
            let board_bounding_box = board.bounding_box;
            self.destination_distance.join(&board_bounding_box, 0);
            self.destination_distance
                .join(&board_bounding_box, self.ctrl.layer_count - 1);
            destination_ok = true;
        }

        // :996-1006.
        if !destination_ok {
            return false;
        }

        // :1007-1023. "process the start items".
        let mut start_rooms = Vec::new();
        for current_item in start_items.iter().rev().copied() {
            // :1009-1011.
            if self.engine.is_stop_requested(stop) {
                return false;
            }
            // :1012-1013.
            item_info::set_start_info(board, current_item, true);
            // :1014. `currentItem instanceof Connectable`.
            if board
                .items
                .get(&current_item)
                .is_none_or(|item| item.as_connectable().is_none())
            {
                continue;
            }
            // :1015-1021. The bound is re-read every iteration, for the same reason as `:980`.
            let mut i = 0;
            while i < board.item_tree_shape_count(current_item, self.search_tree) {
                let contained_shape = {
                    let ctx = board.ctx();
                    board
                        .items
                        .get(&current_item)
                        .and_then(Item::as_connectable)
                        .and_then(|connectable| {
                            connectable.as_dyn().get_trace_connection_shape(
                                self.search_tree,
                                i,
                                &ctx,
                            )
                        })
                };
                let layer = board.item_shape_layer(current_item, i).unwrap_or_else(|| {
                    panic!(
                        "MazeSearchEngine.init: item {current_item:?} has no layer for shape {i} \
                         — Java would have NPE'd at MazeSearchEngine.java:1019"
                    )
                });
                // :1017-1020.
                let new_start_room =
                    self.engine
                        .add_incomplete_expansion_room(None, layer, contained_shape);
                start_rooms.push(new_start_room);
                i += 1;
            }
        }

        // :1026-1032. "complete the start rooms".
        let mut completed_start_rooms = Vec::new();
        if self.engine.maintain_database {
            // :1029-1031. "add the completed start rooms carried over from the last autoroute to
            // the start rooms." `getRoomsWithTargetItems` answers a `TreeSet` sorted by
            // `CompleteFreeSpaceExpansionRoom.compareTo` = `other.id - this.id`, i.e. descending,
            // so the `addAll` appends in that order (Task 6's note on the method).
            completed_start_rooms.extend(
                self.engine
                    .rooms_with_target_items(start_items)
                    .into_iter()
                    .rev(),
            );
        }

        // :1034-1041.
        for current_room in start_rooms {
            // :1035-1037.
            if self.engine.is_stop_requested(stop) {
                return false;
            }
            // :1038-1039. `Err` is Java's `return new ArrayList<>()` (quirk #166), never a
            // failure to propagate.
            let current_completed_rooms = self
                .engine
                .complete_expansion_room(board, current_room)
                .unwrap_or_default();
            completed_start_rooms.extend(current_completed_rooms);
        }

        // :1043-1082. "Put the ItemExpansionDoors of the completed start rooms into the
        // mazeExpansionList."
        let mut start_ok = false;
        for current_room in completed_start_rooms {
            let room_ref = RoomRef::Complete(current_room);
            let target_doors = self.engine.rooms.room_target_doors(room_ref).to_vec();
            for current_door in target_doors {
                // :1050 is `expansionDoorsFound++`, a counter for the dropped debug report; the
                // stop check at :1051 sits **after** it and **before** the destination test, so
                // it fires for destination doors too.
                if self.engine.is_stop_requested(stop) {
                    return false;
                }
                let door = self
                    .engine
                    .rooms
                    .target_door(current_door)
                    .expect("MazeSearchEngine.init: a target door its room still lists");
                let item = door.item;
                let tree_entry_no = door.tree_entry_no;
                // :1054-1057.
                if self
                    .engine
                    .rooms
                    .target_door(current_door)
                    .expect("just read")
                    .is_destination_door(board)
                {
                    continue;
                }
                // :1058-1060. `((Connectable) currentDoor.item).getTraceConnectionShape(...)`.
                let connection_shape = {
                    let ctx = board.ctx();
                    board
                        .items
                        .get(&item)
                        .and_then(Item::as_connectable)
                        .and_then(|connectable| {
                            connectable.as_dyn().get_trace_connection_shape(
                                self.search_tree,
                                tree_entry_no,
                                &ctx,
                            )
                        })
                        .unwrap_or_else(|| {
                            panic!(
                                "MazeSearchEngine.init: the target door's item {item:?} is not \
                                 connectable — Java's cast at MazeSearchEngine.java:1059 would \
                                 have thrown"
                            )
                        })
                };
                // :1061. `currentDoor.room.getShape()`.
                let room_shape = self
                    .engine
                    .rooms
                    .room_shape(room_ref)
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!(
                            "MazeSearchEngine.init: a completed room with no shape — Java would \
                             have NPE'd at MazeSearchEngine.java:1061"
                        )
                    });
                let connection_shape = connection_shape.intersection(&room_shape);
                // :1062-1063.
                let current_center = connection_shape.centre_of_gravity();
                let shape_entry = FloatLine::new(current_center, current_center);
                // :1064-1065.
                let layer = self
                    .engine
                    .rooms
                    .complete_room(current_room)
                    .expect("just read")
                    .get_layer();
                let sorting_value = self
                    .destination_distance
                    .calculate_from_point(&current_center, layer);
                // :1066-1078.
                let new_list_element = MazeListElement {
                    door: ExpandableRef::TargetDoor(current_door),
                    section_no_of_door: 0,
                    backtrack_door: None,
                    section_no_of_backtrack_door: 0,
                    expansion_value: 0.0,
                    sorting_value,
                    next_room: Some(room_ref),
                    shape_entry,
                    room_ripped: false,
                    adjustment: MazeAdjustment::None,
                    already_checked: false,
                    ripup_cost: 0,
                };
                self.push(new_list_element, board);
                // Java bug: `MazeSearchEngine.init`
                //
                // `:1080` sets `startOk = true` **unconditionally**, ignoring the `boolean` the
                // overridden `add` (`:86-124`) just answered. Under a fanout control whose escape
                // window refuses every seeded element, `init` therefore succeeds with an **empty**
                // queue and `getInstance` hands back an engine whose `findConnection` can only
                // answer `null`. `docs/java-quirks.md` #178, JVM-pinned by `P6T11Probe` mode
                // `fanout` (`instance=ok`, `queue n=0`).
                start_ok = true;
            }
        }
        // :1083-1102.
        start_ok
    }

    // =============================================================================================
    // The pop loop (:300-384)
    // =============================================================================================

    /// Port of `findConnection()` (`:294-308`): "does a maze search to find a connection route
    /// between the start and the destination items. If the algorithm succeeds, the ExpansionDoor
    /// and its section number of the found destination is returned, from where the whole found
    /// connection can be backtracked. Otherwise, the return value will be null."
    pub fn find_connection(
        &mut self,
        board: &mut Board,
        stop: StopCheck<'_>,
    ) -> Option<MazeResult> {
        // :301-303.
        while self.occupy_next_element(board, stop) {}
        // :304-306.
        let destination_door = self.destination_door?;
        // :307.
        Some(MazeResult {
            destination_door,
            section_no_of_door: self.section_no_of_destination_door,
        })
    }

    /// Port of `occupyNextElement()` (`:314-384`): "expands the next element in the maze expansion
    /// list. Returns false, if the expansion list is exhausted or the destination is reached."
    ///
    /// The stop check at `:323` is the hot-path one of plan-6 ruling 6, and it sits **before**
    /// `iterator().next()`, so a cancelled run leaves the queue exactly as it found it.
    pub fn occupy_next_element(&mut self, board: &mut Board, stop: StopCheck<'_>) -> bool {
        // :315-317. "destination already reached".
        if self.destination_door.is_some() {
            return false;
        }
        // :318-321.
        let mut list_element: Option<MazeListElement> = None;
        // :322-338. "Search the next element, which is not yet expanded."
        while !self.queue.is_empty() {
            // :323-325.
            if self.engine.is_stop_requested(stop) {
                return false;
            }
            // :327-329. `iterator().next()` then `it.remove()`.
            let popped = self
                .queue
                .pop_first()
                .expect("MazeQueue::pop_first on a non-empty queue");
            // :331-332.
            let is_occupied = self
                .engine
                .maze_search_element(popped.door, popped.section_no_of_door)
                .unwrap_or_else(|| {
                    panic!(
                        "MazeSearchEngine.occupyNextElement: no maze search element for section \
                         {} of {:?} — Java throws at MazeSearchEngine.java:332",
                        popped.section_no_of_door, popped.door
                    )
                })
                .is_occupied;
            // Plan 7 Task 14b's level-8 `POPQ` ledger (quirk #229); see
            // [`crate::autoroute::maze::queue::p7t14b_maze_ledger`].
            if p7t14b_maze_ledger() {
                eprintln!(
                    "POPQ occ={is_occupied} sec={} sort={:.6} exp={:.6} adj={:?}",
                    popped.section_no_of_door,
                    popped.sorting_value,
                    popped.expansion_value,
                    popped.adjustment
                );
            }
            // :334-337.
            if !is_occupied {
                list_element = Some(popped);
                break;
            }
        }
        // Plan 7 Task 14b's level-8 `OCC` ledger — the element this call actually expands.
        if p7t14b_maze_ledger() {
            match &list_element {
                None => eprintln!("OCC found=false"),
                Some(element) => {
                    let room = match element.next_room {
                        None => "null".to_string(),
                        Some(RoomRef::Obstacle(id)) => {
                            self.engine.rooms.obstacle_room(id).map_or_else(
                                || "obst?".to_string(),
                                |room| {
                                    format!(
                                        "obst{}:{}",
                                        room.get_item().0,
                                        room.get_index_in_item()
                                    )
                                },
                            )
                        }
                        Some(_) => "free".to_string(),
                    };
                    eprintln!(
                        "OCC found=true sec={} exp={:.6} sort={:.6} adj={:?} chk={} rip={} \
                         cost={} room={room}",
                        element.section_no_of_door,
                        element.expansion_value,
                        element.sorting_value,
                        element.adjustment,
                        element.already_checked,
                        element.room_ripped,
                        element.ripup_cost
                    );
                }
            }
        }
        // :339-341.
        let Some(list_element) = list_element else {
            return false;
        };

        // :342-346. Java writes through the live `currentDoorSection` reference; the port
        // re-resolves the same section, which is the same object.
        {
            let section = self
                .engine
                .maze_search_element_mut(list_element.door, list_element.section_no_of_door)
                .expect("just resolved above");
            section.backtrack_door = list_element.backtrack_door;
            section.section_no_of_backtrack_door = list_element.section_no_of_backtrack_door;
            section.room_ripped = list_element.room_ripped;
            section.ripup_cost = list_element.ripup_cost;
            section.adjustment = list_element.adjustment;
        }

        // :348-351.
        if matches!(list_element.door, ExpandableRef::Page(_)) {
            MazeExpansionEngine::expand_to_drills_of_page(self, board, &list_element, stop);
            return true;
        }

        // :353-359. "The destination is reached."
        if let ExpandableRef::TargetDoor(current_door) = list_element.door {
            let is_destination = self
                .engine
                .rooms
                .target_door(current_door)
                .expect("just resolved above")
                .is_destination_door(board);
            if is_destination {
                self.destination_door = Some(list_element.door);
                self.section_no_of_destination_door = list_element.section_no_of_door;
                return false;
            }
        }

        // :361-368. "algorithm completed after the first drill".
        if self.ctrl.is_fanout
            && matches!(list_element.door, ExpandableRef::Drill(_))
            && matches!(list_element.backtrack_door, Some(ExpandableRef::Drill(_)))
        {
            self.destination_door = Some(list_element.door);
            self.section_no_of_destination_door = list_element.section_no_of_door;
            return false;
        }

        // :369-373.
        if self.ctrl.vias_allowed
            && matches!(list_element.door, ExpandableRef::Drill(_))
            && !matches!(list_element.backtrack_door, Some(ExpandableRef::Drill(_)))
        {
            MazeExpansionEngine::expand_to_other_layers(self, board, &list_element);
        }

        // :375-380. Note that this is **not** an `else` of the drill branch above: a drill
        // element with a next room reaches both.
        if list_element.next_room.is_some() && !self.expand_to_room_doors(board, &list_element) {
            // "occupation by ripup is delayed or nothing was expanded. In case nothing was
            // expanded allow the section to be occupied from somewhere else, if the next room is
            // thin."
            return true;
        }

        // :382-383.
        self.engine
            .maze_search_element_mut(list_element.door, list_element.section_no_of_door)
            .expect("just resolved above")
            .is_occupied = true;
        true
    }

    // =============================================================================================
    // doorIsSmall (:763-789)
    // =============================================================================================

    /// Port of `doorIsSmall(ExpansionDoor, double)` (`:763-789`): "checks, if the width door is
    /// big enough for a trace with width traceWidth."
    ///
    /// `pub` where Java's is `private` — see the module docs' Visibility note. Its production
    /// caller is `expandToRoomDoors` (Task 12).
    ///
    /// The `FRLogger.trace` at `:769` is dropped; its `return true` is not.
    pub fn door_is_small(&self, board: &Board, door: DoorId, trace_width: f64) -> bool {
        let Some(current_door) = self.engine.rooms.door(door) else {
            // A stale door id, where Java holds a live reference. Answering `false` is the same
            // as the `:787` fall-through.
            return false;
        };
        // :764-766. `door.firstRoom instanceof CompleteFreeSpaceExpansionRoom` is
        // `RoomRef::Complete`; an `ObstacleExpansionRoom` is a `CompleteExpansionRoom` but not a
        // free-space one, so `RoomRef::Obstacle` fails the test exactly as Java's `instanceof`
        // does.
        let both_free_space = matches!(current_door.first_room, RoomRef::Complete(_))
            && matches!(current_door.second_room, RoomRef::Complete(_));
        if current_door.dimension != 1 && !both_free_space {
            // :787.
            return false;
        }
        // :767.
        let Some(door_shape) = self.engine.rooms.door_shape(door) else {
            // Java would NPE on a room with no shape; the empty-shape arm below is the same
            // answer.
            return true;
        };
        // :768-771.
        if door_shape.is_empty() {
            return true;
        }
        // :773-784.
        let door_length = match board.rules.trace_angle_restriction {
            AngleRestriction::NinetyDegree => {
                // :775-777.
                door_shape.bounding_box().max_width()
            }
            AngleRestriction::FortyFiveDegree => {
                // :778-780.
                door_shape
                    .bounding_octagon()
                    .unwrap_or_else(|| {
                        panic!(
                            "MazeSearchEngine.doorIsSmall: a non-empty door shape with no bounding \
                             octagon — Java would have NPE'd at MazeSearchEngine.java:780"
                        )
                    })
                    .max_width()
            }
            AngleRestriction::None => {
                // :781-783.
                let door_line_segment = door_shape.diagonal_corner_segment().unwrap_or_else(|| {
                    panic!(
                        "MazeSearchEngine.doorIsSmall: a non-empty door shape with no diagonal \
                         corner segment — Java would have NPE'd at MazeSearchEngine.java:783"
                    )
                });
                door_line_segment.b.distance(&door_line_segment.a)
            }
        };
        // :785.
        door_length < trace_width
    }

    // =============================================================================================
    // reduceTraceShapesAtTiePins (:154-167)
    // =============================================================================================

    /// Port of the private static `reduceTraceShapesAtTiePins(Collection<Item>, int,
    /// ShapeSearchTree)` (`:154-167`): "looks for pins with more than 1 nets and reduces shapes of
    /// traces of foreign nets, which are already connected to such a pin, so that the pin center
    /// is not blocked for connection."
    ///
    /// `pub` where Java's is `private static` — see the module docs' Visibility note.
    ///
    /// Both iteration orders are Java's `TreeSet<Item>`, i.e. **descending id** (`Item.compareTo`
    /// is `other.id - this.id`, Item.java:95-101): the `itemList` argument is the caller's start
    /// or destination set, and `getNormalContacts()` answers a `TreeSet` too
    /// (DrillItem.java:274-305). `Board::normal_contacts` is a `BTreeSet`, so both walks are
    /// `.rev()`.
    pub fn reduce_trace_shapes_at_tie_pins(
        board: &mut Board,
        item_list: &BTreeSet<ItemId>,
        own_net_no: i32,
        autoroute_tree: TreeId,
    ) {
        // :156.
        for current_item in item_list.iter().rev().copied() {
            // :157. `currentItem instanceof Pin currentTiePin && currentItem.netCount() > 1`.
            let is_tie_pin = board
                .items
                .get(&current_item)
                .is_some_and(|item| matches!(item, Item::Pin(_)) && item.net_count() > 1);
            if !is_tie_pin {
                continue;
            }
            // :158.
            let pin_contacts = board.normal_contacts(current_item);
            for current_contact in pin_contacts.into_iter().rev() {
                // :160-162.
                let is_foreign_trace = board.items.get(&current_contact).is_some_and(|item| {
                    matches!(item, Item::Trace(_)) && !item.contains_net(own_net_no)
                });
                if !is_foreign_trace {
                    continue;
                }
                // :163. `autorouteTree.reduceTraceShapeAtTiePin(currentTiePin, currentContact)`
                // needs the tree, the pin and the trace mutably at once; the tree comes out of
                // the board for the call and goes straight back, exactly as
                // `AutorouteEngine::new` does for `getAutorouteTree`.
                // `ShapeSearchTree::reduce_trace_shape_at_tie_pin` reads both objects' tree
                // shapes out of the cache (`Pin::get_tree_shape_on_layer` and the trace's own
                // shapes), where Java reaches them through `Item.getTreeShape`, which **fills**
                // the cache on demand (Item.java:212-226). The fill is forced here so that a pin
                // or trace the autoroute tree has not yet touched behaves as Java's does.
                board.item_tree_shape_count(current_item, autoroute_tree);
                board.item_tree_shape_count(current_contact, autoroute_tree);
                let Some(Item::Pin(tie_pin)) = board.items.get(&current_item).cloned() else {
                    continue;
                };
                let Some(Item::Trace(mut trace)) = board.items.remove(&current_contact) else {
                    continue;
                };
                let mut trees = std::mem::take(&mut board.trees);
                {
                    let ctx = board.ctx();
                    trees
                        .trees_mut()
                        .find(|tree| tree.id() == autoroute_tree)
                        .unwrap_or_else(|| {
                            panic!(
                                "MazeSearchEngine.reduceTraceShapesAtTiePins: no search tree with \
                                 id {autoroute_tree:?}"
                            )
                        })
                        .reduce_trace_shape_at_tie_pin(&tie_pin, &mut trace, &ctx);
                }
                board.trees = trees;
                board.items.insert(current_contact, Item::Trace(trace));
                // T17: #193's prime suspect — the one board write inside `init`, and the only one
                // that shortens a foreign trace's shape array under rooms that already exist.
                crate::autoroute::instrument::note_mutation(
                    crate::autoroute::instrument::Mutation::TiePinReduction,
                );
            }
        }
    }

    // =============================================================================================
    // The expanders — Tasks 12 and 13
    // =============================================================================================
}

// =================================================================================================
// The two static helpers
// =================================================================================================

/// Port of the private static `segmentProjection(FloatLine, FloatLine)` (`:169-204`): "returns the
/// perpendicular projection of fromSegment onto toSegment. Returns null, if the projection is
/// empty."
///
/// `pub` where Java's is `private static` — see the module docs' Visibility note. Its production
/// caller is `expandToRoomDoors` (Task 12).
///
/// # `==` on `FloatPoint` is Java's reference identity, and the answers agree
///
/// `:180` and `:189` compare with `==`, which in Java is object identity: both
/// `FloatLine.segmentProjection` (FloatLine.java) and `segmentProjection2` return `this.a` /
/// `this.b` **by reference** in one branch each, and a fresh `perpendicularProjection` otherwise.
/// The port compares by value, which is true strictly more often — and where it is true and
/// Java's identity is false, Java's `else if` then compares `distanceSquare(…, toSegment.a)`
/// against the other candidate's with the left side at **zero**, so it picks a point equal to
/// `toSegment.a` anyway. The two therefore answer the same `FloatLine` on every input; the only
/// representable difference is the sign of a zero coordinate.
pub fn segment_projection(from_segment: &FloatLine, to_segment: &FloatLine) -> Option<FloatLine> {
    // :174-176.
    let check_segment = from_segment.adjust_direction(to_segment);
    let first_projection = to_segment.segment_projection(&check_segment);
    let second_projection = to_segment.segment_projection_2(&check_segment);
    match (first_projection, second_projection) {
        // :178-197.
        (Some(first_projection), Some(second_projection)) => {
            // :179-187.
            let result_a =
                if first_projection.a == to_segment.a || second_projection.a == to_segment.a {
                    to_segment.a
                } else if first_projection.a.distance_square(&to_segment.a)
                    <= second_projection.a.distance_square(&to_segment.a)
                {
                    first_projection.a
                } else {
                    second_projection.a
                };
            // :188-196.
            let result_b =
                if first_projection.b == to_segment.b || second_projection.b == to_segment.b {
                    to_segment.b
                } else if first_projection.b.distance_square(&to_segment.b)
                    <= second_projection.b.distance_square(&to_segment.b)
                {
                    first_projection.b
                } else {
                    second_projection.b
                };
            // :197.
            Some(FloatLine::new(result_a, result_b))
        }
        // :198-199.
        (Some(first_projection), None) => Some(first_projection),
        // :200-203.
        (None, second_projection) => second_projection,
    }
}

/// Port of the private static `toImpactedPoints(FloatLine)` (`:287-292`): the two rounded ends of
/// a shape entry.
///
/// Java's only callers are the `FRLogger.trace` payloads this port drops, so the function has no
/// production caller here — exactly as `DestinationDistance.calculateCheapDistance` has none in
/// Java (Task 8 §2.2). It is carried because the task brief names it and because Task 12's
/// tracing, if it is ever restored, needs it to mean the same thing.
pub fn to_impacted_points(shape_entry: Option<&FloatLine>) -> Option<[Point; 2]> {
    // :288-290.
    let shape_entry = shape_entry?;
    // :291.
    Some([
        Point::from(shape_entry.a.round()),
        Point::from(shape_entry.b.round()),
    ])
}

// =================================================================================================
// The two nested result types
// =================================================================================================

/// Port of the nested `MazeSearchEngine.Result` (`:1218-1227`): "the result type of
/// MazeSearchEngine.find_connection."
///
/// renamed: `MazeSearchEngine.Result` -> `MazeResult`; a bare `Result` in a Rust crate would
/// shadow the prelude's, and this type is not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MazeResult {
    /// `public final ExpandableObject destinationDoor` (`:1220`).
    pub destination_door: ExpandableRef,
    /// `public final int sectionNoOfDoor` (`:1221`).
    pub section_no_of_door: i32,
}

/// Port of the nested `MazeSearchEngine.ShoveResult` (`:1233-1257`): "used for the result of
/// MazeShoveViaAlgo.check_shove_via and MazeShoveThinRoomAlgo.check_shove_thin_room."
///
/// Both producers are Task 12's; the type is here because it is a member of this class.
#[derive(Debug, Clone, PartialEq)]
pub struct ShoveResult {
    /// `final ExpansionDoor oppositeDoor` (`:1236`): "the opposite door to be expanded."
    pub opposite_door: DoorId,
    /// `final Collection<ExpansionDoor> sideDoors` (`:1239`): "the doors at the adjusted edge of
    /// the room shape to be expanded."
    pub side_doors: Vec<DoorId>,
    /// `final FloatPoint fromDoorPassingPoint` (`:1242`): "the passing point of a trace through
    /// the from_door after adjustment."
    pub from_door_passing_point: FloatPoint,
    /// `final FloatPoint oppositeDoorPassingPoint` (`:1245`): "the passing point of a trace
    /// through the opposite door after adjustment."
    pub opposite_door_passing_point: FloatPoint,
}

impl ShoveResult {
    /// Port of the constructor (`:1247-1256`).
    pub fn new(
        opposite_door: DoorId,
        side_doors: Vec<DoorId>,
        from_door_passing_point: FloatPoint,
        opposite_door_passing_point: FloatPoint,
    ) -> ShoveResult {
        ShoveResult {
            opposite_door,
            side_doors,
            from_door_passing_point,
            opposite_door_passing_point,
        }
    }
}

// =================================================================================================
// The deferral roster for `autoroute/maze/MazeSearchEngine.java`
// =================================================================================================

// The cost model, the room-door expansion and the two shove checks are Task 12's; they live in
// `autoroute/maze/expand.rs`, which the audit map already points `MazeSearchEngine` at.
//
// not ported: `MazeSearchEngine.describeExpandable` (`:206-250`), `safeMazeSectionCount`
// (`:252-258`), `describeExpandableBounds` (`:260-266`) and `describeRoom` (`:268-285`) — the
// helpers whose only callers are the `FRLogger.trace` payloads plan-6's global constraints drop.
// The plan text says "the eight `describe*` helpers of `MazeSearchEngine` (`:206-296`)"; HEAD has
// **four**, they span `:206-285`, and `:287-292` is `toImpactedPoints`, which *is* ported. Java
// wins over the plan.
//
// not ported: `MazeSearchEngine.fanoutDiagnostics` (`:65`) and every `MazeFanoutDiagnostics` call
// — the plan's not-ported roster names the class (a diagnostics sink, 44 loc).
