//! Port of `autoroute.maze.DestinationDistance` (DestinationDistance.java:11-391) —
//! "calculation of a good lower bound for the distance between a new MazeExpansionElement and the
//! destination set of the expansion".
//!
//! Pure and fully unit-testable: it holds three bounding boxes, a cost model derived once in the
//! constructor, and 250 lines of one-to-four-layer path enumeration. `tests/destination_distance.rs`
//! replays a JVM transcript over it row for row.
//!
//! # Every `Math.min`/`Math.max` here is [`java_min`]/[`java_max`], never `f64::min`/`f64::max`
//!
//! Java propagates a NaN (`Math.min` opens with `if (a != a) return a;`); Rust's `f64::min`
//! *absorbs* it (`x.min(NaN) == x`, IEEE 754-2019 `minimumNumber`), and the two also disagree on
//! signed zero. That is not academic here: quirk #170 — `MazeListElement.compareTo`'s NaN
//! fall-through, and the whole reason its comparator is a transcribed `<`/`>` chain rather than
//! `total_cmp` — argues that a NaN reaches `sortingValue` **through this method**
//! (`MazeSearchEngine.java:884`). On `f64::min` it never could: the first
//! `result = result.min(tmp)` would discard it, and the port would be internally inconsistent.
//! `IntBox::weighted_distance`, which `calculate` calls, already uses `java_max` for the same
//! reason. Pinned by `P6T8Probe` mode `nan` and
//! `tests/destination_distance.rs::a_nan_trace_cost_propagates_through_calculate_as_java_does`.

use fr_geometry::{FloatPoint, IntBox, java_max, java_min};
use fr_settings::ExpansionCostFactor;

/// Port of `maze.DestinationDistance` (DestinationDistance.java:11-391).
///
/// The nine constructor-derived cost fields are `pub` because Java's are package-private and
/// `tests/destination_distance.rs` compares them against the probe's `costs …` row; nothing
/// writes them after the constructor.
#[derive(Debug, Clone, PartialEq)]
pub struct DestinationDistance {
    /// `private final ExpansionCostFactor[] traceCosts` (`:13`).
    trace_costs: Vec<ExpansionCostFactor>,
    /// `private final boolean[] layerActive` (`:14`). Kept for the same reason Java keeps it:
    /// nothing reads it after the constructor, but it is the array `layerCount` is measured from.
    #[allow(dead_code)]
    layer_active: Vec<bool>,
    /// `private final int layerCount` (`:15`) = `layerActive.length` (`:52`).
    layer_count: usize,
    /// `private final int activeLayerCount` (`:16`), `:55-61`.
    active_layer_count: usize,
    /// `private final double minCheapViaCost` (`:17`), the cost
    /// [`calculate_cheap_distance`](Self::calculate_cheap_distance) substitutes.
    min_cheap_via_cost: f64,
    /// `double minComponentSideTraceCost` (`:18`), `:63-71`.
    pub min_component_side_trace_cost: f64,
    /// `double maxComponentSideTraceCost` (`:19`), `:63-71`.
    pub max_component_side_trace_cost: f64,
    /// `double minSolderSideTraceCost` (`:20`), `:73-83`.
    pub min_solder_side_trace_cost: f64,
    /// `double maxSolderSideTraceCost` (`:21`), `:73-83`.
    pub max_solder_side_trace_cost: f64,
    /// `double maxInnerSideTraceCost` (`:22`): "minimum of the maximal trace costs on each inner
    /// layer", `:86-94`.
    pub max_inner_side_trace_cost: f64,
    /// `double minComponentInnerTraceCost` (`:24`): "minimum of minComponentSideTraceCost and
    /// maxInnerSideTraceCost", `:95`.
    pub min_component_inner_trace_cost: f64,
    /// `double minSolderInnerTraceCost` (`:27`): "minimum of minSolderSideTraceCost and
    /// maxInnerSideTraceCost", `:96`.
    pub min_solder_inner_trace_cost: f64,
    /// `double minComponentSolderInnerTraceCost` (`:29`): "minimum of minComponentInnerTraceCost
    /// and minSolderInnerTraceCost", `:97-98`.
    pub min_component_solder_inner_trace_cost: f64,
    /// `private double minNormalViaCost` (`:30`).
    ///
    /// Java's is **not** final: `calculateCheapDistance` (`:382-390`) overwrites it, calls
    /// `calculate` and restores it. The port keeps the field immutable and threads the cost
    /// through instead — see [`calculate_cheap_distance`](Self::calculate_cheap_distance).
    min_normal_via_cost: f64,
    /// `private IntBox componentSideBox = IntBox.EMPTY` (`:33`).
    component_side_box: IntBox,
    /// `private IntBox solderSideBox = IntBox.EMPTY` (`:34`).
    solder_side_box: IntBox,
    /// `private IntBox innerSideBox = IntBox.EMPTY` (`:35`).
    inner_side_box: IntBox,
    /// `private boolean boxIsEmpty = true` (`:36`).
    box_is_empty: bool,
    /// `private boolean componentSideBoxIsEmpty = true` (`:37`).
    component_side_box_is_empty: bool,
    /// `private boolean solderSideBoxIsEmpty = true` (`:38`).
    solder_side_box_is_empty: bool,
    /// `private boolean innerSideBoxIsEmpty = true` (`:39`).
    inner_side_box_is_empty: bool,
}

