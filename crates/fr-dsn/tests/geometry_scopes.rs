//! Black-box tests for Plan 3 Task 5: `io/CoordinateTransform.java`, the DSN geometry scopes
//! (`io/specctra/parser/{Shape,Rectangle,Circle,Polygon,PolygonPath,PolylinePath,Path,Layer,
//! LayerStructure}.java`) and the header scopes
//! (`io/specctra/parser/{Parser,Resolution,Unit}.java`).

use fr_board::{
    Communication, ItemIdGenerator, Layer as BoardLayer, LayerStructure, Unit, WriteResolution,
};
use fr_dsn::parser::geometry::{
    DsnCircle, DsnLayer, DsnLayerStructure, DsnPolygon, DsnPolygonPath, DsnPolylinePath,
    DsnRectangle, DsnShape, read_area_scope, read_scope as read_shape_scope,
};
use fr_dsn::parser::header::{
    read_parser_scope, read_resolution_scope, read_unit_scope, write_parser_scope,
    write_resolution_scope, write_unit_scope,
};
use fr_dsn::parser::scope_parameter::{DsnReadOptions, ReadScopeParameter};
use fr_dsn::{
    CoordinateTransform, DSN_RESERVED, DsnError, DsnScanner, IdentifierType, IndentFileWriter,
};
use fr_geometry::{FloatPoint, IntBox, IntPoint};

fn identifier() -> IdentifierType {
    IdentifierType::new(
        DSN_RESERVED.iter().map(|s| (*s).to_string()).collect(),
        "\"".to_string(),
    )
}

fn render(f: impl FnOnce(&mut IndentFileWriter<&mut Vec<u8>>)) -> String {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut writer = IndentFileWriter::new(&mut buf);
        f(&mut writer);
        writer.flush().expect("write to a Vec never fails");
    }
    String::from_utf8(buf).expect("the writer emits UTF-8")
}

fn pcb_layer() -> DsnLayer {
    DsnLayer::pcb()
}

fn scan(input: &str) -> DsnScanner {
    DsnScanner::new(input)
}

// ---------------------------------------------------------------- CoordinateTransform

#[test]
fn coordinate_transform_scales_both_ways() {
    let transform = CoordinateTransform::new(10.0, 0.0, 0.0).expect("a valid scale");
    assert_eq!(transform.board_to_dsn(1000.0), 100.0);
    assert_eq!(transform.dsn_to_board(100.0), 1000.0);
}

/// fixed: T4 (#89) — replaces `coordinate_transform_with_a_zero_scale_factor_gives_infinity`.
///
/// Java's `value / scaleFactor` on `double`s is IEEE division, not the integer division that
/// throws, so `new CoordinateTransform(0, 0, 0)` is built without complaint and every coordinate
/// it writes afterwards is `Infinity` (or `NaN`, for a value of `0`). `Structure.createBoard`
/// reaches that constructor for real — quirk #94, JVM-verified to report `Success` — so the state
/// is not hypothetical. It is now unrepresentable: the constructor refuses a scale factor that is
/// zero, infinite or `NaN`, and there is no other way to build the type.
#[test]
fn a_zero_scale_is_refused_loudly() {
    for bad in [0.0_f64, -0.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let refused = CoordinateTransform::new(bad, 0.0, 0.0);
        assert!(
            matches!(refused, Err(DsnError::InvalidScaleFactor { .. })),
            "a scale factor of {bad} must be refused, not divided by"
        );
    }
    // The neighbouring legal values are still accepted, so the guard is a guard and not a wall.
    for good in [1.0_f64, 0.1, -10.0, f64::MIN_POSITIVE] {
        assert!(
            CoordinateTransform::new(good, 0.0, 0.0).is_ok(),
            "a scale factor of {good} is finite and non-zero"
        );
    }
}

#[test]
fn coordinate_transform_offsets_points_by_the_base_but_not_relative_points() {
    let transform = CoordinateTransform::new(10.0, 3.0, 5.0).expect("a valid scale");
    let point = FloatPoint::new(1000.0, 2000.0);
    assert_eq!(transform.board_to_dsn_point(&point), [103.0, 205.0]);
    assert_eq!(transform.board_to_dsn_rel_point(&point), [100.0, 200.0]);
    assert_eq!(
        transform.dsn_to_board_point(&[103.0, 205.0]),
        FloatPoint::new(1000.0, 2000.0)
    );
    assert_eq!(
        transform.dsn_to_board_rel(&[100.0, 200.0]),
        FloatPoint::new(1000.0, 2000.0)
    );
}

