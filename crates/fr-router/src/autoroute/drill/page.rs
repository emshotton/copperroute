//! Port of `autoroute.drill.DrillPage` (DrillPage.java:21-193) — one rectangle of the board's
//! drill grid, and the memoised list of expansion drills on it.

use std::sync::Arc;

use fr_board::{Board, ItemId, StopCheck, TreeObject};
use fr_geometry::{IntBox, Point, PolylineArea, TileShape};

use crate::arena::DrillId;
use crate::autoroute::expansion::RoomRef;
use crate::autoroute::maze::AutorouteEngine;
use crate::autoroute::maze::engine::tree_of;
use crate::autoroute::maze::search_element::MazeSearchElement;
use crate::{Arena, ExpansionDrill};

/// One entry of the obstacle cut-out loop (DrillPage.java:73-96), and what the loop did with it.
///
/// Java keeps no such record — the loop's only output is `cutoutShapes`. This exists so the
/// `prevObstacleShape` carry (`:87`) can be asserted against the JVM entry by entry rather than
/// only through the drill count it eventually changes; see
/// [`DrillPage::obstacle_cutout_trace`], which is a **view** of the same loop the port routes
/// `get_drills` through, not a second copy of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutoutEntry {
    /// The item the tree entry belongs to.
    pub item: ItemId,
    /// `TreeEntry.shapeIndexInObject`.
    pub shape_index: usize,
    /// Whether `:77-84` skipped the entry: it is drillable for the current net, or it is an SMD
    /// pin and `attachSmd` is on. A skipped entry never reaches the `prevObstacleShape` carry, so
    /// it does not become the next entry's `prevObstacleShape` either.
    pub skipped: bool,
    /// Whether the entry produced a 2-dimensional cut-out shape (`:90-93`). False for a skipped
    /// entry, for one the carry suppressed (`:87`) and for one whose intersection with the page
    /// is lower-dimensional.
    pub cut_out: bool,
}

/// Port of `DrillPage` (DrillPage.java:21-193), one of the four `ExpandableObject` implementors.
///
/// # `getId()` is not stable (hazard B)
///
/// `getId()` (`:190-193`) is `31 * shape.getId() + netNumber`, and `netNumber` is written by
/// [`get_drills`](Self::get_drills) at `:65`. A page that is already an element of the maze's
/// `TreeSet<MazeListElement>` (plan-6 ruling 4) therefore changes its own sort key the first time
/// the net changes under it, and the set can then neither find nor remove that element. Plan 6
/// reproduces this exactly, insertion order included; it is not to be "fixed" before parity.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillPage {
    /// `public final IntBox shape` (`:24`): "the shape of the page". Public, as Java's is —
    /// `DrillPageArray.overlappingPages:90` and `MazeExpansionEngine` both read it directly.
    pub shape: IntBox,
    /// `private final MazeSearchElement[] mazeSearchElements` (`:26`), one per board layer
    /// (`:39`).
    maze_search_elements: Vec<MazeSearchElement>,
    /// `private Collection<ExpansionDrill> drills` (`:30`): "the list of expansion drills on this
    /// page. Null, if not yet calculated."
    ///
    /// `None` is Java's `null`, and the difference between `None` and `Some(vec![])` is
    /// load-bearing: `:64`'s guard recomputes only for `null`, so an empty list is a *memoised
    /// answer* — see quirk #168.
    ///
    /// The drills themselves live in
    /// [`ExpansionRoomStore::drills`](crate::autoroute::expansion::ExpansionRoomStore::drills);
    /// this is the ownership Java expresses by holding the objects.
    ///
    /// Shared so `:64`'s memo hit need not copy the list. `Arc` and not `Rc` because plan-2
    /// ruling 11 keeps these structures `Send + Sync`.
    drills: Option<Arc<Vec<DrillId>>>,
    /// `private int netNumber = -1` (`:33`): "the number of the net, for which the drills are
    /// calculated".
    net_number: i32,
}

impl DrillPage {
    /// Port of `DrillPage(IntBox, RoutingBoard)` (DrillPage.java:35-43).
    ///
    /// Java keeps the board as a field (`:27`) and reads `getLayerCount()` from it again at
    /// `:107`; the port takes the board per call instead (plan-2 ruling 11 — the back-pointer is
    /// a cycle), so the layer count is captured here, where the array is sized.
    pub fn new(shape: IntBox, board: &Board) -> DrillPage {
        DrillPage {
            shape,
            // :39-42.
            maze_search_elements: vec![MazeSearchElement::default(); board.get_layer_count()],
            // :30, :33 — the field initialisers.
            drills: None,
            net_number: -1,
        }
    }

