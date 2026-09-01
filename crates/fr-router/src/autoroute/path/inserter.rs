//! Port of `app.freerouting.autoroute.path.FoundConnectionInserter`
//! (FoundConnectionInserter.java:23-807) — the only class in Plan 6 that mutates the board's item
//! set.

use fr_board::datastructures::StopCheck;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId};
use fr_geometry::{IntPoint, Point, Polyline};

use crate::autoroute::maze::AutorouteControl;
use crate::autoroute::maze::engine::AutorouteEngine;
use crate::autoroute::path::locator::{
    FoundConnectionLocator, ResultItem, calculate_additional_corner,
};
use crate::board_ext::{ForcedViaInserter, RoutingBoardExt};

/// Port of `FoundConnectionInserter` (FoundConnectionInserter.java:23-807): "inserts the traces
/// and vias of the connection found by the autoroute algorithm."
///
/// # The return value is Java's, not the brief's `InsertedItems`
///
/// Java's `getInstance` (`:40-111`) answers **the instance itself**, or `null`, and the instance
/// is `final` with two `private` fields (`lastCorner`, `firstCorner`, `:27-28`) and no accessor
/// of any kind. Its one caller — `AutorouteEngine.autorouteConnection:265-277` — tests
/// `== null` and nothing else. There is no list of inserted traces or vias anywhere in Java: the
/// board *is* the result, and plan-6 ruling 1(b)'s acceptance ("the same inserted item geometry")
/// is read off the board's item set, which is what `tests/inserter.rs` compares against the JVM.
///
/// So the brief's `InsertedItems { traces, vias }` is **not** ported: collecting it would mean
/// inventing bookkeeping Java does not have, and the ids it would carry are already observable as
/// `board.communication.id_gen.max_generated_id()` plus `board.get_items()`. The port answers
/// `Result<Option<FoundConnectionInserter>, BoardError>`, an exact image of Java's
/// `FoundConnectionInserter | null` plus the `Err` channel below.
///
/// # `Err` propagates — it is never `None`
///
/// `None` is Java's `null`, which `autorouteConnection:271-277` turns into a **message-carrying**
/// `FAILED`. An `Err` is a Java *throw*, and no `catch` covers this class:
/// `AutorouteEngine.autorouteConnection` wraps only `FoundConnectionLocator.getInstance`
/// (`:181-196`), and the call at `:265-266` is outside every `try` in the method (`grep` answers
/// `:139`, `:157`, `:190`, `:518` — none encloses `:265`). The nearest handler is
/// `AutorouteConnectionRouter.route:155-158`, which produces a **bare** `FAILED` with no message.
/// The two `FAILED`s are distinguishable and the port keeps them distinct, so an `Err` out of the
/// Task 10b / 15b chain — [`BoardError::Stopped`] or quirk #109's `combine_traces` failure —
/// must reach the caller as an `Err` and never be flattened into `None`.
///
/// # `connectionItems` is never null
///
/// `:42`'s `connection.connectionItems == null` is dead code (quirk #180): the field is `final`
/// and assigned an empty `LinkedList` at `FoundConnectionLocator:101`, before both of the
/// constructor's early returns. [`FoundConnectionLocator::connection_items`] is a `Vec`, so the
/// test is unrepresentable; an **empty** list is the reachable case, and it inserts nothing.
///
/// not ported: `formatPoint` (`:113-121`), `shouldTraceFanoutDiagnostics` (`:787-791`) and
/// `traceFanoutDiagnostic` (`:793-806`) — `FRLogger` payload builders with no effect on the
/// board. Their call sites are marked where they fall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundConnectionInserter {
    /// `private IntPoint lastCorner` (`:27`), written by [`Self::insert_trace`] at `:134` and
    /// `:451` and read at `:74` and `:95`.
    last_corner: Option<IntPoint>,
    /// `private IntPoint firstCorner` (`:28`), written at `:132` and `:449` and read at `:80`.
    first_corner: Option<IntPoint>,
}

/// The live `Trace` reference `:77`/`:92` hold, reduced to the three fields
/// `RoutingBoard.connectToTrace` reads off it. See [`FoundConnectionInserter::trace_snapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceSnapshot {
    polyline: Polyline,
    layer: usize,
    net_nos: Vec<i32>,
}

impl FoundConnectionInserter {
    /// Port of the private constructor `FoundConnectionInserter(RoutingBoard, AutorouteControl)`
    /// (`:31-34`). Java stores the two arguments in fields; the port passes them down as
    /// parameters instead, because a `&mut Board` cannot be held across the call graph the
    /// methods below need.
    fn new() -> FoundConnectionInserter {
        FoundConnectionInserter {
            last_corner: None,
            first_corner: None,
        }
    }

