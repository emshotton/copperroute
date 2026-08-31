//! Port of `board.optimize.ViaOptimizer` (`board/optimize/ViaOptimizer.java`, 733 lines).

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId};
use fr_geometry::{FloatLine, FloatPoint, IntPoint, Point, Side, java_min};
use fr_settings::ExpansionCostFactor;

use crate::board_ext::drill_item_mover::DrillItemMover;
use crate::board_ext::tightener::PolylineTraceExt;

/// Port of `board.optimize.ViaOptimizer` (ViaOptimizer.java:24-733) — the via-repositioning half
/// of the pull-tight sweep.
///
/// **It is not the optimizer's**: `TraceTightener.optChangedArea:160-164` calls it, so it runs on
/// *every* `optChangedArea` with non-null trace costs — every routed connection
/// (`AutorouteConnectionRouter:107`), every tail removal (`BatchAutorouter:496`) and every fanout
/// pin (`RoutingBoard.fanout:1105`).
///
/// Java's class is `final` with a private constructor and nothing but static methods; the port is
/// a unit struct with associated functions and Java's `RoutingBoard board` parameter kept in
/// place, exactly as [`DrillItemMover`] does (plan-2 ruling 11: no board back-pointers).
///
/// # No `AutorouteEngine` parameter
///
/// The plan's Task 6 brief types `opt_via_location` with an `engine: Option<&mut AutorouteEngine>`
/// and `tightener/mod.rs`'s discharged `obligation:` marker asked for it to be threaded. **Java
/// wins, and nothing here can consume one.** The three callees this method and
/// [`Self::opt_plane_or_fanout_via`] reach —
/// [`DrillItemMover::insert`], [`DrillItemMover::check`] and
/// [`PolylineTraceExt::pull_tight`] — none of them takes an engine in the port, because Java's
/// `DrillItemMover` has no `Stoppable`/engine parameter at all and Java's
/// `PolylineTrace.pullTight(boolean, int, Stoppable)` (:869-890) builds its own `TraceTightener`
/// and reads `this.board.autorouteEngine` internally, which the port fixed as `None` in Plan 7
/// Task 5 (see [`PolylineTraceExt::pull_tight`]'s `:889` comment). Adding a parameter no callee
/// accepts would be a port invention, so the signature drops it.
pub struct ViaOptimizer;