    /// Port of `getDrills(AutorouteEngine, boolean)` (DrillPage.java:62-131): "returns the drills
    /// on this page. If `attachSmd`, drilling to smd pins is allowed."
    ///
    /// # What it does
    ///
    /// Cuts every obstacle out of the page rectangle, splits the remainder into convex pieces
    /// (`:102-103`) and turns each piece into an [`ExpansionDrill`] at its centre of gravity,
    /// keeping the drills that bind a room on every layer (`:125-127`).
    ///
    /// # The memo, and the id it mutates
    ///
    /// The whole body is guarded by `this.drills == null || engine.getNetNumber() !=
    /// this.netNumber` (`:64`), and `:65` writes `netNumber` — which
    /// [`get_id`](Self::get_id) hashes. See the type docs: recomputing a page changes its
    /// identity, and Plan 6 reproduces that.
    ///
    /// # Cancellation (ruling 6, site 6 of 6)
    ///
    /// `:103` hands `autorouteEngine.stoppableThread` to `PolylineArea.splitToConvex` — the raw
    /// stop flag, **not** `AutorouteEngine.isStopRequested()`, so the time limit is deliberately
    /// not consulted here. `splitToConvex` answers `null` when the flag trips
    /// (PolylineArea.java:189-191) and `:108` dereferences it with no check.
    ///
    /// # The cancelled split, and the memo it used to leave behind
    ///
    /// Java bug: `DrillPage.getDrills` — `:108`'s `drillShapes.length` is an unguarded
    /// dereference of a value `:103` can legitimately answer `null` for, so a cancelled split
    /// throws a `NullPointerException` instead of returning. Worse, `:65-66` has already written
    /// the new net number and installed a fresh **empty** `drills` list, so the page is left
    /// memoised as "no drills on this net" and `:64` sends every later call straight past the
    /// recomputation — a page interrupted once answers "no drills here" for the rest of the
    /// connection, silently removing every via candidate on it.
    ///
    /// fixed: T6 (#168) — the register's second option, "install the list only after the split
    /// succeeds". `:65-66`'s two writes are deferred past `:103`, so a cancelled page is left
    /// exactly as it was — `drills` still `None`, `net_number` still the old one — and `:64`
    /// recomputes it on the next call instead of trusting a memo written before the work. The
    /// throw goes with them: there is nothing left to dereference. See `docs/java-quirks.md` #168
    /// and `crates/fr-router/tests/drill.rs`'s
    /// `a_stopped_split_does_not_memoise_an_empty_page`.
    ///
    /// The old drills are freed on the same schedule, and that is deliberate rather than
    /// incidental: freeing them before the split would strand the page holding arena ids it had
    /// already released if the split were then cancelled.
    pub fn get_drills(
        &mut self,
        engine: &mut AutorouteEngine,
        board: &mut Board,
        attach_smd: bool,
        stop: StopCheck<'_>,
    ) -> Arc<Vec<DrillId>> {
        // :64.
        if let Some(drills) = &self.drills
            && engine.get_net_number() == self.net_number
        {
            return Arc::clone(drills);
        }
        // :65-66. Java performs both writes *here*, before the work, which is what quirk #168 is
        // about; `// fixed: T6 (#168)` defers them past `:103` and only the net number is read in
        // between. `cutout_shapes` takes it as an argument, so nothing needs the field yet.
        let new_net_number = engine.get_net_number();

        // :67-96.
        let mut trace = Vec::new();
        let cutout_shapes =
            self.cutout_shapes(engine, board, new_net_number, attach_smd, &mut trace);

        // :97-102. Java copies the collection into an array and hands it to `PolylineArea`; the
        // holes are the cut-out shapes in the order the loop produced them.
        let shape_with_holes = PolylineArea::new(
            TileShape::Box(self.shape).into(),
            cutout_shapes.into_iter().map(Into::into).collect(),
        );
        // :103 — ruling 6's sixth cancellation site.
        //
        // fixed: T6 (#168) — a cancelled split now leaves the page untouched. Java's `:108`
        // dereferenced this null and threw, on a page `:65-66` had already memoised as empty, so
        // the rest of the connection saw "no drills here" and lost every via candidate on it.
        // Returning the empty list without installing it is the register's "install the list only
        // after the split succeeds": `self.drills` stays `None`, `self.net_number` stays the old
        // one, and `:64` recomputes on the next call.
        let Some(drill_shapes) = shape_with_holes.split_to_convex(Some(stop)) else {
            return Arc::new(Vec::new());
        };

        // :65-66, deferred to here. `:66`'s `this.drills = new LinkedList<>()` drops the previous
        // list, and Java's collector reclaims every `ExpansionDrill` on it. The port's arena has
        // no collector, so the ids are handed back here — see [`Self::invalidate`] for why that
        // is safe. Freeing them *after* the split is what keeps a cancelled page consistent: it
        // still owns the drills it is still advertising.
        self.net_number = new_net_number;
        if let Some(old_drills) = self.drills.take() {
            for old_drill in old_drills.iter().copied() {
                engine.rooms.drills.remove(old_drill.0);
            }
        }
        self.drills = Some(Arc::new(Vec::new()));

        // :105-107. "Use the center points of these drill shapes to try making a via."
        //
        // `getLayerCount() - 1` on a **zero**-layer board is Java's `-1`, which builds a drill
        // with empty arrays whose `calculateExpansionRooms` loop runs zero times and answers
        // true; the `usize` subtraction below would underflow instead. No such board exists —
        // `LayerStructure` is built from a non-empty array and every constructor path goes
        // through it — so this is a note, not a guard.
        let drill_first_layer = 0usize;
        let drill_last_layer = board.get_layer_count() - 1;
        // :108.
        for current_drill_shape in drill_shapes {
            // :110-121.
            let mut current_drill_location: Option<Point> = None;
            if attach_smd {
                current_drill_location =
                    calc_pin_center_in_drill(&current_drill_shape, drill_first_layer, board);
                if current_drill_location.is_none() {
                    current_drill_location =
                        calc_pin_center_in_drill(&current_drill_shape, drill_last_layer, board);
                }
            }
            let current_drill_location = current_drill_location.unwrap_or_else(|| {
                // :120. `centreOfGravity()` is a `FloatPoint`; `round()` is `IntPoint`.
                Point::Int(current_drill_shape.centre_of_gravity().round())
            });
            // :122-124.
            let mut new_drill = ExpansionDrill::new(
                current_drill_shape,
                current_drill_location,
                drill_first_layer,
                drill_last_layer,
            );
            // :125-127. Appended to `this.drills` **as it goes**, not collected and assigned at
            // the end: `:126` is `this.drills.add(newDrill)`, so a throw part-way through the
            // loop leaves the page holding the drills built so far, and quirk #168's memo is
            // that partial list rather than always an empty one.
            if new_drill.calculate_expansion_rooms(engine, board) {
                let id = DrillId(engine.rooms.drills.insert(new_drill));
                // A caller still holding an earlier call's `Arc` keeps that snapshot; `:65-66`
                // has just installed a fresh list, so this does not copy.
                Arc::make_mut(self.drills.get_or_insert_with(|| Arc::new(Vec::new()))).push(id);
            }
        }
        // :130.
        self.drills
            .as_ref()
            .map_or_else(|| Arc::new(Vec::new()), Arc::clone)
    }

