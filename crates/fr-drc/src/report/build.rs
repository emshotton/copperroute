use fr_board::structure::Unit;
use fr_board::{Board, ClearanceViolation, Item, ItemId, ItemKind};
use fr_dsn::CoordinateTransform;
use fr_dsn::format::double::java_format_fixed;
use fr_geometry::TileShape;

use crate::checker::DesignRulesChecker;
use crate::report::{KiCadDrcPosition, KiCadDrcReport, KiCadDrcViolation, KiCadDrcViolationItem};
use crate::unconnected::{UnconnectedItems, UnconnectedKind};

#[derive(Debug, Clone, PartialEq)]
pub struct DrcReportOptions {
            pub source: String,
                pub coordinate_unit: String,
                pub date: String,
            pub freerouting_version: String,
                                                        pub quality_score: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrcCoordinates {
        pub transform: CoordinateTransform,
        pub board_unit: Unit,
}

impl DrcCoordinates {
                                            pub fn convert_coordinate(&self, board_coordinate: f64, coordinate_unit: &str) -> f64 {
        let dsn_coordinate = self.transform.board_to_dsn(board_coordinate);

        let target_unit = match coordinate_unit {
            "mm" => Unit::Mm,
            "mil" => Unit::Mil,
            "inch" => Unit::Inch,
            "um" => Unit::Um,
            _ => self.board_unit,
        };

        if target_unit != self.board_unit {
            return Unit::scale(dsn_coordinate, self.board_unit, target_unit);
        }
        dsn_coordinate
    }
}

impl DesignRulesChecker<'_> {
                                                                    pub fn generate_report(
        &mut self,
        coords: &DrcCoordinates,
        options: &DrcReportOptions,
    ) -> KiCadDrcReport {
        let mut report = KiCadDrcReport::new(
            &options.coordinate_unit,
            &options.source,
            format!("Freerouting {}", options.freerouting_version),
            &options.date,
        );
        report.quality_score = options.quality_score.map(f64::from);

        let violations = self.get_all_clearance_violations();

        for violation in &violations {
            report.add_violation(convert_clearance_violation(
                self.board,
                violation,
                coords,
                &options.coordinate_unit,
            ));
        }

        let unconnected_items = self.get_all_unconnected_items();

        for unconnected_item in &unconnected_items {
            let entry = convert_unconnected_items(
                self.board,
                unconnected_item,
                coords,
                &options.coordinate_unit,
            );
            match unconnected_item.kind {
                UnconnectedKind::TrackDangling | UnconnectedKind::ViaDangling => {
                    report.add_violation(entry);
                }
                UnconnectedKind::UnconnectedItems => report.add_unconnected_item(entry),
            }
        }

        report
    }
}

