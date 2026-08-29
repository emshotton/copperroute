//! `DesignRulesChecker.generateReport` and the five private helpers that fill the DTOs.
//!
//! Java: `drc/DesignRulesChecker.java:210-289` (`generateReport`), `:299-355` and `:359-421` (the
//! two `convertToDrcViolation` overloads), `:424-430` (`isHole`), `:439-460`
//! (`getItemDescription`), `:470-493` (`getDetailedTraceDescription`), `:504-530`
//! (`convertCoordinate`).
//!
//! Every `FRLogger.trace` call in that range is dropped (four of them, `:216-227`, `:249-266`,
//! `:271-277`, `:281-288`), per the plan's global constraints. One of them is the *only* place
//! `ClearanceViolation.shape` is read in the whole report path (`:265`) — the positions the report
//! carries are the **items'** bounding-box centres, never the violation shape's centre.

use fr_board::structure::Unit;
use fr_board::{Board, ClearanceViolation, Item, ItemId, ItemKind};
use fr_dsn::CoordinateTransform;
use fr_dsn::format::double::java_format_fixed;
use fr_geometry::TileShape;

use crate::checker::DesignRulesChecker;
use crate::report::{KiCadDrcPosition, KiCadDrcReport, KiCadDrcViolation, KiCadDrcViolationItem};
use crate::unconnected::{UnconnectedItems, UnconnectedKind};

/// Everything `Freerouting.initializeDrc` injects into the report (plan-5 ruling 5). Plan 8 fills
/// it from the CLI; this crate has no clock, no `Constants` and no `BoardStatistics`.
#[derive(Debug, Clone, PartialEq)]
pub struct DrcReportOptions {
    /// `Freerouting.java:339`: `new File(globalSettings.initialInputFile).getName()`, i.e. the
    /// input file's base name.
    pub source: String,
    /// `Freerouting.java:335`: the CLI hard-codes `"mm"` and offers no way to change it
    /// (quirk #151), so three of `convert_coordinate`'s four named branches and its fallback are
    /// unreachable from `-drc`. They are ported anyway — the API/MCP path could reach them.
    pub coordinate_unit: String,
    /// The already-formatted `date` (KiCadDrcReport.java:70 renders
    /// `ZonedDateTime.now()` through `ISO_OFFSET_DATE_TIME`). Injected because `fr-drc` has no
    /// clock (plan-5 ruling 5).
    pub date: String,
    /// `Constants.FREEROUTING_VERSION` (DesignRulesChecker.java:212-213). The `"Freerouting "`
    /// prefix is [`DesignRulesChecker::generate_report`]'s, exactly as it is Java's.
    pub freerouting_version: String,
    /// `Freerouting.java:349`: `(double) finalStats.getNormalizedScore(routerSettings.scoring)` —
    /// a `float` widened to `double`, which is where the `.078369140625` tails come from.
    /// `None` is Java's `null`, which Gson omits from the JSON; `BoardStatistics` itself is
    /// Plan 8's (plan-5 ruling 5).
    pub quality_score: Option<f64>,
}

/// The board unit and the DSN transform `convertCoordinate` reaches through `board.communication`
/// (DesignRulesChecker.java:507, `:510`).
///
/// Java's `BasicBoard` owns both; the port's board owns only the unit — Plan 3 ruling A left
/// [`CoordinateTransform`] in `fr-dsn`, and `Communication.coordinateTransform` is a
/// `not ported:` line there. So the pair arrives as a parameter (plan-5 ruling 7).
#[derive(Debug, Clone, PartialEq)]
pub struct DrcCoordinates {
    /// Java `board.communication.coordinateTransform` (Communication.java:18).
    pub transform: CoordinateTransform,
    /// Java `board.communication.unit` (Communication.java:20).
    pub board_unit: Unit,
}

impl DrcCoordinates {
    /// Port of the private `convertCoordinate` (DesignRulesChecker.java:504-530): board units to
    /// DSN units, then — only when the named unit differs from the board's — [`Unit::scale`].
    ///
    /// The name→[`Unit`] chain (`:513-524`) is `"mm" | "mil" | "inch" | "um"`, anything else the
    /// board's own unit. It is **not** `Unit::from_string` (Unit.java:23-33): that one
    /// upper-cases first, so Java's `convertCoordinate` rejects `"MM"` where `from_string` accepts
    /// it. Transcribed as Java wrote it.
    ///
    /// Quirk #151: the CLI passes `"mm"` and nothing else, so on the `-drc` path only the first
    /// branch runs.
    pub fn convert_coordinate(&self, board_coordinate: f64, coordinate_unit: &str) -> f64 {
        // DesignRulesChecker.java:506-508.
        let dsn_coordinate = self.transform.board_to_dsn(board_coordinate);

        // DesignRulesChecker.java:513-524.
        let target_unit = match coordinate_unit {
            "mm" => Unit::Mm,
            "mil" => Unit::Mil,
            "inch" => Unit::Inch,
            "um" => Unit::Um,
            _ => self.board_unit,
        };

        // DesignRulesChecker.java:526-529.
        if target_unit != self.board_unit {
            return Unit::scale(dsn_coordinate, self.board_unit, target_unit);
        }
        dsn_coordinate
    }
}

