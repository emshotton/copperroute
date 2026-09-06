use std::io::Write;

use fr_board::PadstackId;
use fr_dsn::keyword::{Keyword, ScopeKeyword};
use fr_dsn::lexer::{DsnScanner, Token};
use fr_dsn::parser::library::{write_library_scope, write_padstack_scope};
use fr_dsn::parser::part_library::write_part_library_scope;
use fr_dsn::parser::scope_parameter::{
    DsnReadOptions, ReadScopeParameter, WriteScopeParameter, read_scope,
};

fn read_pcb<T>(text: &str, f: impl FnOnce(bool, &mut ReadScopeParameter<'_>) -> T) -> T {
    let options = DsnReadOptions::default();
    let scanner = DsnScanner::new(text);
    let mut p = ReadScopeParameter::new(scanner, &options);
    assert_eq!(p.scanner.next_token().expect("scan"), Some(Token::Open));
    assert_eq!(
        p.scanner.next_token().expect("scan"),
        Some(Token::Kw(Keyword::PcbScope))
    );
    let ok = read_scope(ScopeKeyword::Pcb, &mut p).expect("no scan error");
    f(ok, &mut p)
}

fn fixture(name: &str) -> String {
    let path = parity::fixture(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()))
}

fn reference_scope(design: &str, header: &str) -> String {
    let path = parity::reference(design, "roundtrip.dsn");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let open = format!("  ({header}");
    let mut out = String::new();
    let mut inside = false;
    for line in text.lines() {
        if !inside && (line == open || line.starts_with(&format!("{open} "))) {
            inside = true;
        }
        if inside {
            out.push_str(line.strip_prefix("  ").unwrap_or(line));
            out.push('\n');
            if line == "  )" {
                break;
            }
        }
    }
    assert!(inside, "no `{header}` scope in {}", path.display());
    out.pop();
    out
}

fn write_with(text: &str, write: impl FnOnce(&mut WriteScopeParameter<'_>)) -> String {
    read_pcb(text, |ok, p| {
        assert!(ok);
        let coordinate_transform = p.coordinate_transform.expect("coordinate transform set");
        let board = p.board.as_ref().expect("board built");
        let mut buf: Vec<u8> = Vec::new();
        {
            let sink: &mut dyn Write = &mut buf;
            let file = fr_dsn::format::IndentFileWriter::new(sink);
            let mut wp = WriteScopeParameter::new(board, file, "\"", &coordinate_transform, false);
            write(&mut wp);
            wp.file.flush().expect("write");
        }
        let out = String::from_utf8(buf).expect("utf-8");
        out.strip_prefix('\n').unwrap_or(&out).to_string()
    })
}

fn synthetic(body: &str) -> String {
    format!(
        "(pcb synthetic.dsn\n  (parser\n    (string_quote \")\n  )\n  (resolution um 10)\n  \
         (unit um)\n  (structure\n    (layer F.Cu\n      (type signal)\n    )\n    (layer B.Cu\n \
         (type signal)\n    )\n    (boundary\n      (rect pcb 0 0 10000 10000)\n    )\n  \
         )\n{body}\n)\n"
    )
}

#[test]
fn issue026_library_scope_reads_five_padstacks_and_two_packages() {
    let text = fixture("Issue026-J2_reference.dsn");
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let board = p.board.as_ref().expect("board built");
        let padstacks = &board.library.padstacks;
        assert_eq!(padstacks.count(), 5);
        let first = padstacks.get(PadstackId(1)).expect("padstack 1");
        assert_eq!(first.name, "Round[A]Pad_1358_um");
        assert_eq!(first.from_layer(), 0);
        assert_eq!(first.to_layer(), 1);
        assert!(!first.attach_allowed);
        assert!(!first.placed_absolute);
        let second = padstacks.get(PadstackId(2)).expect("padstack 2");
        assert_eq!(second.name, "Rect[T]Pad_2680x3600_um");
        assert_eq!(second.from_layer(), 0);
        assert_eq!(second.to_layer(), 0);

        let packages = &board.library.packages;
        assert_eq!(packages.count(), 2);
        let pkg = packages.get(1);
        assert_eq!(pkg.name, "TE_1888019-6:TE_1888019-6");
        assert_eq!(pkg.pin_count(), 42);
        assert!(pkg.is_front);
        assert_eq!(pkg.outline.as_ref().expect("outline array").len(), 17);
        assert_eq!(pkg.keepouts.len(), 4);
        assert!(pkg.via_keepouts.is_empty());
        assert!(pkg.place_keepouts.is_empty());
        assert_eq!(packages.get(2).pin_count(), 17);
        assert_eq!(packages.get(2).outline.as_ref().expect("outline").len(), 0);
    });
}

