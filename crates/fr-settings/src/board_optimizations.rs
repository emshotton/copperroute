//! `RouterSettings.applyBoardSpecificOptimizations` and its two companions
//! (`settings/RouterSettings.java:240-259, 266-435`): the one place where board *geometry* feeds
//! routing *costs*.
//!
//! Kept in its own file rather than in [`crate::router_settings`] because it is the only part of
//! `RouterSettings` that depends on `fr-board` (plan ruling 5), and because the audit map
//! (`scripts/audit-map/fr-settings.map`) already points `RouterSettings` at both files.
//!
//! # What the aspect ratio does
//!
//! The board's bounding box gives two penalties, each `0.1 * Math.round(10 * ratio)` — i.e. the
//! ratio rounded to one decimal, half-up (`:299-303`). Every layer alternates its preferred
//! trace direction, and the penalty for the *other* direction is the one that matches the layer's
//! preferred direction: a wide board makes going against a horizontal layer expensive. All of it
//! is `f64` from the two `int` widths onwards (plan ruling 5), and `Math.round` is
//! [`fr_geometry::java_round`], which returns a `long` and rounds half towards positive infinity
//! — not Rust's `f64::round`, which rounds half away from zero.

use fr_board::Board;
use fr_geometry::java_round;

use crate::{LayerSettings, RouterSettings, ScoringSettings};

impl RouterSettings {
    /// `RouterSettings.applyBoardSpecificOptimizationsIfNeeded` (`:245-253`): re-tune only when
    /// the layer count disagrees with the board's, or the costs were never board-tuned.
    ///
    /// Java's `board == null` guard (`:246-248`) has no counterpart: `&Board` cannot be null.
    ///
    /// The `!Boolean.TRUE.equals(...)` half of the condition is exactly
    /// [`Self::are_board_specific_trace_costs_applied`], so a `null` flag re-tunes — which is
    /// what makes `RoutingJobScheduler.java:186` recompute costs on a freshly merged settings
    /// object (`copyFields` never copies the flag: it is `private`; see `docs/java-quirks.md`
    /// #127).
    pub fn apply_board_specific_optimizations_if_needed(&mut self, board: &Board) {
        let board_layer_count = board.get_layer_count();
        if self.get_layer_count() != board_layer_count
            || !self.are_board_specific_trace_costs_applied()
        {
            self.apply_board_specific_optimizations(board);
        }
    }

    /// `RouterSettings.areBoardSpecificTraceCostsApplied` (`:257-259`):
    /// `Boolean.TRUE.equals(boardSpecificTraceCostsApplied)`, so an absent flag reads `false`.
    pub fn are_board_specific_trace_costs_applied(&self) -> bool {
        matches!(self.board_specific_trace_costs_applied, Some(true))
    }

