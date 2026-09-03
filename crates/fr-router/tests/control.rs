//! Plan 6 Task 8: `autoroute.maze.AutorouteControl` (AutorouteControl.java:18-311) — the block
//! of settings the whole maze reads, copied out of `RouterSettings` rather than borrowed
//! (plan-6 ruling 8).
//!
//! # Where the numbers come from
//!
//! `scripts/differential/java/probes/P6T8Probe.java` mode `ctrl` loads a fixture with the clone's
//! **HEAD jar**, builds `new RouterSettings(board)` and then the real
//! `AutorouteControl(board, net, settings, settings.getViaCosts(), settings.getTraceCosts())`,
//! and prints every field. Its stdout is committed as `tests/data/p6t8-autoroute-control.txt` and
//! [`every_field_of_the_jvm_transcript_is_reproduced`] replays it field for field.
//!
//! # Java-wins corrections to the task brief
//!
//! * The brief says the layer-active force-off (`:150-161`) "is `RoutableLayersSafetyCheckTest`'s
//!   subject". It is not: that test
//!   (`src/test/java/app/freerouting/autoroute/RoutableLayersSafetyCheckTest.java:14-32`) drives
//!   `BatchAutorouter.runBatchLoop` and asserts an `IllegalArgumentException` thrown by
//!   `autoroute/pipeline/AutorouteBatchLoop.java:44-56` — an `autoroute/pipeline` class, i.e.
//!   Plan 7's. What it *shares* with `AutorouteControl` is the predicate: Java's loop looks for
//!   `settings.getLayerActive(i) && layers[i].isSignal`, which is exactly the negation of the
//!   condition `:152` forces off. So the port carries the check at the level it owns —
//!   [`a_power_plane_is_forced_inactive_however_the_settings_ask`] and
//!   [`with_every_layer_disabled_no_layer_is_routable`] — and the exception itself is Plan 7's.
//!   It **landed in Plan 7 Task 10** as [`fr_router::RouterError::NoRoutableLayer`], raised by
//!   `fr_router::pipeline::AutorouteBatchLoop::run`; plan-7 ruling 7 makes it that plan's one new
//!   recovery boundary, and the only one that propagates rather than degrading. The Java suite's
//!   own `assertThrows` half is ported in `tests/java_ports.rs`.
//! * The brief's `a_null_net_uses_clearance_class_one_and_the_first_via_rule` is only reachable
//!   for `netNumber <= 0`. For any *positive* net number the board does not have, `initNet`'s
//!   null-net arm (`:212-216`) does run — and `:219` then dereferenced the same `null` through
//!   `BoardRules.getTraceHalfWidth` (`BoardRules.java:75-77`) and threw. Pinned by the probe
//!   (`ctrl net=1094 threw java.lang.NullPointerException`).
//!
//! # Plan 9 Task 6: one transcript row the port deliberately no longer reproduces
//!
//! Quirk #173 is fixed, so `ctrl net=1094` no longer throws: the null-net arm has its own
//! half-width fallback (the default net class) instead of reaching for a net the board does not
//! have. The jar's stdout stays committed verbatim — that row still says `threw
//! java.lang.NullPointerException`, because that is still what the jar does — and
//! [`every_field_of_the_jvm_transcript_is_reproduced`]'s `" threw "` branch now asserts the
//! port's answer positively rather than asserting a panic.
//! [`a_positive_unknown_net_gets_the_half_width_fallback`] replaces
//! `a_positive_net_the_board_does_not_have_throws_like_java`.

use std::path::Path;

use fr_board::ids::{NetClassId, ViaInfoId};
use fr_board::prelude::*;
use fr_board::rules::{ViaInfo, ViaRule};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, Shape, TileShape};
use fr_router::ExpansionCostFactor;
use fr_router::autoroute::maze::AutorouteControl;
use fr_settings::RouterSettings;

// =================================================================================================
// Helpers
// =================================================================================================

/// A real board, read the way `tests/expansion_rooms.rs` reads one.
fn fixture_board(name: &str) -> Board {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{name} produced no board"))
        }
        other => panic!("{name} did not read: {other:?}"),
    }
}

