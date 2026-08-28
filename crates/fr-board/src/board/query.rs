//! The search half of [`Board`]: `BasicBoard`'s overlap and check queries
//! (BasicBoard.java:918-1099) plus `RoutingBoardSearchFacade`
//! (`board/facade/RoutingBoardSearchFacade.java`).
//!
//! # Why several of these take `&mut self`
//!
//! Java's clearance queries bump `ShapeSearchTree.lastGeneratedEntryId`, a **static** counter
//! used as a tie-break (ShapeSearchTree.java:55,1144-1148). The port makes it a field of
//! [`SearchTreeManager`](crate::searchtree::SearchTreeManager) (global-constraints.md), so every
//! query that reaches `overlappingTreeEntriesWithClearance` mutates the board. The queries that
//! do not — [`Board::overlapping_objects`], [`Board::overlapping_items`], [`Board::pick_items`] —
//! stay `&self`.

use std::collections::BTreeSet;

use fr_geometry::{Area, LineSegment, Point, Polyline, ShapeOps, TileShape, Vector};

use crate::ids::{ItemId, TreeObject};
use crate::items::Item;
use crate::structure::FixedState;

use super::connectivity::StopConnectionOption;
use super::{Board, item_ctx};

impl Board {
    // -- the plain overlap queries ---------------------------------------------------------------

    /// Port of `BasicBoard.overlappingObjects(ConvexShape, int)` (BasicBoard.java:918-920).
    ///
    /// `layer` is `Option<usize>`; `None` is Java's "if layer < 0, the layer is ignored".
    pub fn overlapping_objects(
        &self,
        shape: &TileShape,
        layer: Option<usize>,
    ) -> BTreeSet<TreeObject> {
        let ctx = self.ctx();
        self.trees
            .get_default_tree()
            .overlapping_objects(shape, layer, &[], &self.items, &ctx)
    }