#[test]
fn coordinate_transform_box_and_points() {
    let transform = CoordinateTransform::new(10.0, 1.0, 2.0).expect("a valid scale");
    let b = IntBox::new(IntPoint::new(0, 0), IntPoint::new(100, 200));
    assert_eq!(transform.board_to_dsn_box(&b), [1.0, 2.0, 11.0, 22.0]);
    assert_eq!(transform.board_to_dsn_rel_box(&b), [0.0, 0.0, 10.0, 20.0]);
    let points = [FloatPoint::new(0.0, 0.0), FloatPoint::new(100.0, 200.0)];
    assert_eq!(
        transform.board_to_dsn_points(&points),
        vec![1.0, 2.0, 11.0, 22.0]
    );
    assert_eq!(
        transform.board_to_dsn_rel_points(&points),
        vec![0.0, 0.0, 10.0, 20.0]
    );
}

#[test]
fn board_to_dsn_shape_maps_a_box_to_a_rectangle() {
    let transform = CoordinateTransform::new(10.0, 0.0, 0.0).expect("a valid scale");
    let shape = fr_geometry::Shape::Tile(fr_geometry::TileShape::Box(IntBox::new(
        IntPoint::new(0, 0),
        IntPoint::new(100, 200),
    )));
    let Some(DsnShape::Rect(rect)) = transform.board_to_dsn_shape(&shape, pcb_layer()) else {
        panic!("an IntBox becomes a Rectangle");
    };
    assert_eq!(rect.coor, [0.0, 0.0, 10.0, 20.0]);
}

// ---------------------------------------------------------------- geometry writers

#[test]
fn rectangle_write_scope_matches_the_reference_fixture_line() {
    // tests/reference/Issue143-rpi_splitter/roundtrip.dsn:19 — also a live
    // `java_double_to_string` test (`1650.0`, not `1650`).
    let rect = DsnRectangle::new(
        pcb_layer(),
        [
            -0.393_700_787_401_574_8,
            -0.393_700_787_401_574_8,
            837.401_574_803_149_6,
            1650.0,
        ],
    );
    let out = render(|f| rect.write_scope(f, &identifier()));
    assert_eq!(
        out,
        "\n(rect pcb -0.3937007874015748 -0.3937007874015748 837.4015748031496 1650.0)"
    );
}

#[test]
fn rectangle_write_scope_int_rounds_every_coordinate() {
    let rect = DsnRectangle::new(
        pcb_layer(),
        [
            -0.393_700_787_401_574_8,
            -0.393_700_787_401_574_8,
            837.401_574_803_149_6,
            1650.0,
        ],
    );
    let out = render(|f| rect.write_scope_int(f, &identifier()));
    assert_eq!(out, "\n(rect pcb 0 0 837 1650)");
}

#[test]
fn circle_write_scope_and_write_scope_int() {
    let circle = DsnCircle::new(DsnLayer::signal(), [4.5, 1.5, 2.5]);
    assert_eq!(
        render(|f| circle.write_scope(f, &identifier())),
        "\n(circle signal 4.5 1.5 2.5)"
    );
    // Java `(int) Math.round(2.5)` is 3 (half-up), and `Math.round(1.5)` is 2.
    assert_eq!(
        render(|f| circle.write_scope_int(f, &identifier())),
        "\n(circle signal 5 2 3)"
    );
}

