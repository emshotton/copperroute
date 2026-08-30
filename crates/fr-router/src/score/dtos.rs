//! The ten `core/scoring/BoardStatistics*.java` data-transfer objects the score is assembled
//! from, plus the `java.awt.geom.Rectangle2D.Float` two of them carry.
//!
//! Each is a Java class of `@SerializedName` fields and **no methods** — which is why they arrive
//! here as plain structs and why `scripts/audit-port.sh core/scoring` finds no method rows for
//! them, only the map lines that name their Rust home.
//!
//! Every field is an `Option`, because Java's are boxed (`Integer`, `Float`, `Double`) and start
//! `null`; Gson omits a null field, and Plan 8's JSON surface will need that distinction back.
//! [`Default`] is that all-null state. The one exception is
//! [`BoardStatisticsFanout`](crate::score::BoardStatisticsFanout), whose three fields are
//! primitive `int` in Java and therefore start at `0` — it lives in `statistics.rs` beside the
//! block that fills it, exactly as Java nests it inside `BoardStatistics`.
//!
//! The same convention `fr-drc` established for `BoardStatisticsClearanceViolations`
//! (`crates/fr-drc/src/statistics.rs`), which is the eleventh DTO and is **not** redeclared here:
//! Plan 5 delivered it, and [`BoardStatistics`](crate::score::BoardStatistics) re-uses it.

/// Port of `java.awt.geom.Rectangle2D.Float` as far as `BoardStatisticsBoard` uses it: four
/// `float`s, constructed as `Rectangle2D.Float(x, y, w, h)`.
///
/// **The names lie, and that is Java's doing, not a transcription slip** (quirk #196). The computing
/// constructor builds `boundingBox` as `new Rectangle2D.Float(ur.x, ur.y, ll.x, ll.y)`
/// (BoardStatistics.java:127-131), so `width` and `height` hold the board's *lower-left corner*,
/// not a width and a height — and on every corpus board they are therefore negative.
/// `board.size` (`:132-137`) is the rectangle that really does carry a width and a height.
/// Reproduced verbatim: `Unit.scale` is applied to all four fields at `:381-386` and the
/// manifest Plan 8 writes carries whatever this holds.
///
/// not ported: the rest of `Rectangle2D.Float` (the shape algebra, `setRect`, `outcode`, …) —
/// `BoardStatistics` constructs it, reads its four fields and never asks it a geometric question.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rectangle2DFloat {
    /// Java `Rectangle2D.Float.x`.
    pub x: f32,
    /// Java `Rectangle2D.Float.y`.
    pub y: f32,
    /// Java `Rectangle2D.Float.width`.
    pub width: f32,
    /// Java `Rectangle2D.Float.height`.
    pub height: f32,
}

/// Port of `core.scoring.BoardStatisticsBoard` (BoardStatisticsBoard.java:8-15).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsBoard {
    /// Java `boundingBox` (`:11-12`). See [`Rectangle2DFloat`] for what its four fields hold.
    pub bounding_box: Option<Rectangle2DFloat>,
    /// Java `size` (`:14-15`).
    pub size: Option<Rectangle2DFloat>,
}

/// Port of `core.scoring.BoardStatisticsLayers` (BoardStatisticsLayers.java:7-14).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsLayers {
    /// Java `totalCount` (`:9-10`).
    pub total_count: Option<i32>,
    /// Java `signalCount` (`:12-13`).
    pub signal_count: Option<i32>,
}

/// Port of `core.scoring.BoardStatisticsItems` (BoardStatisticsItems.java:7-32).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsItems {
    /// Java `totalCount` (`:9-10`).
    pub total_count: Option<i32>,
    /// Java `traceCount` (`:12-13`).
    pub trace_count: Option<i32>,
    /// Java `viaCount` (`:15-16`).
    pub via_count: Option<i32>,
    /// Java `conductionAreaCount` (`:18-19`).
    pub conduction_area_count: Option<i32>,
    /// Java `drillItemCount` (`:21-22`).
    ///
    /// Always `0` on a board this port can build: the classification chain
    /// (BoardStatistics.java:161-173) tests `Via` and `Pin` **before** `DrillItem`, and those two
    /// are the only concrete `DrillItem` subclasses in the Java tree.
    pub drill_item_count: Option<i32>,
    /// Java `pinCount` (`:24-25`).
    pub pin_count: Option<i32>,
    /// Java `componentOutlineCount`, serialised as `component_count` (`:27-28`).
    pub component_outline_count: Option<i32>,
    /// Java `otherCount` (`:30-31`).
    pub other_count: Option<i32>,
}

