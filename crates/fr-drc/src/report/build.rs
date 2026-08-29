//! `DesignRulesChecker.generateReport` and the five private helpers that fill the DTOs.
//!
//! Java: `drc/DesignRulesChecker.java:210-290` (`generateReport`), `:299-357` and `:359-422` (the
//! two `convertToDrcViolation` overloads), `:424-431` (`isHole`), `:439-461`
//! (`getItemDescription`), `:470-495` (`getDetailedTraceDescription`), `:504-533`
//! (`convertCoordinate`).
//!
//! Every `FRLogger.trace` call in that range is dropped (four of them, `:218-228`, `:235-255`,
//! `:261-266`, `:279-287`), per the plan's global constraints. One of them is the *only* place
//! `ClearanceViolation.shape` is read in the whole report path (`:255`) — the positions the report
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
    /// `Freerouting.java:335-336`: the CLI hard-codes `"mm"` and offers no way to change it
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
    /// `Freerouting.java:349`: `(double) finalStats.getNormalizedScore(routerSettings.scoring)`.
    ///
    /// **`f32`, not `f64`, and the widening is this crate's.** `BoardStatistics.getNormalizedScore`
    /// returns a `float` (BoardStatistics.java:624) and the report's field is a `Double`
    /// (KiCadDrcReport.java:56-57), so *every* score Java can put in a DRC document is a widened
    /// `float` — which is where the `.078369140625` tails come from. Typing the injected value
    /// `f64` would let a caller hand in a number Java could never produce (`902.0783691`), and
    /// `Double.toString` would render it, so the port would emit a document no jar can match.
    /// Taking an `f32` and performing `Freerouting.java:349`'s cast here makes that
    /// unrepresentable. Plan 8 passes `getNormalizedScore`'s return value straight through.
    ///
    /// `None` is Java's `null`, which Gson omits from the JSON; `BoardStatistics` itself is
    /// Plan 8's (plan-5 ruling 5).
    pub quality_score: Option<f32>,
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
    /// Port of the private `convertCoordinate` (DesignRulesChecker.java:504-533): board units to
    /// DSN units, then — only when the named unit differs from the board's — [`Unit::scale`].
    ///
    /// The name→[`Unit`] chain (`:512-525`) is `"mm" | "mil" | "inch" | "um"`, anything else the
    /// board's own unit. It is **not** `Unit::from_string` (Unit.java:23-33): that one
    /// upper-cases first, so Java's `convertCoordinate` rejects `"MM"` where `from_string` accepts
    /// it. Transcribed as Java wrote it.
    ///
    /// Quirk #151: the CLI passes `"mm"` and nothing else, so on the `-drc` path only the first
    /// branch runs.
    pub fn convert_coordinate(&self, board_coordinate: f64, coordinate_unit: &str) -> f64 {
        // DesignRulesChecker.java:505-507.
        let dsn_coordinate = self.transform.board_to_dsn(board_coordinate);

        // DesignRulesChecker.java:512-525.
        let target_unit = match coordinate_unit {
            "mm" => Unit::Mm,
            "mil" => Unit::Mil,
            "inch" => Unit::Inch,
            "um" => Unit::Um,
            _ => self.board_unit,
        };

        // DesignRulesChecker.java:527-530.
        if target_unit != self.board_unit {
            return Unit::scale(dsn_coordinate, self.board_unit, target_unit);
        }
        dsn_coordinate
    }
}

impl DesignRulesChecker<'_> {
    /// Port of `generateReport(String, String)` (DesignRulesChecker.java:210-290): the whole DRC
    /// report, in Java's body order — which is why `violations` interleaves two sources.
    ///
    /// 1. The header (`:211-213`).
    /// 2. `getAllClearanceViolations()` (`:216`), each converted and appended to `violations`
    ///    (`:231-233`).
    /// 3. **Then** `getAllUnconnectedItems()` (`:259`), each converted and routed: the two
    ///    dangling kinds go to `violations` too, everything else to `unconnectedItems`
    ///    (`:271-276`).
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
        // DesignRulesChecker.java:211-213. Java's `qualityScore` is assigned by the caller
        // afterwards (Freerouting.java:349); the port takes it with the rest (ruling 5).
        let mut report = KiCadDrcReport::new(
            &options.coordinate_unit,
            &options.source,
            format!("Freerouting {}", options.freerouting_version),
            &options.date,
        );
        // `Freerouting.java:349`'s `(double)` cast, performed here so the DTO's field keeps
        // Java's `Double` type while only float-representable values can reach it.
        report.quality_score = options.quality_score.map(f64::from);

        // DesignRulesChecker.java:216.
        let violations = self.get_all_clearance_violations();

        // DesignRulesChecker.java:231-233.
        for violation in &violations {
            report.add_violation(convert_clearance_violation(
                self.board,
                violation,
                coords,
                &options.coordinate_unit,
            ));
        }

        // DesignRulesChecker.java:259.
        let unconnected_items = self.get_all_unconnected_items();

        // DesignRulesChecker.java:269-277.
        for unconnected_item in &unconnected_items {
            let entry = convert_unconnected_items(
                self.board,
                unconnected_item,
                coords,
                &options.coordinate_unit,
            );
            // DesignRulesChecker.java:271-276.
            match unconnected_item.kind {
                UnconnectedKind::TrackDangling | UnconnectedKind::ViaDangling => {
                    report.add_violation(entry);
                }
                UnconnectedKind::UnconnectedItems => report.add_unconnected_item(entry),
            }
        }

        // DesignRulesChecker.java:289.
        report
    }
}

