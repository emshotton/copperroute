//! Plan 7 Task 7: `board.optimize.ViaOptimizer`'s three `repositionVia` overloads — A
//! (ViaOptimizer.java:302-365), B (:367-429) and C (:434-713), the 279-line one.
//!
//! # Where the numbers come from
//!
//! `scripts/differential/java/P7T4.java` grew three modes, one per overload, each driving the
//! private Java method through `setAccessible` over a board the `P6T1` machinery actually routed:
//!
//! * **mode 3** — overload A, over 27 scripted `toLocation`s per via (the contact trace's two inner
//!   corners, the via centre itself, and six offsets at each of four magnitudes);
//! * **mode 4** — overload B, over 11 `toLocation`s x the two role assignments overload C uses;
//! * **mode 5** — overload C, with the arguments `optViaLocation:118-131` builds and five
//!   `(horizontal, vertical)` cost-pair combinations, so every weighted comparison decides both
//!   ways somewhere in the run.
//!
//! Their stdout for `Issue143-rpi_splitter` and `Issue026-J2_reference` is committed as sections of
//! `tests/data/p7t4-via-optimizer.txt` and diffed live by `scripts/differential/run.sh p7t4`.
//! [`overload_a_matches_the_jvm_on_every_scripted_target`],
//! [`overload_b_matches_the_jvm_on_every_scripted_candidate`] and
//! [`overload_c_matches_the_jvm_on_every_cost_pair`] replay all six sections here.
//!
//! # The three overloads mutate nothing
//!
//! `checkTraceSegment` and `DrillItemMover.check` — the latter called with both recursion depths at
//! **zero**, so no shove is attempted — are read-only probes. That is why the driver may call an
//! overload 27 times per via and still end on the board the routing prologue built, and why each of
//! the three modes prints that board afterwards: an inserted item or a burned id would show as a
//! diff in the dump. [`the_general_case_leaves_the_board_untouched_when_no_candidate_improves`]
//! makes the same statement locally with `structural_hash`.
//!
//! The rest of the file isolates the dispatch and the candidate order.

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntPoint, Point, Polyline};
use fr_router::board_ext::ViaOptimizer;
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{ExpansionCostFactor, HostEnvironment, RouterSettings, SettingsSource};
use std::collections::{BTreeMap, BTreeSet};

fn never() -> bool {
    false
}

// =================================================================================================
// The `p7t4` transcript — modes 3, 4 and 5
// =================================================================================================

const TRANSCRIPT: &str = include_str!("data/p7t4-via-optimizer.txt");

fn transcript_section(name: &str) -> Vec<&'static str> {
    let header = format!("######## {name}");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in TRANSCRIPT.lines() {
        if line.starts_with("######## ") {
            inside = line == header;
            continue;
        }
        if inside {
            rows.push(line.trim_end());
        }
    }
    assert!(!rows.is_empty(), "transcript section {name} is empty");
    rows
}

fn fixture_of(tag: &str) -> &'static str {
    match tag {
        "rpi" => "Issue143-rpi_splitter.dsn",
        "j2" => "Issue026-J2_reference.dsn",
        other => panic!("unknown transcript tag {other}"),
    }
}

/// Every `repA` / `repB` / `repC` row the port produces for one `(fixture, mode)`, in the driver's
/// order. The routing prologue and the board dump are `p7t4`'s business and are checked live by
/// `run.sh`; what is replayed here is the overload's own answer.
fn overload_rows(tag: &str, mode: i32) -> Vec<String> {
    let mut board = routed(fixture_of(tag));
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    let mut out = Vec::new();
    for via_id in via_ids {
        match mode {
            3 => rows_a(&mut out, &mut board, via_id),
            4 => rows_b(&mut out, &mut board, via_id),
            5 => rows_c(&mut out, &mut board, via_id),
            other => panic!("mode {other} is not an overload mode"),
        }
    }
    out
}