    /// The obstacle cut-out loop of `getDrills` (DrillPage.java:67-96), as a **view**: the same
    /// call [`get_drills`](Self::get_drills) makes, with the per-entry trace kept instead of
    /// discarded.
    ///
    /// It reads the **engine's** net number rather than the page's, because `:65` has written
    /// the engine's into the page before the loop runs; over a page's own stale `netNumber`,
    /// `:77`'s `isDrillable` would answer differently for every item.
    pub fn obstacle_cutout_trace(
        &self,
        engine: &AutorouteEngine,
        board: &mut Board,
        attach_smd: bool,
    ) -> Vec<CutoutEntry> {
        let mut trace = Vec::new();
        self.cutout_shapes(
            engine,
            board,
            engine.get_net_number(),
            attach_smd,
            &mut trace,
        );
        trace
    }

    /// DrillPage.java:67-96: the shapes to cut out of the page rectangle, and a trace of how each
    /// overlapping tree entry was treated.
    fn cutout_shapes(
        &self,
        engine: &AutorouteEngine,
        board: &mut Board,
        net_number: i32,
        attach_smd: bool,
        trace: &mut Vec<CutoutEntry>,
    ) -> Vec<TileShape> {
        let page_shape = TileShape::Box(self.shape);
        // :67-69. The room-aware twin: by the time drills are built the autoroute tree holds
        // `TreeObject::Room` leaves, whose tree shapes only the room store can resolve.
        let overlaps = {
            let ctx = board.ctx();
            tree_of(board, engine.tree).overlapping_tree_entries_with_rooms(
                &page_shape,
                None,
                &[],
                &board.items,
                &engine.rooms,
                &ctx,
            )
        };

        let mut cutout_shapes: Vec<TileShape> = Vec::new();
        // :72. "Drills on top of existing vias are used in the ripup algorithm."
        let mut prev_obstacle_shape = TileShape::Box(IntBox::EMPTY);
        // :73.
        for current_entry in overlaps {
            // :74-76.
            let TreeObject::Item(item) = current_entry.object else {
                continue;
            };
            // :77-79. Java dereferences `currentItem` and NPEs for a missing item; a missing
            // item cannot arise from an entry the tree itself produced, and `is_some_and`'s
            // `false` keeps the entry in the loop rather than inventing a skip.
            let drillable = board
                .get_item(item)
                .is_some_and(|i| i.is_drillable(net_number));
            if drillable {
                trace.push(CutoutEntry {
                    item,
                    shape_index: current_entry.shape_index,
                    skipped: true,
                    cut_out: false,
                });
                continue;
            }
            // :80-84. `Pin.drillAllowed()` (Pin.java:344-350) is true for an SMD pad, i.e. one
            // whose padstack lives on a single layer. The missing-item `false` is the one
            // `:77-79` above explains: not a skip, and unreachable from a tree entry.
            let smd_skip = attach_smd && {
                let ctx = board.ctx();
                board.get_item(item).is_some_and(|i| match i {
                    fr_board::Item::Pin(pin) => pin.drill_allowed(&ctx),
                    _ => false,
                })
            };
            if smd_skip {
                trace.push(CutoutEntry {
                    item,
                    shape_index: current_entry.shape_index,
                    skipped: true,
                    cut_out: false,
                });
                continue;
            }

            // :85-86. `Item.getTreeShape(tree, index)` through the `&mut` path, which is the one
            // that reproduces `clearDerivedData` for an out-of-range index (plan-6 ruling 10 —
            // `fr-router` never calls the `&self` twin).
            let Some(current_obstacle_shape) =
                board.item_tree_shape(item, engine.tree, current_entry.shape_index)
            else {
                // Java dereferences the shape at `:87` and NPEs; a missing shape cannot arise
                // from an entry the tree itself produced.
                continue;
            };
            // :87-94. "Checked to avoid multiple cutout for example for vias with the same shape
            // on all layers."
            let mut cut_out = false;
            if !prev_obstacle_shape.contains_tile(&current_obstacle_shape) {
                let current_cutout_shape = current_obstacle_shape.intersection(&page_shape);
                if current_cutout_shape.dimension() == 2 {
                    cutout_shapes.push(current_cutout_shape);
                    cut_out = true;
                }
            }
            trace.push(CutoutEntry {
                item,
                shape_index: current_entry.shape_index,
                skipped: false,
                cut_out,
            });
            // :95. The carry advances only for an entry that was **not** skipped, because `:78`
            // and `:82` `continue` before this line.
            prev_obstacle_shape = current_obstacle_shape;
        }
        cutout_shapes
    }

