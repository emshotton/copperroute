//! `BoardStatistics.toString` (core/scoring/BoardStatistics.java:588-591) —
//! `GsonProvider.GSON.toJson(this)`, i.e. the Gson-compatible JSON surface of the whole
//! `core/scoring` family (controller ruling AG).
//!
//! # What Gson does, and where each half of it lives
//!
//! `GsonProvider.GSON` (util/gson/GsonProvider.java:13-20) is
//! `new GsonBuilder().setPrettyPrinting().disableHtmlEscaping()…setStrictness(LENIENT).create()`.
//! Plan 4 solved the *formatting* half once — two-space indent, `": "` after every key, no
//! trailing newline, `Float.toString`/`Double.toString` numbers, `U+2028`/`U+2029` escaped and
//! nothing else — and Plan 5 moved it to [`fr_dsn::format::json::to_gson_string_pretty`], which
//! the DRC report already writes through. **This module adds no formatter**; it adds the
//! *shape*: the key names, the key order and the null omission that Gson's reflective adapter
//! derives from the Java field declarations.
//!
//! Four facts, all read out of the Java field blocks rather than guessed, and all pinned by
//! `P8T2Probe`'s three `synth:` rows:
//!
//! 1. **Declaration order is key order.** `BoardStatistics.java:37-79` gives `host`, `unit`,
//!    `board`, `layers`, `items`, `components`, `pads`, `nets`, `connections`, `traces`, `bends`,
//!    `vias`, `clearance_violations`, `fanout` — fourteen fields, twelve of them DTOs (scan
//!    ruling R16). Each DTO's own order comes from its own file.
//! 2. **A null field is omitted.** `GsonBuilder` is never given `serializeNulls()`, so every
//!    `Integer`/`Float`/`Double` field that is still `null` disappears; a DTO whose fields are
//!    all null serialises as `{}`. All twelve DTO *objects* are non-null always, because
//!    `BoardStatistics`' field initialisers construct them (`:43-78`), so all twelve keys are
//!    always present.
//! 3. **`BoardStatisticsFanout` is the exception.** Its three fields are primitive `int`
//!    (`:638-646`), so they start at `0` and Gson emits `0` rather than omitting them — which is
//!    why the port's [`fr_router::score::BoardStatisticsFanout`] holds plain `i32`s and why this
//!    module writes all three unconditionally.
//! 4. **Two names are not the field names.** `BoardStatisticsBends.ninetyDegreeCount` is
//!    `@SerializedName("90_degree_count")` and `fortyFiveDegreeCount` is `"45_degree_count"` —
//!    both legal JSON keys that begin with a digit — and
//!    `BoardStatisticsItems.componentOutlineCount` is `@SerializedName("component_count")`, which
//!    collides with `components.total_count` two objects later. Both are transcribed from
//!    `BoardStatisticsBends.java:12-16` and `BoardStatisticsItems.java:27-28`.
//!
//! # `host` and `unit`: the empty string is Java's `null`
//!
//! `fr_router::score::BoardStatistics`' two string fields are `String`, not `Option<String>` —
//! the type is `fr-router`'s and plan 8 makes only additive changes to that crate — so this
//! module reads the empty string as Java's `null` and omits the key. The computing constructor
//! can never produce an empty one (`:113-117` builds `hostCad + "," + hostVersion`, minimum
//! `null,null`, quirk #249) and the scraper produces one only from `(parser (hostCad  ))`, which
//! quirk #251 records as the single input where Java prints `"host": ""` and this port omits it.
//!
//! # Three entry points, and which one Tasks 4 and 9 want
//!
//! [`to_gson_string`] is `toString()` byte for byte. [`GsonBoardStatistics`] is the same shape as
//! a [`Serialize`] value, for a caller that must **embed** the statistics inside a larger Gson
//! document and keep the order — the result manifest's `board_statistics` and the MCP tools'
//! `statistics`. [`to_gson_json`] is the [`serde_json::Value`] the plan asked for, and it is
//! **lossy twice over**: `serde_json`'s `Map` is a `BTreeMap` (the workspace does not enable
//! `preserve_order`), so the keys come back alphabetised, and `f32` widens to `f64`, so
//! `Float.toString`'s shorter text is lost. Use it to *inspect* statistics, never to write them;
//! `the_value_form_loses_key_order_and_float_width` (this module's test) pins both losses so
//! neither is a surprise.

