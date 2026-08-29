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

use std::collections::BTreeSet;
use std::panic::AssertUnwindSafe;

use fr_board::ids::TreeObject;
use fr_board::{Board, ItemId, RoomId, ShapeSearchTree, StopCheck, TimeLimit, TreeId};
use fr_geometry::{Simplex, TileShape};

use crate::arena::{DoorId, DrillId, IncompleteRoomId, PageId};
use crate::autoroute::drill::DrillPageArray;
use crate::autoroute::expansion::sorted_neighbours::SortedRoomNeighbours;
use crate::autoroute::expansion::{
    ExpandableRef, ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef,
};
use crate::autoroute::item_info;
use crate::autoroute::tree_ext::AutorouteSearchTreeExt;
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
            // added in Task 9: `RoutingBoard.additionalUpdateAfterChange` (RoutingBoard.java:96-118)
            // — plan-6 ruling 3 puts it on `RoutingBoardExt`, which Task 9 creates. Its body is
            // entirely engine work (invalidate the drill pages of every tree shape, then
            // `removeCompleteExpansionRoom` for every complete room overlapping one, then
            // `item.clearAutorouteInfo()`), so it needs Task 7's `DrillPageArray` as well as this
            // engine; writing it here would take both files from their owning tasks.
            //
            // obligation: Task 9 must call it from here, once per item of `net_number`, in
            // `board.getItems()` order (descending id, quirk #63):
            //
            //     for item in board.items_in_board_order() {
            //         if board.get_item(item).is_some_and(|i| i.contains_net(net_number)) {
            //             board.additional_update_after_change(self, item);
            //         }
            //     }
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

// =================================================================================================
// The deferral roster for `autoroute/maze/AutorouteEngine.java`
// =================================================================================================

// `autorouteConnection` is the engine's other half — the maze search itself — and plan-6 ruling 2
// puts it at the top of this plan's scope, above everything Tasks 7-15 build.
// added in Task 16: `AutorouteEngine.autorouteConnection`
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
// point.
// added in Plan 7: `AutorouteConnectionRouter.route`
// added in Plan 7: `AutorouteConnectionRouter.retryConnectionNecked`