/// Port of `convertToDrcViolation(ClearanceViolation, String)` (DesignRulesChecker.java:299-357).
fn convert_clearance_violation(
    board: &Board,
    violation: &ClearanceViolation,
    coords: &DrcCoordinates,
    coordinate_unit: &str,
) -> KiCadDrcViolation {
    // DesignRulesChecker.java:304-305.
    let first_item_desc = item_description(board, violation.first_item);
    let second_item_desc = item_description(board, violation.second_item);

    // DesignRulesChecker.java:307-317. Java's comment says "the center of gravity of the violation
    // shape"; the code takes each **item's bounding box**' centre instead. The violation shape's
    // own centre appears only in the dropped `FRLogger.trace` at `:255`.
    let first_item_pos = item_position(board, violation.first_item, coords, coordinate_unit);
    let second_item_pos = item_position(board, violation.second_item, coords, coordinate_unit);

    // DesignRulesChecker.java:319-324.
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

    // DesignRulesChecker.java:326-330.
    let kind = if is_hole(board, violation.first_item) || is_hole(board, violation.second_item) {
        "holeClearance"
    } else {
        "clearance"
    };

    // DesignRulesChecker.java:332-354. Both clearances go through `convertCoordinate`: they are
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

    // DesignRulesChecker.java:356.
    KiCadDrcViolation::new(kind, description, "error", items)
}

/// Port of `convertToDrcViolation(UnconnectedItems, String)` (DesignRulesChecker.java:359-422).
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
        // DesignRulesChecker.java:368-376.
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

        // DesignRulesChecker.java:387-392.
        //
        // not ported: the `switch`'s `default -> "Unconnected item: " + itemDesc` arm (DesignRulesChecker.java:391) — dead code.
        // The `switch` runs only inside this branch, whose guard already narrowed `type` to the
        // two literals the arms above cover, so the third arm cannot be reached. Recorded
        // alongside quirk #146. (`audit-port.sh` is line-based, so the citation sits on the
        // marker's own line — `docs/java-quirks.md` §Process notes.)
        let description = match unconnected_items.kind {
            UnconnectedKind::ViaDangling => "Via is not connected or connected on only one layer",
            _ => "Track has unconnected end",
        };
        // DesignRulesChecker.java:394.
        return KiCadDrcViolation::new(
            head_kind_string(unconnected_items.kind),
            description,
            "warning",
            items,
        );
    }

    // DesignRulesChecker.java:397-408: every item of the two disconnected groups, in `allItems`
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

    // DesignRulesChecker.java:421.
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
/// lives with the serialiser (`report/json.rs`'s `HEAD`/`KICAD` rows and
/// `FlavorKeys::violation_type`), because Java stores the HEAD spelling in the DTO and only
/// Gson's `@SerializedName`s differ between the two jars.
fn head_kind_string(kind: UnconnectedKind) -> &'static str {
    match kind {
        UnconnectedKind::UnconnectedItems => "unconnectedItems",
        UnconnectedKind::TrackDangling => "track_dangling",
        UnconnectedKind::ViaDangling => "via_dangling",
    }
}

/// Port of the private `isHole` (DesignRulesChecker.java:424-431): a [`ItemKind::Via`] **or** a
/// [`ItemKind::Pin`].
///
// Java bug: DesignRulesChecker.isHole (DesignRulesChecker.java:425-430) classifies **every** `Pin` as a hole, surface-mount pads included — its own comment admits it ("Pins are treated as holes for DRC classification to match expected output, although this might include SMT pins"). A pad with no drill has no hole clearance to violate, so KiCad would call the same violation `clearance`. Reproduced, because it decides the `type` string *and* the description's first word. Quirks row #152.
fn is_hole(board: &Board, id: ItemId) -> bool {
    matches!(
        board.get_item(id).map(Item::kind),
        Some(ItemKind::Via | ItemKind::Pin),
    )
}

