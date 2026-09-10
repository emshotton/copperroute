use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrcSeverity {
    Error,
    Warning,
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrcConstraints {
    pub netclass_clearance: BTreeMap<String, i32>,
    pub netclass_track_width: BTreeMap<String, i32>,
    pub min_clearance: Option<i32>,
    pub min_track_width: Option<i32>,
    pub hole_clearance: Option<i32>,
    pub hole_to_hole: Option<i32>,
    pub copper_edge_clearance: Option<i32>,
    pub solder_mask_to_copper_clearance: Option<i32>,
    pub solder_mask_min_width: Option<i32>,
    pub min_via_diameter: Option<i32>,
    pub min_via_annular_width: Option<i32>,
    pub min_through_hole_diameter: Option<i32>,
    pub min_microvia_diameter: Option<i32>,
    pub min_microvia_drill: Option<i32>,
    pub severities: BTreeMap<String, DrcSeverity>,
    pub epsilon: i32,
}

impl DrcConstraints {
    #[must_use]
    pub fn merge(dsn: DrcConstraints, project: DrcConstraints) -> DrcConstraints {
        let mut netclass_clearance = dsn.netclass_clearance;
        netclass_clearance.extend(project.netclass_clearance);
        let mut netclass_track_width = dsn.netclass_track_width;
        netclass_track_width.extend(project.netclass_track_width);
        let mut severities = dsn.severities;
        severities.extend(project.severities);
        DrcConstraints {
            netclass_clearance,
            netclass_track_width,
            min_clearance: project.min_clearance.or(dsn.min_clearance),
            min_track_width: project.min_track_width.or(dsn.min_track_width),
            hole_clearance: project.hole_clearance.or(dsn.hole_clearance),
            hole_to_hole: project.hole_to_hole.or(dsn.hole_to_hole),
            copper_edge_clearance: project.copper_edge_clearance.or(dsn.copper_edge_clearance),
            solder_mask_to_copper_clearance: project
                .solder_mask_to_copper_clearance
                .or(dsn.solder_mask_to_copper_clearance),
            solder_mask_min_width: project.solder_mask_min_width.or(dsn.solder_mask_min_width),
            min_via_diameter: project.min_via_diameter.or(dsn.min_via_diameter),
            min_via_annular_width: project.min_via_annular_width.or(dsn.min_via_annular_width),
            min_through_hole_diameter: project
                .min_through_hole_diameter
                .or(dsn.min_through_hole_diameter),
            min_microvia_diameter: project.min_microvia_diameter.or(dsn.min_microvia_diameter),
            min_microvia_drill: project.min_microvia_drill.or(dsn.min_microvia_drill),
            severities,
            epsilon: project.epsilon.max(dsn.epsilon),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn merge_lets_the_project_win_per_field_and_the_dsn_fill_gaps() {
        let mut dsn = DrcConstraints::default();
        dsn.min_track_width = Some(2000);
        dsn.netclass_clearance.insert("Default".to_string(), 2000);
        dsn.netclass_clearance.insert("Power".to_string(), 3000);

        dsn.epsilon = 5;

        let mut project = DrcConstraints::default();
        project.min_track_width = Some(1500);
        project.hole_to_hole = Some(2500);
        project
            .netclass_clearance
            .insert("Default".to_string(), 1800);
        project
            .severities
            .insert("track_width".to_string(), DrcSeverity::Warning);
        project.epsilon = 0;

        let merged = DrcConstraints::merge(dsn, project);
        assert_eq!(merged.min_track_width, Some(1500));
        assert_eq!(merged.hole_to_hole, Some(2500));
        assert_eq!(merged.netclass_clearance["Default"], 1800);
        assert_eq!(merged.netclass_clearance["Power"], 3000);
        assert_eq!(merged.severities["track_width"], DrcSeverity::Warning);
        assert_eq!(merged.epsilon, 5);
    }

    #[test]
    fn a_fresh_board_rules_has_no_constraints() {
        use crate::rules::ClearanceMatrix;
        use crate::structure::{Layer, LayerStructure};
        let ls = LayerStructure::new(vec![Layer::new("F", true)]);
        let rules = crate::rules::BoardRules::new(
            ls.clone(),
            ClearanceMatrix::get_default_instance(&ls, 200),
        );
        assert!(rules.drc_constraints.is_none());
    }
}