impl ViaOptimizer {
    /// Port of `optViaLocation(RoutingBoard, Via, ExpansionCostFactor[], int, int)`
    /// (ViaOptimizer.java:33-158): "optimizes the location of a via connected to at most 2 traces
    /// according to the trace costs on the layers of the connected traces. If traceCosts == null,
    /// the horizontal and vertical trace costs will be set to 1. Returns false, if the via was not
    /// changed."
    ///
    /// The class's only public method, and the one `TraceTightener.optChangedArea:160-164` calls
    /// with `(this.board, via, traceCosts, this.minTranslateDist, 10)` — so Java's
    /// `tracePullTightAccuracy` parameter is fed the tightener's *minimum translate distance*, and
    /// its `maxRecursionDepth` the literal `10`. The parameter names here are Java's, not that
    /// call site's (the brief's `min_translate_dist` / `accuracy` pair had them the wrong way
    /// round).
    ///
    /// `Ok(false)` where Java's parameter type `Via` already excludes the case: an id naming
    /// nothing, or naming something that is not a via.
    pub fn opt_via_location(
        board: &mut Board,
        via: ItemId,
        trace_costs: Option<&[ExpansionCostFactor]>,
        trace_pull_tight_accuracy: i32,
        max_recursion_depth: i32,
    ) -> Result<bool, BoardError> {
        // :39-41. Java's parameter is typed `Via`, so the two type tests below cannot fail there.
        let Some(item) = board.get_item(via) else {
            return Ok(false);
        };
        if !matches!(item, Item::Via(_)) {
            return Ok(false);
        }
        if item.is_shove_fixed(&board.rules) {
            return Ok(false);
        }
        // :42-45. `FRLogger.debug("OptViaAlgo.opt_via_location: probably endless loop")` is
        // dropped with every other log payload (Plan 7's "no FRLogger" constraint).
        if max_recursion_depth <= 0 {
            return Ok(false);
        }
        // :46. `via.getNormalContacts()` is a `TreeSet<Item>`, i.e. **descending** item id
        // (`Item.compareTo`, Item.java:95-101 — `other.id - id`); Conventions §3.
        let contacts: Vec<ItemId> = board.normal_contacts(via).into_iter().rev().collect();
        // :47.
        let mut is_plane_or_fanout_via = contacts.len() == 1;
        let mut first_trace: Option<ItemId> = None;
        let mut second_trace: Option<ItemId> = None;
        // :50-75.
        if !is_plane_or_fanout_via {
            // :51-53.
            if contacts.len() != 2 {
                return Ok(false);
            }
            // :54-64 and :65-74 — the same five lines twice, over the first two contacts of the
            // descending walk.
            for (slot, current_item) in [&mut first_trace, &mut second_trace]
                .into_iter()
                .zip(contacts.iter().copied())
            {
                match Self::contact_role(board, current_item) {
                    ContactRole::FreeTrace => *slot = Some(current_item),
                    ContactRole::Plane => is_plane_or_fanout_via = true,
                    ContactRole::Unusable => return Ok(false),
                }
            }
        }
        // :76-78.
        if is_plane_or_fanout_via {
            return Self::opt_plane_or_fanout_via(
                board,
                via,
                trace_pull_tight_accuracy,
                max_recursion_depth,
            );
        }
        let (Some(first_trace), Some(second_trace)) = (first_trace, second_trace) else {
            // Unreachable: `isPlaneOrFanoutVia` is the only way out of :50-75 with a null trace,
            // and it was just handled. Java would NPE at `:80` instead.
            return Ok(false);
        };
        // :79.
        let Some(via_center) = board.drill_center(via) else {
            return Ok(false);
        };
        // :80-81.
        let first_layer = Self::trace_layer(board, first_trace);
        let second_layer = Self::trace_layer(board, second_trace);

        // :85-87. "Use tolerance-based comparison to match connectivity detection logic."
        // `(int)(double)` truncates toward zero in both languages, and saturates identically.
        let tolerance = Self::via_tolerance(board, via);

        // :89-96 and :98-106.
        let Some(first_trace_from_corner) =
            Self::from_corner(board, first_trace, &via_center, tolerance)
        else {
            // "Via is not connected at trace endpoints - skip optimization."
            return Ok(false);
        };
        let Some(second_trace_from_corner) =
            Self::from_corner(board, second_trace, &via_center, tolerance)
        else {
            return Ok(false);
        };

        // :108-116.
        let (first_layer_trace_costs, second_layer_trace_costs) = match trace_costs {
            Some(costs) => (costs[first_layer], costs[second_layer]),
            None => {
                let unit = ExpansionCostFactor {
                    horizontal: 1.0,
                    vertical: 1.0,
                };
                (unit, unit)
            }
        };

        // :118-131.
        let new_location = Self::reposition_via_general(
            board,
            via,
            Self::trace_half_width(board, first_trace),
            Self::trace_clearance_class(board, first_trace),
            first_layer,
            first_layer_trace_costs,
            &first_trace_from_corner,
            Self::trace_half_width(board, second_trace),
            Self::trace_clearance_class(board, second_trace),
            second_layer,
            second_layer_trace_costs,
            &second_trace_from_corner,
        );
        // :132-134.
        let Some(new_location) = new_location else {
            return Ok(false);
        };
        if new_location == via_center {
            return Ok(false);
        }
        // :135.
        let delta = new_location.difference_by(&via_center);
        // :136-139. Java's `DrillItemMover.insert(via, delta, 9, 9, null, board)` has no
        // `Stoppable`; the port's extra parameter is plan-6 ruling 6's and is `&|| false` here for
        // that reason. `FRLogger.warn("OptViaAlgo.opt_via_location: move via failed")` is dropped.
        if !DrillItemMover::insert(board, via, &delta, 9, 9, None, &|| false)? {
            return Ok(false);
        }
        // :140-149. `pickItems` answers a `TreeSet<Item>` and `ItemSelectionFilter.filter`
        // (ItemSelectionFilter.java:65-73) rebuilds one, so both walks are descending id.
        // `pullTight(true, tracePullTightAccuracy, null)` is the **three-argument** overload,
        // `PolylineTraceExt::pull_tight`.
        for layer in [first_layer, second_layer] {
            let picked: Vec<ItemId> = board
                .pick_traces(&new_location, Some(layer))
                .into_iter()
                .rev()
                .collect();
            for current_item in picked {
                <Board as PolylineTraceExt>::pull_tight(
                    board,
                    current_item,
                    true,
                    trace_pull_tight_accuracy,
                    &|| false,
                )?;
            }
        }
        // :150-156. Java's `break` makes this the **first** via of the descending walk and no
        // other, and the loop body is entered at all only when the pick is non-empty.
        let first_picked_via = board
            .pick_items(&new_location, Some(first_layer))
            .into_iter()
            .rev()
            .find(|id| matches!(board.get_item(*id), Some(Item::Via(_))));
        if let Some(current_item) = first_picked_via {
            Self::opt_via_location(
                board,
                current_item,
                trace_costs,
                trace_pull_tight_accuracy,
                max_recursion_depth - 1,
            )?;
        }
        // :157.
        Ok(true)
    }