fn convert_clearance_violation(
    board: &Board,
    violation: &ClearanceViolation,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> KiCadDrcViolation {
    let first_item_desc = item_description(board, violation.first_item);
    let second_item_desc = item_description(board, violation.second_item);

    let first_item_pos = item_position(board, violation.first_item, coords, coordinate_unit);
    let second_item_pos = item_position(board, violation.second_item, coords, coordinate_unit);

    let items = vec![
        KiCadDrcViolationItem::new(
            &first_item_desc,
            first_item_pos,
            violation.first_item.0.to_string(),
        ),
        KiCadDrcViolationItem::new(
            &second_item_desc,
            second_item_pos,
            violation.second_item.0.to_string(),
        ),
    ];

    let kind = if is_hole(board, violation.first_item) || is_hole(board, violation.second_item) {
        "holeClearance"
    } else {
        "clearance"
    };

    let expected = format_length(violation.expected_clearance, coords, coordinate_unit);
    let actual = format_length(violation.actual_clearance, coords, coordinate_unit);
    let lead = if kind == "holeClearance" {
        "Hole clearance violation"
    } else {
        "Clearance violation"
    };
    let description = format!(
        "{lead} between {first_item_desc} and {second_item_desc} \
         (expected: {expected} {coordinate_unit}, actual: {actual} {coordinate_unit})"
    );

    KiCadDrcViolation::new(kind, description, "error", items)
}

fn convert_unconnected_items(
    board: &Board,
    unconnected_items: &UnconnectedItems,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> KiCadDrcViolation {
    if matches!(
        unconnected_items.kind,
        UnconnectedKind::TrackDangling | UnconnectedKind::ViaDangling
    ) {
        let item = unconnected_items.first_item;
        let item_desc = match unconnected_items.kind {
            UnconnectedKind::ViaDangling => item_description(board, item),
            _ => detailed_trace_description(board, item, coords, coordinate_unit),
        };
        let items = vec![KiCadDrcViolationItem::new(
            &item_desc,
            item_position(board, item, coords, coordinate_unit),
            item.0.to_string(),
        )];

        let description = match unconnected_items.kind {
            UnconnectedKind::ViaDangling => "Via is not connected or connected on only one layer",
            _ => "Track has unconnected end",
        };
        return KiCadDrcViolation::new(
            head_kind_string(unconnected_items.kind),
            description,
            "warning",
            items,
        );
    }

    let items: Vec<KiCadDrcViolationItem> = unconnected_items
        .all_items
        .iter()
        .map(|&id| {
            KiCadDrcViolationItem::new(
                item_description(board, id),
                item_position(board, id, coords, coordinate_unit),
                id.0.to_string(),
            )
        })
        .collect();

    let from_item_desc = item_description(board, unconnected_items.first_item);
    let description = match unconnected_items.second_item {
        Some(second) => format!(
            "Unconnected items: {from_item_desc} and {} ({} total items in net)",
            item_description(board, second),
            unconnected_items.all_items.len(),
        ),
        None => format!("Unconnected item: {from_item_desc}"),
    };

    KiCadDrcViolation::new(
        head_kind_string(unconnected_items.kind),
        description,
        "warning",
        items,
    )
}

fn head_kind_string(kind: UnconnectedKind) -> &'static str {
    match kind {
        UnconnectedKind::UnconnectedItems => "unconnectedItems",
        UnconnectedKind::TrackDangling => "track_dangling",
        UnconnectedKind::ViaDangling => "via_dangling",
    }
}

fn is_hole(board: &Board, id: ItemId) -> bool {
    matches!(
        board.get_item(id).map(Item::kind),
        Some(ItemKind::Via | ItemKind::Pin),
    )
}

pub fn item_description(board: &Board, id: ItemId) -> String {
    let item = board
        .get_item(id)
        .unwrap_or_else(|| panic!("item_description: board has no item {}", id.0));

    let mut desc = match item.kind() {
        ItemKind::Trace => "Trace".to_string(),
        ItemKind::Via => "Via".to_string(),
        ItemKind::Pin => "Pin".to_string(),
        ItemKind::ConductionArea => "Conduction Area".to_string(),
        ItemKind::ObstacleArea => "ObstacleArea".to_string(),
        ItemKind::ViaObstacleArea => "ViaObstacleArea".to_string(),
        ItemKind::ComponentObstacleArea => "ComponentObstacleArea".to_string(),
        ItemKind::ComponentOutline => "ComponentOutline".to_string(),
        ItemKind::BoardOutline => "BoardOutline".to_string(),
        ItemKind::Other => "Item".to_string(),
    };

    if item.net_count() > 0
        && let Some(net) = board.rules.nets.get(item.get_net_number(0))
    {
        desc.push_str(" [");
        desc.push_str(&net.name);
        desc.push(']');
    }

    desc
}

fn detailed_trace_description(
    board: &Board,
    id: ItemId,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> String {
    let mut desc = "Track".to_string();
    let item = board
        .get_item(id)
        .unwrap_or_else(|| panic!("detailed_trace_description: board has no item {}", id.0));

    if item.net_count() > 0
        && let Some(net) = board.rules.nets.get(item.get_net_number(0))
    {
        desc.push_str(" [");
        desc.push_str(&net.name);
        desc.push(']');
    }

    if let Item::Trace(trace) = item {
        let layer_name = board
            .layer_structure()
            .layers
            .get(trace.get_layer())
            .map(|layer| layer.name.as_str())
            .unwrap_or_default();
        desc.push_str(" on ");
        desc.push_str(layer_name);

        let length = format_length(trace.get_length(), coords, coordinate_unit);
        desc.push_str(", length ");
        desc.push_str(&length);
        desc.push(' ');
        desc.push_str(coordinate_unit);
    }

    desc
}

fn item_position(
    board: &Board,
    id: ItemId,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> KiCadDrcPosition {
    let item = board
        .get_item(id)
        .unwrap_or_else(|| panic!("item_position: board has no item {}", id.0));
    let centre = TileShape::Box(item.bounding_box(&board.ctx())).centre_of_gravity();
    KiCadDrcPosition::new(
        coords.convert_coordinate(centre.x, coordinate_unit),
        coords.convert_coordinate(centre.y, coordinate_unit),
    )
}

fn format_length(board_length: f64, coords: &DrcCoordinates, coordinate_unit: &str) -> String {
    java_format_fixed(coords.convert_coordinate(board_length, coordinate_unit), 4)
}