#[test]
fn issue026_pin_padstacks_and_relative_locations_match_java() {
    let text = fixture("Issue026-J2_reference.dsn");
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let board = p.board.as_ref().expect("board built");
        let pkg = board.library.packages.get(1);
        let s1 = pkg.get_pin(0).expect("pin 0");
        assert_eq!(s1.name, "S1");
        assert_eq!(s1.padstack_no, PadstackId(1));
        assert_eq!(s1.relative_location.to_float().x, -89_000.0);
        assert_eq!(s1.relative_location.to_float().y, -57_200.0);
        assert_eq!(s1.rotation_in_degree, 0.0);
        let a6 = pkg.get_pin(16).expect("pin 16");
        assert_eq!(a6.name, "A6");
        assert_eq!(a6.padstack_no, PadstackId(3));
        let b18 = pkg.get_pin(41).expect("pin 41");
        assert_eq!(b18.name, "B18");
        assert_eq!(b18.relative_location.to_float().x, 66_000.0);
        assert_eq!(b18.relative_location.to_float().y, 76_900.0);

        let pad = board.library.packages.get(2).get_pin(16).expect("pin 16");
        assert_eq!(pad.name, "PAD.");
        assert_eq!(pad.padstack_no, PadstackId(2));
        assert_eq!(pad.relative_location.to_float().x, 99_891.0);
        assert_eq!(pad.relative_location.to_float().y, 1980.0);
    });
}

#[test]
fn issue026_keepout_names_are_generated_when_the_dsn_leaves_them_empty() {
    let text = fixture("Issue026-J2_reference.dsn");
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let pkg = p.board.as_ref().expect("board").library.packages.get(1);
        let names: Vec<&str> = pkg.keepouts.iter().map(|k| k.name.as_str()).collect();
        assert_eq!(names, ["keepout_1", "keepout_2", "keepout_3", "keepout_4"]);
        let layers: Vec<i32> = pkg.keepouts.iter().map(|k| k.layer).collect();
        assert_eq!(layers, [0, 1, 0, 1]);
        assert_eq!(pkg.keepouts[0].area.bounding_box().ll.x, -87_950);
        assert_eq!(pkg.keepouts[2].area.bounding_box().ll.x, 72_050);
    });
}

#[test]
fn a_padstack_with_no_shape_on_the_top_layer_starts_at_layer_one() {
    let text = synthetic(
        "  (library\n    (padstack BOTTOMONLY\n      (shape (rect B.Cu -100 -100 100 100))\n      \
         (absolute on)\n    )\n  )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let padstacks = &p.board.as_ref().expect("board").library.padstacks;
        assert_eq!(padstacks.count(), 1);
        let padstack = padstacks.get(PadstackId(1)).expect("padstack 1");
        assert_eq!(padstack.name, "BOTTOMONLY");
        assert_eq!(padstack.from_layer(), 1);
        assert_eq!(padstack.to_layer(), 1);
        assert!(padstack.attach_allowed);
        assert!(padstack.placed_absolute);
        assert!(padstack.get_shape(0).is_none());
        assert!(padstack.get_shape(1).is_some());
    });
}

#[test]
fn pin_coordinates_are_rounded_away_from_zero() {
    let text = synthetic(
        "  (library\n    (image IMG\n      (side back)\n      (pin BOTTOMONLY 1 100.55 \
         -200.55)\n      (pin BOTTOMONLY (rotate 90) 2 0 0)\n    )\n    (padstack BOTTOMONLY\n   \
         (shape (rect B.Cu -100 -100 100 100))\n    )\n  )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let pkg = p.board.as_ref().expect("board").library.packages.get(1);
        assert!(!pkg.is_front);
        let pin0 = pkg.get_pin(0).expect("pin 0");
        assert_eq!(pin0.name, "1");
        assert_eq!(pin0.relative_location.to_float().x, 1006.0);
        assert_eq!(pin0.relative_location.to_float().y, -2006.0);
        assert_eq!(pin0.rotation_in_degree, 0.0);
        let pin1 = pkg.get_pin(1).expect("pin 1");
        assert_eq!(pin1.name, "2");
        assert_eq!(pin1.rotation_in_degree, 90.0);
    });
}

