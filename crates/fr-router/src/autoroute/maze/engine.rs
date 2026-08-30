//! Port of `autoroute.maze.AutorouteEngine` (AutorouteEngine.java:39-675) — the object that owns
//! every expansion room, door, drill and page of one routing run, plus the handle to the
//! compensated search tree the maze search walks.
//!
//! # What this module is and is not
//!
//! Java's class is the router's whole temporary database *and* its entry point
//! (`autorouteConnection`, `:130-…`). This task lands the **database half**: construction,
//! `initConnection`, `clear`, the incomplete/complete room add-and-remove pairs,
//! `completeExpansionRoom`, `completeNeighbourRooms`, the door reset and removal, the target-item
//! query, `validate`, `generateRoomIdNo` and `isStopRequested`. `autorouteConnection` and the
//! maze search it drives are Tasks 11-16.
//!
//! # The three deliberate shape changes
//!
//! 1. **`ExpansionRoomStore` is embedded, not re-declared.** Java's `incompleteExpansionRooms`
//!    (`:71`), `completeExpansionRooms` (`:74`) and `expansionRoomInstanceCount` (`:77`) live in
//!    [`ExpansionRoomStore`] together with the arenas that replace Java's heap (plan-6
//!    ruling 16). The engine holds one and delegates; there is exactly one room-id counter.
//! 2. **The board is a parameter, not a field.** Java's `public final RoutingBoard board` (`:59`)
//!    is a back-pointer into the object that owns the engine (`RoutingBoard.autorouteEngine`), a
//!    cycle the port cannot express while keeping `Board: Send + Sync` (plan-2 ruling 11). Every
//!    method that Java would have read `this.board` in takes `&Board`/`&mut Board`.
//! 3. **`stoppableThread` (`:62`) is a parameter too.** [`StopCheck`] is a borrowed
//!    `&dyn Fn() -> bool`; storing one would put a lifetime on `AutorouteEngine` and thus on every
//!    type that holds one. Ruling 6 fixes the cancellation checks at six call sites, and each of
//!    them has the caller's stop flag in scope, so [`AutorouteEngine::is_stop_requested`] takes it
//!    per call. Java's `stoppableThread == null` (`:300-302`) is the caller passing `&|| false`.
//!
//! # Quirk #162 is a hang, and this module does not guard it
//!
//! `SortedRoomNeighbours.calculateNewIncompleteRooms` does not terminate for roughly 0.4 % of
//! completions (`docs/java-quirks.md` #162: `touchingSideNoOfRoom` is computed against the
//! un-simplified room shape but the backwards walk at `SortedRoomNeighbours.java:562` indexes
//! `toSimplex()`, so a side number the simplex does not have makes `prevNo` cycle for ever).
//! [`AutorouteEngine::complete_expansion_room`] reaches it through `addCompleteRoom`, and it is
//! reproduced rather than guarded — a guard would be a divergence, and the Java run it has to
//! match hangs too. **The wall-clock bound belongs to the caller** (Tasks 9 and 17), which is
//! also where Java's own `TimeLimit` is checked: `isStopRequested` is not consulted anywhere on
//! this path, in Java or here.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;

use fr_board::ids::TreeObject;
use fr_board::{
    Board, Item, ItemId, RoomId, ShapeSearchTree, StopCheck, StopConnectionOption, TimeLimit,
    TreeId,
};
use fr_geometry::{Simplex, TileShape, java_min};
use fr_settings::{ExpansionCostFactor, RouterSettings};

use crate::Arena;
use crate::arena::{DoorId, DrillId, IncompleteRoomId, PageId};
use crate::autoroute::attempt::{AutorouteAttemptResult, AutorouteAttemptState};
use crate::autoroute::drill::DrillPageArray;
use crate::autoroute::expansion::sorted_neighbours::SortedRoomNeighbours;
use crate::autoroute::expansion::{
    ExpandableRef, ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef,
};
use crate::autoroute::item_info;
use crate::autoroute::maze::{AutorouteControl, MazeResult, MazeSearchElement, MazeSearchEngine};
use crate::autoroute::path::{Connection, FoundConnectionInserter, FoundConnectionLocator};
use crate::autoroute::tree_ext::AutorouteSearchTreeExt;
use crate::board_ext::RoutingBoardExt;
use crate::error::RouterError;

/// Port of `AutorouteEngine` (AutorouteEngine.java:39-675).
///
/// See the module docs for the three places the port's shape differs from Java's.
#[derive(Debug)]
pub struct AutorouteEngine {
    /// `incompleteExpansionRooms` (`:71`), `completeExpansionRooms` (`:74`) and
    /// `expansionRoomInstanceCount` (`:77`), plus the arenas that stand in for Java's heap.
    ///
    /// Public because Tasks 7 and 11-16 read rooms and doors out of it constantly, exactly as
    /// Java's package-private field access does.
    pub rooms: ExpansionRoomStore,

    /// `autorouteSearchTree` (`:47`): "the current search tree used in autorouting. It depends on
    /// the trace clearance class used in the autoroute algorithm."
    ///
    /// A [`TreeId`] rather than a reference: the tree lives in `board.trees`, and the engine
    /// borrows the board per call.
    pub tree: TreeId,

    /// `maintainDatabase` (`:53`): "if `maintainDatabase`, the autorouter database is maintained
    /// after a connection is completed for performance reasons."
    pub maintain_database: bool,

    /// The `maxDrillPageWidth` of `:89-90`, kept so [`drill_pages`](Self::drill_pages) is built
    /// from the number this constructor computed rather than recomputing it from a board that may
    /// have changed since.
    pub max_drill_page_width: i32,

    /// `drillPageArray` (`:56`): "the 2-dimensional array of rectangular pages of
    /// `ExpansionDrill`s", built once by the constructor (`:91`) and never replaced.
    ///
    /// Private with [`drill_pages`](Self::drill_pages)/[`drill_pages_mut`](Self::drill_pages_mut)
    /// accessors where Java's field is package-private, because
    /// [`drill_page_drills`](Self::drill_page_drills) has to move the grid out of it and put it
    /// back — a caller holding the field open across that would see a grid with no pages.
    drill_page_array: DrillPageArray,

    /// `completeExpansionRooms` (`:74`): "the list of complete expansion rooms on the routing
    /// board."
    ///
    /// **This is not the same set as `rooms.complete_rooms`, and the difference is reachable.**
    /// `SortedRoomNeighbours.calculate` builds a `CompleteFreeSpaceExpansionRoom` *before* it
    /// knows whether the room will be kept: the `edgeRemoved` retry (SortedRoomNeighbours.java:
    /// 111-114) throws one away and recurses, and `addCompleteRoom` (`:528-530`) throws away one
    /// whose completed shape is no longer 2-dimensional. Java drops those on the floor — they
    /// become heap garbage that is never in `completeExpansionRooms` — while the port's arena is
    /// the heap and keeps them. Probe mode 1 makes it concrete: nine rooms are constructed, six
    /// reach the list, and rooms 3, 4 and 5 exist only as garbage.
    ///
    /// So every method Java writes as a walk over `completeExpansionRooms` walks **this** vector:
    /// `clear` (`:309`), `initConnection` (`:102`), `getRoomsWithTargetItems` (`:623`),
    /// `validate` (`:642`) and `resetAllDoors` (`:656`). Walking the arena instead would answer
    /// rooms Java cannot see.
    complete_expansion_rooms: Vec<RoomId>,

    /// `netNumber` (`:65`): "the net number used for routing in this autoroute algorithm."
    /// `-1` until the first [`init_connection`](Self::init_connection) (`:87`).
    net_number: i32,

    /// `timeLimit` (`:68`): "to stop the expansion algorithm after a time limit is exceeded."
    time_limit: Option<TimeLimit>,

    /// The heap `autoroute.path.Connection` objects live on (plan-6 ruling 15).
    ///
    /// **Not a Java field.** Java's `Connection.get` allocates on the heap and every member item
    /// points at the object through `ItemAutorouteInfo.precalculatedConnection`; the port's
    /// `AutorouteInfo` holds a [`fr_board::ConnectionId`] into this arena instead. It lives on the
    /// engine because that is the scope the memo has: `resetAllDoors` (`:654-668`) clears every
    /// item's `precalculatedConnection` and is the engine's own method.
    ///
    /// Nothing removes from it. A cleared memo leaves its slot behind exactly as Java leaves the
    /// object for the collector, and [`Arena`] never reuses an index, so a `ConnectionId` a
    /// cleared item still happened to hold could not name a *different* connection.
    pub connections: Arena<Connection>,
}

impl AutorouteEngine {
    /// Port of `AutorouteEngine(RoutingBoard, int, boolean)` (AutorouteEngine.java:83-93).
    ///
    /// Takes the board **mutably** because `SearchTreeManager.getAutorouteTree` (`:88`) may build
    /// a compensated tree and fill it from the board's items, which needs `&mut Item` for the
    /// lazily precalculated tree shapes.
    ///
    /// `stoppableThread = null` (`:92`) has no field to set — see the module docs.
    pub fn new(
        board: &mut Board,
        trace_clearance_class_index: usize,
        maintain_database: bool,
    ) -> AutorouteEngine {
        // AutorouteEngine.java:88. The items and the tree manager both live in `board`, so they
        // are lifted out for the call and put straight back; `getAutorouteTree` needs `&mut` on
        // one and `&` on the other.
        let tree = {
            let mut items = std::mem::take(&mut board.items);
            let mut trees = std::mem::take(&mut board.trees);
            let id = {
                let ctx = board.ctx();
                let mut refs: Vec<&mut fr_board::Item> = items.values_mut().rev().collect();
                trees
                    .get_autoroute_tree(trace_clearance_class_index, &mut refs, &ctx)
                    .id()
            };
            board.items = items;
            board.trees = trees;
            id
        };

        // AutorouteEngine.java:89-90. Java's `(int)` cast on the `double` product truncates
        // toward zero and saturates, which is what Rust's `as i32` does.
        let default_via_diameter = board
            .rules
            .get_default_via_diameter(&board.library.padstacks);
        let max_drill_page_width = ((5.0 * default_via_diameter) as i32).max(10_000);

        // AutorouteEngine.java:91.
        let drill_page_array = DrillPageArray::new(board, max_drill_page_width);

        AutorouteEngine {
            rooms: ExpansionRoomStore::new(),
            complete_expansion_rooms: Vec::new(),
            connections: Arena::new(),
            tree,
            maintain_database,
            max_drill_page_width,
            drill_page_array,
            // :87.
            net_number: -1,
            time_limit: None,
        }
    }

