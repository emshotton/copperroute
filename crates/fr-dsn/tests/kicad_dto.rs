use fr_dsn::error::BoardReadResult;
use fr_dsn::kicad::read_board;

fn parse_error(json: &str) -> (String, String) {
    match read_board(json, None) {
        BoardReadResult::ParseError { location, detail } => (location, detail),
        other => panic!("expected a ParseError, got {other:?}"),
    }
}

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

#[test]
fn a_null_layer_name_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu","type":"signal"},{"name":null,"type":"signal"}]}"#,
        "layers",
        "layers[1].name",
    );
}

#[test]
fn a_null_net_name_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],"nets":[{"name":"GND"},{"name":null}]}"#,
        "nets",
        "nets[1].name",
    );
}

#[test]
fn a_null_net_class_name_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],"netClasses":[{"name":null}]}"#,
        "netClasses",
        "netClasses[0].name",
    );
}

#[test]
fn a_null_component_reference_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":null,"footprint":"F","pads":[]}]}"#,
        "components",
        "components[0].reference",
    );
}

#[test]
fn a_null_pad_name_is_rejected_with_the_section_named() {
    assert_names_file_section_and_object(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":"U1","footprint":"F","pads":[{"name":null}]}]}"#,
        "components",
        "components[0].pads[0].name",
    );
}

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

#[test]
fn a_null_list_still_takes_the_unguarded_dereference() {
    let (location, detail) = parse_error(r#"{"layers":null}"#);
    assert_eq!(location, "json_payload");
    assert!(
        detail.contains("java.util.List.isEmpty()") && detail.contains("boardJson.layers"),
        "the null-list NPE is unchanged: {detail}"
    );
}
