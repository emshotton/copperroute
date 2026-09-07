mod common;

use copper_board::{FixedState, Item, PackagePin};
use copper_dsn::CoordinateTransform;
use copper_dsn::format::IndentFileWriter;
use copper_dsn::keyword::{Keyword, ScopeKeyword};
use copper_dsn::lexer::{DsnScanner, Token};
use copper_dsn::parser::placement::{ComponentPlacement, write_placement_scope};
use copper_dsn::parser::scope_parameter::{
    DsnReadOptions, ReadScopeParameter, WriteScopeParameter, read_scope,
};
use copper_geometry::{Circle, IntPoint, Point, Shape, Vector};

fn read_placement<T>(text: &str, f: impl FnOnce(bool, &[ComponentPlacement]) -> T) -> T {
    let options = DsnReadOptions::default();
    let scanner = DsnScanner::new(text);
    let mut p = ReadScopeParameter::new(scanner, &options);
    assert_eq!(p.scanner.next_token().expect("scan"), Some(Token::Open));
    assert_eq!(
        p.scanner.next_token().expect("scan"),
        Some(Token::Kw(Keyword::PlacementScope))
    );
    let ok = read_scope(ScopeKeyword::Placement, &mut p).expect("no scan error");
    let list = std::mem::take(&mut p.placement_list);
    f(ok, &list)
}

fn fixture(name: &str) -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../freerouting/fixtures/"
    );
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

#[test]
fn a_place_scope_reads_name_coordinates_side_and_rotation() {
    read_placement(
        "(placement (component X (place C1 1000 -2000 front 90)))",
        |ok, list| {
            assert!(ok);
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].lib_name, "X");
            assert_eq!(list[0].locations.len(), 1);
            let loc = &list[0].locations[0];
            assert_eq!(loc.name, "C1");
            assert_eq!(loc.coor, Some([1000.0, -2000.0]));
            assert!(loc.is_front);
            assert!((loc.rotation - 90.0).abs() < f64::EPSILON);
            assert!(!loc.position_fixed);
            assert_eq!(loc.part_number, None);
            assert!(loc.pin_infos.is_empty());
        },
    );
}

#[test]
fn a_place_scope_without_coordinates_leaves_the_location_unset() {
    read_placement("(placement (component X (place C1)))", |ok, list| {
        assert!(ok);
        let loc = &list[0].locations[0];
        assert_eq!(loc.name, "C1");
        assert_eq!(loc.coor, None);
        assert!(loc.is_front);
        assert_eq!(loc.rotation, 0.0);
        assert!(!loc.position_fixed);
    });
}

#[test]
fn a_place_scope_reads_lock_type_part_number_pins_and_keepouts() {
    let text = concat!(
        "(placement (component X (place C1 10 20 back 270 (lock_type position) (PN \"ABC-123\") ",
        "(pin 3 (clearance_class hi)) (pin 1 (clearance_class lo)) ",
        "(keepout K1 (clearance_class kk)) (via_keepout V1 (clearance_class vv)) ",
        "(place_keepout P1 (clearance_class pp)))))",
    );
    read_placement(text, |ok, list| {
        assert!(ok);
        let loc = &list[0].locations[0];
        assert_eq!(loc.coor, Some([10.0, 20.0]));
        assert!(!loc.is_front);
        assert!((loc.rotation - 270.0).abs() < f64::EPSILON);
        assert!(loc.position_fixed);
        assert_eq!(loc.part_number.as_deref(), Some("ABC-123"));
        assert_eq!(
            loc.pin_infos.keys().cloned().collect::<Vec<_>>(),
            vec!["1".to_string(), "3".to_string()]
        );
        assert_eq!(loc.pin_infos["1"].clearance_class, "lo");
        assert_eq!(loc.pin_infos["3"].clearance_class, "hi");
        assert_eq!(loc.keepout_infos["K1"].clearance_class, "kk");
        assert_eq!(loc.via_keepout_infos["V1"].clearance_class, "vv");
        assert_eq!(loc.place_keepout_infos["P1"].clearance_class, "pp");
    });
}