/// `P7T4.driveOverloadA`.
fn rows_a(out: &mut Vec<String>, board: &mut Board, via_id: ItemId) {
    let traces = trace_contacts(board, via_id);
    let Some(trace) = traces.first().copied() else {
        out.push(format!("repA id={} state=NO_TRACE_CONTACT", via_id.0));
        return;
    };
    let center = center_of(board, via_id).to_float().round();
    let (hw, cl, layer) = trace_params(board, trace);
    let targets = targets_a(center, &polyline_of(board, trace));
    for (k, to) in targets.iter().enumerate() {
        let answer = ViaOptimizer::reposition_via_toward_location(board, via_id, to, hw, layer, cl);
        out.push(format!(
            "repA id={} k={k} to=({},{}) hw={hw} layer={layer} cl={cl} -> {}",
            via_id.0,
            to.x,
            to.y,
            answer.map_or_else(|| "null".to_string(), |p| dump_point(&p))
        ));
    }
}

/// `P7T4.driveOverloadB`.
fn rows_b(out: &mut Vec<String>, board: &mut Board, via_id: ItemId) {
    let traces = trace_contacts(board, via_id);
    let Some(t1) = traces.first().copied() else {
        out.push(format!("repB id={} state=NO_TRACE_CONTACT", via_id.0));
        return;
    };
    let t2 = traces.get(1).copied().unwrap_or(t1);
    let via_center = center_of(board, via_id);
    let tolerance = tolerance_of(board, via_id);
    let center = via_center.to_float().round();
    let c1 = from_corner_of(board, t1, &via_center, tolerance)
        .to_float()
        .round();
    let c2 = from_corner_of(board, t2, &via_center, tolerance)
        .to_float()
        .round();
    let params1 = trace_params(board, t1);
    let params2 = trace_params(board, t2);
    for (k, to) in targets_b(center, c1, c2).iter().enumerate() {
        for r in 0..2 {
            let (moved_hw, moved_cl, moved_layer) = if r == 0 { params2 } else { params1 };
            let (conn_hw, conn_cl, conn_layer) = if r == 0 { params1 } else { params2 };
            let connect = if r == 0 { c1 } else { c2 };
            let answer = ViaOptimizer::reposition_via_check_candidate(
                board,
                via_id,
                to,
                moved_hw,
                moved_layer,
                moved_cl,
                &connect,
                conn_hw,
                conn_layer,
                conn_cl,
            );
            out.push(format!(
                "repB id={} k={k} r={r} to=({},{}) connect=({},{}) -> {answer}",
                via_id.0, to.x, to.y, connect.x, connect.y
            ));
        }
    }
}

/// `P7T4.driveOverloadC`.
fn rows_c(out: &mut Vec<String>, board: &mut Board, via_id: ItemId) {
    let class = classify(board, via_id);
    if class != "TWO_TRACES" {
        out.push(format!("repC id={} class={class} state=SKIP", via_id.0));
        return;
    }
    let (t1, t2, c1, c2) = overload_c_arguments(board, via_id);
    let (hw1, cl1, layer1) = trace_params(board, t1);
    let (hw2, cl2, layer2) = trace_params(board, t2);
    for (k, pair) in COST_PAIRS.iter().enumerate() {
        let answer = ViaOptimizer::reposition_via_general(
            board,
            via_id,
            hw1,
            cl1,
            layer1,
            ExpansionCostFactor {
                horizontal: pair[0],
                vertical: pair[1],
            },
            &c1,
            hw2,
            cl2,
            layer2,
            ExpansionCostFactor {
                horizontal: pair[2],
                vertical: pair[3],
            },
            &c2,
        );
        out.push(format!(
            "repC id={} k={k} costs1=({},{}) costs2=({},{}) from1={} from2={} -> {}",
            via_id.0,
            fr_dsn::java_double_to_string(pair[0]),
            fr_dsn::java_double_to_string(pair[1]),
            fr_dsn::java_double_to_string(pair[2]),
            fr_dsn::java_double_to_string(pair[3]),
            dump_point(&c1),
            dump_point(&c2),
            answer.map_or_else(|| "null".to_string(), |p| dump_point(&p))
        ));
    }
}

