//! The board half of `PolylineTrace`: `combine` (PolylineTrace.java:174-192) with its two
//! private halves `combineAtStart` (:201-332) and `combineAtEnd` (:341-456),
//! `split(IntOctagon)` (:464-691), `split(Point)` (:698-712) with the private
//! `split(int, Line)` (:719-760) and `splitInsideDrillPadProhibited` (:768-792), and
//! `normalize` (:801-803), whose body is the whole of
//! `board/trace/PolylineTraceNormalization.java`.
//!
//! Every one of these needs the board — the search tree, the item list, `removeItem` and
//! `insertTraceWithoutCleaning` — so, as Task 11 did for the rest of the board-dependent `Item`
//! family, they are inherent methods of [`Board`] taking an [`ItemId`] rather than methods of
//! [`PolylineTrace`](crate::items::PolylineTrace). (The task brief named the files `src/items/trace_normalize.rs` and
//! `src/board/normalize.rs`; the first moved here so that the "no `Item.board` back-pointer"
//! rule of `global-constraints.md` keeps one home for the board-dependent bodies.) The pure
//! geometry they stand on — `split_polyline_at_line`, `perpendicular_split_line`,
//! `clip_intersects_segment` — is Task 8's, in [`crate::items::trace`].
//!
//! # Errors, not empty traces
//!
//! `Polyline::from_lines` is a `Result` (Plan 1 ruling 12) because of quirk #22: Java's
//! `removeOverlaps` throws `ArrayIndexOutOfBoundsException` on a line array that cancels itself
//! out completely. Java propagates that exception out of `combineAtEnd` — and
//! `combineAtEnd` itself observes the difference, because it tests
//! `joinedPolyline.lines.length != newLineCount` immediately after the constructor
//! (PolylineTrace.java:303-311), so "an empty polyline" and "a thrown exception" are different
//! outcomes. Every body here therefore threads the error out as
//! [`crate::BoardError::Normalization`]. The one place Java stops it
//! is `BasicBoard.insertTrace`'s own `catch (Exception)` (BasicBoard.java:230-241), reproduced
//! verbatim in [`Board::insert_trace`].
//!
//! # Iteration order
//!
//! Java's `pickItems` answers a `TreeSet<Item>`, which iterates in **descending** item id
//! (quirk #44); the port's `BTreeSet<ItemId>` is ascending, so every walk of one here is
//! `.rev()`ed. `getNormalContacts` is a `TreeSet` too, but both `combine` halves reject a
//! contact set of any size but one before they look at it, so its order cannot matter.

use std::collections::BTreeMap;

use fr_geometry::{IntOctagon, Line, Point, Polyline, TileShape};

use crate::datastructures::StopCheck;
use crate::error::BoardError;
use crate::ids::{ItemId, TreeObject};
use crate::items::Item;

use super::{Board, item_ctx};

/// Java `PolylineTraceNormalization.MAX_NORMALIZATION_DEPTH` (PolylineTraceNormalization.java:16).
///
/// `AGENTS.md` §"Trace Normalisation" says 34; the source says 16, and the source wins
/// (docs/java-quirks.md).
pub const MAX_NORMALIZATION_DEPTH: u32 = 16;

impl Board {
    // -- combine (PolylineTrace.java:174-456) ----------------------------------------------------

    /// Port of `Trace.combine` (Trace.java:463) / `PolylineTrace.combine`
    /// (PolylineTrace.java:174-192): absorb the neighbouring traces this one can be joined with,
    /// at its start first and then at its end, until neither end can grow.
    ///
    /// The loop is Java's own, and deliberately iterative: `combine` used to recurse once per
    /// merge and blew the stack on a long chain of collinear segments
    /// (`src/test/java/app/freerouting/fixtures/CombineStackOverflowTest.java`).
    ///
    /// Java's observer notification (:184-187) and `board.additionalUpdateAfterChange` are not
    /// ported and Plan 7's respectively.
    // added in Plan 7: `RoutingBoard.additionalUpdateAfterChange` (PolylineTrace.java:188) —
    // see `Board::insert_item`'s marker for the measurement Plan 6 Task 16 made and why the
    // wiring waits for `BatchAutorouter`.
    // not ported: the `board.communication.observers.notifyChanged` call (PolylineTrace.java:184-
    // 187) — `global-constraints.md` forbids board observers.
    pub fn combine_trace(&mut self, id: ItemId) -> Result<bool, BoardError> {
        let mut something_changed = false;
        // PolylineTrace.java:181. An item the port has removed is Java's `isOnTheBoard() == false`.
        while self.items.get(&id).is_some_and(Item::is_on_the_board)
            && (self.combine_trace_at_start(id, true)? || self.combine_trace_at_end(id, true)?)
        {
            something_changed = true;
        }
        Ok(something_changed)
    }