    /// Port of `optPlaneOrFanoutVia(RoutingBoard, Via, int, int)` (ViaOptimizer.java:161-296):
    /// "optimisations for vias with only 1 connected Trace (Plane or Fanout Vias)."
    ///
    /// Private in Java; `pub` here so `tests/via_optimizer.rs` and the `p7t4` differential driver
    /// can drive it directly — an integration test and a separate binary are both *outside* the
    /// crate, so `pub(crate)` (which the brief asked for) does not reach them, and the Java twin
    /// pays for the same reach with `setAccessible`. Nothing inside `fr-router` calls it except
    /// [`Self::opt_via_location`].
    // pub seam: `optPlaneOrFanoutVia` is `private` in Java (ViaOptimizer.java:161); the only
    // callers of this `pub` are `crates/fr-router/tests/via_optimizer.rs` and
    // `scripts/differential/rust/src/bin/p7t4.rs`, both outside the crate.
    pub fn opt_plane_or_fanout_via(
        board: &mut Board,
        via: ItemId,
        trace_pull_tight_accuracy: i32,
        max_recursion_depth: i32,
    ) -> Result<bool, BoardError> {
        // :163-166. The `FRLogger.debug` is dropped.
        if max_recursion_depth <= 0 {
            return Ok(false);
        }
        // :167-170. Descending id, as at `:46`.
        let contact_list: Vec<ItemId> = board.normal_contacts(via).into_iter().rev().collect();
        if contact_list.is_empty() {
            return Ok(false);
        }
        // :171-187.
        let mut contact_plane: Option<ItemId> = None;
        let mut contact_trace: Option<ItemId> = None;
        for current_contact in contact_list {
            match board.get_item(current_contact) {
                // :174-179.
                Some(Item::ConductionArea(_)) => {
                    if contact_plane.is_some() {
                        return Ok(false);
                    }
                    contact_plane = Some(current_contact);
                }
                // :179-184.
                Some(item) if item.is_trace() => {
                    if item.is_shove_fixed(&board.rules) || contact_trace.is_some() {
                        return Ok(false);
                    }
                    contact_trace = Some(current_contact);
                }
                // :184-186.
                _ => return Ok(false),
            }
        }
        // :188-190.
        let Some(contact_trace) = contact_trace else {
            return Ok(false);
        };
        // :191.
        let Some(via_center) = board.drill_center(via) else {
            return Ok(false);
        };

        // :193-194. "Use tolerance based on via size, matching the logic in opt_via_location."
        let tolerance = Self::via_tolerance(board, via);

        // :196-204.
        let at_first_corner = {
            let first = Self::trace_corner(board, contact_trace, TraceEnd::First);
            let last = Self::trace_corner(board, contact_trace, TraceEnd::Last);
            if Self::is_within_tolerance(first.as_ref(), &via_center, tolerance) {
                true
            } else if Self::is_within_tolerance(last.as_ref(), &via_center, tolerance) {
                false
            } else {
                // "Via is not connected at trace endpoints - skip optimization."
                return Ok(false);
            }
        };
        // :205-211.
        let Some(Item::Trace(trace)) = board.get_item(contact_trace) else {
            return Ok(false);
        };
        let trace_polyline = trace.polyline().clone();
        let corner_count = trace_polyline.corner_count();
        let check_corner = if at_first_corner {
            trace_polyline.corner(1)
        } else {
            trace_polyline.corner(corner_count.wrapping_sub(2))
        };
        let Some(check_corner) = check_corner else {
            // Java would throw out of `Polyline.corner`; the port has no caller that can reach it
            // (a trace on a board always has at least two corners).
            return Ok(false);
        };
        // :212-215.
        let rounded_check_corner: IntPoint = check_corner.to_float().round();
        let trace_half_width = Self::trace_half_width(board, contact_trace);
        let trace_layer = Self::trace_layer(board, contact_trace);
        let trace_cl_class_no = Self::trace_clearance_class(board, contact_trace);
        // :216-217.
        let mut new_via_location = Self::reposition_via_toward_location(
            board,
            via,
            &rounded_check_corner,
            trace_half_width,
            trace_layer,
            trace_cl_class_no,
        );
        // :218-260. "try to project the via to the previous line".
        if new_via_location.is_none() && corner_count >= 3 {
            // :220-227.
            let prev_corner = if at_first_corner {
                trace_polyline.corner(2)
            } else {
                trace_polyline.corner(corner_count - 3)
            };
            if let Some(prev_corner) = prev_corner {
                // :228-230.
                let float_check_corner = check_corner.to_float();
                let float_via_center = via_center.to_float();
                let float_prev_corner = prev_corner.to_float();
                // :231.
                if float_check_corner.scalar_product(&float_via_center, &float_prev_corner) != 0.0 {
                    // :232-234.
                    let current_line = FloatLine::new(float_check_corner, float_prev_corner);
                    let projection = Point::Int(
                        current_line
                            .perpendicular_projection(&float_via_center)
                            .round(),
                    );
                    let diff_vector = projection.difference_by(&via_center);
                    // :235-242.
                    let mut projection_ok = true;
                    let angle_restriction = board.rules.trace_angle_restriction;
                    if projection == via_center
                        || angle_restriction == AngleRestriction::NinetyDegree
                            && !diff_vector.is_orthogonal()
                        || angle_restriction == AngleRestriction::FortyFiveDegree
                            && !diff_vector.is_multiple_of_45_degree()
                    {
                        projection_ok = false;
                    }
                    // :243-258.
                    if projection_ok
                        && DrillItemMover::check(board, via, &diff_vector, 0, 0, None, None)
                    {
                        let net_numbers = Self::net_nos(board, via);
                        let ok_length = board.check_trace_segment(
                            &via_center,
                            &projection,
                            trace_layer,
                            &net_numbers,
                            trace_half_width,
                            trace_cl_class_no,
                            false,
                        );
                        // :254-256.
                        if ok_length >= f64::from(i32::MAX) {
                            new_via_location = Some(projection);
                        }
                    }
                }
            }
        }
        // :261-263.
        let Some(new_via_location) = new_via_location else {
            return Ok(false);
        };
        // :264-280. "check, that the new location is inside the contact plane".
        if let Some(contact_plane) = contact_plane {
            let plane_layer = match board.get_item(contact_plane) {
                Some(Item::ConductionArea(area)) => area.get_layer(),
                _ => return Ok(false),
            };
            let picked: Vec<ItemId> = board
                .pick_items(&new_via_location, Some(plane_layer))
                .into_iter()
                .rev()
                .filter(|id| matches!(board.get_item(*id), Some(Item::ConductionArea(_))))
                .collect();
            // Java's `currentItem == contactPlane` is a reference identity test; on the port an
            // item is identified by its id, and the pick answers ids.
            if !picked.contains(&contact_plane) {
                return Ok(false);
            }
        }
        // :281.
        let diff_vector = new_via_location.difference_by(&via_center);
        // :282-285. The `FRLogger.warn` is dropped.
        if !DrillItemMover::insert(board, via, &diff_vector, 9, 9, None, &|| false)? {
            return Ok(false);
        }
        // :286-291.
        let picked: Vec<ItemId> = board
            .pick_traces(&new_via_location, Some(trace_layer))
            .into_iter()
            .rev()
            .collect();
        for current_item in picked {
            <Board as PolylineTraceExt>::pull_tight(
                board,
                current_item,
                true,
                trace_pull_tight_accuracy,
                &|| false,
            )?;
        }
        // :292-294. The recursive result is discarded, exactly as Java discards it.
        if new_via_location == check_corner {
            Self::opt_plane_or_fanout_via(
                board,
                via,
                trace_pull_tight_accuracy,
                max_recursion_depth - 1,
            )?;
        }
        // :295.
        Ok(true)
    }