/// Port of the private `getItemDescription` (DesignRulesChecker.java:439-461): the item's kind,
/// then `" [<net name>]"` when it carries a net.
///
/// **Public here although Java's is private**, so that `tests/report.rs` can pin the whole variant
/// table: the `else` arm is Java's `getClass().getSimpleName()` over five `Item` subclasses that no
/// fixture in the corpus puts into a violation, and an untested table rots.
///
// totalized: getItemDescription's net lookup (DesignRulesChecker.java:456) — Java writes `board.rules.nets.get(item.getNetNumber(0)).name` with no null check, so a net number the `Nets` table does not know throws `NullPointerException`. The port omits the suffix instead. Unreachable from `fr-dsn`, whose reader registers every net it assigns.
pub fn item_description(board: &Board, id: ItemId) -> String {
    // Java is handed the `Item` itself and dereferences it, so an id the board does not know is a
    // `NullPointerException` there. Every id reaching here came out of this board's own violation
    // or unconnected list, so the arm is unreachable — and it panics rather than returning a
    // plausible-looking empty description, which would put a silently wrong string into a parity
    // document. `tests/corpus.rs` runs the whole path over all 105 corpus fixtures.
    let item = board
        .get_item(id)
        .unwrap_or_else(|| panic!("item_description: board has no item {}", id.0));

    // DesignRulesChecker.java:442-452. The chain tests `Trace`, `Via`, `Pin`, `ConductionArea`,
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

    // DesignRulesChecker.java:454-458.
    if item.net_count() > 0
        && let Some(net) = board.rules.nets.get(item.get_net_number(0))
    {
        desc.push_str(" [");
        desc.push_str(&net.name);
        desc.push(']');
    }

    desc
}

/// Port of the private `getDetailedTraceDescription` (DesignRulesChecker.java:470-495): `"Track"`,
/// the net, the layer name and the length — used **only** for `track_dangling` (`:375`).
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
    // As in `item_description`: unreachable, and a panic rather than a truncated `"Track"`.
    let item = board
        .get_item(id)
        .unwrap_or_else(|| panic!("detailed_trace_description: board has no item {}", id.0));

    // DesignRulesChecker.java:473-477 — the same net suffix as `getItemDescription`, written out
    // a second time in Java. See that method's `totalized:` marker for the null-net arm.
    if item.net_count() > 0
        && let Some(net) = board.rules.nets.get(item.get_net_number(0))
    {
        desc.push_str(" [");
        desc.push_str(&net.name);
        desc.push(']');
    }

    // DesignRulesChecker.java:479-491: `instanceof Trace`, so a non-trace stops at the net suffix.
    // `getAllUnconnectedItems` only ever hands this method a trace (`:153`, `:161`).
    if let Item::Trace(trace) = item {
        // totalized: getDetailedTraceDescription's layer lookup (DesignRulesChecker.java:482) — Java indexes `board.layerStructure.layers[layer]` unguarded, so a trace whose layer is outside the board's stack throws `ArrayIndexOutOfBoundsException`. The port writes an empty layer name instead. Near-unreachable: the `Trace` constructor clamps the layer to `layerCount - 1` (Trace.java:45-47), so only `Trace.setLayer` (Trace.java:71-73), which does not clamp, can push one out of range — and nothing in the DRC path calls it.
        let layer_name = board
            .layer_structure()
            .layers
            .get(trace.get_layer())
            .map(|layer| layer.name.as_str())
            .unwrap_or_default();
        desc.push_str(" on ");
        desc.push_str(layer_name);

        // DesignRulesChecker.java:485-491. `String.format("%.4f", …)` — locale-free here
        // (plan-5 ruling 6).
        let length = format_length(trace.get_length(), coords, coordinate_unit);
        desc.push_str(", length ");
        desc.push_str(&length);
        desc.push(' ');
        desc.push_str(coordinate_unit);
    }

    desc
}

/// `item.boundingBox().centreOfGravity()` (DesignRulesChecker.java:308, `:313`, `:378`, `:401`)
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
    // As in `item_description`: unreachable — every id here came out of the board's own violation
    // or unconnected list, and Java holds the `Item` and calls `boundingBox()` on it. A panic
    // rather than a `(0.0, 0.0)` that would read as a real coordinate.
    let item = board
        .get_item(id)
        .unwrap_or_else(|| panic!("item_position: board has no item {}", id.0));
    let centre = TileShape::Box(item.bounding_box(&board.ctx())).centre_of_gravity();
    KiCadDrcPosition::new(
        coords.convert_coordinate(centre.x, coordinate_unit),
        coords.convert_coordinate(centre.y, coordinate_unit),
    )
}

/// A board-unit length through `convertCoordinate` and then `%.4f`
/// (DesignRulesChecker.java:340-352, `:487-489`).
///
/// `java_format_fixed` reproduces `java.util.Formatter`'s HALF_UP rounding of the *shortest*
/// round-trip digits, and writes `.` whatever the machine's locale is — Java's own output is
/// locale-dependent here (plan-5 ruling 6, quirk #145).
fn format_length(board_length: f64, coords: &DrcCoordinates, coordinate_unit: &str) -> String {
    java_format_fixed(coords.convert_coordinate(board_length, coordinate_unit), 4)
}