    /// Port of the private `PolylineTrace.combineAtStart(boolean)` (PolylineTrace.java:201-332):
    /// if exactly one other trace meets this one at its first corner and matches it in every
    /// respect, prepend that trace's corners to this one and delete it.
    ///
    /// `ignore_areas` is Java's parameter: with it set, conduction areas are dropped from the
    /// contact set before it is counted (:206-209). `combine` always passes `true`.
    ///
    /// Java's `board.itemList.saveForUndo(this)` (:275) is dropped — Plan 2 replaces the
    /// `UndoableObjects` stack with `Board::clone` (Task 12) — and its `FRLogger.trace`
    /// net-49 debug block (:202-231,254-269) is diagnostic only.
    // not ported: the net-49 `FRLogger.trace` debug block (PolylineTrace.java:202-231,254-269).
    pub fn combine_trace_at_start(
        &mut self,
        id: ItemId,
        ignore_areas: bool,
    ) -> Result<bool, BoardError> {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return Ok(false);
        };
        // PolylineTrace.java:204. A corner-less polyline would make Java's
        // `getNormalContacts(null, false)` throw at `p_point.equals(...)` (Trace.java:174); it
        // cannot arise, because `insertTraceWithoutCleaning` rejects a polyline of under two
        // corners (BasicBoard.java:185-187), and the empty contact set is refused two lines down
        // anyway.
        let Some(start_corner) = trace.first_corner() else {
            return Ok(false);
        };
        let Some(other_id) = self.single_combine_contact(id, &start_corner, ignore_areas) else {
            return Ok(false);
        };
        // PolylineTrace.java:234-272: the one contact must be a matching `PolylineTrace` that
        // ends (or, reversed, starts) at this trace's start corner.
        let Some(reverse_order) =
            self.combine_partner(id, other_id, &start_corner, CombineEnd::Start)
        else {
            return Ok(false);
        };
        self.combine_join(
            id,
            other_id,
            reverse_order,
            CombineEnd::Start,
            &start_corner,
        )
    }

    /// Port of the private `PolylineTrace.combineAtEnd(boolean)` (PolylineTrace.java:341-456),
    /// the mirror image of [`Board::combine_trace_at_start`].
    // not ported: the net-49 `FRLogger.trace` debug block (PolylineTrace.java:342-371).
    pub fn combine_trace_at_end(
        &mut self,
        id: ItemId,
        ignore_areas: bool,
    ) -> Result<bool, BoardError> {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return Ok(false);
        };
        // PolylineTrace.java:344.
        let Some(end_corner) = trace.last_corner() else {
            return Ok(false);
        };
        let Some(other_id) = self.single_combine_contact(id, &end_corner, ignore_areas) else {
            return Ok(false);
        };
        let Some(reverse_order) = self.combine_partner(id, other_id, &end_corner, CombineEnd::End)
        else {
            return Ok(false);
        };
        self.combine_join(id, other_id, reverse_order, CombineEnd::End, &end_corner)
    }

    /// `getNormalContacts(corner, false)`, minus the conduction areas when `ignore_areas`, and
    /// only if exactly one contact is left (PolylineTrace.java:205-233 = :345-378).
    fn single_combine_contact(
        &self,
        id: ItemId,
        corner: &Point,
        ignore_areas: bool,
    ) -> Option<ItemId> {
        let mut contacts: Vec<ItemId> = self
            .trace_normal_contacts_at(id, corner, false)
            .into_iter()
            .collect();
        if ignore_areas {
            // PolylineTrace.java:206-209 / :347-350.
            contacts.retain(|c| !matches!(self.items.get(c), Some(Item::ConductionArea(_))));
        }
        // PolylineTrace.java:232-234 / :373-375.
        if contacts.len() != 1 {
            return None;
        }
        Some(contacts[0])
    }

    /// The `for (Item currentObject : contacts)` body of both halves (PolylineTrace.java:235-272
    /// and :377-397): `Some(reverse_order)` when the contact is a `PolylineTrace` this one may
    /// be joined with, `None` for Java's `traceFound == false`.
    ///
    /// The contact set has exactly one element by the time this runs, so Java's loop-with-break
    /// collapses to a single test and its `TreeSet` order is not observable.
    fn combine_partner(
        &self,
        id: ItemId,
        other_id: ItemId,
        corner: &Point,
        at: CombineEnd,
    ) -> Option<bool> {
        let this_item = self.items.get(&id)?;
        let other_item = self.items.get(&other_id)?;
        let (Item::Trace(this), Item::Trace(other)) = (this_item, other_item) else {
            return None;
        };
        // PolylineTrace.java:239-244 / :381-386.
        if other.get_layer() != this.get_layer()
            || !other_item.nets_equal(this_item)
            || other.get_half_width() != this.get_half_width()
            || other_item.get_fixed_state() != this_item.get_fixed_state()
            || other_item.is_deletion_forbidden(&self.rules)
            || this_item.is_deletion_forbidden(&self.rules)
        {
            return None;
        }
        // `combineAtStart` joins the other trace's *last* corner to this start corner and
        // reverses when it is the other's first (PolylineTrace.java:245-251); `combineAtEnd` is
        // the mirror (:387-393).
        let (straight, reversed) = match at {
            CombineEnd::Start => (other.last_corner(), other.first_corner()),
            CombineEnd::End => (other.first_corner(), other.last_corner()),
        };
        if straight.as_ref() == Some(corner) {
            Some(false)
        } else if reversed.as_ref() == Some(corner) {
            Some(true)
        } else {
            None
        }
    }

    /// The shared tail of both halves (PolylineTrace.java:275-331 and :417-455): build the joined
    /// polyline, hand the search-tree entries over, delete the absorbed trace, and mark the
    /// changed area.
    fn combine_join(
        &mut self,
        id: ItemId,
        other_id: ItemId,
        reverse_order: bool,
        at: CombineEnd,
        corner: &Point,
    ) -> Result<bool, BoardError> {
        // Java's `board.itemList.saveForUndo(this)` (:275 / :417) is dropped; see the doc
        // comment.
        let (this_lines, layer) = match self.items.get(&id) {
            Some(Item::Trace(t)) => (t.polyline().lines().to_vec(), t.get_layer()),
            _ => return Ok(false),
        };
        let other_lines: Vec<Line> = match self.items.get(&other_id) {
            // PolylineTrace.java:279-288 / :421-430.
            Some(Item::Trace(t)) if reverse_order => t
                .polyline()
                .lines()
                .iter()
                .rev()
                .map(Line::opposite)
                .collect(),
            Some(Item::Trace(t)) => t.polyline().lines().to_vec(),
            _ => return Ok(false),
        };
        debug_assert!(
            this_lines.len() >= 3 && other_lines.len() >= 3,
            "combine: both polylines have at least three lines, as every board inserter \
             guarantees (BasicBoard.java:185-187)"
        );

        // PolylineTrace.java:289-303 / :431-445. `combineAtStart` skips the join line when the
        // other trace's *last* interior line is this trace's first; `combineAtEnd` when this
        // trace's last interior line is the other's first.
        let skip_line = match at {
            CombineEnd::Start => {
                other_lines[other_lines.len() - 2].is_equal_or_opposite(&this_lines[1])
            }
            CombineEnd::End => {
                this_lines[this_lines.len() - 2].is_equal_or_opposite(&other_lines[1])
            }
        };
        let mut new_line_count = this_lines.len() + other_lines.len() - 2;
        if skip_line {
            new_line_count -= 1;
        }
        // Java's two `System.arraycopy` calls plus the `joinPos` decrement: the head is copied
        // whole, and when a line is skipped the tail's first element overwrites the head's last.
        let (head, tail) = match at {
            CombineEnd::Start => (&other_lines, &this_lines),
            CombineEnd::End => (&this_lines, &other_lines),
        };
        let mut new_lines: Vec<Line> = Vec::with_capacity(new_line_count);
        new_lines.extend_from_slice(&head[..head.len() - 1]);
        if skip_line {
            new_lines.pop();
        }
        new_lines.extend_from_slice(&tail[1..]);
        debug_assert_eq!(new_lines.len(), new_line_count);
        // PolylineTrace.java:303 / :445 — quirk #22 makes this a `Result`.
        let joined_polyline = Polyline::from_lines(new_lines)?;

        // PolylineTrace.java:308-311 / :450-453: both traces must have default-tree entries, or
        // the optimised merge would dereference a null entry array. The port's entries are a
        // `Vec<Option<LeafId>>` behind an `Option`, so the guard is `Option` handling, not a null
        // test — but the branch it selects is Java's.
        let has_tree_entries = self.trace_has_default_entries(id, other_id);
        if joined_polyline.lines().len() != new_line_count || !has_tree_entries {
            // PolylineTrace.java:312-315 / :454-457.
            self.replace_trace_geometry(id, joined_polyline);
        } else {
            // PolylineTrace.java:316-327 / :458-469.
            let mut to_no = match at {
                CombineEnd::Start => other_lines.len(),
                CombineEnd::End => this_lines.len(),
            };
            if skip_line {
                to_no -= 1;
            }
            let from_entry_no = match at {
                CombineEnd::Start => other_lines.len() - 3,
                CombineEnd::End => this_lines.len() - 3,
            };
            match at {
                CombineEnd::Start => {
                    self.merge_trace_entries_in_front(
                        other_id,
                        id,
                        &joined_polyline,
                        from_entry_no,
                        to_no,
                    );
                }
                CombineEnd::End => {
                    self.merge_trace_entries_at_end(
                        other_id,
                        id,
                        &joined_polyline,
                        from_entry_no,
                        to_no,
                    );
                }
            }
            if let Some(other) = self.items.get_mut(&other_id) {
                other.clear_tree_entries();
            }
            if let Some(Item::Trace(this)) = self.items.get_mut(&id) {
                this.set_polyline(joined_polyline);
            }
        }
        // PolylineTrace.java:328-333 / :470-475.
        let collapsed = matches!(
            self.items.get(&id),
            Some(Item::Trace(t)) if t.polyline().lines().len() < 3
        );
        if collapsed {
            self.remove_item(id);
        }
        self.remove_item(other_id);
        self.join_changed_area(&corner.to_float(), layer);
        Ok(true)
    }

    // -- split (PolylineTrace.java:464-792) ------------------------------------------------------

    /// Port of `Trace.split(IntOctagon)` (Trace.java:471) / `PolylineTrace.split(IntOctagon)`
    /// (PolylineTrace.java:464-691): look up the traces intersecting this one, split them and it
    /// at the intersection points, drop any cycle a split piece created, and answer the pieces.
    ///
    /// If nothing was split the answer is just this trace. `clip`, Java's `clipShape`, restricts
    /// the scan to the segments whose bounding box it meets (:475-479).
    ///
    /// # Two things a reader should not mistake for port bugs
    ///
    /// * **The re-read appends.** `ShapeSearchTree.overlappingTreeEntries` *adds* to the
    ///   collection it is handed (ShapeSearchTree.java:429-430); it does not clear it. So the
    ///   "reread the overlapping tree entries and reset the iterator" step (:584-588) leaves the
    ///   stale entries in front of the fresh ones and walks them all again. Reproduced; quirk
    ///   row.
    /// * **A stale entry can name a trace that is no longer on the board.** Java's `TreeEntry`
    ///   holds the item itself, so the removed trace's polyline is still readable. The port's
    ///   entries hold an [`ItemId`], so each read of the tree snapshots the items it named into
    ///   `entry_items` below; nothing `split` reads off them (lines, layer, nets, fixed state)
    ///   can change between the read and the use, and `isOnTheBoard` is taken from the live
    ///   board.
    ///
    /// # This method can fail to terminate (quirk #76)
    ///
    /// Two rails joined by four or more rungs on one net make the re-read above loop forever, in
    /// Java and here alike, and neither `MAX_NORMALIZATION_DEPTH` nor `MAX_NORMALIZE_ITERATIONS`
    /// reaches it — both count outer passes this never leaves.
    /// `crates/fr-board/tests/trace_normalize.rs`'s
    /// `a_four_rung_ladder_never_finishes_normalizing` is the (`#[ignore]`d) reproduction.
    ///
    /// **Where the ladder actually hangs first** (Plan 3 Task 10): not here. The walk below
    /// retires only ~60 entries a minute on a four-rung ladder because each one is stuck inside
    /// `BasicBoard.removeIfCycle` -> [`Board::connection_items`], whose walk along the contacts
    /// has no visited set and circles a closed connection for ever (quirk #106). Both loops are
    /// escapable now — see [`Board::split_trace_checked`] and
    /// [`Board::connection_items_checked`].
    //
    // obligation: Plan 3 (docs/java-quirks.md, "Ladder hang in DSN import") — **discharged in
    // Plan 3 Task 10**, ruling 4's option (b): `Wiring.java:347` ends every DSN read with
    // `normalizeAllTraces()`, and `fr-dsn` now runs that under a `TimeLimit`-backed `StopCheck`
    // (`DsnReadOptions::normalize_time_limit`, default 60 s) rather than bounding this walk, so
    // every design that terminates normalises identically.
    pub fn split_trace(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
    ) -> Result<Vec<ItemId>, BoardError> {
        self.split_trace_checked(id, clip, &|| false)
    }

    /// [`Board::split_trace`] under a [`StopCheck`], consulted at the head of the entry walk —
    /// the loop quirk #76 never leaves (Plan 3 ruling 4). A trip answers
    /// [`BoardError::Stopped`].
    ///
    /// The check sits *inside* the `while cursor < entries.len()` loop rather than around it
    /// because that loop is the one that does not terminate: the per-segment `for` above it and
    /// the recursion below both make progress.
    // added in Plan 3: PolylineTrace.split (plan ruling 4)
    pub fn split_trace_checked(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
        stop: StopCheck<'_>,
    ) -> Result<Vec<ItemId>, BoardError> {
        let mut result: Vec<ItemId> = Vec::new();
        // totalized: Java can be handed a trace that has already been removed — `normalize`
        // recurses on a piece its own `combine` collapsed to under three lines
        // (PolylineTraceNormalization.java:122-123) — and walks the dead object's polyline,
        // answering the empty collection as soon as one of its segments still overlaps
        // something, and `[this]` when none does. The port keys items by id and the item is
        // gone, so it answers the empty collection, which is what the overlap that caused the
        // combine makes Java answer too. See docs/java-quirks.md.
        let Some(item) = self.items.get(&id) else {
            return Ok(result);
        };
        // PolylineTrace.java:466-470.
        if !item.nets_normal() {
            result.push(id);
            return Ok(result);
        }
        let Item::Trace(trace) = item else {
            return Ok(vec![id]);
        };
        let layer = trace.get_layer();
        // `this.lines` never changes inside `split` — nothing here calls `setPolyline` — so a
        // snapshot is exactly Java's live field, and it survives the removal of the trace the
        // `DrillItem` branch can perform.
        let lines = trace.polyline().clone();
        let snapshot = item.clone();
        let is_user_fixed = item.is_user_fixed();
        let net_nos = item.net_nos().to_vec();

        let mut own_trace_split = false;
        for i in 0..lines.lines().len().saturating_sub(2) {
            // PolylineTrace.java:475-480, through the filter Task 8 extracted for this caller.
            // It is read off the snapshot rather than the live item, because the `DrillItem`
            // branch below can remove the trace part-way through this loop while Java keeps
            // reading its (unchanged) polyline.
            if let (Some(clip), Item::Trace(snapshot_trace)) = (clip, &snapshot)
                && !snapshot_trace.clip_intersects_segment(i, clip)
            {
                continue;
            }
            // PolylineTrace.java:481-482.
            let Some(current_shape) = self.split_current_shape(id, &snapshot, i) else {
                continue;
            };
            let Some(current_line_segment) = fr_geometry::LineSegment::from_polyline(&lines, i + 1)
            else {
                continue;
            };
            // PolylineTrace.java:483-486: the list the re-read appends to.
            let mut entry_items: BTreeMap<ItemId, Item> = BTreeMap::new();
            let mut entries =
                self.split_overlapping_entries(&current_shape, layer, &mut entry_items);
            let mut cursor = 0usize;
            while cursor < entries.len() {
                if stop() {
                    return Err(BoardError::Stopped);
                }
                // PolylineTrace.java:488-492.
                if !self.items.get(&id).is_some_and(Item::is_on_the_board) {
                    return Ok(result);
                }
                let found_entry = entries[cursor];
                cursor += 1;
                let TreeObject::Item(found_id) = found_entry.object else {
                    // PolylineTrace.java:494-496: an expansion room is not an `Item`.
                    continue;
                };
                let shape_index = found_entry.shape_index;
                let Some(found_item) = self
                    .items
                    .get(&found_id)
                    .or_else(|| entry_items.get(&found_id))
                else {
                    continue;
                };
                if found_id == id {
                    // PolylineTrace.java:499-503.
                    if shape_index + 1 >= i && shape_index <= i + 1 {
                        continue;
                    }
                    // PolylineTrace.java:505-513: intermediate segments of length 0.
                    let same = if i < shape_index {
                        lines.corner(i + 1) == lines.corner(shape_index)
                    } else {
                        lines.corner(shape_index + 1) == lines.corner(i)
                    };
                    if same {
                        continue;
                    }
                }
                // PolylineTrace.java:515-517.
                if !found_item.shares_net_no(&net_nos) {
                    continue;
                }
                match found_item {
                    Item::Trace(found_trace) => {
                        let found_lines = found_trace.polyline().clone();
                        // PolylineTrace.java:519-521.
                        let Some(found_line_segment) =
                            fr_geometry::LineSegment::from_polyline(&found_lines, shape_index + 1)
                        else {
                            continue;
                        };
                        let mut split_pieces: Vec<ItemId> = Vec::new();
                        let mut found_trace_split = false;
                        if found_id != id {
                            // PolylineTrace.java:545-585: try splitting the found trace first.
                            let intersecting =
                                found_line_segment.intersection(&current_line_segment);
                            for line in &intersecting {
                                let pieces =
                                    self.split_trace_at_line(found_id, shape_index + 1, line)?;
                                let Some(pieces) = pieces else { continue };
                                for piece in pieces.into_iter().flatten() {
                                    found_trace_split = true;
                                    split_pieces.push(piece);
                                }
                                if found_trace_split {
                                    // PolylineTrace.java:584-588: the board changed, so re-read
                                    // — appending to the same list — and restart the walk.
                                    let fresh = self.split_overlapping_entries(
                                        &current_shape,
                                        layer,
                                        &mut entry_items,
                                    );
                                    entries.extend(fresh);
                                    cursor = 0;
                                    break;
                                }
                            }
                            // PolylineTrace.java:592-594.
                            if !found_trace_split {
                                split_pieces.push(found_id);
                            }
                        }
                        // PolylineTrace.java:597-610: now try splitting the own trace.
                        let intersecting = current_line_segment.intersection(&found_line_segment);
                        for line in &intersecting {
                            let pieces = self.split_trace_at_line(id, i + 1, line)?;
                            let Some(pieces) = pieces else { continue };
                            own_trace_split = true;
                            for piece in pieces.into_iter().flatten() {
                                result.extend(self.split_trace_checked(piece, clip, stop)?);
                            }
                            break;
                        }
                        if found_trace_split || own_trace_split {
                            // PolylineTrace.java:611-648: remove the cycles a split piece
                            // created — the found trace's pieces first, this trace's last, "to
                            // preserve them, if possible".
                            for piece in &split_pieces {
                                self.remove_if_cycle_checked(*piece, stop)?;
                            }
                            for piece in result.clone() {
                                self.remove_if_cycle_checked(piece, stop)?;
                            }
                        }
                        // PolylineTrace.java:649-651.
                        if own_trace_split {
                            break;
                        }
                    }
                    Item::Pin(_) | Item::Via(_) => {
                        // PolylineTrace.java:652-659: cut the trace at a drill centre it runs
                        // through. Java throws the two pieces away, so `ownTraceSplit` stays
                        // false and the collection this method answers does not contain them.
                        let Some(split_point) = self.drill_center(found_id) else {
                            continue;
                        };
                        if current_line_segment.contains(&split_point)
                            && let Point::Int(int_point) = split_point
                        {
                            let direction = current_line_segment
                                .get_line()
                                .direction()
                                .turn_45_degree(2);
                            let split_line = Line::from_direction(int_point, &direction);
                            self.split_trace_at_line(id, i + 1, &split_line)?;
                        }
                    }
                    Item::ConductionArea(_) if !is_user_fixed => {
                        // PolylineTrace.java:660-681.
                        let mut ignore_areas = false;
                        if let Some(first_net) = net_nos.first()
                            && let Some(net) = self.rules.nets.get(*first_net)
                        {
                            ignore_areas = self
                                .rules
                                .net_classes
                                .get(net.get_net_class())
                                .get_ignore_cycles_with_areas();
                        }
                        if !ignore_areas
                            && self.trace_start_contacts(id).contains(&found_id)
                            && self.trace_end_contacts(id).contains(&found_id)
                        {
                            // This trace can be removed because of a cycle with the area.
                            self.remove_item(id);
                            return Ok(result);
                        }
                    }
                    _ => {}
                }
            }
            // PolylineTrace.java:683-685.
            if own_trace_split {
                break;
            }
        }
        // PolylineTrace.java:686-688.
        if !own_trace_split {
            result.push(id);
        }
        // added in Plan 7: `RoutingBoard.additionalUpdateAfterChange` — the loop over a result
        // of more than one piece (PolylineTrace.java:689-693). See `Board::insert_item`'s marker
        // for the measurement Plan 6 Task 16 made and why the wiring waits for
        // `BatchAutorouter`.
        Ok(result)
    }

    /// `this.getTreeShape(defaultTree, index)` (Item.java:212-225) for a trace `split` may
    /// already have removed from the item list.
    ///
    /// Java reads it off the live object, whose precalculated tree shapes outlive the removal;
    /// the port recomputes it from the snapshot taken when `split` started, which is the same
    /// value — `calculateTreeShapes` is a pure function of the item's geometry, its clearance
    /// class and the tree.
    fn split_current_shape(
        &mut self,
        id: ItemId,
        snapshot: &Item,
        index: usize,
    ) -> Option<TileShape> {
        if self.items.contains_key(&id) {
            let tree = self.default_tree_id();
            return self.item_tree_shape(id, tree, index);
        }
        let ctx = item_ctx!(self);
        self.trees
            .get_default_tree()
            .calculate_tree_shapes(snapshot, &ctx)
            .get(index)
            .cloned()
            .flatten()
    }

    /// `defaultTree.overlappingTreeEntries(currentShape, getLayer(), overlappingTreeEntries)`
    /// (PolylineTrace.java:486), plus the snapshot of the items the entries name (see
    /// [`Board::split_trace`]'s doc comment).
    fn split_overlapping_entries(
        &mut self,
        shape: &TileShape,
        layer: usize,
        entry_items: &mut BTreeMap<ItemId, Item>,
    ) -> Vec<crate::datastructures::TreeEntry<TreeObject>> {
        let ctx = item_ctx!(self);
        let entries = self.trees.get_default_tree().overlapping_tree_entries(
            shape,
            Some(layer),
            &[],
            &self.items,
            &ctx,
        );
        for entry in &entries {
            if let TreeObject::Item(entry_id) = entry.object
                && let Some(item) = self.items.get(&entry_id)
            {
                entry_items.insert(entry_id, item.clone());
            }
        }
        entries
    }

    /// Port of the private `PolylineTrace.split(int lineIndex, Line newEndLine)`
    /// (PolylineTrace.java:719-760): cut this trace in two at `new_end_line`, replacing it on the
    /// board with the two pieces.
    ///
    /// `None` is Java's `null` — the trace is off the board (:720-722), its deletion is forbidden
    /// (:727-729), the polyline split refused (:731-737), or `splitInsideDrillPadProhibited`
    /// rejected the intersection (:738-740). `Some([a, b])` carries Java's two array slots, each
    /// of which is `None` when `insertTraceWithoutCleaning` refused the piece.
    pub fn split_trace_at_line(
        &mut self,
        id: ItemId,
        line_index: usize,
        new_end_line: &Line,
    ) -> Result<Option<[Option<ItemId>; 2]>, BoardError> {
        let Some(item) = self.items.get(&id) else {
            // PolylineTrace.java:720-722.
            return Ok(None);
        };
        if !item.is_on_the_board() {
            return Ok(None);
        }
        // PolylineTrace.java:723-729.
        if item.is_deletion_forbidden(&self.rules) {
            return Ok(None);
        }
        let Item::Trace(trace) = item else {
            return Ok(None);
        };
        // PolylineTrace.java:731-737.
        let Some(pieces) = trace.split_polyline_at_line(line_index, new_end_line)? else {
            return Ok(None);
        };
        // PolylineTrace.java:738-740.
        if self.split_inside_drill_pad_prohibited(id, line_index, new_end_line) {
            return Ok(None);
        }
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return Ok(None);
        };
        let layer = trace.get_layer();
        let half_width = trace.get_half_width();
        let item = &self.items[&id];
        let net_nos = item.net_nos().to_vec();
        let clearance_class = item.clearance_class();
        let fixed_state = item.get_fixed_state();
        let [first, second] = pieces;
        // PolylineTrace.java:741-758.
        self.remove_item(id);
        let a = self.insert_trace_without_cleaning(
            first,
            layer,
            half_width,
            net_nos.clone(),
            clearance_class,
            fixed_state,
        );
        let b = self.insert_trace_without_cleaning(
            second,
            layer,
            half_width,
            net_nos,
            clearance_class,
            fixed_state,
        );
        Ok(Some([a, b]))
    }

    /// Port of `Trace.split(Point)` (Trace.java:477) / `PolylineTrace.split(Point)`
    /// (PolylineTrace.java:698-712): cut this trace at `point`, at the first of its segments that
    /// contains it.
    ///
    /// This is the per-candidate loop
    /// [`PolylineTrace::split_polyline_at_point`](crate::items::PolylineTrace::split_polyline_at_point)'s doc comment
    /// describes: Java's `split(int, Line)` can refuse for four board reasons, after which the
    /// `for` continues to the next segment.
    pub fn split_trace_at_point(
        &mut self,
        id: ItemId,
        point: &Point,
    ) -> Result<Option<[Option<ItemId>; 2]>, BoardError> {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return Ok(None);
        };
        let segment_count = trace.tile_shape_count();
        for i in 0..segment_count {
            let Some(Item::Trace(trace)) = self.items.get(&id) else {
                return Ok(None);
            };
            // PolylineTrace.java:700-704.
            let Some(split_line) = trace.perpendicular_split_line(i, point) else {
                continue;
            };
            if let Some(pieces) = self.split_trace_at_line(id, i + 1, &split_line)? {
                return Ok(Some(pieces));
            }
        }
        Ok(None)
    }

    /// Port of the private `PolylineTrace.splitInsideDrillPadProhibited`
    /// (PolylineTrace.java:768-792): may the trace be split where line `line_index` meets `line`?
    ///
    /// It may not, if the intersection is inside a same-net pin pad but not at that pin's centre.
    /// Java's own comment records why vias are excluded: extending the rule to them broke
    /// connections when the autorouter connected to a trace.
    //
    // Java bug: `currentTrace != this && a || b` parses as `(currentTrace != this && a) || b`
    // (PolylineTrace.java:786-787), so the `lastCorner` half also fires for *this* trace and for
    // a foreign trace whose first corner does not match. Reproduced; see docs/java-quirks.md.
    fn split_inside_drill_pad_prohibited(
        &self,
        id: ItemId,
        line_index: usize,
        line: &Line,
    ) -> bool {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            // PolylineTrace.java:769-771: Java's `board == null` guard.
            return false;
        };
        let Some(this_line) = trace.polyline().lines().get(line_index) else {
            return false;
        };
        let intersection = this_line.intersection(line);
        let layer = trace.get_layer();
        let this_item = &self.items[&id];
        let mut pad_found = false;
        // `pickItems` answers a `TreeSet<Item>`, i.e. descending id (quirk #44), and this loop
        // can return early, so the order is observable.
        for other_id in self
            .pick_items(&intersection, Some(layer))
            .into_iter()
            .rev()
        {
            let Some(other) = self.items.get(&other_id) else {
                continue;
            };
            // PolylineTrace.java:777-779.
            if !other.shares_net(this_item) {
                continue;
            }
            match other {
                Item::Pin(_) => {
                    // PolylineTrace.java:780-785.
                    if self.drill_center(other_id).as_ref() == Some(&intersection) {
                        return false;
                    }
                    pad_found = true;
                }
                Item::Trace(other_trace) => {
                    // PolylineTrace.java:786-789, with Java's precedence bug kept.
                    if (other_id != id
                        && other_trace.first_corner().as_ref() == Some(&intersection))
                        || other_trace.last_corner().as_ref() == Some(&intersection)
                    {
                        return false;
                    }
                }
                _ => {}
            }
        }
        pad_found
    }

    // -- normalize (PolylineTraceNormalization.java) ----------------------------------------------

    /// Port of `PolylineTrace.normalize(IntOctagon)` (PolylineTrace.java:801-803) and the
    /// package-private `PolylineTraceNormalization.normalize(PolylineTrace, IntOctagon)`
    /// (PolylineTraceNormalization.java:20-22): split this trace and the traces it overlaps, then
    /// combine every piece. `true` if anything changed.
    pub fn normalize_trace(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
    ) -> Result<bool, BoardError> {
        self.normalize_trace_at_depth(id, clip, 0)
    }

    /// [`Board::normalize_trace`] under a [`StopCheck`], threaded through the recursion and into
    /// [`Board::split_trace_checked`] (Plan 3 ruling 4). A trip answers [`BoardError::Stopped`].
    // added in Plan 3: PolylineTrace.normalize (plan ruling 4)
    pub fn normalize_trace_checked(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        self.normalize_trace_at_depth_checked(id, clip, 0, stop)
    }

    /// Port of the private `PolylineTraceNormalization.normalize(PolylineTrace, IntOctagon, int)`
    /// (PolylineTraceNormalization.java:24-132), the recursion [`Board::normalize_trace`] enters
    /// at depth 0.
    ///
    /// Over [`MAX_NORMALIZATION_DEPTH`] the answer is `Ok(false)` — Java **returns** rather than
    /// throwing (:26-41), and its own comment explains why that is safe: the outer
    /// `normalizeTraces` loop reads `false` as "nothing changed for this trace", the geometry
    /// stays structurally valid, its endpoint contacts are preserved, and `removeTails` will not
    /// touch it.
    ///
    /// `pub` so that a test can enter the recursion at a depth it would take a pathological board
    /// to reach; Java's overload is private and its callers all start at 0.
    // not ported: the observer bracket (PolylineTraceNormalization.java:49-57,128-130), the
    // over-depth `FRLogger.debug` (:32-39), the net-49 `FRLogger.trace` blocks (:43-47,60-101)
    // and the degenerate-trace `FRLogger.debug` (:111-120) — logging and observers, which
    // `global-constraints.md` drops.
    pub fn normalize_trace_at_depth(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
        depth: u32,
    ) -> Result<bool, BoardError> {
        self.normalize_trace_at_depth_checked(id, clip, depth, &|| false)
    }

    /// [`Board::normalize_trace_at_depth`] under a [`StopCheck`] (Plan 3 ruling 4). The check is
    /// consulted once per recursion level and, through [`Board::split_trace_checked`], on every
    /// step of the entry walk quirk #76 hangs in.
    // added in Plan 3: PolylineTraceNormalization.normalize (plan ruling 4)
    pub fn normalize_trace_at_depth_checked(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
        depth: u32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        if stop() {
            return Err(BoardError::Stopped);
        }
        // PolylineTraceNormalization.java:26-41.
        if depth > MAX_NORMALIZATION_DEPTH {
            return Ok(false);
        }
        // PolylineTraceNormalization.java:58-59.
        let split_pieces = self.split_trace_checked(id, clip, stop)?;
        let mut result = split_pieces.len() != 1;
        for piece in split_pieces {
            // PolylineTraceNormalization.java:86-87.
            if !self.items.get(&piece).is_some_and(Item::is_on_the_board) {
                continue;
            }
            let trace_combined = self.combine_trace(piece)?;
            // PolylineTraceNormalization.java:102-103: a trace of a single corner.
            let degenerate = matches!(
                self.items.get(&piece),
                Some(Item::Trace(t))
                    if t.corner_count() == 2 && t.first_corner() == t.last_corner()
            );
            if degenerate {
                // PolylineTraceNormalization.java:104-121: a `USER_FIXED` degenerate trace cannot
                // be removed, and removing it anyway — or reporting `true` — would spin the outer
                // `normalizeTraces` loop forever.
                if !self.items[&piece].is_deletion_forbidden(&self.rules) {
                    self.remove_item(piece);
                    result = true;
                }
            } else if trace_combined {
                // PolylineTraceNormalization.java:122-125: the recursive result is discarded.
                self.normalize_trace_at_depth_checked(piece, clip, depth + 1, stop)?;
                result = true;
            }
        }
        Ok(result)
    }
}