    /// Port of `getShape()` (DrillPage.java:133-136). Java's field is an `IntBox` and the
    /// `ExpandableObject` method widens it to a `TileShape`.
    pub fn get_shape(&self) -> TileShape {
        TileShape::Box(self.shape)
    }

    /// Port of `getDimension()` (DrillPage.java:138-141): the constant 2.
    pub fn get_dimension(&self) -> i32 {
        2
    }

    /// Port of `mazeSearchElementCount()` (DrillPage.java:143-146): one per board layer.
    pub fn maze_search_element_count(&self) -> usize {
        self.maze_search_elements.len()
    }

    /// Port of `getMazeSearchElement(int)` (DrillPage.java:148-151).
    ///
    /// # Panics
    /// On an out-of-range index, which is Java's `ArrayIndexOutOfBoundsException` at `:150`.
    pub fn get_maze_search_element(&self, index: usize) -> &MazeSearchElement {
        &self.maze_search_elements[index]
    }

    /// [`get_maze_search_element`](Self::get_maze_search_element), mutably — Java hands back the
    /// array element itself and the maze search writes through it.
    pub fn get_maze_search_element_mut(&mut self, index: usize) -> &mut MazeSearchElement {
        &mut self.maze_search_elements[index]
    }

    /// Port of `reset()` (DrillPage.java:153-164): "resets all drills of this page for
    /// autorouting the next connection."
    ///
    /// The maze scratch only — the memoised drill list survives, and so do the rooms each drill
    /// is bound to. Dropping the list is [`invalidate`](Self::invalidate)'s job.
    ///
    /// Takes the drill arena because the drills live there rather than in the page (module docs).
    pub fn reset(&mut self, drills: &mut Arena<ExpansionDrill>) {
        // :156-160.
        if let Some(ids) = &self.drills {
            for id in ids.iter() {
                if let Some(drill) = drills.get_mut(id.0) {
                    drill.reset();
                }
            }
        }
        // :161-163.
        for element in &mut self.maze_search_elements {
            element.reset();
        }
    }

