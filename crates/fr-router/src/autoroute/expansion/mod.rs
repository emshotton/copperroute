//! The `app.freerouting.autoroute.expansion` package: the rooms the maze search expands
//! through, the doors between them, and the two interfaces that unify them.
//!
//! # The shape of the port
//!
//! Java's object graph here is **cyclic** — a room holds its doors, a door holds both of its
//! rooms — so the port stores every room and every door in an [`Arena`] and refers to them by
//! index (plan-6 ruling 16). [`ExpansionRoomStore`] is the collection of those arenas: it is
//! what Java's heap plus `AutorouteEngine`'s three room lists add up to, and Task 6's
//! `AutorouteEngine` embeds one rather than re-declaring the arenas.
//!
//! What is a method on a Java room lives on the room struct if it needs only that room's own
//! state ([`FreeSpaceExpansionRoom::add_door`], [`ExpansionDoor::get_id`]), and on the store if
//! it needs the graph ([`ExpansionRoomStore::room_id_no`], which dispatches over the three
//! `getId()` implementations, or [`ExpansionRoomStore::door_shape`], which needs both rooms).
//!
//! # The five `getId()`s, and why none of them is an arena index
//!
//! | Object | Java | Formula |
//! |---|---|---|
//! | `CompleteFreeSpaceExpansionRoom` | `:99-102` | the engine counter — the only true id |
//! | `ObstacleExpansionRoom` | `:48-51` | `(itemId << 10) \| indexInItem` — aliases (quirk #156) |
//! | `IncompleteFreeSpaceExpansionRoom` | `:37-41` | `31 * shape.getId() + layer`, and the shape is mutable |
//! | `ExpansionDoor` | `:184-190` | `min(id1,id2) * 31 + max(id1,id2)` |
//! | `TargetItemExpansionDoor` | `:70-74` | `31 * item.getId() + room.getId()` |
//!
//! Four of the five are **hashes** (quirk #8's family) and all four overflow silently; each is
//! transcribed with its `wrapping_*`. They reach [`ExpansionDoor::get_id`], which is a sort key
//! of `MazeListElement` (plan-6 ruling 4), so none may be "cleaned up" before parity.

pub mod complete_room;
pub mod door;
pub mod free_space_room;
pub mod incomplete_room;
pub mod obstacle_room;
pub mod room;
pub mod sorted_neighbours;
pub mod sorted_neighbours_45;
pub mod sorted_neighbours_orthogonal;
pub mod target_door;

pub use complete_room::CompleteFreeSpaceExpansionRoom;
pub use door::ExpansionDoor;
pub use free_space_room::FreeSpaceExpansionRoom;
pub use incomplete_room::IncompleteFreeSpaceExpansionRoom;
pub use obstacle_room::ObstacleExpansionRoom;
pub use room::{ExpandableRef, RoomRef};
pub use sorted_neighbours::{
    CalculationMode, SortedRoomNeighbour, SortedRoomNeighbours, select_calculation_mode,
};
pub use sorted_neighbours_45::Sorted45DegreeRoomNeighbours;
pub use sorted_neighbours_orthogonal::SortedOrthogonalRoomNeighbours;
pub use target_door::{TargetItemExpansionDoor, target_door_id};

use fr_board::searchtree::ShapeSearchTree;
use fr_board::{Board, ItemId, ObstacleRoomId, RoomId, TreeId, TreeObject};
use fr_geometry::{FloatLine, TileShape};

use crate::Arena;
use crate::arena::{DoorId, IncompleteRoomId, TargetDoorId};
use crate::autoroute::drill::ExpansionDrill;

