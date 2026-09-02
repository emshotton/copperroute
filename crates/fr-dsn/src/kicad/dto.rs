//! Port of `io/kicad/KiCadBoardJson.java` (142 lines): the twelve data-transfer objects
//! `KiCadJsonReader` and `KiCadJsonWriter` exchange.
//!
//! # The field names are the wire contract
//!
//! Java declares no `@SerializedName` anywhere in this file, so Gson's field names *are* the
//! Java field names — `designName`, `hostCad`, `netClasses`, `containsPlane`, `isObstacle`,
//! `startLayerIndex`. Those are what KiCad writes and what the writer (Task 10) must write back,
//! so this module keeps the Java spelling on the **Rust** fields too rather than declaring
//! `#[serde(rename)]` on every one of them: a rename attribute is a second place to get the
//! contract wrong, and the transcription is then no longer greppable against the Java file. That
//! is what the `#![allow(non_snake_case)]` below buys, and it buys nothing else — no other module
//! in this crate carries it.
//!
//! # Gson's three-way distinction, and how serde reproduces it
//!
//! Gson's reflective adapter writes a field only when the JSON *has* the key, so Java has three
//! states where serde normally has two:
//!
//! | JSON | Gson | this module |
//! |---|---|---|
//! | key absent | the Java field initializer survives (`new ArrayList<>()`, `UnitJson.MM`, `1.0`) | `#[serde(default = "…")]` reproducing that initializer |
//! | `"k": null` | a **reference** field is set to `null`; a **primitive** field is left alone (`ReflectiveTypeAdapterFactory.BoundField.read` skips the write) | [`nullable`] for primitives, `Option<T>` for references |
//! | `"k": <value>` | the value | the value |
//!
//! The distinction is load-bearing: `readBoard` guards `components`, `traces`, `vias` and
//! `conductionAreas` with `!= null` (KiCadJsonReader.java:182, :204, :216, :230) and does **not**
//! guard `layers`, `netClasses`, `clearanceRules` or `nets` (`:105`, `:124`, `:156`, `:446`), so
//! an explicit `"layers": null` is a `NullPointerException` there and an absent `layers` is an
//! empty list. Both are measured — `crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt`, stems
//! `layers-null` and `netclasses-null`.
//!
//! # The write side (Plan 8 Task 10)
//!
//! `KiCadJsonWriter.write` (KiCadJsonWriter.java:32-226) fills this same tree and hands it to
//! `GsonProvider.GSON.toJson`, so [`crate::kicad::writer::write`] serialises these structs through
//! [`crate::format::json::to_gson_string_pretty`] — the identical formatter, already JVM-verified.
//! Two Gson behaviours the derive has to reproduce:
//!
//! * **Key order is declaration order.** Gson's `ReflectiveTypeAdapterFactory` walks
//!   `getDeclaredFields()`, and serde's derive emits fields in declaration order too, so the
//!   Java field order in each struct above *is* the wire order. Do not reorder a field.
//! * **A `null` reference field is omitted**, because `GsonProvider` never calls
//!   `serializeNulls()` — hence the `skip_serializing_if = "Option::is_none"` on every `Option`
//!   field. On the write path only `hostCad`/`hostVersion` (which the writer never sets) and a
//!   `netName` whose net lookup failed can be `None`; every list the writer touches is `Some`,
//!   so it serialises as `[]` exactly as Java's `new ArrayList<>()` does.
//!
//! # Not ported
//!
// not ported: `KiCadBoardJson.Point2D()` and `Point2D(double, double)` — Java's two constructors
// (KiCadBoardJson.java:129, :137-140). The no-argument one is `Default`; the two-argument one has
// no caller in `KiCadJsonReader` (the reader only ever *reads* points) and Task 10's writer builds
// its points with struct literals. The audit's `renamed:` line for `Point2D` is in
// `crates/fr-drc/src/lib.rs`, where `scripts/audit-map/fr-drc.map` maps the class.
#![allow(non_snake_case)] // see "The field names are the wire contract" above

use serde::{Deserialize, Serialize};

