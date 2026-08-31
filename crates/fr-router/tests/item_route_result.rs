//! Plan 7 Task 9 — `autoroute/ItemRouteResult.java` (145 lines), the optimizer's per-item
//! scorecard.
//!
//! Every literal below comes from `scripts/differential/java/probes/P7T9Probe.java` run against
//! the HEAD jar, committed verbatim as `tests/data/p7t9-item-route-result.txt` and replayed here.
//! The probe's class comment says how to regenerate it.
//!
//! # Floating point crosses the boundary as text
//!
//! The transcript carries `Float.toString`/`Double.toString` output and this file **parses** it
//! rather than formatting its own. Both languages' printers emit the shortest round-tripping
//! form and both parsers are correctly rounded, so a parsed comparison is exact — and a
//! `java_float_to_string` the port does not otherwise need never enters the test.

use std::cmp::Ordering;

use fr_board::ItemId;
use fr_router::pipeline::ItemRouteResult;

const TRANSCRIPT: &str = include_str!("data/p7t9-item-route-result.txt");

/// One `[tuples]` row: the seven constructor arguments and the four answers the JVM printed.
struct Tuple {
    k: usize,
    item_id: u32,
    via_count_before: i32,
    via_count_after: i32,
    trace_length_before: f64,
    trace_length_after: f64,
    incomplete_count_before: i32,
    incomplete_count_after: i32,
    improved: bool,
    improvement_percentage: f32,
    via_count_reduced: i32,
    length_reduced: f64,
}

impl Tuple {
    /// The `ItemRouteResult` this row's seven arguments construct.
    fn build(&self) -> ItemRouteResult {
        ItemRouteResult::new(
            ItemId(self.item_id),
            self.via_count_before,
            self.via_count_after,
            self.trace_length_before,
            self.trace_length_after,
            self.incomplete_count_before,
            self.incomplete_count_after,
        )
    }
}

/// The lines of one `[section]` of the transcript, comments and the section header dropped.
fn section(name: &str) -> Vec<&'static str> {
    let mut lines = Vec::new();
    let mut inside = false;
    for line in TRANSCRIPT.lines() {
        if line.starts_with('[') {
            inside = line == format!("[{name}]");
            continue;
        }
        if inside && !line.starts_with('#') && !line.is_empty() {
            lines.push(line);
        }
    }
    assert!(!lines.is_empty(), "section [{name}] is empty or missing");
    lines
}

fn tuples() -> Vec<Tuple> {
    section("tuples")
        .into_iter()
        .map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(f.len(), 13, "unexpected tuple row: {line}");
            assert_eq!(f[8], "->", "unexpected tuple row: {line}");
            Tuple {
                k: f[0].parse().expect("k"),
                item_id: f[1].parse().expect("itemId"),
                via_count_before: f[2].parse().expect("vcBefore"),
                via_count_after: f[3].parse().expect("vcAfter"),
                trace_length_before: f[4].parse().expect("tlBefore"),
                trace_length_after: f[5].parse().expect("tlAfter"),
                incomplete_count_before: f[6].parse().expect("icBefore"),
                incomplete_count_after: f[7].parse().expect("icAfter"),
                improved: f[9].parse().expect("improved"),
                improvement_percentage: f[10].parse().expect("improvementPercentage"),
                via_count_reduced: f[11].parse().expect("viaCountReduced"),
                length_reduced: f[12].parse().expect("lengthReduced"),
            }
        })
        .collect()
}

// =================================================================================================
// The ladder — ItemRouteResult.java:39-57
// =================================================================================================

/// Rung one (`:39-42`): fewer incompletes after wins, more loses, and neither rung below is
/// consulted — the vias and the trace length are set to *lose* in the improving case and to
/// *win* in the worsening one, so a port that ran the rungs in the wrong order would answer the
/// opposite of both assertions.
#[test]
fn the_first_rung_is_the_incomplete_count() {
    let better = ItemRouteResult::new(ItemId(1), 0, 9, 0.0, 9.0, 3, 2);
    assert!(better.improved(), "ItemRouteResult.java:39-40");

    let worse = ItemRouteResult::new(ItemId(1), 9, 0, 9.0, 0.0, 2, 3);
    assert!(!worse.improved(), "ItemRouteResult.java:41-42");
}

/// Rung two (`:44-47`), reached only when the incompletes tie (`:43`): fewer vias after wins.
/// The trace length is set to lose in the improving case, so rung three cannot be what answers.
#[test]
fn the_second_rung_is_the_via_count() {
    let better = ItemRouteResult::new(ItemId(1), 4, 2, 1.0, 9.0, 2, 2);
    assert!(better.improved(), "ItemRouteResult.java:44-45");

    let worse = ItemRouteResult::new(ItemId(1), 2, 4, 9.0, 1.0, 2, 2);
    assert!(!worse.improved(), "ItemRouteResult.java:46-47");
}