/// Every expansion room and door of one routing run, plus the room-id counter.
///
/// No single Java class corresponds: this is `AutorouteEngine`'s `incompleteExpansionRooms`
/// (AutorouteEngine.java:71) and `expansionRoomInstanceCount` (`:77`) together with the heap that
/// holds every complete room, obstacle room and door. It is **not**
/// `completeExpansionRooms` (`:74`): that list is a strict subset of the complete-room arena, it
/// lives on `AutorouteEngine`, and it is the container every walk Java writes over
/// `completeExpansionRooms` must use — see [`AutorouteEngine::complete_expansion_rooms`] and
/// `docs/java-quirks.md` #165.
///
/// Task 6's `AutorouteEngine` **embeds** one of these; it must not declare the arenas a second
/// time.
///
/// [`AutorouteEngine::complete_expansion_rooms`]:
///     crate::autoroute::maze::AutorouteEngine::complete_expansion_rooms
#[derive(Debug, Clone, Default)]
pub struct ExpansionRoomStore {
    /// Every `CompleteFreeSpaceExpansionRoom` ever constructed, which is Java's **heap** rather
    /// than its `AutorouteEngine.completeExpansionRooms` list (:74): `SortedRoomNeighbours`
    /// builds a room before it knows whether it will be kept, and both the `edgeRemoved` retry
    /// (SortedRoomNeighbours.java:111-114) and `AutorouteEngine.addCompleteRoom`'s
    /// dimension check (AutorouteEngine.java:528-530) discard one. Java's list is
    /// `AutorouteEngine::complete_expansion_rooms`; every walk Java writes over
    /// `completeExpansionRooms` must use that, not this arena.
    ///
    /// The arena index is the [`RoomId`] the shared search tree stores as a
    /// [`TreeObject::Room`].
    pub complete_rooms: Arena<CompleteFreeSpaceExpansionRoom>,
    /// `AutorouteEngine.incompleteExpansionRooms` (:71).
    pub incomplete_rooms: Arena<IncompleteFreeSpaceExpansionRoom>,
    /// The obstacle rooms, which Java reaches only through
    /// `ItemAutorouteInfo.expansionRoomArr` (ItemAutorouteInfo.java:20) — the array
    /// [`fr_board::AutorouteInfo`] holds as `Vec<Option<ObstacleRoomId>>`.
    pub obstacle_rooms: Arena<ObstacleExpansionRoom>,
    /// Every `ExpansionDoor`. Java has no list of them; each door is reachable from its two
    /// rooms, and the port needs one place to own them.
    pub doors: Arena<ExpansionDoor>,
    /// Every `TargetItemExpansionDoor`, likewise (Java reaches them through
    /// `CompleteFreeSpaceExpansionRoom.targetDoors`).
    pub target_doors: Arena<TargetItemExpansionDoor>,
    /// Every `ExpansionDrill` a `DrillPage` has built (`autoroute/drill/DrillPage.java:122-127`).
    ///
    /// Java has no such container: a drill is owned by the page's `drills` list. The port needs
    /// one place to own them because a drill is an `ExpandableObject`, so the maze search stores
    /// it in a `MazeSearchElement.backtrackDoor` as a bare index
    /// ([`crate::arena::DrillId`]) — the same reason the doors live here.
    ///
    /// [`Self::clear`] deliberately leaves this arena alone: `AutorouteEngine.clear`
    /// (AutorouteEngine.java:306-317) does not touch `drillPageArray` either, so a page's
    /// memoised drills survive it in Java too. See that method's docs for what that costs.
    ///
    /// Slots **are** released, but by the page that owns them: `DrillPage::invalidate` and
    /// `get_drills`' recompute path hand their ids back, which is where Java's collector
    /// reclaims the objects. Without that the arena would grow once per changed item over a
    /// whole routing run; with it, the live count tracks the pages' lists exactly
    /// (`crates/fr-router/tests/drill.rs`'s `invalidating_a_page_frees_its_drills_arena_slots`).
    pub drills: Arena<ExpansionDrill>,
    /// `AutorouteEngine.expansionRoomInstanceCount` (:77).
    room_instance_count: i32,
    /// Whether `AutorouteEngine.incompleteExpansionRooms` (`:71`) is **non-null**.
    ///
    /// Java's field starts `null` and is created lazily by the *first*
    /// `addIncompleteExpansionRoom` (`:342-345`); `clear` (`:314`) sets it back to `null`. The
    /// arena above is the list's contents, so this flag is all that is left of the null-ness —
    /// and the null-ness is observable, because `removeIncompleteExpansionRoom` (`:368-371`)
    /// dereferences the field with no guard. See
    /// [`Self::remove_incomplete_expansion_room`] and `docs/java-quirks.md` #169.
    incomplete_list_created: bool,
}

/// The store is the room lookup the room-bearing search-tree queries take: a complete room's
/// `getTreeShape`/`shapeLayer` are its own shape and layer
/// (CompleteFreeSpaceExpansionRoom.java:66-74), and this is where they live.
///
/// A stale [`RoomId`] answers `None`, which is Java's dead reference; the tree turns that into
/// the `NullPointerException` Java would have thrown.
impl fr_board::RoomLookup for ExpansionRoomStore {
    fn room_tree_shape(&self, id: RoomId) -> Option<&TileShape> {
        self.complete_rooms.get(id.0)?.get_tree_shape(0)
    }

    fn room_shape_layer(&self, id: RoomId) -> Option<usize> {
        Some(self.complete_rooms.get(id.0)?.shape_layer(0))
    }
}

impl ExpansionRoomStore {
    /// An empty store — the state `AutorouteEngine`'s constructor leaves (all three room lists
    /// `null`, the counter 0).
    pub fn new() -> ExpansionRoomStore {
        ExpansionRoomStore::default()
    }

    /// The body of `AutorouteEngine.generateRoomIdNo` (AutorouteEngine.java:672-674):
    /// `return ++expansionRoomInstanceCount`, so the **first** id handed out is 1, not 0.
    ///
    /// The counter ticks once per `SortedRoomNeighbours.calculate` call
    /// (SortedRoomNeighbours.java:104), where it is an *argument*, evaluated before the method
    /// knows whether a complete room will be built at all: the obstacle-room branch (`:193`)
    /// discards it, and the `edgeRemoved` retry (`:111-114`) recurses and takes another. So room
    /// ids **skip**, and consecutive rooms do not generally carry consecutive ids. They are
    /// still strictly increasing in creation order, which is what makes [`RoomId`] — the arena
    /// index — order the search tree the same way Java's id does.
    ///
    /// Task 6's `AutorouteEngine::generate_room_id_no` delegates here. The name deliberately
    /// differs from Java's: `audit-port.sh` scopes `AutorouteEngine` to
    /// `autoroute/maze/engine.rs`, so the obligation to write the engine method stays open
    /// until Task 6 writes it there, rather than being discharged from this file.
    pub fn next_room_id_no(&mut self) -> i32 {
        self.room_instance_count = self.room_instance_count.wrapping_add(1);
        self.room_instance_count
    }