    /// Port of `invalidate()` (DrillPage.java:166-172): "invalidates the drills of this page so
    /// that they are recalculated at the next call of `getDrills()`."
    ///
    /// `netNumber` is **not** restored, so an invalidated page keeps the id its last
    /// recomputation gave it.
    ///
    /// # Why the arena slots are freed here, and why that is not a divergence
    ///
    /// Java's whole body is `this.drills = null`; the `ExpansionDrill`s it dropped are reclaimed
    /// by the collector *if nothing else holds them*. The port has no collector, and
    /// `invalidateDrillPages` fires once per changed item (`AutorouteEngine.java:411`,
    /// `RoutingBoard.java:107`), so leaving the slots would grow
    /// [`ExpansionRoomStore::drills`](super::super::expansion::ExpansionRoomStore::drills)
    /// without bound over a routing run. Freeing them is only sound if no live holder of one of
    /// these ids can be **dereferenced** afterwards, and in Java's lifecycle none can:
    ///
    /// * **During the maze search**, `MazeListElement.door` and `MazeSearchElement.backtrackDoor`
    ///   do hold page drills — but no page can be invalidated then. `invalidateDrillPages` has
    ///   exactly two Java call sites: `AutorouteEngine.removeCompleteExpansionRoom:411`, whose
    ///   own callers are `initConnection:108` and `additionalUpdateAfterChange:113`; and
    ///   `RoutingBoard.additionalUpdateAfterChange:107`, whose callers are `initConnection:115`
    ///   and five board-mutation sites (`BoardItemRepository.java:165,192`,
    ///   `PolylineTrace.java:189,687,944`, `ShapeTraceEntries.java:112`). The search mutates no
    ///   items: `MazeTraceShover` reaches `RoutingBoard.checkForcedTracePolyline:408-448`, which
    ///   calls only `TraceShover.check` (`:231`), never `TraceShover.insert` (`:417`).
    /// * **After the maze search**, `autorouteConnection:258-262` *does* mutate the board and so
    ///   does invalidate pages — but by then the only live holder of a page drill is
    ///   `FoundConnectionLocator.backtrackArray`, which is never read again. The whole backtrack
    ///   walk runs in that class's constructor (`FoundConnectionLocator.java:73-180`);
    ///   `FoundConnectionInserter` reads only `connection.connectionItems`
    ///   (`FoundConnectionInserter.java:42,47`), a list of plain corner/layer records; and the
    ///   one remaining reader, `FoundConnectionLocator.emitDiagnostics:502-511`, is
    ///   `not ported:`.
    /// * `Via.autorouteDrillInfo` (`Via.java:52`) is a drill `Via.getAutorouteDrillInfo:204-209`
    ///   builds itself and no page ever owns, so no page can free it.
    ///
    /// [`Arena`] never reuses an index, so if that argument is ever broken by a later task the
    /// symptom is a `None` at the dereference — a loud, locatable failure — not a silently
    /// aliased drill.
    ///
    /// **This is not `ExpansionRoomStore::clear`'s job.** `AutorouteEngine.clear` (`:306-317`)
    /// does *not* touch `drillPageArray`, so the store deliberately leaves the arena alone
    /// there; the page is what owns its drill ids, and this is where Java drops them.
    pub fn invalidate(&mut self, drills: &mut Arena<ExpansionDrill>) {
        // :171, plus the collection Java gets for free.
        if let Some(old_drills) = self.drills.take() {
            for id in old_drills.iter().copied() {
                drills.remove(id.0);
            }
        }
    }