fn transcript_overload_rows(tag: &str, mode: i32, prefix: &str) -> Vec<String> {
    transcript_section(&format!("{tag} mode {mode}"))
        .into_iter()
        .filter(|row| row.starts_with(prefix))
        .map(str::to_string)
        .collect()
}

/// Overload A (`:302-365`) over `p7t4` mode 3's scripted targets, on both fixtures with vias.
///
/// The family reaches every branch: `k = 2` is the via centre, which is `:312-314`'s
/// `fromLocation.equals(toLocation)`; the 100 000-unit offsets run off the board and hit
/// `:325-327`'s `okLength <= 0`; the two inner corners are the arguments
/// `optPlaneOrFanoutVia:216-217` really passes and answer through `:331`'s `>= Integer.MAX_VALUE`
/// branch; and the intermediate magnitudes fall into the `:353-363` halving loop.
#[test]
fn overload_a_matches_the_jvm_on_every_scripted_target() {
    for tag in ["rpi", "j2"] {
        assert_eq!(
            overload_rows(tag, 3),
            transcript_overload_rows(tag, 3, "repA "),
            "p7t4 {tag} mode 3"
        );
    }
}

/// Overload B (`:367-429`) over `p7t4` mode 4's scripted candidates. `k = 6` is the via centre
/// (`:381-384`) and `k = 7`/`k = 8` are moves of length 1 and `sqrt(2)`, both inside `:388-397`'s
/// `lengthApprox() <= 1.5` refusal — which only fires under `AngleRestriction.NONE`, so on these
/// two `FORTYFIVE_DEGREE` boards the guard is passed over and the `checkTraceSegment` pair decides.
#[test]
fn overload_b_matches_the_jvm_on_every_scripted_candidate() {
    for tag in ["rpi", "j2"] {
        assert_eq!(
            overload_rows(tag, 4),
            transcript_overload_rows(tag, 4, "repB "),
            "p7t4 {tag} mode 4"
        );
    }
}

/// Overload C (`:434-713`) over `p7t4` mode 5's five cost pairs. `rpi`'s section is all `SKIP`
/// rows — none of its vias classifies `TWO_TRACES` after the 12-connection prefix, so the dispatch
/// replica is pinned too — and `j2`'s six vias x five pairs are the real evidence.
#[test]
fn overload_c_matches_the_jvm_on_every_cost_pair() {
    for tag in ["rpi", "j2"] {
        assert_eq!(
            overload_rows(tag, 5),
            transcript_overload_rows(tag, 5, "repC "),
            "p7t4 {tag} mode 5"
        );
    }
}

// =================================================================================================
// The dispatch: which overload each shape of via reaches
// =================================================================================================