use fr_dsn::format::json::to_gson_string_pretty;
use fr_router::score::{
    BoardStatistics, BoardStatisticsBends, BoardStatisticsBoard,
    BoardStatisticsClearanceViolations, BoardStatisticsComponents, BoardStatisticsConnections,
    BoardStatisticsFanout, BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets,
    BoardStatisticsPads, BoardStatisticsTraces, BoardStatisticsVias, Rectangle2DFloat,
};
use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};

/// One DTO wrapper: the `@SerializedName` on the left, the port's field on the right, in Java's
/// declaration order. Every field is an `Option` and a null one is skipped, which is exactly what
/// Gson does without `serializeNulls()`.
macro_rules! gson_dto {
    ($(#[$meta:meta])* $wrapper:ident, $inner:ty, $($key:literal => $field:ident),+ $(,)?) => {
        $(#[$meta])*
        struct $wrapper<'a>(&'a $inner);

        impl Serialize for $wrapper<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut map = serializer.serialize_map(None)?;
                $(
                    if let Some(value) = self.0.$field {
                        map.serialize_entry($key, &value)?;
                    }
                )+
                map.end()
            }
        }
    };
}

gson_dto!(
    /// `BoardStatisticsLayers.java:9-13`.
    GsonLayers, BoardStatisticsLayers,
    "total_count" => total_count,
    "signal_count" => signal_count,
);

gson_dto!(
    /// `BoardStatisticsItems.java:9-31`. `componentOutlineCount` is `"component_count"` (`:27`),
    /// which collides with `components.total_count`; that is Java's name, not a slip.
    GsonItems, BoardStatisticsItems,
    "total_count" => total_count,
    "trace_count" => trace_count,
    "via_count" => via_count,
    "conduction_area_count" => conduction_area_count,
    "drill_item_count" => drill_item_count,
    "pin_count" => pin_count,
    "component_count" => component_outline_count,
    "other_count" => other_count,
);

gson_dto!(
    /// `BoardStatisticsComponents.java:9-10`.
    GsonComponents, BoardStatisticsComponents,
    "total_count" => total_count,
);

gson_dto!(
    /// `BoardStatisticsPads.java:9-10`.
    GsonPads, BoardStatisticsPads,
    "total_count" => total_count,
);

gson_dto!(
    /// `BoardStatisticsNets.java:9-13`.
    GsonNets, BoardStatisticsNets,
    "total_count" => total_count,
    "class_count" => class_count,
);

gson_dto!(
    /// `BoardStatisticsConnections.java:9-13`.
    GsonConnections, BoardStatisticsConnections,
    "maximum_count" => maximum_count,
    "incomplete_count" => incomplete_count,
);

gson_dto!(
    /// `BoardStatisticsTraces.java:11-49`. Java declares `totalVerticalLength` **before**
    /// `totalHorizontalLength` (`:41`, `:45`), which is the reverse of how the computing
    /// constructor assigns them; the declaration wins.
    GsonTraces, BoardStatisticsTraces,
    "total_count" => total_count,
    "total_segment_count" => total_segment_count,
    "total_length" => total_length,
    "total_length_mm" => total_length_mm,
    "total_weighted_length" => total_weighted_length,
    "average_length" => average_length,
    "total_vertical_length" => total_vertical_length,
    "total_horizontal_length" => total_horizontal_length,
    "total_angled_length" => total_angled_length,
);

gson_dto!(
    /// `BoardStatisticsBends.java:9-19`. Two keys begin with a digit (`:12`, `:15`) — legal JSON,
    /// and not derivable from the Java field names.
    GsonBends, BoardStatisticsBends,
    "total_count" => total_count,
    "90_degree_count" => ninety_degree_count,
    "45_degree_count" => forty_five_degree_count,
    "other_angle_count" => other_angle_count,
);