    /// Port of `getInstance(FoundConnectionLocator, RoutingBoard, AutorouteControl)` (`:40-111`):
    /// "creates a new instance of FoundConnectionInserter. Returns null if the insertion did not
    /// succeed."
    ///
    /// `engine` is Java's `RoutingBoard.autorouteEngine` field, which
    /// [`RoutingBoardExt::insert_forced_trace_polyline`] needs so the `PolylineTrace.change`
    /// inside its pull-tight tail can run `additionalUpdateAfterChange`. Java reads it off the
    /// board and null-tests it (`RoutingBoard.java:100`); the port passes it, and `None` is that
    /// null. `RoutingBoard.autorouteEngine` is assigned only by `initAutoroute` (`:892`), so a
    /// caller that built its `AutorouteEngine` directly — every probe and test in this plan —
    /// leaves it null and must pass `None`.
    ///
    /// `stop` is plan-6 ruling 6's / plan-3 ruling F's: the chain below reaches
    /// `Board::split_traces_checked` and `Board::normalize_traces_checked`, the two `fr-board`
    /// walks that do not terminate on quirk #76's ladder board. Java has no cancellation here.
    pub fn get_instance(
        connection: Option<&FoundConnectionLocator>,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        stop: StopCheck<'_>,
    ) -> Result<Option<FoundConnectionInserter>, BoardError> {
        // :42-44. `connection == null` is the `Option`; `connection.connectionItems == null` is
        // quirk #180's dead test, unrepresentable here.
        let Some(connection) = connection else {
            return Ok(None);
        };
        // `:77` and `:92` read `connection.targetItem` / `connection.startItem`, which Java holds
        // as live **object references** taken during the locator's walk. The insert below can
        // split either of them in two (`BasicBoard.splitTraces` through `insertVia`) or remove
        // it outright, and Java's reference survives that: `connectToTrace` then works off the
        // original, undivided polyline and removes the tails at *its* two end corners. An id
        // lookup after the insert answers `None` and skips the whole block. So the two traces are
        // snapshotted here, where Java's references are already live and the board still holds
        // them — see `docs/java-quirks.md` #186 and `Board::connect_to_trace_of`.
        //
        // obligation: `AutorouteEngine.autorouteConnection:260-263` — Java's reference is older
        // still, taken during the locator's walk, so a start or target trace that the *ripup*
        // removes at `:260` is a live object there and `None` here. Strictly smaller than the
        // deviation #186 fixes; Task 16 owns that ripup and should pass the snapshot in rather
        // than let this method take it. **Re-marked in Task 17**: measured over the whole
        // acceptance corpus (369 connections, `tests/reference/router-fixtures.txt`), the ripup
        // never removes either endpoint — `connection.start_item` and `connection.target_item`
        // resolve on the board at this point in **all 311** evaluations, the 97 connections that
        // did rip something included. The sibling snapshot that *is* reached is
        // `describe_connection`'s (`engine.rs`), which `router-j2-reference` k = 19 forced Task 17
        // to fix.
        let target_trace = Self::trace_snapshot(board, connection.target_item);
        let start_trace = Self::trace_snapshot(board, connection.start_item);
        // :45.
        let mut current_layer = connection.target_layer;
        // :46.
        let mut new_instance = FoundConnectionInserter::new();
        // :47-73. Every via comes from the layer change between two consecutive `ResultItem`s —
        // `connectionItems` holds traces only (Task 14 §2.1).
        for current_new_item in &connection.connection_items {
            // :48-65 is `FRLogger.trace` only. `:49-53`'s `corners.length > 0` guard is
            // defensive: `calculateNextTrace:411` seeds the corner list with `currentFromPoint`
            // before anything else, so a `ResultItem` always has at least one corner and `:66`'s
            // unguarded `corners[0]` cannot throw.
            // :66-68.
            let via_location = Point::Int(current_new_item.corners[0]);
            if !new_instance.insert_via(
                board,
                ctrl,
                Some(&via_location),
                current_layer,
                current_new_item.layer,
                stop,
            )? {
                return Ok(None);
            }
            // :69.
            current_layer = current_new_item.layer;
            // :70-72.
            if !new_instance.insert_trace(
                board,
                ctrl,
                engine.as_deref_mut(),
                current_new_item,
                stop,
            )? {
                return Ok(None);
            }
        }
        // :74-76. The last via closes the connection back onto the start item's layer.
        let last_corner = new_instance.last_corner.map(Point::Int);
        if !new_instance.insert_via(
            board,
            ctrl,
            last_corner.as_ref(),
            current_layer,
            connection.start_layer,
            stop,
        )? {
            return Ok(None);
        }
        // :77-91.
        // `:78`'s `else` (`:84-90`) is the `FRLogger.warn` for a null `firstCorner`, which
        // happens only when `connectionItems` is empty — so the two tests collapse into one.
        //
        // Java bug: FoundConnectionInserter.getInstance:82 sizes the stub onto the **target**
        // item from `ctrl.traceHalfWidth[connection.startLayer]`, while
        // `RoutingBoard.connectToTrace:1135` inserts it on `toTrace.getLayer()` — the target
        // trace's layer. The two indices are crossed; see docs/java-quirks.md #187.
        if let Some(target_trace) = &target_trace
            && let Some(first_corner) = new_instance.first_corner
        {
            board.connect_to_trace_of(
                &Point::Int(first_corner),
                &target_trace.polyline,
                target_trace.layer,
                &target_trace.net_nos,
                ctrl.trace_half_width[connection.start_layer],
                ctrl.trace_clearance_class_index,
            );
        }
        // :92-106.
        // `:93`'s `else` (`:99-105`) is the matching `FRLogger.warn`.
        //
        // Java bug: FoundConnectionInserter.getInstance:97 is the mirror of `:82` — the stub onto
        // the **start** item is sized from `ctrl.traceHalfWidth[connection.targetLayer]` and
        // inserted on the start trace's own layer. docs/java-quirks.md #187.
        if let Some(start_trace) = &start_trace
            && let Some(last_corner) = new_instance.last_corner
        {
            board.connect_to_trace_of(
                &Point::Int(last_corner),
                &start_trace.polyline,
                start_trace.layer,
                &start_trace.net_nos,
                ctrl.trace_half_width[connection.target_layer],
                ctrl.trace_clearance_class_index,
            );
        }

        // :108.
        board.normalize_traces_checked(ctrl.net_number, stop)?;

        // :110.
        Ok(Some(new_instance))
    }

    /// What `:77`'s and `:92`'s `instanceof PolylineTrace` reference carries into
    /// `RoutingBoard.connectToTrace` — the three fields it reads (`polyline()`, `getLayer()`,
    /// `netNumbers`), taken while the item is still in the board.
    fn trace_snapshot(board: &Board, item: Option<ItemId>) -> Option<TraceSnapshot> {
        let id = item?;
        let item @ Item::Trace(trace) = board.get_item(id)? else {
            // `:77`/`:92`'s `instanceof PolylineTrace`, which a `Pin` start or target fails.
            return None;
        };
        Some(TraceSnapshot {
            polyline: trace.polyline().clone(),
            layer: trace.get_layer(),
            net_nos: item.net_nos().to_vec(),
        })
    }