    /// Port of `initConnection(int, Stoppable, TimeLimit)` (AutorouteEngine.java:95-123):
    /// "initializes a connection search for the specified net number."
    ///
    /// The invalidation at `:98-118` runs only when the database is being maintained **and** the
    /// net actually changed. Java's `completeExpansionRooms != null` guard (`:99`) has no
    /// counterpart — an empty arena selects no rooms.
    ///
    /// The `Stoppable` argument is the port's per-call [`StopCheck`] (module docs).
    pub fn init_connection(
        &mut self,
        board: &mut Board,
        net_number: i32,
        time_limit: Option<TimeLimit>,
    ) {
        // :97-98.
        if self.maintain_database && net_number != self.net_number {
            // :100-109. "Invalidate the net dependent complete free space expansion rooms."
            // Java collects into a second list first, because `removeCompleteExpansionRoom`
            // mutates the one it is iterating; collecting the ids does the same job.
            let rooms_to_remove: Vec<RoomId> = self
                .complete_expansion_rooms
                .iter()
                .copied()
                .filter(|room| {
                    self.rooms
                        .complete_room(*room)
                        .is_some_and(|r| r.is_net_dependent())
                })
                .collect();
            for room in rooms_to_remove {
                self.remove_complete_expansion_room(board, room);
            }

            // :111-117. "Invalidate the neighbour rooms of the items of netNumber."
            //
            // discharged in Task 9: `RoutingBoard.additionalUpdateAfterChange`
            // (RoutingBoard.java:96-118) landed as `RoutingBoardExt::additional_update_after_change`
            // (`crates/fr-router/src/board_ext/routing_board_ext.rs`), and this is Java's `:112-116`
            // loop over `board.getItems()` — `Board::items_in_board_order`, descending id
            // (quirk #63). `additional_update_after_change` removes rooms from `self`, so the id
            // list is materialised first, exactly as Java's `Collection<Item> itemList` snapshot is.
            let items: Vec<ItemId> = board
                .items_in_board_order()
                .into_iter()
                .filter(|id| {
                    board
                        .get_item(*id)
                        .is_some_and(|item| item.contains_net(net_number))
                })
                .collect();
            for item in items {
                board.additional_update_after_change(self, item);
            }
        }
        // :120-122.
        self.net_number = net_number;
        self.time_limit = time_limit;
    }

    /// `completeExpansionRooms` (`:74`) in list order — see the field docs for why this is not
    /// the complete-room arena. Java has no accessor; this one exists so the maze search and the
    /// tests can walk the list the way Java's package-private field access does.
    pub fn complete_expansion_rooms(&self) -> &[RoomId] {
        &self.complete_expansion_rooms
    }

    /// Port of `getNetNumber()` (AutorouteEngine.java:288-291): "the net number of the current
    /// connection to route", `-1` before the first [`init_connection`](Self::init_connection).
    pub fn get_net_number(&self) -> i32 {
        self.net_number
    }

    /// Port of `isStopRequested()` (AutorouteEngine.java:293-304): "returns if the user has
    /// stopped the autorouter."
    ///
    /// The time limit is tested **first** and short-circuits, so an expired limit answers `true`
    /// without consulting the stop flag. Java's `stoppableThread == null` (`:300-302`) is the
    /// caller passing `&|| false`.
    ///
    /// Ruling 6 fixes the six call sites (`MazeSearchEngine.java:323`, `:975`, `:1009`, `:1035`,
    /// `:1051` and `DrillPage.java:103`); adding a seventh would make a timed-out run stop
    /// earlier than Java's and change the routed-connection count.
    pub fn is_stop_requested(&self, stop: StopCheck<'_>) -> bool {
        // :295-299.
        if let Some(time_limit) = &self.time_limit
            && time_limit.is_exceeded()
        {
            return true;
        }
        // :300-303.
        stop()
    }

    /// Port of `clear()` (AutorouteEngine.java:306-317): "clears all temporary data."
    ///
    /// `:308-312` takes every complete room out of the search tree **before** the lists go —
    /// otherwise the tree would keep `TreeObject::Room` leaves whose arena slots no longer exist —
    /// and `:316` clears every item's autoroute scratch, which is what stops an item keeping an
    /// `ObstacleRoomId` into a restarted arena.
    pub fn clear(&mut self, board: &mut Board) {
        // :308-315. `ExpansionRoomStore::clear` walks the *arena* rather than
        // `completeExpansionRooms`, which is a superset: the extra rooms are the garbage of
        // [`Self::complete_expansion_rooms`], and they were never inserted into the tree, so
        // their `removeFromTree` is the no-op Java's `MinAreaTree.removeLeaf` performs for a null
        // leaf.
        {
            let tree = tree_mut(board, self.tree);
            self.rooms.clear(tree);
        }
        // :313.
        self.complete_expansion_rooms.clear();
        // :316.
        board.clear_all_item_temporary_autoroute_data();
    }

    /// Port of `addIncompleteExpansionRoom(TileShape, int, TileShape)`
    /// (AutorouteEngine.java:341-350): "creates a new `FreeSpaceExpansionRoom` and adds it to the
    /// room list. Its shape is normally unbounded at construction time of the room."
    ///
    /// A `None` shape is Java's `null`, which the constructor's javadoc documents as "the whole
    /// plane" (`IncompleteFreeSpaceExpansionRoom.java:14-16`).
    pub fn add_incomplete_expansion_room(
        &mut self,
        shape: Option<TileShape>,
        layer: usize,
        contained_shape: Option<TileShape>,
    ) -> IncompleteRoomId {
        self.rooms
            .new_incomplete_room(shape, layer, contained_shape)
    }

    /// Port of `getFirstIncompleteExpansionRoom()` (AutorouteEngine.java:356-365): "the first
    /// element in the list of incomplete expansion rooms, or null if the list is empty."
    ///
    /// Java's two `null`/`isEmpty` guards are the one `None` here. The arena appends and never
    /// reuses a slot, so ascending index is Java's list order.
    pub fn get_first_incomplete_expansion_room(&self) -> Option<IncompleteRoomId> {
        self.rooms
            .incomplete_rooms
            .iter()
            .next()
            .map(|(index, _)| IncompleteRoomId(index))
    }

    /// Port of `removeIncompleteExpansionRoom(IncompleteFreeSpaceExpansionRoom)`
    /// (AutorouteEngine.java:367-371): "removes an incomplete room from the database."
    pub fn remove_incomplete_expansion_room(&mut self, room: IncompleteRoomId) {
        self.rooms.remove_incomplete_expansion_room(room);
    }

    /// Port of `removeAllDoors(ExpansionRoom)` (AutorouteEngine.java:602-615): "removes all doors
    /// from room."
    pub fn remove_all_doors(&mut self, room: RoomRef) {
        self.rooms.remove_all_doors(room);
    }

    /// Port of `invalidateDrillPages(TileShape)` (AutorouteEngine.java:597-600): "invalidates all
    /// drill pages intersecting with shape so they must be recalculated at the next call of
    /// `getDrills()`."
    ///
    /// Its two call sites are [`Self::remove_complete_expansion_room`] (`:411`) and Task 9's
    /// `RoutingBoard.additionalUpdateAfterChange` (RoutingBoard.java:107).
    ///
    /// A shape that misses the board's bounding box invalidates **nothing**:
    /// `DrillPageArray::overlapping_pages` intersects with the bounds before it walks the grid
    /// (DrillPageArray.java:79).
    /// Each invalidated page hands its drill ids back to `rooms.drills` — see
    /// [`DrillPage::invalidate`](crate::autoroute::drill::DrillPage::invalidate) for why that is
    /// the reclamation Java's collector performs rather than a divergence.
    pub fn invalidate_drill_pages(&mut self, shape: &TileShape) {
        // :599. Two disjoint fields of `self`: the grid and the drill arena.
        self.drill_page_array
            .invalidate(shape, &mut self.rooms.drills);
    }

    /// `drillPageArray` (`:56`) for a read.
    pub fn drill_pages(&self) -> &DrillPageArray {
        &self.drill_page_array
    }

    /// `drillPageArray` (`:56`) for a write.
    pub fn drill_pages_mut(&mut self) -> &mut DrillPageArray {
        &mut self.drill_page_array
    }

