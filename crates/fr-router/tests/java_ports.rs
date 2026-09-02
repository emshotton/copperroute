//! Plan 6 Task 18: **the three in-scope Java test suites, ported one method for one method**
//! (plan-6 ruling 12, spec §14.1).
//!
//! This file exists so the trail from a Java suite to its Rust counterpart is one `grep`. The
//! three sources, verbatim paths under `$FREEROUTING_JAVA_DIR/src/test/java/app/freerouting`:
//!
//! | Java suite | lines | ported below |
//! |---|---|---|
//! | `autoroute/maze/MazeListElementTest.java` | `:12-83` | [`maze_list_element_test`] |
//! | `autoroute/expansion/SortedRoomNeighboursFactoryTest.java` | `:12-34` | [`sorted_room_neighbours_factory_test`] |
//! | `autoroute/RoutableLayersSafetyCheckTest.java` | `:13-33` | [`routable_layers_safety_check_test`] |
//!
//! Spec §14.1's other three named suites are **not** here, because they drive
//! `autoroute/pipeline`, which plan-6 ruling 2 puts in Plan 7. **All three landed there**, and
//! this is where to find them: `StrictDrcEnforcementTest` is `tests/strict_drc.rs` (Plan 7
//! Task 8), `RoutingPipelineComparisonTest` is `tests/pipeline.rs:104-105` (Plan 7 Task 15), and
//! `BatchAutorouterDebugTest` is deliberately **not** ported — `tests/pass_runner.rs`'s module doc
//! carries the grep that shows its 21 `@Test` methods drive `debug/DebugControl`, not the router.
//! `BoardHistoryTest`, the fourth suite of plan-7 ruling 14, is `tests/board_history.rs:915-1060`.
//!
//! The pass loop those suites call — `BatchAutorouter.runBatchLoop` / `AutorouteBatchLoop.run`,
//! the only producer of the `IllegalArgumentException` below — **landed in Plan 7 Task 10** as
//! `fr_router::pipeline::AutorouteBatchLoop::run`.
//!
//! # Where the *extended* assertions live
//!
//! Each suite's two or three Java assertions are reproduced here exactly. The quirk assertions
//! the same subjects need — the four tie-breaks and quirks #170/#171 for `MazeListElement`, the
//! non-transitive comparator (quirk #160) for the neighbour sorter, `AutorouteControl`'s whole
//! JVM field transcript — stay in the task files that produced them:
//! `tests/maze_list_element.rs` (Task 8), `tests/sorted_neighbours.rs` and
//! `tests/sorted_neighbours_regimes.rs` (Tasks 4/5), `tests/control.rs` (Task 8).
//!
//! # The third suite is split across the Plan 6 / Plan 7 seam
//!
//! `RoutableLayersSafetyCheckTest` loads `Issue508-DAC2020_bm01.dsn`, turns every layer off in
//! `job.routerSettings` and asserts an `IllegalArgumentException` from
//! `BatchAutorouter::runBatchLoop`. That throw is `autoroute/pipeline/AutorouteBatchLoop.java:52-55`
//! — Plan 7's. What Plan 6 owns is the **predicate** the throw tests: `AutorouteBatchLoop.java:44-50`
//! scans for `settings.getLayerActive(i) && layers[i].isSignal`, which is exactly what
//! `AutorouteControl`'s constructor (`autoroute/maze/AutorouteControl.java:145-161`) copies into
//! `ctrl.layerActive`. So the port asserts, on the same board and after the same mutation, that no
//! layer is routable — the state that makes Plan 7's throw correct.

use std::cmp::Ordering;
use std::collections::HashMap;

use fr_board::prelude::*;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{FloatLine, FloatPoint};
use fr_router::JavaTreeSet;
use fr_router::arena::DoorId;
use fr_router::autoroute::expansion::ExpandableRef;
use fr_router::autoroute::expansion::sorted_neighbours::{
    CalculationMode, select_calculation_mode,
};
use fr_router::autoroute::maze::{AutorouteControl, MazeAdjustment, MazeListElement};
use fr_settings::RouterSettings;

// =================================================================================================
// `autoroute/maze/MazeListElementTest.java:12-83`
// =================================================================================================