/// `new RouterSettings(board)` (RouterSettings.java:127-131).
fn board_settings(board: &Board) -> RouterSettings {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

/// `AutorouteControl(board, net, settings, settings.getViaCosts(), settings.getTraceCosts())`
/// (`:123-132`), which is what `AutorouteConnectionRouter.route:43` builds.
fn control(board: &Board, net_no: i32, settings: &RouterSettings) -> AutorouteControl {
    let trace_costs = settings.get_trace_costs();
    AutorouteControl::new(
        board,
        net_no,
        settings,
        settings.get_via_costs(),
        &trace_costs,
    )
}

/// `P6T8Probe.f(double)`.
fn f(v: f64) -> String {
    format!("{v:.6}")
}

/// `P6T8Probe.dumpControl`, field for field and in the same order, so the two strings can be
/// compared directly.
fn dump_control(board: &Board, net_no: i32, settings: &RouterSettings) -> String {
    let c = control(board, net_no, settings);
    let mut s = format!("ctrl net={net_no}");
    s += &format!(
        " pureSmd={}",
        AutorouteControl::is_pure_smd_net(board, net_no)
    );
    s += &format!(" layerCount={}", c.layer_count);
    s += " layerActive=";
    for b in &c.layer_active {
        s.push(if *b { '1' } else { '0' });
    }
    s += " traceHalfWidth=";
    for v in &c.trace_half_width {
        s += &format!("{v},");
    }
    s += " compensatedTraceHalfWidth=";
    for v in &c.compensated_trace_half_width {
        s += &format!("{v},");
    }
    s += " viaRadii=";
    for v in &c.via_radii {
        s += &format!("{},", f(*v));
    }
    s += " bendCosts=";
    for v in &c.bend_costs {
        s += &format!("{},", f(*v));
    }
    s += &format!(
        " traceClearanceClassIndex={}",
        c.trace_clearance_class_index
    );
    s += &format!(" viaClearanceClass={}", c.via_clearance_class);
    s += &format!(
        " viaRule={}",
        match &c.via_rule {
            Some(rule) => rule.name.clone(),
            None => "null".to_string(),
        }
    );
    s += " viaInfos=";
    for m in &c.via_infos {
        s += &format!(
            "[{}..{} attach={}]",
            m.from_layer, m.to_layer, m.attach_smd_allowed
        );
    }
    s += &format!(" attachSmdAllowed={}", c.attach_smd_allowed);
    s += &format!(" maxViaRadius={}", f(c.max_via_radius));
    s += &format!(" minNormalViaCost={}", f(c.min_normal_via_cost));
    s += &format!(" minCheapViaCost={}", f(c.min_cheap_via_cost));
    s += &format!(" viasAllowed={}", c.vias_allowed);
    s += &format!(" withNeckdown={}", c.with_neckdown);
    s += &format!(" viaLowerBound={}", c.via_lower_bound);
    s += &format!(" viaUpperBound={}", c.via_upper_bound);
    s += &format!(" tidyRegionWidth={}", c.tidy_region_width);
    s += &format!(" pullTightAccuracy={}", c.pull_tight_accuracy);
    s += &format!(
        " maxShoveTraceRecursionDepth={}",
        c.max_shove_trace_recursion_depth
    );
    s += &format!(
        " maxShoveViaRecursionDepth={}",
        c.max_shove_via_recursion_depth
    );
    s += &format!(
        " maxSpringOverRecursionDepth={}",
        c.max_spring_over_recursion_depth
    );
    s += &format!(" ripupAllowed={}", c.ripup_allowed);
    s += &format!(" ripupCosts={}", c.ripup_costs);
    s += &format!(" ripupPassNo={}", c.ripup_pass_no);
    s += &format!(" isFanout={}", c.is_fanout);
    s += &format!(" fanoutStartPinLayer={}", c.fanout_start_pin_layer);
    s += &format!(" removeUnconnectedVias={}", c.remove_unconnected_vias);
    s += &format!(
        " addViaCostsAllZero={}",
        c.add_via_costs
            .iter()
            .all(|row| row.iter().all(|v| *v == 0))
    );
    s
}

fn transcript() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/p6t8-autoroute-control.txt");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

// =================================================================================================
// The JVM transcript, replayed
// =================================================================================================

/// Every `ctrl` row of `P6T8Probe ctrl`'s stdout, on the two fixtures it was run over.
///
/// The `board …`, `layer …`, `settings …` and `firstPureSmdNet=…` header rows are checked too:
/// they are what makes the `ctrl` rows meaningful (a port that read a *different* board would
/// otherwise agree on the control block by accident).
#[test]
fn every_field_of_the_jvm_transcript_is_reproduced() {
    let text = transcript();
    let mut board: Option<Board> = None;
    let mut settings: Option<RouterSettings> = None;
    let mut ctrl_rows = 0usize;

    for line in text.lines() {
        let line = line.trim();
        if let Some(stem) = line.strip_prefix("######## ") {
            let b = fixture_board(&format!("{stem}.dsn"));
            settings = Some(board_settings(&b));
            board = Some(b);
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let b = board.as_ref().expect("a fixture header first");
        let s = settings.as_ref().expect("a fixture header first");

        if let Some(rest) = line.strip_prefix("board ") {
            assert_eq!(
                format!(
                    "layerCount={} maxNetNo={} viaRules={} viaInfos={}",
                    b.get_layer_count(),
                    b.rules.nets.max_net_number(),
                    b.rules.via_rules.len(),
                    b.rules.via_infos.count()
                ),
                rest
            );
        } else if let Some(rest) = line.strip_prefix("layer ") {
            let index: usize = rest.split(' ').next().unwrap().parse().unwrap();
            let layer = &b.layer_structure().layers[index];
            assert_eq!(
                format!(
                    "{index} name={} isSignal={} settingActive={}",
                    layer.name,
                    layer.is_signal,
                    s.get_layer_active(index)
                ),
                rest
            );
        } else if let Some(rest) = line.strip_prefix("settings ") {
            assert_eq!(
                format!(
                    "viaCosts={} viasAllowed={}",
                    s.get_via_costs(),
                    s.get_vias_allowed()
                ),
                rest
            );
        } else if let Some(rest) = line.strip_prefix("firstPureSmdNet=") {
            let (pure, mixed) = first_pure_and_mixed(b);
            assert_eq!(format!("{pure} firstMixedNet={mixed}"), rest);
        } else if line.contains(" threw ") {
            // A jar row this plan has deliberately moved away from. The transcript is the jar's
            // own stdout and stays committed verbatim — `ctrl net=1094 threw
            // java.lang.NullPointerException` is still what the jar does — but quirk #173 is
            // fixed here, so the port answers instead of throwing. Asserted positively rather
            // than just "does not panic": the null-net arm's own fallback is the default net
            // class's width on every layer. See `a_positive_unknown_net_gets_the_half_width_
            // fallback` for the full row and `docs/java-quirks.md` #173.
            let net: i32 = line
                .strip_prefix("ctrl net=")
                .and_then(|r| r.split(' ').next())
                .and_then(|n| n.parse().ok())
                .expect("a net number");
            assert!(
                b.rules.nets.get(net).is_none(),
                "the jar threw here because net {net} does not exist"
            );
            let ctrl =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| control(b, net, s)))
                    .unwrap_or_else(|_| {
                        panic!("net {net} must no longer throw — quirk #173 is fixed (fixed: T6)")
                    });
            let default_class = b.rules.net_classes.get(NetClassId(0));
            for layer in 0..b.get_layer_count() {
                assert_eq!(
                    ctrl.trace_half_width[layer],
                    default_class.get_trace_half_width(layer),
                    "net {net}, layer {layer}: the null-net arm's own half-width fallback"
                );
            }
            assert_eq!(ctrl.trace_clearance_class_index, 1, "`:213`, unchanged");
        } else if line.starts_with("ctrl net=") {
            let net: i32 = line
                .strip_prefix("ctrl net=")
                .and_then(|r| r.split(' ').next())
                .and_then(|n| n.parse().ok())
                .expect("a net number");
            assert_eq!(dump_control(b, net, s), line);
            ctrl_rows += 1;
        } else {
            panic!("unparsed transcript row: {line}");
        }
    }

    assert_eq!(
        ctrl_rows, 5,
        "five successful `ctrl` rows in the transcript"
    );
}