    /// Port of `isWithinTolerance(Point, Point, int)` (ViaOptimizer.java:719-732): "checks if two
    /// points are within the specified tolerance distance. Uses Manhattan distance for efficiency,
    /// matching the logic in `DrillItem.getNormalContacts()`."
    ///
    /// The first argument is `Option` because Java's `p1 == null` guard (`:720-722`) is live for
    /// it: every call site feeds `PolylineTrace.firstCorner()` or `lastCorner()`, whose port
    /// counterparts answer `Option<Point>`. The second is Java's `viaCenter`, which no call site
    /// can make null.
    ///
    /// `Math.abs` on a `double` is `f64::abs`; the comparison is `<=`, so the boundary is
    /// **inside** the tolerance.
    ///
    /// Private in Java; `pub` here for the reason [`Self::opt_plane_or_fanout_via`] gives.
    // pub seam: `isWithinTolerance` is `private` in Java (ViaOptimizer.java:719); the only callers
    // of this `pub` are `crates/fr-router/tests/via_optimizer.rs` and
    // `scripts/differential/rust/src/bin/p7t4.rs`, both outside the crate.
    // Java bug: ViaOptimizer.isWithinTolerance — quirk #206. The javadoc says this "matches the
    // logic in DrillItem.getNormalContacts()", and `optViaLocation:85-86` repeats the claim. It
    // does not: `getNormalContacts` matches a trace end **exactly** (DrillItem.java:288-290), so
    // every contact this class sees already has the via centre on a corner and the tolerance can
    // only pick the *wrong* end — `firstCorner` is tested first, so a trace whose far corner is
    // within `minWidth/2 + 1` Manhattan of a via sitting on its near corner reads `corner(1)`
    // instead of `corner(cornerCount - 2)`. Reproduced, test order included.
    pub fn is_within_tolerance(p1: Option<&Point>, p2: &Point, tolerance: i32) -> bool {
        // :720-722.
        let Some(p1) = p1 else {
            return false;
        };
        // :724-725.
        let fp1 = p1.to_float();
        let fp2 = p2.to_float();
        // :729-731.
        let dx = (fp1.x - fp2.x).abs();
        let dy = (fp1.y - fp2.y).abs();
        (dx + dy) <= f64::from(tolerance)
    }

    // -- the three `repositionVia` overloads ------------------------------------------------------

    /// Port of `repositionVia(RoutingBoard, Via, IntPoint, int, int, int)` **overload A**
    /// (ViaOptimizer.java:302-365): "tries to move the via into the direction of `toLocation` as
    /// far as possible. Return the new location of the via, or `null`, if no move was possible."
    ///
    /// Java overloads on the arity and types of the argument list; Rust has no overloading, so the
    /// three are renamed. This one has **one** caller in Java, `optPlaneOrFanoutVia:216-217` — the
    /// one-contact / plane-or-fanout arm — plus the **eight** call expressions
    /// [`Self::reposition_via_general`] contains (`:467`, `:475`, `:495`, `:517`, `:558`, `:562`,
    /// `:568`, `:572`), of which the first two are the collinear arm's either/or.
    ///
    /// The method **mutates nothing**: `checkTraceSegment` and `DrillItemMover.check` are both
    /// read-only probes. `&mut Board` is the port's, because both of those take `&mut Board` here
    /// (the search trees are rebuilt lazily).
    ///
    /// Two `double` details are Java's and are reproduced literally: `okLength >= Integer.MAX_VALUE`
    /// is the "no obstacle at all" sentinel `checkTraceSegment` answers (`:331`), and the halving
    /// loop's `okLength = Math.min(okLength, distance)` is [`java_min`], not `f64::min` (they differ
    /// on `NaN` and on signed zero).
    ///
    /// Private in Java; `pub` here for the reason [`Self::opt_plane_or_fanout_via`] gives.
    // renamed: `ViaOptimizer.repositionVia(RoutingBoard, Via, IntPoint, int, int, int)` overload A
    // (ViaOptimizer.java:302-365) -> `ViaOptimizer::reposition_via_toward_location`. Java overloads
    // on the argument list and Rust does not, so the three share a Java name and need three Rust
    // ones; this one is named for what its javadoc says it does, "move the via into the direction
    // of toLocation as far as possible".
    // pub seam: `repositionVia` overload A is `private` in Java (ViaOptimizer.java:302); the only
    // callers of this `pub` are `crates/fr-router/tests/via_optimizer_reposition.rs` and
    // `scripts/differential/rust/src/bin/p7t4.rs`, both outside the crate.
    // Java bug: ViaOptimizer.repositionVia's angle-restriction asymmetry — quirk #207. No overload
    // tests `board.rules.getTraceAngleRestriction()` **against the delta it produces**, yet
    // `optPlaneOrFanoutVia:236-241` — the fallback reached only when *this* method answers `null` —
    // refuses a projection whose delta is not orthogonal (`NINETY_DEGREE`) or a multiple of 45
    // degrees (`FORTYFIVE_DEGREE`). Two overloads do *read* the restriction, and neither reading is
    // a test of the answer: overload B at `:388-390` (a `NONE`-only refusal of moves shorter than
    // 1.5) and overload C at `:528-529` (the acute-angle arm's `!= NINETY_DEGREE` gate, which
    // selects a family of candidates rather than checking any candidate's delta). Reproduced: no
    // test here either. Latent on the corpus, because a walk toward a trace corner inherits the
    // trace's own angle.
    pub fn reposition_via_toward_location(
        board: &mut Board,
        via: ItemId,
        to_location: &IntPoint,
        trace_half_width: i32,
        trace_layer: usize,
        trace_cl_class: usize,
    ) -> Option<Point> {
        // :310. Java's parameter type `Via` already excludes an id naming nothing.
        let from_location = board.drill_center(via)?;
        let to_point = Point::Int(*to_location);
        // :312-314. `Point::eq` is Java's class-sensitive `equals`, so a rational centre is never
        // equal to the `IntPoint` argument — exactly as in Java.
        if from_location == to_point {
            return None;
        }
        // :316-324.
        let net_numbers = Self::net_nos(board, via);
        let mut ok_length = board.check_trace_segment(
            &from_location,
            &to_point,
            trace_layer,
            &net_numbers,
            trace_half_width,
            trace_cl_class,
            false,
        );
        // :325-327.
        if ok_length <= 0.0 {
            return None;
        }
        // :328-335.
        let float_from_location = from_location.to_float();
        let float_to_location = to_point.to_float();
        let new_float_to_location = if ok_length >= f64::from(i32::MAX) {
            float_to_location
        } else {
            float_from_location.change_length(&float_to_location, ok_length)
        };
        // :336-338. Java's `DrillItemMover.check(via, delta, 0, 0, null, board, null)` — the two
        // recursion depths are zero, so no shove is attempted and nothing is inserted.
        let new_to_location = Point::Int(new_float_to_location.round());
        let delta = new_to_location.difference_by(&from_location);
        let check_ok = DrillItemMover::check(board, via, &delta, 0, 0, None, None);
        // :340-342.
        if check_ok {
            return Some(new_to_location);
        }
        // :344.
        let min_length = 0.3 * f64::from(trace_half_width) + 1.0;
        // :346.
        ok_length = java_min(ok_length, float_from_location.distance(&float_to_location));
        // :348-351.
        let mut current_length = ok_length / 2.0;
        ok_length = 0.0;
        let mut result: Option<Point> = None;
        // :353-363 — a binary search on the length, keeping the **last** accepted point, which is
        // also the farthest: `okLength` only grows, so a later `checkPoint` is never nearer than an
        // earlier one. Java's loop, halving included, is transcribed rather than restructured.
        while current_length >= min_length {
            let check_point = Point::Int(
                float_from_location
                    .change_length(&float_to_location, ok_length + current_length)
                    .round(),
            );
            let delta = check_point.difference_by(&from_location);
            if DrillItemMover::check(board, via, &delta, 0, 0, None, None) {
                ok_length += current_length;
                result = Some(check_point);
            }
            current_length /= 2.0;
        }
        // :364.
        result
    }