    /// Port of `BasicBoard.overlappingItems(Area, int)` (BasicBoard.java:938-950): the items
    /// overlapping any convex piece of `area`.
    pub fn overlapping_items(&self, area: &Area, layer: Option<usize>) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        // Java dereferences `area.splitToConvex()` without a null check
        // (BasicBoard.java:940-941); the `expect` reproduces that NullPointerException.
        let tiles = area.split_to_convex().expect(
            "BasicBoard.overlappingItems: area.splitToConvex() is null — Java throws a \
             NullPointerException here too (BasicBoard.java:941)",
        );
        for tile in tiles {
            for object in self.overlapping_objects(&tile, layer) {
                if let TreeObject::Item(id) = object {
                    result.insert(id);
                }
            }
        }
        result
    }

    /// Port of `BasicBoard.overlappingItemsWithClearance`
    /// (BasicBoard.java:928-932).
    ///
    /// The result is a `Vec<ItemId>` in Java's `TreeSet<Item>` order — **descending** id — for
    /// the reason `ShapeSearchTree::overlapping_items_with_clearance` gives.
    pub fn overlapping_items_with_clearance(
        &mut self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class: usize,
    ) -> Vec<ItemId> {
        let ctx = item_ctx!(self);
        let entry_counter = self.trees.entry_counter_mut();
        let counter_value = &mut *entry_counter;
        // The default tree and the counter both live in `trees`, so take a snapshot of the
        // counter, run the query against the tree, and write the counter back.
        let mut counter = *counter_value;
        let result = self
            .trees
            .get_default_tree()
            .overlapping_items_with_clearance(
                shape,
                layer,
                ignore_net_nos,
                clearance_class,
                &self.items,
                &ctx,
                &mut counter,
            );
        *self.trees.entry_counter_mut() = counter;
        result
    }

    /// Port of `BasicBoard.pickItems(Point, int, ItemSelectionFilter)`
    /// (BasicBoard.java:1086-1099), without the filter — see [`Board`]'s `not ported:` note on
    /// `ItemSelectionFilter`.
    pub fn pick_items(&self, location: &Point, layer: Option<usize>) -> BTreeSet<ItemId> {
        let point_shape = TileShape::Box(TileShape::get_instance_from_point(location));
        self.overlapping_objects(&point_shape, layer)
            .into_iter()
            .filter_map(|object| match object {
                TreeObject::Item(id) => Some(id),
                TreeObject::Room(_) => None,
            })
            .collect()
    }

    /// [`Board::pick_items`] restricted to traces — Java's
    /// `new ItemSelectionFilter(SelectableChoices.TRACES)`, which is what
    /// `BasicBoard.splitTraces` (:892-894) and `RoutingBoard.insertForcedTracePolyline` (:492-494)
    /// both build.
    ///
    /// Not a Java method; it stands in for the one `ItemSelectionFilter` use the model needs.
    pub fn pick_traces(&self, location: &Point, layer: Option<usize>) -> BTreeSet<ItemId> {
        self.pick_items(location, layer)
            .into_iter()
            .filter(|id| self.items.get(id).is_some_and(Item::is_trace))
            .collect()
    }

    // -- the check queries -------------------------------------------------------------------------

    /// Port of `BasicBoard.checkShape` (BasicBoard.java:956-980): can an object of this shape,
    /// nets and clearance class be inserted on `layer` without a clearance violation?
    pub fn check_shape(
        &mut self,
        shape: &Area,
        layer: Option<usize>,
        net_nos: &[i32],
        clearance_class: usize,
    ) -> bool {
        let bounding_box = self.bounding_box;
        // BasicBoard.java:957: `shape.splitToConvex()` is dereferenced with no null check.
        let tiles = shape.split_to_convex().expect(
            "BasicBoard.checkShape: shape.splitToConvex() is null — Java throws a \
             NullPointerException here too (BasicBoard.java:957)",
        );
        for tile in tiles {
            // BasicBoard.java:961-963.
            if !tile.is_contained_in(&bounding_box) {
                return false;
            }
            let obstacles =
                self.overlapping_objects_with_clearance(&tile, layer, net_nos, clearance_class);
            for object in obstacles {
                let TreeObject::Item(other_id) = object else {
                    continue;
                };
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                // BasicBoard.java:968-976: an obstacle only if it obstructs **every** net.
                let is_obstacle = net_nos
                    .iter()
                    .all(|net_no| other.is_obstacle_for_net(*net_no));
                if is_obstacle {
                    return false;
                }
            }
        }
        true
    }

    /// Port of `BasicBoard.checkTraceShape` (BasicBoard.java:988-1047): can a trace line of this
    /// shape be inserted on `layer` without a clearance violation?
    ///
    /// `contact_pins` is Java's `Set<Pin> contactPins`: when it is `Some`, every pin *not* in it
    /// counts as an obstacle even on the trace's own net, which is what keeps the router out of
    /// acid traps (BasicBoard.java:1010-1014).
    pub fn check_trace_shape(
        &mut self,
        shape: &TileShape,
        layer: usize,
        net_nos: &[i32],
        clearance_class: usize,
        contact_pins: Option<&BTreeSet<ItemId>>,
    ) -> bool {
        // BasicBoard.java:990-992.
        if !shape.is_contained_in(&self.bounding_box) {
            return false;
        }
        let compensation_used = self
            .trees
            .get_default_tree()
            .is_clearance_compensation_used();
        // BasicBoard.java:996-1001.
        let tree_entries = if compensation_used {
            let ctx = self.ctx();
            self.trees.get_default_tree().overlapping_tree_entries(
                shape,
                Some(layer),
                &[],
                &self.items,
                &ctx,
            )
        } else {
            self.overlapping_tree_entries_with_clearance(shape, Some(layer), &[], clearance_class)
        };
        for entry in tree_entries {
            let TreeObject::Item(other_id) = entry.object else {
                continue;
            };
            let Some(other) = self.items.get(&other_id) else {
                continue;
            };
            if let Some(contact_pins) = contact_pins {
                // BasicBoard.java:1006-1015.
                if contact_pins.contains(&other_id) {
                    continue;
                }
                if matches!(other, Item::Pin(_)) {
                    return false;
                }
            }
            // BasicBoard.java:1016-1021.
            let mut is_obstacle = net_nos
                .iter()
                .all(|net_no| other.is_trace_obstacle(*net_no));
            let other_is_trace = other.is_trace();
            // BasicBoard.java:1022-1041: a foreign-net trace inside a tie pin's shape is not an
            // obstacle. The qualifying pins are collected first so the shape lookups below can
            // take `&mut self` (they fill the tree-shape cache, Item.java:227-238).
            if is_obstacle
                && other_is_trace
                && let Some(contact_pins) = contact_pins
            {
                // BasicBoard.java:1027-1029.
                let tie_pins: Vec<ItemId> = contact_pins
                    .iter()
                    .copied()
                    .filter(|pin_id| match self.items.get(pin_id) {
                        Some(pin @ Item::Pin(_)) => {
                            pin.net_count() > 1 && pin.shares_net(&self.items[&other_id])
                        }
                        _ => false,
                    })
                    .collect();
                let mut intersection: Option<TileShape> = None;
                for pin_id in tie_pins {
                    if intersection.is_none() {
                        // BasicBoard.java:1030-1034.
                        let Some(obstacle_trace_shape) =
                            self.item_tile_shape(other_id, entry.shape_index)
                        else {
                            continue;
                        };
                        intersection = Some(shape.intersection(&obstacle_trace_shape));
                    }
                    let Some(pin_shape) = self.drill_item_tile_shape_on_layer(pin_id, layer) else {
                        continue;
                    };
                    if pin_shape.contains_approx(intersection.as_ref().expect("just set")) {
                        is_obstacle = false;
                        break;
                    }
                }
            }
            if is_obstacle {
                return false;
            }
        }
        true
    }

    /// Port of `BasicBoard.checkPolylineTrace` (BasicBoard.java:1053-1074).
    pub fn check_polyline_trace(
        &mut self,
        polyline: &Polyline,
        layer: usize,
        pen_half_width: i32,
        net_nos: &[i32],
        clearance_class: usize,
    ) -> bool {
        // BasicBoard.java:1055-1069: Java builds a temporary `PolylineTrace` that is never
        // inserted, purely to reach `tileShapeCount()`, `getTileShape(i)` and
        // `touchingPinsAtEndCorners()`. The port computes the same three things directly — but
        // the temporary still runs `Item`'s constructor, whose `id <= 0` branch draws from
        // `board.communication.idGenerator` (Item.java:85-90), so the id sequence advances by one
        // per call and every later insert on the board is numbered accordingly.
        self.new_item_id();
        //
        // `getTileShape(i)` on that temporary goes through the default tree
        // (Item.java:194-201 -> ShapeSearchTree.java:992-1004), so the shapes carry the
        // **compensated** half width and come from the tree's own `offsetShape` — which a
        // 90-degree tree overrides to `offsetBox` (ShapeSearchTree90Degree.java:486-490).
        let default_tree = self.trees.get_default_tree();
        let compensated_half_width = pen_half_width
            + default_tree.clearance_compensation_value(clearance_class, layer, &self.rules);
        // `PolylineTraceGeometry.tileShapeCount` is `max(lines.length - 2, 0)`.
        let shape_count = polyline.lines().len().saturating_sub(2);
        let shapes: Vec<TileShape> = (0..shape_count)
            .filter_map(|i| default_tree.offset_shape(polyline, compensated_half_width, i))
            .collect();
        let contact_pins = self.touching_pins_at_end_corners_of(
            polyline,
            layer,
            pen_half_width,
            net_nos,
            clearance_class,
        );
        for shape in shapes {
            if !self.check_trace_shape(&shape, layer, net_nos, clearance_class, Some(&contact_pins))
            {
                return false;
            }
        }
        true
    }

    /// Port of `Trace.touchingPinsAtEndCorners` (Trace.java:390-410) for an item already on the
    /// board.
    pub fn touching_pins_at_end_corners(&mut self, id: ItemId) -> BTreeSet<ItemId> {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return BTreeSet::new();
        };
        let polyline = trace.polyline().clone();
        let layer = trace.get_layer();
        let half_width = trace.get_half_width();
        let clearance_class = trace.hdr.clearance_class();
        let net_nos = trace.hdr.net_nos.clone();
        self.touching_pins_at_end_corners_of(
            &polyline,
            layer,
            half_width,
            &net_nos,
            clearance_class,
        )
    }

    /// The body of `Trace.touchingPinsAtEndCorners` (Trace.java:390-410) expressed over a bare
    /// polyline, so that `checkPolylineTrace`'s temporary trace (BasicBoard.java:1055-1066) does
    /// not have to exist.
    fn touching_pins_at_end_corners_of(
        &mut self,
        polyline: &Polyline,
        layer: usize,
        half_width: i32,
        net_nos: &[i32],
        clearance_class: usize,
    ) -> BTreeSet<ItemId> {
        let mut result = BTreeSet::new();
        let end_points = [polyline.first_corner(), polyline.last_corner()];
        for end_point in end_points.into_iter().flatten() {
            // Trace.java:397-398.
            let octagon = end_point
                .surrounding_octagon()
                .enlarge(f64::from(half_width));
            let overlaps = self.overlapping_items_with_clearance(
                &TileShape::Octagon(octagon),
                Some(layer),
                &[],
                clearance_class,
            );
            for other_id in overlaps {
                // Trace.java:403.
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                if matches!(other, Item::Pin(_)) && other.shares_net_no(net_nos) {
                    result.insert(other_id);
                }
            }
        }
        result
    }

    /// The `overlappingObjectsWithClearance` wrapper (ShapeSearchTree.java:530-549) reached
    /// through the board — Java writes `defaultTree.overlappingObjectsWithClearance(...)` inline
    /// at BasicBoard.java:965.
    fn overlapping_objects_with_clearance(
        &mut self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class: usize,
    ) -> BTreeSet<TreeObject> {
        let ctx = item_ctx!(self);
        let mut counter = self.trees.entry_counter();
        let result = self
            .trees
            .get_default_tree()
            .overlapping_objects_with_clearance(
                shape,
                layer,
                ignore_net_nos,
                clearance_class,
                &self.items,
                &ctx,
                &mut counter,
            );
        *self.trees.entry_counter_mut() = counter;
        result
    }

    /// The five-argument `overlappingTreeEntriesWithClearance`
    /// (ShapeSearchTree.java:443-507) reached through the board.
    fn overlapping_tree_entries_with_clearance(
        &mut self,
        shape: &TileShape,
        layer: Option<usize>,
        ignore_net_nos: &[i32],
        clearance_class: usize,
    ) -> Vec<crate::TreeEntry<TreeObject>> {
        let ctx = item_ctx!(self);
        let mut counter = self.trees.entry_counter();
        let result = self
            .trees
            .get_default_tree()
            .overlapping_tree_entries_with_clearance(
                shape,
                layer,
                ignore_net_nos,
                clearance_class,
                &self.items,
                &ctx,
                &mut counter,
            );
        *self.trees.entry_counter_mut() = counter;
        result
    }

    // -- RoutingBoardSearchFacade ------------------------------------------------------------------

    /// Port of `RoutingBoardSearchFacade.checkTraceSegment(Point, Point, …)`
    /// (RoutingBoardSearchFacade.java:29-44), which `RoutingBoard.checkTraceSegment`
    /// (RoutingBoard.java:198-215) delegates to.
    ///
    /// Returns the length of the segment from `from_point` that can be inserted without a
    /// clearance violation. `0` means "blocked at the start"; `i32::MAX as f64` means "no
    /// conflict at all" (RoutingBoardSearchFacade.java:61).
    ///
    /// Java's `new Polyline(fromPoint, toPoint)` followed by `new LineSegment(polyline, 1)`
    /// (:40-41) is the two-point polyline's middle segment.
    #[allow(clippy::too_many_arguments)] // Java's parameter list, kept.
    pub fn check_trace_segment(
        &mut self,
        from_point: &Point,
        to_point: &Point,
        layer: usize,
        net_nos: &[i32],
        trace_half_width: i32,
        clearance_class: usize,
        only_not_shovable_obstacles: bool,
    ) -> f64 {
        // RoutingBoardSearchFacade.java:37-39.
        if from_point == to_point {
            return 0.0;
        }
        let polyline = Polyline::from_two_points(from_point, to_point);
        let Some(line_segment) = LineSegment::from_polyline(&polyline, 1) else {
            // Java's `new LineSegment(polyline, 1)` would throw on a polyline with fewer than
            // three lines; `from_polyline` answers `None` for the same input.
            return 0.0;
        };
        self.check_trace_segment_of_line_segment(
            &line_segment,
            layer,
            net_nos,
            trace_half_width,
            clearance_class,
            only_not_shovable_obstacles,
        )
    }

    /// Port of `RoutingBoardSearchFacade.checkTraceSegment(LineSegment, …)`
    /// (RoutingBoardSearchFacade.java:46-109).
    // renamed: the `LineSegment` overload -> check_trace_segment_of_line_segment (Rust has no
    // overloading).
    #[allow(clippy::too_many_arguments)] // Java's parameter list, kept.
    pub fn check_trace_segment_of_line_segment(
        &mut self,
        line_segment: &LineSegment,
        layer: usize,
        net_nos: &[i32],
        trace_half_width: i32,
        clearance_class: usize,
        only_not_shovable_obstacles: bool,
    ) -> f64 {
        // RoutingBoardSearchFacade.java:53-56.
        let Ok(check_polyline) = line_segment.to_polyline() else {
            return 0.0;
        };
        if check_polyline.lines().len() != 3 {
            return 0.0;
        }
        let Some(shape_to_check) = check_polyline.offset_shape(trace_half_width, 0) else {
            return 0.0;
        };
        let from_point = line_segment.start_point_approx();
        let to_point = line_segment.end_point_approx();
        let line_length = to_point.distance(&from_point);
        // RoutingBoardSearchFacade.java:61.
        let mut ok_length = f64::from(i32::MAX);
        let compensation_used = self
            .trees
            .get_default_tree()
            .is_clearance_compensation_used();

        let obstacle_entries = self.overlapping_tree_entries_with_clearance(
            &shape_to_check,
            Some(layer),
            net_nos,
            clearance_class,
        );

        let default_tree = self.default_tree_id();
        for entry in obstacle_entries {
            let TreeObject::Item(obstacle_id) = entry.object else {
                continue;
            };
            let Some(obstacle) = self.items.get(&obstacle_id) else {
                continue;
            };
            // RoutingBoardSearchFacade.java:71-75.
            if only_not_shovable_obstacles
                && obstacle.is_routable()
                && !obstacle.is_shove_fixed(&self.rules)
            {
                continue;
            }
            // RoutingBoardSearchFacade.java:76-78: `getTreeShape(defaultTree, index)`, which
            // recomputes if the item's cache was dropped since insertion (Item.java:212-226).
            let Some(obstacle_shape) =
                self.item_tree_shape_ref(obstacle_id, default_tree, entry.shape_index)
            else {
                continue;
            };
            let obstacle_shape = obstacle_shape.into_owned();
            let obstacle = &self.items[&obstacle_id];
            // RoutingBoardSearchFacade.java:82-93.
            let (current_offset_shape, shorten_value) = if compensation_used {
                let compensation = self
                    .rules
                    .clearance_matrix
                    .clearance_compensation_value(obstacle.clearance_class(), layer);
                (
                    shape_to_check.clone(),
                    f64::from(trace_half_width + compensation),
                )
            } else {
                let clearance_value =
                    self.clearance_value(obstacle.clearance_class(), clearance_class, layer);
                (
                    shape_to_check.offset(f64::from(clearance_value)),
                    f64::from(trace_half_width + clearance_value),
                )
            };
            // RoutingBoardSearchFacade.java:94-97.
            let intersection = obstacle_shape.intersection(&current_offset_shape);
            if intersection.is_empty() {
                continue;
            }
            let Some(nearest_obstacle_point) = intersection.nearest_point_approx(&from_point)
            else {
                continue;
            };
            // RoutingBoardSearchFacade.java:99-105.
            let projection =
                from_point.scalar_product(&to_point, &nearest_obstacle_point) / line_length;
            let projection = 0.0_f64.max(projection - shorten_value - 1.0);
            if projection < ok_length {
                ok_length = projection;
                if ok_length <= 0.0 {
                    return 0.0;
                }
            }
        }
        ok_length
    }

    /// Port of `RoutingBoardSearchFacade.checkMoveItem`
    /// (RoutingBoardSearchFacade.java:111-145): can `id` be translated by `vector` without
    /// overlaps or clearance violations?
    ///
    /// Java's `ignoreItems` is an in/out parameter — it adds `item` to it (:123-125) — so the
    /// port takes `&mut Option<BTreeSet<ItemId>>`.
    pub fn check_move_item(
        &mut self,
        id: ItemId,
        vector: &Vector,
        ignore_items: &mut Option<BTreeSet<ItemId>>,
    ) -> bool {
        let ctx = self.ctx();
        let Some(item) = self.items.get(&id) else {
            return false;
        };
        // RoutingBoardSearchFacade.java:112-115.
        if item.net_count() > 1 {
            return false;
        }
        // RoutingBoardSearchFacade.java:116-122.
        let contact_count = if item.as_connectable().is_some() {
            self.all_contacts(id).len()
        } else {
            0
        };
        let item = &self.items[&id];
        if item.is_trace() && contact_count > 0 {
            return false;
        }
        // RoutingBoardSearchFacade.java:123-125.
        if let Some(ignore_items) = ignore_items {
            ignore_items.insert(id);
        }
        let net_nos = item.net_nos().to_vec();
        let clearance_class = item.clearance_class();
        let shape_layers: Vec<usize> = (0..item.tile_shape_count(&ctx))
            .map(|i| item.shape_layer(i, &ctx))
            .collect();
        let moved: Vec<(TileShape, usize)> = shape_layers
            .into_iter()
            .enumerate()
            .filter_map(|(i, layer)| {
                self.item_tile_shape(id, i)
                    .map(|shape| (shape.translate_by(vector), layer))
            })
            .collect();
        let bounding_box = self.bounding_box;
        for (moved_shape, shape_layer) in moved {
            // RoutingBoardSearchFacade.java:128-130.
            if !moved_shape.is_contained_in(&bounding_box) {
                return false;
            }
            let obstacles = self.overlapping_items_with_clearance(
                &moved_shape,
                Some(shape_layer),
                &net_nos,
                clearance_class,
            );
            let item = &self.items[&id];
            for other_id in obstacles {
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                // RoutingBoardSearchFacade.java:134-141.
                match ignore_items.as_ref() {
                    Some(ignore_items) => {
                        if !ignore_items.contains(&other_id) && other.is_obstacle(item, &self.ctx())
                        {
                            return false;
                        }
                    }
                    None => {
                        if other_id != id && other.is_obstacle(item, &self.ctx()) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    /// Port of `RoutingBoardSearchFacade.checkChangeNet`
    /// (RoutingBoardSearchFacade.java:147-163).
    pub fn check_change_net(&mut self, id: ItemId, new_net_no: i32) -> bool {
        let ctx = self.ctx();
        let Some(item) = self.items.get(&id) else {
            return false;
        };
        let clearance_class = item.clearance_class();
        let shape_layers: Vec<usize> = (0..item.tile_shape_count(&ctx))
            .map(|i| item.shape_layer(i, &ctx))
            .collect();
        let shapes: Vec<(TileShape, usize)> = shape_layers
            .into_iter()
            .enumerate()
            .filter_map(|(i, layer)| self.item_tile_shape(id, i).map(|shape| (shape, layer)))
            .collect();
        for (shape, shape_layer) in shapes {
            let obstacles = self.overlapping_items_with_clearance(
                &shape,
                Some(shape_layer),
                &[new_net_no],
                clearance_class,
            );
            for other_id in obstacles {
                if other_id == id {
                    continue;
                }
                let Some(other) = self.items.get(&other_id) else {
                    continue;
                };
                // RoutingBoardSearchFacade.java:155-159.
                if other.as_connectable().is_some() && !other.contains_net(new_net_no) {
                    return false;
                }
            }
        }
        true
    }

    /// Port of `RoutingBoardSearchFacade.pickNearestRoutingItem`
    /// (RoutingBoardSearchFacade.java:165-217): the connectable item nearest `location` that a
    /// route may start from or connect to.
    pub fn pick_nearest_routing_item(
        &self,
        location: &Point,
        layer: Option<usize>,
        from_item: Option<ItemId>,
    ) -> Option<ItemId> {
        let ctx = self.ctx();
        let point_shape = TileShape::Box(TileShape::get_instance_from_point(location));
        // `BasicBoard.overlappingItems` answers a `TreeSet<Item>`, i.e. **descending id**
        // (quirk #44), and the order decides which of two equidistant candidates wins the
        // `currentDistance < minDist` test (RoutingBoardSearchFacade.java:185,193).
        let found_items: Vec<ItemId> = self
            .overlapping_items(&Area::Shape(point_shape.into()), layer)
            .into_iter()
            .rev()
            .collect();
        let pick_location = location.to_float();
        let mut min_dist = f64::from(i32::MAX);
        let mut nearest_item: Option<ItemId> = None;
        let mut ignore_set: Option<BTreeSet<ItemId>> = None;
        for current_id in found_items {
            let Some(current) = self.items.get(&current_id) else {
                continue;
            };
            // RoutingBoardSearchFacade.java:173-175.
            if !current.is_connectable() {
                continue;
            }
            let nearest_is_drill = nearest_item
                .and_then(|id| self.items.get(&id))
                .is_some_and(Item::is_drill_item);
            let nearest_is_trace = nearest_item
                .and_then(|id| self.items.get(&id))
                .is_some_and(Item::is_trace);
            let mut candidate_found = false;
            let mut current_distance = 0.0;
            match current {
                // RoutingBoardSearchFacade.java:178-188.
                Item::Trace(trace) => {
                    if layer.is_none_or(|layer| trace.get_layer() == layer) {
                        if nearest_is_drill {
                            continue;
                        }
                        let trace_radius = f64::from(trace.get_half_width());
                        current_distance = trace.polyline().distance(&pick_location);
                        if current_distance < min_dist && current_distance <= trace_radius {
                            candidate_found = true;
                        }
                    }
                }
                // RoutingBoardSearchFacade.java:189-196.
                Item::Via(_) | Item::Pin(_) => {
                    if layer.is_none_or(|layer| current.is_on_layer(layer, &ctx)) {
                        let center = self
                            .drill_center(current_id)
                            .expect("a drill item has a centre");
                        current_distance = center.to_float().distance(&pick_location);
                        if current_distance < min_dist || nearest_is_trace {
                            candidate_found = true;
                        }
                    }
                }
                // RoutingBoardSearchFacade.java:197-202.
                Item::ConductionArea(area) => {
                    if layer.is_none_or(|layer| area.get_layer() == layer) && nearest_item.is_none()
                    {
                        candidate_found = true;
                        current_distance = f64::from(i32::MAX);
                    }
                }
                _ => {}
            }
            if candidate_found {
                // RoutingBoardSearchFacade.java:203-211.
                if let Some(from_item) = from_item {
                    let ignore_set =
                        ignore_set.get_or_insert_with(|| self.connected_set(from_item, -1, false));
                    if ignore_set.contains(&current_id) {
                        continue;
                    }
                }
                min_dist = current_distance;
                nearest_item = Some(current_id);
            }
        }
        nearest_item
    }

    // -- BasicBoard / RoutingBoard queries built on the above ---------------------------------------

    /// Port of `BasicBoard.getTraceTail` (BasicBoard.java:1306-1329): a trace of exactly these
    /// nets that ends at `location` with no contact there.
    pub fn get_trace_tail(
        &self,
        location: &Point,
        layer: Option<usize>,
        net_nos: &[i32],
    ) -> Option<ItemId> {
        let point_shape = TileShape::Box(TileShape::get_instance_from_point(location));
        for object in self.overlapping_objects(&point_shape, layer) {
            let TreeObject::Item(id) = object else {
                continue;
            };
            let Some(item @ Item::Trace(trace)) = self.items.get(&id) else {
                continue;
            };
            // BasicBoard.java:1311-1313.
            if !item.nets_equal_to(net_nos) {
                continue;
            }
            // BasicBoard.java:1314-1319.
            if trace.first_corner().as_ref() == Some(location)
                && self.trace_start_contacts(id).is_empty()
            {
                return Some(id);
            }
            // BasicBoard.java:1320-1325.
            if trace.last_corner().as_ref() == Some(location)
                && self.trace_end_contacts(id).is_empty()
            {
                return Some(id);
            }
        }
        None
    }

    /// Port of `BasicBoard.removeIfCycle` (BasicBoard.java:1335-1365): if this trace is part of a
    /// cycle, remove its whole connection, then remove the tails that removal exposed.
    pub fn remove_if_cycle(&mut self, id: ItemId) -> bool {
        let Some(item @ Item::Trace(trace)) = self.items.get(&id) else {
            return false;
        };
        // BasicBoard.java:1336-1341.
        if !item.is_on_the_board() || !self.is_trace_cycle(id) {
            return false;
        }
        let current_layer = trace.get_layer();
        let net_nos = item.net_nos().to_vec();
        let end_corners = [trace.first_corner(), trace.last_corner()];
        // BasicBoard.java:1349-1353.
        let tail_before: Vec<bool> = end_corners
            .iter()
            .map(|corner| {
                corner.as_ref().is_some_and(|corner| {
                    self.get_trace_tail(corner, Some(current_layer), &net_nos)
                        .is_some()
                })
            })
            .collect();
        // BasicBoard.java:1354-1355.
        let connection_items = self.connection_items(id, StopConnectionOption::None);
        self.remove_items(connection_items);
        // BasicBoard.java:1356-1363.
        for (index, corner) in end_corners.iter().enumerate() {
            if tail_before[index] {
                continue;
            }
            let Some(corner) = corner else { continue };
            if let Some(tail) = self.get_trace_tail(corner, Some(current_layer), &net_nos) {
                let tail_connection = self.connection_items(tail, StopConnectionOption::None);
                self.remove_items(tail_connection);
            }
        }
        true
    }

    /// Port of `RoutingBoard.containsTraceTails` (RoutingBoard.java:1176-1187).
    pub fn contains_trace_tails(
        &self,
        ids: impl IntoIterator<Item = ItemId>,
        except_net_nos: &[i32],
    ) -> bool {
        for id in ids {
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if !item.is_trace() {
                continue;
            }
            if !item.nets_equal_to(except_net_nos) && self.is_tail(id) {
                return true;
            }
        }
        false
    }

    /// Port of `RoutingBoard.removeTraceTails` (RoutingBoard.java:1193-1238): removes every
    /// trace/via stub of `net_number` (or of all nets when `net_number <= 0`).
    pub fn remove_trace_tails(
        &mut self,
        net_number: i32,
        stop_connection_option: StopConnectionOption,
    ) -> Result<bool, crate::BoardError> {
        let mut stub_set: BTreeSet<ItemId> = BTreeSet::new();
        // RoutingBoard.java:1195-1219, over the item list in board order.
        for id in self.items_in_board_order() {
            let Some(item) = self.items.get(&id) else {
                continue;
            };
            if !item.is_routable() || item.net_count() != 1 {
                continue;
            }
            if net_number > 0 && item.get_net_number(0) != net_number {
                continue;
            }
            if !self.is_tail(id) {
                continue;
            }
            if matches!(self.items.get(&id), Some(Item::Via(_))) {
                // RoutingBoard.java:1207-1216.
                if stop_connection_option == StopConnectionOption::Via {
                    continue;
                }
                if stop_connection_option == StopConnectionOption::FanoutVia
                    && self.is_fanout_via(id, None)
                {
                    continue;
                }
            }
            stub_set.insert(id);
        }
        // RoutingBoard.java:1220-1231.
        let mut stub_connections: BTreeSet<ItemId> = BTreeSet::new();
        for id in stub_set {
            if self.normal_contacts(id).len() == 1 {
                stub_connections.extend(self.connection_items(id, stop_connection_option));
            } else {
                stub_connections.insert(id);
            }
        }
        if stub_connections.is_empty() {
            return Ok(false);
        }
        self.remove_items(stub_connections);
        // RoutingBoard.java:1236.
        self.combine_traces(net_number)?;
        Ok(true)
    }

    /// Port of `RoutingBoard.connectToTrace` (RoutingBoard.java:1116-1170): insert a stub from
    /// `from_point` to the nearest point on `to_trace`.
    pub fn connect_to_trace(
        &mut self,
        from_point: &Point,
        to_trace: ItemId,
        pen_half_width: i32,
        clearance_class: usize,
    ) -> bool {
        let Some(item @ Item::Trace(trace)) = self.items.get(&to_trace) else {
            // RoutingBoard.java:1119-1121: not a `PolylineTrace`.
            return false;
        };
        let polyline = trace.polyline().clone();
        let trace_layer = trace.get_layer();
        let net_nos = item.net_nos().to_vec();
        let first_corner = trace.first_corner();
        let last_corner = trace.last_corner();
        // RoutingBoard.java:1123-1126.
        if polyline.contains(from_point) {
            return true;
        }
        // RoutingBoard.java:1127-1134.
        let Some(projection_line) = polyline.projection_line(from_point) else {
            return false;
        };
        let Ok(connection_line) = projection_line.to_polyline() else {
            return false;
        };
        if connection_line.lines().len() != 3 {
            return false;
        }
        // RoutingBoard.java:1136-1139.
        if !self.check_polyline_trace(
            &connection_line,
            trace_layer,
            pen_half_width,
            &net_nos,
            clearance_class,
        ) {
            return false;
        }
        // RoutingBoard.java:1140-1144.
        if self.changed_area.is_some() {
            for i in 0..connection_line.corner_count() {
                if let Some(corner) = connection_line.corner_approx(i) {
                    self.join_changed_area(&corner, trace_layer);
                }
            }
        }
        // RoutingBoard.java:1146-1152.
        self.insert_trace(
            connection_line,
            trace_layer,
            pen_half_width,
            net_nos.clone(),
            clearance_class,
            FixedState::Unfixed,
        );
        // RoutingBoard.java:1157-1168.
        for corner in [first_corner, last_corner].into_iter().flatten() {
            if *from_point == corner {
                continue;
            }
            if let Some(tail) = self.get_trace_tail(&corner, Some(trace_layer), &net_nos)
                && !self.items[&tail].is_user_fixed()
            {
                self.remove_item(tail);
            }
        }
        true
    }

    /// Port of `RoutingBoard.reduceNetsOfRouteItems` (RoutingBoard.java:1284-1356): drop net
    /// numbers from multi-net traces and vias that their contacts do not carry.
    //
    // Java bug: the method computes `result` but never assigns it (RoutingBoard.java:1285,1355),
    // so it always returns `false` even when it changed something — its doc comment promises
    // "true, if the nets of some items were reduced". Reproduced. See docs/java-quirks.md.
    pub fn reduce_nets_of_route_items(&mut self) -> bool {
        let result = false;
        let mut something_changed = true;
        while something_changed {
            something_changed = false;
            for id in self.items_in_board_order() {
                let Some(item) = self.items.get(&id) else {
                    continue;
                };
                // RoutingBoard.java:1296-1299.
                if item.net_nos().len() <= 1 || item.get_fixed_state() == FixedState::SystemFixed {
                    continue;
                }
                let net_nos = item.net_nos().to_vec();
                if matches!(item, Item::Via(_)) {
                    // RoutingBoard.java:1300-1314.
                    let contacts = self.normal_contacts(id);
                    let mut to_remove = None;
                    'outer: for current_net_number in &net_nos {
                        for contact_id in &contacts {
                            let Some(contact) = self.items.get(contact_id) else {
                                continue;
                            };
                            if !contact.contains_net(*current_net_number) {
                                to_remove = Some(*current_net_number);
                                break 'outer;
                            }
                        }
                    }
                    if let Some(net_number) = to_remove {
                        self.items
                            .get_mut(&id)
                            .expect("present")
                            .remove_from_net(net_number);
                        something_changed = true;
                        break;
                    }
                } else if item.is_trace() {
                    // RoutingBoard.java:1315-1349.
                    let mut removed = None;
                    let mut contacts = self.trace_start_contacts(id);
                    'ends: for end in 0..2 {
                        for current_net_number in &net_nos {
                            let mut pin_found = false;
                            for contact_id in &contacts {
                                let Some(contact @ Item::Pin(_)) = self.items.get(contact_id)
                                else {
                                    continue;
                                };
                                pin_found = true;
                                if !contact.contains_net(*current_net_number) {
                                    removed = Some(*current_net_number);
                                    break;
                                }
                            }
                            if !pin_found {
                                // RoutingBoard.java:1330-1339: at tie pins, traces may carry
                                // different nets, so only non-pin contacts are consulted.
                                for contact_id in &contacts {
                                    let Some(contact) = self.items.get(contact_id) else {
                                        continue;
                                    };
                                    if !matches!(contact, Item::Pin(_))
                                        && !contact.contains_net(*current_net_number)
                                    {
                                        removed = Some(*current_net_number);
                                        break;
                                    }
                                }
                            }
                            if removed.is_some() {
                                break 'ends;
                            }
                        }
                        if end == 0 {
                            contacts = self.trace_end_contacts(id);
                        }
                    }
                    if let Some(net_number) = removed {
                        self.items
                            .get_mut(&id)
                            .expect("present")
                            .remove_from_net(net_number);
                        something_changed = true;
                        break;
                    }
                }
            }
        }
        result
    }
}