/// `P6T8Probe.control`'s scan.
fn first_pure_and_mixed(board: &Board) -> (i32, i32) {
    let mut pure = -1;
    let mut mixed = -1;
    for n in 1..=board.rules.nets.max_net_number() {
        if board.get_connectable_items(n).is_empty() {
            continue;
        }
        if AutorouteControl::is_pure_smd_net(board, n) {
            if pure < 0 {
                pure = n;
            }
        } else if mixed < 0 {
            mixed = n;
        }
    }
    (pure, mixed)
}

// =================================================================================================
// The branches the transcript covers, named
// =================================================================================================

/// Quirk #172, both halves (`:263-269` and `:277-281`), pinned by the probe on two fixtures.
///
/// `Issue593-BBD_Mars-64` net 1 is pure SMD, and its only via info says `attach=false`
/// (`viaInfos=[0..1 attach=false]`) — yet `ctrl.attachSmdAllowed` is `true`, and
/// `minNormalViaCost` is `400.0` where the same board's net 0 gets `4000.0`: the `× 0.1` of
/// `:280`. `Issue508-DAC2020_bm01` says the same with `300.0` against `3000.0`.
#[test]
fn pure_smd_relaxes_attach_and_scales_the_via_cost() {
    for (name, pure_net, mixed_net, pure_cost, normal_cost) in [
        ("Issue593-BBD_Mars-64.dsn", 1, 0, 400.0, 4000.0),
        ("Issue508-DAC2020_bm01.dsn", 8, 1, 300.0, 3000.0),
    ] {
        let board = fixture_board(name);
        let settings = board_settings(&board);

        assert!(AutorouteControl::is_pure_smd_net(&board, pure_net));
        let pure = control(&board, pure_net, &settings);
        assert!(
            !pure.via_infos[0].attach_smd_allowed,
            "{name}: the padstack itself still says attach=false"
        );
        assert!(
            pure.attach_smd_allowed,
            "{name}: :263-269 relaxes the routing gate anyway"
        );
        assert_eq!(pure.min_normal_via_cost, pure_cost, "{name}: :277-281");
        assert_eq!(pure.min_cheap_via_cost, 0.8 * pure_cost, "{name}: :283");

        assert!(!AutorouteControl::is_pure_smd_net(&board, mixed_net));
        let normal = control(&board, mixed_net, &settings);
        assert!(!normal.attach_smd_allowed, "{name}: no relaxation");
        assert_eq!(normal.min_normal_via_cost, normal_cost, "{name}");
        assert_eq!(
            normal.min_normal_via_cost,
            10.0 * pure.min_normal_via_cost,
            "{name}: the two differ by exactly the 0.1 factor"
        );
    }
}