#[test]
fn both_clearance_class_spellings_parse_identically() {
    let snake = read_placement(
        "(placement (component X (place C1 10 20 front 45 (pin 1 (clearance_class hi)))))",
        |ok, list| {
            assert!(ok);
            list[0].locations[0].pin_infos["1"].clearance_class.clone()
        },
    );
    let camel = read_placement(
        "(placement (component X (place C1 10 20 front 45 (pin 1 (clearanceClass hi)))))",
        |ok, list| {
            assert!(ok);
            list[0].locations[0].pin_infos["1"].clearance_class.clone()
        },
    );
    assert_eq!(snake, "hi");
    assert_eq!(camel, "hi");
}

#[test]
fn a_lowercase_pn_scope_is_still_a_part_number() {
    read_placement(
        "(placement (component X (place C1 10 20 front 45 (pn ABC))))",
        |ok, list| {
            assert!(ok);
            assert_eq!(list[0].locations[0].part_number.as_deref(), Some("ABC"));
        },
    );
}

#[test]
fn a_missing_side_keyword_warns_and_keeps_the_front_side() {
    read_placement(
        "(placement (component X (place C1 1.5 -2.5 sideways 90)))",
        |ok, list| {
            assert!(ok);
            let loc = &list[0].locations[0];
            assert_eq!(loc.coor, Some([1.5, -2.5]));
            assert!(loc.is_front);
            assert!((loc.rotation - 90.0).abs() < f64::EPSILON);
        },
    );
}

#[test]
fn an_unreadable_item_clearance_info_fails_the_whole_component() {
    read_placement(
        "(placement (component X (place C1 10 20 front 45 (pin 1 (nonsense hi)))))",
        |ok, list| {
            assert!(!ok);
            assert!(list.is_empty());
        },
    );
}

#[test]
fn several_components_and_several_places_all_land_in_the_placement_list() {
    read_placement(
        concat!(
            "(placement (component A (place A1 1 2 front 0) (place A2 3 4 back 180)) ",
            "(component B (place B1 5 6 front 90)))",
        ),
        |ok, list| {
            assert!(ok);
            assert_eq!(list.len(), 2);
            assert_eq!(list[0].lib_name, "A");
            assert_eq!(
                list[0]
                    .locations
                    .iter()
                    .map(|l| l.name.clone())
                    .collect::<Vec<_>>(),
                vec!["A1".to_string(), "A2".to_string()]
            );
            assert_eq!(list[1].lib_name, "B");
            assert_eq!(list[1].locations[0].name, "B1");
        },
    );
}

fn write_placement(
    add: impl FnOnce(&mut copper_board::Board),
    flip_style_rotate_first: bool,
) -> String {
    let text = fixture("empty_board.dsn");
    let options = DsnReadOptions::default();
    let scanner = DsnScanner::new(&text);
    let mut p = ReadScopeParameter::new(scanner, &options);
    assert_eq!(p.scanner.next_token().expect("scan"), Some(Token::Open));
    assert_eq!(
        p.scanner.next_token().expect("scan"),
        Some(Token::Kw(Keyword::PcbScope))
    );
    assert!(read_scope(ScopeKeyword::Pcb, &mut p).expect("no scan error"));
    let coordinate_transform = CoordinateTransform::new(10.0, 0.0, 0.0).expect("a valid scale");
    let mut board = p.board.take().expect("board built");
    board
        .components
        .set_flip_style_rotate_first(flip_style_rotate_first);
    add(&mut board);

    let mut out: Vec<u8> = Vec::new();
    {
        let sink: &mut dyn std::io::Write = &mut out;
        let file = IndentFileWriter::new(sink);
        let mut w = WriteScopeParameter::new(&board, file, "\"", &coordinate_transform, false);
        write_placement_scope(&mut w);
        w.file.flush().expect("flush");
    }
    String::from_utf8(out).expect("utf-8")
}

