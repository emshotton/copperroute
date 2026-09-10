use copper_board::Board;
use copper_settings::RouterSettings;

pub fn prepare_board(board: &mut Board, settings: &RouterSettings) -> bool {
    let copper_changed = match settings.copper_to_edge_clearance_um {
        Some(clearance_um) => board.apply_copper_to_edge_clearance_override(clearance_um),
        None => false,
    };
    let hole_changed = match settings.hole_clearance_um {
        Some(clearance_um) => board.apply_hole_clearance_override(clearance_um),
        None => false,
    };
    let raised = raise_to_project_minimums(board);
    copper_changed || hole_changed || raised
}

/// Resolved settings always carry a hole clearance (zero by default), so a project
/// minimum has to raise the applied value rather than fill in a missing one.
pub fn raise_to_project_minimums(board: &mut Board) -> bool {
    let mask_raised = board.raise_solder_mask_clearances();
    let Some(constraints) = board.rules.drc_constraints.as_ref() else {
        return mask_raised;
    };
    let copper_minimum = constraints.min_clearance;
    let copper_edge_minimum = constraints.copper_edge_clearance;
    let hole_minimum = constraints.hole_clearance;
    let clearance_raised =
        copper_minimum.is_some_and(|minimum| board.raise_copper_clearances_to(minimum));
    let copper_raised =
        copper_edge_minimum.is_some_and(|minimum| board.raise_copper_to_edge_clearance_to(minimum));
    let hole_raised = hole_minimum.is_some_and(|minimum| board.raise_hole_clearance_to(minimum));
    copper_raised || hole_raised || mask_raised || clearance_raised
}

#[cfg(test)]
mod tests {
    use super::*;
    use copper_board::DrcConstraints;

    #[test]
    fn project_copper_minimum_floors_lower_classes_without_lowering_larger_ones() {
        let json = r#"{"resolution":10000,"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],"netClasses":[{"name":"Default","clearance":0.1,"traceWidth":0.2},{"name":"Wide","clearance":0.3,"traceWidth":0.2}]}"#;
        let copper_dsn::BoardReadResult::Success {
            board: Some(mut board),
            ..
        } = copper_dsn::kicad::read_board(json, None)
        else {
            panic!("fixture must load")
        };
        board
            .rules
            .clearance_matrix
            .append_class("dedicated_non_copper");
        let excluded = board.rules.clearance_matrix.get_class_count() - 1;
        let before = board.rules.clearance_matrix.clone();
        board.rules.drc_constraints = Some(DrcConstraints {
            min_clearance: Some(2000),
            ..DrcConstraints::default()
        });
        assert!(raise_to_project_minimums(&mut board));
        let after = &board.rules.clearance_matrix;
        for layer in 0..2 {
            for a in 0..before.get_class_count() {
                for b in 0..before.get_class_count() {
                    let old = before.get_value(a, b, layer, false);
                    let expected = if a == 0 || b == 0 || a == excluded || b == excluded {
                        old
                    } else {
                        old.max(2000)
                    };
                    assert_eq!(after.get_value(a, b, layer, false), expected);
                }
            }
        }
        assert!(!raise_to_project_minimums(&mut board));
    }
    #[test]
    fn copper_floor_refreshes_compensated_pin_shapes() {
        let json = r#"{"resolution":10000,"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],"netClasses":[{"name":"Default","clearance":0.1,"traceWidth":0.2}],"nets":[{"id":1,"name":"A"}],"components":[{"reference":"J1","position":{"x":0,"y":0},"layer":"F.Cu","pads":[{"name":"1","netName":"A","shape":"rect","size":{"x":1,"y":1},"layers":["F.Cu","B.Cu"]}]}]}"#;
        let copper_dsn::BoardReadResult::Success {
            board: Some(mut board),
            ..
        } = copper_dsn::kicad::read_board(json, None)
        else {
            panic!("fixture must load")
        };
        board.set_clearance_compensation_used(true);
        let tree = board.trees.get_default_tree().id();
        let pin = board.get_pins()[0];
        let before = board
            .get_item(pin)
            .unwrap()
            .get_tree_shape(tree, 0)
            .unwrap()
            .clone();
        assert!(board.raise_copper_clearances_to(2000));
        let after = board
            .get_item(pin)
            .unwrap()
            .get_tree_shape(tree, 0)
            .unwrap();
        assert_ne!(&before, after);
    }
}