/// Rung three (`:49-55`), reached only when both counts tie (`:43`, `:48`): a shorter trace wins,
/// a longer one loses, and **an exact tie loses** — `:53-54`'s `else { improved = false; }`, which
/// is the one arm of the ladder that has no `<`/`>` behind it.
#[test]
fn the_third_rung_is_the_trace_length_and_a_tie_is_not_an_improvement() {
    let better = ItemRouteResult::new(ItemId(1), 3, 3, 10.0, 9.5, 2, 2);
    assert!(better.improved(), "ItemRouteResult.java:49-50");

    let worse = ItemRouteResult::new(ItemId(1), 3, 3, 9.5, 10.0, 2, 2);
    assert!(!worse.improved(), "ItemRouteResult.java:51-52");

    let tied = ItemRouteResult::new(ItemId(1), 3, 3, 10.0, 10.0, 2, 2);
    assert!(!tied.improved(), "ItemRouteResult.java:53-54");
}

// =================================================================================================
// improvementPercentage — ItemRouteResult.java:59-65 (quirk #212)
// =================================================================================================

/// The Java bug, with the value `BatchOptimizer.java:340-348` computes beside it.
///
/// `:63` writes `viaCountAfter / viaCountBefore` on two `int`s, so the via term is an **integer**
/// division that truncates towards zero before it is widened for the sum. With
/// `viaCountBefore = 4`, `viaCountAfter = 2` the via ratio is `0`, not `0.5`.
///
/// `BatchOptimizer.java:345` writes the same ratio as `(float) result.viaCount() / <denominator>`
/// and gets `0.5`. The two numbers are computed from different denominators there (the optimizer
/// divides by the *board's* via count, not by this result's `viaCountBefore`), so the second
/// assertion recomputes the expression with this result's own operands — the point being the
/// **cast**, not the operands.
#[test]
fn improvement_percentage_truncates_the_via_term() {
    let r = ItemRouteResult::new(ItemId(1), 4, 2, 100.0, 50.0, 1, 1);

    // Java: 1.0 - ((2/4) + (50.0/100.0)) / 2 = 1.0 - ((0) + 0.5) / 2 = 0.75.
    assert_eq!(
        r.improvement_percentage(),
        0.75_f32,
        "ItemRouteResult.java:59-65 with the integer division (quirk #212)"
    );

    // The same expression with `BatchOptimizer.java:345`'s `(float)` cast in front of the via
    // term: 1.0 - ((0.5) + 0.5) / 2 = 0.5. Half the ported value, on a case the corpus reaches.
    let corrected = 1.0_f64 - ((2.0_f64 / 4.0) + (50.0 / 100.0)) / 2.0;
    assert_eq!(corrected as f32, 0.5_f32, "BatchOptimizer.java:340-348");
    assert_ne!(r.improvement_percentage(), corrected as f32);
}

/// `:61`'s guard: **either** `viaCountBefore == 0` **or** `traceLengthBefore == 0` answers a flat
/// `0`, which is what keeps the integer division at `:63` from throwing.
#[test]
fn improvement_percentage_is_zero_when_either_denominator_is_zero() {
    let no_vias = ItemRouteResult::new(ItemId(1), 0, 3, 100.0, 50.0, 1, 1);
    assert_eq!(no_vias.improvement_percentage(), 0.0_f32, ":61");

    let no_length = ItemRouteResult::new(ItemId(1), 4, 2, 0.0, 50.0, 1, 1);
    assert_eq!(no_length.improvement_percentage(), 0.0_f32, ":61");

    let neither = ItemRouteResult::new(ItemId(1), 0, 0, 0.0, 0.0, 1, 1);
    assert_eq!(neither.improvement_percentage(), 0.0_f32, ":61");
}

// =================================================================================================
// The JVM transcript
// =================================================================================================

/// `:17-20` — the one-argument constructor, whose `this(itemId, 0, 0, 0, 0, 0, 1)` makes the
/// ladder answer `false` before `:19`'s redundant explicit assignment.
#[test]
fn the_unimproved_constructor_matches_the_jvm() {
    let line = section("unimproved-ctor")[0];
    let fields: std::collections::BTreeMap<&str, &str> = line
        .split_whitespace()
        .map(|f| f.split_once('=').expect("key=value"))
        .collect();

    let r = ItemRouteResult::unimproved(ItemId(fields["itemId"].parse().expect("itemId")));
    assert_eq!(r.item_id().0.to_string(), fields["itemId"]);
    assert_eq!(r.improved().to_string(), fields["improved"]);
    assert_eq!(
        r.improvement_percentage(),
        fields["improvementPercentage"].parse::<f32>().expect("f32")
    );
    assert_eq!(r.via_count().to_string(), fields["viaCount"]);
    assert_eq!(
        r.trace_length(),
        fields["traceLength"].parse::<f64>().expect("f64")
    );
    assert_eq!(r.incomplete_count().to_string(), fields["incompleteCount"]);
    assert_eq!(
        r.incomplete_count_before().to_string(),
        fields["incompleteCountBefore"]
    );
    assert_eq!(r.via_count_reduced().to_string(), fields["viaCountReduced"]);
    assert_eq!(
        r.length_reduced(),
        fields["lengthReduced"].parse::<f64>().expect("f64")
    );
}