    /// Port of `repositionVia(RoutingBoard, Via, IntPoint, int, int, int, IntPoint, int, int, int)`
    /// **overload B** (ViaOptimizer.java:367-429) — the candidate check. It answers "may the via be
    /// moved to `toLocation`, with the *other* trace then running from `toLocation` to
    /// `connectLocation`?", and it is reached **only** from inside
    /// [`Self::reposition_via_general`]'s four axis-parallel decomposition arms (`:599`, `:627`,
    /// `:665`, `:696`). It mutates nothing.
    ///
    /// Unlike overload A this one demands a *clear* segment on both legs: `okLength <
    /// Integer.MAX_VALUE` refuses at `:411` and `:425`, i.e. anything short of "no obstacle at
    /// all" is a refusal, and no partial move is attempted.
    ///
    /// `:388-397` is Java's own comment, kept: under `AngleRestriction.NONE` a move shorter than
    /// 1.5 is refused because `TraceTightenerAnyAngle.reduceCorners` may not be able to remove the
    /// overlap it generates, which would loop forever.
    ///
    /// Private in Java; `pub` here for the reason [`Self::opt_plane_or_fanout_via`] gives.
    // renamed: `ViaOptimizer.repositionVia(RoutingBoard, Via, IntPoint, int, int, int, IntPoint,
    // int, int, int)` overload B (ViaOptimizer.java:367-429) ->
    // `ViaOptimizer::reposition_via_check_candidate`, for the reason overload A's marker gives.
    // Named for its role: it answers yes/no about one candidate location, and only overload C asks.
    // pub seam: `repositionVia` overload B is `private` in Java (ViaOptimizer.java:367); the only
    // callers of this `pub` are `crates/fr-router/tests/via_optimizer_reposition.rs` and
    // `scripts/differential/rust/src/bin/p7t4.rs`, both outside the crate.
    #[allow(clippy::too_many_arguments)]
    pub fn reposition_via_check_candidate(
        board: &mut Board,
        via: ItemId,
        to_location: &IntPoint,
        trace_half_width_1: i32,
        trace_layer_1: usize,
        trace_cl_class_1: usize,
        connect_location: &IntPoint,
        trace_half_width_2: i32,
        trace_layer_2: usize,
        trace_cl_class_2: usize,
    ) -> bool {
        // :379.
        let Some(from_location) = board.drill_center(via) else {
            return false;
        };
        let to_point = Point::Int(*to_location);
        // :381-384. The `FRLogger.trace` is dropped with every other log payload.
        if from_location == to_point {
            return false;
        }
        // :386.
        let delta = to_point.difference_by(&from_location);
        // :388-397.
        if board.rules.trace_angle_restriction == AngleRestriction::None
            && delta.length_approx() <= 1.5
        {
            return false;
        }
        // :399.
        let net_numbers = Self::net_nos(board, via);
        // :401-409.
        let ok_length = board.check_trace_segment(
            &from_location,
            &to_point,
            trace_layer_1,
            &net_numbers,
            trace_half_width_1,
            trace_cl_class_1,
            false,
        );
        // :411-413.
        if ok_length < f64::from(i32::MAX) {
            return false;
        }
        // :415-423.
        let ok_length = board.check_trace_segment(
            &to_point,
            &Point::Int(*connect_location),
            trace_layer_2,
            &net_numbers,
            trace_half_width_2,
            trace_cl_class_2,
            false,
        );
        // :425-427.
        if ok_length < f64::from(i32::MAX) {
            return false;
        }
        // :428.
        DrillItemMover::check(board, via, &delta, 0, 0, None, None)
    }

