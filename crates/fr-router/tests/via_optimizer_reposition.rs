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

/// **Board item ids are no longer comparable across this transcript; everything else still is.**
/// `accepted at plan9-t7t8 (ruling CC)`.
///
/// Task 8's door-set and id fixes (#163, #171, #165b) make the port's routing prologue mint more
/// board items than the jar's on two of the three fixtures, so the ids the jar recorded name
/// different items in the port's board. Measured at the accept wave: `Issue649-kicad_ecc83`'s
/// sections are byte-identical to the jar, `Issue143-rpi_splitter`'s ids run up to 2 higher and
/// `Issue026-J2_reference`'s up to 12, and the shift is **not** uniform — it starts partway
/// through the routing prologue, so low ids keep their values and high ones do not.
///
/// The answer here is not a fitted offset. It is to compare every row with its ids **blanked**,
/// which is exact on everything the overloads are actually about — the scripted targets and
/// candidates, the cost pairs, the from-corners, the layers, the half widths, the clearance
/// classes and every returned point — and then to require the ids themselves to be **one
/// consistent renaming**: the jar's id `a` must map to the same port id everywhere in the section,
/// and no two jar ids may map to one port id.
///
/// That is stronger than a pasted table and much stronger than ignoring the ids:
///
/// * any byte outside an id token moving fails, in either direction;
/// * a row appearing, vanishing or changing order fails, because the blanked comparison is a
///   whole-sequence equality;
/// * the same jar id resolving to two different port ids fails, and two jar ids collapsing onto
///   one port id fails — so a via genuinely swapping places with another is caught even though
///   its number is not pinned.
///
/// What is deliberately given up is the id VALUES, and the reason is that they are the one thing
/// this fixture pair can no longer say anything true about. `via_optimizer.rs` carries the
/// retirement record for the three sections where even a renaming cannot reconcile the two boards.
const ID_KEYS: [&str; 7] = [
    "via id=",
    "item id=",
    "maxId=",
    "repA id=",
    "repB id=",
    "repC id=",
    "contacts=[",
];

/// Splits `line` into its id values, in order, and the line with each of them replaced by `#`.
///
/// An id is a digit run introduced by one of [`ID_KEYS`]; inside a `contacts=[a:Type,b:Type]` list
/// the scan continues over the commas, so both contacts are ids. Coordinates, cost factors, half
/// widths, clearance classes and layer numbers are never introduced by a key and are left alone.
fn split_ids(line: &str) -> (Vec<i64>, String) {
    let mut ids = Vec::new();
    let mut blanked = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let Some((at, key_len)) = ID_KEYS
            .iter()
            .filter_map(|key| rest.find(key).map(|at| (at, key.len())))
            .min_by_key(|(at, _)| *at)
        else {
            break;
        };
        let head = at + key_len;
        blanked.push_str(&rest[..head]);
        rest = &rest[head..];
        loop {
            let digits = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let Ok(id) = rest[..digits].parse::<i64>() else {
                break;
            };
            ids.push(id);
            blanked.push('#');
            rest = &rest[digits..];
            // Step over `:Type,` to reach the next id of a contact list; stop at anything else.
            let Some(comma) = rest.find(',') else { break };
            if rest[..comma].contains(']') || rest[..comma].contains(' ') {
                break;
            }
            blanked.push_str(&rest[..=comma]);
            rest = &rest[comma + 1..];
        }
    }
    blanked.push_str(rest);
    (ids, blanked)
}

/// The comparison described on [`ID_KEYS`]: every row equal once ids are blanked, and the ids
/// themselves one consistent renaming.
fn assert_rows_match_up_to_one_renaming(label: &str, ours: &[String], theirs: &[String]) {
    let blank = |rows: &[String]| -> (Vec<Vec<i64>>, Vec<String>) {
        rows.iter().map(|row| split_ids(row)).unzip()
    };
    let (our_ids, our_blanked) = blank(ours);
    let (their_ids, their_blanked) = blank(theirs);
    assert_eq!(
        our_blanked, their_blanked,
        "{label}: a difference outside the board item ids"
    );

    let mut forward: BTreeMap<i64, i64> = BTreeMap::new();
    let mut backward: BTreeMap<i64, i64> = BTreeMap::new();
    for (row, (theirs_row, ours_row)) in their_ids.iter().zip(&our_ids).enumerate() {
        assert_eq!(theirs_row.len(), ours_row.len(), "{label} row {row}");
        for (&jar, &port) in theirs_row.iter().zip(ours_row) {
            if let Some(&seen) = forward.get(&jar) {
                assert_eq!(
                    seen, port,
                    "{label} row {row}: the jar's id {jar} is the port's {seen} elsewhere and \
                     {port} here — not one renaming"
                );
            }
            if let Some(&seen) = backward.get(&port) {
                assert_eq!(
                    seen, jar,
                    "{label} row {row}: the port's id {port} stands for the jar's {seen} \
                     elsewhere and {jar} here — not one renaming"
                );
            }
            forward.insert(jar, port);
            backward.insert(port, jar);
        }
    }
    assert!(
        !forward.is_empty(),
        "{label}: no id was compared, so the renaming check did nothing"
    );
}