/// The port of `MazeListElementTest` (`autoroute/maze/MazeListElementTest.java:12-83`).
///
/// `MazeListElementTest.TestDoor` (`:37-78`) is an `ExpandableObject` whose only live method is
/// `getId()`. The port cannot mirror that as a type: `ExpandableObject.getId()` is not a property
/// of the reference the element holds — for an `ExpansionDoor` it is a hash of the two rooms' ids
/// (`ExpansionDoor.java:184-190`), for a `DrillPage` a hash of its shape and its **mutable**
/// `netNumber` (`DrillPage.java:189-193`, quirk #167). So `MazeListElement::compare_to` takes a
/// resolver where Java performs a virtual call, and the resolver here *is* `TestDoor.getId()`.
mod maze_list_element_test {
    use super::*;

    /// `MazeListElementTest.TestDoor` (`:37-78`), as a map from the element's arena reference to
    /// the id the Java test's field would have held.
    fn test_doors(ids: &[i32]) -> impl Fn(ExpandableRef) -> i32 + use<> {
        let map: HashMap<ExpandableRef, i32> = ids
            .iter()
            .map(|id| (ExpandableRef::Door(DoorId(*id as u32)), *id))
            .collect();
        move |door| *map.get(&door).expect("a TestDoor the test registered")
    }

    /// `new MazeListElement(new TestDoor(door), 0, null, 0, expansion, sorting, null, null,
    /// false, null, false)` — the Java test's constructor call, with the two arguments it passes
    /// `null` that the port makes non-optional (`shapeEntry`, `adjustment`) at their production
    /// values.
    fn element(door: i32, expansion: f64, sorting: f64) -> MazeListElement {
        MazeListElement {
            door: ExpandableRef::Door(DoorId(door as u32)),
            section_no_of_door: 0,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: expansion,
            sorting_value: sorting,
            next_room: None,
            shape_entry: FloatLine::new(FloatPoint::new(0.0, 0.0), FloatPoint::new(0.0, 0.0)),
            room_ripped: false,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        }
    }

    /// `MazeListElementTest.compareToReturnsZeroForSameInstance` (`:14-20`).
    #[test]
    fn compare_to_returns_zero_for_same_instance() {
        let ids = test_doors(&[1]);
        let element = element(1, 0.0, 1.0);

        assert_eq!(element.compare_to(&element, &ids), Ordering::Equal);
    }

    /// `MazeListElementTest.compareToSortsBySortingValue` (`:22-35`), including its message
    /// "Lower sortingValue must be expanded first".
    ///
    /// Java's `SortedSet<MazeListElement> queue = new TreeSet<>()` is a [`JavaTreeSet`] here
    /// (plan-6 rulings Y and Z: the comparator is not a total order, so a `BTreeSet` keeps and
    /// orders different elements), and `queue.first()` is its first in-order entry.
    #[test]
    fn compare_to_sorts_by_sorting_value() {
        let ids = test_doors(&[1, 2]);
        let lower_cost = element(1, 0.0, 1.0);
        let higher_cost = element(2, 0.0, 2.0);

        let mut queue: JavaTreeSet<MazeListElement> = JavaTreeSet::new();
        queue.add_by(higher_cost, |a, b| a.compare_to(b, &ids));
        queue.add_by(lower_cost.clone(), |a, b| a.compare_to(b, &ids));

        let first = queue.iter().next().expect("a non-empty queue");
        assert_eq!(
            first, &lower_cost,
            "Lower sortingValue must be expanded first"
        );
    }
}

// =================================================================================================
// `autoroute/expansion/SortedRoomNeighboursFactoryTest.java:12-34`
// =================================================================================================

/// The port of `SortedRoomNeighboursFactoryTest`
/// (`autoroute/expansion/SortedRoomNeighboursFactoryTest.java:12-34`).
///
/// The Java suite mocks three `ShapeSearchTree` **subclasses**; the port has one tree type
/// parameterised by its [`AngleRestriction`], which is the field
/// `SortedRoomNeighbours.selectCalculationMode` (`SortedRoomNeighbours.java:80-95`) is really
/// dispatching on — `ShapeSearchTree90Degree`/`ShapeSearchTree45Degree` exist in Java only to
/// carry it. Each Java method therefore becomes one test over the corresponding restriction.
mod sorted_room_neighbours_factory_test {
    use super::*;

    fn tree(angle: AngleRestriction) -> ShapeSearchTree {
        ShapeSearchTree::new(TreeId(1), angle, 0)
    }

    /// `selectsOrthogonalCalculationForOrthogonalSearchTree` (`:15-20`).
    #[test]
    fn selects_orthogonal_calculation_for_orthogonal_search_tree() {
        assert_eq!(
            select_calculation_mode(&tree(AngleRestriction::NinetyDegree)),
            CalculationMode::Orthogonal
        );
    }