/// fixed: T4 (#93) — replaces `circle_bounding_box_is_javas_doubled_box`.
///
/// Java bug (register row 93): `Circle.boundingBox` (Circle.java:56-64) spends the whole of
/// `coor[0]` on each side, though `coor[0]` is a **diameter** everywhere else in the same class
/// and at its only producer (Circle.java:40,48; CoordinateTransform.java:99).
///
/// Hand-computed, all four bounds, for a diameter-4 circle centred on `(10, 20)`:
/// radius = 4 / 2 = 2, so `[10 - 2, 20 - 2, 10 + 2, 20 + 2] == [8, 18, 12, 22]`.
/// Java's answer was `[6, 16, 14, 24]` — twice as wide and twice as tall.
///
/// The second case is the round trip that makes the reading unambiguous: a board `Circle` of
/// radius 25 becomes, through `CoordinateTransform::board_to_dsn_shape`, a `DsnCircle` whose
/// `coor[0]` is `2 * board_to_dsn(25)`, and the bounding box of *that* must be the box of the
/// circle we started from.
#[test]
fn a_circle_bounding_box_is_not_twice_too_wide() {
    let circle = DsnCircle::new(DsnLayer::signal(), [4.0, 10.0, 20.0]);
    assert_eq!(circle.bounding_box().coor, [8.0, 18.0, 12.0, 22.0]);

    // Round trip: radius 25 at (100, 200) on a scale factor of 10 is a DSN circle of diameter
    // 5 centred on (10, 20), whose box is [10 - 2.5, 20 - 2.5, 10 + 2.5, 20 + 2.5].
    let transform = CoordinateTransform::new(10.0, 0.0, 0.0).expect("a valid scale");
    let board_circle =
        fr_geometry::Shape::Circle(fr_geometry::Circle::new(IntPoint::new(100, 200), 25));
    let Some(DsnShape::Circle(round_tripped)) =
        transform.board_to_dsn_shape(&board_circle, DsnLayer::signal())
    else {
        panic!("a Circle becomes a Circle");
    };
    assert_eq!(round_tripped.coor, [5.0, 10.0, 20.0]);
    assert_eq!(
        round_tripped.bounding_box().coor,
        [7.5, 17.5, 12.5, 22.5],
        "the box of the circle the board actually has"
    );
}

#[test]
fn polygon_write_scope_indents_one_corner_pair_per_line() {
    let polygon = DsnPolygon::new(DsnLayer::signal(), vec![0.0, 0.0, 837.0, 0.0]);
    assert_eq!(
        render(|f| polygon.write_scope(f, &identifier())),
        "\n(polygon signal 0\n  0.0 0.0\n  837.0 0.0\n)"
    );
    assert_eq!(
        render(|f| polygon.write_scope_int(f, &identifier())),
        "\n(polygon signal 0\n  0 0\n  837 0\n)"
    );
}

#[test]
fn polygon_path_write_scope_writes_the_width_then_the_corners() {
    let path = DsnPolygonPath::new(DsnLayer::signal(), 10.0, vec![0.0, 0.0, 5.0, 6.0]);
    assert_eq!(
        render(|f| path.write_scope(f, &identifier())),
        "\n(path signal 10.0\n  0.0 0.0\n  5.0 6.0\n)"
    );
    assert_eq!(
        render(|f| path.write_scope_int(f, &identifier())),
        "\n(path signal 10.0\n  0 0\n  5 6\n)"
    );
}

#[test]
fn polyline_path_write_scope_writes_four_numbers_per_line_each_followed_by_a_space() {
    let path = DsnPolylinePath::new(DsnLayer::signal(), 10.0, vec![0.0, 1.0, 2.0, 3.0]);
    assert_eq!(
        render(|f| path.write_scope(f, &identifier())),
        "\n(polyline_path signal 10.0\n  0.0 1.0 2.0 3.0 \n)"
    );
    assert_eq!(
        render(|f| path.write_scope_int(f, &identifier())),
        "\n(polyline_path signal 10.0\n  0 1 2 3 \n)"
    );
}

#[test]
fn write_hole_scope_wraps_the_shape_in_a_window_scope() {
    let rect = DsnRectangle::new(DsnLayer::signal(), [0.0, 0.0, 1.0, 1.0]);
    let shape = DsnShape::Rect(rect);
    assert_eq!(
        render(|f| shape.write_hole_scope(f, &identifier())),
        "\n(window\n  (rect signal 0.0 0.0 1.0 1.0)\n)"
    );
}

// ---------------------------------------------------------------- geometry readers

#[test]
fn read_shape_scope_reads_a_rectangle() {
    let mut scanner = scan("(rect signal 0 0 10 20)");
    let shape = read_shape_scope(&mut scanner, None)
        .expect("no scan error")
        .expect("a rectangle");
    let DsnShape::Rect(rect) = shape else {
        panic!("expected a rectangle");
    };
    assert_eq!(rect.coor, [0.0, 0.0, 10.0, 20.0]);
    assert_eq!(rect.layer.name, "signal");
}