    /// `AutorouteEngine.clear` (AutorouteEngine.java:306-317) minus its last line: **take every
    /// complete room out of the search tree first** (`:308-312`), then drop every room and door
    /// and reset the counter (`:313-315`).
    ///
    /// The tree removal is not optional. Java's loop over `completeExpansionRooms` calling
    /// `currentRoom.removeFromTree(this.autorouteSearchTree)` is what stops the shared tree from
    /// keeping `TreeObject::Room` leaves after the rooms are gone; without it the next overlap
    /// query would read a room key whose arena slot no longer exists — and, until Task 4 teaches
    /// `ShapeSearchTree`'s `tree_shape_of`/`ignore_object` to resolve a room, that is a **panic**
    /// rather than a stale read.
    ///
    /// **Every id handed out before this call becomes meaningless**, because [`Arena::clear`]
    /// restarts the indices — including the [`ObstacleRoomId`]s stored on the board's items.
    ///
    /// obligation: `AutorouteEngine.clear` must follow this with
    /// `RoutingBoard.clearAllItemTemporaryAutorouteData` (`RoutingBoard.java:1241`), which is
    /// AutorouteEngine.java:316 — otherwise the items keep `ObstacleRoomId`s into a restarted
    /// arena. **Discharged in Task 6**:
    /// [`AutorouteEngine::clear`](crate::autoroute::maze::AutorouteEngine::clear) calls it as its
    /// last statement, and `clear_empties_the_room_database_the_tree_and_the_items_scratch`
    /// (`crates/fr-router/tests/engine_rooms.rs`) asserts the item scratch is gone afterwards.
    ///
    /// This method also walks the **arena**, not `AutorouteEngine`'s
    /// `completeExpansionRooms` list, so it visits the abandoned rooms of quirk #165 as well.
    /// That is equivalent, not sloppy: an abandoned room was never inserted into the tree
    /// (`AutorouteEngine.addCompleteRoom:535` is the only insert), so its `removeFromTree` is the
    /// no-op Java's `MinAreaTree.removeLeaf` performs for a null leaf.
    pub fn clear(&mut self, tree: &mut ShapeSearchTree) {
        // AutorouteEngine.java:308-312.
        for (_, room) in self.complete_rooms.iter_mut() {
            room.remove_from_tree(tree);
        }
        self.complete_rooms.clear();
        self.incomplete_rooms.clear();
        self.obstacle_rooms.clear();
        self.doors.clear();
        self.target_doors.clear();
        self.room_instance_count = 0;
        // AutorouteEngine.java:314: `incompleteExpansionRooms = null`.
        self.incomplete_list_created = false;
        // `self.drills` is deliberately **not** cleared: `AutorouteEngine.clear` (`:306-317`)
        // does not touch `drillPageArray`, so in Java the pages keep their memoised drills and
        // those drills keep references to rooms that have just left the tree. The port's ids go
        // stale in a different way — `Arena::clear` restarts the room indices, so a kept
        // `RoomRef` would alias a *different* room rather than a dead one — but the only caller
        // is `RoutingBoard.finishAutoroute` (`:899-905`), which drops the engine on the next
        // line, so neither the stale references nor the aliases are ever read.
    }

    // --- construction ---------------------------------------------------------------------

    /// `new CompleteFreeSpaceExpansionRoom(shape, layer, id)`
    /// (SortedRoomNeighbours.java:192, CompleteFreeSpaceExpansionRoom.java:34-38), placed in the
    /// arena.
    ///
    /// `id` is a value [`next_room_id_no`](Self::next_room_id_no) handed out; the returned
    /// [`RoomId`] is the arena index, which is a different number (see
    /// [`next_room_id_no`](Self::next_room_id_no)). The room learns its own index so it can
    /// answer `getObject()` and remove its own tree entry.
    pub fn new_complete_room(&mut self, shape: Option<TileShape>, layer: usize, id: i32) -> RoomId {
        // `Arena::insert` always appends, so this is the index it is about to return.
        let room_id = RoomId(
            u32::try_from(self.complete_rooms.slot_count())
                .expect("expansion-room arena index overflowed u32"),
        );
        let index = self
            .complete_rooms
            .insert(CompleteFreeSpaceExpansionRoom::new(
                shape, layer, id, room_id,
            ));
        debug_assert_eq!(index, room_id.0);
        room_id
    }

    /// The whole of `AutorouteEngine.addIncompleteExpansionRoom(TileShape, int, TileShape)`
    /// (AutorouteEngine.java:341-350): allocate the room, **create the list if it is null**
    /// (`:343-345`) and append.
    ///
    /// The arena is the list's contents, so the append is the insert; the lazy creation is the
    /// [`incomplete_list_created`](Self::incomplete_list_created) flag, and it is not decoration
    /// — see [`remove_incomplete_expansion_room`](Self::remove_incomplete_expansion_room).
    ///
    /// Use [`new_unlisted_incomplete_room`](Self::new_unlisted_incomplete_room) where Java calls
    /// the **constructor** directly instead of this method.
    pub fn new_incomplete_room(
        &mut self,
        shape: Option<TileShape>,
        layer: usize,
        contained_shape: Option<TileShape>,
    ) -> IncompleteRoomId {
        // AutorouteEngine.java:343-345.
        self.incomplete_list_created = true;
        self.new_unlisted_incomplete_room(shape, layer, contained_shape)
    }

    /// `new IncompleteFreeSpaceExpansionRoom(shape, layer, containedShape)`
    /// (IncompleteFreeSpaceExpansionRoom.java:18-22) — the bare constructor, **without**
    /// `addIncompleteExpansionRoom`'s list append.
    ///
    /// Its one Java caller is `ExpansionDrill.calculateExpansionRooms` (ExpansionDrill.java:
    /// 76-77), which builds a room it hands straight to `completeExpansionRoom` and never puts
    /// in the engine's list. The port cannot express "in the heap but not in the list" for
    /// incomplete rooms — the arena is both — so what this preserves is the part that *is*
    /// observable: the list's null-ness, which `removeIncompleteExpansionRoom` reads.
    pub fn new_unlisted_incomplete_room(
        &mut self,
        shape: Option<TileShape>,
        layer: usize,
        contained_shape: Option<TileShape>,
    ) -> IncompleteRoomId {
        IncompleteRoomId(
            self.incomplete_rooms
                .insert(IncompleteFreeSpaceExpansionRoom::new(
                    shape,
                    layer,
                    contained_shape,
                )),
        )
    }

