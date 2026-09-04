use fr_dsn::format::json::to_gson_string_pretty;
use fr_router::score::{
    BoardStatistics, BoardStatisticsBends, BoardStatisticsBoard,
    BoardStatisticsClearanceViolations, BoardStatisticsComponents, BoardStatisticsConnections,
    BoardStatisticsFanout, BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets,
    BoardStatisticsPads, BoardStatisticsTraces, BoardStatisticsVias, Rectangle2DFloat,
};
use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};

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
        GsonLayers, BoardStatisticsLayers,
    "total_count" => total_count,
    "signal_count" => signal_count,
);

gson_dto!(
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
        GsonComponents, BoardStatisticsComponents,
    "total_count" => total_count,
);

gson_dto!(
        GsonPads, BoardStatisticsPads,
    "total_count" => total_count,
);

gson_dto!(
        GsonNets, BoardStatisticsNets,
    "total_count" => total_count,
    "class_count" => class_count,
);

gson_dto!(
        GsonConnections, BoardStatisticsConnections,
    "maximum_count" => maximum_count,
    "incomplete_count" => incomplete_count,
);

gson_dto!(
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
            GsonBends, BoardStatisticsBends,
    "total_count" => total_count,
    "90_degree_count" => ninety_degree_count,
    "45_degree_count" => forty_five_degree_count,
    "other_angle_count" => other_angle_count,
);

gson_dto!(
        GsonVias, BoardStatisticsVias,
    "total_count" => total_count,
    "through_hole_count" => through_hole_count,
    "blind_count" => blind_count,
    "buried_count" => buried_count,
);

gson_dto!(
            GsonClearanceViolations, BoardStatisticsClearanceViolations,
    "total_count" => total_count,
    "min_violation_um" => min_violation_um,
    "max_violation_um" => max_violation_um,
    "avg_violation_um" => avg_violation_um,
);

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

pub struct GsonBoardStatistics<'a>(pub &'a BoardStatistics);

impl Serialize for GsonBoardStatistics<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let stats = self.0;
        let mut map = serializer.serialize_map(None)?;
        if !stats.host.is_empty() {
            map.serialize_entry("host", &stats.host)?;
        }
        if !stats.unit.is_empty() {
            map.serialize_entry("unit", &stats.unit)?;
        }
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

pub fn to_gson_string(stats: &BoardStatistics) -> String {
    to_gson_string_pretty(&GsonBoardStatistics(stats))
        .expect("Gson.toJson throws IllegalArgumentException on a non-finite float; so does this")
}

pub fn to_gson_json(stats: &BoardStatistics) -> serde_json::Value {
    serde_json::to_value(GsonBoardStatistics(stats))
        .expect("Gson.toJson throws IllegalArgumentException on a non-finite float; so does this")
}

#[cfg(test)]
mod tests {
    use super::*;

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