/// Port of `core.scoring.BoardStatisticsComponents` (BoardStatisticsComponents.java:7-11).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsComponents {
    /// Java `totalCount` (`:9-10`).
    pub total_count: Option<i32>,
}

/// Port of `core.scoring.BoardStatisticsPads` (BoardStatisticsPads.java:7-11).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsPads {
    /// Java `totalCount` (`:9-10`).
    pub total_count: Option<i32>,
}

/// Port of `core.scoring.BoardStatisticsNets` (BoardStatisticsNets.java:7-14).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsNets {
    /// Java `totalCount` (`:9-10`) — `board.rules.nets.maxNetNumber()`, i.e. the largest net
    /// number rather than a count of the nets that carry items.
    pub total_count: Option<i32>,
    /// Java `classCount` (`:12-13`).
    pub class_count: Option<i32>,
}

/// Port of `core.scoring.BoardStatisticsConnections` (BoardStatisticsConnections.java:7-14).
///
/// Both fields stay `None` when the computing constructor runs with
/// `includeConnections = false` (BoardStatistics.java:265-271) — and
/// [`BoardStatistics::maximum_score`](crate::score::BoardStatistics::maximum_score) would then
/// unbox a null `Integer` in Java, so the port's `None` is a panic there too, deliberately.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsConnections {
    /// Java `maximumCount` (`:9-10`) — `DesignRulesChecker.maxConnections`.
    pub maximum_count: Option<i32>,
    /// Java `incompleteCount` (`:12-13`) — `DesignRulesChecker.getIncompleteCount()`.
    pub incomplete_count: Option<i32>,
}

/// Port of `core.scoring.BoardStatisticsTraces` (BoardStatisticsTraces.java:7-49).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsTraces {
    /// Java `totalCount` (`:11-12`).
    pub total_count: Option<i32>,
    /// Java `totalSegmentCount` (`:15-16`).
    pub total_segment_count: Option<i32>,
    /// Java `totalLength` (`:19-20`), in **raw board units**.
    pub total_length: Option<f32>,
    /// Java `totalLengthMm` (`:30-31`): [`Self::total_length`] normalised to millimetres by the
    /// HEAD-only correction at BoardStatistics.java:190-203. This is the length
    /// [`calculate_score`](crate::score::BoardStatistics::calculate_score) multiplies by
    /// `defaultPreferredDirectionTraceCost`.
    pub total_length_mm: Option<f32>,
    /// Java `totalWeightedLength` (`:33-34`) — length × (half width + clearance), halved for a
    /// `SHOVE_FIXED` trace. `BatchOptimizer.optRoutePass` reads it as `minCumulativeTraceLength`,
    /// so it is a routing decision rather than a report number.
    pub total_weighted_length: Option<f32>,
    /// Java `averageLength` (`:37-38`).
    pub average_length: Option<f32>,
    /// Java `totalVerticalLength` (`:41-42`).
    pub total_vertical_length: Option<f32>,
    /// Java `totalHorizontalLength` (`:45-46`).
    pub total_horizontal_length: Option<f32>,
    /// Java `totalAngledLength` (`:48-49`).
    pub total_angled_length: Option<f32>,
}

/// Port of `core.scoring.BoardStatisticsBends` (BoardStatisticsBends.java:7-20).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsBends {
    /// Java `totalCount` (`:9-10`).
    pub total_count: Option<i32>,
    /// Java `ninetyDegreeCount`, serialised as `90_degree_count` (`:12-13`).
    pub ninety_degree_count: Option<i32>,
    /// Java `fortyFiveDegreeCount`, serialised as `45_degree_count` (`:15-16`).
    pub forty_five_degree_count: Option<i32>,
    /// Java `otherAngleCount` (`:18-19`).
    pub other_angle_count: Option<i32>,
}

/// Port of `core.scoring.BoardStatisticsVias` (BoardStatisticsVias.java:7-24).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsVias {
    /// Java `totalCount` (`:10-11`).
    pub total_count: Option<i32>,
    /// Java `throughHoleCount` (`:14-15`).
    pub through_hole_count: Option<i32>,
    /// Java `blindCount` (`:18-19`).
    pub blind_count: Option<i32>,
    /// Java `buriedCount` (`:22-23`).
    pub buried_count: Option<i32>,
}