    /// Port of the twelve-argument `repositionVia` **overload C** (ViaOptimizer.java:434-713):
    /// "tries to reposition the via to a better location according to the trace costs. Returns
    /// `null`, if no better location was found." The two-trace arm's, called once from
    /// `optViaLocation:118-131`, and the only caller of [`Self::reposition_via_check_candidate`].
    ///
    /// The longest method in the class and a plain sequence of candidate attempts, each of which
    /// **returns the first success**; there is no scoring across candidates and therefore no
    /// `max_by` that could keep the wrong one of a tie. The four families, in Java's order:
    ///
    /// 1. `:462-480` — the **overlapping-lines** case (`sideOf == COLLINEAR` and a positive scalar
    ///    product): move toward whichever from-corner is *nearer*, and return that answer whatever
    ///    it is, including `None`. Note the crossed half-widths — the move toward the **first**
    ///    corner is checked with the **second** trace's width, layer and clearance class, because
    ///    it is the second trace that has to be re-routed to the new via location. Every later arm
    ///    crosses them the same way.
    /// 2. `:485-526` — the two **weighted-distance** attempts: if the same from-corner costs more
    ///    under its own layer's costs than under the other layer's, try moving there. Java compares
    ///    `floatFirstTraceFromCorner` against itself under two cost pairs at `:485-490` (and
    ///    `floatSecondTraceFromCorner` against itself at `:507-512`) — that is deliberate, not a
    ///    copy-paste slip: it asks "is this corner cheaper to reach on the other layer?".
    /// 3. `:528-578` — the **acute-angle** case, skipped under `NINETY_DEGREE`: shorten the longer
    ///    of the two legs to the length of the shorter, then try both endpoints, cheaper first.
    /// 4. `:581-711` — **decomposition into axis-parallel parts**, two attempts per non-orthogonal
    ///    delta, each of which asks overload B whether the L-shaped detour through `checkLocation`
    ///    is clear on both legs.
    ///
    /// It mutates nothing: every callee is a read-only probe.
    ///
    /// Private in Java; `pub` here for the reason [`Self::opt_plane_or_fanout_via`] gives.
    // renamed: `ViaOptimizer.repositionVia(RoutingBoard, Via, int, int, int, ExpansionCostFactor,
    // Point, int, int, int, ExpansionCostFactor, Point)` overload C (ViaOptimizer.java:434-713) ->
    // `ViaOptimizer::reposition_via_general`, for the reason overload A's marker gives. Named for
    // its javadoc's "reposition the via to a better location according to the trace costs" — the
    // general case, and the only one `optViaLocation` calls.
    // pub seam: `repositionVia` overload C is `private` in Java (ViaOptimizer.java:434); the only
    // callers of this `pub` are `crates/fr-router/tests/via_optimizer_reposition.rs` and
    // `scripts/differential/rust/src/bin/p7t4.rs`, both outside the crate.
    #[allow(clippy::too_many_arguments)]
    pub fn reposition_via_general(
        board: &mut Board,
        via: ItemId,
        first_trace_half_width: i32,
        first_trace_cl_class: usize,
        first_trace_layer: usize,
        first_trace_costs: ExpansionCostFactor,
        first_trace_from_corner: &Point,
        second_trace_half_width: i32,
        second_trace_cl_class: usize,
        second_trace_layer: usize,
        second_trace_costs: ExpansionCostFactor,
        second_trace_from_corner: &Point,
    ) -> Option<Point> {
        // :447.
        let via_location = board.drill_center(via)?;
        // :449-452.
        let first_delta = first_trace_from_corner.difference_by(&via_location);
        let second_delta = second_trace_from_corner.difference_by(&via_location);
        let scalar_product = first_delta.scalar_product(&second_delta);
        // :454-460.
        let float_via_location = via_location.to_float();
        let float_first_trace_from_corner = first_trace_from_corner.to_float();
        let float_second_trace_from_corner = second_trace_from_corner.to_float();
        let first_trace_from_corner_distance =
            float_via_location.distance(&float_first_trace_from_corner);
        let second_trace_from_corner_distance =
            float_via_location.distance(&float_second_trace_from_corner);
        let rounded_first_trace_from_corner = float_first_trace_from_corner.round();
        let rounded_second_trace_from_corner = float_second_trace_from_corner.round();

        // :462-480. "handle case of overlapping lines first".
        if via_location.side_of(first_trace_from_corner, second_trace_from_corner)
            == Side::Collinear
            && scalar_product > 0.0
        {
            if second_trace_from_corner_distance < first_trace_from_corner_distance {
                return Self::reposition_via_toward_location(
                    board,
                    via,
                    &rounded_second_trace_from_corner,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                );
            }
            return Self::reposition_via_toward_location(
                board,
                via,
                &rounded_first_trace_from_corner,
                second_trace_half_width,
                second_trace_layer,
                second_trace_cl_class,
            );
        }
        // :483. Java declares `Point result;` uninitialised; every read below is preceded by an
        // assignment in the same block, so `None` here changes nothing.
        let mut result: Option<Point>;

        // :485-490.
        let mut current_weighted_distance_1 = float_via_location.weighted_distance(
            &float_first_trace_from_corner,
            first_trace_costs.horizontal,
            first_trace_costs.vertical,
        );
        let mut current_weighted_distance_2 = float_via_location.weighted_distance(
            &float_first_trace_from_corner,
            second_trace_costs.horizontal,
            second_trace_costs.vertical,
        );

        // :492-504. "try to move the via in direction of firstTraceFromCorner".
        if current_weighted_distance_1 > current_weighted_distance_2 {
            result = Self::reposition_via_toward_location(
                board,
                via,
                &rounded_first_trace_from_corner,
                second_trace_half_width,
                second_trace_layer,
                second_trace_cl_class,
            );
            if result.is_some() {
                return result;
            }
        }

        // :506-512.
        current_weighted_distance_1 = float_via_location.weighted_distance(
            &float_second_trace_from_corner,
            second_trace_costs.horizontal,
            second_trace_costs.vertical,
        );
        current_weighted_distance_2 = float_via_location.weighted_distance(
            &float_second_trace_from_corner,
            first_trace_costs.horizontal,
            first_trace_costs.vertical,
        );

        // :514-526. "try to move the via in direction of secondTraceFromCorner".
        if current_weighted_distance_1 > current_weighted_distance_2 {
            result = Self::reposition_via_toward_location(
                board,
                via,
                &rounded_second_trace_from_corner,
                first_trace_half_width,
                first_trace_layer,
                first_trace_cl_class,
            );
            if result.is_some() {
                return result;
            }
        }

        // :528-578. "acute angle".
        if scalar_product > 0.0
            && board.rules.trace_angle_restriction != AngleRestriction::NinetyDegree
        {
            // :531-548.
            let to_point_1: IntPoint;
            let to_point_2: IntPoint;
            let float_to_point_1: FloatPoint;
            let float_to_point_2: FloatPoint;
            if first_trace_from_corner_distance < second_trace_from_corner_distance {
                to_point_1 = rounded_first_trace_from_corner;
                float_to_point_1 = float_first_trace_from_corner;
                float_to_point_2 = float_via_location.change_length(
                    &float_second_trace_from_corner,
                    first_trace_from_corner_distance,
                );
                to_point_2 = float_to_point_2.round();
            } else {
                float_to_point_1 = float_via_location.change_length(
                    &float_first_trace_from_corner,
                    second_trace_from_corner_distance,
                );
                to_point_1 = float_to_point_1.round();
                to_point_2 = rounded_second_trace_from_corner;
                float_to_point_2 = float_second_trace_from_corner;
            }
            // :549-554. Both weighted distances are measured between the **same** pair of points,
            // under the two layers' cost pairs.
            current_weighted_distance_1 = float_to_point_1.weighted_distance(
                &float_to_point_2,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );
            current_weighted_distance_2 = float_to_point_1.weighted_distance(
                &float_to_point_2,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );

            if current_weighted_distance_1 > current_weighted_distance_2 {
                // :556-564. "try moving the via first into the direction of toPoint1".
                result = Self::reposition_via_toward_location(
                    board,
                    via,
                    &to_point_1,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                );
                if result.is_none() {
                    result = Self::reposition_via_toward_location(
                        board,
                        via,
                        &to_point_2,
                        first_trace_half_width,
                        first_trace_layer,
                        first_trace_cl_class,
                    );
                }
            } else {
                // :566-574. "try moving the via first into the direction of toPoint2".
                result = Self::reposition_via_toward_location(
                    board,
                    via,
                    &to_point_2,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                );
                if result.is_none() {
                    result = Self::reposition_via_toward_location(
                        board,
                        via,
                        &to_point_1,
                        second_trace_half_width,
                        second_trace_layer,
                        second_trace_cl_class,
                    );
                }
            }
            // :576-578.
            if result.is_some() {
                return result;
            }
        }

        // :581. "try decomposition in axisparallel parts".

        // :583-643.
        if !first_delta.is_orthogonal() {
            // :584-585.
            let mut float_check_location =
                FloatPoint::new(float_via_location.x, float_first_trace_from_corner.y);

            // :587-595. `currentWeightedDistance1` is computed **once** here and read again by the
            // second attempt at :625 — Java does not recompute it, and it does not change.
            current_weighted_distance_1 = float_via_location.weighted_distance(
                &float_first_trace_from_corner,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );
            current_weighted_distance_2 = float_via_location.weighted_distance(
                &float_check_location,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );
            let mut current_weighted_distance_3 = float_check_location.weighted_distance(
                &float_first_trace_from_corner,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );

            // :597-613.
            if current_weighted_distance_1
                > current_weighted_distance_2 + current_weighted_distance_3
            {
                let check_location = float_check_location.round();
                let check_ok = Self::reposition_via_check_candidate(
                    board,
                    via,
                    &check_location,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                    &rounded_first_trace_from_corner,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                );
                if check_ok {
                    return Some(Point::Int(check_location));
                }
            }

            // :615-623.
            float_check_location =
                FloatPoint::new(float_first_trace_from_corner.x, float_via_location.y);

            current_weighted_distance_2 = float_via_location.weighted_distance(
                &float_check_location,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );
            current_weighted_distance_3 = float_check_location.weighted_distance(
                &float_first_trace_from_corner,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );

            // :625-641.
            if current_weighted_distance_1
                > current_weighted_distance_2 + current_weighted_distance_3
            {
                let check_location = float_check_location.round();
                let check_ok = Self::reposition_via_check_candidate(
                    board,
                    via,
                    &check_location,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                    &rounded_first_trace_from_corner,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                );
                if check_ok {
                    return Some(Point::Int(check_location));
                }
            }
        }

        // :645-711 — the same twice more, with the two traces' roles exchanged.
        if !second_delta.is_orthogonal() {
            // :646-647.
            let mut float_check_location =
                FloatPoint::new(float_via_location.x, float_second_trace_from_corner.y);

            // :649-661.
            current_weighted_distance_1 = float_via_location.weighted_distance(
                &float_second_trace_from_corner,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );
            current_weighted_distance_2 = float_via_location.weighted_distance(
                &float_check_location,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );
            let mut current_weighted_distance_3 = float_check_location.weighted_distance(
                &float_second_trace_from_corner,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );

            // :663-679.
            if current_weighted_distance_1
                > current_weighted_distance_2 + current_weighted_distance_3
            {
                let check_location = float_check_location.round();
                let check_ok = Self::reposition_via_check_candidate(
                    board,
                    via,
                    &check_location,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                    &rounded_second_trace_from_corner,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                );
                if check_ok {
                    return Some(Point::Int(check_location));
                }
            }

            // :682-691.
            float_check_location =
                FloatPoint::new(float_second_trace_from_corner.x, float_via_location.y);

            current_weighted_distance_2 = float_via_location.weighted_distance(
                &float_check_location,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );
            current_weighted_distance_3 = float_check_location.weighted_distance(
                &float_second_trace_from_corner,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );

            // :693-709.
            if current_weighted_distance_1
                > current_weighted_distance_2 + current_weighted_distance_3
            {
                let check_location = float_check_location.round();
                let check_ok = Self::reposition_via_check_candidate(
                    board,
                    via,
                    &check_location,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                    &rounded_second_trace_from_corner,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                );
                if check_ok {
                    return Some(Point::Int(check_location));
                }
            }
        }
        // :712.
        None
    }