impl DestinationDistance {
    /// Port of `DestinationDistance(ExpansionCostFactor[], boolean[], double, double)`
    /// (`:45-99`). "traceCosts and layerActive are arrays of dimension layerCount."
    pub fn new(
        trace_costs: &[ExpansionCostFactor],
        layer_active: &[bool],
        min_normal_via_cost: f64,
        min_cheap_via_cost: f64,
    ) -> DestinationDistance {
        let layer_count = layer_active.len(); // :52
        let active_layer_count = layer_active.iter().filter(|a| **a).count(); // :55-61

        // :63-71. Java leaves both at their `double` default of 0.0 when layer 0 is inactive.
        let (min_component_side_trace_cost, max_component_side_trace_cost) = if layer_active[0] {
            let c = trace_costs[0];
            if c.horizontal < c.vertical {
                (c.horizontal, c.vertical)
            } else {
                (c.vertical, c.horizontal)
            }
        } else {
            (0.0, 0.0)
        };

        // :73-83
        let (min_solder_side_trace_cost, max_solder_side_trace_cost) =
            if layer_active[layer_count - 1] {
                let c = trace_costs[layer_count - 1];
                if c.horizontal < c.vertical {
                    (c.horizontal, c.vertical)
                } else {
                    (c.vertical, c.horizontal)
                }
            } else {
                (0.0, 0.0)
            };

        // :85-94. "Note: for inner layers we assume, that cost in preferred direction is 1."
        let mut max_inner_side_trace_cost =
            java_min(max_component_side_trace_cost, max_solder_side_trace_cost);
        for ind2 in 1..layer_count.saturating_sub(1) {
            if !layer_active[ind2] {
                continue; // :88-90
            }
            let current_max_cost =
                java_max(trace_costs[ind2].horizontal, trace_costs[ind2].vertical);
            max_inner_side_trace_cost = java_min(max_inner_side_trace_cost, current_max_cost);
        }
        // :95-98
        let min_component_inner_trace_cost =
            java_min(min_component_side_trace_cost, max_inner_side_trace_cost);
        let min_solder_inner_trace_cost =
            java_min(min_solder_side_trace_cost, max_inner_side_trace_cost);
        let min_component_solder_inner_trace_cost =
            java_min(min_component_inner_trace_cost, min_solder_inner_trace_cost);

        DestinationDistance {
            trace_costs: trace_costs.to_vec(),
            layer_active: layer_active.to_vec(),
            layer_count,
            active_layer_count,
            min_cheap_via_cost,
            min_component_side_trace_cost,
            max_component_side_trace_cost,
            min_solder_side_trace_cost,
            max_solder_side_trace_cost,
            max_inner_side_trace_cost,
            min_component_inner_trace_cost,
            min_solder_inner_trace_cost,
            min_component_solder_inner_trace_cost,
            min_normal_via_cost,
            component_side_box: IntBox::EMPTY,
            solder_side_box: IntBox::EMPTY,
            inner_side_box: IntBox::EMPTY,
            box_is_empty: true,
            component_side_box_is_empty: true,
            solder_side_box_is_empty: true,
            inner_side_box_is_empty: true,
        }
    }