    /// `selects45DegreeCalculationFor45DegreeSearchTree` (`:22-27`).
    #[test]
    fn selects_45_degree_calculation_for_45_degree_search_tree() {
        assert_eq!(
            select_calculation_mode(&tree(AngleRestriction::FortyFiveDegree)),
            CalculationMode::FortyFiveDegree
        );
    }

    /// `selectsAnyAngleCalculationForOtherSearchTrees` (`:29-34`).
    #[test]
    fn selects_any_angle_calculation_for_other_search_trees() {
        assert_eq!(
            select_calculation_mode(&tree(AngleRestriction::None)),
            CalculationMode::AnyAngle
        );
    }
}

// =================================================================================================
// `autoroute/RoutableLayersSafetyCheckTest.java:13-33`
// =================================================================================================

/// The Plan 6 half of `RoutableLayersSafetyCheckTest`
/// (`autoroute/RoutableLayersSafetyCheckTest.java:13-33`) — see this file's module docs for why
/// the `assertThrows` itself is Plan 7's.
mod routable_layers_safety_check_test {
    use super::*;

    /// `RoutingFixtureTest.getRoutingJob`'s board half (`fixtures/RoutingFixtureTest.java:53-92`),
    /// reduced to what this suite reads: `HeadlessBoardManager.loadFromSpecctraDsn` on the
    /// fixture's bytes.
    fn load(name: &str) -> Board {
        let path = parity::fixture(name);
        let file = std::fs::File::open(&path)
            .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
        match fr_dsn::read_board(file, None, Some(name), &DsnReadOptions::default()) {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => {
                *board.unwrap_or_else(|| panic!("{name} produced no board"))
            }
            other => panic!("{name} did not read: {other:?}"),
        }
    }

    /// `RoutableLayersSafetyCheckTest.testRoutingFailsWhenAllLayersDisabledCurrent` (`:15-32`),
    /// as far as the Plan 6 seam reaches: the same board, the same "disable all layers" loop
    /// (`:24-26`), and the predicate `AutorouteBatchLoop.java:44-50` throws on.
    ///
    /// The board is the DSN this suite names, not a synthetic stand-in, so the assertion is over
    /// the same `layerStructure` (six layers, four of them signal) Java's loop walks.
    ///
    /// The `assertThrows` half — `BatchAutorouter.runBatchLoop` — **landed in Plan 7 Task 10**:
    /// `fr_router::pipeline::AutorouteBatchLoop::run` answers
    /// `Err(fr_router::RouterError::NoRoutableLayer)` on this board, and
    /// `crates/fr-router/tests/batch_loop.rs` asserts it there. This test keeps the Plan 6 half —
    /// the predicate, over the same DSN — so the seam stays visible.
    ///
    /// Runs in a debug build: the DSN read is the whole cost (~40 ms), because nothing here
    /// builds a search tree or routes anything.
    #[test]
    fn routing_fails_when_all_layers_disabled_current() {
        let board = load("Issue508-DAC2020_bm01.dsn");

        // `new RouterSettings(board)` (RouterSettings.java:127-131).
        let mut settings = RouterSettings::new();
        settings.set_layer_count(board.get_layer_count());
        settings.apply_board_specific_optimizations(&board);

        // The board must have something to disable, or the assertion below is vacuous.
        assert!(
            (0..settings.get_layer_count()).any(
                |i| settings.get_layer_active(i) && board.layer_structure().layers[i].is_signal
            ),
            "the fixture must start with at least one routable layer"
        );

        // `:24-26`: `for (int i = 0; i < layerCount; i++) setLayerActive(i, false);`
        for i in 0..settings.get_layer_count() {
            settings.set_layer_active(i, false);
        }

        // `AutorouteBatchLoop.java:44-50`'s `anyRoutable` scan, which now finds nothing — the
        // state `:52-55` turns into the `IllegalArgumentException` this suite asserts.
        let any_routable = (0..settings.get_layer_count())
            .any(|i| settings.get_layer_active(i) && board.layer_structure().layers[i].is_signal);
        assert!(!any_routable, "no layer may be routable once all are off");

        // And the same reaches `AutorouteControl`, which is where this crate reads it
        // (`AutorouteControl.java:145-161`).
        let trace_costs = settings.get_trace_costs();
        let net_no = 1;
        let ctrl = AutorouteControl::new(
            &board,
            net_no,
            &settings,
            settings.get_via_costs(),
            &trace_costs,
        );
        assert!(
            !ctrl.layer_active.iter().any(|active| *active),
            "ctrl.layerActive must be all false"
        );
    }
}