/// `optViaLocation:47` and `:76-78` — a via with **one** contact skips `firstTrace`/`secondTrace`
/// entirely and goes to `optPlaneOrFanoutVia`, whose `:216-217` is **overload A**.
///
/// *(The plan's first draft called overload A "the two-contact case" and named the port
/// `reposition_via_two_contacts`; Task 6 corrected both. Overload A has exactly one Java caller,
/// the one-contact arm, and overload C is the two-trace one.)*
///
/// Pinned on `Issue143-rpi_splitter`'s two one-contact vias: overload A, called directly with the
/// arguments `:212-217` builds, answers the location the arm then moves the via to — via 187's
/// `(932812,1011224)` is `opt_plane_or_fanout_via`'s final answer as well, while via 84's
/// `(1016000,3007058)` **is** the check corner, so `:292-294` recurses once and the via ends at
/// `(1016000,3119161)` (`p7t4` rpi modes 0 and 1).
#[test]
fn a_one_contact_via_takes_overload_a() {
    let board = routed("Issue143-rpi_splitter.dsn");
    for (via_id, overload_a_answer, final_center) in [
        (
            ItemId(187),
            IntPoint::new(932_812, 1_011_224),
            IntPoint::new(932_812, 1_011_224),
        ),
        (
            ItemId(84),
            IntPoint::new(1_016_000, 3_007_058),
            IntPoint::new(1_016_000, 3_119_161),
        ),
    ] {
        assert_eq!(
            board.normal_contacts(via_id).len(),
            1,
            "via {} is the one-contact shape",
            via_id.0
        );

        // `optPlaneOrFanoutVia:205-217`, replicated: the check corner, then overload A.
        let mut scratch = board.clone();
        let trace = trace_contacts(&scratch, via_id)[0];
        let via_center = center_of(&scratch, via_id);
        let tolerance = tolerance_of(&scratch, via_id);
        let check_corner = from_corner_of(&scratch, trace, &via_center, tolerance);
        let (hw, cl, layer) = trace_params(&scratch, trace);
        let before = scratch.structural_hash();
        let direct = ViaOptimizer::reposition_via_toward_location(
            &mut scratch,
            via_id,
            &check_corner.to_float().round(),
            hw,
            layer,
            cl,
        );
        assert_eq!(
            direct,
            Some(Point::Int(overload_a_answer)),
            "overload A on via {}",
            via_id.0
        );
        assert_eq!(
            before,
            scratch.structural_hash(),
            "overload A mutates nothing"
        );

        // And `:76-78` really routes the via there: both entry points end on the same centre.
        for label in ["opt_via_location", "opt_plane_or_fanout_via"] {
            let mut scratch = board.clone();
            let moved = if label == "opt_via_location" {
                ViaOptimizer::opt_via_location(&mut scratch, via_id, None, 500, 10)
            } else {
                ViaOptimizer::opt_plane_or_fanout_via(&mut scratch, via_id, 500, 10)
            }
            .expect("cannot fail");
            assert!(moved, "{label} on via {}", via_id.0);
            assert_eq!(
                scratch.drill_center(via_id).expect("still a via"),
                Point::Int(final_center),
                "{label} on via {}",
                via_id.0
            );
        }
    }
}

/// `optViaLocation:118-131` — a via with **two** free trace contacts, each at an endpoint, reaches
/// **overload C**, and the twelve arguments are the two traces' half width, clearance class, layer,
/// per-layer cost factor and from-corner, in that order per trace.
///
/// Pinned on `Issue026-J2_reference`'s via 264, the first of the walk (so the board is the one the
/// routing prologue left): overload C, called directly, answers `(1316044,-867516)`, and
/// `opt_via_location` moves the via exactly there.
#[test]
fn a_two_trace_via_takes_overload_c() {
    let board = routed("Issue026-J2_reference.dsn");
    let via_id = ItemId(264);
    assert_eq!(classify(&board, via_id), "TWO_TRACES");

    let mut scratch = board.clone();
    let (t1, t2, c1, c2) = overload_c_arguments(&scratch, via_id);
    let (hw1, cl1, layer1) = trace_params(&scratch, t1);
    let (hw2, cl2, layer2) = trace_params(&scratch, t2);
    let unit = ExpansionCostFactor {
        horizontal: 1.0,
        vertical: 1.0,
    };
    let before = scratch.structural_hash();
    let direct = ViaOptimizer::reposition_via_general(
        &mut scratch,
        via_id,
        hw1,
        cl1,
        layer1,
        unit,
        &c1,
        hw2,
        cl2,
        layer2,
        unit,
        &c2,
    );
    assert_eq!(
        direct,
        Some(Point::Int(IntPoint::new(1_316_044, -867_516))),
        "overload C on via 264"
    );
    assert_eq!(
        before,
        scratch.structural_hash(),
        "overload C mutates nothing"
    );

    let mut scratch = board.clone();
    assert!(
        ViaOptimizer::opt_via_location(&mut scratch, via_id, None, 500, 10).expect("cannot fail"),
        ":118-131 fed the move"
    );
    assert_eq!(
        scratch.drill_center(via_id).expect("still a via"),
        Point::Int(IntPoint::new(1_316_044, -867_516)),
        "optViaLocation moved the via to overload C's answer"
    );
}