    /// Port of `join(IntBox, int)` (`:101-114`): "joins box to the bounding box of the specified
    /// layer."
    ///
    /// Three buckets, not `layerCount`: layer 0, layer `layerCount - 1`, and **one shared union**
    /// for every inner layer (`:109-112`).
    pub fn join(&mut self, box_to_join: &IntBox, layer: usize) {
        if layer == 0 {
            self.component_side_box = self.component_side_box.union(box_to_join);
            self.component_side_box_is_empty = false;
        } else if layer == self.layer_count - 1 {
            self.solder_side_box = self.solder_side_box.union(box_to_join);
            self.solder_side_box_is_empty = false;
        } else {
            self.inner_side_box = self.inner_side_box.union(box_to_join);
            self.inner_side_box_is_empty = false;
        }
        self.box_is_empty = false;
    }

    /// Port of `calculate(FloatPoint, int)` (`:116-119`): "calculates the lower bound distance
    /// from point on layer."
    ///
    /// renamed: `DestinationDistance.calculate(FloatPoint, int)` ->
    /// `calculate_from_point`; Rust has no overloading, and the `IntBox` arm keeps the plain
    /// name because it is the one `calculateCheapDistance` calls.
    pub fn calculate_from_point(&self, point: &FloatPoint, layer: usize) -> f64 {
        self.calculate(&point.bounding_box(), layer)
    }

    /// Port of `calculate(IntBox, int)` (`:121-379`): "calculates the lower bound distance from
    /// box on layer."
    pub fn calculate(&self, box_: &IntBox, layer: usize) -> f64 {
        self.calculate_with_via_cost(box_, layer, self.min_normal_via_cost)
    }

    /// Port of `calculateCheapDistance(IntBox, int)` (`:381-390`): "calculates cheap distance for
    /// box on layer using cheap via cost."
    ///
    /// Java does it by **mutating** `minNormalViaCost` to `minCheapViaCost`, calling `calculate`
    /// and writing the old value back (hazard J). The port passes the cost down as a parameter,
    /// so the method takes `&self` and no state can be observed mid-flight. The answers are
    /// identical: `minNormalViaCost` is read at eleven places inside `calculate` and written
    /// nowhere else.
    ///
    /// This method has **no caller anywhere in the Java tree** at HEAD — `grep -rn
    /// calculateCheapDistance src/` finds only its own declaration. It is ported because
    /// `audit-port.sh` demands every `public` member, and because `minCheapViaCost` exists on
    /// `AutorouteControl` solely to be handed to this constructor.
    pub fn calculate_cheap_distance(&self, box_: &IntBox, layer: usize) -> f64 {
        self.calculate_with_via_cost(box_, layer, self.min_cheap_via_cost)
    }