    /// Port of `insertTrace(ResultItem)` (`:127-453`): "inserts the trace by shoving aside
    /// obstacle traces and vias. Returns false, that was not possible for the whole trace."
    ///
    /// # `pinEdgeToTurnDist` is not restored on a throw
    ///
    /// `:140-141` saves and clears `board.rules.pinEdgeToTurnDist` and `:447` restores it, with
    /// no `try`/`finally` between them. Every `insertForcedTracePolyline` in the loop can throw
    /// (quirk #185, quirk #22's polyline constructor, quirk #109's `combine`), and Java then
    /// leaves the rule at `-1` for the rest of the session. The port propagates its `Err` the
    /// same way rather than inventing a guard — the caller Java reaches is
    /// `AutorouteConnectionRouter.route:155-158`, which abandons the board, not the setting.
    #[allow(clippy::too_many_lines)] // Java's method, kept whole.
    fn insert_trace(
        &mut self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        trace: &ResultItem,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        // :128-136. "Single-point trace: the start and end are the same location (already at the
        // target). Set both firstCorner and lastCorner so that connect_to_trace is not called
        // with null."
        if trace.corners.len() == 1 {
            if self.first_corner.is_none() {
                self.first_corner = Some(trace.corners[0]);
            }
            self.last_corner = Some(trace.corners[0]);
            return Ok(true);
        }

        // :138-141. "switch off correcting connection to pin because it may get wrong in
        // inserting the polygon line for line."
        let saved_edge_to_turn_dist = board.rules.get_pin_edge_to_turn_dist();
        board.rules.set_pin_edge_to_turn_dist(-1.0);

        // :143-165. "Look for pins att the start and the end of trace in case that neckdown is
        // necessary."
        let mut start_pin: Option<ItemId> = None;
        let mut end_pin: Option<ItemId> = None;
        if ctrl.with_neckdown {
            // :147-148's `ItemSelectionFilter(PINS)` is the `Item::Pin` match below — see
            // `Board::pick_items`' `not ported:` note on the filter class.
            let mut current_end_corner = Point::Int(trace.corners[0]);
            for i in 0..2 {
                let picked = board.pick_items(&current_end_corner, Some(trace.layer));
                // `BasicBoard.pickItems` and `ItemSelectionFilter.filter` both answer a
                // `TreeSet<Item>`, and `Item.compareTo` (Item.java:95-103) is
                // `other.id - this.id` — **descending**. The assignment at `:157`/`:159` is
                // unconditional, so Java's winner is the *lowest* id it visits last; the port's
                // `BTreeSet` is ascending, so it walks it in reverse to land on the same pin.
                //
                // obligation: `FoundConnectionInserter.insertTrace:151-162` — **re-marked in
                // Task 17**. The `.rev()` is Java's descending `TreeSet<Item>` order
                // (`Item.compareTo`, quirk #44), read off the source rather than guessed, and it
                // can only be observed when **two** own-net pins share a trace end. This *is*
                // live code on a real board — `AutorouteControl.java:168` is
                // `withNeckdown = settings.getAutomaticNeckdown()`, `DefaultSettings.java:103`
                // sets it true, and Task 17's driver builds its settings from `DefaultSettings`
                // exactly so that this path runs — and the corpus does reach the loop (820
                // evaluations, 564 of which find exactly one pin). But **no evaluation anywhere
                // in the corpus finds two**: instrumented, the pin count at a trace end is 0 or 1
                // every time, so `.rev()` and the forward walk still agree.
                for id in picked.into_iter().rev() {
                    let Some(item @ Item::Pin(_)) = board.get_item(id) else {
                        continue;
                    };
                    // :154-155.
                    if item.contains_net(ctrl.net_number)
                        && board.drill_center(id).as_ref() == Some(&current_end_corner)
                    {
                        // :156-160.
                        if i == 0 {
                            start_pin = Some(id);
                        } else {
                            end_pin = Some(id);
                        }
                    }
                }
                // :163.
                current_end_corner = Point::Int(trace.corners[trace.corners.len() - 1]);
            }
        }
        // :166-167.
        let net_numbers = [ctrl.net_number];

        // :169-170.
        let mut from_corner_no = 0usize;
        let mut result = true;
        // :171-405.
        for i in 1..trace.corners.len() {
            // :172-173.
            let current_corner_arr: Vec<Point> = trace.corners[from_corner_no..=i]
                .iter()
                .map(|corner| Point::Int(*corner))
                .collect();
            let insert_polyline = Polyline::from_points(&current_corner_arr);
            // :174 and :189-200 read `maxGeneratedId` for `FRLogger.trace` only.
            // :175-188. `tidyWidth` is `Integer.MAX_VALUE`, `withCheck` true, `timeLimit` null.
            let ok_point = board.insert_forced_trace_polyline(
                engine.as_deref_mut(),
                &insert_polyline,
                ctrl.trace_half_width[trace.layer],
                trace.layer,
                &net_numbers,
                ctrl.trace_clearance_class_index,
                ctrl.max_shove_trace_recursion_depth,
                ctrl.max_shove_via_recursion_depth,
                ctrl.max_spring_over_recursion_depth,
                i32::MAX,
                ctrl.pull_tight_accuracy,
                true,
                None,
                stop,
            )?;
            // `okPoint != insertPolyline.lastCorner()` is Java's reference test; the port's is by
            // value, which agrees on every answer this method can produce — see
            // `RoutingBoardExt::insert_forced_trace_segment`'s "Java's `==` on the returned
            // corner" note, measured over 452 probe rows in Task 15b.
            let last_corner = insert_polyline.last_corner();
            let first_corner = insert_polyline.first_corner();
            // :201-202.
            let mut neckdown_inserted = false;
            let mut micro_neckdown_inserted = false;
            // :203-209.
            if let Some(ok) = ok_point.as_ref()
                && ok_point != last_corner
                && ctrl.with_neckdown
                && current_corner_arr.len() == 2
            {
                neckdown_inserted = self.insert_neckdown(
                    board,
                    ctrl,
                    engine.as_deref_mut(),
                    ok,
                    &current_corner_arr[1],
                    trace.layer,
                    start_pin,
                    end_pin,
                    stop,
                )?;
            }
            // :210-217.
            if !neckdown_inserted
                && ok_point != last_corner
                && ctrl.is_fanout
                && current_corner_arr.len() == 2
            {
                micro_neckdown_inserted = self.insert_fanout_micro_neckdown(
                    board,
                    ctrl,
                    engine.as_deref_mut(),
                    ok_point.as_ref(),
                    &current_corner_arr[1],
                    trace.layer,
                    &net_numbers,
                    start_pin,
                    end_pin,
                    stop,
                )?;
            }
            if ok_point == last_corner || neckdown_inserted || micro_neckdown_inserted {
                // :218-263 — the ADVANCE arm, whose body below `:219` is `FRLogger.trace` only.
                from_corner_no = i;
            // obligation: `FoundConnectionInserter.insertTrace:264` — **discharged in Task 17**.
            // Task 15 could reach the VIOLATION_CORRECTED arm (dropping `:275-276` fails mode
            // `around`) but never on the **last** corner, so the `i != trace.corners.length - 1`
            // guard was unobservable there. Task 17's corpus reaches it with `i` at the last
            // corner on `router-rpi-splitter` (2 evaluations), `router-j2-reference` (6) and
            // `router-dac2020-bm01` (7), and every one of those connections matches the HEAD jar
            // byte for byte.
            } else if ok_point == first_corner && i != trace.corners.len() - 1 {
                // :264-319. "if okPoint == insertPolyline.firstCorner() the spring over may have
                // failed. Spring over may correct the situation because an insertion, which is ok
                // with clearance compensation may cause violations without clearance
                // compensation. In this case repeating the insertion with more distant corners
                // may allow the spring over to correct the situation."
                if from_corner_no > 0 {
                    // :272-273. "trace.corners[i] may be inside the offset for the substitute
                    // trace around a spring_over obstacle (if clearance compensation is off)."
                    if current_corner_arr.len() < 3 {
                        // :275-276, "first correction".
                        from_corner_no -= 1;
                    }
                }
                // :279-319 is `FRLogger.trace` only.
            } else {
                // :320-404 — the FAIL arm; everything above `:402` is diagnostics.
                result = false;
                break;
            }
        }

        // :407-431. Every corner before the last one may have left a stub behind.
        for i in 0..trace.corners.len() - 1 {
            let corner = Point::Int(trace.corners[i]);
            if let Some(trace_stub) = board.get_trace_tail(&corner, Some(trace.layer), &net_numbers)
            {
                // :411-427 is `FRLogger.trace`; `:428` is the removal.
                board.remove_item(trace_stub);
            }
        }
        // :433-445 is `FRLogger.trace`.

        // :447.
        board
            .rules
            .set_pin_edge_to_turn_dist(saved_edge_to_turn_dist);
        // :448-451.
        //
        // obligation: `FoundConnectionInserter.insertTrace:448-450` — **discharged in Task 17**.
        // `firstCorner` is first-write-wins, and Task 15's only fixture that read it (`diag`)
        // answered the same board for either corner. Task 17's corpus performs the **second and
        // later** write — the one this guard suppresses — on `router-rpi-splitter` (6
        // evaluations), `router-j2-reference` (8) and `router-dac2020-bm01` (85), and every one
        // of those connections matches the HEAD jar byte for byte.
        if self.first_corner.is_none() {
            self.first_corner = Some(trace.corners[0]);
        }
        self.last_corner = Some(trace.corners[trace.corners.len() - 1]);
        // :452.
        Ok(result)
    }

