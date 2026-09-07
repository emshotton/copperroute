//! Rust twin of `scripts/differential/java/P3T15.java` (Plan 3 Task 15).
//!
//! Reads one Specctra DSN through `copper-dsn` and dumps either the board it built (mode 0) or one
//! of the three writers' output verbatim (modes 1-3); mode 4 is the `p3t3` token stream, so a
//! single driver can sweep the corpus. See `scripts/differential/README.md`.
//!
//! Because modes 0-3 go through the whole parser rather than the bare scanner, this driver
//! exercises the hand-rolled `next_string`/`next_string_list`/`next_double` bypass path that
//! `p3t3`'s pure `next_token` loop never reaches.

use std::io::{BufWriter, Write};

use copper_board::board::Board;
use copper_board::items::Item;
use copper_dsn::parser::scope_parameter::DsnReadOptions;
use copper_dsn::BoardReadResult;

#[path = "../token_dump.rs"]
mod token_dump;

fn main() {
    let mut args = std::env::args().skip(1);
    let (path, mode) = match (args.next(), args.next()) {
        (Some(path), Some(mode)) => (path, mode.parse::<u8>().expect("mode must be 0-4")),
        _ => {
            eprintln!("usage: p3t15 <file.dsn> <mode 0-4>");
            std::process::exit(2);
        }
    };

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    if mode == 4 {
        token_dump::dump(&mut out, &path);
        return;
    }

    // `RefWriter.java:26` / `P3T15.java`: the file name with a trailing `.dsn` stripped.
    let design_name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .strip_suffix(".dsn")
        .unwrap_or_default()
        .to_string();

    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("cannot open {path}: {e}"));
    let options = DsnReadOptions::default();
    let result = copper_dsn::read_board(file, None, Some(&design_name), &options);

    let (board, ct, warnings) = match result {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            warnings,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            warnings,
            ..
        } => match board {
            Some(board) => (*board, coordinate_transform, warnings),
            None => {
                writeln!(out, "RESULT NoBoard").expect("write");
                return;
            }
        },
        // fixed: T4 (#91) — the port's fifth variant. Java has no counterpart, so a fixture that
        // reached it would print a line the Java side cannot, i.e. it would show up as a DIFF in
        // `sweep-p3t15.sh` rather than pass silently.
        BoardReadResult::Partial { diagnostic, .. } => {
            writeln!(out, "RESULT Partial | {diagnostic}").expect("write");
            return;
        }
        BoardReadResult::ParseError { location, detail } => {
            writeln!(out, "RESULT ParseError {location} | {detail}").expect("write");
            return;
        }
        BoardReadResult::IoError(_) => {
            writeln!(out, "RESULT IoError").expect("write");
            return;
        }
    };

    // Java's writers re-derive the transform from `board.communication`; this port takes it
    // explicitly (Task 10, controller ruling A), and `Structure.createBoard`'s is the one that
    // round-trips a file's coordinates unchanged.
    let ct = ct.unwrap_or_else(|| panic!("{path} produced no coordinate transform"));

    match mode {
        0 => dump_items(&mut out, &board, &warnings),
        1 => copper_dsn::dsn_writer::write(&board, &ct, &mut out, &design_name, false).expect("write"),
        2 => copper_dsn::ses_writer::write(&board, &ct, &mut out, &design_name).expect("write"),
        3 => copper_dsn::rules_writer::write(&board, &ct, None, &mut out, &design_name).expect("write"),
        _ => {
            eprintln!("unknown mode: {mode}");
            std::process::exit(2);
        }
    }
}

/// `P3T15.dumpItems`.
fn dump_items(out: &mut impl Write, board: &Board, warnings: &[String]) {
    writeln!(out, "layers {}", board.get_layer_count()).expect("write");
    let mut ids = board.items_in_board_order();
    ids.sort();
    let mut count = 0usize;
    for id in ids {
        let item = board.get_item(id).expect("item in board order exists");
        writeln!(out, "item {} {}", id.0, describe(item, board)).expect("write");
        count += 1;
    }
    writeln!(out, "itemcount {count}").expect("write");
    for w in warnings {
        writeln!(out, "warning {w}").expect("write");
    }
}

/// `P3T15.describe`. `kind` is Java's `getClass().getSimpleName()`.
fn describe(item: &Item, board: &Board) -> String {
    let ctx = board.ctx();
    let kind = match item {
        // The Rust variant is `Trace`; the only concrete Java subclass is `PolylineTrace`.
        Item::Trace(_) => "PolylineTrace",
        Item::Via(_) => "Via",
        Item::Pin(_) => "Pin",
        Item::ObstacleArea(_) => "ObstacleArea",
        Item::ConductionArea(_) => "ConductionArea",
        Item::ViaObstacleArea(_) => "ViaObstacleArea",
        Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
        Item::ComponentOutline(_) => "ComponentOutline",
        Item::BoardOutline(_) => "BoardOutline",
    };
    let mut nets = String::new();
    for i in 0..item.net_count() {
        nets.push_str(&format!("{},", item.get_net_number(i)));
    }
    let box_ = item.bounding_box(&ctx);
    format!(
        "{kind} layers={}..{} nets=[{nets}] cl={} fixed={} cmp={} bbox=({},{},{},{}) tiles={}",
        item.first_layer(&ctx),
        item.last_layer(&ctx),
        item.clearance_class(),
        fixed_state_name(item),
        item.component_id(),
        box_.ll.x,
        box_.ll.y,
        box_.ur.x,
        box_.ur.y,
        item.tile_shape_count(&ctx),
    )
}

/// `FixedState.name()` — the Java enum constant's spelling.
fn fixed_state_name(item: &Item) -> &'static str {
    use copper_board::structure::layer::FixedState;
    match item.get_fixed_state() {
        FixedState::Unfixed => "UNFIXED",
        FixedState::ShoveFixed => "SHOVE_FIXED",
        FixedState::UserFixed => "USER_FIXED",
        FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}