/// **Overload B has no caller outside overload C.** In Java it is `private` and the only four
/// invocations are the axis-parallel decomposition arms at `:599`, `:627`, `:665` and `:696`; the
/// port must not have grown a fifth. Asserted on the source, because a call graph is not something
/// a board fixture can show.
#[test]
fn overload_b_is_reached_only_from_c() {
    const SOURCE: &str = include_str!("../src/board_ext/via_optimizer.rs");
    let calls = SOURCE
        .matches("Self::reposition_via_check_candidate(")
        .count();
    assert_eq!(
        calls, 4,
        "overload B is called exactly four times, all from overload C's decomposition arms"
    );
    // Overload A is the other way round: one Java caller (`optPlaneOrFanoutVia:216-217`) plus the
    // seven calls overload C makes into it (:467, :475, :495, :517, :558, :562, :568, :572 — eight
    // call expressions, of which the first two are the collinear arm's either/or).
    let a_calls = SOURCE
        .matches("Self::reposition_via_toward_location(")
        .count();
    assert_eq!(
        a_calls, 9,
        "overload A is called once from the plane/fanout arm and eight times from overload C"
    );
}

// =================================================================================================
// The candidate order, and the fact that nothing moves
// =================================================================================================

/// **Java wins over the plan's transcription note.** The note said "where two candidates tie, Java
/// keeps the **first** found in contact order; the port must not use a `max_by` that keeps the
/// last". There is no `max_by` and no scoring pass anywhere in overload C: it is a sequence of
/// **gated attempts**, each of which returns the moment it succeeds. So the note's requirement
/// splits into two facts, both pinned here:
///
/// 1. **A tie is not a candidate at all.** Every gate is a strict `>` (`:492`, `:514`, `:555`,
///    `:597`, `:625`, `:663`, `:693`), so equal weighted distances skip the arm. `j2`'s via 231
///    shows it end to end: with `costs1 == costs2` overload C answers `null`, and with the very
///    same geometry under `(1.0, 2.0)` / `(2.0, 1.0)` it answers `(1228467,-826441)` and under
///    `(2.0, 1.0)` / `(1.0, 2.0)` it answers `(1231088,-829062)` — three different answers from one
///    board, decided only by the gates. The two `:492`/`:514` gate expressions are recomputed here
///    from [`FloatPoint::weighted_distance`] and asserted **equal**, which is what makes the
///    strictness observable rather than inferred.
/// 2. **The first success returns.** Via 130's five cost pairs all answer `(1337405,-857248)`,
///    which is the *second* from-corner: the `:462-480` overlapping-lines arm runs before every
///    cost gate and returns whatever it finds, cost factors included or not.
#[test]
fn a_candidate_tie_keeps_the_first_in_contact_order() {
    let board = routed("Issue026-J2_reference.dsn");

    // (1) The tie, on via 231.
    let via_id = ItemId(231);
    let (t1, t2, c1, c2) = overload_c_arguments(&board, via_id);
    let via_center = center_of(&board, via_id).to_float();
    let unit = ExpansionCostFactor {
        horizontal: 1.0,
        vertical: 1.0,
    };
    // `:485-490` and `:506-512` under equal costs.
    for corner in [&c1, &c2] {
        let float_corner = corner.to_float();
        let a = via_center.weighted_distance(&float_corner, unit.horizontal, unit.vertical);
        let b = via_center.weighted_distance(&float_corner, unit.horizontal, unit.vertical);
        assert_eq!(a, b, "equal costs make the gate a tie");
        // The gate is `currentWeightedDistance1 > currentWeightedDistance2`; `Greater` is the only
        // ordering that opens it, and equality is not it.
        assert_ne!(
            a.partial_cmp(&b),
            Some(std::cmp::Ordering::Greater),
            "Java's `>` is strict, so the arm is skipped"
        );
    }
    let answers: Vec<Option<Point>> = COST_PAIRS
        .iter()
        .map(|pair| {
            let mut scratch = board.clone();
            ViaOptimizer::reposition_via_general(
                &mut scratch,
                via_id,
                half_width_of(&board, t1),
                clearance_of(&board, t1),
                layer_of(&board, t1),
                ExpansionCostFactor {
                    horizontal: pair[0],
                    vertical: pair[1],
                },
                &c1,
                half_width_of(&board, t2),
                clearance_of(&board, t2),
                layer_of(&board, t2),
                ExpansionCostFactor {
                    horizontal: pair[2],
                    vertical: pair[3],
                },
                &c2,
            )
        })
        .collect();
    assert_eq!(answers[0], None, "the tie pair reaches no candidate");
    assert_eq!(
        answers[1],
        Some(Point::Int(IntPoint::new(1_228_467, -826_441))),
        "(1,2)/(2,1) opens the :514 arm"
    );
    assert_eq!(
        answers[2],
        Some(Point::Int(IntPoint::new(1_231_088, -829_062))),
        "(2,1)/(1,2) opens the :492 arm"
    );

    // (2) The first success returns: via 130's collinear arm ignores the costs entirely.
    let via_id = ItemId(130);
    let (t1, t2, c1, c2) = overload_c_arguments(&board, via_id);
    for pair in &COST_PAIRS {
        let mut scratch = board.clone();
        let answer = ViaOptimizer::reposition_via_general(
            &mut scratch,
            via_id,
            half_width_of(&board, t1),
            clearance_of(&board, t1),
            layer_of(&board, t1),
            ExpansionCostFactor {
                horizontal: pair[0],
                vertical: pair[1],
            },
            &c1,
            half_width_of(&board, t2),
            clearance_of(&board, t2),
            layer_of(&board, t2),
            ExpansionCostFactor {
                horizontal: pair[2],
                vertical: pair[3],
            },
            &c2,
        );
        assert_eq!(
            answer,
            Some(Point::Int(IntPoint::new(1_337_405, -857_248))),
            "via 130 answers the second from-corner whatever the costs are"
        );
        assert_eq!(
            answer,
            Some(Point::Int(c2.to_float().round())),
            "and that answer is the nearer from-corner, :466-474"
        );
    }
}