    /// Port of `otherRoom(CompleteExpansionRoom)` (DrillPage.java:184-187): the constant `null`.
    pub fn other_room(&self, _room: RoomRef) -> Option<RoomRef> {
        None
    }

    /// Port of `getId()` (DrillPage.java:189-193): `31 * shape.getId() + netNumber`.
    ///
    /// A Java `int` hash, so it wraps — and it is **not stable**, because `netNumber` moves. See
    /// the type docs and `docs/java-quirks.md` #167.
    pub fn get_id(&self) -> i32 {
        // Java bug: `DrillPage.getId` — half of this hash is `netNumber`, which
        // `getDrills` overwrites at `:65`, so a page that is already an element of the maze's
        // `TreeSet<MazeListElement>` changes the sort key it is stored under (plan-6 ruling 4,
        // hazard B). Reproduced, not fixed: quirk #167.
        31i32
            .wrapping_mul(self.shape.get_id())
            .wrapping_add(self.net_number)
    }

    /// The memoised drill list: `None` for Java's `null` (never calculated, or invalidated),
    /// `Some` for a calculated one — including the empty list of quirk #168.
    ///
    /// Java has no accessor; the field is read directly by `reset`, `invalidate` and
    /// `emitDiagnostics`. This one exists so the port's callers and tests can see the *state*
    /// rather than only the answer.
    pub fn drills(&self) -> Option<&[DrillId]> {
        self.drills.as_ref().map(|list| list.as_slice())
    }

    /// `netNumber` (`:33`) — the net the memoised list was calculated for, `-1` before the first
    /// [`get_drills`](Self::get_drills).
    pub fn net_number(&self) -> i32 {
        self.net_number
    }
}

/// Port of the private static `calcPinCenterInDrill(TileShape, int, RoutingBoard)`
/// (DrillPage.java:45-60): "looks if `drillShape` contains the center of a drillable `Pin` on
/// `layer`. Returns null if no such Pin was found."
///
/// The loop keeps the **last** match, not the first — there is no `break` — so the answer depends
/// on the iteration order of `BasicBoard.overlappingItems`' `TreeSet<Item>`, which is
/// **descending** item id (quirk #63). `Board::overlapping_items` answers a `BTreeSet<ItemId>` in
/// ascending order, so this walks it in reverse; with two drillable pins in one drill shape the
/// two orders pick different centres.
fn calc_pin_center_in_drill(drill_shape: &TileShape, layer: usize, board: &Board) -> Option<Point> {
    // :50.
    let overlapping_items = board.overlapping_items(
        &fr_geometry::Area::Shape(fr_geometry::Shape::Tile(drill_shape.clone())),
        Some(layer),
    );
    let ctx = board.ctx();
    // :51-58, in Java's descending-id order.
    let mut result = None;
    for item in overlapping_items.into_iter().rev() {
        let Some(fr_board::Item::Pin(pin)) = board.get_item(item) else {
            continue;
        };
        if pin.drill_allowed(&ctx) && drill_shape.contains_inside(&pin.get_center(&ctx)) {
            result = Some(pin.get_center(&ctx));
        }
    }
    result
}

// =================================================================================================
// The deferral roster for `autoroute/drill/DrillPage.java`
// =================================================================================================

// not ported: `DrillPage.emitDiagnostics` — it feeds `AutorouteDiagnostic.Sink`, a GUI overlay
// (`global-constraints.md`: no GUI, no observers), and no routing decision reads it.

#[cfg(test)]
mod tests {
    use super::*;

    /// The `getId()` of an untouched page: `31 * shape.getId() + (-1)`. `P6T7Probe` mode 7,
    /// `fresh netNumber=-1 id=-29760001 shapeId=-960000`.
    #[test]
    fn a_fresh_pages_id_hashes_the_minus_one_net() {
        let shape = IntBox::from_coords(-1000, -1000, 1000, 1000);
        assert_eq!(shape.get_id(), -960_000);
        let page = DrillPage {
            shape,
            maze_search_elements: vec![MazeSearchElement::default(); 2],
            drills: None,
            net_number: -1,
        };
        assert_eq!(page.get_id(), -29_760_001);
        assert_eq!(page.net_number(), -1);
        assert_eq!(page.drills(), None);
        assert_eq!(page.get_dimension(), 2);
        assert_eq!(page.get_shape(), TileShape::Box(shape));
    }
}
