//! `Item.clearanceViolations` and the three members around it: the count, the private bisection
//! that measures the actual clearance, and `Via`'s escape-via override — plus the two headless
//! statics of `drc.ClearanceViolation`.
//!
//! Java: `board/model/items/Item.java:357-493`, `board/model/items/Via.java:88-112`,
//! `drc/ClearanceViolation.java:64-94`.
//!
//! They are `Board` methods rather than [`Item`] methods because Java reaches `this.board` for
//! the search tree, the rules and the other items, and because the body **mutates two things**
//! (plan-5 ruling 8): the queried item's `smallestClearance` (Item.java:451-453) and the search
//! tree's entry counter (`ShapeSearchTree.lastGeneratedEntryId`, ShapeSearchTree.java:55, a
//! `SearchTreeManager` field here). Hence `&mut self`.

use fr_geometry::{TileShape, java_round};

use crate::Board;
use crate::board::item_ctx;
use crate::ids::{ItemId, TreeObject};
use crate::items::{ClearanceViolation, Item, ItemCtx};

impl Board {
    /// Port of `Item.clearanceViolationCount` (Item.java:357-361).
    pub fn clearance_violation_count(&mut self, id: ItemId) -> usize {
        self.clearance_violations(id).len()
    }

    /// Port of `Item.clearanceViolations` (Item.java:363-469) **and** `Via`'s override
    /// (Via.java:88-112), which Java reaches through dynamic dispatch on the same name.
    ///
    /// The result is in the search tree's own entry order — Java collects into a `LinkedList`
    /// (`:368`) while walking the `TreeSet<EntrySortedByClearance>` that
    /// [`ShapeSearchTree::overlapping_tree_entries_with_clearance`](crate::ShapeSearchTree::overlapping_tree_entries_with_clearance)
    /// reproduces (clearance first, then entry id).
    ///
    /// `&mut self`: the call lowers `this.smallestClearance` (`:451-453`) and advances the
    /// search-tree entry counter, exactly as Java's does.
    pub fn clearance_violations(&mut self, id: ItemId) -> Vec<ClearanceViolation> {
        // Item.java:368-371: an item with no board answers the empty list; an id this board does
        // not know is the same thing.
        let mut result: Vec<ClearanceViolation> = Vec::new();
        let Some(item) = self.items.get(&id) else {
            return result;
        };
        let this_clearance_class = item.header().clearance_class();
        let this_is_trace = matches!(item, Item::Trace(_));
        // Item.java:374, :375, :460: the loop bound and every `shapeLayer(i)` the body reads.
        // Java re-evaluates `tileShapeCount()` on every iteration and `shapeLayer(i)` three
        // times per shape; both are stable across the body (nothing it calls changes an item's
        // shape count or the layer a shape sits on), so hoisting them changes no value.
        let shape_layers: Vec<usize> = {
            let ctx = item_ctx!(self);
            (0..item.tile_shape_count(&ctx))
                .map(|i| item.shape_layer(i, &ctx))
                .collect()
        };
        // Item.java:386, :393: `thisTrace.getNormalContacts(firstCorner(), true)` and the same
        // at `lastCorner()`. Java recomputes them per candidate; both sets are pure functions of
        // the trace and the board (the query behind them neither caches nor advances the entry
        // counter), so hoisting them out of the loop is observationally identical and keeps the
        // `&self` call away from the `&mut` borrow the tree query needs.
        let (first_corner, last_corner) = match item {
            Item::Trace(trace) => (trace.first_corner(), trace.last_corner()),
            _ => (None, None),
        };
        let contacts_at_first =
            first_corner.map(|point| self.trace_normal_contacts_at(id, &point, true));
        let contacts_at_last =
            last_corner.map(|point| self.trace_normal_contacts_at(id, &point, true));

        for (i, &layer) in shape_layers.iter().enumerate() {
            // Item.java:375, :434: `shape1` is this item's i-th tile shape.
            // totalized: Item.clearanceViolations' `shape1 == null` arm (Item.java:436-440) is unreachable in Java — a null `currentTileShape` reaches `overlappingTreeEntriesWithClearance` first (:376-378) and NPEs there — so the port skips the shape index instead of crashing. The `FRLogger.warn` is dropped either way.
            let Some(current_tile_shape) = self.item_tile_shape(id, i) else {
                continue;
            };
            // Item.java:376-378. `ignoreNetNos` is `new int[0]`: same-net filtering is
            // `isObstacle`'s job below, not the query's.
            let entries = {
                let Board {
                    items,
                    trees,
                    library,
                    components,
                    rules,
                    bounding_box,
                    max_tree_shape_width,
                    ..
                } = self;
                let ctx = ItemCtx {
                    library,
                    components,
                    rules,
                    bounding_box,
                    max_tree_shape_width: *max_tree_shape_width,
                };
                let (tree, counter) = trees.default_tree_and_counter_mut();
                tree.overlapping_tree_entries_with_clearance(
                    &current_tile_shape,
                    Some(layer),
                    &[],
                    this_clearance_class,
                    &*items,
                    &ctx,
                    counter,
                )
            };

            for entry in entries {
                // Item.java:380-382: skip anything that is not an item, and skip this item.
                let TreeObject::Item(other_id) = entry.object else {
                    continue;
                };
                if other_id == id {
                    continue;
                }
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                // Item.java:379: `currentItem.isObstacle(this)` — the *candidate's* method.
                let mut is_obstacle = {
                    let ctx = item_ctx!(self);
                    other.is_obstacle(&self.items[&id], &ctx)
                };

                // Item.java:383-411: two traces connected to the same tie pin may overlap
                // without sharing a net.
                if is_obstacle && this_is_trace && matches!(other, Item::Trace(_)) {
                    // Item.java:386-397: the first-corner contacts, then — only if they did not
                    // contain the candidate — the last-corner ones. Java **reuses**
                    // `currentContacts` for the second test, so the pin scan at :399-408 runs
                    // over whichever set was assigned last, which is the matching one whenever
                    // `contactFound`.
                    let current_contacts = match (&contacts_at_first, &contacts_at_last) {
                        (Some(first), _) if first.contains(&other_id) => Some(first),
                        (_, Some(last)) if last.contains(&other_id) => Some(last),
                        _ => None,
                    };
                    if let Some(current_contacts) = current_contacts {
                        for contact_id in current_contacts {
                            // Item.java:401-406.
                            let Some(contact @ Item::Pin(_)) = self.items.get(contact_id) else {
                                continue;
                            };
                            if contact.shares_net(&self.items[&id])
                                && contact.shares_net(&self.items[&other_id])
                            {
                                is_obstacle = false;
                                break;
                            }
                        }
                    }
                }

                // Item.java:413.
                if !is_obstacle {
                    continue;
                }
                // Item.java:435: `shape2 = currentItem.getTileShape(entry.shapeIndexInObject)`.
                // The live half of Java's null check (:436-440) — reached on
                // `Issue754-avionics_hub.dsn`, where `ConvexObstacle.getTileShape` answers null
                // out of range — is this `continue`; only its `FRLogger.warn` is dropped.
                let Some(shape2) = self.item_tile_shape(other_id, entry.shape_index) else {
                    continue;
                };
                let other_clearance_class = self.items[&other_id].header().clearance_class();
                // Item.java:424-425: `getValue(currentItem.clearanceClassIndex,
                // this.clearanceClassIndex, shapeLayer(i), false)` — **the other item's class
                // first**, and `getValue` reads `row[classJ].column[classI]` (quirk #83), so
                // transposing the two is a silent wrong answer on an asymmetric matrix. The
                // `false` is `addSafetyMargin`; the tree pre-filter passes `true`
                // (ShapeSearchTree.java:484) and the two are deliberately not unified.
                let minimum_clearance = f64::from(self.rules.clearance_matrix.get_value(
                    other_clearance_class,
                    this_clearance_class,
                    layer,
                    false,
                ));

                // Item.java:427-439. Headless freerouting never turns clearance compensation on
                // (`SearchTreeManager.java:35`; both setters are GUI), so the `else` arm is the
                // parity path — both are ported.
                let (cl_comp1, cl_comp2) = if self.trees.is_clearance_compensation_used() {
                    let tree = self.trees.get_default_tree();
                    (
                        tree.clearance_compensation_value(this_clearance_class, layer, &self.rules),
                        tree.clearance_compensation_value(
                            other_clearance_class,
                            layer,
                            &self.rules,
                        ),
                    )
                } else {
                    let cl_comp1 = java_round(0.5 * minimum_clearance) as i32;
                    (
                        cl_comp1,
                        java_round(minimum_clearance - f64::from(cl_comp1)) as i32,
                    )
                };

                // Item.java:441-442: each shape is enlarged only when its own component is > 0.
                let enlarged_shape1 = if cl_comp1 > 0 {
                    current_tile_shape.enlarge(f64::from(cl_comp1))
                } else {
                    current_tile_shape.clone()
                };
                let enlarged_shape2 = if cl_comp2 > 0 {
                    shape2.enlarge(f64::from(cl_comp2))
                } else {
                    shape2.clone()
                };
                // Item.java:444-445.
                let intersection = enlarged_shape1.intersection(&enlarged_shape2);
                if intersection.dimension() != 2 {
                    continue;
                }
                // Item.java:447-449.
                let actual_clearance = Board::calculate_clearance_between_two_shapes(
                    &current_tile_shape,
                    &shape2,
                    minimum_clearance,
                    cl_comp1,
                    cl_comp2,
                );
                // Item.java:451-453 — the monotone minimum on the *queried* item. Java bug: Item.clearanceViolations never resets `smallestClearance`, so it only ever falls, over the whole life of the item (quirk #153).
                let header = self
                    .items
                    .get_mut(&id)
                    .expect("Board::clearance_violations: present, just read")
                    .header_mut();
                if header.smallest_clearance < 0.0 || actual_clearance < header.smallest_clearance {
                    header.smallest_clearance = actual_clearance;
                }
                // Item.java:455-463.
                result.push(ClearanceViolation {
                    first_item: id,
                    second_item: other_id,
                    shape: intersection,
                    layer,
                    expected_clearance: minimum_clearance,
                    actual_clearance,
                });
            }
        }

        // Via.java:88-112, which runs after the base list is built.
        if let Some(Item::Via(via)) = self.items.get(&id)
            && via.is_escape_via
            && via.escape_via_smd_layer >= 0
        {
            let smd_layer = via.escape_via_smd_layer as usize;
            let this_item = &self.items[&id];
            // Via.java:95-105: `firstItem == this ? secondItem : (secondItem == this ?
            // firstItem : null)`, where the `null` arm — neither item is this via — **keeps**
            // the violation. `first_item` is always the queried item here, so that arm is
            // unreachable; it is transcribed rather than simplified away.
            result.retain(|violation| {
                if violation.layer != smd_layer {
                    return true;
                }
                let other = if violation.first_item == id {
                    Some(violation.second_item)
                } else if violation.second_item == id {
                    Some(violation.first_item)
                } else {
                    None
                };
                match other.and_then(|other| self.items.get(&other)) {
                    Some(other) => !other.shares_net(this_item),
                    None => true,
                }
            });
        }
        result
    }