    /// Whether `AutorouteEngine.incompleteExpansionRooms` (`:71`) is non-null — see the field.
    pub fn incomplete_list_created(&self) -> bool {
        self.incomplete_list_created
    }

    /// `new ObstacleExpansionRoom(item, indexInItem, tree)` (ItemAutorouteInfo.java:78,
    /// ObstacleExpansionRoom.java:26-31) — the closure
    /// [`crate::autoroute::item_info::get_expansion_room`] takes.
    pub fn new_obstacle_room(
        &mut self,
        board: &mut Board,
        item: ItemId,
        index_in_item: usize,
        tree: TreeId,
    ) -> ObstacleRoomId {
        let room = ObstacleExpansionRoom::new(board, item, index_in_item, tree);
        ObstacleRoomId(self.obstacle_rooms.insert(room))
    }

    /// `new ExpansionDoor(firstRoom, secondRoom, dimension)` (ExpansionDoor.java:28-32).
    ///
    /// Linking the door into the two rooms is a separate step in Java too
    /// (`ObstacleExpansionRoom.createOverlapDoor:96-98` does the three calls in a row), so this
    /// only allocates; use [`add_door`](Self::add_door) for each side.
    pub fn new_door(
        &mut self,
        first_room: RoomRef,
        second_room: RoomRef,
        dimension: i32,
    ) -> DoorId {
        DoorId(
            self.doors
                .insert(ExpansionDoor::new(first_room, second_room, dimension)),
        )
    }

    /// `new ExpansionDoor(firstRoom, secondRoom)` (ExpansionDoor.java:35-39), whose dimension is
    /// the dimension of the two rooms' intersection.
    ///
    /// `None` where Java throws a `NullPointerException`: one of the two rooms is not in the
    /// arena, or has the null shape `IncompleteFreeSpaceExpansionRoom`'s constructor documents
    /// (`:14-16`).
    pub fn new_door_from_shapes(
        &mut self,
        first_room: RoomRef,
        second_room: RoomRef,
    ) -> Option<DoorId> {
        let first_shape = self.room_shape(first_room)?.clone();
        let second_shape = self.room_shape(second_room)?.clone();
        let door = ExpansionDoor::new_with_computed_dimension(
            first_room,
            second_room,
            &first_shape,
            &second_shape,
        );
        Some(DoorId(self.doors.insert(door)))
    }

    /// `new TargetItemExpansionDoor(item, treeEntryNo, room, searchTree)`
    /// (TargetItemExpansionDoor.java:20-32). Java's null room is `None`.
    pub fn new_target_door(
        &mut self,
        board: &mut Board,
        item: ItemId,
        tree_entry_no: usize,
        room: Option<RoomRef>,
        tree: TreeId,
    ) -> TargetDoorId {
        let room_shape = room.and_then(|r| self.room_shape(r)).cloned();
        let door = TargetItemExpansionDoor::new(
            board,
            item,
            tree_entry_no,
            room,
            room_shape.as_ref(),
            tree,
        );
        TargetDoorId(self.target_doors.insert(door))
    }

    // --- the search tree ------------------------------------------------------------------

    /// The `this.autorouteSearchTree.insert(completedRoom)` half of
    /// `AutorouteEngine.addCompleteRoom` (AutorouteEngine.java:534).
    ///
    /// A room that is not in the arena, or has no shape, is not inserted — Java would have NPE'd
    /// on the shape at `ShapeTree.insert`'s `boundingShape` call.
    ///
    /// Calling this twice for the same room would leak the first leaf, exactly as Java's
    /// `insert(Storable)` would overwrite `treeEntries` and orphan the old array. Java's one
    /// caller (`AutorouteEngine.addCompleteRoom:534`) runs once per room, and so must this one.
    pub fn insert_complete_room(&mut self, tree: &mut ShapeSearchTree, room: RoomId) {
        let Some(shape) = self
            .complete_rooms
            .get(room.0)
            .and_then(|r| r.get_shape())
            .cloned()
        else {
            return;
        };
        let leaf = tree.insert_room(room, &shape);
        if let Some(r) = self.complete_rooms.get_mut(room.0) {
            r.set_search_tree_entries(leaf);
        }
    }

    /// Takes a complete room **out of the tree and out of the arena in one operation**, and
    /// answers whether there was one to take.
    ///
    /// This is `CompleteFreeSpaceExpansionRoom.removeFromTree` (`:56-59`) plus
    /// the arena half of `completeExpansionRooms.remove(room)` (AutorouteEngine.java:406) — the
    /// two halves Java performs in the same method, which is exactly why Java can never hand an
    /// already-removed leaf to `MinAreaTree.removeLeaf` and trip quirk #39's silent tree
    /// corruption. `fr-board`'s `ShapeTree::remove_leaf` panics there instead (Plan 2 ruling 8),
    /// so making the two halves inseparable is what keeps that panic unreachable: a second call
    /// finds no room and answers `false`.
    ///
    /// It is **not** `AutorouteEngine.removeCompleteExpansionRoom` (`:377-412`), which also
    /// regenerates incomplete rooms for the 1-dimensional neighbours and invalidates the drill
    /// pages; that is Task 6's, and it ends with this.
    pub fn remove_complete_room(&mut self, tree: &mut ShapeSearchTree, room: RoomId) -> bool {
        match self.complete_rooms.remove(room.0) {
            Some(mut r) => {
                r.remove_from_tree(tree);
                true
            }
            None => false,
        }
    }

    // --- accessors ------------------------------------------------------------------------

    /// The complete room at this index, or `None` for a stale id (Java's dead reference).
    pub fn complete_room(&self, room: RoomId) -> Option<&CompleteFreeSpaceExpansionRoom> {
        self.complete_rooms.get(room.0)
    }

