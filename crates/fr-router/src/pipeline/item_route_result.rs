//! Port of `autoroute/ItemRouteResult.java` (145 lines) — the optimizer's per-item scorecard.
//!
//! One value object, built once per optimized item by `BatchOptimizer.optRouteItem`, holding the
//! via count, the trace length and the incomplete count on both sides of a re-route. The
//! interesting code is all in the **constructor**: `:39-57` runs the improvement ladder and
//! `:59-65` computes the improvement percentage, and the two methods the plan's draft named
//! `improved()` (`:39-57`) and `improvementPercentage()` (`:59-65`) are in fact the two plain
//! getters at `:102-104` and `:107-109`. Java wins; the port has the same shape.

use std::cmp::Ordering;

use fr_board::ItemId;

/// Port of `autoroute.ItemRouteResult` (ItemRouteResult.java:1-145) — the routing result of a
/// single item, comparing metrics before and after routing.
///
/// # The field block, transcribed
///
/// Java's nine fields at `:6-14`, in order: `itemId`, `improvementPercentage`, `viaCountBefore`,
/// `viaCountAfter`, `traceLengthBefore`, `traceLengthAfter`, `incompleteCountBefore`,
/// `incompleteCountAfter`, `improved`. All are `private final` except `improved`, which
/// [`ItemRouteResult::update_improved`] (`:137-139`) writes.
///
/// There is **no `minCumulativeTraceLength` field**, which the plan's first draft listed: that
/// name is a local of `BatchOptimizer.optRoutePass` (`BatchOptimizer.java:288`) and never reaches
/// this class.
///
/// `improvementPercentage` is a **`float`**, not a `double` (`:7`, and `:107` returns one) — the
/// expression at `:59-65` is computed in `double` and cast down at `:60`. The port's accessor
/// answers `f32` for the same reason, and the plan's `-> f64` is corrected here.
///
/// # `itemId` is Java's `int`, held as an [`ItemId`]
///
/// `:6` is an `int` and its only producer is `item.getId()` (`BatchOptimizer.java:255`), so the
/// port holds the crate's id newtype. Nothing constructs one from a non-id integer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemRouteResult {
    /// `private final int itemId` (`:6`).
    item_id: ItemId,
    /// `private final float improvementPercentage` (`:7`), computed at `:59-65`.
    improvement_percentage: f32,
    /// `private final int viaCountBefore` (`:8`).
    via_count_before: i32,
    /// `private final int viaCountAfter` (`:9`).
    via_count_after: i32,
    /// `private final double traceLengthBefore` (`:10`).
    trace_length_before: f64,
    /// `private final double traceLengthAfter` (`:11`).
    trace_length_after: f64,
    /// `private final int incompleteCountBefore` (`:12`).
    incomplete_count_before: i32,
    /// `private final int incompleteCountAfter` (`:13`).
    incomplete_count_after: i32,
    /// `private boolean improved` (`:14`), decided at `:39-57` and re-writable at `:137-139`.
    improved: bool,
}

impl ItemRouteResult {
    /// Port of `ItemRouteResult(int)` (`:17-20`): "constructs an unimproved `ItemRouteResult` for
    /// the given item ID."
    ///
    /// Java delegates to the seven-argument constructor with `(itemId, 0, 0, 0, 0, 0, 1)` and
    /// **then** assigns `improved = false` at `:19`. That assignment is redundant — the ladder
    /// has already answered `false`, because `incompleteCountAfter = 1` is greater than
    /// `incompleteCountBefore = 0` (`:41-42`) — and the port keeps the delegation rather than the
    /// dead store, which is the same object either way. `P7T9Probe`'s `[unimproved-ctor]` line
    /// pins every field of it.
    pub fn unimproved(item_id: ItemId) -> ItemRouteResult {
        // :18.
        ItemRouteResult::new(item_id, 0, 0, 0.0, 0.0, 0, 1)
    }