    /// The body of `calculate(IntBox, int)` (`:121-379`), with `minNormalViaCost` as a parameter
    /// rather than a mutable field (see [`calculate_cheap_distance`](Self::calculate_cheap_distance)).
    fn calculate_with_via_cost(
        &self,
        box_: &IntBox,
        layer: usize,
        min_normal_via_cost: f64,
    ) -> f64 {
        if self.box_is_empty {
            // :123-125. `Integer.MAX_VALUE` widened to a `double`, not `Double.MAX_VALUE`.
            return f64::from(i32::MAX);
        }

        // :127-144
        let (component_side_delta_x, component_side_delta_y) =
            deltas(box_, &self.component_side_box);
        // :146-163
        let (solder_side_delta_x, solder_side_delta_y) = deltas(box_, &self.solder_side_box);
        // :165-182
        let (inner_side_delta_x, inner_side_delta_y) = deltas(box_, &self.inner_side_box);

        // :184-193
        let (component_side_max_delta, component_side_min_delta) =
            max_min(component_side_delta_x, component_side_delta_y);
        // :195-204
        let (solder_side_max_delta, solder_side_min_delta) =
            max_min(solder_side_delta_x, solder_side_delta_y);
        // :206-215
        let (inner_side_max_delta, inner_side_min_delta) =
            max_min(inner_side_delta_x, inner_side_delta_y);

        let mut result = f64::from(i32::MAX); // :217

        if layer == 0 {
            // :219 — calculate shortest distance to component side box.
            // :220-226 — one layer distance.
            if !self.component_side_box_is_empty {
                result = box_.weighted_distance(
                    &self.component_side_box,
                    self.trace_costs[0].horizontal,
                    self.trace_costs[0].vertical,
                );
            }
            if self.active_layer_count <= 1 {
                return result; // :228-230
            }

            // :232-245 — two layer distance on component and solder side.
            let mut tmp_distance =
                if self.min_solder_side_trace_cost < self.min_component_side_trace_cost {
                    self.min_solder_side_trace_cost * solder_side_max_delta
                        + self.min_component_side_trace_cost * solder_side_min_delta
                        + min_normal_via_cost
                } else {
                    self.min_component_side_trace_cost * solder_side_max_delta
                        + self.min_solder_side_trace_cost * solder_side_min_delta
                        + min_normal_via_cost
                };
            result = java_min(result, tmp_distance); // :247

            // :249-255 — two layer distance on component and solder side with two vias.
            tmp_distance = component_side_max_delta
                + component_side_min_delta * self.min_component_inner_trace_cost
                + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance); // :257

            if self.active_layer_count == 2 {
                return result; // :259-261
            }

            // :263-266 — two layer distance on component side and an inner side.
            tmp_distance = inner_side_max_delta
                + inner_side_min_delta * self.min_component_inner_trace_cost
                + min_normal_via_cost;
            result = java_min(result, tmp_distance); // :268

            // :270-276 — three layer distance. The `+ +` at `:274` is Java's own double unary
            // plus, which is a no-op.
            tmp_distance = solder_side_max_delta
                + self.min_component_solder_inner_trace_cost * solder_side_min_delta
                + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            // :278-279
            tmp_distance =
                component_side_max_delta + component_side_min_delta + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            if self.active_layer_count == 3 {
                return result; // :281-283
            }

            // :285-287
            tmp_distance = inner_side_max_delta + inner_side_min_delta + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            // :289-293 — four layer distance.
            tmp_distance =
                solder_side_max_delta + solder_side_min_delta + 3.0 * min_normal_via_cost;
            return java_min(result, tmp_distance);
        }

        if layer == self.layer_count - 1 {
            // :295 — calculate the shortest distance to solder side box.
            // :296-302 — one layer distance.
            if !self.solder_side_box_is_empty {
                result = box_.weighted_distance(
                    &self.solder_side_box,
                    self.trace_costs[layer].horizontal,
                    self.trace_costs[layer].vertical,
                );
            }

            // :304-316 — two layer distance.
            let mut tmp_distance =
                if self.min_component_side_trace_cost < self.min_solder_side_trace_cost {
                    self.min_component_side_trace_cost * component_side_max_delta
                        + self.min_solder_side_trace_cost * component_side_min_delta
                        + min_normal_via_cost
                } else {
                    self.min_solder_side_trace_cost * component_side_max_delta
                        + self.min_component_side_trace_cost * component_side_min_delta
                        + min_normal_via_cost
                };
            result = java_min(result, tmp_distance); // :317

            // :318-320
            tmp_distance = solder_side_max_delta
                + solder_side_min_delta * self.min_solder_inner_trace_cost
                + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            if self.active_layer_count <= 2 {
                return result; // :321-323
            }

            // :324-326
            tmp_distance = inner_side_min_delta * self.min_solder_inner_trace_cost
                + inner_side_max_delta
                + min_normal_via_cost;
            result = java_min(result, tmp_distance);

            // :328-334 — three layer distance.
            tmp_distance = component_side_max_delta
                + self.min_component_solder_inner_trace_cost * component_side_min_delta
                + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            // :335-336
            tmp_distance =
                solder_side_max_delta + solder_side_min_delta + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            if self.active_layer_count == 3 {
                return result; // :337-339
            }

            // :340-341
            tmp_distance = inner_side_max_delta + inner_side_min_delta + 2.0 * min_normal_via_cost;
            result = java_min(result, tmp_distance);

            // :343-346 — four layer distance.
            tmp_distance =
                component_side_max_delta + component_side_min_delta + 3.0 * min_normal_via_cost;
            return java_min(result, tmp_distance);
        }