    /// [`complete_room`](Self::complete_room), mutably.
    pub fn complete_room_mut(
        &mut self,
        room: RoomId,
    ) -> Option<&mut CompleteFreeSpaceExpansionRoom> {
        self.complete_rooms.get_mut(room.0)
    }

    /// The incomplete room at this index.
    pub fn incomplete_room(
        &self,
        room: IncompleteRoomId,
    ) -> Option<&IncompleteFreeSpaceExpansionRoom> {
        self.incomplete_rooms.get(room.0)
    }

    /// [`incomplete_room`](Self::incomplete_room), mutably.
    pub fn incomplete_room_mut(
        &mut self,
        room: IncompleteRoomId,
    ) -> Option<&mut IncompleteFreeSpaceExpansionRoom> {
        self.incomplete_rooms.get_mut(room.0)
    }

    /// The obstacle room at this index.
    pub fn obstacle_room(&self, room: ObstacleRoomId) -> Option<&ObstacleExpansionRoom> {
        self.obstacle_rooms.get(room.0)
    }

    /// [`obstacle_room`](Self::obstacle_room), mutably.
    pub fn obstacle_room_mut(
        &mut self,
        room: ObstacleRoomId,
    ) -> Option<&mut ObstacleExpansionRoom> {
        self.obstacle_rooms.get_mut(room.0)
    }

    /// The door at this index.
    pub fn door(&self, door: DoorId) -> Option<&ExpansionDoor> {
        self.doors.get(door.0)
    }

    /// [`door`](Self::door), mutably.
    pub fn door_mut(&mut self, door: DoorId) -> Option<&mut ExpansionDoor> {
        self.doors.get_mut(door.0)
    }

    /// The target door at this index.
    pub fn target_door(&self, door: TargetDoorId) -> Option<&TargetItemExpansionDoor> {
        self.target_doors.get(door.0)
    }

    /// [`target_door`](Self::target_door), mutably.
    pub fn target_door_mut(&mut self, door: TargetDoorId) -> Option<&mut TargetItemExpansionDoor> {
        self.target_doors.get_mut(door.0)
    }

    // --- the `ExpansionRoom` interface, dispatched over the three implementors --------------

    /// `ExpansionRoom.getShape()` (ExpansionRoom.java:28) for any room kind. `None` is Java's
    /// null shape, or a stale id.
    pub fn room_shape(&self, room: RoomRef) -> Option<&TileShape> {
        match room {
            RoomRef::Complete(id) => self.complete_rooms.get(id.0)?.get_shape(),
            RoomRef::Obstacle(id) => self.obstacle_rooms.get(id.0)?.get_shape(),
            RoomRef::Incomplete(id) => self.incomplete_rooms.get(id.0)?.get_shape(),
        }
    }

    /// `ExpansionRoom.getLayer()` (ExpansionRoom.java:31). The board is needed only for an
    /// obstacle room, whose layer is `item.shapeLayer(indexInItem)`, recomputed on every call
    /// (ObstacleExpansionRoom.java:38-41).
    pub fn room_layer(&self, board: &Board, room: RoomRef) -> Option<usize> {
        match room {
            RoomRef::Complete(id) => Some(self.complete_rooms.get(id.0)?.get_layer()),
            RoomRef::Obstacle(id) => self.obstacle_rooms.get(id.0)?.get_layer(board),
            RoomRef::Incomplete(id) => Some(self.incomplete_rooms.get(id.0)?.get_layer()),
        }
    }

    /// `ExpansionRoom.getId()` (ExpansionRoom.java:34) — the three-way dispatch over the three
    /// formulas in this module's table. `room_id_no` rather than `room_id`, because the answer
    /// is a Java `int` hash and **not** the [`RoomRef`] that addresses the room.
    ///
    /// # Panics
    /// For an incomplete room with no shape — Java's `NullPointerException`, see
    /// [`IncompleteFreeSpaceExpansionRoom::get_id`].
    pub fn room_id_no(&self, room: RoomRef) -> Option<i32> {
        match room {
            RoomRef::Complete(id) => Some(self.complete_rooms.get(id.0)?.get_id()),
            RoomRef::Obstacle(id) => Some(self.obstacle_rooms.get(id.0)?.get_id()),
            RoomRef::Incomplete(id) => Some(self.incomplete_rooms.get(id.0)?.get_id()),
        }
    }

    /// `CompleteExpansionRoom.getObject()` (CompleteExpansionRoom.java:14): the room's
    /// `SearchTreeObject`. A complete free-space room is its own
    /// (CompleteFreeSpaceExpansionRoom.java:126-129); an obstacle room's is its **item**
    /// (ObstacleExpansionRoom.java:131-134); an incomplete room has none — it does not implement
    /// `CompleteExpansionRoom`.
    pub fn get_object(&self, room: RoomRef) -> Option<TreeObject> {
        match room {
            RoomRef::Complete(id) => Some(self.complete_rooms.get(id.0)?.get_object()),
            RoomRef::Obstacle(id) => Some(self.obstacle_rooms.get(id.0)?.get_object()),
            RoomRef::Incomplete(_) => None,
        }
    }

    /// `ExpansionRoom.addDoor(ExpansionDoor)` (ExpansionRoom.java:10) for any room kind.
    pub fn add_door(&mut self, room: RoomRef, door: DoorId) {
        match room {
            RoomRef::Complete(id) => {
                if let Some(r) = self.complete_rooms.get_mut(id.0) {
                    r.add_door(door);
                }
            }
            RoomRef::Obstacle(id) => {
                if let Some(r) = self.obstacle_rooms.get_mut(id.0) {
                    r.add_door(door);
                }
            }
            RoomRef::Incomplete(id) => {
                if let Some(r) = self.incomplete_rooms.get_mut(id.0) {
                    r.add_door(door);
                }
            }
        }
    }