#[test]
fn an_unplaced_component_is_written_with_a_bare_place_scope() {
    let text = write_placement(
        |board| {
            let pkg = board.library.packages.add(
                "PKG",
                Vec::new(),
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                true,
            );
            board
                .components
                .add("C1", None, 0.0, true, pkg, pkg, false, None);
        },
        false,
    );
    assert_eq!(
        text,
        concat!(
            "\n(placement",
            "\n  (component PKG",
            "\n    (place \n      C1",
            "\n    )",
            "\n  )",
            "\n)",
        )
    );
}

#[test]
fn a_placed_component_writes_coordinates_side_rotation_and_pin_clearance_classes() {
    let text = write_placement(
        |board| {
            let padstack = board.library.padstacks.add_layer_range(
                Shape::Circle(Circle::new(IntPoint::new(0, 0), 100)),
                0,
                1,
            );
            let pin = PackagePin::new("D+", padstack, Vector::new(0, 0), 0.0);
            let pkg = board.library.packages.add(
                "PKG",
                vec![pin],
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                true,
            );
            let id = board
                .components
                .add(
                    "J1",
                    Some(Point::new(10_000, 20_000)),
                    90.0,
                    true,
                    pkg,
                    pkg,
                    false,
                    None,
                )
                .id;
            board.insert_pin(id, 0, Vec::new(), 1, FixedState::SystemFixed);
        },
        true,
    );
    assert_eq!(
        text,
        concat!(
            "\n(placement",
            "\n  (place_control (flip_style rotate_first))",
            "\n  (component PKG",
            "\n    (place \n      J1 1000 2000 front 90",
            "\n      (pin D+ (clearance_class default))",
            "\n    )",
            "\n  )",
            "\n)",
        )
    );
}

#[test]
fn the_lock_type_position_arm_survives_a_whole_file_read() {
    let (board, ct) = common::read_directed("lock-type");

    let components: Vec<String> = (1..=i32::try_from(board.components.count())
        .expect("two components"))
        .map(|i| {
            let c = board.components.get(i);
            format!(
                "[component] {i} {} positionFixed={} placed={} front={}",
                c.name,
                c.position_fixed,
                c.is_placed(),
                c.placed_on_front(),
            )
        })
        .collect();
    common::assert_rows_match(
        &components,
        &common::directed_rows("lock-type", "[component]"),
        "lock-type components",
    );

    let ctx = board.ctx();
    let pins: Vec<String> = {
        let mut ids = board.get_pins();
        ids.sort_unstable();
        ids.iter()
            .map(|id| {
                let item = board.get_item(*id).expect("pin id is live");
                let Item::Pin(pin) = item else {
                    panic!("get_pins answers pins");
                };
                let nets: Vec<String> = (0..item.header().net_count())
                    .map(|i| item.header().get_net_number(i).to_string())
                    .collect();
                format!(
                    "[pin] {} {}-{} fixed={} nets=[{}]",
                    id.0,
                    board
                        .components
                        .get(item.header().get_component_id())
                        .name
                        .clone(),
                    pin.name(&ctx).unwrap_or("null"),
                    match item.header().get_fixed_state() {
                        FixedState::Unfixed => "UNFIXED",
                        FixedState::ShoveFixed => "SHOVE_FIXED",
                        FixedState::UserFixed => "USER_FIXED",
                        FixedState::SystemFixed => "SYSTEM_FIXED",
                    },
                    nets.join(","),
                )
            })
            .collect()
    };
    common::assert_rows_match(
        &pins,
        &common::directed_rows("lock-type", "[pin]"),
        "lock-type pins",
    );

    let (expected, bytes) = common::directed_ses("lock-type", "ses");
    let actual = common::write_directed_ses(&board, &ct, "lock-type");
    assert_eq!(actual, expected, "lock-type SES");
    assert_eq!(actual.len(), bytes, "lock-type SES byte count");
    assert!(
        actual.contains(" (lock_type position))"),
        "the arm's whole observable effect on the SES side"
    );
}