impl DesignRulesChecker<'_> {
    /// Port of `generateReport(String, String)` (DesignRulesChecker.java:210-289): the whole DRC
    /// report, in Java's body order — which is why `violations` interleaves two sources.
    ///
    /// 1. The header (`:212-214`).
    /// 2. `getAllClearanceViolations()` (`:216`), each converted and appended to `violations`
    ///    (`:229-231`).
    /// 3. **Then** `getAllUnconnectedItems()` (`:238`), each converted and routed: the two
    ///    dangling kinds go to `violations` too, everything else to `unconnectedItems`
    ///    (`:268-276`).
    ///
    /// So `violations` is *all* clearance entries followed by *all* dangling entries, each in its
    /// own list's order — on the dev-board fixture, two `holeClearance` then eight
    /// `track_dangling`.
    ///
    /// `&mut self` because step 2 is (plan-5 ruling 8): every clearance query lowers an item's
    /// `smallestClearance` and advances the search tree's entry counter.
    pub fn generate_report(
        &mut self,
        coords: &DrcCoordinates,
        options: &DrcReportOptions,
    ) -> KiCadDrcReport {
        // DesignRulesChecker.java:211-214. Java's `qualityScore` is assigned by the caller
        // afterwards (Freerouting.java:349); the port takes it with the rest (ruling 5).
        let mut report = KiCadDrcReport::new(
            &options.coordinate_unit,
            &options.source,
            format!("Freerouting {}", options.freerouting_version),
            &options.date,
        );
        report.quality_score = options.quality_score;

        // DesignRulesChecker.java:216.
        let violations = self.get_all_clearance_violations();

        // DesignRulesChecker.java:229-231.
        for violation in &violations {
            report.add_violation(convert_clearance_violation(
                self.board,
                violation,
                coords,
                &options.coordinate_unit,
            ));
        }

        // DesignRulesChecker.java:238.
        let unconnected_items = self.get_all_unconnected_items();

        // DesignRulesChecker.java:249-277.
        for unconnected_item in &unconnected_items {
            let entry = convert_unconnected_items(
                self.board,
                unconnected_item,
                coords,
                &options.coordinate_unit,
            );
            // DesignRulesChecker.java:268-276.
            match unconnected_item.kind {
                UnconnectedKind::TrackDangling | UnconnectedKind::ViaDangling => {
                    report.add_violation(entry);
                }
                UnconnectedKind::UnconnectedItems => report.add_unconnected_item(entry),
            }
        }

        // DesignRulesChecker.java:288.
        report
    }
}