    /// `ExpansionRoom.getDoors()` (ExpansionRoom.java:13). An empty slice for a stale id, where
    /// Java would NPE.
    pub fn room_doors(&self, room: RoomRef) -> &[DoorId] {
        match room {
            RoomRef::Complete(id) => self
                .complete_rooms
                .get(id.0)
                .map_or(&[][..], |r| r.get_doors()),
            RoomRef::Obstacle(id) => self
                .obstacle_rooms
                .get(id.0)
                .map_or(&[][..], |r| r.get_doors()),
            RoomRef::Incomplete(id) => self
                .incomplete_rooms
                .get(id.0)
                .map_or(&[][..], |r| r.get_doors()),
        }
    }

    /// `CompleteExpansionRoom.getTargetDoors()` (CompleteExpansionRoom.java:11). Only a complete
    /// free-space room has any; the other two answer Java's fresh empty list.
    pub fn room_target_doors(&self, room: RoomRef) -> &[TargetDoorId] {
        match room {
            RoomRef::Complete(id) => self
                .complete_rooms
                .get(id.0)
                .map_or(&[][..], |r| r.get_target_doors()),
            RoomRef::Obstacle(_) | RoomRef::Incomplete(_) => &[],
        }
    }

    /// `ExpansionRoom.clearDoors()` (ExpansionRoom.java:16) — and, for a complete free-space
    /// room, the target doors too (CompleteFreeSpaceExpansionRoom.java:196-201).
    pub fn clear_doors(&mut self, room: RoomRef) {
        match room {
            RoomRef::Complete(id) => {
                if let Some(r) = self.complete_rooms.get_mut(id.0) {
                    r.clear_doors();
                }
            }
            RoomRef::Obstacle(id) => {
                if let Some(r) = self.obstacle_rooms.get_mut(id.0) {
                    r.clear_doors();
                }
            }
            RoomRef::Incomplete(id) => {
                if let Some(r) = self.incomplete_rooms.get_mut(id.0) {
                    r.clear_doors();
                }
            }
        }
    }

    /// `ExpansionRoom.resetDoors()` (ExpansionRoom.java:19): "clears the autorouting info of all
    /// doors for routing the next connection."
    ///
    /// The door ids are copied out first, because the doors and the room live in two arenas of
    /// the same struct and Java's loop holds neither borrow.
    pub fn reset_doors(&mut self, room: RoomRef) {
        let doors: Vec<DoorId> = self.room_doors(room).to_vec();
        for door in doors {
            if let Some(d) = self.doors.get_mut(door.0) {
                d.reset();
            }
        }
        // CompleteFreeSpaceExpansionRoom.java:203-209: the target doors as well.
        if let RoomRef::Complete(id) = room {
            let target_doors: Vec<TargetDoorId> = self
                .complete_rooms
                .get(id.0)
                .map_or(Vec::new(), |r| r.get_target_doors().to_vec());
            for door in target_doors {
                if let Some(d) = self.target_doors.get_mut(door.0) {
                    d.reset();
                }
            }
        }
    }

    /// `ExpansionRoom.doorExists(ExpansionRoom)` (ExpansionRoom.java:22): "checks if this room
    /// already has a door to other."
    pub fn door_exists(&self, room: RoomRef, other: RoomRef) -> bool {
        match room {
            RoomRef::Complete(id) => self
                .complete_rooms
                .get(id.0)
                .is_some_and(|r| r.door_exists(&self.doors, other)),
            RoomRef::Obstacle(id) => self
                .obstacle_rooms
                .get(id.0)
                .is_some_and(|r| r.door_exists(&self.doors, other)),
            RoomRef::Incomplete(id) => self
                .incomplete_rooms
                .get(id.0)
                .is_some_and(|r| r.door_exists(&self.doors, other)),
        }
    }

    /// `ExpansionRoom.removeDoor(ExpandableObject)` (ExpansionRoom.java:25): "removes door from
    /// this room. Returns false if this room did not contain door."
    pub fn remove_door(&mut self, room: RoomRef, door: ExpandableRef) -> bool {
        match room {
            RoomRef::Complete(id) => self
                .complete_rooms
                .get_mut(id.0)
                .is_some_and(|r| r.remove_door(door)),
            RoomRef::Obstacle(id) => self
                .obstacle_rooms
                .get_mut(id.0)
                .is_some_and(|r| r.remove_door(door)),
            RoomRef::Incomplete(id) => match door {
                ExpandableRef::Door(d) => self
                    .incomplete_rooms
                    .get_mut(id.0)
                    .is_some_and(|r| r.remove_door(d)),
                _ => false,
            },
        }
    }

    // --- the `ExpandableObject` methods that need the graph ---------------------------------

    /// `ExpansionDoor.getShape()` (ExpansionDoor.java:41-47): the intersection of the two rooms'
    /// shapes, recomputed on every call as Java does. `None` for a stale door id or a room with
    /// no shape, where Java throws.
    pub fn door_shape(&self, door: DoorId) -> Option<TileShape> {
        let d = self.doors.get(door.0)?;
        let first = self.room_shape(d.first_room)?;
        let second = self.room_shape(d.second_room)?;
        Some(d.get_shape(first, second))
    }

    /// `ExpansionDoor.getSectionSegments(double)` (ExpansionDoor.java:104-143), with both room
    /// shapes resolved out of the arenas. **Mutates the door**: it allocates the section array
    /// (`:141`).
    ///
    /// An empty vector for a stale door id or a room with no shape, where Java throws — the same
    /// answer `:110` and `:126` already give for a door that cannot be sectioned.
    pub fn door_section_segments(&mut self, door: DoorId, offset: f64) -> Vec<FloatLine> {
        let Some(d) = self.doors.get(door.0) else {
            return Vec::new();
        };
        let (Some(first), Some(second)) = (
            self.room_shape(d.first_room).cloned(),
            self.room_shape(d.second_room).cloned(),
        ) else {
            return Vec::new();
        };
        match self.doors.get_mut(door.0) {
            Some(d) => d.get_section_segments(&first, &second, offset),
            None => Vec::new(),
        }
    }