/// Which end of the trace a combine is happening at — the one difference between
/// `PolylineTrace.combineAtStart` and `combineAtEnd`, which are otherwise line-for-line the same.
///
/// Not a Java type: Java writes the two methods out twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CombineEnd {
    /// `combineAtStart` (PolylineTrace.java:201-332).
    Start,
    /// `combineAtEnd` (PolylineTrace.java:341-456).
    End,
}

impl Board {
    /// Port of `PolylineTrace.change(Polyline)` (PolylineTrace.java:936-1005): replace a trace's
    /// geometry, reusing every search-tree entry the two polylines have in common, and normalise
    /// what is left.
    ///
    /// Java returns `void` and swallows a failed normalisation with `FRLogger.error` (:1002-1004),
    /// so this returns `()` as well — the second and last place a
    /// [`crate::BoardError`] stops (the other is [`Board::insert_trace`]).
    ///
    /// # Java compares the two line arrays by **object identity**
    ///
    /// `PolylineTrace.java:960` and `:972` use `!=` on `Line` objects, not `equals` — and `Line`
    /// does not override `equals` in a way this code could reach anyway, so what the two loops
    /// find is the first and the last line that is *not the same object*. The port reproduces
    /// that with [`Line::is_same_object`](fr_geometry::Line::is_same_object): the port's `Line` is a `Copy` value that
    /// carries an identity token taken at construction, so a value copied out of the old
    /// polyline compares "same object" and a freshly constructed line never does, exactly as in
    /// Java.
    ///
    /// The two indices are not cosmetic. They set `keepAtStartCount` / `keepAtEndCount`, which
    /// decide how many of the trace's search-tree leaves `ShapeSearchTree.changeEntries` reuses
    /// rather than removes and re-inserts — and a leaf removed and re-inserted lands somewhere
    /// else in `MinAreaTree`, so the *shape* of the search tree diverges. A value comparison
    /// keeps more leaves (a tightener that rebuilds a line with an unchanged value looks
    /// "unchanged" to it), and `ShapeSearchTree45Degree.completeShape` — whose obstacle order is
    /// its tree-walk order — then completes a different room. Plan 6 Task 17b bisected the
    /// `router-dac2020-bm01` `ripupPassNo >= 2` divergence to exactly this: quirk #74.
    ///
    /// Java's "both polylines are equal, no change necessary" early returns (`:963`, `:975`) are
    /// therefore reachable only for an array whose lines are the very objects already stored,
    /// and the port now takes them under the same condition.
    ///
    // obligation: `PolylineTrace.change`'s `board.additionalUpdateAfterChange(this)`
    // (PolylineTrace.java:944 — the plan's `:942` predates a HEAD edit) needs an
    // `AutorouteEngine`, which `fr-board` cannot name, so this method cannot make it.
    // Plan 6 Task 15b (controller ruling AB) makes it **at the call site** instead:
    // `fr_router::board_ext::PolylineTraceExt::pull_tight_with_engine` runs it, guarded by
    // Java's own `isOnTheBoard()` test (`:938-942`), immediately before calling this
    // method. Every further caller Plan 7 wires up — `PolylineTrace.correctConnectionToPin`
    // (`:1229`) and `TraceShover`'s two `change` calls (`:385`, `:540`) — must do the same.
    // not ported: `board.itemList.saveForUndo(this)` (:948) — no undo stack (Task 12) — the
    // observer notification (:987-990) and the `FRLogger.error` in the catch (:1003).
    pub fn change_trace(&mut self, id: ItemId, new_polyline: Polyline) {
        let Some(item) = self.items.get(&id) else {
            return;
        };
        // PolylineTrace.java:937-941.
        if !item.is_on_the_board() {
            if let Some(Item::Trace(trace)) = self.items.get_mut(&id) {
                trace.set_polyline(new_polyline);
            }
            return;
        }
        let Item::Trace(trace) = item else {
            return;
        };
        let layer = trace.get_layer();
        let old_lines = trace.polyline().lines().to_vec();
        let new_lines = new_polyline.lines();

        // PolylineTrace.java:955-967: the first line of the new polyline that differs.
        let last_index = new_lines.len().min(old_lines.len());
        let mut index_of_first_different_line = last_index;
        for i in 0..last_index {
            // Java bug: PolylineTrace.change compares `newPolyline.lines[i] != lines.lines[i]`
            // (PolylineTrace.java:960) — `!=` on two `Line` objects, i.e. **reference identity**,
            // where `Line.equals` (Line.java:57-75) is what the loop reads as if it meant.
            // Reproduced, quirk #74: `Line::is_same_object` is that identity.
            if !new_lines[i].is_same_object(&old_lines[i]) {
                index_of_first_different_line = i;
                break;
            }
        }
        if index_of_first_different_line == last_index {
            return;
        }
        // PolylineTrace.java:968-979: and the last.
        let mut index_of_last_different_line: i64 = -1;
        for i in 1..=last_index {
            // Java bug: PolylineTrace.change, PolylineTrace.java:972 — the same `!=` identity
            // comparison as at `:960`, on the tail. Reproduced, quirk #74.
            if !new_lines[new_lines.len() - i].is_same_object(&old_lines[old_lines.len() - i]) {
                index_of_last_different_line = (new_lines.len() - i) as i64;
                break;
            }
        }
        if index_of_last_different_line < 0 {
            return;
        }
        // PolylineTrace.java:980-984.
        let keep_at_start_count = index_of_first_different_line.saturating_sub(2);
        let keep_at_end_count =
            (new_lines.len() as i64 - index_of_last_different_line - 3).max(0) as usize;
        self.change_trace_entries(id, &new_polyline, keep_at_start_count, keep_at_end_count);
        if let Some(Item::Trace(trace)) = self.items.get_mut(&id) {
            trace.set_polyline(new_polyline);
        }
        // PolylineTrace.java:992-1004. The second of the two places a normalisation failure is
        // swallowed rather than propagated (the other is `Board::insert_trace`,
        // BasicBoard.java:230-241): Java wraps this call in its own `catch (Exception)`
        // (PolylineTrace.java:1000-1004) and only logs, so the port drops the `BoardError` here
        // too. Every other caller of `normalize_trace` threads it out.
        let clip_shape = self.changed_area.as_ref().map(|area| area.get_area(layer));
        let _ = self.normalize_trace(id, clip_shape.as_ref());
    }
}
