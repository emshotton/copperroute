//! The KiCad board-JSON **package-deduplication ladder** — `KiCadJsonReader.java:580-619`.
//!
//! `:603`'s `catch (Exception e)` looks like a component skip and is not one: it logs "package
//! deduplication error, falling back" and adds a **duplicate** package under the raw
//! `comp.footprint`, then carries on. The one throw it could catch was
//! `arePackagePinsIdentical:908`'s `pin1.name.equals(pin2.name)` over a `null` pin name, so a
//! board whose pads had no `name` silently gained one package per component — three components,
//! three packages all called `NONAME`.
//!
//! fixed: T7 (#285) — the throw is refused at the DTO boundary (see `kicad_dto.rs`), so the
//! fallback is unreachable and is gone with the two side tables that fed it. This file is what
//! replaces the three-`NONAME` reproduction: the same three components, expressing "unnamed" the
//! way a board can still express it.

use fr_board::Board;
use fr_dsn::error::BoardReadResult;
use fr_dsn::kicad::read_board;

fn board(json: &str) -> Board {
    match read_board(json, None) {
        BoardReadResult::Success { board: Some(b), .. } => *b,
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

/// Three components sharing a footprint, each with one pad named `""`.
///
/// A `null` pad name is refused now (`kicad_dto.rs`), so `""` is what a board with nothing to say
/// about its pads writes — and `""` never threw, which is exactly why this case was invisible
/// under Java: it deduplicated correctly while its `null` sibling produced three `NONAME`
/// packages beside it. One package, and it carries the footprint's name rather than the raw
/// `comp.footprint` fallback the `catch` used.
#[test]
fn three_components_with_unnamed_pads_yield_one_package() {
    let board = board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[
              {"reference":"U1","footprint":"SOT-23","position":{"x":0,"y":0},
               "pads":[{"name":"","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]},
              {"reference":"U2","footprint":"SOT-23","position":{"x":5,"y":0},
               "pads":[{"name":"","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]},
              {"reference":"U3","footprint":"SOT-23","position":{"x":10,"y":0},
               "pads":[{"name":"","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]}
            ]}"#,
    );
    assert_eq!(
        board.library.packages.count(),
        1,
        "three identical components share one package"
    );
    assert_eq!(board.library.packages.get(1).name, "SOT-23");
    assert_eq!(board.components.count(), 3, "and all three still load");
}

/// The ladder itself is untouched: three components with the **same** footprint and genuinely
/// different pins still get three packages, under the `<base>::<suffix>` names `Packages.get`
/// strips back to the base.
#[test]
fn three_components_with_different_pins_still_yield_three_packages() {
    let board = board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[
              {"reference":"U1","footprint":"SOT-23","position":{"x":0,"y":0},
               "pads":[{"name":"1","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]},
              {"reference":"U2","footprint":"SOT-23","position":{"x":5,"y":0},
               "pads":[{"name":"2","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]},
              {"reference":"U3","footprint":"SOT-23","position":{"x":10,"y":0},
               "pads":[{"name":"3","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]}
            ]}"#,
    );
    assert_eq!(board.library.packages.count(), 3);
    assert_eq!(board.library.packages.get(1).name, "SOT-23");
    assert_eq!(board.library.packages.get(2).name, "SOT-23::1");
    assert_eq!(board.library.packages.get(3).name, "SOT-23::2");
}