#[test]
fn read_shape_scope_reads_a_polygon_on_the_pcb_layer() {
    let mut scanner = scan("(polygon pcb 0 1.5 2.5 3 4)");
    let shape = read_shape_scope(&mut scanner, None)
        .expect("no scan error")
        .expect("a polygon");
    let DsnShape::Polygon(polygon) = shape else {
        panic!("expected a polygon");
    };
    assert_eq!(polygon.coor, vec![1.5, 2.5, 3.0, 4.0]);
    assert_eq!(polygon.layer.name, "pcb");
}

#[test]
fn read_shape_scope_reads_a_path_and_its_width() {
    let mut scanner = scan("(path signal 10 0 0 5 6)");
    let shape = read_shape_scope(&mut scanner, None)
        .expect("no scan error")
        .expect("a path");
    let DsnShape::Path(path) = shape else {
        panic!("expected a polygon path");
    };
    assert_eq!(path.width, 10.0);
    assert_eq!(path.coordinate_arr, vec![0.0, 0.0, 5.0, 6.0]);
}

#[test]
fn read_shape_scope_skips_a_path_with_too_few_coordinates() {
    // Shape.java:470-478 — fewer than five numbers (width + two corners) is dropped.
    let mut scanner = scan("(path signal 10 0 0)");
    assert!(
        read_shape_scope(&mut scanner, None)
            .expect("no scan error")
            .is_none()
    );
}

#[test]
fn read_shape_scope_reads_a_polyline_path() {
    let mut scanner = scan("(polyline_path signal 10 0 0 5 6)");
    let shape = read_shape_scope(&mut scanner, None)
        .expect("no scan error")
        .expect("a polyline path");
    let DsnShape::PolylinePath(path) = shape else {
        panic!("expected a polyline path");
    };
    assert_eq!(path.width, 10.0);
    assert_eq!(path.coordinate_arr, vec![0.0, 0.0, 5.0, 6.0]);
}

#[test]
fn read_shape_scope_returns_none_for_an_unknown_shape_keyword() {
    let mut scanner = scan("(qarc signal 1 2 3)");
    assert!(
        read_shape_scope(&mut scanner, None)
            .expect("no scan error")
            .is_none()
    );
}

#[test]
fn read_area_scope_collects_the_border_the_window_and_the_clearance_class() {
    let mut scanner = scan(
        "my_area (rect signal 0 0 10 10) (window (circle signal 2 5 5)) \
         (clearance_class \"kls\"))",
    );
    let area = read_area_scope(&mut scanner, None, false)
        .expect("no scan error")
        .expect("an area");
    assert_eq!(area.area_name.as_deref(), Some("my_area"));
    assert_eq!(area.clearance_class_name.as_deref(), Some("kls"));
    assert_eq!(area.shape_list.len(), 2);
    assert!(matches!(area.shape_list[0], Some(DsnShape::Rect(_))));
    assert!(matches!(area.shape_list[1], Some(DsnShape::Circle(_))));
}

#[test]
fn read_area_scope_skips_window_scopes_when_asked() {
    let mut scanner = scan("my_area (rect signal 0 0 10 10) (window (circle signal 2 5 5)))");
    let area = read_area_scope(&mut scanner, None, true)
        .expect("no scan error")
        .expect("an area");
    assert_eq!(area.shape_list.len(), 1);
}

// ---------------------------------------------------------------- Layer / LayerStructure

#[test]
fn layer_structure_get_no_finds_a_named_layer() {
    let layers = DsnLayerStructure::new(vec![
        DsnLayer::new("F.Cu", 0, true),
        DsnLayer::new("In1.Cu", 1, true),
        DsnLayer::new("B.Cu", 2, true),
    ]);
    assert_eq!(layers.get_no("F.Cu"), Some(0));
    assert_eq!(layers.get_no("In1.Cu"), Some(1));
    assert_eq!(layers.get_no("B.Cu"), Some(2));
    assert_eq!(layers.get_no("nope"), None);
}

#[test]
fn layer_structure_get_no_has_the_electra_top_bottom_fallbacks() {
    // LayerStructure.java:37-43 — a name merely *containing* "Top"/"Bottom" maps to the first
    // resp. last layer, for Electra-written outlines.
    let layers = DsnLayerStructure::new(vec![
        DsnLayer::new("F.Cu", 0, true),
        DsnLayer::new("In1.Cu", 1, true),
        DsnLayer::new("B.Cu", 2, true),
    ]);
    assert_eq!(layers.get_no("Outline_Top"), Some(0));
    assert_eq!(layers.get_no("Outline_Bottom"), Some(2));
    // `layers.length - 1` is -1 for an empty structure, which every Java caller tests as
    // "not found".
    assert_eq!(DsnLayerStructure::new(Vec::new()).get_no("Bottom"), None);
}