    /// `drillPage.getDrills(autorouteEngine, ctrl.attachSmdAllowed)`
    /// (MazeExpansionEngine.java:148-150) — the **borrow bridge**, and the one method in this
    /// file that is not a port of a Java member.
    ///
    /// Java's `DrillPage.getDrills(AutorouteEngine, boolean)` is a method on an object the engine
    /// owns (`:56`) that takes the engine, so a Rust caller would need `&mut` on the page and
    /// `&mut` on its owner at the same time. This moves the page grid out of the array, runs
    /// `getDrills`, and puts it back — including on an unwind, because
    /// `DrillPage::get_drills` panics where Java throws (quirk #168) and an engine left with an
    /// empty grid would then fail its **next** `overlappingPages` instead.
    ///
    /// Nothing reachable from `getDrills` touches the grid: the only Java writers are
    /// `invalidateDrillPages` and `resetAllDoors`, whose callers are `initConnection`,
    /// `removeCompleteExpansionRoom`, `RoutingBoard.additionalUpdateAfterChange` and
    /// `autorouteConnection` — none of which `completeExpansionRoom` can reach.
    ///
    /// **This `catch_unwind` is not a sixth recovery boundary** (plan-6 ruling 7 fixes five). It
    /// recovers nothing: it restores one field and calls `resume_unwind`, so the panic and every
    /// observable effect are exactly what they would be without it — a `Drop` guard written as a
    /// `finally`. Ruling 7's five boundaries are the sites that *degrade to a value*; Task 18's
    /// audit should count those, not lexical occurrences.
    pub fn drill_page_drills(
        &mut self,
        board: &mut Board,
        page: PageId,
        attach_smd: bool,
        stop: StopCheck<'_>,
    ) -> Vec<DrillId> {
        let (i, j) = self.drill_page_array.split(page);
        let mut pages = self.drill_page_array.take_pages();
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
            pages[j][i].get_drills(self, board, attach_smd, stop)
        }));
        self.drill_page_array.restore_pages(pages);
        match outcome {
            Ok(drills) => drills,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Port of `generateRoomIdNo()` (AutorouteEngine.java:671-674): `++expansionRoomInstanceCount`,
    /// so the **first** id handed out is 1.
    ///
    /// The counter ticks once per `SortedRoomNeighbours.calculate` call, where it is an argument
    /// evaluated before the method knows whether a complete room will be built at all — so room
    /// ids skip. See [`ExpansionRoomStore::next_room_id_no`].
    pub fn generate_room_id_no(&mut self) -> i32 {
        self.rooms.next_room_id_no()
    }

    /// `ExpandableObject.getId()` (ExpandableObject.java:32-33) dispatched over the four
    /// implementors, which is what `MazeListElement.compareTo` (MazeListElement.java:95-96)
    /// performs as a virtual call.
    ///
    /// It lives on the engine because none of the four ids is a property of the reference alone:
    /// `ExpansionDoor.getId` needs both rooms, `TargetItemExpansionDoor.getId` its item and its
    /// room, `ExpansionDrill.getId` its own location and layers, and `DrillPage.getId` its shape
    /// and its **mutable** `netNumber` (quirk #167) — the last two out of the drill arena and the
    /// [`DrillPageArray`], which only the engine owns. `MazeQueue` closes a resolver over this.
    ///
    /// # Panics
    ///
    /// On a stale reference. Java holds a live object at every call site of `compareTo`, so a
    /// `None` here is a port bug (an arena slot freed while a `MazeListElement` still names it),
    /// not a Java behaviour to reproduce.
    pub fn expandable_id_no(&self, object: ExpandableRef) -> i32 {
        match object {
            ExpandableRef::Door(door) => self
                .rooms
                .door_id_no(door)
                .expect("ExpansionDoor.getId: a live door with two live rooms"),
            ExpandableRef::TargetDoor(door) => self
                .rooms
                .target_door_id_no(door)
                .expect("TargetItemExpansionDoor.getId: a live target door"),
            ExpandableRef::Drill(drill) => self
                .rooms
                .drills
                .get(drill.0)
                .expect("ExpansionDrill.getId: a live drill")
                .get_id(),
            ExpandableRef::Page(page) => self.drill_page_array.page(page).get_id(),
        }
    }

    /// `ExpandableObject.getShape()` (ExpandableObject.java:16) dispatched over the four
    /// implementors — the virtual call `MazeExpansionEngine.java:39`,
    /// `FoundConnectionLocator45Degree.java:50` and `:306` and
    /// `FoundConnectionLocatorAnyAngle.java:46` and `:58` all make.
    ///
    /// `None` is Java's `NullPointerException` on a door whose rooms have no shape, or a stale
    /// reference.
    pub fn expandable_shape(&self, object: ExpandableRef) -> Option<TileShape> {
        match object {
            ExpandableRef::Door(door) => self.rooms.door_shape(door),
            ExpandableRef::TargetDoor(door) => {
                Some(self.rooms.target_door(door)?.get_shape().clone())
            }
            ExpandableRef::Drill(drill) => {
                Some(self.rooms.drills.get(drill.0)?.get_shape().clone())
            }
            ExpandableRef::Page(page) => {
                Some(TileShape::Box(self.drill_page_array.page(page).shape))
            }
        }
    }

    /// `ExpandableObject.getDimension()` (ExpandableObject.java:13) dispatched over the four
    /// implementors — `MazeSearchEngine.java:461`, `MazeRipupResolver.java:220` and
    /// `FoundConnectionLocator45Degree.java:52`/`:308`.
    ///
    /// The three non-door implementors answer the constant 2
    /// (TargetItemExpansionDoor.java:39-42, ExpansionDrill.java:99-102, DrillPage.java's twin);
    /// a stale door reference answers 0, where Java would have held a live object.
    pub fn expandable_dimension(&self, object: ExpandableRef) -> i32 {
        match object {
            ExpandableRef::Door(door) => self.rooms.door(door).map_or(0, |door| door.dimension),
            ExpandableRef::TargetDoor(_) | ExpandableRef::Drill(_) | ExpandableRef::Page(_) => 2,
        }
    }

    /// `ExpandableObject.getMazeSearchElement(int)` (ExpandableObject.java:35-36) dispatched over
    /// the four implementors, which is what `MazeSearchEngine.occupyNextElement` performs as a
    /// virtual call at `MazeSearchEngine.java:332`, `:342-346` and `:382`.
    ///
    /// It lives on the engine for the same reason [`Self::expandable_id_no`] does: two of the four
    /// arrays are reachable only through the drill arena and the [`DrillPageArray`], which only
    /// the engine owns.
    ///
    /// `None` is Java's throw — an `ExpansionDoor` whose section array is still `null`
    /// (`ExpansionDoor.java:99-102`), an index past the end, or a stale reference. A
    /// `TargetItemExpansionDoor` ignores the index entirely (TargetItemExpansionDoor.java:55-58),
    /// and so a negative one is `None` here where Java answers the single element; that cannot
    /// arise, because `MazeListElement.sectionNoOfDoor` is only ever a loop index or `0`.
    pub fn maze_search_element(
        &self,
        object: ExpandableRef,
        section: i32,
    ) -> Option<&MazeSearchElement> {
        let index = usize::try_from(section).ok()?;
        match object {
            ExpandableRef::Door(door) => self.rooms.door(door)?.get_maze_search_element(index),
            ExpandableRef::TargetDoor(door) => {
                Some(self.rooms.target_door(door)?.get_maze_search_element(index))
            }
            ExpandableRef::Drill(drill) => {
                let drill = self.rooms.drills.get(drill.0)?;
                (index < drill.maze_search_element_count())
                    .then(|| drill.get_maze_search_element(index))
            }
            ExpandableRef::Page(page) => {
                let page = self.drill_page_array.page(page);
                (index < page.maze_search_element_count())
                    .then(|| page.get_maze_search_element(index))
            }
        }
    }

    /// `ExpandableObject.otherRoom(CompleteExpansionRoom)` (ExpandableObject.java:22) dispatched
    /// over the four implementors, which is what `MazeSearchEngine.expandToDoorSection` performs
    /// as a virtual call at `MazeSearchEngine.java:851`.
    ///
    /// It lives here for the same reason [`Self::maze_search_element`] does. `None` is Java's
    /// `null`, which three of the four implementors answer unconditionally
    /// (TargetItemExpansionDoor.java:50-53, ExpansionDrill.java:104-107, DrillPage.java:184-187);
    /// only `ExpansionDoor` (`:78-92`) has a room on the other side, and it narrows an incomplete
    /// one back to `null`.
    pub fn expandable_other_room(&self, object: ExpandableRef, room: RoomRef) -> Option<RoomRef> {
        match object {
            ExpandableRef::Door(door) => self.rooms.door(door)?.other_complete_room(room),
            ExpandableRef::TargetDoor(door) => self.rooms.target_door(door)?.other_room(room),
            ExpandableRef::Drill(drill) => self.rooms.drills.get(drill.0)?.other_room(room),
            ExpandableRef::Page(page) => self.drill_page_array.page(page).other_room(room),
        }
    }

    /// `ExpandableObject.mazeSearchElementCount()` (ExpandableObject.java:31) dispatched over the
    /// four implementors — `MazeSearchEngine.shoveTraceRoom`'s virtual call at
    /// `MazeSearchEngine.java:1132`.
    ///
    /// `None` is the unallocated `sectionArr` an `ExpansionDoor` throws on
    /// (ExpansionDoor.java:95-97) or a stale reference; the other three always answer.
    pub fn maze_search_element_count(&self, object: ExpandableRef) -> Option<usize> {
        match object {
            ExpandableRef::Door(door) => self.rooms.door(door)?.maze_search_element_count(),
            ExpandableRef::TargetDoor(door) => {
                Some(self.rooms.target_door(door)?.maze_search_element_count())
            }
            ExpandableRef::Drill(drill) => {
                Some(self.rooms.drills.get(drill.0)?.maze_search_element_count())
            }
            ExpandableRef::Page(page) => {
                Some(self.drill_page_array.page(page).maze_search_element_count())
            }
        }
    }

    /// [`maze_search_element`](Self::maze_search_element), mutably — Java's callers write the
    /// element's public fields through the reference they hold.
    pub fn maze_search_element_mut(
        &mut self,
        object: ExpandableRef,
        section: i32,
    ) -> Option<&mut MazeSearchElement> {
        let index = usize::try_from(section).ok()?;
        match object {
            ExpandableRef::Door(door) => self
                .rooms
                .door_mut(door)?
                .get_maze_search_element_mut(index),
            ExpandableRef::TargetDoor(door) => Some(
                self.rooms
                    .target_door_mut(door)?
                    .get_maze_search_element_mut(index),
            ),
            ExpandableRef::Drill(drill) => {
                let drill = self.rooms.drills.get_mut(drill.0)?;
                if index >= drill.maze_search_element_count() {
                    return None;
                }
                Some(drill.get_maze_search_element_mut(index))
            }
            ExpandableRef::Page(page) => {
                let page = self.drill_page_array.page_mut(page);
                if index >= page.maze_search_element_count() {
                    return None;
                }
                Some(page.get_maze_search_element_mut(index))
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // The room lifecycle
    // ---------------------------------------------------------------------------------------

    /// Port of `removeCompleteExpansionRoom(CompleteFreeSpaceExpansionRoom)`
    /// (AutorouteEngine.java:376-412): "removes a complete expansion room from the database and
    /// creates new incomplete expansion rooms for the neighbours."
    ///
    /// Answers whether there was a room **in the arena** to remove; Java's method is `void`.
    ///
    /// The `bool` is `ExpansionRoomStore::remove_complete_room`'s result, i.e. *arena* presence —
    /// **not** membership of `completeExpansionRooms`. The list removal at `:406` is
    /// unconditional here, so an abandoned room of `docs/java-quirks.md` #165 (in the arena,
    /// never in the list) still answers `true`, and only a stale [`RoomId`] answers `false`.
    /// Java's `completeExpansionRooms == null` branch (`:407-410`) is an `FRLogger.warn` and is
    /// dropped; it has no counterpart in the return value, because the port's list is never
    /// null.
    pub fn remove_complete_expansion_room(&mut self, board: &mut Board, room: RoomId) -> bool {
        let room_ref = RoomRef::Complete(room);
        // :379-381.
        let Some(room_shape) = self.rooms.room_shape(room_ref).cloned() else {
            // Java dereferences the shape at `:389` and NPEs; a stale id is Java's dead
            // reference, which cannot arise from `completeExpansionRooms`.
            return false;
        };
        let room_layer = match self.rooms.complete_room(room) {
            Some(r) => r.get_layer(),
            None => return false,
        };

        // :382-402. Java iterates `room.getDoors()` while the loop body mutates *other* rooms'
        // door lists (`:387`) and appends to `incompleteExpansionRooms` (`:397`) — never this
        // room's own list, which `:403` clears afterwards — so a snapshot is the same traversal.
        let room_doors: Vec<DoorId> = self.rooms.room_doors(room_ref).to_vec();
        for current_door in room_doors {
            // :383-386, and **the overload matters**. `room` is declared
            // `CompleteFreeSpaceExpansionRoom` here, so `currentDoor.otherRoom(room)` binds the
            // narrowing `otherRoom(CompleteExpansionRoom)` overload (ExpansionDoor.java:78-92),
            // which answers `null` for an *incomplete* neighbour — not the
            // `otherRoom(ExpansionRoom)` overload (`:62-72`) that `completeExpansionRoom` and
            // `removeAllDoors` bind. So every incomplete neighbour is skipped by `:385`, and it
            // keeps its door to this room until `removeAllDoors` at `:403` — which *does* use the
            // wide overload — unlinks it and removes the room outright.
            //
            // `completeNeighbourRooms` casts its argument back to `(ExpansionRoom)` at `:578` for
            // exactly this reason and says so in a comment; there is no such cast here.
            // See `docs/java-quirks.md` #164.
            let Some(current_neighbour) = self
                .rooms
                .door(current_door)
                .and_then(|d| d.other_complete_room(room_ref))
            else {
                continue;
            };
            // :387.
            self.rooms
                .remove_door(current_neighbour, ExpandableRef::Door(current_door));
            // :388-389.
            let Some(neighbour_shape) = self.rooms.room_shape(current_neighbour).cloned() else {
                // Java NPEs at `:389` on a room whose shape is the documented `null`.
                continue;
            };
            let intersection = room_shape.intersection(&neighbour_shape);
            // :390.
            if intersection.dimension() != 1 {
                continue;
            }
            // :391-395. "Add a new incomplete room to currentNeighbour."
            let touching_sides = room_shape
                .touching_sides(&neighbour_shape)
                .unwrap_or_else(|| {
                    panic!(
                        "AutorouteEngine.removeCompleteExpansionRoom: a 1-dimensional \
                         intersection with no touching sides — Java throws an \
                         ArrayIndexOutOfBoundsException at AutorouteEngine.java:394, because \
                         TileShape.touchingSides answers `new int[0]` (TileShape.java:588-591)"
                    )
                });
            let border_line = neighbour_shape
                .border_line(touching_sides[1])
                .unwrap_or_else(|| {
                    panic!(
                        "AutorouteEngine.removeCompleteExpansionRoom: the neighbour has no border \
                         line {} — Java NPEs at AutorouteEngine.java:394",
                        touching_sides[1]
                    )
                })
                .opposite();
            // `Simplex.getInstance(Line[])` (Simplex.java:37-47), **not** `TileShape.getInstance`:
            // `:395` keeps the simplex rather than simplifying it to a box or an octagon.
            let new_incomplete_room_shape =
                TileShape::Simplex(Simplex::from_lines(vec![border_line]));
            // :396-397.
            let new_incomplete_room = self.add_incomplete_expansion_room(
                Some(new_incomplete_room_shape),
                room_layer,
                Some(intersection),
            );
            // :398-400.
            let new_door = self.rooms.new_door(
                current_neighbour,
                RoomRef::Incomplete(new_incomplete_room),
                1,
            );
            self.rooms.add_door(current_neighbour, new_door);
            self.rooms
                .add_door(RoomRef::Incomplete(new_incomplete_room), new_door);
        }

        // :403.
        self.remove_all_doors(room_ref);
        // :404. `ExpansionRoomStore::remove_complete_room` performs the tree removal and the
        // arena removal in one operation, which is what makes `MinAreaTree.removeLeaf`'s
        // quirk-#39 branch unreachable (plan-2 ruling 8). Java's `:406` list removal follows.
        let removed = {
            let tree = tree_mut(board, self.tree);
            self.rooms.remove_complete_room(tree, room)
        };
        // :405-410. Java's `else` branch is an `FRLogger.warn` for a null list; dropped. A room
        // that is not in the list is Java's `ArrayList.remove` answering false, which it ignores.
        self.complete_expansion_rooms.retain(|r| *r != room);
        // :411.
        self.invalidate_drill_pages(&room_shape);
        removed
    }

    /// Port of `completeExpansionRoom(IncompleteFreeSpaceExpansionRoom)`
    /// (AutorouteEngine.java:414-522): "completes the shape of room. Returns the resulting rooms
    /// after completing the shape. `room` will no longer exist after this function."
    ///
    /// # Plan-6 ruling 7's first recovery boundary, and what Java actually degrades to
    ///
    /// The whole body sits in a `try` whose `catch (Exception)` (`:518-521`) logs and returns
    /// **`new ArrayList<>()`** — a fresh, empty collection. `result` is declared *inside* the try
    /// (`:422`), so the rooms completed before the throw are **not** returned; they stay in
    /// `completeExpansionRooms` and in the search tree, because those are side effects rather
    /// than the return value. The task brief and the controller's note both describe the catch as
    /// returning "the rooms completed so far"; Java does not, and Java wins.
    ///
    /// So `Err` here *is* Java's empty collection: `complete_expansion_room(..).unwrap_or_default()`
    /// reproduces `:520` exactly, and every caller in Tasks 11-16 must write it that way rather
    /// than propagating. The boundary is a [`std::panic::catch_unwind`], because the exceptions
    /// this catch exists for are `NullPointerException`s inside ported geometry, which the port
    /// raises as panics (ruling 7).
    //
    // obligation: `AutorouteEngine.completeExpansionRoom` — Tasks 11-16 must consume this with
    // `.unwrap_or_default()` (or a `match` answering an empty collection), **never** with `?`.
    // `Err` is not a failure to propagate: it is Java's `return new ArrayList<>()` at
    // AutorouteEngine.java:520, and propagating it would abort a connection Java completes.
    // `docs/java-quirks.md` #166 carries the reasoning.
    pub fn complete_expansion_room(
        &mut self,
        board: &mut Board,
        room: IncompleteRoomId,
    ) -> Result<Vec<RoomId>, RouterError> {
        // AutorouteEngine.java:421 / :518-521.
        match std::panic::catch_unwind(AssertUnwindSafe(|| {
            self.complete_expansion_room_inner(board, room)
        })) {
            Ok(rooms) => Ok(rooms),
            Err(payload) => Err(RouterError::Panicked(panic_message(&payload))),
        }
    }

    /// The body of the `try` at AutorouteEngine.java:421-517.
    fn complete_expansion_room_inner(
        &mut self,
        board: &mut Board,
        room: IncompleteRoomId,
    ) -> Vec<RoomId> {
        let room_ref = RoomRef::Incomplete(room);
        // :422.
        let mut result: Vec<RoomId> = Vec::new();

        // :423-434. The first door to an existing **complete free-space** room whose dimension is
        // 2 supplies both `fromDoorShape` and `ignoreObject`; the loop breaks on it.
        let mut from_door_shape: Option<TileShape> = None;
        let mut ignore_object: Option<TreeObject> = None;
        for current_door in self.rooms.room_doors(room_ref).to_vec() {
            let Some(door) = self.rooms.door(current_door) else {
                continue;
            };
            let other_room = door.other_room(room_ref);
            let dimension = door.get_dimension();
            // :428-429: `otherRoom instanceof CompleteFreeSpaceExpansionRoom && dimension == 2`.
            if let Some(RoomRef::Complete(free_room)) = other_room
                && dimension == 2
            {
                // :430-431.
                from_door_shape = self.rooms.door_shape(current_door);
                ignore_object = Some(TreeObject::Room(free_room));
                // :432.
                break;
            }
        }
        // :435-448 and :453-467 are `FRLogger.trace` payloads; both are dropped.

        // :449-450.
        let completed_shapes = self.complete_shape(board, room, ignore_object, &from_door_shape);

        // :469. Note the order: `completeShape` above has already read the room's doors.
        self.remove_incomplete_expansion_room(room);

        // :470-516.
        let mut is_first_completed_room = true;
        for current_incomplete_room in completed_shapes {
            // :472-474.
            let dimension = current_incomplete_room
                .get_shape()
                .unwrap_or_else(|| {
                    panic!(
                        "AutorouteEngine.completeExpansionRoom: a completed shape with no shape — \
                         Java NPEs at AutorouteEngine.java:472"
                    )
                })
                .dimension();
            if dimension != 2 {
                continue;
            }
            if is_first_completed_room {
                // :475-491. Only this one is added directly.
                is_first_completed_room = false;
                if let Some(completed_room) = self.add_complete_room(board, current_incomplete_room)
                {
                    result.push(completed_room);
                }
            } else {
                // :492-515. "The shape of the first completed room may have changed and may
                // intersect now with the other shapes. Therefore, the completed shapes have to be
                // recalculated."
                let recalculated = {
                    let tmp = self
                        .rooms
                        .incomplete_rooms
                        .insert(current_incomplete_room.clone());
                    let out = self.complete_shape(
                        board,
                        IncompleteRoomId(tmp),
                        ignore_object,
                        &from_door_shape,
                    );
                    // The candidate is not in Java's `incompleteExpansionRooms` — it is a local
                    // that `completeShape` returned — so the arena slot it needed in order to be
                    // named goes straight back out. `Arena::insert` appends and never reuses a
                    // slot, so the hole is inert.
                    self.rooms.incomplete_rooms.remove(tmp);
                    out
                };
                for tmp_room in recalculated {
                    // :510-513.
                    if let Some(completed_room) = self.add_complete_room(board, tmp_room) {
                        result.push(completed_room);
                    }
                }
            }
        }
        // :517.
        result
    }

    /// `this.autorouteSearchTree.completeShape(room, this.netNumber, ignoreObject, fromDoorShape)`
    /// (AutorouteEngine.java:450 and `:497-498`), with the room read back out of the arena and the
    /// three lookups the port's `completeShape` needs assembled from the board.
    fn complete_shape(
        &self,
        board: &Board,
        room: IncompleteRoomId,
        ignore_object: Option<TreeObject>,
        from_door_shape: &Option<TileShape>,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom> {
        let incomplete = self.rooms.incomplete_room(room).unwrap_or_else(|| {
            panic!(
                "AutorouteEngine.completeExpansionRoom: incomplete room {room:?} is not in the \
                 arena — Java holds a live reference and would dereference it at \
                 AutorouteEngine.java:425"
            )
        });
        let ctx = board.ctx();
        tree_of(board, self.tree).complete_shape(
            incomplete,
            self.net_number,
            ignore_object,
            from_door_shape.as_ref(),
            &board.items,
            &self.rooms,
            &ctx,
        )
    }

    /// Port of the private `addCompleteRoom(IncompleteFreeSpaceExpansionRoom)`
    /// (AutorouteEngine.java:524-545): "calculates the doors and adds the completed room to the
    /// room database."
    ///
    /// # The one container Java has two of
    ///
    /// Java hands `calculateDoors` a room that is **not** in `incompleteExpansionRooms` — it is a
    /// local that `completeShape` returned. The port's sorters name a room by its arena index, so
    /// the candidate is put in the incomplete arena for the duration of the call and taken out
    /// again; `Arena::insert` appends and never reuses a slot, so nothing else can observe it.
    ///
    /// The `null` return at `:528-530` leaves a `CompleteFreeSpaceExpansionRoom` that Java built
    /// but never added to `completeExpansionRooms`, and it is reachable: on probe mode 1 three of
    /// the nine rooms `calculateNeighbours` constructs never reach the list. They stay in the
    /// port's arena, because the arena is Java's heap — which is why
    /// [`Self::complete_expansion_rooms`] exists as a separate list rather than being the arena's
    /// key set. See that field's docs.
    fn add_complete_room(
        &mut self,
        board: &mut Board,
        room: IncompleteFreeSpaceExpansionRoom,
    ) -> Option<RoomId> {
        // :526-527.
        let temporary = self.rooms.incomplete_rooms.insert(room);
        let completed_room =
            self.calculate_doors(board, RoomRef::Incomplete(IncompleteRoomId(temporary)));
        self.rooms.incomplete_rooms.remove(temporary);

        // :528-530. Java's cast to `CompleteFreeSpaceExpansionRoom` cannot fail here: the input is
        // an incomplete room, so `calculateNeighbours` took the `:190-193` branch.
        let RoomRef::Complete(completed_room) = completed_room? else {
            return None;
        };
        let dimension = self
            .rooms
            .complete_room(completed_room)
            .and_then(|r| r.get_shape())
            .map(TileShape::dimension)?;
        if dimension != 2 {
            return None;
        }

        // :531-534.
        self.complete_expansion_rooms.push(completed_room);
        // :535.
        {
            let tree = tree_mut(board, self.tree);
            self.rooms.insert_complete_room(tree, completed_room);
        }
        // :536-543 is an `FRLogger.trace`; dropped.
        Some(completed_room)
    }

    /// Port of the private `calculateDoors(ExpansionRoom)` (AutorouteEngine.java:555-561):
    /// "calculates the neighbours of room and inserts doors to the new created neighbour rooms.
    /// The shape of the result room may be different to the shape of room."
    fn calculate_doors(&mut self, board: &mut Board, room: RoomRef) -> Option<RoomRef> {
        // :560.
        SortedRoomNeighbours::complete(room, self.net_number, board, &mut self.rooms, self.tree)
    }

    /// Port of `completeNeighbourRooms(CompleteExpansionRoom)` (AutorouteEngine.java:563-592):
    /// "completes the shapes of the neighbour rooms of room, so that the doors of room will not
    /// change later on."
    ///
    /// `:573-584` **restarts the door iterator after every completed neighbour**, because
    /// completing one mutates `room`'s door list — Java's own comment says so ("keep v1.9
    /// semantics"). The port cannot hold a borrow across the call either, so it re-reads the door
    /// list each round and re-checks the door is still live.
    ///
    /// Java's `room.getDoors() == null` guard (`:568-570`) has no counterpart: the port's door
    /// lists are always allocated.
    pub fn complete_neighbour_rooms(&mut self, board: &mut Board, room: RoomRef) {
        let mut index = 0usize;
        loop {
            // `it.hasNext()` / `it.next()` over the **current** door list (`:574-575`).
            let doors = self.rooms.room_doors(room).to_vec();
            let Some(current_door) = doors.get(index).copied() else {
                return;
            };
            index += 1;

            // :576-581. Java casts to `ExpansionRoom` "because `ExpansionDoor.otherRoom` works
            // differently with parameter type `CompleteExpansionRoom`", which is the
            // [`crate::ExpansionDoor::other_room`] overload the port already takes.
            let Some(neighbour_room) = self
                .rooms
                .door(current_door)
                .and_then(|d| d.other_room(room))
            else {
                continue;
            };
            match neighbour_room {
                // :582-584.
                RoomRef::Incomplete(free_room) => {
                    // Java discards the returned collection, and so does the degraded value the
                    // recovery boundary answers in its place (`:520`).
                    let _ = self.complete_expansion_room(board, free_room);
                    // `it = room.getDoors().iterator()` — back to the first door.
                    index = 0;
                }
                // :585-589.
                RoomRef::Obstacle(obstacle_neighbour_room) => {
                    let all_doors_calculated = self
                        .rooms
                        .obstacle_room(obstacle_neighbour_room)
                        .is_some_and(|r| r.all_doors_calculated());
                    if !all_doors_calculated {
                        self.calculate_doors(board, neighbour_room);
                        if let Some(r) = self.rooms.obstacle_room_mut(obstacle_neighbour_room) {
                            r.set_doors_calculated(true);
                        }
                    }
                }
                // A complete free-space neighbour is neither `instanceof` branch: Java falls off
                // the end of the `if` and moves on.
                RoomRef::Complete(_) => {}
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Queries and the between-connection reset
    // ---------------------------------------------------------------------------------------

    /// Port of the package-private `getRoomsWithTargetItems(Set<Item>)`
    /// (AutorouteEngine.java:617-634): "returns all complete free space expansion rooms with a
    /// target door to an item in the set `items`."
    ///
    /// Java's `TreeSet<CompleteFreeSpaceExpansionRoom>` sorts by
    /// `CompleteFreeSpaceExpansionRoom.compareTo`, which is `other.id - this.id`, i.e.
    /// **descending** by room id. The port answers a `BTreeSet<RoomId>` over arena indices, which
    /// are minted in the same creation order as the ids, so **Java's iteration order is
    /// `.rev()`** (plan-2 ruling 14's third rule). Callers that reproduce a Java loop over this
    /// set must iterate it backwards.
    pub fn rooms_with_target_items(&self, items: &BTreeSet<ItemId>) -> BTreeSet<RoomId> {
        let mut result = BTreeSet::new();
        // :622-631.
        for current_room in &self.complete_expansion_rooms {
            let Some(room) = self.rooms.complete_room(*current_room) else {
                continue;
            };
            for current_target_door in room.get_target_doors() {
                let Some(door) = self.rooms.target_door(*current_target_door) else {
                    continue;
                };
                if items.contains(&door.item) {
                    result.insert(*current_room);
                }
            }
        }
        result
    }

    /// Port of `validate()` (AutorouteEngine.java:636-648): "checks if the internal datastructure
    /// is valid."
    ///
    /// Java's `completeExpansionRooms == null` early `true` (`:638-640`) is the empty arena. Note
    /// that it does **not** short-circuit: every room is validated even after one has failed,
    /// because `CompleteFreeSpaceExpansionRoom.validate` is where the diagnostics come from.
    pub fn validate(&self, board: &Board) -> bool {
        let mut result = true;
        // :642-646.
        for current_room in &self.complete_expansion_rooms {
            let Some(room) = self.rooms.complete_room(*current_room) else {
                continue;
            };
            if !room.validate(self, board, *current_room) {
                result = false;
            }
        }
        result
    }

    /// Port of the private `resetAllDoors()` (AutorouteEngine.java:650-668): "resets all doors for
    /// autorouting the next connection in case the autorouting database is retained."
    ///
    /// `:662` is the **only** site in the whole autoroute package that uses
    /// `Item.getAutorouteInfoPur()` — the nullable accessor that creates nothing — and both calls
    /// underneath it (`resetDoors()` at `:664` and `setPrecalculatedConnection(null)` at `:665`)
    /// go through that same non-creating reference. Reaching for `getAutorouteInfo()` here would
    /// allocate scratch on every item Java skips, so the port writes through
    /// `Item::get_autoroute_info_pur_mut` (the plan-1 obligation naming this method).
    ///
    /// `pub` where Java's is `private`: its Java caller is `autorouteConnection` (`:271-274`),
    /// which is Task 16's, so there is nothing inside the crate to call it yet.
    pub fn reset_all_doors(&mut self, board: &mut Board) {
        // :655-659.
        let complete = self.complete_expansion_rooms.clone();
        for room in complete {
            self.rooms.reset_doors(RoomRef::Complete(room));
        }

        // :660-667, in `board.getItems()` order (descending id, quirk #63).
        for item in board.items_in_board_order() {
            // :662-663: the non-creating fetch and its null guard.
            if board
                .get_item(item)
                .and_then(|i| i.get_autoroute_info_pur())
                .is_none()
            {
                continue;
            }
            // :664.
            let rooms = &mut self.rooms;
            item_info::reset_doors(board, item, |_, obstacle_room| {
                rooms.reset_doors(RoomRef::Obstacle(obstacle_room));
            });
            // :665.
            if let Some(info) = board
                .get_item_mut(item)
                .and_then(|i| i.get_autoroute_info_pur_mut())
            {
                info.precalculated_connection = None;
            }
        }

        // :668. Two disjoint fields of `self`: the grid and the drill arena.
        self.drill_page_array.reset(&mut self.rooms.drills);
    }

    /// Port of `autorouteConnection(Set<Item>, Set<Item>, AutorouteControl, SortedSet<Item>,
    /// Map<Item,Integer>)` (AutorouteEngine.java:130-280): "auto-routes a connection between
    /// `startSet` and `destSet`".
    ///
    /// `ripped` is Java's `SortedSet<Item> rippedItemList`, i.e. a `TreeSet<Item>` ordered by
    /// `Item.compareTo` (`Item.java:95-102`, `other.id - this.id`) and therefore **descending by
    /// id** (quirk #44): the `:247` loop over it, and `describeConnection`'s two joins, are
    /// written `.rev()` here. `ripup_costs` is the optional `Map<Item,Integer>` the locator's
    /// `backtrack` (`:260`, `:319`) fills and null-checks.
    ///
    /// # The three recovery boundaries this method owns
    ///
    /// Plan-6 ruling 7 fixes five `catch (Exception)` sites; three of them are here — `:139`
    /// (maze construction), `:157` (`findConnection`) and `:190`
    /// ([`FoundConnectionLocator::get_instance`]) — and each degrades to a *value*, never to a
    /// propagated error. The port's currency for Java's exception is a panic in ported geometry,
    /// so each is a [`std::panic::catch_unwind`] whose `Err` is the same `null` Java's `catch`
    /// assigns. `AutorouteEngine.completeExpansionRoom:518` is the fourth (Task 6's) and
    /// [`route_connection`] carries the fifth.
    ///
    /// # What is *not* caught
    ///
    /// `:260-266` — `board.removeItems`, `removeTraceTails` and
    /// [`FoundConnectionInserter::get_instance`] — sits **outside** every one of Java's four
    /// `try` blocks, so a throw there reaches `AutorouteConnectionRouter.route:155-158`'s bare
    /// `FAILED`. The port's two `Result` channels there are turned into a panic naming that
    /// handler, which [`route_connection`]'s boundary then converts to the same bare `FAILED` —
    /// and, as in Java, *without* running the necked retry, which a returned `FAILED` would
    /// enable (`AutorouteConnectionRouter.java:123-125`).
    ///
    /// # Not ported
    ///
    // not ported: `AutorouteEngine.autorouteConnection` — the observer bracketing of `:254-258`
    // and `:268-270` (`global-constraints.md` forbids board observers) and the
    // `ctrl.netNumber == 33 || 66 || 67` `FRLogger.trace` block of `:163-177` (plan-6 ruling 14,
    // quirk #158).
    #[allow(clippy::too_many_arguments)]
    pub fn autoroute_connection(
        &mut self,
        board: &mut Board,
        start: &BTreeSet<ItemId>,
        dest: &BTreeSet<ItemId>,
        ctrl: &AutorouteControl,
        ripped: &mut BTreeSet<ItemId>,
        ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
        stop: StopCheck<'_>,
    ) -> AutorouteAttemptResult {
        self.autoroute_connection_impl(board, start, dest, ctrl, ripped, ripup_costs, stop, false)
    }

    /// [`Self::autoroute_connection`] with ruling 7's boundary #4 (`AutorouteEngine.java:190`)
    /// forced, so that `:215-219`'s message-less `FAILED` — which has no other trigger — can be
    /// pinned against the JVM.
    ///
    /// **Test-only, and named so that a production call reads as the mistake it would be.** The
    /// JVM's lever is `P6T16Probe`'s `locatorfail` mode: an unmodifiable `rippedItemList`, whose
    /// `add` throws inside `backtrack:318`. A `BTreeSet` cannot refuse an insert, so the port
    /// cannot reproduce that lever and injects the panic instead.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn autoroute_connection_with_forced_locator_failure(
        &mut self,
        board: &mut Board,
        start: &BTreeSet<ItemId>,
        dest: &BTreeSet<ItemId>,
        ctrl: &AutorouteControl,
        ripped: &mut BTreeSet<ItemId>,
        ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
        stop: StopCheck<'_>,
        force: bool,
    ) -> AutorouteAttemptResult {
        self.autoroute_connection_impl(board, start, dest, ctrl, ripped, ripup_costs, stop, force)
    }

    /// [`Self::autoroute_connection`] with ruling 7's boundary #4 forced.
    ///
    /// `panic_in_locator` has no Java counterpart and no production caller: it exists because
    /// `:215-219`'s degraded `FAILED` is reachable **only** through the `:190` catch.
    /// `FoundConnectionLocator::get_instance` itself answers `None` for exactly one input — a
    /// null `mazeSearchResult` — which `:180` has already excluded, so on the JVM the only lever
    /// is an exception, and `P6T16Probe`'s `locatorfail` mode pulls it by handing
    /// `autorouteConnection` an **unmodifiable** `rippedItemList` (`backtrack:318` calls `add`
    /// on it). A `BTreeSet` cannot refuse an insert, so the port cannot reproduce that lever and
    /// injects the panic instead; the value asserted against is the probe's.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn autoroute_connection_impl(
        &mut self,
        board: &mut Board,
        start: &BTreeSet<ItemId>,
        dest: &BTreeSet<ItemId>,
        ctrl: &AutorouteControl,
        ripped: &mut BTreeSet<ItemId>,
        ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
        stop: StopCheck<'_>,
        panic_in_locator: bool,
    ) -> AutorouteAttemptResult {
        // The two sets' `Item.toString()`s, taken **before** anything can remove an item.
        // Java's sets are `Set<Item>`, so every `describeConnection` below prints a live
        // reference — including a trace `:260` ripped or the inserter split away. See
        // [`describe_connection`]'s doc comment for the measurement.
        let start_names = connection_item_names(board, start);
        let dest_names = connection_item_names(board, dest);

        // :136-161 — ruling 7's boundaries #2 (`:139`, the maze construction) and #3 (`:157`,
        // `findConnection`), which have to be **nested** rather than sequential: a
        // `MazeSearchEngine` borrows this engine for its whole life, so it cannot be carried out
        // of the first `catch_unwind` and into the second. The two are still distinct, because
        // the inner catch fires first and the outer one never sees that panic:
        //
        // * `None` — boundary #2 tripped, or `getInstance` answered `null` on its own. Both are
        //   Java's `mazeSearchAlgo == null` (`:142`, `:145`).
        // * `Some(None)` — the engine was built and boundary #3 tripped, or `findConnection`
        //   answered `null`. Both are Java's `searchResult == null`.
        //
        // Boundary #3 is not theoretical: `DrillPage.getDrills` panics when the stop flag trips
        // inside `splitToConvex` (Java NPEs on `drillShapes.length`, DrillPage.java:108), which
        // is the one production path that reaches it. `P6T16Probe`'s `stopafter` mode pins it.
        let maze_outcome: Option<Option<MazeResult>> = {
            let engine: &mut AutorouteEngine = self;
            let board: &mut Board = board;
            std::panic::catch_unwind(AssertUnwindSafe(move || {
                // :138.
                let mut maze_search_algo =
                    MazeSearchEngine::get_instance(start, dest, engine, board, ctrl, stop)?;
                // :153-161.
                Some(
                    std::panic::catch_unwind(AssertUnwindSafe(|| {
                        maze_search_algo.find_connection(board, stop)
                    }))
                    .unwrap_or(None),
                )
            }))
            .unwrap_or(None)
        };

        // :145-151. Note this returns **before** the cleanup of `:198-205`, where every later
        // early return runs it: a connection whose maze could not be built leaves the rooms and
        // their leaves in the compensated autoroute tree exactly as `initConnection` left them.
        // The asymmetry is Java's and is transcribed rather than tidied, but it is **latent**:
        // `MazeSearchEngine::get_instance` answers `None` only when `init` fails, which is
        // before any room has been completed. Measured on the JVM by `P6T16Probe` mode `nomaze`,
        // whose **free-angle row** — the only one of the three that reaches this return; the
        // other two get here through `:207-213` instead — prints `completeRooms n=0` and
        // `treeSize=4` (the four board items) after the failure. That one row is why it earns no
        // quirk row.
        let Some(search_result) = maze_outcome else {
            return AutorouteAttemptResult::with_details(
                AutorouteAttemptState::Failed,
                format!(
                    "Failed to route connection between {}, because the maze search algorithm \
                     could not be created.",
                    describe_connection_from_names(&start_names, &dest_names)
                ),
            );
        };

        // :179-196 — ruling 7's boundary #4.
        let autoroute_result = match search_result.as_ref() {
            None => None,
            Some(search_result) => {
                let engine: &mut AutorouteEngine = self;
                let angle_restriction = board.rules.trace_angle_restriction;
                let board: &mut Board = board;
                let ripped: &mut BTreeSet<ItemId> = ripped;
                std::panic::catch_unwind(AssertUnwindSafe(move || {
                    assert!(
                        !panic_in_locator,
                        "AutorouteEngine.autoroute_connection: the injected \
                         FoundConnectionLocator.get_instance failure of plan-6 ruling 7's fourth \
                         recovery boundary (AutorouteEngine.java:190)"
                    );
                    // :183-189.
                    FoundConnectionLocator::get_instance(
                        Some(search_result),
                        ctrl,
                        engine,
                        board,
                        angle_restriction,
                        ripped,
                        ripup_costs,
                    )
                }))
                .unwrap_or(None)
            }
        };

        // :198-205. **Before every early return below.** Getting this order wrong leaks rooms
        // into the next connection and is invisible until a later fixture routes differently.
        if self.maintain_database {
            self.reset_all_doors(board);
        } else {
            self.clear(board);
        }

        // :207-213.
        if search_result.is_none() {
            return AutorouteAttemptResult::with_details(
                AutorouteAttemptState::Failed,
                format!(
                    "Failed to route connection between {}, because no connection was found \
                     between their nets.",
                    describe_connection_from_names(&start_names, &dest_names)
                ),
            );
        }

        // :215-219 — the boundary-#4 degradation, and the only `FAILED` message with no
        // "because" clause.
        let Some(autoroute_result) = autoroute_result else {
            return AutorouteAttemptResult::with_details(
                AutorouteAttemptState::Failed,
                format!(
                    "Failed to route connection between {}.",
                    describe_connection_from_names(&start_names, &dest_names)
                ),
            );
        };

        // :221-228. Reachable only for a **power plane** layer: a disabled *signal* layer makes
        // `MazeSearchEngine.expandToRoomDoors:396-399` refuse before any target door is expanded,
        // so the search answers null and `:207` fires instead. `P6T16Probe`'s `inactive` mode
        // builds the plane.
        if !ctrl.layer_active[autoroute_result.start_layer]
            || !ctrl.layer_active[autoroute_result.target_layer]
        {
            return AutorouteAttemptResult::with_details(
                AutorouteAttemptState::Failed,
                format!(
                    "Failed to route connection between {}, because some of their layers are \
                     disabled.",
                    describe_connection_from_names(&start_names, &dest_names)
                ),
            );
        }

        // :230-235 is **dead code** and cannot be ported as a branch: `connectionItems` is a
        // `final` field assigned `new LinkedList<>()` at `FoundConnectionLocator.java:101`,
        // before both of the constructor's early returns, so Java's definite-assignment rule
        // makes the `== null` test unsatisfiable. Its two warn branches leave an **empty** list,
        // which the inserter walks zero times. Recorded as quirk #180 by Task 14, which pinned
        // `connectionItems n=0` on both branches; the `SKIPPED` state therefore has
        // no producer anywhere in the autoroute path.
        // Java bug: `AutorouteEngine.autorouteConnection`'s `connectionItems == null` arm
        // (`:230-235`) is unreachable — see docs/java-quirks.md #180.

        // :237-245. "Delete the ripped connections."
        let mut ripped_connections: BTreeSet<ItemId> = BTreeSet::new();
        let mut changed_nets: BTreeSet<i32> = BTreeSet::new();
        // obligation: `AutorouteEngine.autorouteConnection`'s `:241-245` `StopConnectionOption`
        // choice — **discharged in Task 17**. Task 16 measured that inverting it left all 14 probe
        // modes identical, because those fixtures build `RouterSettings::new()` (fanout disabled,
        // so `removeUnconnectedVias` is `true` and the option is `None`) and none of them has a
        // **fanout via**. Task 17's driver builds its settings from `DefaultSettings`, where
        // `fanout.enabled` is `true` (`DefaultSettings.java:117`), so
        // `removeUnconnectedVias = !isFanoutEnabled()` is **false** and every one of the 311 corpus
        // connections that reaches `:241-245` takes the `FanoutVia` arm — the one no unit fixture
        // took — and matches the HEAD jar byte for byte. The other 58 of the corpus's 369 are
        // **55** that return `NO_UNCONNECTED_NETS` at `AutorouteConnectionRouter.route:49-52` plus
        // **3** that take `:207-213`'s message-carrying `FAILED` ("no connection was found"):
        // `router-rpi-splitter` k = 2 and `router-dac2020-bm01` k = 120 and k = 126. What a corpus board still cannot show is the *difference*
        // between the two arms, since `getConnectionItems`/`removeTraceTails` branch on the option
        // only for a fanout via (Item.java:735, RoutingBoard.java:1207-1216) and `BatchFanout` is
        // Plan 7's.
        let stop_connection_option = if ctrl.remove_unconnected_vias {
            StopConnectionOption::None
        } else {
            StopConnectionOption::FanoutVia
        };

        // :247-252, over Java's `TreeSet<Item>` order — descending id.
        // obligation: `AutorouteEngine.autorouteConnection`'s `:247` loop order — **discharged
        // in Task 17**. No fixture in `tests/autoroute_connection.rs` rips more than **one** item,
        // so reversing this loop left all 14 probe modes byte-identical.
        // `router-dac2020-bm01` rips **two** items at k = 252 and **three** at k = 261, 279, 286
        // and 293 (fifteen multi-rip connections in all, plus one on `router-j2-reference`), and
        // every one of them matches the HEAD jar's ripped set, per-item ripup costs and inserted
        // geometry. The order still cannot be *isolated* by mutation — `rippedConnections` is a
        // set and `changedNets` a sorted set, so it reaches the output only through
        // `getConnectionItems`' `FanoutVia` arm reading the partially built result
        // (Item.java:735), which needs a fanout via (Plan 7).
        for current_ripped_item in ripped.iter().rev() {
            ripped_connections
                .extend(board.connection_items(*current_ripped_item, stop_connection_option));
            // totalized: `:249`'s `currentRippedItem.netCount()` on an id the board no longer
            // holds. Java's `rippedItemList` is a set of live `Item` references, so it always
            // reads the count; the port skips the id. Unreachable today — nothing is removed
            // until `:260`, three statements below — and it is the same shape
            // `describe_connection` documents at its own site.
            let Some(item) = board.get_item(*current_ripped_item) else {
                continue;
            };
            for i in 0..item.net_count() {
                changed_nets.insert(item.get_net_number(i));
            }
        }

        // :260, over `rippedConnections`' own descending order.
        // obligation: `BasicBoard.removeItems`' iteration order here — **discharged in Task 17**
        // alongside `:247`'s. Task 16 had the same one-ripped-item ceiling, so ascending left
        // every probe mode identical. `router-dac2020-bm01` removes **two** connection items at
        // once seven times, three 22 times, four three times and **seven** once (and
        // `router-j2-reference` two once), all matching the HEAD jar. The order is load-bearing
        // in principle because `removeItem` refuses a deletion-forbidden item and the survivors'
        // contacts change as the loop runs.
        board.remove_items(ripped_connections.iter().rev().copied());

        // :262-263, over `changedNets`' ascending `TreeSet<Integer>` order.
        for current_net_number in &changed_nets {
            if let Err(error) =
                board.remove_trace_tails(*current_net_number, stop_connection_option)
            {
                panic!(
                    "AutorouteEngine.autoroute_connection:263: removeTraceTails failed ({error}) \
                     — Java's throw here is caught only by \
                     AutorouteConnectionRouter.route:155-158"
                );
            }
        }

        // :265-266. No `catch` encloses this call: an `Err` is a Java throw and belongs to
        // `route:155-158`, while `Ok(None)` is Java's `null` and belongs to `:271-277`.
        //
        // **The stop check is `&|| false`, not `stop`** (controller ruling AC). Java tests
        // cancellation **nowhere** below `:265`: plan-6 ruling 6's six sites are all in
        // `MazeSearchEngine.init` / the pop loop plus `DrillPage.java:103`, and
        // `FoundConnectionInserter.getInstance`, `ForcedViaInserter.insert`,
        // `BasicBoard.insertVia`, `BasicBoard.splitTraces` and `PolylineTrace.split` carry no
        // test at HEAD. Handing the caller's `stop` down here would make an external cancel that
        // trips *after* the search returns abort an insert Java completes — after `:260` has
        // already removed the ripped items — so the port would emit a board worse than either
        // outcome plus a bare `FAILED`. Exact parity wins.
        //
        // The price is that **quirk #76's ladder hang becomes reachable from the router's insert
        // path, exactly as it is in Java**: `PolylineTrace.split`'s entry re-walk does not
        // terminate on a four-rung ladder, and the `StopCheck` plan-3 ruling F added to
        // `Board::split_traces_checked` is what a caller would have used to escape it. That
        // check stays for `fr-dsn`'s reader, which is the caller ruling F was written about. The
        // **wall clock is the batch loop's**: `AutorouteConnectionRouter.route:71-74` builds a
        // per-connection `TimeLimit` that `AutorouteEngine.isStopRequested` consults at ruling
        // 6's six sites, and Plan 7's `AutorouteBatchLoop` owns everything above that.
        let insert_found_connection_algo = FoundConnectionInserter::get_instance(
            Some(&autoroute_result),
            board,
            ctrl,
            Some(self),
            &|| false,
        );
        match insert_found_connection_algo {
            Err(error) => panic!(
                "AutorouteEngine.autoroute_connection:265: FoundConnectionInserter.getInstance \
                 failed ({error}) — Java's throw here is caught only by \
                 AutorouteConnectionRouter.route:155-158"
            ),
            // :271-277.
            // obligation: `AutorouteEngine.autorouteConnection`'s `:271-277` arm — **discharged
            // in Task 17**. No unit fixture reached it: Task 15 pinned
            // `FoundConnectionInserter::get_instance` answering `None` on three boards
            // (`P6T15Probe`'s `viafail`), but every lever that produces it from *outside*
            // `autorouteConnection` — an empty `ctrl.viaRule`, a user-fixed via on the drill —
            // also changes what the maze search finds, because `ForcedViaInserter.check` reads
            // the same rule. Task 17's corpus reaches it on a real board, on **20** connections
            // in all: `router-rpi-splitter` k = 3 and 8; `router-j2-reference` k = 6, 9, 10, 12,
            // 15 and 19; `router-dac2020-bm01` k = 23, 25, 26, 124, 125, 127, 162, 202, 246, 287,
            // 288 and 290 — each with this exact message, matched word for word against the HEAD
            // jar. `router-j2-reference` k = 19 is also the connection that proved
            // `describe_connection` had to snapshot its names (see that function).
            Ok(None) => AutorouteAttemptResult::with_details(
                AutorouteAttemptState::Failed,
                format!(
                    "Failed to route connection between {}, because the new connection could not \
                     be inserted.",
                    describe_connection_from_names(&start_names, &dest_names)
                ),
            ),
            // :279.
            Ok(Some(_)) => AutorouteAttemptResult::new(AutorouteAttemptState::Routed),
        }
    }
}

// =================================================================================================
// Helpers
// =================================================================================================

/// `this.autorouteSearchTree` (`:47`) for a read.
pub(crate) fn tree_of(board: &Board, tree_id: TreeId) -> &ShapeSearchTree {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == tree_id)
        .unwrap_or_else(|| panic!("AutorouteEngine: no search tree with id {tree_id:?}"))
}

/// `this.autorouteSearchTree` (`:47`) for a write — `insert` (`:535`) and `removeFromTree`
/// (`:310`, `:404`).
pub(crate) fn tree_mut(board: &mut Board, tree_id: TreeId) -> &mut ShapeSearchTree {
    board
        .trees
        .trees_mut()
        .find(|tree| tree.id() == tree_id)
        .unwrap_or_else(|| panic!("AutorouteEngine: no search tree with id {tree_id:?}"))
}

/// The text of a caught panic, so that the degraded `AutorouteAttemptResult` of ruling 7's later
/// boundaries can carry Java's `FRLogger.error` payload.
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "AutorouteEngine.complete_expansion_room: a panic with no message".to_string()
    }
}

/// Port of the private static `describeConnection(Set<Item>, Set<Item>)`
/// (AutorouteEngine.java:282-287) — the `", "`-joined `Item.toString()` of each set, with
/// `" and "` between them.
///
/// Both sets are Java `TreeSet<Item>`s, so both joins run **descending by id**
/// (`Item.compareTo`, Item.java:95-102, is `other.id - this.id` — quirk #44).
///
/// `Item.toString` is [`fr_board::Item`]'s `Display`.
///
/// # The sets hold live references, so their names outlive the board
///
/// Java's sets are `Set<Item>` — object references taken at `AutorouteConnectionRouter.route:54-68`
/// — while the port's are `BTreeSet<ItemId>`. An item the connection *removes* (a ripped item at
/// `:260`, or a trace the inserter split or whose tail it deleted) is still printed by Java, whose
/// reference is alive, and would be silently dropped by an id lookup against the board as it
/// stands at `:271`. So the names are snapshotted at entry to
/// [`AutorouteEngine::autoroute_connection`] and every message is built from the snapshot by
/// the private `describe_connection_from_names`; this entry point resolves them against `board`
/// for a caller who has not mutated it.
///
/// Snapshotting is exactly equivalent to Java's late evaluation, because `Item.toString`
/// (Item.java:1258-1269) and `Pin.toString` (`:676-692`) read only `getClass().getSimpleName()`,
/// `componentId` and `pinIndex`, none of which a routing pass can change.
///
/// Measured: `router-j2-reference` k = 19 is the corpus connection that proves it. Its dest set is
/// `[946,945,944,936,935,43,42,37,36]` on both sides, but the failed insert removes one of the five
/// traces, so before this snapshot the port's `:271-277` message carried four `polylinetrace`s
/// where the JVM's carries five.
///
/// `pub` where Java's is `private static`: `P6T16Probe`'s `describe` mode reaches it by
/// reflection, and the port's test needs the same direct call — every other route to it is a
/// `FAILED` message, which a fixture that happens to route does not produce.
pub fn describe_connection(
    board: &Board,
    start_set: &BTreeSet<ItemId>,
    dest_set: &BTreeSet<ItemId>,
) -> String {
    describe_connection_from_names(
        &connection_item_names(board, start_set),
        &connection_item_names(board, dest_set),
    )
}

/// One set's `Item.toString()`s, in Java's descending-id `TreeSet<Item>` order — the snapshot
/// [`describe_connection`]'s doc comment explains.
pub(crate) fn connection_item_names(board: &Board, set: &BTreeSet<ItemId>) -> Vec<String> {
    set.iter()
        .rev()
        .filter_map(|id| board.get_item(*id))
        .map(ToString::to_string)
        .collect()
}

/// `describeConnection`'s two `", "` joins with `" and "` between them, over names already taken.
pub(crate) fn describe_connection_from_names(start: &[String], dest: &[String]) -> String {
    format!("{} and {}", start.join(", "), dest.join(", "))
}

/// Steps 1-5 of `AutorouteConnectionRouter.route(Item, int, SortedSet<Item>, Map<Item,Integer>,
/// int)` (AutorouteConnectionRouter.java:30-100) — the Plan 6 half of the seam (plan-6 ruling 2).
/// Steps 6-8 (`optChangedArea`, the necked retry and the strict-DRC rollback) are Plan 7's and
/// are marked as such at the end of this function.
///
/// # The parameters Java reads off `BatchAutorouter`
///
/// `route:38-47` reads five values off `router`, and **four of them are per-`BatchAutorouter`
/// fields, not functions of `RouterSettings`** — because there are two production constructors
/// and they disagree. `BatchAutorouter.java:110-121` (the `RoutingJob` one) derives
/// `removeUnconnectedVias = !settings.isFanoutEnabled()`, `traceCosts = getTraceCosts()` and
/// `startRipupCosts = settings.getStartRipupCosts()`; `:253-261`
/// (`autoroutePassesForOptimizingItem`, the optimizer's autorouter, which is Plan 7's
/// `BatchOptimizer` path) passes `removeUnconnectedVias = true` **unconditionally** and takes
/// `startRipupCosts` from its caller. So all four are parameters here — `trace_costs`,
/// `start_ripup_costs`, `remove_unconnected_vias` and `retain_autoroute_database` — and Plan 7
/// wires the second constructor without changing this signature.
///
/// The values the `RoutingJob` constructor uses are the documented defaults, and are what every
/// call site in `tests/autoroute_connection.rs` passes: `settings.get_start_ripup_costs()`,
/// `!settings.is_fanout_enabled()` and `false`. `retain_autoroute_database`
/// (`BatchAutorouter.isRetainAutorouteDatabase`, `:172-173`) is a **benchmark-only** system
/// property (`BatchAutorouter.java:63-64,151-154`; `BatchAutorouterThread.java:90` hard-codes
/// `false`), so it is `false` in every production and parity run. The fifth value,
/// `getTracePullTightAccuracy`, is read only by step 6, which is Plan 7's.
///
/// `RoutingBoard.finishAutoroute` is deliberately **absent**: Java calls it only from
/// `RoutingPipeline.java:110` and `RoutingBoardUndoFacade.java:55`, never from `route`.
///
/// # Deviation from the task brief: `engine` is an `Option`
///
/// `RoutingBoard.initAutoroute` (`:882-897`) reads *and writes* the nullable field
/// `RoutingBoard.autorouteEngine` — it reuses the stored engine when `retainAutorouteDatabase`
/// and the compensated clearance class match, and replaces it otherwise. `&mut Option<_>` **is**
/// that field; a `&mut AutorouteEngine` could not express the reuse test, and the write happens
/// before `autorouteConnection` runs, so it survives the boundary below exactly as Java's field
/// assignment survives a throw.
///
/// # Ruling 7's fifth recovery boundary
///
/// `:35`'s `try` / `:154-158`'s `catch (Exception e)` wraps the whole function and degrades to a
/// **bare** `FAILED` — `new AutorouteAttemptResult(FAILED)`, with no details, which is what tells
/// it apart from every message-carrying `FAILED` `autoroute_connection` produces. It is reachable
/// in production: `AutorouteControl::new` panics for a positive net the board does not have
/// (`AutorouteControl.java:219`, pinned by `P6T8Probe ctrl`), and both of
/// [`AutorouteEngine::autoroute_connection`]'s uncaught error channels end here.
// added in Plan 7: `AutorouteConnectionRouter.retryConnectionNecked` (AutorouteConnectionRouter
// .java:162), and with it steps 6-8 of `AutorouteConnectionRouter.route`.
// not ported: `AutorouteConnectionRouter.route`'s `router.setAirLine` (`:70`), a GUI progress
// sink, and its two `isBenchmarkProfileEnabled` timing blocks (`:84-87`).
#[allow(clippy::too_many_arguments)]
pub fn route_connection(
    board: &mut Board,
    engine: &mut Option<AutorouteEngine>,
    item: ItemId,
    net_no: i32,
    settings: &RouterSettings,
    trace_costs: &[ExpansionCostFactor],
    ripped: &mut BTreeSet<ItemId>,
    ripup_costs: &mut BTreeMap<ItemId, i32>,
    ripup_pass_no: i32,
    start_ripup_costs: i32,
    remove_unconnected_vias: bool,
    retain_autoroute_database: bool,
    stop: StopCheck<'_>,
) -> AutorouteAttemptResult {
    // :35, `:154-158` — ruling 7's fifth recovery boundary.
    std::panic::catch_unwind(AssertUnwindSafe(|| {
        route_connection_steps_1_to_5(
            board,
            engine,
            item,
            net_no,
            settings,
            trace_costs,
            ripped,
            ripup_costs,
            ripup_pass_no,
            start_ripup_costs,
            remove_unconnected_vias,
            retain_autoroute_database,
            stop,
        )
    }))
    // :155-158.
    .unwrap_or_else(|_| AutorouteAttemptResult::new(AutorouteAttemptState::Failed))
}

/// The body of [`route_connection`], i.e. `AutorouteConnectionRouter.route:36-90` inside its
/// `try`.
#[allow(clippy::too_many_arguments)]
fn route_connection_steps_1_to_5(
    board: &mut Board,
    engine: &mut Option<AutorouteEngine>,
    item: ItemId,
    net_no: i32,
    settings: &RouterSettings,
    trace_costs: &[ExpansionCostFactor],
    ripped: &mut BTreeSet<ItemId>,
    ripup_costs: &mut BTreeMap<ItemId, i32>,
    ripup_pass_no: i32,
    start_ripup_costs: i32,
    remove_unconnected_vias: bool,
    retain_autoroute_database: bool,
    stop: StopCheck<'_>,
) -> AutorouteAttemptResult {
    // :37-40.
    let route_net = board.rules.nets.get(net_no);
    let contains_plane = route_net.is_some_and(fr_board::rules::Net::contains_plane);
    let current_via_costs = if contains_plane {
        settings.get_plane_via_costs()
    } else {
        settings.get_via_costs()
    };

    // :42-47.
    let mut autoroute_control =
        AutorouteControl::new(board, net_no, settings, current_via_costs, trace_costs);
    autoroute_control.ripup_allowed = true;
    // obligation: `AutorouteConnectionRouter.route`'s `:45` `startRipupCosts * ripupPassNo` —
    // **discharged in Task 17 for `ripupPassNo` 1, 2 and 4, with one recorded XDIFF**. Task 16
    // measured that probe mode `routeripup`'s three passes all rip the same item at the same
    // cost, because `MazeRipupResolver`'s price saturates on a one-trace obstacle. Task 17 reran
    // the whole corpus at `ripupPassNo = 2` and `= 4` (`scripts/differential/run.sh p6t1 <dsn>
    // 100000 <pass>`): `router-rpi-splitter`, `router-j2-reference` and `router-ecc83-input`
    // MATCH at both, with ripup costs that differ from pass 1's, and pass 4 additionally
    // exercises `MazeRipupResolver`'s `randomize` draw (plan-6 ruling 5's bit-exact
    // `JavaRandom`, seeded with `ctrl.ripupCosts`). `router-dac2020-bm01` matches for
    // k = 1..266 at both passes and then diverges — see `crates/fr-router/README.md`
    // "The one open divergence" for the measurement.
    autoroute_control.ripup_costs = start_ripup_costs * ripup_pass_no;
    // obligation: `AutorouteConnectionRouter.route`'s `:46` `removeUnconnectedVias` —
    // **discharged in Task 17** together with `:241-245`'s `StopConnectionOption`, as Task 16
    // predicted. Task 16 measured that flipping the flag left every probe mode identical, because
    // those fixtures build `RouterSettings::new()`. Task 17's driver takes the value the
    // `RoutingJob` constructor computes (`!settings.isFanoutEnabled()`) from a `DefaultSettings`
    // table, where fanout is enabled — so the corpus runs the flag **false** on all 369
    // connections, the opposite of every unit fixture, and matches the HEAD jar throughout.
    autoroute_control.remove_unconnected_vias = remove_unconnected_vias;

    // :49-52.
    let unconnected_set = board.unconnected_set(item, net_no);
    if unconnected_set.is_empty() {
        return AutorouteAttemptResult::new(AutorouteAttemptState::NoUnconnectedNets);
    }

    // :54-68. Java's `getConnectedSet(int)` is the `stopAtPlane = false` overload
    // (Item.java:596-598).
    let connected_set = board.connected_set(item, net_no, false);
    let (route_start_set, route_dest_set) = if contains_plane {
        // :57-61, over the `TreeSet<Item>`'s descending id order.
        for current_item in connected_set.iter().rev() {
            if matches!(board.get_item(*current_item), Some(Item::ConductionArea(_))) {
                return AutorouteAttemptResult::new(AutorouteAttemptState::ConnectedToPlane);
            }
        }
        // :62-64 — the plane swap.
        (connected_set, unconnected_set)
    } else {
        // :65-68.
        (unconnected_set, connected_set)
    };

    // :71-74. `Math.min` before the `(int)` cast, so the product saturates at `Integer.MAX_VALUE`
    // rather than wrapping.
    let max_milliseconds = java_min(
        100_000.0 * f64::powf(2.0, f64::from(ripup_pass_no - 1)),
        f64::from(i32::MAX),
    );
    let time_limit = TimeLimit::new(max_milliseconds as i32);

    // :76-82. The write to `RoutingBoard.autorouteEngine` happens here, before the connection
    // runs, so it survives an unwind exactly as Java's field assignment survives a throw.
    *engine = Some(board.init_autoroute(
        engine.take(),
        net_no,
        autoroute_control.trace_clearance_class_index,
        Some(time_limit),
        retain_autoroute_database,
    ));
    let autoroute_engine = engine
        .as_mut()
        .expect("initAutoroute always answers an engine");

    // :88-90.
    autoroute_engine.autoroute_connection(
        board,
        &route_start_set,
        &route_dest_set,
        &autoroute_control,
        ripped,
        Some(ripup_costs),
        stop,
    )

    // added in Plan 7: steps 6-8 of `AutorouteConnectionRouter.route` — `optChangedArea`
    // (`:92-118`), the necked retry (`:120-145`) and `applyStrictDrcAfterRoute` (`:147-152`),
    // together with the `maxItemIdBeforeRoute` / `strictDrcBoardSnapshot` of `:83-85` that only
    // those three read.
}

// =================================================================================================
// The deferral roster for `autoroute/maze/AutorouteEngine.java`
// =================================================================================================

// `autorouteConnection` is the engine's other half — the maze search itself — and plan-6 ruling 2
// puts it at the top of this plan's scope, above everything Tasks 7-15 build. Task 16 landed it
// as `AutorouteEngine::autoroute_connection`, together with `describeConnection`
// (`describe_connection`) and steps 1-5 of `AutorouteConnectionRouter.route`
// (`route_connection`).
//
// not ported: `AutorouteEngine.emitDiagnostics` — it walks `AutorouteDiagnostic.Sink`, a GUI
// overlay (`global-constraints.md`: no GUI, no observers), and no routing decision reads it. Its
// `autorouteInfo != null` guard at `:330` is dead in Java as well, because `getAutorouteInfo()`
// never returns null.
//
// not ported: `AutorouteEngine.describeShapeBounds` — a private `FRLogger.trace` formatter
// (`:547-553`).
//
// `AutorouteConnectionRouter` is `autoroute/pipeline`'s, and plan-6 ruling 2 splits it at step 5;
// `scripts/audit-map/fr-router.map` maps the class here because steps 1-5 are this engine's entry
// point. Steps 1-5 are `route_connection` above; steps 6-8 and `retryConnectionNecked` stay
// Plan 7's.
// added in Plan 7: `AutorouteConnectionRouter.retryConnectionNecked`