/// `:137-139` — the only mutator on the class, and the only reason `improved` is not `final`.
#[test]
fn update_improved_matches_the_jvm() {
    let lines = section("update-improved");
    let mut r = ItemRouteResult::unimproved(ItemId(1));
    assert_eq!(format!("initial={}", r.improved()), lines[0]);
    r.update_improved(true);
    assert_eq!(format!("afterTrue={}", r.improved()), lines[1]);
    r.update_improved(false);
    assert_eq!(format!("afterFalse={}", r.improved()), lines[2]);
}

/// All 500 scripted tuples: the ladder's answer, `improvementPercentage`, `viaCountReduced` and
/// `lengthReduced`, each against the JVM's own printed value.
#[test]
fn every_scripted_tuple_matches_the_jvm() {
    let tuples = tuples();
    assert_eq!(tuples.len(), 500, "P7T9Probe writes 500 rows");
    for t in &tuples {
        let r = t.build();
        assert_eq!(r.improved(), t.improved, "improved(), tuple k={}", t.k);
        assert_eq!(
            r.improvement_percentage(),
            t.improvement_percentage,
            "improvementPercentage(), tuple k={}",
            t.k
        );
        assert_eq!(
            r.via_count_reduced(),
            t.via_count_reduced,
            "viaCountReduced(), tuple k={}",
            t.k
        );
        assert_eq!(
            r.length_reduced(),
            t.length_reduced,
            "lengthReduced(), tuple k={}",
            t.k
        );
        assert_eq!(r.item_id(), ItemId(t.item_id), "itemId(), tuple k={}", t.k);
        assert_eq!(r.via_count(), t.via_count_after, "viaCount(), k={}", t.k);
        assert_eq!(
            r.trace_length(),
            t.trace_length_after,
            "traceLength(), k={}",
            t.k
        );
        assert_eq!(
            r.incomplete_count(),
            t.incomplete_count_after,
            "incompleteCount(), k={}",
            t.k
        );
        assert_eq!(
            r.incomplete_count_before(),
            t.incomplete_count_before,
            "incompleteCountBefore(), k={}",
            t.k
        );
    }
}

/// `:92-94` — `improvedOver` is `compareTo(...) < 0`, asserted both ways round on every
/// consecutive pair together with the sign of `compareTo` itself.
#[test]
fn improved_over_matches_the_jvm() {
    let results: Vec<ItemRouteResult> = tuples().iter().map(Tuple::build).collect();
    for line in section("improved-over") {
        let f: Vec<&str> = line.split_whitespace().collect();
        let a: usize = f[0].parse().expect("a");
        let b: usize = f[1].parse().expect("b");
        assert_eq!(
            results[a].improved_over(&results[b]).to_string(),
            f[2],
            "improvedOver({a}, {b})"
        );
        assert_eq!(
            results[b].improved_over(&results[a]).to_string(),
            f[3],
            "improvedOver({b}, {a})"
        );
        let signum = match results[a].compare_to(&results[b]) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        };
        assert_eq!(signum.to_string(), f[4], "compareTo({a}, {b})");
    }
}

/// `compareTo` (`:69-89`) — the **ordering** it produces over the probe's 500 tuples, not a
/// per-pair value: the JVM sorted the keys `0..499` with `List.sort`, which is stable, and this
/// sorts the same tuples with `slice::sort_by`, which is stable too. A behavioural test, as the
/// brief asks.
///
/// **`compareTo` has no live caller.** Its only reader is the `PriorityQueue<ItemRouteResult>` in
/// the GUI-only multithreaded optimizer (`autoroute/pipeline/BatchAutorouterThread.java`), which
/// `RoutingPipeline` never constructs on the headless path; it is ported for the audit and pinned
/// here so a later plan that revives that path inherits a checked comparator.
#[test]
fn compare_to_matches_the_jvm() {
    let tuples = tuples();
    let results: Vec<ItemRouteResult> = tuples.iter().map(Tuple::build).collect();

    let mut order: Vec<usize> = (0..results.len()).collect();
    order.sort_by(|a, b| results[*a].compare_to(&results[*b]));

    let expected: Vec<usize> = section("sorted")[0]
        .split_whitespace()
        .map(|k| k.parse().expect("k"))
        .collect();
    assert_eq!(order, expected, "the JVM's key order after List.sort");
}