#[test]
fn layer_structure_from_a_board_layer_structure_numbers_layers_in_order() {
    let board_layers = LayerStructure::new(vec![
        BoardLayer::new("F.Cu", true),
        BoardLayer::new("GND", false),
        BoardLayer::new("B.Cu", true),
    ]);
    let dsn = DsnLayerStructure::from_board(&board_layers);
    assert_eq!(dsn.layers.len(), 3);
    assert_eq!(dsn.layers[1].name, "GND");
    assert_eq!(dsn.layers[1].no, 1);
    assert!(!dsn.layers[1].is_signal);
    assert_eq!(dsn.signal_layer_count(), 2);
}

#[test]
fn layer_structure_contains_plane_only_looks_at_power_layers() {
    let layers = DsnLayerStructure::new(vec![
        DsnLayer::with_nets("F.Cu", 0, true, vec!["VCC".to_string()]),
        DsnLayer::with_nets("GND", 1, false, vec!["GND".to_string()]),
    ]);
    assert!(layers.contains_plane("GND"));
    assert!(!layers.contains_plane("VCC"));
}

#[test]
fn the_two_shared_layer_constants_have_the_java_names_and_numbers() {
    assert_eq!(DsnLayer::pcb().name, "pcb");
    assert_eq!(DsnLayer::pcb().no, -1);
    assert!(!DsnLayer::pcb().is_signal);
    assert_eq!(DsnLayer::signal().name, "signal");
    assert_eq!(DsnLayer::signal().no, -1);
    assert!(DsnLayer::signal().is_signal);
}

// ---------------------------------------------------------------- header scopes

#[test]
fn write_parser_scope_uses_the_2_3_0_snake_case_literals() {
    let info = Communication {
        host_cad: Some("KiCad".to_string()),
        host_version: Some("8.0.1".to_string()),
        constants: vec![vec!["a".to_string(), "b".to_string()]],
        write_resolution: Some(WriteResolution::new("mil", 10)),
        ..Communication::default()
    };
    let out = render(|f| write_parser_scope(f, &info, &identifier(), false));
    assert_eq!(
        out,
        "\n(parser\
         \n  (string_quote \")\
         \n  (space_in_quoted_tokens on)\
         \n  (host_cad KiCad)\
         \n  (host_version \"8.0.1\")\
         \n  (constant a b )\
         \n  (write_resolution m 10)\
         \n  (generated_by_freerouting)\
         \n)"
    );
}

#[test]
fn write_parser_scope_reduced_drops_the_quote_and_the_freerouting_marker() {
    let info = Communication {
        host_cad: Some("KiCad".to_string()),
        ..Communication::default()
    };
    let out = render(|f| write_parser_scope(f, &info, &identifier(), true));
    assert_eq!(out, "\n(parser\n  (host_cad KiCad)\n)");
}

#[test]
fn read_parser_scope_reads_the_quote_char_host_and_freerouting_marker() {
    let options = DsnReadOptions::default();
    let mut p = ReadScopeParameter::new(
        scan(
            "(string_quote ')(host_cad KiCad)(host_version 8.0)(constant a b)(write_resolution mil 10)(generated_by_freerouting))",
        ),
        &options,
    );
    assert!(read_parser_scope(&mut p).expect("no scan error"));
    assert_eq!(p.string_quote, "'");
    assert_eq!(p.host_cad.as_deref(), Some("KiCad"));
    assert_eq!(p.host_version.as_deref(), Some("8.0"));
    assert_eq!(p.constants, vec![vec!["a".to_string(), "b".to_string()]]);
    let write_resolution = p.write_resolution.as_ref().expect("a write resolution");
    assert_eq!(write_resolution.char_name, "mil");
    assert_eq!(write_resolution.positive_int, 10);
    // Java bug (JVM-verified against tools/freerouting-2.3.0.jar): the scanner never returns
    // `Keyword.GENERATED_BY_FREEROUTING` — `generated_by_freerouting` comes back as a plain
    // String, so `Parser.readScope`'s branch for it is dead and `dsnFileGeneratedByHost` stays
    // `true` even for a file freerouting itself wrote. See docs/java-quirks.md.
    assert!(p.dsn_file_generated_by_host);
}