    // -- helpers: the accessor chains Java writes inline -------------------------------------------

    /// `optViaLocation:56-64` / `:66-74`, as one classification of a single contact.
    fn contact_role(board: &Board, contact: ItemId) -> ContactRole {
        let Some(item) = board.get_item(contact) else {
            return ContactRole::Unusable;
        };
        // `currentItem.isShoveFixed() || !(currentItem instanceof PolylineTrace)`.
        if item.is_shove_fixed(&board.rules) || !item.is_trace() {
            if matches!(item, Item::ConductionArea(_)) {
                ContactRole::Plane
            } else {
                ContactRole::Unusable
            }
        } else {
            ContactRole::FreeTrace
        }
    }

    /// `:87` and `:194`, which are the same two lines: `(int) (via.minWidth() / 2) + 1`.
    fn via_tolerance(board: &Board, via: ItemId) -> i32 {
        let ctx = board.ctx();
        let min_width = match board.get_item(via) {
            Some(Item::Via(v)) => v.min_width(&ctx),
            Some(Item::Pin(p)) => p.min_width(&ctx),
            _ => return 1,
        };
        (min_width / 2.0) as i32 + 1
    }

    /// `:89-96` and `:98-106` — the same eight lines twice, answering the "from" corner or `None`
    /// for "the via is not at an endpoint of this trace".
    ///
    /// `corner_count().wrapping_sub(2)` on a trace with fewer than two corners wraps to a huge
    /// index, `Polyline::corner` answers `None`, and this method answers `None` — i.e. "not at an
    /// endpoint". Java throws out of `Polyline.corner` instead, and no caller can reach it: a
    /// trace on a board always has at least two corners, and `getNormalContacts` only ever hands
    /// back a trace whose first or last corner **is** the via centre.
    fn from_corner(
        board: &Board,
        trace: ItemId,
        via_center: &Point,
        tolerance: i32,
    ) -> Option<Point> {
        let first = Self::trace_corner(board, trace, TraceEnd::First);
        let last = Self::trace_corner(board, trace, TraceEnd::Last);
        let Some(Item::Trace(polyline_trace)) = board.get_item(trace) else {
            return None;
        };
        let polyline = polyline_trace.polyline();
        if Self::is_within_tolerance(first.as_ref(), via_center, tolerance) {
            polyline.corner(1)
        } else if Self::is_within_tolerance(last.as_ref(), via_center, tolerance) {
            polyline.corner(polyline.corner_count().wrapping_sub(2))
        } else {
            None
        }
    }

