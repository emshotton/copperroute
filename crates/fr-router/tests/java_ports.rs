//! |---|---|---|
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


mod maze_list_element_test {
    use super::*;

            fn test_doors(ids: &[i32]) -> impl Fn(ExpandableRef) -> i32 + use<> {
        let map: HashMap<ExpandableRef, i32> = ids
            .iter()
            .map(|id| (ExpandableRef::Door(DoorId(*id as u32)), *id))
            .collect();
        move |door| *map.get(&door).expect("a TestDoor the test registered")
    }

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

        #[test]
    fn compare_to_returns_zero_for_same_instance() {
        let ids = test_doors(&[1]);
        let element = element(1, 0.0, 1.0);

        assert_eq!(element.compare_to(&element, &ids), Ordering::Equal);
    }

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


mod sorted_room_neighbours_factory_test {
    use super::*;

    fn tree(angle: AngleRestriction) -> ShapeSearchTree {
        ShapeSearchTree::new(TreeId(1), angle, 0)
    }

        #[test]
    fn selects_orthogonal_calculation_for_orthogonal_search_tree() {
        assert_eq!(
            select_calculation_mode(&tree(AngleRestriction::NinetyDegree)),
            CalculationMode::Orthogonal
        );
    }

        #[test]
    fn selects_45_degree_calculation_for_45_degree_search_tree() {
        assert_eq!(
            select_calculation_mode(&tree(AngleRestriction::FortyFiveDegree)),
            CalculationMode::FortyFiveDegree
        );
    }

        #[test]
    fn selects_any_angle_calculation_for_other_search_trees() {
        assert_eq!(
            select_calculation_mode(&tree(AngleRestriction::None)),
            CalculationMode::AnyAngle
        );
    }
}


mod routable_layers_safety_check_test {
    use super::*;

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

                                                                #[test]
    fn routing_fails_when_all_layers_disabled_current() {
        let board = load("Issue508-DAC2020_bm01.dsn");

        let mut settings = RouterSettings::new();
        settings.set_layer_count(board.get_layer_count());
        settings.apply_board_specific_optimizations(&board);

        assert!(
            (0..settings.get_layer_count()).any(
                |i| settings.get_layer_active(i) && board.layer_structure().layers[i].is_signal
            ),
            "the fixture must start with at least one routable layer"
        );

        for i in 0..settings.get_layer_count() {
            settings.set_layer_active(i, false);
        }

        let any_routable = (0..settings.get_layer_count())
            .any(|i| settings.get_layer_active(i) && board.layer_structure().layers[i].is_signal);
        assert!(!any_routable, "no layer may be routable once all are off");

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