#[test]
fn package_outlines_keepouts_and_side_match_java() {
    let text = synthetic(
        "  (library\n    (image IMG\n      (side back)\n      (pin BOTTOMONLY 1 0 0)\n      \
         (outline (path signal 100  0 0  1000 0  1000 1000  0 0))\n      (outline (rect signal \
         -10 -10 10 10))\n      (keepout \"\" (circle F.Cu 500 0 0))\n      (via_keepout \"vk\" \
         (rect B.Cu -5 -5 5 5))\n      (place_keepout \"pk\" (rect F.Cu -6 -6 6 6))\n    )\n    \
         (padstack BOTTOMONLY\n      (shape (rect B.Cu -100 -100 100 100))\n    )\n  )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let pkg = p.board.as_ref().expect("board").library.packages.get(1);
        assert_eq!(pkg.name, "IMG");
        assert!(!pkg.is_front);
        assert_eq!(pkg.outline.as_ref().expect("outline").len(), 2);
        assert_eq!(
            pkg.outline_widths.as_ref().expect("widths").as_slice(),
            [100.0, 0.0]
        );
        assert_eq!(
            pkg.outline_is_closed.as_ref().expect("closed").as_slice(),
            [true, true]
        );
        assert_eq!(pkg.keepouts.len(), 1);
        assert_eq!(pkg.keepouts[0].name, "keepout_1");
        assert_eq!(pkg.keepouts[0].layer, 0);
        assert_eq!(pkg.via_keepouts.len(), 1);
        assert_eq!(pkg.via_keepouts[0].name, "vk");
        assert_eq!(pkg.via_keepouts[0].layer, 1);
        assert_eq!(pkg.place_keepouts.len(), 1);
        assert_eq!(pkg.place_keepouts[0].name, "pk");
        assert_eq!(pkg.place_keepouts[0].layer, 0);
    });
}

#[test]
fn an_open_path_outline_is_not_closed() {
    let text = synthetic(
        "  (library\n    (image IMG\n      (outline (path signal 100  0 0  1000 0  1000 \
         1000))\n    )\n  )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let pkg = p.board.as_ref().expect("board").library.packages.get(1);
        assert_eq!(
            pkg.outline_is_closed.as_ref().expect("closed").as_slice(),
            [false]
        );
    });
}

#[test]
fn a_second_image_with_the_same_name_and_the_same_pins_is_deduplicated() {
    let text = synthetic(
        "  (library\n    (image IMG\n      (pin PS 1 0 0)\n    )\n    (image IMG\n      (pin PS 1 \
         0 0)\n    )\n    (image IMG\n      (pin PS 1 100 0)\n    )\n    (padstack PS\n      \
         (shape (rect F.Cu -100 -100 100 100))\n    )\n  )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let packages = &p.board.as_ref().expect("board").library.packages;
        assert_eq!(packages.count(), 2);
        assert_eq!(packages.get(1).name, "IMG");
        assert_eq!(packages.get(2).name, "IMG::1");
    });
}

#[test]
fn a_repeated_padstack_name_keeps_the_first_definition() {
    let text = synthetic(
        "  (library\n    (padstack PS\n      (shape (rect F.Cu -100 -100 100 100))\n    )\n    \
         (padstack PS\n      (shape (rect B.Cu -100 -100 100 100))\n    )\n  )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let padstacks = &p.board.as_ref().expect("board").library.padstacks;
        assert_eq!(padstacks.count(), 1);
        assert_eq!(padstacks.get(PadstackId(1)).expect("ps").from_layer(), 0);
        assert_eq!(padstacks.get(PadstackId(1)).expect("ps").to_layer(), 0);
    });
}