/// `:712` — overload C's fall-through. Every gate can be open and every probe can still refuse, and
/// the method then answers `null` **having changed nothing**; `optViaLocation:132-134` turns that
/// into `return false`, which is why Task 6 could stub it inertly.
///
/// `j2`'s via 124 is the case: all five cost pairs answer `null`. The board-untouched half is
/// asserted for **every** via and **every** pair, successes included — overload C never mutates.
#[test]
fn the_general_case_leaves_the_board_untouched_when_no_candidate_improves() {
    let mut board = routed("Issue026-J2_reference.dsn");
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    let mut refusals = 0;
    let mut successes = 0;
    for via_id in via_ids {
        if classify(&board, via_id) != "TWO_TRACES" {
            continue;
        }
        let (t1, t2, c1, c2) = overload_c_arguments(&board, via_id);
        let (hw1, cl1, layer1) = trace_params(&board, t1);
        let (hw2, cl2, layer2) = trace_params(&board, t2);
        for pair in &COST_PAIRS {
            let before = board.structural_hash();
            let answer = ViaOptimizer::reposition_via_general(
                &mut board,
                via_id,
                hw1,
                cl1,
                layer1,
                ExpansionCostFactor {
                    horizontal: pair[0],
                    vertical: pair[1],
                },
                &c1,
                hw2,
                cl2,
                layer2,
                ExpansionCostFactor {
                    horizontal: pair[2],
                    vertical: pair[3],
                },
                &c2,
            );
            assert_eq!(
                before,
                board.structural_hash(),
                "overload C mutated the board at via {}",
                via_id.0
            );
            if answer.is_none() {
                refusals += 1;
            } else {
                successes += 1;
            }
            if via_id == ItemId(124) {
                assert_eq!(answer, None, "via 124 refuses every cost pair");
            }
        }
    }
    assert!(refusals > 0, "the fall-through at :712 is exercised");
    assert!(successes > 0, "and so is the success path");
}

// =================================================================================================
// `P7T4.java`'s scripted families and accessors
// =================================================================================================

/// `P7T4.COST_PAIRS`.
const COST_PAIRS: [[f64; 4]; 5] = [
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 2.0, 2.0, 1.0],
    [2.0, 1.0, 1.0, 2.0],
    [1.0, 1.0, 2.0, 2.0],
    [3.0, 1.0, 1.0, 3.0],
];