/// The transcript comparison every overload test runs.
fn assert_overload_rows(tag: &str, mode: i32, prefix: &str) {
    assert_rows_match_up_to_one_renaming(
        &format!("p7t4 {tag} mode {mode}"),
        &overload_rows(tag, mode),
        &transcript_overload_rows(tag, mode, prefix),
    );
}

// =================================================================================================
// RETIREMENT RECORD — the `Issue026-J2_reference` arm of the p7t4 family (ruling CC)
// =================================================================================================
//
// **What retired.** Every `j2`-keyed assertion in this file: the `j2` arm of the three
// `overload_*_matches_the_jvm_*` tests, and the two tests that were `j2`-only,
// `a_two_trace_via_takes_overload_c` (via 264) and `a_candidate_tie_keeps_the_first_in_contact_order`
// (vias 231 and 130). Nothing is silently deleted; this block is the record ruling CC requires.
//
// **The last matching run.** `cf4c6f3`, the commit immediately before Task 8's merge `57055c1`.
// Re-run at the accept wave in a worktree at that commit: `cargo nextest run -p fr-router
// --test via_optimizer --test via_optimizer_reposition` — **18 passed, 0 failed**. Everything
// below was green there and is green there still.
//
// **The diverging cause.** `tests/data/p7t4-via-optimizer.txt` is a **jar** transcript over a board
// the **port** routes: `routed("Issue026-J2_reference.dsn")` calls `route_connection` twelve times
// before the first via is examined. Task 8's fixes change what those twelve calls produce, so the
// jar's transcript describes a board this port no longer builds. On `Issue026-J2_reference` the
// difference is not only identity — measured at the wave, the port's `item id` for one net-10
// trace carries corners `(1178685,-829062) (1228467,-829062)` where the jar's carries
// `(1178686,-829063) (1228466,-829063)`, one integer unit apart on two corners. No renaming of ids
// and no offset can reconcile that, which is exactly ruling CC's argument.
//
// **Why the other two fixtures did NOT retire, measured the same day.**
// `Issue649-kicad_ecc83`'s sections are byte-identical to the jar. `Issue143-rpi_splitter`'s
// differ **only** in board item ids, so its assertions survive under
// `assert_rows_match_up_to_one_renaming` above — every cost pair, from-corner, layer, half width,
// clearance class and returned point still compared against the jar, exactly.
//
// **The diverging cause rows.** #163, #171 and #165b (the door-set fixes) and #156/#167/#158 (one
// shared id counter), all `fixed: T8`.
//
// **The authorizing rows.** Ruling CC(b) — the plan9-t7t8 bench adjudicated Task 8's BP12
// escalation as variance and opened this accept wave; Task 8 §6a, which raised the family as a
// design question rather than a re-cut; Task 8 §6's note that inventing a port lane for the
// unit-level jar transcripts is a controller ruling, not a task decision.
//
// **What still covers the subject.** Overloads A, B and C are all still pinned against the jar
// row for row on `Issue143-rpi_splitter` (modes 3, 4 and 5), and overload A is pinned twice more
// by `a_one_contact_via_takes_overload_a`. What is lost with the `j2` arm is the two-trace
// evidence for overload C's own arithmetic: `rpi`'s mode-5 section is all `SKIP` rows, so
// **overload C's cost gates are no longer compared with the jar's on any fixture**. The DISPATCH
// to overload C is still pinned — `rpi` classifies four vias `TWO_TRACES` in mode 0, and
// `via_optimizer.rs`'s count assertion holds that number — so it is the arithmetic inside
// `:434-713`, not the route into it, that this retirement costs. Reported to the controller as
// such rather than left implicit in a row count.

/// Overload A (`:302-365`) over `p7t4` mode 3's scripted targets, on the one fixture with vias
/// whose board the port still builds the jar's way — see the retirement record above for the
/// `j2` arm.
///
/// The family reaches every branch: `k = 2` is the via centre, which is `:312-314`'s
/// `fromLocation.equals(toLocation)`; the 100 000-unit offsets run off the board and hit
/// `:325-327`'s `okLength <= 0`; the two inner corners are the arguments
/// `optPlaneOrFanoutVia:216-217` really passes and answer through `:331`'s `>= Integer.MAX_VALUE`
/// branch; and the intermediate magnitudes fall into the `:353-363` halving loop.
#[test]
fn overload_a_matches_the_jvm_on_every_scripted_target() {
    assert_overload_rows("rpi", 3, "repA ");
}

