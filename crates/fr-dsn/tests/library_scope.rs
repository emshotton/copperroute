//! `Library.readScope`/`writeScope`, `Package` and `PartLibrary`.
//!
//! Java authority: `io/specctra/parser/{Library,Package,PartLibrary}.java`.
//! Every expected number and string below was produced by running the pinned 2.3.0 jar once (or
//! read out of `tests/reference/`, itself generated with that jar); the command that produced it
//! is recorded on the test.
//!
//! The probe the read tests cite is `LibProbe.java` — a throwaway `DsnReader.readBoard` +
//! `board.library` dump, compiled and run as
//! ```text
//! javac -cp tools/freerouting-2.3.0.jar -d <dir> LibProbe.java
//! java -Djava.awt.headless=true -cp <dir>:tools/freerouting-2.3.0.jar LibProbe <file.dsn>
//! ```
//! with `/opt/homebrew/opt/openjdk@25/bin/{javac,java}`. Its body is reproduced in
//! `.superpowers/sdd/2026-08-28-plan-3-dsn/task-7-report.md`.

use std::io::Write;

use fr_board::PadstackId;
use fr_dsn::keyword::{Keyword, ScopeKeyword};
use fr_dsn::lexer::{DsnScanner, Token};
use fr_dsn::parser::library::{write_library_scope, write_padstack_scope};
use fr_dsn::parser::part_library::write_part_library_scope;
use fr_dsn::parser::scope_parameter::{
    DsnReadOptions, ReadScopeParameter, WriteScopeParameter, read_scope,
};

/// Reads a whole `(pcb …)` file the way a later task's `DsnReader::read_board` will.
fn read_pcb<T>(text: &str, f: impl FnOnce(bool, &mut ReadScopeParameter<'_>) -> T) -> T {
    let options = DsnReadOptions::default();
    let scanner = DsnScanner::new(text).expect("fits the lexer buffer");
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
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../freerouting/fixtures/"
    );
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// The `(library …)` block of a committed 2.3.0 reference round-trip, dedented by the two
/// spaces the enclosing `(pcb …)` scope adds, so it can be compared against a writer run at
/// indent level 0.
fn reference_scope(design: &str, header: &str) -> String {
    let path = format!(
        "{}/../../tests/reference/{design}/roundtrip.dsn",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
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
    assert!(inside, "no `{header}` scope in {path}");
    // The writers emit no trailing newline after the closing bracket.
    out.pop();
    out
}

/// Runs `write` against the board a read of `text` produced, returning the bytes, with the
/// leading newline `IndentFileWriter.startScope()` always emits stripped.
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

/// The two-layer preamble the synthetic fixtures below share.
fn synthetic(body: &str) -> String {
    format!(
        "(pcb synthetic.dsn\n  (parser\n    (string_quote \")\n  )\n  (resolution um 10)\n  \
         (unit um)\n  (structure\n    (layer F.Cu\n      (type signal)\n    )\n    (layer B.Cu\n \
         (type signal)\n    )\n    (boundary\n      (rect pcb 0 0 10000 10000)\n    )\n  \
         )\n{body}\n)\n"
    )
}

// ------------------------------------------------------------------- Library.readScope

#[test]
fn issue026_library_scope_reads_five_padstacks_and_two_packages() {
    // LibProbe on ../freerouting/fixtures/Issue026-J2_reference.dsn:
    //   padstackCount=5
    //   padstack 1 name='Round[A]Pad_1358_um' from_layer=0 to_layer=1 attach_allowed=false
    //             placed_absolute=false
    //   padstack 2 name='Rect[T]Pad_2680x3600_um' from_layer=0 to_layer=0
    //   packageCount=2
    //   package 1 name='TE_1888019-6:TE_1888019-6' pinCount=42 isFront=true outlines=17
    //             keepouts=4 viaKeepouts=0 placeKeepouts=0
    //   package 2 name='TE_1-84953-5:TE_1-84953-5' pinCount=17 isFront=true outlines=0
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
    // LibProbe, same command:
    //   pin 0 name='S1' padstack=1 rel=(-89000.0,-57200.0) rot=0.0
    //   pin 16 name='A6' padstack=3 rel=(-26000.0,51800.0) rot=0.0
    //   pin 41 name='B18' padstack=3 rel=(66000.0,76900.0) rot=0.0
    //   package 2: pin 16 name='PAD.' padstack=2 rel=(99891.0,1980.0) rot=0.0
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
    // LibProbe, same command:
    //   keepout 0 name='keepout_1' layer=0 area=Circle: center (-80000,0)radius 7,950
    //   keepout 1 name='keepout_2' layer=1
    //   keepout 2 name='keepout_3' layer=0 area=Circle: center (80000,0)radius 7,950
    //   keepout 3 name='keepout_4' layer=1
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
    // LibProbe on the synthetic file below (written to a temp path and read the same way):
    //   padstack 1 name='BOTTOMONLY' from_layer=1 to_layer=1 attach_allowed=true
    //             placed_absolute=true shape0=null shape1=IntBox
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
        // `(attach off)` absent -> Java's `isDrilllable` stays true.
        assert!(padstack.attach_allowed);
        assert!(padstack.placed_absolute);
        assert!(padstack.get_shape(0).is_none());
        assert!(padstack.get_shape(1).is_some());
    });
}