/// Port of `convertToDrcViolation(ClearanceViolation, String)` (DesignRulesChecker.java:299-355).
fn convert_clearance_violation(
    board: &Board,
    violation: &ClearanceViolation,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> KiCadDrcViolation {
    // DesignRulesChecker.java:304-305.
    let first_item_desc = item_description(board, violation.first_item);
    let second_item_desc = item_description(board, violation.second_item);

    // DesignRulesChecker.java:308-319. Java's comment says "the center of gravity of the violation
    // shape"; the code takes each **item's bounding box**' centre instead. The violation shape's
    // own centre appears only in the dropped `FRLogger.trace` at `:265`.
    let first_item_pos = item_position(board, violation.first_item, coords, coordinate_unit);
    let second_item_pos = item_position(board, violation.second_item, coords, coordinate_unit);

    // DesignRulesChecker.java:321-326.
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

    // DesignRulesChecker.java:329-336.
    let kind = if is_hole(board, violation.first_item) || is_hole(board, violation.second_item) {
        "holeClearance"
    } else {
        "clearance"
    };

    // DesignRulesChecker.java:339-354. Both clearances go through `convertCoordinate`: they are
    // board-unit *lengths*, and the report's unit is the one the coordinates are in. The two arms
    // differ only in the leading word.
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

    // DesignRulesChecker.java:354.
    KiCadDrcViolation::new(kind, description, "error", items)
}

/// Port of `convertToDrcViolation(UnconnectedItems, String)` (DesignRulesChecker.java:359-421).
fn convert_unconnected_items(
    board: &Board,
    unconnected_items: &UnconnectedItems,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> KiCadDrcViolation {
    // DesignRulesChecker.java:365-394: the two dangling kinds return early, with a single item.
    if matches!(
        unconnected_items.kind,
        UnconnectedKind::TrackDangling | UnconnectedKind::ViaDangling
    ) {
        let item = unconnected_items.first_item;
        // DesignRulesChecker.java:369-376.
        let item_desc = match unconnected_items.kind {
            UnconnectedKind::ViaDangling => item_description(board, item),
            // `getDetailedTraceDescription` is reached from here and nowhere else.
            _ => detailed_trace_description(board, item, coords, coordinate_unit),
        };
        let items = vec![KiCadDrcViolationItem::new(
            &item_desc,
            item_position(board, item, coords, coordinate_unit),
            item.0.to_string(),
        )];

        // DesignRulesChecker.java:386-391.
        //
        // not ported: the `switch`'s `default -> "Unconnected item: " + itemDesc` arm
        // (DesignRulesChecker.java:390) — dead code. The `switch` runs only inside this branch,
        // whose guard already narrowed `type` to the two literals the arms above cover, so the
        // third arm cannot be reached. Recorded alongside quirk #146.
        let description = match unconnected_items.kind {
            UnconnectedKind::ViaDangling => "Via is not connected or connected on only one layer",
            _ => "Track has unconnected end",
        };
        // DesignRulesChecker.java:393.
        return KiCadDrcViolation::new(
            head_kind_string(unconnected_items.kind),
            description,
            "warning",
            items,
        );
    }

    // DesignRulesChecker.java:398-406: every item of the two disconnected groups, in `allItems`
    // order — ascending id in the port (plan-5 ruling 3), hash-ordered in Java (quirk #144).
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

    // DesignRulesChecker.java:410-419.
    let from_item_desc = item_description(board, unconnected_items.first_item);
    let description = match unconnected_items.second_item {
        Some(second) => format!(
            "Unconnected items: {from_item_desc} and {} ({} total items in net)",
            item_description(board, second),
            unconnected_items.all_items.len(),
        ),
        None => format!("Unconnected item: {from_item_desc}"),
    };

    // DesignRulesChecker.java:420.
    KiCadDrcViolation::new(
        head_kind_string(unconnected_items.kind),
        description,
        "warning",
        items,
    )
}

/// Java's `UnconnectedItems.type` string, in the HEAD spelling (plan-5 ruling 1):
/// `"unconnectedItems"` (UnconnectedItems.java:31, `:36`), `"track_dangling"`
/// (DesignRulesChecker.java:161) and `"via_dangling"` (`:172`).
///
/// Ruling 2's `KiCad` flavor renames the first of the three to `unconnected_items`; that key table
/// is **Task 8's** and lives with the serialiser, because Java stores the HEAD spelling in the DTO
/// and only Gson's `@SerializedName`s differ between the two jars.
fn head_kind_string(kind: UnconnectedKind) -> &'static str {
    match kind {
        UnconnectedKind::UnconnectedItems => "unconnectedItems",
        UnconnectedKind::TrackDangling => "track_dangling",
        UnconnectedKind::ViaDangling => "via_dangling",
    }
}

/// Port of the private `isHole` (DesignRulesChecker.java:424-430): a [`ItemKind::Via`] **or** a
/// [`ItemKind::Pin`].
///
// Java bug: DesignRulesChecker.isHole (DesignRulesChecker.java:426-429) classifies **every** `Pin` as a hole, surface-mount pads included — its own comment admits it ("Pins are treated as holes for DRC classification to match expected output, although this might include SMT pins"). A pad with no drill has no hole clearance to violate, so KiCad would call the same violation `clearance`. Reproduced, because it decides the `type` string *and* the description's first word. Quirks row #152.
fn is_hole(board: &Board, id: ItemId) -> bool {
    matches!(
        board.get_item(id).map(Item::kind),
        Some(ItemKind::Via | ItemKind::Pin),
    )
}

/// Port of the private `getItemDescription` (DesignRulesChecker.java:439-460): the item's kind,
/// then `" [<net name>]"` when it carries a net.
///
/// **Public here although Java's is private**, so that `tests/report.rs` can pin the whole variant
/// table: the `else` arm is Java's `getClass().getSimpleName()` over five `Item` subclasses that no
/// fixture in the corpus puts into a violation, and an untested table rots.
///
// totalized: getItemDescription's net lookup (DesignRulesChecker.java:457) — Java writes `board.rules.nets.get(item.getNetNumber(0)).name` with no null check, so a net number the `Nets` table does not know throws `NullPointerException`. The port omits the suffix instead. Unreachable from `fr-dsn`, whose reader registers every net it assigns.
pub fn item_description(board: &Board, id: ItemId) -> String {
    let Some(item) = board.get_item(id) else {
        // Java dereferences the `Item` it was handed; an id this board does not know cannot reach
        // here from any caller in this module.
        return String::new();
    };

    // DesignRulesChecker.java:442-453. The chain tests `Trace`, `Via`, `Pin`, `ConductionArea`,
    // then falls back to the Java simple class name — which for the five remaining `Item`
    // subclasses is the variant name verbatim. `Trace` is abstract in Java and the concrete class
    // is `PolylineTrace`, but the `instanceof Trace` arm catches it first.
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
        // `Item.getBoardItemType`'s `OTHER` is unreachable from `Item::kind` (items/mod.rs:120-124);
        // Java's `getSimpleName()` would print the subclass's own name, which there is none of.
        ItemKind::Other => "Item".to_string(),
    };

    // DesignRulesChecker.java:456-459.
    if item.net_count() > 0
        && let Some(net) = board.rules.nets.get(item.get_net_number(0))
    {
        desc.push_str(" [");
        desc.push_str(&net.name);
        desc.push(']');
    }

    desc
}