    /// Port of `insertFanoutMicroNeckdown(Point, Point, int, int[], Pin, Pin)` (`:455-523`): the
    /// fanout-only retry that re-inserts the stalled segment at a succession of narrower half
    /// widths.
    ///
    /// # renamed: the `LinkedHashSet` of `:462`
    ///
    /// Java's `LinkedHashSet<Integer>` is **insertion-ordered and de-duplicated**, and the loop
    /// at `:473` takes the *first* candidate that reaches the target — so the order is
    /// load-bearing. The port is a `Vec<i32>` with a `contains` membership test, not a
    /// `BTreeSet`, which would sort [69, 75, 60, 50] into [50, 60, 69, 75] and pick a different
    /// winner. `P6T15Probe`'s mode `micro` measures exactly that: with a null `startPin` and a
    /// base half width of 100 the JVM inserts a **69**-wide trace, where any sorted set answers
    /// 50.
    ///
    /// `okPoint` is an `Option` because `:457` reads it as one; `targetPoint` is a `&Point`
    /// because `:458`'s `targetPoint == null` is unreachable — the only caller (`:216`) passes
    /// `currentCornerArr[1]`, an element of a `Polyline`'s corner array.
    #[allow(clippy::too_many_arguments)] // Java's parameter list plus the board, engine and stop.
    fn insert_fanout_micro_neckdown(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        ok_point: Option<&Point>,
        target_point: &Point,
        layer: usize,
        net_numbers: &[i32],
        start_pin: Option<ItemId>,
        end_pin: Option<ItemId>,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        // :457.
        let from_point = ok_point.unwrap_or(target_point);
        // :458-460.
        if from_point == target_point {
            return Ok(false);
        }
        // :461.
        let base_half_width = ctrl.trace_half_width[layer];
        // :462-471. `add` on a `LinkedHashSet` keeps the *first* insertion's position.
        let mut candidate_half_widths: Vec<i32> = Vec::new();
        let add = |value: i32, list: &mut Vec<i32>| {
            if !list.contains(&value) {
                list.push(value);
            }
        };
        let ctx = board.ctx();
        // :463-465.
        if let Some(pin_id) = start_pin
            && let Some(Item::Pin(pin)) = board.get_item(pin_id)
            && pin.is_on_layer(layer, &ctx)
        {
            add(
                pin.get_trace_neckdown_halfwidth(layer, &ctx),
                &mut candidate_half_widths,
            );
        }
        // :466-468.
        if let Some(pin_id) = end_pin
            && let Some(Item::Pin(pin)) = board.get_item(pin_id)
            && pin.is_on_layer(layer, &ctx)
        {
            add(
                pin.get_trace_neckdown_halfwidth(layer, &ctx),
                &mut candidate_half_widths,
            );
        }
        // :469-471. Java's `int` division truncates towards zero, and so does Rust's;
        // `Math.max(int, int)` is `i32::max`. The two `* 3`s are `int` multiplications that
        // **wrap** in Java (above a half width of 715 827 882) where Rust would panic in a debug
        // build, so they are `wrapping_mul`.
        add(
            1.max(base_half_width.wrapping_mul(3) / 4),
            &mut candidate_half_widths,
        );
        add(
            1.max(base_half_width.wrapping_mul(3) / 5),
            &mut candidate_half_widths,
        );
        add(1.max(base_half_width / 2), &mut candidate_half_widths);

        // :473-509.
        for candidate_half_width in candidate_half_widths {
            // :474-476.
            if candidate_half_width <= 0 || candidate_half_width >= base_half_width {
                continue;
            }
            // :477-491.
            let candidate_ok_point = board.insert_forced_trace_segment(
                engine.as_deref_mut(),
                from_point,
                target_point,
                candidate_half_width,
                layer,
                net_numbers,
                ctrl.trace_clearance_class_index,
                ctrl.max_shove_trace_recursion_depth,
                ctrl.max_shove_via_recursion_depth,
                ctrl.max_spring_over_recursion_depth,
                i32::MAX,
                ctrl.pull_tight_accuracy,
                true,
                None,
                stop,
            )?;
            // :492-508. Java's `==` is a reference test that value equality reproduces here.
            if candidate_ok_point.as_ref() == Some(target_point) {
                return Ok(true);
            }
        }
        // :510-522 is `traceFanoutDiagnostic` only.
        Ok(false)
    }

    /// Port of `insertNeckdown(Point, Point, int, Pin, Pin)` (`:525-537`): try the start pin's
    /// neck first, then the end pin's.
    ///
    /// Both tests are Java references (`:528`, `:534`). They reproduce as value comparisons
    /// because [`Self::try_neck_down`] hands back either its own `fromCorner`/`toCorner`
    /// arguments or `insertForcedTraceSegment`'s answer, and both are value-faithful — with one
    /// premise: `fromCorner != toCorner` **by value**, which the only production call site
    /// (`:208`) guarantees, since `:203` has already established `okPoint != lastCorner`.
    #[allow(clippy::too_many_arguments)] // Java's parameter list plus the board, engine and stop.
    fn insert_neckdown(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        layer: usize,
        start_pin: Option<ItemId>,
        end_pin: Option<ItemId>,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        // :526-531. Note the swap: the start pin's neck runs *back* from `toCorner`.
        if let Some(pin) = start_pin {
            let ok_point = self.try_neck_down(
                board,
                ctrl,
                engine.as_deref_mut(),
                to_corner,
                from_corner,
                layer,
                pin,
                true,
                stop,
            )?;
            if ok_point.as_ref() == Some(from_corner) {
                return Ok(true);
            }
        }
        // :532-535.
        if let Some(pin) = end_pin {
            let ok_point = self.try_neck_down(
                board,
                ctrl,
                engine,
                from_corner,
                to_corner,
                layer,
                pin,
                false,
                stop,
            )?;
            return Ok(ok_point.as_ref() == Some(to_corner));
        }
        // :536.
        Ok(false)
    }