    /// Port of the private `Item.calculateClearanceBetweenTwoShapes` (Item.java:471-493): the
    /// largest enlargement at which the two **raw** shapes still do not overlap, found by
    /// exactly 16 halvings of `[0, minimum_clearance]`.
    ///
    /// The answer is the last `low`, never `mid` (`:492`), and the two components decide how the
    /// enlargement is split between the shapes. Nothing here reads the board, so it is an
    /// associated function; Java's is a private instance method that never touches `this`.
    pub fn calculate_clearance_between_two_shapes(
        raw_shape1: &TileShape,
        raw_shape2: &TileShape,
        minimum_clearance: f64,
        cl_comp1: i32,
        cl_comp2: i32,
    ) -> f64 {
        // Item.java:472-474: shapes that already overlap in dimension 2 have zero clearance.
        if raw_shape1.intersection(raw_shape2).dimension() == 2 {
            return 0.0;
        }
        // Item.java:475-482. `clComp1 + clComp2` is an `int` addition in Java, i.e. wrapping;
        // `wrapping_add` keeps that rather than panicking in a debug build.
        let mut low = 0.0;
        let mut high = minimum_clearance;
        let sum_comp = f64::from(cl_comp1.wrapping_add(cl_comp2));
        let factor1 = if sum_comp > 0.0 {
            f64::from(cl_comp1) / sum_comp
        } else {
            0.5
        };
        let factor2 = if sum_comp > 0.0 {
            f64::from(cl_comp2) / sum_comp
        } else {
            0.5
        };
        // Item.java:484-491: exactly 16 iterations.
        for _ in 0..16 {
            let mid = (low + high) * 0.5;
            let s1 = raw_shape1.enlarge(mid * factor1);
            let s2 = raw_shape2.enlarge(mid * factor2);
            if s1.intersection(&s2).dimension() == 2 {
                high = mid;
            } else {
                low = mid;
            }
        }
        // Item.java:492.
        low
    }