#[test]
fn write_padstack_scope_emits_the_2_3_0_bytes_for_a_round_via_pad() {
    let text = synthetic(
        "  (library\n    (padstack VIA\n      (shape (circle F.Cu 600 0 0))\n      (shape (circle \
         B.Cu 600 0 0))\n      (attach off)\n    )\n  )",
    );
    let out = write_with(&text, |wp| {
        let padstack = wp
            .board
            .library
            .padstacks
            .get(PadstackId(1))
            .expect("padstack 1")
            .clone();
        write_padstack_scope(wp, &padstack);
    });
    assert_eq!(
        out,
        "(padstack VIA\n  (shape\n    (circle F.Cu 600.0 0.0 0.0)\n  )\n  (shape\n    (circle \
         B.Cu 600.0 0.0 0.0)\n  )\n  (attach off)\n)"
    );
}

#[test]
fn write_library_scope_reproduces_the_2_3_0_reference_bytes() {
    let text = fixture("Issue026-J2_reference.dsn");
    let out = write_with(&text, write_library_scope);
    assert_eq!(out, reference_scope("Issue026-J2_reference", "library"));
}

#[test]
fn part_library_scope_reads_logical_parts_and_mappings() {
    let text = synthetic(
        "  (part_library\n    (logical_part_mapping LP1\n      (comp U2 U1)\n    )\n    \
         (logical_part LP1\n      (pin 2 0 GATEA 3 B 4)\n      (pin 1 0 GATEA 3 A 4)\n    )\n  )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        assert_eq!(p.logical_parts.len(), 1);
        let part = &p.logical_parts[0];
        assert_eq!(part.name, "LP1");
        assert_eq!(part.part_pins.len(), 2);
        assert_eq!(part.part_pins[0].pin_name, "2");
        assert_eq!(part.part_pins[0].gate_name, "GATEA");
        assert_eq!(part.part_pins[0].gate_swap_code, 3);
        assert_eq!(part.part_pins[0].gate_pin_name, "B");
        assert_eq!(part.part_pins[0].gate_pin_swap_code, 4);
        assert_eq!(part.part_pins[1].pin_name, "1");
        assert_eq!(part.part_pins[1].gate_pin_name, "A");

        assert_eq!(p.logical_part_mappings.len(), 1);
        let mapping = &p.logical_part_mappings[0];
        assert_eq!(mapping.name, "LP1");
        assert_eq!(mapping.components, ["U1", "U2"]);
    });
}

#[test]
fn write_part_library_scope_emits_the_2_3_0_logical_part_literal() {
    let text = synthetic(
        "  (part_library\n    (logical_part_mapping LP1\n      (comp U1)\n    )\n    \
         (logical_part LP1\n      (pin 1 0 GATEA 3 A 4)\n    )\n  )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let coordinate_transform = p.coordinate_transform.expect("coordinate transform");
        let logical_parts = p.logical_parts.clone();
        let board = p.board.as_mut().expect("board built");
        for part in &logical_parts {
            let pins = part
                .part_pins
                .iter()
                .enumerate()
                .map(|(i, pin)| {
                    fr_board::PartPin::new(
                        i as i32,
                        pin.pin_name.clone(),
                        pin.gate_name.clone(),
                        pin.gate_swap_code,
                        pin.gate_pin_name.clone(),
                        pin.gate_pin_swap_code,
                    )
                })
                .collect();
            board.library.logical_parts.add(part.name.clone(), pins);
        }
        let mut buf: Vec<u8> = Vec::new();
        {
            let sink: &mut dyn Write = &mut buf;
            let file = fr_dsn::format::IndentFileWriter::new(sink);
            let mut wp = WriteScopeParameter::new(board, file, "\"", &coordinate_transform, false);
            write_part_library_scope(&mut wp);
            wp.file.flush().expect("write");
        }
        let out = String::from_utf8(buf).expect("utf-8");
        let out = out.strip_prefix('\n').unwrap_or(&out).to_string();
        assert_eq!(
            out,
            "(part_library\n  (logical_part_mapping LP1\n    (comp)\n  )\n  (logical_part \
             LP1\n    \n    (pin \"1\" 0 GATEA 3 A 4)\n  )\n)"
        );
    });
}

#[test]
fn an_empty_part_library_writes_nothing() {
    let text = synthetic("  (library\n  )");
    let out = write_with(&text, write_part_library_scope);
    assert_eq!(out, "");
}