    /// Port of `tryNeckDown(Point, Point, int, Pin, boolean)` (`:539-676`): insert the segment at
    /// full width for as long as the board allows, then finish into the pin at the pin's own
    /// neckdown half width.
    ///
    /// `at_start` is Java's fifth parameter and Java never reads it — the body has no occurrence
    /// of it. It is kept so the two call sites at `:527` and `:533` transcribe literally.
    #[allow(clippy::too_many_arguments)] // Java's parameter list plus the board, engine and stop.
    #[allow(clippy::too_many_lines)] // Java's method, kept whole.
    fn try_neck_down(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        mut engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        layer: usize,
        pin: ItemId,
        at_start: bool,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError> {
        let _ = at_start;
        // Everything Java reads off the `Pin` object, read before the board is borrowed mutably.
        let ctx = board.ctx();
        let Some(Item::Pin(pin_item)) = board.get_item(pin) else {
            // Java's parameter is a `Pin`, so this arm cannot be reached from `insertNeckdown`,
            // whose two pins came out of `pickItems`' `Item::Pin` match.
            return Ok(None);
        };
        // :540-542.
        if !pin_item.is_on_layer(layer, &ctx) {
            return Ok(None);
        }
        // :543.
        let pin_center = pin_item.get_center(&ctx).to_float();
        let pin_clearance_class = board
            .get_item(pin)
            .expect("the pin was just read")
            .clearance_class();
        // :547.
        let pin_max_width = pin_item.get_max_width(layer, &ctx);
        // :552.
        let neck_down_halfwidth = pin_item.get_trace_neckdown_halfwidth(layer, &ctx);

        // :544-546.
        let current_clearance = f64::from(board.rules.clearance_matrix.get_value(
            ctrl.trace_clearance_class_index,
            pin_clearance_class,
            layer,
            true,
        ));
        // :547.
        let pin_neck_down_distance = 2.0 * (0.5 * pin_max_width + current_clearance);
        // :548-550.
        if pin_center.distance(&to_corner.to_float()) >= pin_neck_down_distance {
            return Ok(None);
        }
        // :553-555.
        //
        // obligation: `FoundConnectionInserter.tryNeckDown:553` — **re-marked in Task 17**. The
        // `>=`/`>` question needs a pin whose neckdown half width *equals* `ctrl.traceHalfWidth`.
        // Task 17's driver does put the method in production shape — its settings come from
        // `DefaultSettings`, so `automaticNeckdown` is true and `tryNeckDown` runs — and the
        // corpus reaches this gate 116 times (`router-rpi-splitter` 1, `router-j2-reference` 37,
        // `router-dac2020-bm01` 78). **Every one of them is a strict inequality**, and every one
        // returns here, so nothing below this line is reached on any corpus connection either
        // (see the `:586-588` marker).
        if neck_down_halfwidth >= ctrl.trace_half_width[layer] {
            return Ok(None);
        }

        // :557-558.
        let float_from_corner = from_corner.to_float();
        let float_to_corner = to_corner.to_float();
        // :560.
        let tolerance = 2.0;
        // :562-563.
        let net_numbers = [ctrl.net_number];

        // :565-573.
        let mut ok_length = board.check_trace_segment(
            from_corner,
            to_corner,
            layer,
            &net_numbers,
            ctrl.trace_half_width[layer],
            ctrl.trace_clearance_class_index,
            true,
        );
        // :574-576.
        if ok_length >= f64::from(i32::MAX) {
            return Ok(Some(from_corner.clone()));
        }
        // :577.
        ok_length -= tolerance;
        // :578-660.
        let mut neck_down_end_point: Point;
        if ok_length <= tolerance {
            // :579-580.
            neck_down_end_point = from_corner.clone();
        } else {
            // :582-583.
            let float_neck_down_end_point =
                float_from_corner.change_length(&float_to_corner, ok_length);
            neck_down_end_point = Point::Int(float_neck_down_end_point.round());
            // :584-588. "add a corner in case neckDownEndPoint is not exactly on the line from
            // fromCorner to toCorner"
            //
            // obligation: `FoundConnectionInserter.tryNeckDown:586-588` — **re-marked in
            // Task 17**. The one Task 15 row that reaches this arm has `|dx| > |dy|` strictly, so
            // `>=` and `>` agree; a neck along an exact diagonal would separate them. Task 17's
            // corpus does not help: instrumented, **no** corpus connection gets past `:553`'s
            // gate at all (0 of 116 evaluations), so this line is never evaluated on a real
            // board. The board that would discriminate it needs a pin narrower than the trace
            // *and* a clear diagonal run from it.
            let horizontal_first = (float_from_corner.x - float_neck_down_end_point.x).abs()
                >= (float_from_corner.y - float_neck_down_end_point.y).abs();
            // :589-595.
            let mut add_corner = Point::Int(
                calculate_additional_corner(
                    float_from_corner,
                    float_neck_down_end_point,
                    horizontal_first,
                    board.rules.trace_angle_restriction,
                )
                .round(),
            );
            // :596-613.
            let mut current_ok_point = self.forced_segment(
                board,
                ctrl,
                engine.as_deref_mut(),
                from_corner,
                &add_corner,
                ctrl.trace_half_width[layer],
                layer,
                &net_numbers,
                stop,
            )?;
            if current_ok_point.as_ref() != Some(&add_corner) {
                return Ok(Some(from_corner.clone()));
            }
            // :614-631.
            current_ok_point = self.forced_segment(
                board,
                ctrl,
                engine.as_deref_mut(),
                &add_corner,
                &neck_down_end_point,
                ctrl.trace_half_width[layer],
                layer,
                &net_numbers,
                stop,
            )?;
            if current_ok_point.as_ref() != Some(&neck_down_end_point) {
                return Ok(Some(from_corner.clone()));
            }
            // :632-638.
            add_corner = Point::Int(
                calculate_additional_corner(
                    float_neck_down_end_point,
                    float_to_corner,
                    !horizontal_first,
                    board.rules.trace_angle_restriction,
                )
                .round(),
            );
            // :639-659.
            if &add_corner != to_corner {
                current_ok_point = self.forced_segment(
                    board,
                    ctrl,
                    engine.as_deref_mut(),
                    &neck_down_end_point,
                    &add_corner,
                    ctrl.trace_half_width[layer],
                    layer,
                    &net_numbers,
                    stop,
                )?;
                if current_ok_point.as_ref() != Some(&add_corner) {
                    return Ok(Some(from_corner.clone()));
                }
                // :658.
                neck_down_end_point = add_corner;
            }
        }

        // :662-675.
        self.forced_segment(
            board,
            ctrl,
            engine,
            &neck_down_end_point,
            to_corner,
            neck_down_halfwidth,
            layer,
            &net_numbers,
            stop,
        )
    }

    /// The four `insertForcedTraceSegment` calls of `tryNeckDown` (`:596`, `:614`, `:641`,
    /// `:662`) differ only in their two corners and the half width; every other argument is
    /// `ctrl`'s, `Integer.MAX_VALUE`, `true` and `null`.
    ///
    /// Not a Java method — it is the repeated argument list, named once.
    #[allow(clippy::too_many_arguments)]
    fn forced_segment(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError> {
        board.insert_forced_trace_segment(
            engine,
            from_corner,
            to_corner,
            half_width,
            layer,
            net_numbers,
            ctrl.trace_clearance_class_index,
            ctrl.max_shove_trace_recursion_depth,
            ctrl.max_shove_via_recursion_depth,
            ctrl.max_spring_over_recursion_depth,
            i32::MAX,
            ctrl.pull_tight_accuracy,
            true,
            None,
            stop,
        )
    }

    /// Port of `insertVia(Point, int, int)` (`:683-785`): "searches the cheapest via masks
    /// containing fromLayer and toLayer, so that a forced via is possible at location with this
    /// mask and inserts the via. Returns false, if no suitable via mask was found or if the
    /// algorithm failed."
    ///
    /// A refused [`ForcedViaInserter::check`] is **not** an error: it is a `false` that makes the
    /// loop try the next candidate, and — when no candidate survives — a `false` return that
    /// `getInstance:67` turns into `None`. Only [`ForcedViaInserter::insert`]'s `Err` (quirk
    /// #76's `Stopped`, quirk #109's `combine_traces`) propagates.
    ///
    /// # Panics
    ///
    /// * when `location` is `None` **and** a padstack spanning `fromLayer..toLayer` was found.
    ///   Java's only dereference of `location` in this method's reach is
    ///   `ForcedViaInserter.check`'s `location.differenceBy(Point.ZERO)`
    ///   (ForcedViaInserter.java:140), called from `:708` — which `:704-706` guards. So a null
    ///   `location` with **no** spanning padstack is not an NPE in Java: it falls through to
    ///   `:721-751` and returns `false`, i.e. `Ok(None)` here and `autorouteConnection:271-277`'s
    ///   *message-carrying* `FAILED`, not `AutorouteConnectionRouter.route:155-158`'s bare one.
    ///   The `expect` therefore sits inside the loop, at Java's deref, and not above it.
    ///
    ///   A null `location` here is `getInstance:74`'s null `lastCorner`, i.e. an empty
    ///   `connectionItems`, and it really can arrive with `currentLayer != startLayer`:
    ///   `FoundConnectionLocator.java:130-135` (quirk #180's second early return) returns with
    ///   `targetLayer` at its `0` default **after** `:114` has set
    ///   `startLayer = startDoor.room.getLayer()`, which is nonzero whenever the start door's
    ///   room is not on layer 0. `a_null_last_corner_with_no_spanning_padstack_answers_none` and
    ///   `a_null_last_corner_panics_where_java_dereferences_it` pin both halves.
    /// * when `ctrl.viaRule` is `None` — `:701` dereferences it with no guard, exactly as
    ///   `AutorouteControl.rebuildViaInfo:235` already has.
    fn insert_via(
        &self,
        board: &mut Board,
        ctrl: &AutorouteControl,
        location: Option<&Point>,
        input_from_layer: usize,
        input_to_layer: usize,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        // :684-686.
        if input_from_layer == input_to_layer {
            return Ok(true); // no via necessary
        }
        // :687-696. "sort the input layers"
        //
        // obligation: `FoundConnectionInserter.insertVia:687-696` — **discharged in Task 17**.
        // On Task 15's two-layer boards with a full-span padstack the sort could not change
        // `:704`'s answer. Task 17's corpus calls `insertVia` with `fromLayer > toLayer` — the
        // input the swap exists for — on `router-rpi-splitter` (2 of 6 calls),
        // `router-j2-reference` (4 of 8) and `router-dac2020-bm01` (34 of 86), and every one of
        // those connections matches the HEAD jar byte for byte, vias included.
        let (from_layer, to_layer) = if input_from_layer < input_to_layer {
            (input_from_layer, input_to_layer)
        } else {
            (input_to_layer, input_from_layer)
        };
        // :697-698.
        let net_numbers = [ctrl.net_number];
        // :699-700.
        let mut via_info = None;
        let mut found_suitable_span = false;
        // :701.
        let via_rule = ctrl
            .via_rule
            .as_ref()
            .expect("FoundConnectionInserter.insertVia:701 dereferences ctrl.viaRule (NPE)");
        let via_count = via_rule.via_count();
        for i in 0..via_count {
            // :702-703.
            let current_via_info = via_rule.get_via(i).clone();
            let current_via_padstack = current_via_info.get_padstack();
            // :704-706.
            let padstack_from = board
                .library
                .padstacks
                .padstack_from_layer(current_via_padstack);
            let padstack_to = board
                .library
                .padstacks
                .padstack_to_layer(current_via_padstack);
            if padstack_from > java_int(from_layer) || padstack_to < java_int(to_layer) {
                continue;
            }
            // :707.
            found_suitable_span = true;
            // :708-719. `check` is where Java first touches `location`
            // (`ForcedViaInserter.java:140`, `location.differenceBy(Point.ZERO)`), so the
            // `Option` is opened here and not above the loop — see this method's `# Panics`.
            let location = location
                .expect("FoundConnectionInserter.insertVia:708 -> ForcedViaInserter.java:140 dereferences a null location (NPE)");
            if ForcedViaInserter::check(
                board,
                &current_via_info,
                location,
                &net_numbers,
                ctrl.max_shove_trace_recursion_depth,
                ctrl.max_shove_via_recursion_depth,
                Some(&ctrl.trace_half_width),
                ctrl.trace_clearance_class_index,
            ) {
                via_info = Some(current_via_info);
                break;
            }
        }
        // :721-752. The whole body is `FRLogger.debug` and `traceFanoutDiagnostic`; only the
        // `return false` at `:751` is behaviour. `found_suitable_span` picks between the two
        // diagnostic messages and nothing else.
        let Some(via_info) = via_info else {
            let _ = found_suitable_span;
            return Ok(false);
        };
        // :753-783. "insert the via". Reached only when `:708`'s `check` answered true, which
        // already opened the `Option` above.
        let location = location
            .expect("FoundConnectionInserter.insertVia:754 is reached only through :708's check");
        if !ForcedViaInserter::insert(
            board,
            &via_info,
            location,
            &net_numbers,
            ctrl.trace_clearance_class_index,
            &ctrl.trace_half_width,
            ctrl.max_shove_trace_recursion_depth,
            ctrl.max_shove_via_recursion_depth,
            stop,
        )? {
            return Ok(false);
        }
        // :784.
        Ok(true)
    }
}

/// `:704-705` compares a padstack's `int` layer bounds against `fromLayer`/`toLayer`, which are
/// `int` in Java and `usize` here.
fn java_int(layer: usize) -> i32 {
    i32::try_from(layer).expect("a board layer index fits in an int, as it does in Java")
}

// =================================================================================================
// The neckdown tests
// =================================================================================================
//
// `insertNeckdown` (`:525-537`) is package-private in Java and `tryNeckDown` (`:539-676`) and
// `insertFanoutMicroNeckdown` (`:455-523`) are private, so `tests/inserter.rs` — a different
// crate — cannot reach them. They are pinned here instead, against the same JVM transcript:
// `P6T15Probe`'s modes `neck` and `micro`, whose rows are read off the HEAD jar exactly as
// `tests/inserter.rs`' five modes are. The probe calls all three by reflection on an instance
// built from the private constructor, which is what `FoundConnectionInserter::new` is here.
//
// Java compares `tryNeckDown`'s answer with `==` at `:528`, `:534` and `:611`/`:629`/`:655`; the
// port compares by value. The transcript prints Java's reference answer as `isFrom=`/`isTo=` on
// every row of mode `neck`, and every one of them agrees with a value comparison — the only
// non-argument answer in the file is `(-316,60)`, which equals neither corner.
#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use fr_board::ids::{PadstackId, ViaInfoId};
    use fr_board::prelude::*;
    use fr_board::rules::{ViaInfo, ViaRule};
    use fr_geometry::{IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape};
    use fr_settings::RouterSettings;

    use super::FoundConnectionInserter;
    use crate::autoroute::maze::AutorouteControl;

    const T15: &str = include_str!("../../../tests/data/p6t15-inserter.txt");

    /// The rows of one `=== mode <mode> ===` section of the Task 15 transcript.
    fn section(mode: &str) -> Vec<&'static str> {
        let header = format!("=== mode {mode} ===");
        let mut rows = Vec::new();
        let mut inside = false;
        for line in T15.lines() {
            if line.starts_with("=== mode ") {
                inside = line == header;
                continue;
            }
            if inside {
                rows.push(line.trim_end());
            }
        }
        assert!(!rows.is_empty(), "transcript section `{mode}` is empty");
        rows
    }