/// Port of `KiCadBoardJson` (KiCadBoardJson.java:7-32): the root DTO.
///
/// Every list is `Option<Vec<_>>` and every nested object `Option<_>` because Gson can set them
/// to Java `null` — see the module docs' three-way table.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct KiCadBoardJson {
    /// `KiCadBoardJson.designName` (:8). Never read by `readBoard`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designName: Option<String>,
    /// `KiCadBoardJson.hostCad` (:9) — `readBoard:314-315`, with the `"KiCad"` fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostCad: Option<String>,
    /// `KiCadBoardJson.hostVersion` (:10) — `readBoard:316-319`, with the `"v10.0"` fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostVersion: Option<String>,
    /// `KiCadBoardJson.unit` (:11), whose Java initializer is `UnitJson.MM`.
    #[serde(
        default = "unit_default",
        deserialize_with = "gson_enum",
        skip_serializing_if = "Option::is_none"
    )]
    pub unit: Option<UnitJson>,
    /// `KiCadBoardJson.resolution` (:12), whose Java initializer is `1.0` — the one primitive in
    /// the tree that does not default to zero.
    #[serde(default = "one", deserialize_with = "nullable_or_one")]
    pub resolution: f64,

    /// `KiCadBoardJson.layers` (:14). **Unguarded** at `readBoard:105`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<LayerJson>>,
    /// `KiCadBoardJson.netClasses` (:15). **Unguarded** at `readBoard:124`/`:141`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub netClasses: Option<Vec<NetClassJson>>,
    /// `KiCadBoardJson.nets` (:16). **Unguarded** at `readBoard:446`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub nets: Option<Vec<NetJson>>,
    /// `KiCadBoardJson.clearanceRules` (:17). **Unguarded** at `readBoard:156`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub clearanceRules: Option<Vec<CustomClearanceRuleJson>>,
    /// `KiCadBoardJson.components` (:18). Guarded at `readBoard:182` and `:457`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<ComponentJson>>,
    /// `KiCadBoardJson.outline` (:19), whose Java initializer is `new OutlineJson()`.
    #[serde(default = "outline_default", skip_serializing_if = "Option::is_none")]
    pub outline: Option<OutlineJson>,

    /// `KiCadBoardJson.traces` (:21). Guarded at `readBoard:216` and `:475`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub traces: Option<Vec<TraceJson>>,
    /// `KiCadBoardJson.vias` (:22). Guarded at `readBoard:204` and `:482`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub vias: Option<Vec<ViaJson>>,
    /// `KiCadBoardJson.conductionAreas` (:23). Guarded at `readBoard:230` and `:468`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub conductionAreas: Option<Vec<ConductionAreaJson>>,
}

/// Port of `KiCadBoardJson.UnitJson` (KiCadBoardJson.java:26-30): "coordinate units used by the
/// JSON representation".
///
/// Deserialized by [`gson_enum`], which is Gson's `EnumTypeAdapter`: an exact, **case-sensitive**
/// match against the three constant names, and `null` for anything else. `"mil"` is therefore not
/// `MIL` — measured, stem `unit-lowercase`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum UnitJson {
    /// `UnitJson.MM` (:27).
    MM,
    /// `UnitJson.MIL` (:28).
    MIL,
    /// `UnitJson.UM` (:29).
    UM,
}

/// Port of `KiCadBoardJson.LayerJson` (KiCadBoardJson.java:33-37).
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct LayerJson {
    /// `LayerJson.index` (:34). Never read by `readBoard`, which uses the list position.
    #[serde(default, deserialize_with = "nullable")]
    pub index: i32,
    /// `LayerJson.name` (:35).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `LayerJson.type` (:36) — `"signal"` or `"plane"`. `type` is a Rust keyword, so the field
    /// is spelled `r#type`; the JSON key is still `type`.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Port of `KiCadBoardJson.NetClassJson` (KiCadBoardJson.java:40-47).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct NetClassJson {
    /// `NetClassJson.name` (:41).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `NetClassJson.clearance` (:42).
    #[serde(default, deserialize_with = "nullable")]
    pub clearance: f64,
    /// `NetClassJson.traceWidth` (:43).
    #[serde(default, deserialize_with = "nullable")]
    pub traceWidth: f64,
    /// `NetClassJson.viaDiameter` (:44).
    #[serde(default, deserialize_with = "nullable")]
    pub viaDiameter: f64,
    /// `NetClassJson.viaDrill` (:45).
    #[serde(default, deserialize_with = "nullable")]
    pub viaDrill: f64,
    /// `NetClassJson.netNames` (:46). Never read by `readBoard`: a net's class comes from
    /// `NetJson.className`, not from this list.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub netNames: Option<Vec<String>>,
}