#[test]
fn pin_coordinates_are_rounded_with_javas_half_up_rule() {
    // LibProbe on the same synthetic file: `(pin BOTTOMONLY 1 100.55 -200.55)` at resolution 10
    // gives `pin 0 name='1' padstack=1 rel=(1006.0,-2005.0) rot=0.0` — `Math.round` is half-up
    // *towards positive infinity*, so -2005.5 rounds to -2005, not -2006 (Library.java:317,319).
    // `(pin BOTTOMONLY (rotate 90) 2 0 0)` gives `pin 1 name='2' rel=(0.0,0.0) rot=90.0`.
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
        assert_eq!(pin0.relative_location.to_float().y, -2005.0);
        assert_eq!(pin0.rotation_in_degree, 0.0);
        let pin1 = pkg.get_pin(1).expect("pin 1");
        assert_eq!(pin1.name, "2");
        assert_eq!(pin1.rotation_in_degree, 90.0);
    });
}

#[test]
fn package_outlines_keepouts_and_side_match_java() {
    // LibProbe on the synthetic file below:
    //   package 1 name='IMG' pinCount=1 isFront=false outlines=2 keepouts=1 viaKeepouts=1
    //             placeKeepouts=1
    //   keepout 0 name='keepout_1' layer=0
    //   viaKeepout 0 name='vk' layer=1
    //   placeKeepout 0 name='pk' layer=0
    //   outline 0 width=100.0 closed=true
    //   outline 1 width=0.0 closed=true
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
    // The control for the `closed=true` case above: LibProbe on the same file with the path's
    // last corner moved off its first prints `outline 0 width=100.0 closed=false`.
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
    // Library.readScope's `::<n>` loop (Library.java:409-449): an identical repeat is dropped,
    // a differing one is added as `<name>::1`. LibProbe on the file below prints
    //   packageCount=2
    //   package 1 name='IMG' pinCount=1
    //   package 2 name='IMG::1' pinCount=1
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
    // Library.readPadstackScope:158-161 — `if (boardPadstacks.get(name) != null) return true;`.
    // LibProbe on the file below prints `padstackCount=1` with `from_layer=0 to_layer=0`.
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

// ------------------------------------------------------------------ Library.writeScope

#[test]
fn write_padstack_scope_emits_the_2_3_0_bytes_for_a_round_via_pad() {
    // `tests/reference/tutorial_board/roundtrip.dsn:50-58`, generated by
    // `./scripts/gen-reference.sh` against tools/freerouting-2.3.0.jar. `Circle.writeScope`
    // writes all three `coor` entries, so the diameter is followed by the centre — the plan
    // brief's `(shape (circle F.Cu 600.0))` is one datum short (Java wins).
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
    // Bit-parity against `tests/reference/Issue026-J2_reference/roundtrip.dsn`'s `(library …)`
    // block (lines 118-444), generated by `./scripts/gen-reference.sh` against
    // tools/freerouting-2.3.0.jar. Nothing after `Library.readScope` mutates
    // `board.library.padstacks`/`.packages` in the DSN reader, so a library-only read is enough
    // to reproduce the block byte for byte.
    let text = fixture("Issue026-J2_reference.dsn");
    let out = write_with(&text, write_library_scope);
    assert_eq!(out, reference_scope("Issue026-J2_reference", "library"));
}

// ------------------------------------------------------------------------- PartLibrary

#[test]
fn part_library_scope_reads_logical_parts_and_mappings() {
    // LibProbe on a synthetic file with the same `part_library` scope (plus the `placement`,
    // `library` and `network` scopes `Network.insertLogicalParts` needs) prints
    //   logicalPartCount=1
    //   logicalPart 1 name='LP1' pinCount=2
    //     partPin 0 index=0 pin='1' gate='GATEA' gateSwap=3 gatePin='A' gatePinSwap=4
    //     partPin 1 index=1 pin='2' gate='GATEA' gateSwap=3 gatePin='B' gatePinSwap=4
    // `PartLibrary.readScope` itself only fills `ReadScopeParameter.logicalParts`/
    // `.logicalPartMappings` in *file* order (Network's `LogicalParts.add` is what sorts by pin
    // index later, and that is Task 9's), so this asserts the file order `2`, `1`.
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
        // Java's `SortedSet<String>` (a `TreeSet`) — sorted, not file order.
        assert_eq!(mapping.components, ["U1", "U2"]);
    });
}