    /// Port of the seven-argument constructor (`:23-66`): "constructs an `ItemRouteResult`
    /// comparing metrics before and after routing." It stores the six measurements, runs the
    /// improvement ladder (`:39-57`) and computes the improvement percentage (`:59-65`).
    ///
    /// # The ladder, `:39-57`
    ///
    /// Three rungs, each consulted only when the one above ties: the incomplete count, then the
    /// via count, then the trace length. The bottom arm (`:53-54`) makes an **exact tie** count
    /// as *not* improved, which is the one arm with no comparison behind it.
    ///
    /// # `improvementPercentage`, `:59-65` — **quirk #212**
    ///
    /// See [`ItemRouteResult::improvement_percentage`]: `viaCountAfter / viaCountBefore` is an
    /// `int` division.
    pub fn new(
        item_id: ItemId,
        via_count_before: i32,
        via_count_after: i32,
        trace_length_before: f64,
        trace_length_after: f64,
        incomplete_count_before: i32,
        incomplete_count_after: i32,
    ) -> ItemRouteResult {
        // :39-57 — the ladder.
        let improved = if incomplete_count_after < incomplete_count_before {
            // :39-40.
            true
        } else if incomplete_count_after > incomplete_count_before {
            // :41-42.
            false
        } else if via_count_after < via_count_before {
            // :44-45.
            true
        } else if via_count_after > via_count_before {
            // :46-47.
            false
        } else if trace_length_after < trace_length_before {
            // :49-50.
            true
        } else {
            // :51-54 — a longer trace and an exact tie are both `false`, so the two arms fold.
            // Written as one `else` rather than as Java's two because they assign the same value
            // and no side effect distinguishes them.
            false
        };

        // :59-65. `viaCountAfter / viaCountBefore` is an **int** division (quirk #212); the
        // `i32` division below truncates towards zero exactly as Java's does, and `:61`'s guard
        // is what keeps it from dividing by zero.
        // Java bug: `ItemRouteResult.<init>` — the seven-argument constructor (`:23-66`), at `:63`: `viaCountAfter / viaCountBefore` is `int / int`, so the via term truncates to 0 or 1 while the trace term is a `double` (quirk #212).
        let improvement_percentage = if via_count_before != 0 && trace_length_before != 0.0 {
            let via_term = f64::from(via_count_after / via_count_before);
            let length_term = trace_length_after / trace_length_before;
            (1.0 - ((via_term + length_term) / 2.0)) as f32
        } else {
            0.0
        };

        ItemRouteResult {
            // :31-37.
            item_id,
            improvement_percentage,
            via_count_before,
            via_count_after,
            trace_length_before,
            trace_length_after,
            incomplete_count_before,
            incomplete_count_after,
            improved,
        }
    }

    /// Port of `compareTo(ItemRouteResult)` (`:69-89`) — the same three-rung ladder as the
    /// constructor's, but between two results' *after* measurements: incompletes, then vias,
    /// then trace length, `-1` / `0` / `+1`.
    ///
    /// # It has no live caller
    ///
    /// `ItemRouteResult implements Comparable` for one reason: the
    /// `PriorityQueue<ItemRouteResult>` inside the GUI-only multithreaded optimizer
    /// (`autoroute/pipeline/BatchAutorouterThread.java`), which nothing on the headless path
    /// constructs. It is ported for the audit and pinned behaviourally by
    /// `crates/fr-router/tests/item_route_result.rs`'s `compare_to_matches_the_jvm`, which
    /// asserts the *ordering* it produces over `P7T9Probe`'s 500 tuples rather than a per-pair
    /// value.
    ///
    /// # Why `Ordering` and not `Ord`
    ///
    /// The trace-length rung compares `f64`s with `<` and `>` and answers `0` when neither holds
    /// (`:84-86`), which is what Java's `compareTo` does with a NaN on either side. That relation
    /// is not a total order, so implementing [`Ord`] would be a lie; the method is spelled out
    /// instead and the tests sort with `sort_by`.
    pub fn compare_to(&self, r: &ItemRouteResult) -> Ordering {
        if self.incomplete_count_after < r.incomplete_count_after {
            // :70-71.
            Ordering::Less
        } else if self.incomplete_count_after > r.incomplete_count_after {
            // :72-73.
            Ordering::Greater
        } else if self.via_count_after < r.via_count_after {
            // :75-76.
            Ordering::Less
        } else if self.via_count_after > r.via_count_after {
            // :77-78.
            Ordering::Greater
        } else if self.trace_length_after < r.trace_length_after {
            // :80-81.
            Ordering::Less
        } else if self.trace_length_after > r.trace_length_after {
            // :82-83.
            Ordering::Greater
        } else {
            // :84-86.
            Ordering::Equal
        }
    }