/// Port of `KiCadBoardJson.NetJson` (KiCadBoardJson.java:50-55).
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct NetJson {
    /// `NetJson.id` (:51). Never read by `readBoard`: net numbers come from `Nets.add`.
    #[serde(default, deserialize_with = "nullable")]
    pub id: i32,
    /// `NetJson.name` (:52).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `NetJson.className` (:53).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub className: Option<String>,
    /// `NetJson.containsPlane` (:54).
    #[serde(default, deserialize_with = "nullable")]
    pub containsPlane: bool,
}

/// Port of `KiCadBoardJson.CustomClearanceRuleJson` (KiCadBoardJson.java:58-62).
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct CustomClearanceRuleJson {
    /// `CustomClearanceRuleJson.classA` (:59).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classA: Option<String>,
    /// `CustomClearanceRuleJson.classB` (:60).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classB: Option<String>,
    /// `CustomClearanceRuleJson.clearance` (:61).
    #[serde(default, deserialize_with = "nullable")]
    pub clearance: f64,
}

/// Port of `KiCadBoardJson.ComponentJson` (KiCadBoardJson.java:65-73).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ComponentJson {
    /// `ComponentJson.reference` (:66), e.g. `"U1"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// `ComponentJson.value` (:67), e.g. `"STM32F405"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// `ComponentJson.footprint` (:68), e.g. `"Package_QFP:LQFP-64"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<String>,
    /// `ComponentJson.position` (:69), whose Java initializer is `new Point2D()`.
    #[serde(default = "point_default", skip_serializing_if = "Option::is_none")]
    pub position: Option<Point2D>,
    /// `ComponentJson.rotation` (:70), in degrees.
    #[serde(default, deserialize_with = "nullable")]
    pub rotation: f64,
    /// `ComponentJson.layer` (:71), `"F.Cu"` or `"B.Cu"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// `ComponentJson.pads` (:72). Guarded at `readBoard:190` and `:459`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub pads: Option<Vec<PadJson>>,
}

/// Port of `KiCadBoardJson.PadJson` (KiCadBoardJson.java:76-85).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PadJson {
    /// `PadJson.name` (:77), e.g. `"1"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `PadJson.netName` (:78), e.g. `"GND"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netName: Option<String>,
    /// `PadJson.shape` (:79) — `"rect"`, `"circle"` or `"oval"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    /// `PadJson.size` (:80), whose Java initializer is `new Point2D()`.
    #[serde(default = "point_default", skip_serializing_if = "Option::is_none")]
    pub size: Option<Point2D>,
    /// `PadJson.offset` (:81), whose Java initializer is `new Point2D()`.
    #[serde(default = "point_default", skip_serializing_if = "Option::is_none")]
    pub offset: Option<Point2D>,
    /// `PadJson.position` (:82) — the one `Point2D` field with **no** Java initializer, so an
    /// absent key leaves it `null` and `readBoard:192` guards it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Point2D>,
    /// `PadJson.drill` (:83).
    #[serde(default, deserialize_with = "nullable")]
    pub drill: f64,
    /// `PadJson.layers` (:84) — the layers this pad exists on.
    ///
    /// The **element** type is `Option<String>` because Gson stores a JSON `null` inside a
    /// `List<String>` as a `null` reference and `readBoard:545` then calls
    /// `boardLayers[li].name.equalsIgnoreCase(null)`, which is `false` rather than a throw — so
    /// `{"layers": [null, "B.Cu"]}` **loads** in Java, on `B.Cu`. A `Vec<String>` would make
    /// `serde_json` reject the whole file instead. Measured, stem `pad-layers-null-element` of
    /// `crates/fr-dsn/tests/data/p8t8-kicad-read-b.txt`; quirk #283, which records the three
    /// sibling lists where Java *also* stores the `null` but then throws on it, and where the port
    /// therefore diverges only in the `ParseError`'s prose.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<Option<String>>>,
}