#[test]
fn write_part_library_scope_emits_the_2_3_0_logical_part_literal() {
    // Plan ruling 1: `PartLibrary.writeScope` at the clone's HEAD writes `"logicalPart "`
    // (PartLibrary.java:58), one of the fifteen camelCased regressions; 2.3.0 writes
    // `"logical_part "`, and that is what the port emits. Verified against the pinned jar:
    //   javap -c -p -cp tools/freerouting-2.3.0.jar \
    //       app.freerouting.io.specctra.parser.PartLibrary | grep -A2 'ldc.*logical'
    // shows `String logical_part_mapping ` and `String logical_part `.
    //
    // No fixture in ../freerouting/fixtures carries a `part_library` scope, so the bytes were
    // pinned by round-tripping a synthetic one through the 2.3.0 writer:
    //   javac -cp tools/freerouting-2.3.0.jar -d <dir> scripts/gen-reference/RefWriter.java
    //   java -Djava.awt.headless=true -cp <dir>:tools/freerouting-2.3.0.jar RefWriter \
    //        parts.dsn parts.out.dsn parts.out.ses
    // whose `parts.out.dsn:56-65` reads (the `logical_part` line is followed by a blank line
    // carrying the scope's four-space indent, from `writeScope`'s `newLine()` + the per-pin
    // `newLine()`):
    //   (part_library
    //     (logical_part_mapping LP1
    //       (comp U1)
    //     )
    //     (logical_part LP1
    //
    //       (pin "1" 0 GATEA 3 A 4)
    //       (pin "2" 0 GATEA 3 B 4)
    //     )
    //   )
    // The pin *name* goes through `IdentifierType.write`, which quotes anything matching
    // `^-?\d.*`; the two swap codes go through `String.valueOf(int)` and stay bare.
    let text = synthetic(
        "  (part_library\n    (logical_part_mapping LP1\n      (comp U1)\n    )\n    \
         (logical_part LP1\n      (pin 1 0 GATEA 3 A 4)\n    )\n  )",
    );
    // The board has no components (the `placement` reader is Task 8's), so the `(comp …)` list
    // is empty; the logical parts themselves come from the read.
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
    // `PartLibrary.writeScope:25-27` — `if (logicalParts.count() <= 0) return;`.
    let text = synthetic("  (library\n  )");
    let out = write_with(&text, write_part_library_scope);
    assert_eq!(out, "");
}
