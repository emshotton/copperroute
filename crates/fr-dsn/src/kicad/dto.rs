#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct KiCadBoardJson {
    /// Explicit permission for newly routed vias to share SMD pad copper.
    #[serde(default, skip_serializing_if = "is_false")]
    pub viaInPadAllowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designName: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostCad: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostVersion: Option<String>,
    #[serde(
        default = "unit_default",
        deserialize_with = "gson_enum",
        skip_serializing_if = "Option::is_none"
    )]
    pub unit: Option<UnitJson>,
    #[serde(default = "one", deserialize_with = "nullable_or_one")]
    pub resolution: f64,

    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<LayerJson>>,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub netClasses: Option<Vec<NetClassJson>>,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub nets: Option<Vec<NetJson>>,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub clearanceRules: Option<Vec<CustomClearanceRuleJson>>,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<ComponentJson>>,
    #[serde(default = "outline_default", skip_serializing_if = "Option::is_none")]
    pub outline: Option<OutlineJson>,

    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub traces: Option<Vec<TraceJson>>,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub vias: Option<Vec<ViaJson>>,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub conductionAreas: Option<Vec<ConductionAreaJson>>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum UnitJson {
    MM,
    MIL,
    UM,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct LayerJson {
    #[serde(default, deserialize_with = "nullable")]
    pub index: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct NetClassJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viaInPadAllowed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub clearance: f64,
    #[serde(default, deserialize_with = "nullable")]
    pub traceWidth: f64,
    #[serde(default, deserialize_with = "nullable")]
    pub viaDiameter: f64,
    #[serde(default, deserialize_with = "nullable")]
    pub viaDrill: f64,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub netNames: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct NetJson {
    #[serde(default, deserialize_with = "nullable")]
    pub id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub className: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub containsPlane: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct CustomClearanceRuleJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classA: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classB: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub clearance: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ComponentJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<String>,
    #[serde(default = "point_default", skip_serializing_if = "Option::is_none")]
    pub position: Option<Point2D>,
    #[serde(default, deserialize_with = "nullable")]
    pub rotation: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub pads: Option<Vec<PadJson>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PadJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netName: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(default = "point_default", skip_serializing_if = "Option::is_none")]
    pub size: Option<Point2D>,
    #[serde(default = "point_default", skip_serializing_if = "Option::is_none")]
    pub offset: Option<Point2D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Point2D>,
    #[serde(default, deserialize_with = "nullable")]
    pub drill: f64,
    #[serde(default)]
    pub nonPlated: bool,
    #[serde(default)]
    pub drillEstimated: bool,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<Option<String>>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OutlineJson {
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub corners: Option<Vec<Point2D>>,
    #[serde(default, deserialize_with = "nullable")]
    pub clearance: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TraceJson {
    #[serde(default, deserialize_with = "nullable")]
    pub id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netName: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub width: f64,
    #[serde(default, deserialize_with = "nullable")]
    pub layerIndex: i32,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<Point2D>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ViaJson {
    #[serde(default, deserialize_with = "nullable")]
    pub id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netName: Option<String>,
    #[serde(default = "point_default", skip_serializing_if = "Option::is_none")]
    pub position: Option<Point2D>,
    #[serde(default, deserialize_with = "nullable")]
    pub diameter: f64,
    #[serde(default, deserialize_with = "nullable")]
    pub drill: f64,
    #[serde(default, deserialize_with = "nullable")]
    pub startLayerIndex: i32,
    #[serde(default, deserialize_with = "nullable")]
    pub endLayerIndex: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ConductionAreaJson {
    #[serde(default, deserialize_with = "nullable")]
    pub id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netName: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub layerIndex: i32,
    #[serde(default, deserialize_with = "nullable")]
    pub isObstacle: bool,
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<Point2D>>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq)]
pub struct Point2D {
    #[serde(default, deserialize_with = "nullable")]
    pub x: f64,
    #[serde(default, deserialize_with = "nullable")]
    pub y: f64,
}

impl Default for KiCadBoardJson {
    fn default() -> KiCadBoardJson {
        KiCadBoardJson {
            viaInPadAllowed: false,
            designName: None,
            hostCad: None,
            hostVersion: None,
            unit: unit_default(),
            resolution: one(),
            layers: empty(),
            netClasses: empty(),
            nets: empty(),
            clearanceRules: empty(),
            components: empty(),
            outline: outline_default(),
            traces: empty(),
            vias: empty(),
            conductionAreas: empty(),
        }
    }
}

impl Default for NetClassJson {
    fn default() -> NetClassJson {
        NetClassJson {
            viaInPadAllowed: None,
            name: None,
            clearance: 0.0,
            traceWidth: 0.0,
            viaDiameter: 0.0,
            viaDrill: 0.0,
            netNames: empty(),
        }
    }
}

impl Default for ComponentJson {
    fn default() -> ComponentJson {
        ComponentJson {
            reference: None,
            value: None,
            footprint: None,
            position: point_default(),
            rotation: 0.0,
            layer: None,
            pads: empty(),
        }
    }
}

impl Default for PadJson {
    fn default() -> PadJson {
        PadJson {
            name: None,
            netName: None,
            shape: None,
            size: point_default(),
            offset: point_default(),
            position: None,
            drill: 0.0,
            nonPlated: false,
            drillEstimated: false,
            layers: empty(),
        }
    }
}

impl Default for OutlineJson {
    fn default() -> OutlineJson {
        OutlineJson {
            corners: empty(),
            clearance: 0.0,
        }
    }
}

impl Default for TraceJson {
    fn default() -> TraceJson {
        TraceJson {
            id: 0,
            netName: None,
            width: 0.0,
            layerIndex: 0,
            points: empty(),
        }
    }
}

impl Default for ViaJson {
    fn default() -> ViaJson {
        ViaJson {
            id: 0,
            netName: None,
            position: point_default(),
            diameter: 0.0,
            drill: 0.0,
            startLayerIndex: 0,
            endLayerIndex: 0,
        }
    }
}

impl Default for ConductionAreaJson {
    fn default() -> ConductionAreaJson {
        ConductionAreaJson {
            id: 0,
            netName: None,
            layerIndex: 0,
            isObstacle: false,
            polygon: empty(),
        }
    }
}

fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn nullable_or_one<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or(1.0))
}

fn gson_enum<'de, D>(deserializer: D) -> Result<Option<UnitJson>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(
        match Option::<String>::deserialize(deserializer)?.as_deref() {
            Some("MM") => Some(UnitJson::MM),
            Some("MIL") => Some(UnitJson::MIL),
            Some("UM") => Some(UnitJson::UM),
            _ => None,
        },
    )
}

