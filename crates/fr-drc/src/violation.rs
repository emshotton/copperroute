use fr_board::{Board, DrcSeverity, Item, ItemId, ItemKind};
use fr_geometry::FloatPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DrcViolationKind {
    Clearance,
    ShortingItems,
    TracksCrossing,
    HoleClearance,
    HoleToHole,
    CopperEdgeClearance,
    TrackWidth,
    ViaDiameter,
    AnnularWidth,
    DrillOutOfRange,
    MicroviaDrillOutOfRange,
}

impl DrcViolationKind {
    pub const ALL: [DrcViolationKind; 11] = [
        DrcViolationKind::Clearance,
        DrcViolationKind::ShortingItems,
        DrcViolationKind::TracksCrossing,
        DrcViolationKind::HoleClearance,
        DrcViolationKind::HoleToHole,
        DrcViolationKind::CopperEdgeClearance,
        DrcViolationKind::TrackWidth,
        DrcViolationKind::ViaDiameter,
        DrcViolationKind::AnnularWidth,
        DrcViolationKind::DrillOutOfRange,
        DrcViolationKind::MicroviaDrillOutOfRange,
    ];

    #[must_use]
    pub fn kicad_type(self) -> &'static str {
        match self {
            DrcViolationKind::Clearance => "clearance",
            DrcViolationKind::ShortingItems => "shorting_items",
            DrcViolationKind::TracksCrossing => "tracks_crossing",
            DrcViolationKind::HoleClearance => "hole_clearance",
            DrcViolationKind::HoleToHole => "hole_to_hole",
            DrcViolationKind::CopperEdgeClearance => "copper_edge_clearance",
            DrcViolationKind::TrackWidth => "track_width",
            DrcViolationKind::ViaDiameter => "via_diameter",
            DrcViolationKind::AnnularWidth => "annular_width",
            DrcViolationKind::DrillOutOfRange => "drill_out_of_range",
            DrcViolationKind::MicroviaDrillOutOfRange => "microvia_drill_out_of_range",
        }
    }

    #[must_use]
    pub fn from_kicad_type(name: &str) -> Option<DrcViolationKind> {
        DrcViolationKind::ALL
            .into_iter()
            .find(|kind| kind.kicad_type() == name)
    }

    #[must_use]
    pub fn is_pair(self) -> bool {
        matches!(
            self,
            DrcViolationKind::Clearance
                | DrcViolationKind::ShortingItems
                | DrcViolationKind::TracksCrossing
                | DrcViolationKind::HoleClearance
                | DrcViolationKind::HoleToHole
                | DrcViolationKind::CopperEdgeClearance
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrcViolation {
    pub kind: DrcViolationKind,
    pub severity: DrcSeverity,
    pub first_item: ItemId,
    pub second_item: Option<ItemId>,
    pub layer: Option<usize>,
    pub position: FloatPoint,
    pub expected: f64,
    pub actual: f64,
    pub estimated: bool,
}

impl DrcViolation {
    #[must_use]
    pub fn shortfall(&self) -> f64 {
        (self.expected - self.actual).max(0.0)
    }

    #[must_use]
    pub fn involves_routing(&self, board: &Board) -> bool {
        let is_routing = |id: ItemId| {
            matches!(
                board.get_item(id).map(Item::kind),
                Some(ItemKind::Trace | ItemKind::Via)
            )
        };
        is_routing(self.first_item) || self.second_item.is_some_and(is_routing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_kicad_type_string() {
        for kind in DrcViolationKind::ALL {
            assert_eq!(DrcViolationKind::from_kicad_type(kind.kicad_type()), Some(kind));
        }
        assert_eq!(DrcViolationKind::Clearance.kicad_type(), "clearance");
        assert_eq!(DrcViolationKind::HoleToHole.kicad_type(), "hole_to_hole");
        assert_eq!(
            DrcViolationKind::MicroviaDrillOutOfRange.kicad_type(),
            "microvia_drill_out_of_range"
        );
        assert_eq!(DrcViolationKind::from_kicad_type("silk_overlap"), None);
    }

    #[test]
    fn the_shortfall_is_never_negative() {
        let violation = DrcViolation {
            kind: DrcViolationKind::TrackWidth,
            severity: DrcSeverity::Error,
            first_item: ItemId(1),
            second_item: None,
            layer: Some(0),
            position: FloatPoint::new(0.0, 0.0),
            expected: 100.0,
            actual: 150.0,
            estimated: false,
        };
        assert_eq!(violation.shortfall(), 0.0);
    }
}