    /// `ExpansionDoor.getId()` (ExpansionDoor.java:184-190), with both room ids resolved through
    /// [`room_id_no`](Self::room_id_no).
    pub fn door_id_no(&self, door: DoorId) -> Option<i32> {
        let d = self.doors.get(door.0)?;
        Some(ExpansionDoor::id(
            self.room_id_no(d.first_room)?,
            self.room_id_no(d.second_room)?,
        ))
    }

    /// The room-list half of `AutorouteEngine.removeAllDoors(ExpansionRoom)`
    /// (AutorouteEngine.java:603-615): unlink every door of `room` from the room on its other
    /// side, drop any incomplete room that other side turns out to be, and then clear `room`'s
    /// own door list.
    ///
    /// Java's method is on the engine because it needs `incompleteExpansionRooms`; the port's is
    /// on the store because that list *is* the store's incomplete arena. Task 6's
    /// `AutorouteEngine::remove_all_doors` delegates here — the name deliberately matches Java's
    /// only in this file, which `audit-port.sh` does not scope `AutorouteEngine` to, so the
    /// obligation to write the engine method stays open (the same arrangement as
    /// [`next_room_id_no`](Self::next_room_id_no)).
    pub fn remove_all_doors(&mut self, room: RoomRef) {
        // Java iterates `room.getDoors()` while `removeIncompleteExpansionRoom` mutates *other*
        // rooms' door lists, never this one's, so a snapshot is the same traversal.
        let doors: Vec<DoorId> = self.room_doors(room).to_vec();
        for door in doors {
            let Some(other) = self.doors.get(door.0).and_then(|d| d.other_room(room)) else {
                // AutorouteEngine.java:606-608.
                continue;
            };
            self.remove_door(other, ExpandableRef::Door(door));
            if let RoomRef::Incomplete(id) = other {
                self.remove_incomplete_expansion_room(id);
            }
        }
        self.clear_doors(room);
    }

    /// The room-list half of `AutorouteEngine.removeIncompleteExpansionRoom`
    /// (AutorouteEngine.java:368-371): `removeAllDoors(room)` and then drop it from the
    /// incomplete list.
    ///
    /// Java's `incompleteExpansionRooms.remove(room)` is an `ArrayList.remove(Object)`, i.e. the
    /// first element `equals` it — `IncompleteFreeSpaceExpansionRoom` has no `equals` override,
    /// so that is reference identity, which is what removing the arena slot is.
    ///
    /// # Panics
    ///
    /// Java bug: `AutorouteEngine.removeIncompleteExpansionRoom` — `:370` dereferences
    /// `incompleteExpansionRooms` with no null guard, although every other reader of the field
    /// (`getFirstIncompleteExpansionRoom:357`, `initConnection:99`, `clear:307`) has one. The
    /// list is null until the first `addIncompleteExpansionRoom` (`:343-345`) and again after
    /// `clear` (`:314`), so this throws a `NullPointerException` on an engine that has never had
    /// an incomplete room added. It is reachable: `ExpansionDrill.calculateExpansionRooms:79`
    /// reaches it through `completeExpansionRoom:469` for a room it built with the bare
    /// constructor, and `completeExpansionRoom`'s own `catch` then turns the throw into an empty
    /// room list, so **every drill on a virgin engine is silently dropped**. See
    /// `docs/java-quirks.md` #169; `crates/fr-router/tests/drill.rs`'s
    /// `a_virgin_engine_yields_no_drills_at_all` is the probe's mode 8 verbatim.
    pub fn remove_incomplete_expansion_room(&mut self, room: IncompleteRoomId) {
        self.remove_all_doors(RoomRef::Incomplete(room));
        assert!(
            self.incomplete_list_created,
            "AutorouteEngine.removeIncompleteExpansionRoom: incompleteExpansionRooms is null — \
             Java throws a NullPointerException here too (AutorouteEngine.java:370), and the \
             lazily created list (`:343-345`) does not exist until the first \
             addIncompleteExpansionRoom"
        );
        self.incomplete_rooms.remove(room.0);
    }