    /// Port of `ClearanceViolation.aggregateSortedBySeverity` (ClearanceViolation.java:64-75)
    /// over this board's own items, i.e. Java's `aggregateSortedBySeverity(board.getItems())`
    /// (RatsnestClearanceHeadlessTest.java:96).
    ///
    /// **Every pair is reported twice** — once on each item — exactly as Java's class doc says
    /// (`:52-55`); the deduplicated whole-board list is `DesignRulesChecker`'s
    /// (`getAllClearanceViolations`), which Plan 5 Task 4 owns.
    ///
    /// The walk is `board.getItems()` order (descending id, quirk #63), which matters because
    /// the sort is stable in Java (`List.sort` is a TimSort) and in Rust.
    pub fn aggregate_violations_sorted_by_severity(&mut self) -> Vec<ClearanceViolation> {
        // ClearanceViolation.java:65-68.
        let mut violations = Vec::new();
        for id in self.items_in_board_order() {
            violations.extend(self.clearance_violations(id));
        }
        // ClearanceViolation.java:69-74: `-Double.compare(shortfall1, shortfall2)`, i.e. the
        // largest expected-minus-actual first. `total_cmp` is `Double.compare` for every value
        // reachable here (both fields are finite).
        violations.sort_by(|a, b| {
            (b.expected_clearance - b.actual_clearance)
                .total_cmp(&(a.expected_clearance - a.actual_clearance))
        });
        violations
    }

    /// Port of `ClearanceViolation.smallestClearance` (ClearanceViolation.java:86-94) over this
    /// board's own items.
    ///
    /// Reads the `smallestClearance` field that [`Self::clearance_violations`] fills in, so it
    /// is only meaningful after [`Self::aggregate_violations_sorted_by_severity`] (or an
    /// equivalent walk) has run; on an untouched board every item still carries the `-1.0`
    /// sentinel and the answer is `Double.MAX_VALUE`.
    pub fn smallest_clearance(&self) -> f64 {
        // ClearanceViolation.java:87-93.
        let mut smallest = f64::MAX;
        for item in self.get_items() {
            let item_smallest = item.header().smallest_clearance;
            if item_smallest >= 0.0 && item_smallest < smallest {
                smallest = item_smallest;
            }
        }
        smallest
    }
}