/// Port of the private `getDetailedTraceDescription` (DesignRulesChecker.java:470-493): `"Track"`,
/// the net, the layer name and the length — used **only** for `track_dangling` (`:374`).
///
/// The leading word is `"Track"`, not `"Trace"`: this helper does not call
/// [`item_description`] and spells the kind itself (`:471`).
fn detailed_trace_description(
    board: &Board,
    id: ItemId,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> String {
    // DesignRulesChecker.java:471.
    let mut desc = "Track".to_string();
    let Some(item) = board.get_item(id) else {
        return desc;
    };

    // DesignRulesChecker.java:474-478 — the same net suffix as `getItemDescription`, written out
    // a second time in Java. See that method's `totalized:` marker for the null-net arm.
    if item.net_count() > 0
        && let Some(net) = board.rules.nets.get(item.get_net_number(0))
    {
        desc.push_str(" [");
        desc.push_str(&net.name);
        desc.push(']');
    }

    // DesignRulesChecker.java:481-491: `instanceof Trace`, so a non-trace stops at the net suffix.
    // `getAllUnconnectedItems` only ever hands this method a trace (`:153`, `:161`).
    if let Item::Trace(trace) = item {
        let layer_name = board
            .layer_structure()
            .layers
            .get(trace.get_layer())
            .map(|layer| layer.name.as_str())
            .unwrap_or_default();
        desc.push_str(" on ");
        desc.push_str(layer_name);

        // DesignRulesChecker.java:486-490. `String.format("%.4f", …)` — locale-free here
        // (plan-5 ruling 6).
        let length = format_length(trace.get_length(), coords, coordinate_unit);
        desc.push_str(", length ");
        desc.push_str(&length);
        desc.push(' ');
        desc.push_str(coordinate_unit);
    }

    desc
}

/// `item.boundingBox().centreOfGravity()` (DesignRulesChecker.java:305, `:310`, `:373`, `:392`)
/// with [`DrcCoordinates::convert_coordinate`] applied to each half.
///
/// `IntBox` inherits `centreOfGravity` from `PolylineShape` (PolylineShape.java:119-133) — the
/// arithmetic mean of the corners, which for a box is its centre.
fn item_position(
    board: &Board,
    id: ItemId,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> KiCadDrcPosition {
    let Some(item) = board.get_item(id) else {
        return KiCadDrcPosition::new(0.0, 0.0);
    };
    let centre = TileShape::Box(item.bounding_box(&board.ctx())).centre_of_gravity();
    KiCadDrcPosition::new(
        coords.convert_coordinate(centre.x, coordinate_unit),
        coords.convert_coordinate(centre.y, coordinate_unit),
    )
}

/// A board-unit length through `convertCoordinate` and then `%.4f`
/// (DesignRulesChecker.java:341-352, `:487-488`).
///
/// `java_format_fixed` reproduces `java.util.Formatter`'s HALF_UP rounding of the *shortest*
/// round-trip digits, and writes `.` whatever the machine's locale is — Java's own output is
/// locale-dependent here (plan-5 ruling 6, quirk #145).
fn format_length(board_length: f64, coords: &DrcCoordinates, coordinate_unit: &str) -> String {
    java_format_fixed(coords.convert_coordinate(board_length, coordinate_unit), 4)
}