    /// `TargetItemExpansionDoor.getId()` (TargetItemExpansionDoor.java:70-74), with the room id
    /// resolved — `0` for Java's null room.
    pub fn target_door_id_no(&self, door: TargetDoorId) -> Option<i32> {
        let d = self.target_doors.get(door.0)?;
        let room_id = match d.room {
            Some(room) => self.room_id_no(room)?,
            None => 0,
        };
        Some(target_door_id(d.item, room_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntBox;

    fn boxed(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
        TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
    }

    fn two_rooms(store: &mut ExpansionRoomStore) -> (RoomRef, RoomRef) {
        let a = store.next_room_id_no();
        let a = store.new_complete_room(Some(boxed(0, 0, 10, 10)), 0, a);
        let b = store.next_room_id_no();
        let b = store.new_complete_room(Some(boxed(10, 0, 20, 10)), 0, b);
        (RoomRef::Complete(a), RoomRef::Complete(b))
    }

    #[test]
    fn the_counter_pre_increments_so_the_first_room_id_is_one() {
        // AutorouteEngine.java:673, `return ++expansionRoomInstanceCount`.
        let mut store = ExpansionRoomStore::new();
        assert_eq!(store.next_room_id_no(), 1);
        assert_eq!(store.next_room_id_no(), 2);
        assert_eq!(store.next_room_id_no(), 3);
    }

    /// A bare compensated tree, for the `clear` tests — no board needed.
    fn bare_tree() -> ShapeSearchTree {
        ShapeSearchTree::new(
            fr_board::TreeId(0),
            fr_board::structure::AngleRestriction::NinetyDegree,
            0,
        )
    }

    #[test]
    fn clear_removes_the_tree_leaves_before_it_drains_the_arenas() {
        // AutorouteEngine.java:306-317: the loop at :308-312 runs *before* the lists are nulled.
        let mut tree = bare_tree();
        let mut store = ExpansionRoomStore::new();
        let (a, b) = two_rooms(&mut store);
        let (RoomRef::Complete(a_id), RoomRef::Complete(b_id)) = (a, b) else {
            unreachable!()
        };
        store.insert_complete_room(&mut tree, a_id);
        store.insert_complete_room(&mut tree, b_id);
        assert_eq!(tree.tree().leaf_count(), 2);

        let door = store.new_door(a, b, 1);
        store.add_door(a, door);
        store.clear(&mut tree);

        assert_eq!(tree.tree().leaf_count(), 0, "the room leaves are gone");
        assert!(tree.tree().is_empty());
        assert!(store.complete_rooms.is_empty());
        assert!(store.doors.is_empty());
        assert_eq!(store.next_room_id_no(), 1, "the counter went back to 0");
        // And the indices restart, which is why every surviving id is meaningless.
        let fresh = store.next_room_id_no();
        assert_eq!(
            store.new_complete_room(Some(boxed(0, 0, 1, 1)), 0, fresh),
            RoomId(0)
        );
    }

    #[test]
    fn clear_is_safe_for_a_room_that_never_entered_the_tree() {
        // A shapeless room is never inserted, so its `tree_leaf` is `None` and
        // `ShapeTree::remove_leaf_opt` skips it (MinAreaTree.java:121-123).
        let mut tree = bare_tree();
        let mut store = ExpansionRoomStore::new();
        let id = store.next_room_id_no();
        store.new_complete_room(None, 0, id);
        store.clear(&mut tree);
        assert!(tree.tree().is_empty());
        assert!(store.complete_rooms.is_empty());
    }

    #[test]
    fn reset_doors_reaches_both_the_doors_and_the_target_doors() {
        // CompleteFreeSpaceExpansionRoom.java:203-209.
        let mut store = ExpansionRoomStore::new();
        let (a, b) = two_rooms(&mut store);
        let door = store.new_door(a, b, 1);
        store.add_door(a, door);
        store.door_mut(door).unwrap().allocate_sections(2);
        store
            .door_mut(door)
            .unwrap()
            .get_maze_search_element_mut(0)
            .unwrap()
            .is_occupied = true;

        let target = TargetDoorId(
            store
                .target_doors
                .insert(TargetItemExpansionDoor::with_shape(
                    ItemId(1),
                    0,
                    Some(a),
                    boxed(0, 0, 1, 1),
                )),
        );
        let RoomRef::Complete(a_id) = a else {
            unreachable!()
        };
        store
            .complete_room_mut(a_id)
            .unwrap()
            .add_target_door(target);
        store
            .target_door_mut(target)
            .unwrap()
            .get_maze_search_element_mut(0)
            .room_ripped = true;

        store.reset_doors(a);
        assert!(
            !store
                .door(door)
                .unwrap()
                .get_maze_search_element(0)
                .unwrap()
                .is_occupied
        );
        assert!(
            !store
                .target_door(target)
                .unwrap()
                .get_maze_search_element(0)
                .room_ripped
        );
    }

    #[test]
    fn only_a_complete_free_space_room_has_target_doors() {
        // CompleteExpansionRoom.java:11 versus ObstacleExpansionRoom.java:121-124 and
        // IncompleteFreeSpaceExpansionRoom.java:33-35, both of which answer a fresh empty list.
        let mut store = ExpansionRoomStore::new();
        let (a, _) = two_rooms(&mut store);
        let incomplete =
            RoomRef::Incomplete(store.new_incomplete_room(Some(boxed(0, 0, 1, 1)), 0, None));
        assert!(store.room_target_doors(a).is_empty());
        assert!(store.room_target_doors(incomplete).is_empty());
        // An incomplete room is not a `CompleteExpansionRoom`, so it has no `getObject()`.
        assert_eq!(store.get_object(incomplete), None);
    }

    #[test]
    fn a_stale_id_reads_a_hole_rather_than_someone_elses_room() {
        // Ruling 16: no generation counter, so this is Java's dead reference, not a panic.
        let mut store = ExpansionRoomStore::new();
        let (a, _) = two_rooms(&mut store);
        let RoomRef::Complete(id) = a else {
            unreachable!()
        };
        store.complete_rooms.remove(id.0);
        assert_eq!(store.room_shape(a), None);
        assert_eq!(store.room_id_no(a), None);
        assert!(store.room_doors(a).is_empty());
        assert!(!store.door_exists(a, a));
        assert!(!store.remove_door(a, ExpandableRef::Door(DoorId(0))));
    }

    #[test]
    fn the_three_get_ids_dispatch_to_three_different_formulas() {
        let mut store = ExpansionRoomStore::new();
        let counter = store.next_room_id_no();
        let complete =
            RoomRef::Complete(store.new_complete_room(Some(boxed(0, 0, 4, 4)), 2, counter));
        assert_eq!(store.room_id_no(complete), Some(counter));

        let shape = boxed(1, 1, 5, 5);
        let incomplete =
            RoomRef::Incomplete(store.new_incomplete_room(Some(shape.clone()), 3, None));
        assert_eq!(
            store.room_id_no(incomplete),
            Some(shape.get_id().wrapping_mul(31).wrapping_add(3))
        );
    }
}