fn empty<T>() -> Option<Vec<T>> {
    Some(Vec::new())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedJson {
    pub section: &'static str,
    pub object: String,
    pub problem: &'static str,
}

impl KiCadBoardJson {
    pub fn validate(&self) -> Result<(), MalformedJson> {
        for (index, layer) in self.layers.iter().flatten().enumerate() {
            if layer.name.is_none() {
                return Err(MalformedJson {
                    section: "layers",
                    object: format!("layers[{index}].name"),
                    problem: "is null, and a board layer must have a name",
                });
            }
        }
        for (index, net_class) in self.netClasses.iter().flatten().enumerate() {
            if net_class.name.is_none() {
                return Err(MalformedJson {
                    section: "netClasses",
                    object: format!("netClasses[{index}].name"),
                    problem: "is null, and a net class must have a name",
                });
            }
        }
        for (index, net) in self.nets.iter().flatten().enumerate() {
            if net.name.is_none() {
                return Err(MalformedJson {
                    section: "nets",
                    object: format!("nets[{index}].name"),
                    problem: "is null, and a net must have a name",
                });
            }
        }
        for (index, component) in self.components.iter().flatten().enumerate() {
            if component.reference.is_none() {
                return Err(MalformedJson {
                    section: "components",
                    object: format!("components[{index}].reference"),
                    problem: "is null, and a component must have a reference",
                });
            }
            for (pad_index, pad) in component.pads.iter().flatten().enumerate() {
                if pad.name.is_none() {
                    return Err(MalformedJson {
                        section: "components",
                        object: format!("components[{index}].pads[{pad_index}].name"),
                        problem: "is null, and a pad must have a name",
                    });
                }
                for (layer_index, layer_name) in pad.layers.iter().flatten().enumerate() {
                    if layer_name.is_none() {
                        return Err(MalformedJson {
                            section: "components",
                            object: format!(
                                "components[{index}].pads[{pad_index}].layers[{layer_index}]"
                            ),
                            problem: "is a null array element, and a layer name must be a string",
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

fn unit_default() -> Option<UnitJson> {
    Some(UnitJson::MM)
}

fn one() -> f64 {
    1.0
}

fn outline_default() -> Option<OutlineJson> {
    Some(OutlineJson::default())
}

fn point_default() -> Option<Point2D> {
    Some(Point2D::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_key_takes_the_java_field_initializer() {
        let board: KiCadBoardJson = serde_json::from_str("{}").expect("parses");
        assert_eq!(board.unit, Some(UnitJson::MM));
        assert_eq!(board.resolution, 1.0);
        assert_eq!(board.layers, Some(Vec::new()));
        assert_eq!(board.outline, Some(OutlineJson::default()));
        assert_eq!(board.designName, None);
    }

    #[test]
    fn an_explicit_null_clears_a_reference_and_spares_a_primitive() {
        let board: KiCadBoardJson =
            serde_json::from_str(r#"{"layers": null, "outline": null, "resolution": null}"#)
                .expect("parses");
        assert_eq!(board.layers, None);
        assert_eq!(board.outline, None);
        assert_eq!(board.resolution, 1.0);
    }

    #[test]
    fn the_unit_enum_is_case_sensitive_and_null_for_anything_else() {
        for (json, expected) in [
            (r#"{"unit":"MM"}"#, Some(UnitJson::MM)),
            (r#"{"unit":"MIL"}"#, Some(UnitJson::MIL)),
            (r#"{"unit":"UM"}"#, Some(UnitJson::UM)),
            (r#"{"unit":"mil"}"#, None),
            (r#"{"unit":"FOO"}"#, None),
            (r#"{"unit":null}"#, None),
        ] {
            let board: KiCadBoardJson = serde_json::from_str(json).expect("parses");
            assert_eq!(board.unit, expected, "for {json}");
        }
    }

    #[test]
    fn an_unknown_key_is_ignored_the_way_gson_ignores_it() {
        let board: KiCadBoardJson = serde_json::from_str(
            r#"{"netClasses":[{"name":"power","clearance":0.3,"uviaDiameter":0.3}]}"#,
        )
        .expect("parses");
        let classes = board.netClasses.expect("present");
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name.as_deref(), Some("power"));
    }
}