/// Overload B (`:367-429`) over `p7t4` mode 4's scripted candidates. `k = 6` is the via centre
/// (`:381-384`) and `k = 7`/`k = 8` are moves of length 1 and `sqrt(2)`, both inside `:388-397`'s
/// `lengthApprox() <= 1.5` refusal — which only fires under `AngleRestriction.NONE`, so on these
/// two `FORTYFIVE_DEGREE` boards the guard is passed over and the `checkTraceSegment` pair decides.
#[test]
fn overload_b_matches_the_jvm_on_every_scripted_candidate() {
    assert_overload_rows("rpi", 4, "repB ");
}

/// Overload C (`:434-713`) over `p7t4` mode 5's five cost pairs.
///
/// `rpi`'s section is all `SKIP` rows, so what survives here is the **dispatch replica**, pinned
/// against the jar's own classification of every `rpi` via. `j2`'s six vias x five pairs were the real
/// evidence for the cost gates themselves and they retired at the plan9-t7t8 accept wave; the
/// record above says so in those words rather than leaving the thinner test looking like the
/// original.
#[test]
fn overload_c_matches_the_jvm_on_every_cost_pair() {
    assert_overload_rows("rpi", 5, "repC ");
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
    // PORT-REGRESSION PIN, `accepted at plan9-t7t8 (ruling CC)`: the jar's via is `187`, the
    // port's is `189` — Task 8's door-set fixes mint two more board items over this fixture's
    // routing prologue. Via `84` is below the point where the two boards part and keeps its
    // number on both sides. **Both answers are the jar's, to the digit**, and so is the recursion
    // this test is named for.
    for (via_id, overload_a_answer, final_center) in [
        (
            ItemId(189),
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

// **RETIRED at the plan9-t7t8 accept wave (ruling CC).**
// `a_two_trace_via_takes_overload_c` stood here. It pinned `optViaLocation:118-131` — a via with
// two free trace contacts reaching overload C, and the twelve arguments `:118-131` builds — on
// `Issue026-J2_reference`'s via 264: overload C called directly answered `(1316044,-867516)` and
// `opt_via_location` moved the via exactly there.
//
// It was `j2`-keyed, so the retirement record at the top of this file carries its cause, its last
// matching run and its authorizing rows. The short version: the port no longer builds the board
// the jar's transcript describes, by one integer unit on one net-10 trace, and via 264 is not the
// same via on the two sides.
//
// **Nothing replaces the two-trace evidence for the overload's own arithmetic**: `rpi`'s mode-5
// section is all `SKIP` rows. Recorded as the real cost of the retirement.

// **Overload B has no caller outside overload C.** In Java it is `private` and the only four
// invocations are the axis-parallel decomposition arms at `:599`, `:627`, `:665` and `:696`; the
// port must not have grown a fifth. Asserted on the source, because a call graph is not something
// a board fixture can show.
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

// **RETIRED at the plan9-t7t8 accept wave (ruling CC).**
// `a_candidate_tie_keeps_the_first_in_contact_order` stood here, and it is worth recording what
// it established rather than only that it existed, because nothing replaces it.
//
// It answered the plan's transcription note — "where two candidates tie, Java keeps the **first**
// found in contact order; the port must not use a `max_by` that keeps the last" — by showing the
// note asks for the wrong thing. There is no `max_by` and no scoring pass anywhere in overload C;
// it is a sequence of **gated attempts**, each returning the moment it succeeds. So the note
// split into two facts:
//
// 1. **A tie is not a candidate at all.** Every gate is a strict `>` (`:492`, `:514`, `:555`,
//    `:597`, `:625`, `:663`, `:693`), so equal weighted distances skip the arm. `j2`'s via 231
//    showed it end to end: with `costs1 == costs2` overload C answered `null`, and with the very
//    same geometry under `(1.0, 2.0)` / `(2.0, 1.0)` it answered `(1228467,-826441)` and under
//    `(2.0, 1.0)` / `(1.0, 2.0)` `(1231088,-829062)` — three answers from one board, decided only
//    by the gates.
// 2. **The first success returns.** Via 130's five cost pairs all answered `(1337405,-857248)`,
//    the *second* from-corner: `:462-480`'s overlapping-lines arm runs before every cost gate and
//    returns whatever it finds.
//
// Both vias are `Issue026-J2_reference`'s, so the retirement record at the top of this file
// carries the cause. The strictness of the gates is now unpinned on every fixture, which is the
// second half of what this wave gave up here.

// `:712` — overload C's fall-through. Every gate can be open and every probe can still refuse, and
// the method then answers `null` **having changed nothing**; `optViaLocation:132-134` turns that
// into `return false`, which is why Task 6 could stub it inertly.
//
// `j2`'s via 124 is the case: all five cost pairs answer `null`. The board-untouched half is
// asserted for **every** via and **every** pair, successes included — overload C never mutates.
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