/// `P7T4.targetsA`.
fn targets_a(center: IntPoint, polyline: &Polyline) -> Vec<IntPoint> {
    let mut result = vec![
        polyline.corner(1).expect("two corners").to_float().round(),
        polyline
            .corner(polyline.corner_count() - 2)
            .expect("two corners")
            .to_float()
            .round(),
        center,
    ];
    for d in [1, 1000, 10_000, 100_000] {
        result.push(IntPoint::new(center.x + d, center.y));
        result.push(IntPoint::new(center.x, center.y + d));
        result.push(IntPoint::new(center.x + d, center.y + d));
        result.push(IntPoint::new(center.x - d, center.y));
        result.push(IntPoint::new(center.x, center.y - d));
        result.push(IntPoint::new(center.x - d, center.y - d));
    }
    result
}

/// `P7T4.targetsB`.
fn targets_b(center: IntPoint, c1: IntPoint, c2: IntPoint) -> Vec<IntPoint> {
    vec![
        IntPoint::new(center.x, c1.y),
        IntPoint::new(c1.x, center.y),
        IntPoint::new(center.x, c2.y),
        IntPoint::new(c2.x, center.y),
        c1,
        c2,
        center,
        IntPoint::new(center.x + 1, center.y),
        IntPoint::new(center.x + 1, center.y + 1),
        IntPoint::new(center.x + 1000, center.y + 1000),
        IntPoint::new(center.x - 1000, center.y),
    ]
}

/// The twelve arguments `optViaLocation:118-131` builds, as `(firstTrace, secondTrace,
/// firstTraceFromCorner, secondTraceFromCorner)`.
fn overload_c_arguments(board: &Board, via: ItemId) -> (ItemId, ItemId, Point, Point) {
    let traces = trace_contacts(board, via);
    let via_center = center_of(board, via);
    let tolerance = tolerance_of(board, via);
    let c1 = from_corner_of(board, traces[0], &via_center, tolerance);
    let c2 = from_corner_of(board, traces[1], &via_center, tolerance);
    (traces[0], traces[1], c1, c2)
}

/// `(halfWidth, clearanceClassIndex, layer)` of a trace, read before the board is borrowed
/// mutably by the overload under test.
fn trace_params(board: &Board, trace: ItemId) -> (i32, usize, usize) {
    (
        half_width_of(board, trace),
        clearance_of(board, trace),
        layer_of(board, trace),
    )
}

fn trace_contacts(board: &Board, via: ItemId) -> Vec<ItemId> {
    board
        .normal_contacts(via)
        .into_iter()
        .rev()
        .filter(|id| board.get_item(*id).is_some_and(Item::is_trace))
        .collect()
}

fn polyline_of(board: &Board, trace: ItemId) -> Polyline {
    match board.get_item(trace) {
        Some(Item::Trace(t)) => t.polyline().clone(),
        _ => panic!("a trace contact"),
    }
}

fn half_width_of(board: &Board, trace: ItemId) -> i32 {
    match board.get_item(trace) {
        Some(Item::Trace(t)) => t.get_half_width(),
        _ => panic!("a trace contact"),
    }
}

fn layer_of(board: &Board, trace: ItemId) -> usize {
    match board.get_item(trace) {
        Some(Item::Trace(t)) => t.get_layer(),
        _ => panic!("a trace contact"),
    }
}

fn clearance_of(board: &Board, trace: ItemId) -> usize {
    board
        .get_item(trace)
        .expect("a trace contact")
        .clearance_class()
}

fn center_of(board: &Board, via: ItemId) -> Point {
    board.drill_center(via).expect("a via has a centre")
}

/// `optViaLocation:87` / `optPlaneOrFanoutVia:194`.
fn tolerance_of(board: &Board, via: ItemId) -> i32 {
    let min_width = match board.get_item(via) {
        Some(Item::Via(v)) => v.min_width(&board.ctx()),
        _ => panic!("a via"),
    };
    (min_width / 2.0) as i32 + 1
}