    /// Port of `improvedOver(ItemRouteResult)` (`:92-94`): "returns true if this result represents
    /// an improvement over r" — `compareTo(r) < 0`.
    pub fn improved_over(&self, r: &ItemRouteResult) -> bool {
        // :93.
        self.compare_to(r) == Ordering::Less
    }

    /// Port of `itemId()` (`:97-99`) — "the ID of the routed item".
    pub fn item_id(&self) -> ItemId {
        self.item_id
    }

    /// Port of `improved()` (`:102-104`) — "true if the routing result was improved".
    ///
    /// A plain getter: the ladder that decided the value is the **constructor**'s (`:39-57`), and
    /// [`ItemRouteResult::update_improved`] can overwrite it afterwards.
    pub fn improved(&self) -> bool {
        self.improved
    }

    /// Port of `improvementPercentage()` (`:107-109`) — "the calculated improvement percentage",
    /// a `float`.
    ///
    /// **Java bug (quirk #212).** The value is `1.0 - ((viaCountAfter / viaCountBefore) +
    /// (traceLengthAfter / traceLengthBefore)) / 2`, and the first ratio is an `int` division
    /// (`:63`): it truncates towards zero, so it is `0` whenever the re-route left fewer vias
    /// than it started with and `1` when the counts are equal, while the trace term beside it is
    /// a full `double`. A re-route that halves the vias contributes exactly as much as one that
    /// removes them all.
    ///
    /// `BatchOptimizer.java:340-348` writes the same shape of expression with a `(float)` cast in
    /// front of the via term and therefore gets the right answer — so **the field is wrong and
    /// the value the optimizer uses is right**, and both are ported: this one here, the
    /// optimizer's in Task 14. Nothing reads this field on the headless path, which is why the
    /// bug has no corpus consequence.
    pub fn improvement_percentage(&self) -> f32 {
        self.improvement_percentage
    }

    /// Port of `viaCount()` (`:112-114`) — the via count **after** routing.
    pub fn via_count(&self) -> i32 {
        self.via_count_after
    }

    /// Port of `traceLength()` (`:117-119`) — the total trace length **after** routing.
    pub fn trace_length(&self) -> f64 {
        self.trace_length_after
    }

    /// Port of `incompleteCount()` (`:122-124`) — incomplete connections **after** routing.
    pub fn incomplete_count(&self) -> i32 {
        self.incomplete_count_after
    }

    /// Port of `viaCountReduced()` (`:127-129`) — `viaCountBefore - viaCountAfter`, so a
    /// *negative* answer means the re-route added vias.
    pub fn via_count_reduced(&self) -> i32 {
        // :128. Java's `int` arithmetic wraps on overflow; the port's `wrapping_sub` says so
        // rather than panicking in a debug build, and no caller can reach the two-billion-via
        // board that would show it.
        self.via_count_before.wrapping_sub(self.via_count_after)
    }

    /// Port of `lengthReduced()` (`:132-134`) — `traceLengthBefore - traceLengthAfter`.
    pub fn length_reduced(&self) -> f64 {
        // :133.
        self.trace_length_before - self.trace_length_after
    }

    /// Port of `updateImproved(boolean)` (`:137-139`) — the only mutator on the class, and the
    /// only reason `improved` is not `final`. `BatchOptimizer.optRouteItem` calls it when the
    /// board-level verdict differs from the per-item one the constructor computed.
    pub fn update_improved(&mut self, improved: bool) {
        // :138.
        self.improved = improved;
    }

    /// Port of `incompleteCountBefore()` (`:142-144`).
    pub fn incomplete_count_before(&self) -> i32 {
        self.incomplete_count_before
    }
}