gson_dto!(
    /// `BoardStatisticsVias.java:10-23`.
    GsonVias, BoardStatisticsVias,
    "total_count" => total_count,
    "through_hole_count" => through_hole_count,
    "blind_count" => blind_count,
    "buried_count" => buried_count,
);

gson_dto!(
    /// `BoardStatisticsClearanceViolations.java:9-22` — the DTO `fr-drc` owns (plan-5 ruling 5).
    /// Its last three fields are `Double`, not `Float`, so they print through `Double.toString`.
    GsonClearanceViolations, BoardStatisticsClearanceViolations,
    "total_count" => total_count,
    "min_violation_um" => min_violation_um,
    "max_violation_um" => max_violation_um,
    "avg_violation_um" => avg_violation_um,
);

/// `java.awt.geom.Rectangle2D.Float`'s four public fields, in the order the JDK declares them.
///
/// Gson reflects over `getDeclaredFields()` of the class and then of each superclass;
/// `Rectangle2D` and `RectangularShape` declare no instance fields, so these four are the whole
/// object. See [`Rectangle2DFloat`]'s own docs for why `width`/`height` do not hold a width and a
/// height on `board.bounding_box` (quirk #196).
struct GsonRectangle<'a>(&'a Rectangle2DFloat);

impl Serialize for GsonRectangle<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("x", &self.0.x)?;
        map.serialize_entry("y", &self.0.y)?;
        map.serialize_entry("width", &self.0.width)?;
        map.serialize_entry("height", &self.0.height)?;
        map.end()
    }
}

/// `BoardStatisticsBoard.java:11-15`. Both members are objects, so they need the wrapper above
/// rather than the `gson_dto!` macro's scalar arm.
struct GsonBoard<'a>(&'a BoardStatisticsBoard);

impl Serialize for GsonBoard<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(bounding_box) = &self.0.bounding_box {
            map.serialize_entry("bounding_box", &GsonRectangle(bounding_box))?;
        }
        if let Some(size) = &self.0.size {
            map.serialize_entry("size", &GsonRectangle(size))?;
        }
        map.end()
    }
}

/// `BoardStatistics.BoardStatisticsFanout` (`BoardStatistics.java:637-647`). Primitive `int`s:
/// all three keys are always present, and `0` is a value rather than an absence.
struct GsonFanout<'a>(&'a BoardStatisticsFanout);

impl Serialize for GsonFanout<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("total_smd_pins", &self.0.total_smd_pins)?;
        map.serialize_entry("pins_to_escape", &self.0.pins_to_escape)?;
        map.serialize_entry("escaped_count", &self.0.escaped_count)?;
        map.end()
    }
}

/// [`BoardStatistics`] as Gson serialises it — the embeddable form of [`to_gson_string`].
///
/// A caller that writes a *larger* Gson document with the statistics inside it (the result
/// manifest's `board_statistics`, the MCP tools' `statistics`) puts this in its own ordered
/// struct and serialises the whole thing through
/// [`fr_dsn::format::json::to_gson_string_pretty`]. Going through [`to_gson_json`] instead would
/// alphabetise the keys and widen every `f32`.
pub struct GsonBoardStatistics<'a>(pub &'a BoardStatistics);