    /// `RouterSettings.applyBoardSpecificOptimizations` (`:266-435`): derives the per-layer
    /// routability, preferred direction, bend cost and trace costs from the board's bounding box
    /// and layer structure.
    ///
    /// Transcribed in Java's order:
    /// 1. the two widths and the layer count (`:267-270`);
    /// 2. `scoring` is instantiated when absent (`:285-287`);
    /// 3. the two aspect-ratio penalties (`:299-303`);
    /// 4. `layers` is reallocated when absent or the wrong length, keeping the entries that fit
    ///    (`:306-318`), which clears the applied flag (`:307`);
    /// 5. each cost array is reallocated when absent or the wrong length, clearing the flag again
    ///    (`:325-334`);
    /// 6. the two `default*TraceCost`s fall back to `1.0` (`:338-343`);
    /// 7. the running direction and the `initializeTraceCosts` decision (`:345-346`);
    /// 8. the per-layer loop (`:348-374`);
    /// 9. the outer-layer bonus and the applied flag (`:375-387`).
    ///
    /// not ported: the `originalRoutable`/`originalBendCost`/`originalPrefHoriz`/
    /// `originalPrefCost`/`originalUndesiredCost` snapshot (`:272-295`) and the change summary it
    /// exists for (`:388-434`). The summary's only consumer is `FRLogger.debug`, which the plan's
    /// Global Constraints drop, so the snapshot has no reader left; the `scoring == null` guard
    /// that sits between its two halves (`:285-287`) *is* ported, because the rest of the method
    /// depends on it.
    pub fn apply_board_specific_optimizations(&mut self, board: &Board) {
        // :267-268. `IntBox.width()`/`height()` are `int`s widened to `double`, exactly as Java
        // widens them at the assignment.
        let bounding_box = board.get_bounding_box();
        let horizontal_width = f64::from(bounding_box.width());
        let vertical_width = f64::from(bounding_box.height());

        // :270.
        let layer_count = board.get_layer_count();

        // :285-287.
        if self.scoring.is_none() {
            self.scoring = Some(ScoringSettings::default());
        }

        // :299-303 — "additional costs against preferred direction with 1 digit behind the
        // decimal point". `java_round` is `Math.round(double) -> long`; the `as f64` is Java's
        // implicit widening of that `long` before the multiplication.
        let horizontal_add_costs_against_preferred_dir =
            0.1 * java_round(10.0 * horizontal_width / vertical_width) as f64;
        let vertical_add_costs_against_preferred_dir =
            0.1 * java_round(10.0 * vertical_width / horizontal_width) as f64;

        // :306-324 — initialize the layer specific settings.
        //
        // Java's `else` arm (:317-323) replaces any `null` *element* with a fresh
        // `LayerSettings`. `Vec<LayerSettings>` has no null element to replace — plan ruling 4
        // makes the *fields* nullable, not the array slots — so that arm is a structural no-op
        // here and only the reallocation branch has anything to do.
        if !matches!(&self.layers, Some(layers) if layers.len() == layer_count) {
            self.board_specific_trace_costs_applied = Some(false);
            let old_layers = self.layers.take();
            let mut layers = Vec::with_capacity(layer_count);
            for i in 0..layer_count {
                layers.push(match old_layers.as_ref().and_then(|old| old.get(i)) {
                    Some(existing) => *existing,
                    None => LayerSettings::default(),
                });
            }
            self.layers = Some(layers);
        }

        // :325-334, plus :338-343's guard "against null defaults that can occur when
        // RouterSettings is deserialized without going through DefaultSettings".
        let scoring = self.scoring.as_mut().expect("instantiated above");
        let mut costs_reallocated = false;
        if !matches!(&scoring.preferred_direction_trace_cost, Some(a) if a.len() == layer_count) {
            scoring.preferred_direction_trace_cost = Some(vec![0.0; layer_count]);
            costs_reallocated = true;
        }
        if !matches!(&scoring.undesired_direction_trace_cost, Some(a) if a.len() == layer_count) {
            scoring.undesired_direction_trace_cost = Some(vec![0.0; layer_count]);
            costs_reallocated = true;
        }
        if scoring.default_preferred_direction_trace_cost.is_none() {
            scoring.default_preferred_direction_trace_cost = Some(1.0);
        }
        if scoring.default_undesired_direction_trace_cost.is_none() {
            scoring.default_undesired_direction_trace_cost = Some(1.0);
        }
        let default_preferred_direction_trace_cost = scoring
            .default_preferred_direction_trace_cost
            .expect("set above");
        let default_undesired_direction_trace_cost = scoring
            .default_undesired_direction_trace_cost
            .expect("set above");
        // :357-360 — the fallback for an absent per-layer bend cost, read **unclamped**, unlike
        // `RouterSettings.setBendCost` (:675-690) and `getBendCost` (:692-703), which both clamp
        // into `[MIN_BEND_COST, MAX_BEND_COST]`. `scoring` is non-null by now, so Java's
        // `scoring != null &&` half of the condition is always true.
        let default_bend_cost = scoring.default_bend_cost.unwrap_or(0.0);
        // Java writes `boardSpecificTraceCostsApplied = false` inside each of the two `if`s
        // (:328, :333); both writes are the same value, so one write after the pair is the same
        // state — and it has to be, since `scoring` is borrowed from `self` above.
        if costs_reallocated {
            self.board_specific_trace_costs_applied = Some(false);
        }

        // :345-346.
        let mut current_preferred_direction_is_horizontal = horizontal_width < vertical_width;
        let initialize_trace_costs = !self.are_board_specific_trace_costs_applied();

        // :348-374.
        let layer_structure = board.layer_structure();
        let layers = self.layers.as_mut().expect("allocated above");
        let scoring = self.scoring.as_mut().expect("instantiated above");
        let preferred_costs = scoring
            .preferred_direction_trace_cost
            .as_mut()
            .expect("allocated above");
        let undesired_costs = scoring
            .undesired_direction_trace_cost
            .as_mut()
            .expect("allocated above");
        for i in 0..layer_count {
            let is_signal = layer_structure.layers[i].is_signal;
            // :349-351 — only a signal layer advances the alternation, so a plane layer leaves
            // the next signal layer facing the same way this one does.
            if is_signal {
                current_preferred_direction_is_horizontal =
                    !current_preferred_direction_is_horizontal;
            }
            // :352-356.
            if !is_signal {
                layers[i].routable = Some(false);
            } else if layers[i].routable.is_none() {
                layers[i].routable = Some(true);
            }
            // :357-360.
            if layers[i].bend_cost.is_none() {
                layers[i].bend_cost = Some(default_bend_cost);
            }
            // :361-363 — not signal-gated: a plane layer inherits the running direction even
            // though it did not advance it.
            if layers[i].preferred_direction_horizontal.is_none() {
                layers[i].preferred_direction_horizontal =
                    Some(current_preferred_direction_is_horizontal);
            }
            // :365-373 — the penalty lands on the undesired entry only.
            if initialize_trace_costs {
                preferred_costs[i] = default_preferred_direction_trace_cost;
                undesired_costs[i] = default_undesired_direction_trace_cost;
                if current_preferred_direction_is_horizontal {
                    undesired_costs[i] += horizontal_add_costs_against_preferred_dir;
                } else {
                    undesired_costs[i] += vertical_add_costs_against_preferred_dir;
                }
            }
        }

        // :375-387 — increase costs on the outer layers.
        if initialize_trace_costs {
            let signal_layer_count = layer_structure.signal_layer_count();
            if signal_layer_count > 2 {
                let outer_add_costs = 0.2 * signal_layer_count as f64;
                preferred_costs[0] += outer_add_costs;
                preferred_costs[layer_count - 1] += outer_add_costs;
                undesired_costs[0] += outer_add_costs;
                undesired_costs[layer_count - 1] += outer_add_costs;
            }
            self.board_specific_trace_costs_applied = Some(true);
        }
    }
}