/// `P7T4.fromCornerOf` — `optViaLocation:89-96`, falling back to `corner(1)`.
fn from_corner_of(board: &Board, trace: ItemId, via_center: &Point, tolerance: i32) -> Point {
    let polyline = polyline_of(board, trace);
    let (first, last) = match board.get_item(trace) {
        Some(Item::Trace(t)) => (t.first_corner(), t.last_corner()),
        _ => panic!("a trace contact"),
    };
    if ViaOptimizer::is_within_tolerance(first.as_ref(), via_center, tolerance) {
        return polyline.corner(1).expect("two corners");
    }
    if ViaOptimizer::is_within_tolerance(last.as_ref(), via_center, tolerance) {
        return polyline
            .corner(polyline.corner_count() - 2)
            .expect("two corners");
    }
    polyline.corner(1).expect("two corners")
}

/// `P7T4.classify` — the read-only replica of `optViaLocation:39-106`.
fn classify(board: &Board, via: ItemId) -> &'static str {
    let item = board.get_item(via).expect("a via");
    if item.is_shove_fixed(&board.rules) {
        return "SHOVE_FIXED";
    }
    let contacts: Vec<ItemId> = board.normal_contacts(via).into_iter().rev().collect();
    if contacts.len() == 1 {
        return "PLANE_OR_FANOUT_ONE_CONTACT";
    }
    if contacts.len() != 2 {
        return "WRONG_CONTACT_COUNT";
    }
    let mut traces: Vec<ItemId> = Vec::new();
    let mut is_plane_or_fanout_via = false;
    for contact in contacts {
        let Some(contact_item) = board.get_item(contact) else {
            return "UNUSABLE_CONTACT";
        };
        if contact_item.is_shove_fixed(&board.rules) || !contact_item.is_trace() {
            if matches!(contact_item, Item::ConductionArea(_)) {
                is_plane_or_fanout_via = true;
            } else {
                return "UNUSABLE_CONTACT";
            }
        } else {
            traces.push(contact);
        }
    }
    if is_plane_or_fanout_via {
        return "PLANE_OR_FANOUT_CONDUCTION";
    }
    let via_center = center_of(board, via);
    let tolerance = tolerance_of(board, via);
    for trace in traces {
        let Some(Item::Trace(t)) = board.get_item(trace) else {
            return "UNUSABLE_CONTACT";
        };
        let (first, last) = (t.first_corner(), t.last_corner());
        if !ViaOptimizer::is_within_tolerance(first.as_ref(), &via_center, tolerance)
            && !ViaOptimizer::is_within_tolerance(last.as_ref(), &via_center, tolerance)
        {
            return "NOT_AT_ENDPOINT";
        }
    }
    "TWO_TRACES"
}

fn dump_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

// =================================================================================================
// `P6T1.java`'s choices, as `via_optimizer.rs` transcribes them
// =================================================================================================

fn routed(design_name: &str) -> Board {
    let path = parity::fixture(design_name);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let mut board =
        match fr_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => {
                *board.expect("the fixture produces a board")
            }
            other => panic!("{design_name} did not read: {other:?}"),
        };
    let settings = build_settings(&board);
    for (item_id, net_no) in pick_connections(&board, 12) {
        if board.get_item(item_id).is_none() {
            continue;
        }
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let trace_costs = settings.get_trace_costs();
        let mut engine = None;
        route_connection(
            &mut board,
            &mut engine,
            item_id,
            net_no,
            &settings,
            &trace_costs,
            &mut ripped,
            &mut ripup_costs,
            1,
            settings.get_start_ripup_costs(),
            !settings.is_fanout_enabled(),
            false,
            &never,
        );
    }
    board
}

fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

fn pick_connections(board: &Board, max_items: usize) -> Vec<(ItemId, i32)> {
    let mut result = Vec::new();
    for item_id in board.items_in_board_order() {
        let Some(item) = board.get_item(item_id) else {
            continue;
        };
        if item.as_connectable().is_none() {
            continue;
        }
        for net_no in item.net_nos().to_vec() {
            if board.unconnected_set(item_id, net_no).is_empty() {
                continue;
            }
            result.push((item_id, net_no));
            if result.len() >= max_items {
                return result;
            }
        }
    }
    result
}