/// `isPureSmdNet` (`:189-202`): "an empty net is **not** pure SMD" (`:191-193`), which is what
/// stops net 0 and every unused net number from taking the relaxation.
#[test]
fn an_empty_net_is_not_pure_smd() {
    let board = fixture_board("Issue593-BBD_Mars-64.dsn");
    assert!(board.get_connectable_items(0).is_empty());
    assert!(!AutorouteControl::is_pure_smd_net(&board, 0));
    let unused = board.rules.nets.max_net_number() + 1000;
    assert!(board.get_connectable_items(unused).is_empty());
    assert!(!AutorouteControl::is_pure_smd_net(&board, unused));
}

/// `initNet` (`:217-222`): `netNumber > 0` reads the net's own half widths, anything else fell
/// back to **net 1's**.
///
/// Quirk #173's fix replaced that fallback with the default net class's widths, and this test
/// still passes — **because on this board the two coincide**, which the assertion below now says
/// out loud instead of leaving it as luck. Net 1 of `Issue508-DAC2020_bm01.dsn` is on the default
/// net class, so "net 1's widths" and "the default class's widths" are the same numbers. On a
/// board where net 1 carried a class of its own they would differ, and that is exactly why the
/// register calls net 1 an arbitrary choice.
#[test]
fn net_zero_falls_back_to_net_one_half_widths() {
    let board = fixture_board("Issue508-DAC2020_bm01.dsn");
    let settings = board_settings(&board);

    assert_eq!(
        board
            .rules
            .nets
            .get(1)
            .expect("net 1 exists on this board")
            .get_net_class(),
        NetClassId(0),
        "the old fallback (net 1) and the new one (the default class) agree here only because \
         net 1 is on the default class"
    );

    let zero = control(&board, 0, &settings);
    let one = control(&board, 1, &settings);
    for layer in 0..board.get_layer_count() {
        assert_eq!(
            zero.trace_half_width[layer],
            board.rules.get_trace_half_width(1, layer)
        );
        assert_eq!(zero.trace_half_width[layer], one.trace_half_width[layer]);
    }
    // The clearance-compensation term of `:223-225` rides on the same widths.
    assert_eq!(
        zero.compensated_trace_half_width,
        one.compensated_trace_half_width
    );
}