impl Serialize for GsonBoardStatistics<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let stats = self.0;
        let mut map = serializer.serialize_map(None)?;
        // `:37-41` — the two `String` fields, whose empty value is Java's `null` (module docs).
        if !stats.host.is_empty() {
            map.serialize_entry("host", &stats.host)?;
        }
        if !stats.unit.is_empty() {
            map.serialize_entry("unit", &stats.unit)?;
        }
        // `:43-78` — the twelve DTOs, every one of them non-null in Java because the field
        // initialiser constructs it, so every one of them always present here.
        map.serialize_entry("board", &GsonBoard(&stats.board))?;
        map.serialize_entry("layers", &GsonLayers(&stats.layers))?;
        map.serialize_entry("items", &GsonItems(&stats.items))?;
        map.serialize_entry("components", &GsonComponents(&stats.components))?;
        map.serialize_entry("pads", &GsonPads(&stats.pads))?;
        map.serialize_entry("nets", &GsonNets(&stats.nets))?;
        map.serialize_entry("connections", &GsonConnections(&stats.connections))?;
        map.serialize_entry("traces", &GsonTraces(&stats.traces))?;
        map.serialize_entry("bends", &GsonBends(&stats.bends))?;
        map.serialize_entry("vias", &GsonVias(&stats.vias))?;
        map.serialize_entry(
            "clearance_violations",
            &GsonClearanceViolations(&stats.clearance_violations),
        )?;
        map.serialize_entry("fanout", &GsonFanout(&stats.fanout))?;
        map.end()
    }
}

/// renamed: BoardStatistics.toString (core/scoring/BoardStatistics.java:588-591) — `to_gson_string`, because `Display`/`ToString` on a foreign type is not this crate's to implement and a name that says *which* JSON dialect is written is worth more than the Java spelling.
///
/// # Panics
///
/// Where `Gson.toJson` throws `IllegalArgumentException` — a non-finite `f32`/`f64` anywhere in
/// the object. `GsonProvider` never calls `serializeSpecialFloatingPointValues()`, so `NaN` and
/// `±Infinity` are refused on both sides; see `fr_dsn::format::json`'s module docs, point 3.
/// Reachable in principle from a degenerate board (an average trace length over zero traces), and
/// pinned by `crates/fr-core/tests/stats.rs`.
pub fn to_gson_string(stats: &BoardStatistics) -> String {
    to_gson_string_pretty(&GsonBoardStatistics(stats))
        .expect("Gson.toJson throws IllegalArgumentException on a non-finite float; so does this")
}

/// The same object as a [`serde_json::Value`], for **inspection only**.
///
/// Two losses, both structural and both pinned by this module's
/// `the_value_form_loses_key_order_and_float_width`:
///
/// - `serde_json::Map` is a `BTreeMap` unless the crate is built with `preserve_order`, which
///   this workspace does not enable and plan 8 forbids adding, so the keys come back
///   **alphabetised** rather than in Java's declaration order.
/// - `serde_json`'s `serialize_f32` widens to `f64`, so a `Float` field prints through
///   `Double.toString` instead of `Float.toString` — `0.1f` becomes `0.10000000149011612`.
///
/// Write statistics through [`to_gson_string`], or embed [`GsonBoardStatistics`] in the
/// surrounding struct.
///
/// # Panics
///
/// On a non-finite float, exactly as [`to_gson_string`] does.
pub fn to_gson_json(stats: &BoardStatistics) -> serde_json::Value {
    serde_json::to_value(GsonBoardStatistics(stats))
        .expect("Gson.toJson throws IllegalArgumentException on a non-finite float; so does this")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two losses [`to_gson_json`]'s docs promise, asserted rather than described.
    #[test]
    fn the_value_form_loses_key_order_and_float_width() {
        let mut stats = BoardStatistics::default();
        stats.traces.total_length = Some(0.1);

        let string = to_gson_string(&stats);
        assert!(
            string.starts_with("{\n  \"board\": {},\n  \"layers\": {},"),
            "{string}"
        );
        assert!(string.contains("\"total_length\": 0.1"), "{string}");

        let value = to_gson_json(&stats);
        let keys: Vec<&str> = value
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "bends",
                "board",
                "clearance_violations",
                "components",
                "connections",
                "fanout",
                "items",
                "layers",
                "nets",
                "pads",
                "traces",
                "vias",
            ],
            "serde_json's Map is a BTreeMap without `preserve_order`"
        );
        assert_eq!(
            value["traces"]["total_length"].as_f64(),
            Some(0.1_f32 as f64),
            "0.1f32 widened to f64 is 0.10000000149011612, not 0.1"
        );
    }
}