    fn trace_corner(board: &Board, trace: ItemId, end: TraceEnd) -> Option<Point> {
        match board.get_item(trace) {
            Some(Item::Trace(t)) => match end {
                TraceEnd::First => t.first_corner(),
                TraceEnd::Last => t.last_corner(),
            },
            _ => None,
        }
    }

    fn trace_layer(board: &Board, trace: ItemId) -> usize {
        match board.get_item(trace) {
            Some(Item::Trace(t)) => t.get_layer(),
            _ => 0,
        }
    }

    fn trace_half_width(board: &Board, trace: ItemId) -> i32 {
        match board.get_item(trace) {
            Some(Item::Trace(t)) => t.get_half_width(),
            _ => 0,
        }
    }

    fn trace_clearance_class(board: &Board, trace: ItemId) -> usize {
        match board.get_item(trace) {
            Some(item) => item.clearance_class(),
            None => 0,
        }
    }

    fn net_nos(board: &Board, item: ItemId) -> Vec<i32> {
        match board.get_item(item) {
            Some(item) => item.net_nos().to_vec(),
            None => Vec::new(),
        }
    }
}

/// The three outcomes of `optViaLocation:56-64`'s test on one contact.
enum ContactRole {
    /// A `PolylineTrace` that is not shove fixed — `firstTrace` / `secondTrace`.
    FreeTrace,
    /// A `ConductionArea`, which sets `isPlaneOrFanoutVia`.
    Plane,
    /// Anything else: `return false`.
    Unusable,
}

/// Which end of a trace `:89` / `:91` reads.
enum TraceEnd {
    First,
    Last,
}