/// `initNet`'s null-net arm (`:212-216`): clearance class **1** and
/// `board.rules.viaRules.firstElement()`, not the net class's.
#[test]
fn a_null_net_uses_clearance_class_one_and_the_first_via_rule() {
    let board = fixture_board("Issue508-DAC2020_bm01.dsn");
    let settings = board_settings(&board);
    assert!(
        board.rules.nets.get(0).is_none(),
        "net 0 is Java's null net"
    );

    let zero = control(&board, 0, &settings);
    assert_eq!(zero.trace_clearance_class_index, 1);
    // `:214` reads `board.rules.viaRules.firstElement()` — the object, which the port's control
    // block now owns a copy of (Plan 7 Task 11).
    assert_eq!(zero.via_rule.as_ref(), Some(&board.rules.via_rules[0]));
    assert!(board.rules.via_rules.len() > 1, "there is a second rule");
}

/// Quirk #173, inverted. The other half of the same arm: for a **positive** net number the board
/// does not have, `:212-216` ran and then `:219` threw — the probe's
/// `ctrl net=1094 threw java.lang.NullPointerException`. `RoutingBoard.java:1023` builds a control
/// from a pin's net number, so a stale net number was enough to kill the connection.
///
/// Task 6 gives the null-net arm its own half-width fallback: the **default net class**, which is
/// where a net with no class of its own belongs and is already the arm's choice for the clearance
/// class (`:213`'s literal 1 is `BoardRules.defaultClearanceClass`). The register's other option —
/// moving the `netNumber > 0` test above the null lookup — would have kept the arm reading net 1's
/// widths, and net 1 is arbitrary: whichever net was declared first, not necessarily existing, and
/// with no relation to a net the board does not have.
#[test]
fn a_positive_unknown_net_gets_the_half_width_fallback() {
    let board = fixture_board("Issue593-BBD_Mars-64.dsn");
    let settings = board_settings(&board);
    let unknown = board.rules.nets.max_net_number() + 1000;
    assert!(
        board.rules.nets.get(unknown).is_none(),
        "the net the control is built for must be one the board does not have"
    );

    let ctrl = control(&board, unknown, &settings);

    // `:212-216`, unchanged — the arm that always worked.
    assert_eq!(ctrl.trace_clearance_class_index, 1);
    assert_eq!(ctrl.via_rule.as_ref(), Some(&board.rules.via_rules[0]));
    // `:217-222`, the arm that used to throw. Every layer answers the default net class's width.
    let default_class = board.rules.net_classes.get(NetClassId(0));
    for layer in 0..board.get_layer_count() {
        assert_eq!(
            ctrl.trace_half_width[layer],
            default_class.get_trace_half_width(layer),
            "layer {layer} takes the default net class's width, not net 1's and not a throw"
        );
    }
    // And the compensation of `:223-225` rides on it rather than on a half-built control.
    assert!(
        ctrl.compensated_trace_half_width
            .iter()
            .zip(&ctrl.trace_half_width)
            .all(|(compensated, half)| compensated >= half),
        "`:223-225` adds a non-negative clearance compensation to each width"
    );
}

// =================================================================================================
// The layer-active gate (`:149-162` + `:226-228`)
// =================================================================================================