/// Port of `KiCadBoardJson.OutlineJson` (KiCadBoardJson.java:88-91).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OutlineJson {
    /// `OutlineJson.corners` (:89). **Unguarded** at `readBoard:169`, which calls
    /// `boardJson.outline.corners.size()` after only a `boardJson.outline == null` test.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub corners: Option<Vec<Point2D>>,
    /// `OutlineJson.clearance` (:90), "outline/edge clearance class mapping". **Never read**:
    /// `readBoard:310` hard-codes `outlineClearanceNo = 1`.
    #[serde(default, deserialize_with = "nullable")]
    pub clearance: f64,
}

/// Port of `KiCadBoardJson.TraceJson` (KiCadBoardJson.java:94-100).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TraceJson {
    /// `TraceJson.id` (:95).
    #[serde(default, deserialize_with = "nullable")]
    pub id: i32,
    /// `TraceJson.netName` (:96).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netName: Option<String>,
    /// `TraceJson.width` (:97).
    #[serde(default, deserialize_with = "nullable")]
    pub width: f64,
    /// `TraceJson.layerIndex` (:98).
    #[serde(default, deserialize_with = "nullable")]
    pub layerIndex: i32,
    /// `TraceJson.points` (:99). Guarded at `readBoard:218`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<Point2D>>,
}

/// Port of `KiCadBoardJson.ViaJson` (KiCadBoardJson.java:103-111).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ViaJson {
    /// `ViaJson.id` (:104).
    #[serde(default, deserialize_with = "nullable")]
    pub id: i32,
    /// `ViaJson.netName` (:105).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netName: Option<String>,
    /// `ViaJson.position` (:106), whose Java initializer is `new Point2D()`.
    #[serde(default = "point_default", skip_serializing_if = "Option::is_none")]
    pub position: Option<Point2D>,
    /// `ViaJson.diameter` (:107).
    #[serde(default, deserialize_with = "nullable")]
    pub diameter: f64,
    /// `ViaJson.drill` (:108).
    #[serde(default, deserialize_with = "nullable")]
    pub drill: f64,
    /// `ViaJson.startLayerIndex` (:109).
    #[serde(default, deserialize_with = "nullable")]
    pub startLayerIndex: i32,
    /// `ViaJson.endLayerIndex` (:110).
    #[serde(default, deserialize_with = "nullable")]
    pub endLayerIndex: i32,
}

/// Port of `KiCadBoardJson.ConductionAreaJson` (KiCadBoardJson.java:114-120).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ConductionAreaJson {
    /// `ConductionAreaJson.id` (:115).
    #[serde(default, deserialize_with = "nullable")]
    pub id: i32,
    /// `ConductionAreaJson.netName` (:116).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netName: Option<String>,
    /// `ConductionAreaJson.layerIndex` (:117).
    #[serde(default, deserialize_with = "nullable")]
    pub layerIndex: i32,
    /// `ConductionAreaJson.isObstacle` (:118).
    #[serde(default, deserialize_with = "nullable")]
    pub isObstacle: bool,
    /// `ConductionAreaJson.polygon` (:119). Guarded at `readBoard:232`.
    #[serde(default = "empty", skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<Point2D>>,
}

/// Port of `KiCadBoardJson.Point2D` (KiCadBoardJson.java:124-140): "two-dimensional point in the
/// JSON representation".
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq)]
pub struct Point2D {
    /// `Point2D.x` (:125).
    #[serde(default, deserialize_with = "nullable")]
    pub x: f64,
    /// `Point2D.y` (:126).
    #[serde(default, deserialize_with = "nullable")]
    pub y: f64,
}