#[test]
fn read_parser_scope_skips_unknown_sub_scopes() {
    let options = DsnReadOptions::default();
    let mut p = ReadScopeParameter::new(
        scan("(space_in_quoted_tokens on)(host_cad CadSoft))"),
        &options,
    );
    assert!(read_parser_scope(&mut p).expect("no scan error"));
    assert_eq!(p.host_cad.as_deref(), Some("CadSoft"));
}

#[test]
fn read_resolution_scope_sets_the_unit_and_the_resolution() {
    let options = DsnReadOptions::default();
    let mut p = ReadScopeParameter::new(scan("mil 2540)"), &options);
    assert!(read_resolution_scope(&mut p).expect("no scan error"));
    assert_eq!(p.unit, Unit::Mil);
    assert_eq!(p.resolution, 2540);
}

#[test]
fn read_resolution_scope_rejects_an_unknown_unit() {
    let options = DsnReadOptions::default();
    let mut p = ReadScopeParameter::new(scan("furlong 2540)"), &options);
    assert!(!read_resolution_scope(&mut p).expect("no scan error"));
}

#[test]
fn write_resolution_and_unit_scopes_match_the_reference_fixture() {
    // tests/reference/Issue143-rpi_splitter/roundtrip.dsn:9-10.
    let communication = Communication::new(Unit::Mil, 2540, ItemIdGenerator::new(), None, None);
    assert_eq!(
        render(|f| write_resolution_scope(f, &communication)),
        "\n(resolution mil 2540)"
    );
    assert_eq!(render(|f| write_unit_scope(f, Unit::Mil)), "\n(unit mil)");
}

#[test]
fn read_unit_scope_is_javas_dead_always_false_override() {
    // Unit.java:23-26 — `Unit` is never registered as a `Keyword`, so this override is
    // unreachable in Java and returns `false` unconditionally.
    let options = DsnReadOptions::default();
    let mut p = ReadScopeParameter::new(scan("mil)"), &options);
    assert!(!read_unit_scope(&mut p).expect("no scan error"));
}

// =================================================================================================
// Plan 9 Task 11, quirk #88 — `PolygonPath.boundingBox` grew the upper x bound once per coordinate
// =================================================================================================

/// **fixed: T11 (#88).** `bounds[2] = Math.max(bounds[2], arr[i]) + offset` put the `+ offset`
/// **outside** the `max` on the x axis (PolygonPath.java:122) where the y axis one line below has
/// it inside (:126), so the upper x bound grew by `width/2` once per x coordinate instead of once
/// in total. The y axis getting it right is what makes it a transcription slip rather than intent.
///
/// The recurrence `b_k = max(b_{k-1}, x_{k-1}) + off` solves to `max_j (x_j + (k-j)·off)`, so the
/// over-estimate is `(k-2)·offset` for a closed path and grows **linearly with the corner count**.
/// For the square below — 4 corners at width 200, so `offset = 100` — that is `1300` where `1100`
/// is right:
///
/// ```text
/// bounds[2] from i32::MIN, x coordinates 0, 1000, 1000, 0:
///   after 0    : max(MIN, 0)    + 100 =  100
///   after 1000 : max(100, 1000) + 100 = 1100
///   after 1000 : max(1100,1000) + 100 = 1200
///   after 0    : max(1200, 0)   + 100 = 1300     <- and the path's own maximum is 1000
/// ```
///
/// A 100-corner keepout outline at the same width was over-wide by about 9800 units. `boundingBox`
/// is used for sizing, so tightening it shrinks boxes.
#[test]
fn bounding_box_is_square_for_a_square_path() {
    let path = DsnPolygonPath::new(
        DsnLayer::signal(),
        200.0,
        vec![0.0, 0.0, 1000.0, 0.0, 1000.0, 1000.0, 0.0, 1000.0],
    );
    let b = path.bounding_box();
    assert_eq!(b.coor[0], -100.0);
    assert_eq!(b.coor[1], -100.0);
    assert_eq!(b.coor[3], 1100.0);
    assert_eq!(b.coor[2], 1100.0, "fixed: T11 (#88) — was 1300.0");
    // The path is a square, so its bounding box must be one. That statement survives a change of
    // fixture where the four literals above would not.
    assert_eq!(
        b.coor[2] - b.coor[0],
        b.coor[3] - b.coor[1],
        "a square path has a square bounding box"
    );
}