        // :349-357 — distance to inner layer box, one layer distance.
        if !self.inner_side_box_is_empty {
            result = box_.weighted_distance(
                &self.inner_side_box,
                self.trace_costs[layer].horizontal,
                self.trace_costs[layer].vertical,
            );
        }

        // :359-363 — two layer distance.
        let mut tmp_distance = inner_side_max_delta + inner_side_min_delta + min_normal_via_cost;
        result = java_min(result, tmp_distance);
        // :364-368
        tmp_distance = component_side_max_delta
            + component_side_min_delta * self.min_component_inner_trace_cost
            + min_normal_via_cost;
        result = java_min(result, tmp_distance);
        // :369-371
        tmp_distance = solder_side_max_delta
            + solder_side_min_delta * self.min_solder_inner_trace_cost
            + min_normal_via_cost;
        result = java_min(result, tmp_distance);

        // :373-376 — three layer distance.
        tmp_distance =
            component_side_max_delta + component_side_min_delta + 2.0 * min_normal_via_cost;
        result = java_min(result, tmp_distance);
        // :377-378
        tmp_distance = solder_side_max_delta + solder_side_min_delta + 2.0 * min_normal_via_cost;
        java_min(result, tmp_distance)
    }
}

/// The axis gap between two boxes, `0` where they overlap on that axis — `:130-144` and its two
/// copies at `:149-163` and `:168-182`, which are textually identical.
///
/// Java's operands are two `int`s and the *subtraction* happens in `int` before the result is
/// widened to the `double` it is assigned to, so it wraps rather than saturating; `wrapping_sub`
/// keeps that. It is not reachable on a real board — `IntBox.EMPTY`'s corners are `±CRIT_INT`
/// (`2^25`) and every board coordinate is inside that, so the widest gap is `2^26` — but the port
/// is not in a position to prove the bound for a caller, and a silent `f64` subtraction here
/// would be the one place the two languages' arithmetic differs.
fn deltas(box_: &IntBox, other: &IntBox) -> (f64, f64) {
    let delta_x = if box_.ll.x > other.ur.x {
        f64::from(box_.ll.x.wrapping_sub(other.ur.x))
    } else if box_.ur.x < other.ll.x {
        f64::from(other.ll.x.wrapping_sub(box_.ur.x))
    } else {
        0.0
    };
    let delta_y = if box_.ll.y > other.ur.y {
        f64::from(box_.ll.y.wrapping_sub(other.ur.y))
    } else if box_.ur.y < other.ll.y {
        f64::from(other.ll.y.wrapping_sub(box_.ur.y))
    } else {
        0.0
    };
    (delta_x, delta_y)
}

/// `(max, min)` of the two deltas — `:187-193` and its two copies. Java's test is a strict `>`,
/// so a tie takes the `else` branch; with equal inputs the answer is the same either way.
fn max_min(delta_x: f64, delta_y: f64) -> (f64, f64) {
    if delta_x > delta_y {
        (delta_x, delta_y)
    } else {
        (delta_y, delta_x)
    }
}