/// A three-layer board whose middle layer is a power plane, so `:152`'s
/// `!layers[i].isSignal && activeSetting` fires — plus the one via info and via rule
/// `initNet`'s `viaRules.firstElement()` (`:214`) and `rebuildViaInfo` (`:235`) need.
fn plane_board() -> Board {
    let layers = || {
        LayerStructure::new(vec![
            Layer::new("front", true),
            Layer::new("power", false),
            Layer::new("back", true),
        ])
    };
    let mut padstacks = Padstacks::new(layers());
    let via_padstack = padstacks.add_layer_range(
        Shape::Tile(TileShape::Box(IntBox::from_coords(-300, -300, 300, 300))),
        0,
        2,
    );
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules
        .via_infos
        .add(ViaInfo::new("via", via_padstack, 1, false));
    let mut via_rule = ViaRule::new("default");
    via_rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
    rules.via_rules.push(via_rule);
    // One net class and one net, because `initNet`'s net<=0 arm still reads **net 1's** trace
    // half widths (`:221`) and Java NPEs on a board with no nets at all.
    let class = rules.net_classes.append("default", &layers(), false);
    rules
        .net_classes
        .get_mut(class)
        .set_via_rule(Some(rules.via_rules[0].clone()));
    rules.nets.add("n1", 1, false, class);
    Board::new(
        Vec::new(),
        0,
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

/// `:152-161`: a dedicated power plane "cannot be routed. Forcing active state to false" — Java
/// logs a WARN and writes `false`; the port drops the log and keeps the write.
#[test]
fn a_power_plane_is_forced_inactive_however_the_settings_ask() {
    let board = plane_board();
    let mut settings = board_settings(&board);
    for layer in 0..3 {
        settings.set_layer_active(layer, true);
    }
    let ctrl = control(&board, 0, &settings);
    assert_eq!(ctrl.layer_active, vec![true, false, true]);

    // And with the setting already off, the `else` at `:160` copies the setting unchanged.
    settings.set_layer_active(0, false);
    let ctrl = control(&board, 0, &settings);
    assert_eq!(ctrl.layer_active, vec![false, false, true]);
}

/// The condition `AutorouteBatchLoop.java:44-50` scans for — `getLayerActive(i) &&
/// layers[i].isSignal` — expressed where this crate owns it. With every layer's setting off,
/// no layer is routable, which is what makes Plan 7's `IllegalArgumentException` correct.
///
/// The throw itself is `autoroute/pipeline`'s and **landed in Plan 7 Task 10**:
/// `fr_router::pipeline::AutorouteBatchLoop::run` answers
/// [`fr_router::RouterError::NoRoutableLayer`] on exactly this predicate.
#[test]
fn with_every_layer_disabled_no_layer_is_routable() {
    let board = plane_board();
    let mut settings = board_settings(&board);
    for layer in 0..3 {
        settings.set_layer_active(layer, false);
    }
    let ctrl = control(&board, 0, &settings);
    assert_eq!(ctrl.layer_active, vec![false, false, false]);
    assert!(!ctrl.layer_active.iter().any(|active| *active));
}

// =================================================================================================
// The private constructor's literals (`:163-186`) and ruling 8's two copied settings
// =================================================================================================

/// The five hard-coded recursion/accuracy constants of `:169-173`, plus the ripup and via-bound
/// defaults of `:181-186` and the flags of `:163-167`, `:180`.
#[test]
fn every_hard_coded_constant_matches_java() {
    let board = plane_board();
    let settings = board_settings(&board);
    let c = control(&board, 0, &settings);

    assert_eq!(c.tidy_region_width, i32::MAX, ":169");
    assert_eq!(c.pull_tight_accuracy, 500, ":170");
    assert_eq!(c.max_shove_trace_recursion_depth, 20, ":171");
    assert_eq!(c.max_shove_via_recursion_depth, 5, ":172");
    assert_eq!(c.max_spring_over_recursion_depth, 5, ":173");

    assert_eq!(c.via_lower_bound, 0, ":181");
    assert_eq!(c.via_upper_bound, c.layer_count, ":182");
    assert!(!c.ripup_allowed, ":184");
    assert_eq!(c.ripup_costs, 1000, ":185");
    assert_eq!(c.ripup_pass_no, 1, ":186");

    assert!(!c.is_fanout, ":163");
    assert_eq!(c.fanout_start_pin_name, None, ":164");
    assert_eq!(c.fanout_start_pin_center, None, ":165");
    assert_eq!(c.fanout_start_pin_layer, -1, ":166");
    assert!(c.remove_unconnected_vias, ":167");

    // `:174-178` zeroes every `addViaCosts[i].toLayer[j]` — the array exists and is all zero.
    assert_eq!(c.add_via_costs.len(), c.layer_count);
    assert!(
        c.add_via_costs
            .iter()
            .all(|row| row.len() == c.layer_count && row.iter().all(|v| *v == 0))
    );
}

/// Plan-6 ruling 8: `ctrl.settings` has exactly two readers anywhere in
/// `autoroute/{maze,expansion,drill,path}` — `MazeSearchEngine.java:96-97` and `:111-112` — so
/// the port copies those two numbers instead of holding a `&RouterSettings`.
///
/// Java's expressions are
/// `ctrl.settings.fanout != null && ctrl.settings.fanout.maxEscapeLengthMm != null ?
/// maxEscapeLengthMm * 1000.0 : 3000.0` and its `min` twin with `500.0`, so the copied value is
/// **already scaled** — the field is not a millimetre count.
#[test]
fn the_two_fanout_escape_lengths_are_copied_with_javas_defaults() {
    let board = plane_board();

    // `new RouterSettings()` builds a `FanoutSettings` whose two lengths are unset, which is
    // Java's `maxEscapeLengthMm == null` — the literal fall-backs.
    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    settings.fanout = None;
    let c = control(&board, 0, &settings);
    assert_eq!(c.fanout_max_escape_length, 3000.0);
    assert_eq!(c.fanout_min_escape_length, 500.0);

    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    if let Some(fanout) = settings.fanout.as_mut() {
        fanout.max_escape_length_mm = None;
        fanout.min_escape_length_mm = None;
    }
    let c = control(&board, 0, &settings);
    assert_eq!(c.fanout_max_escape_length, 3000.0);
    assert_eq!(c.fanout_min_escape_length, 500.0);

    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    if let Some(fanout) = settings.fanout.as_mut() {
        fanout.max_escape_length_mm = Some(4.5);
        fanout.min_escape_length_mm = Some(2.5);
    }
    let c = control(&board, 0, &settings);
    assert_eq!(c.fanout_max_escape_length, 4500.0);
    assert_eq!(c.fanout_min_escape_length, 2500.0);
}

/// `:145-147` and `:141`, `:168`: `bendCosts`, `viasAllowed` and `withNeckdown` are straight
/// copies, per layer where Java's is.
#[test]
fn the_per_layer_and_flag_copies_track_the_settings() {
    let board = plane_board();
    let mut settings = board_settings(&board);
    settings.set_bend_cost(0, 7.5);
    settings.set_bend_cost(2, 0.25);
    let c = control(&board, 0, &settings);

    assert_eq!(c.bend_costs, vec![7.5, settings.get_bend_cost(1), 0.25]);
    assert_eq!(c.vias_allowed, settings.get_vias_allowed());
    assert_eq!(c.with_neckdown, settings.get_automatic_neckdown());
    assert_eq!(c.layer_count, board.get_layer_count());
    assert_eq!(
        c.trace_costs,
        settings.get_trace_costs(),
        ":179 stores the array it was handed"
    );
}

/// `rebuildViaInfo` is `public` and re-runnable (`:234`): `BatchAutorouterThread.java:470` and
/// `AutorouteConnectionRouter.java:193` build a control per connection, but the ripup passes call
/// this directly with a new `viaCosts`. It is **not** idempotent on `viaRadii` and
/// `maxViaRadius`, which it `max`es into rather than resetting (`:258`, `:272-273`).
#[test]
fn rebuild_via_info_accumulates_the_radii_it_is_run_over() {
    let board = fixture_board("Issue593-BBD_Mars-64.dsn");
    let settings = board_settings(&board);
    let mut c = control(&board, 1, &settings);
    let radii = c.via_radii.clone();
    let max = c.max_via_radius;

    c.rebuild_via_info(&board, settings.get_via_costs() * 3, 1);

    assert_eq!(c.via_radii, radii, "the max is already at its fixed point");
    assert_eq!(c.max_via_radius, max);
    assert_eq!(
        c.min_normal_via_cost,
        3.0 * 400.0,
        "but the cost tracks the new viaCosts"
    );
    assert_eq!(c.min_cheap_via_cost, 0.8 * (3.0 * 400.0));
}

/// `ExpansionCostFactor` must be the *re-exported* `fr_settings` type (plan-6 ruling 8), so a
/// cost array built from settings drops straight into the control block.
#[test]
fn the_trace_costs_are_the_re_exported_settings_type() {
    let board = plane_board();
    let settings = board_settings(&board);
    let costs: Vec<ExpansionCostFactor> = settings.get_trace_costs();
    let c = AutorouteControl::new(&board, 0, &settings, 1, &costs);
    assert_eq!(c.trace_costs, costs);
}