// ------------------------------------------------- `new X()`, i.e. the Java field initializers
//
// A derived `Default` would zero every field, which is **not** what `new KiCadBoardJson()` or
// `new OutlineJson()` produce: Java initializes `unit` to `MM`, `resolution` to `1.0`, every list
// to `new ArrayList<>()` and four `Point2D` fields to `new Point2D()`. `outline_default` and
// `point_default` below hand those values to serde, so the two must agree — hence hand-written
// impls rather than the derive, for every class that has an initializer at all. `LayerJson`,
// `NetJson`, `CustomClearanceRuleJson` and `Point2D` have none, and keep the derive.

impl Default for KiCadBoardJson {
    /// `new KiCadBoardJson()` (KiCadBoardJson.java:8-23).
    fn default() -> KiCadBoardJson {
        KiCadBoardJson {
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
    /// `new NetClassJson()` (KiCadBoardJson.java:41-46).
    fn default() -> NetClassJson {
        NetClassJson {
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
    /// `new ComponentJson()` (KiCadBoardJson.java:66-72).
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
    /// `new PadJson()` (KiCadBoardJson.java:77-84). Note `position`: the one `Point2D` field Java
    /// leaves `null`.
    fn default() -> PadJson {
        PadJson {
            name: None,
            netName: None,
            shape: None,
            size: point_default(),
            offset: point_default(),
            position: None,
            drill: 0.0,
            layers: empty(),
        }
    }
}

impl Default for OutlineJson {
    /// `new OutlineJson()` (KiCadBoardJson.java:89-90) — the value `KiCadBoardJson.outline`'s own
    /// initializer produces, so an absent `outline` key is an outline with an **empty** corner
    /// list, not a `null` one.
    fn default() -> OutlineJson {
        OutlineJson {
            corners: empty(),
            clearance: 0.0,
        }
    }
}

impl Default for TraceJson {
    /// `new TraceJson()` (KiCadBoardJson.java:95-99).
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
    /// `new ViaJson()` (KiCadBoardJson.java:104-110).
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
    /// `new ConductionAreaJson()` (KiCadBoardJson.java:115-119).
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

// ================================================================== the Gson-shaped defaults

/// Gson's treatment of `"k": null` on a **primitive** field: the write is skipped, so the field
/// keeps whatever the Java initializer gave it — zero for every primitive in this tree except
/// `KiCadBoardJson.resolution`, which has [`nullable_or_one`] of its own.
fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// [`nullable`] for `KiCadBoardJson.resolution` (KiCadBoardJson.java:12), whose Java initializer
/// is `1.0` rather than `0.0`.
fn nullable_or_one<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or(1.0))
}

/// Gson's `TypeAdapters.EnumTypeAdapter.read` (`nameToConstant.get(in.nextString())`): an exact,
/// case-sensitive match against the constant names, and `null` — not an error — for anything
/// else. serde's derived enum adapter rejects an unknown variant instead, so [`UnitJson`] cannot
/// use it.
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

/// The `= new ArrayList<>()` initializer every list field in the tree carries: an **absent** key
/// leaves an empty list, which is not the same as an explicit `null`.
fn empty<T>() -> Option<Vec<T>> {
    Some(Vec::new())
}

/// `KiCadBoardJson.unit = UnitJson.MM` (KiCadBoardJson.java:11).
fn unit_default() -> Option<UnitJson> {
    Some(UnitJson::MM)
}

/// `KiCadBoardJson.resolution = 1.0` (KiCadBoardJson.java:12).
fn one() -> f64 {
    1.0
}

/// `KiCadBoardJson.outline = new OutlineJson()` (KiCadBoardJson.java:19).
fn outline_default() -> Option<OutlineJson> {
    Some(OutlineJson::default())
}

/// The `= new Point2D()` initializer on `ComponentJson.position`, `PadJson.size`,
/// `PadJson.offset` and `ViaJson.position`. `PadJson.position` deliberately does **not** use it.
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
        // Gson skips the write for a primitive, so the `= 1.0` initializer survives.
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
        // Every corpus fixture carries `uviaDiameter`/`uviaDrill` on its net classes, which
        // `NetClassJson` does not declare.
        let board: KiCadBoardJson = serde_json::from_str(
            r#"{"netClasses":[{"name":"power","clearance":0.3,"uviaDiameter":0.3}]}"#,
        )
        .expect("parses");
        let classes = board.netClasses.expect("present");
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name.as_deref(), Some("power"));
    }
}
