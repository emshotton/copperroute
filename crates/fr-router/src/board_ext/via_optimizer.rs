//! Port of `board.optimize.ViaOptimizer` (`board/optimize/ViaOptimizer.java`, 733 lines).

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId};
use fr_geometry::{FloatLine, IntPoint, Point};
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

    /// **Not a Java method.** A read-only replica of `optPlaneOrFanoutVia:167-215` that answers
    /// "would this via reach `reposition_via_toward_location`'s Task 7 guard?", so a test
    /// or a differential driver can skip exactly the vias whose answer Task 6 does not have —
    /// and skip them on the *Java* side too, so no committed transcript row records a port-only
    /// move (controller ruling B1).
    ///
    /// It touches nothing: every step is a lookup Java performs before `:216`, in Java's order and
    /// with Java's early exits (`:168-170` empty, `:171-187` the plane/trace classification,
    /// `:188-190` no contact trace, `:196-204` not at an endpoint, `:205-211` the check corner).
    /// `scripts/differential/java/P7T4.java`'s `reachesOverloadA` is the same twenty lines on the
    /// Java side, and the two agreeing on every via of every corpus stem is itself evidence for
    /// the classification.
    ///
    /// **Task 7 deletes this**, together with the guard it predicts.
    // pub seam: none in Java — the port's own Task 7 guard predicate. Its callers are
    // `crates/fr-router/tests/via_optimizer.rs` and `scripts/differential/rust/src/bin/p7t4.rs`,
    // both outside this crate, so `pub(crate)` cannot reach them.
    pub fn reaches_task_seven_guard(board: &Board, via: ItemId) -> bool {
        // :167-170.
        let contact_list: Vec<ItemId> = board.normal_contacts(via).into_iter().rev().collect();
        if contact_list.is_empty() {
            return false;
        }
        // :171-187.
        let mut contact_plane_seen = false;
        let mut contact_trace: Option<ItemId> = None;
        for current_contact in contact_list {
            match board.get_item(current_contact) {
                Some(Item::ConductionArea(_)) => {
                    if contact_plane_seen {
                        return false;
                    }
                    contact_plane_seen = true;
                }
                Some(item) if item.is_trace() => {
                    if item.is_shove_fixed(&board.rules) || contact_trace.is_some() {
                        return false;
                    }
                    contact_trace = Some(current_contact);
                }
                _ => return false,
            }
        }
        // :188-190.
        let Some(contact_trace) = contact_trace else {
            return false;
        };
        // :191, :194.
        let Some(via_center) = board.drill_center(via) else {
            return false;
        };
        let tolerance = Self::via_tolerance(board, via);
        // :196-204.
        let first = Self::trace_corner(board, contact_trace, TraceEnd::First);
        let last = Self::trace_corner(board, contact_trace, TraceEnd::Last);
        let at_first_corner = if Self::is_within_tolerance(first.as_ref(), &via_center, tolerance) {
            true
        } else if Self::is_within_tolerance(last.as_ref(), &via_center, tolerance) {
            false
        } else {
            return false;
        };
        // :205-211 — the port answers `false` where `Polyline::corner` would, for the reason
        // `from_corner` gives.
        let Some(Item::Trace(trace)) = board.get_item(contact_trace) else {
            return false;
        };
        let polyline = trace.polyline();
        let corner_count = polyline.corner_count();
        let check_corner = if at_first_corner {
            polyline.corner(1)
        } else {
            polyline.corner(corner_count.wrapping_sub(2))
        };
        // :216-217 is the next statement, so reaching here is reaching the guard.
        check_corner.is_some()
    }

    // -- Task 7's three overloads ------------------------------------------------------------------

    /// # Panics
    ///
    /// **Always.** This is Plan 7 Task 7's `ViaOptimizer.repositionVia` overload A, and Task 6
    /// deliberately does not answer for it — see the marker below and the module's
    /// "Why overload A panics and overload C does not".
    // added in Task 7: `ViaOptimizer.repositionVia` overload A (ViaOptimizer.java:302-365) — "tries
    // to move the via into the direction of toLocation as far as possible. Return the new location
    // of the via, or null, if no move was possible." One caller: `optPlaneOrFanoutVia:216-217`, the
    // **one**-contact / plane-or-fanout arm. Task 7 replaces this body; nothing else about the call
    // site changes. The `unimplemented!` is Plan 6 Task 9's precedent for a half-closed cycle
    // (`board_ext/mod.rs:19`), and controller ruling B1 requires it here rather than a `None`.
    #[allow(clippy::too_many_arguments)]
    fn reposition_via_toward_location(
        _board: &mut Board,
        _via: ItemId,
        _to_location: &IntPoint,
        _trace_half_width: i32,
        _trace_layer: usize,
        _trace_cl_class: usize,
    ) -> Option<Point> {
        unimplemented!(
            "ViaOptimizer.repositionVia overload A (ViaOptimizer.java:302-365) is Plan 7 Task 7's; \
             answering `None` here would send optPlaneOrFanoutVia into its :218-260 projection \
             fallback, which Java reaches only when its own overload A answered null, and that \
             fallback inserts"
        )
    }

    // added in Task 7: `ViaOptimizer.repositionVia` overload C (ViaOptimizer.java:435-713) — the
    // twelve-argument one `optViaLocation:118-131` calls, the **two-trace** arm's, and the only
    // caller of overload B (`:367-429`). Unlike overload A this one may answer `None`: Java's
    // `:132-134` turns `null` into `return false` with **nothing mutated**, so the stub reproduces a
    // board Java can really produce — it is a *missing* move, never a wrong one. Measured on
    // `Issue026-J2_reference` at `routeK = 12`: **four of six vias** reach it and Java moves all
    // four, which is the whole of `p7t4`'s residual diff.
    #[allow(clippy::too_many_arguments)]
    fn reposition_via_general(
        _board: &mut Board,
        _via: ItemId,
        _first_trace_half_width: i32,
        _first_trace_cl_class: usize,
        _first_trace_layer: usize,
        _first_layer_trace_costs: ExpansionCostFactor,
        _first_trace_from_corner: &Point,
        _second_trace_half_width: i32,
        _second_trace_cl_class: usize,
        _second_trace_layer: usize,
        _second_layer_trace_costs: ExpansionCostFactor,
        _second_trace_from_corner: &Point,
    ) -> Option<Point> {
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