    fn assert_rows_match(mode: &str, actual: &[String]) {
        let expected = section(mode);
        let mut diffs = Vec::new();
        for i in 0..expected.len().max(actual.len()) {
            let want = expected.get(i).copied().unwrap_or("<missing>");
            let got = actual.get(i).map(String::as_str).unwrap_or("<missing>");
            if want != got {
                diffs.push(format!("row {i}\n  jvm:  {want}\n  rust: {got}"));
            }
        }
        assert!(
            diffs.is_empty(),
            "mode `{mode}`: {} of {} rows differ\n{}",
            diffs.len(),
            expected.len().max(actual.len()),
            diffs
                .iter()
                .take(12)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// `P6T15Probe.buildNeck` — `P6T13Probe.buildSimple` with a wider default trace half width
    /// and a **user-fixed** foreign-net blocker at x = 0.
    ///
    /// Both matter. `tryNeckDown:553` gives up unless the pin's neckdown half width — 49 for the
    /// 100-unit `smd` pad, 69 for the 140-unit `thru` octagon — is strictly below
    /// `ctrl.traceHalfWidth`, and `buildSimple`'s 30 is below both; and with nothing in the way
    /// `checkTraceSegment` (`:566`) answers `Integer.MAX_VALUE`, so `:574-576` returns before
    /// anything is inserted.
    fn neck_board(trace_half_width: i32) -> Board {
        let layers =
            || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
        let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
        let mut rules = BoardRules::new(layers(), clearance_matrix);
        rules.trace_angle_restriction = AngleRestriction::None;
        rules.set_default_trace_half_widths(30);

        let mut padstacks = Padstacks::new(layers());
        padstacks.add(
            "smd",
            vec![
                Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                    -50, -50, 50, 50,
                )))),
                None,
            ],
            false,
            false,
        );
        let through_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
            -70, -70, 70, 70, -140, 140, -140, 140,
        )));
        padstacks.add(
            "thru",
            vec![Some(through_shape.clone()), Some(through_shape)],
            true,
            false,
        );
        let via_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
            -100, -100, 100, 100, -200, 200, -200, 200,
        )));
        let via = padstacks.add(
            "via",
            vec![Some(via_shape.clone()), Some(via_shape)],
            true,
            false,
        );

        rules.via_infos.add(ViaInfo::new("v", via, 1, false));
        let mut via_rule = ViaRule::new("rule");
        via_rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
        rules.via_rules.push(via_rule);
        let default_class = rules.get_default_net_class();
        rules
            .net_classes
            .get_mut(default_class)
            .set_via_rule(Some(rules.via_rules[0].clone()));

        let mut board = Board::new(
            Vec::new(),
            0,
            IntBox::from_coords(-1000, -1000, 1000, 1000),
            rules,
            BoardLibrary::new(padstacks, Packages::new()),
            Components::new(),
            Communication::default(),
        );
        let default_class = board.rules.get_default_net_class();
        board.rules.nets.add("N1", 1, false, default_class);
        let pkg = board.library.packages.add(
            "pkg",
            vec![
                PackagePin::new("P1", PadstackId(1), IntVector::new(-400, 0).into(), 0.0),
                PackagePin::new("P2", PadstackId(2), IntVector::new(400, 0).into(), 0.0),
            ],
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            true,
        );
        board
            .components
            .add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);
        board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed); // id 2, the smd pin
        board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed); // id 3, the thru pin

        // `buildNeck`'s own two lines, after `buildSimple`.
        board.rules.set_default_trace_half_widths(trace_half_width);
        let default_class = board.rules.get_default_net_class();
        board.rules.nets.add("N2", 1, false, default_class);
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(0, -900), Point::new(0, 900)]),
            0,
            30,
            vec![2],
            1,
            FixedState::UserFixed,
        ); // id 4
        board
    }

    /// `P6T15Probe.control(board, 1, true)`.
    fn neck_control(board: &Board) -> AutorouteControl {
        let mut settings = RouterSettings::new();
        settings.set_layer_count(board.get_layer_count());
        settings.apply_board_specific_optimizations(board);
        settings.set_automatic_neckdown(true);
        let trace_costs = settings.get_trace_costs();
        AutorouteControl::new(board, 1, &settings, settings.get_via_costs(), &trace_costs)
    }

    struct Counter {
        calls: Cell<u32>,
    }

    impl Counter {
        fn new() -> Counter {
            Counter {
                calls: Cell::new(0),
            }
        }

        fn check(&self) -> bool {
            self.calls.set(self.calls.get() + 1);
            false
        }
    }

    /// `P6T15Probe.ptOf`.
    fn point_of(point: Option<&Point>) -> String {
        match point {
            None => "null".to_string(),
            Some(Point::Int(p)) => format!("({},{})", p.x, p.y),
            Some(other) => {
                let f = other.to_float();
                format!("~({},{})", f.x, f.y)
            }
        }
    }

    /// `P6T15Probe.ln`.
    fn line_of(line: &fr_geometry::Line) -> String {
        format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
    }

    /// `P6T15Probe.poly`. Every corner these fixtures produce is an `IntPoint`, so the rational
    /// arm of `pt` (`~(x,y)`) is unreachable here and asserted away.
    fn poly_of(polyline: &Polyline) -> String {
        let lines: Vec<String> = polyline.lines().iter().map(line_of).collect();
        let corners: Vec<String> = (0..polyline.corner_count())
            .map(|i| match polyline.corner(i) {
                Some(Point::Int(p)) => format!("({},{})", p.x, p.y),
                other => panic!("a rational corner {other:?} the JVM transcript does not carry"),
            })
            .collect();
        format!(
            "n={} lines=[{}] corners=[{}]",
            polyline.lines().len(),
            lines.join(","),
            corners.join(",")
        )
    }

    /// `P6T15Probe.boardDump`, restricted to the item kinds these two fixtures hold.
    fn board_dump(board: &Board) -> Vec<String> {
        let ctx = board.ctx();
        let mut out = vec![format!(
            "    maxId={}",
            board.communication.id_gen.max_generated_id()
        )];
        for item in board.get_items() {
            let type_name = match item {
                Item::Trace(_) => "PolylineTrace",
                Item::Pin(_) => "Pin",
                Item::BoardOutline(_) => "BoardOutline",
                other => panic!("the neck fixture holds no {other:?}"),
            };
            let nets: Vec<String> = item.net_nos().iter().map(i32::to_string).collect();
            let mut line = format!(
                "    item id={} type={} nets=[{}] cl={}",
                item.id().0,
                type_name,
                nets.join(","),
                item.clearance_class()
            );
            match item {
                Item::Trace(trace) => line.push_str(&format!(
                    " layer={} hw={} {}",
                    trace.get_layer(),
                    trace.get_half_width(),
                    poly_of(trace.polyline())
                )),
                Item::Pin(pin) => {
                    let center = pin.get_center(&ctx).to_float().round();
                    line.push_str(&format!(" center=({},{})", center.x, center.y));
                }
                _ => {}
            }
            out.push(line);
        }
        out
    }

    const WIDTHS: [i32; 2] = [100, 60];

    const SMD_CENTER: IntPoint = IntPoint { x: -400, y: 0 };
    const THRU_CENTER: IntPoint = IntPoint { x: 400, y: 0 };

    /// Probe mode `neck`: `tryNeckDown` (`:539-676`) over the five corner pairs times both pins
    /// times two trace half widths, then `insertNeckdown` (`:525-537`) over three corner pairs
    /// times the four `(startPin, endPin)` combinations.
    ///
    /// The discriminating rows are `pin=3 from=(-400,0) to=(400,0)` at half width 100 — the only
    /// one that reaches the `:578-660` insertion arm, leaving a 100-wide segment and a 69-wide
    /// neck behind — and the four `insertNeckdown … result=true` rows, which are the only ones in
    /// the file where `:662`'s neck segment reaches its target.
    #[test]
    fn the_neckdown_retry_matches_the_jvm() {
        let pairs: [(IntPoint, IntPoint); 5] = [
            (IntPoint::new(-200, 0), SMD_CENTER),
            (IntPoint::new(-100, 0), SMD_CENTER),
            (IntPoint::new(200, 0), THRU_CENTER),
            (IntPoint::new(-200, 300), SMD_CENTER),
            (SMD_CENTER, THRU_CENTER),
        ];
        let mut rows = Vec::new();
        for width in WIDTHS {
            for pin_choice in 0..2 {
                for (from, to) in pairs {
                    let mut board = neck_board(width);
                    let ctrl = neck_control(&board);
                    let pin = ItemId(if pin_choice == 0 { 2 } else { 3 });
                    let inserter = FoundConnectionInserter::new();
                    let counter = Counter::new();
                    let from = Point::Int(from);
                    let to = Point::Int(to);
                    let result = inserter
                        .try_neck_down(&mut board, &ctrl, None, &from, &to, 0, pin, true, &|| {
                            counter.check()
                        })
                        .expect("no fixture row reaches an error");
                    rows.push(format!(
                        "tryNeckDown hw={} pin={} from={} to={} result={} isFrom={} isTo={}",
                        width,
                        pin.0,
                        point_of(Some(&from)),
                        point_of(Some(&to)),
                        point_of(result.as_ref()),
                        result.as_ref() == Some(&from),
                        result.as_ref() == Some(&to),
                    ));
                    rows.extend(board_dump(&board));
                }
            }
        }
        let pairs2: [(IntPoint, IntPoint); 3] = [
            (IntPoint::new(-200, 0), THRU_CENTER),
            (SMD_CENTER, IntPoint::new(-310, 0)),
            (IntPoint::new(-310, 0), SMD_CENTER),
        ];
        for width in WIDTHS {
            for (from, to) in pairs2 {
                for start_choice in 0..2 {
                    for end_choice in 0..2 {
                        let mut board = neck_board(width);
                        let ctrl = neck_control(&board);
                        let start_pin = (start_choice != 0).then_some(ItemId(2));
                        let end_pin = (end_choice != 0).then_some(ItemId(3));
                        let inserter = FoundConnectionInserter::new();
                        let counter = Counter::new();
                        let from = Point::Int(from);
                        let to = Point::Int(to);
                        let result = inserter
                            .insert_neckdown(
                                &mut board,
                                &ctrl,
                                None,
                                &from,
                                &to,
                                0,
                                start_pin,
                                end_pin,
                                &|| counter.check(),
                            )
                            .expect("no fixture row reaches an error");
                        rows.push(format!(
                            "insertNeckdown hw={} from={} to={} startPin={} endPin={} result={}",
                            width,
                            point_of(Some(&from)),
                            point_of(Some(&to)),
                            start_pin.map_or_else(|| "null".to_string(), |id| id.0.to_string()),
                            end_pin.map_or_else(|| "null".to_string(), |id| id.0.to_string()),
                            result,
                        ));
                        rows.extend(board_dump(&board));
                    }
                }
            }
        }
        assert_rows_match("neck", &rows);
    }

    /// Probe mode `micro`: `insertFanoutMicroNeckdown` (`:455-523`), which exists here to pin the
    /// **insertion order** of `:462`'s `LinkedHashSet`.
    ///
    /// The far pair leaves room for every candidate, so the inserted trace's half width is
    /// literally the set's first element: 75 with no pins (`[75, 60, 50]`), 69 with the end pin
    /// only (`[69, 75, 60, 50]`), 49 with the start pin. A `BTreeSet` would answer 50, 50 and 49
    /// — so two of those four rows fail the moment the `Vec` is replaced by a sorted set. The
    /// `base=66` rows add the de-duplication: `max(1, 66 * 3 / 4)` is 49, the same value the smd
    /// pin contributed, and the set keeps the **first** position.
    #[test]
    fn the_micro_neckdown_candidate_order_is_javas_insertion_order() {
        let widths = [100, 66];
        let pairs: [(IntPoint, IntPoint); 2] = [
            (IntPoint::new(-700, 0), SMD_CENTER),
            (IntPoint::new(-300, 0), SMD_CENTER),
        ];
        let mut rows = Vec::new();
        for width in widths {
            for (from, to) in pairs {
                for start_choice in 0..2 {
                    for end_choice in 0..2 {
                        let mut board = neck_board(width);
                        let ctrl = neck_control(&board);
                        let start_pin = (start_choice != 0).then_some(ItemId(2));
                        let end_pin = (end_choice != 0).then_some(ItemId(3));
                        let inserter = FoundConnectionInserter::new();
                        let counter = Counter::new();
                        let from = Point::Int(from);
                        let to = Point::Int(to);
                        let result = inserter
                            .insert_fanout_micro_neckdown(
                                &mut board,
                                &ctrl,
                                None,
                                Some(&from),
                                &to,
                                0,
                                &[1],
                                start_pin,
                                end_pin,
                                &|| counter.check(),
                            )
                            .expect("no fixture row reaches an error");
                        rows.push(format!(
                            "micro base={} from={} to={} startPin={} endPin={} result={}",
                            width,
                            point_of(Some(&from)),
                            point_of(Some(&to)),
                            start_pin.map_or_else(|| "null".to_string(), |id| id.0.to_string()),
                            end_pin.map_or_else(|| "null".to_string(), |id| id.0.to_string()),
                            result,
                        ));
                        rows.extend(board_dump(&board));
                    }
                }
            }
        }
        // `:457-460`: an equal target is the early false, before the set is built at all.
        let mut board = neck_board(100);
        let ctrl = neck_control(&board);
        let inserter = FoundConnectionInserter::new();
        let counter = Counter::new();
        let from = Point::new(-700, 0);
        let same = inserter
            .insert_fanout_micro_neckdown(
                &mut board,
                &ctrl,
                None,
                Some(&from),
                &from,
                0,
                &[1],
                Some(ItemId(2)),
                None,
                &|| counter.check(),
            )
            .expect("the early return cannot fail");
        rows.push(format!("micro same={same}"));
        rows.extend(board_dump(&board));
        assert_rows_match("micro", &rows);
    }
}
