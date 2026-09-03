//! The KiCad board-JSON **DTO boundary**: a `null` where the board needs a name is refused here,
//! with a diagnostic that names the file, the section and the offending object.
//!
//! Java stores the `null` verbatim and dies hundreds of lines later inside a JDK collection —
//! a `null` layer name at `KiCadJsonReader.java:545` but only for a board that also has pads
//! naming layers, a `null` net name inside `Nets.get`'s walk, a `null` component `reference`
//! inside `ConcurrentSkipListMap.put` via `Component.compareTo`, and a `null` element of
//! `pad.layers` not at all (`equalsIgnoreCase(null)` is `false`, so the pad silently loses a
//! layer). One tolerant site against three throwing ones, in one reader.
//!
//! fixed: T7 (#282, #283, #287) — the four refusals below.

use fr_dsn::error::BoardReadResult;
use fr_dsn::kicad::read_board;

/// The `(location, detail)` of a [`BoardReadResult::ParseError`], or a panic naming what came
/// back instead.
fn parse_error(json: &str) -> (String, String) {
    match read_board(json, None) {
        BoardReadResult::ParseError { location, detail } => (location, detail),
        other => panic!("expected a ParseError, got {other:?}"),
    }
}

/// Every one of the four refusals must name three things: the **file** (so a user with a stack of
/// exports knows which reader complained), the **section** (which is also the `location`, i.e.
/// what `fr_core::load` quotes back) and the **offending object** by its full JSON path.
fn assert_names_file_section_and_object(json: &str, section: &str, object: &str) {
    let (location, detail) = parse_error(json);
    assert_eq!(location, section, "the location is the section: {detail}");
    assert!(
        detail.contains("KiCad board JSON file"),
        "the diagnostic must name the file: {detail}"
    );
    assert!(
        detail.contains(section),
        "the diagnostic must name the section `{section}`: {detail}"
    );
    assert!(
        detail.contains(object),
        "the diagnostic must name the object `{object}`: {detail}"
    );
}

/// A `null` layer name. Java reaches `:545`'s
/// `boardLayers[li].name.equalsIgnoreCase(layerName)` **only** if some pad also names a layer, so
/// the same board without pads loaded silently with a layer called `""`.
#[test]
fn a_null_layer_name_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu","type":"signal"},{"name":null,"type":"signal"}]}"#,
        "layers",
        "layers[1].name",
    );
}

/// A `null` net name. Java stores it and dies in `Nets.get`'s walk — at `:491` if anything
/// references a net at all, otherwise in section 9.
#[test]
fn a_null_net_name_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],"nets":[{"name":"GND"},{"name":null}]}"#,
        "nets",
        "nets[1].name",
    );
}

/// A `null` net-class name — the sibling of the layer name at `:131`, which ends up in the
/// clearance matrix and NPEs in `ClearanceMatrix.getNo`.
#[test]
fn a_null_net_class_name_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],"netClasses":[{"name":null}]}"#,
        "netClasses",
        "netClasses[0].name",
    );
}

/// A `null` component `reference`. Java's `Components.add` hands the component to the undo
/// container, whose `ConcurrentSkipListMap.put` orders keys through
/// `Component.compareTo -> this.name.compareToIgnoreCase(...)`, so it dies inside the container
/// before anything reads the component.
#[test]
fn a_null_component_reference_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":null,"footprint":"F","pads":[]}]}"#,
        "components",
        "components[0].reference",
    );
}

/// A `null` pad name — the throw `arePackagePinsIdentical` makes, and therefore the whole reason
/// quirk #285's duplicate packages exist.
#[test]
fn a_null_pad_name_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":"U1","footprint":"F","pads":[{"name":null}]}]}"#,
        "components",
        "components[0].pads[0].name",
    );
}

/// A `null` **array element**. This is the one Java *tolerates*: Gson deserializes
/// `[null, "B.Cu"]` into a two-element list whose first entry is `null`, and
/// `equalsIgnoreCase(null)` is `false`, so the pad silently spans `B.Cu` only.
#[test]
fn a_null_array_element_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":"U1","footprint":"F",
                           "pads":[{"name":"1","layers":[null,"B.Cu"]}]}]}"#,
        "components",
        "components[0].pads[0].layers[0]",
    );
}

/// The refusals are **name** refusals, not a blanket ban on `null`. Every nullable field Java
/// tolerates by design — a pad with no `netName`, a net with no `className`, a component with no
/// `value`, a clearance rule naming a class that is not there — still loads, because a `null`
/// there means "absent", not "malformed".
#[test]
fn a_nullable_field_that_means_absent_still_loads() {
    let json = r#"{
        "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "nets":[{"name":"GND","className":null}],
        "clearanceRules":[{"classA":null,"classB":null,"clearance":0.3}],
        "components":[{"reference":"U1","value":null,"footprint":"F","layer":null,
                       "pads":[{"name":"1","netName":null,"shape":null,
                                "size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]}]
    }"#;
    assert!(
        matches!(read_board(json, None), BoardReadResult::Success { .. }),
        "a nullable-means-absent field must not be refused"
    );
}

/// A `null` **list** keeps Java's `NullPointerException` shape: the reader dereferences
/// `boardJson.layers` with no guard at `:105`, and the DTO validation must not get in front of
/// that — it validates the contents of a list, never its presence.
#[test]
fn a_null_list_still_takes_the_unguarded_dereference() {
    let (location, detail) = parse_error(r#"{"layers":null}"#);
    assert_eq!(location, "json_payload");
    assert!(
        detail.contains("java.util.List.isEmpty()") && detail.contains("boardJson.layers"),
        "the null-list NPE is unchanged: {detail}"
    );
}
