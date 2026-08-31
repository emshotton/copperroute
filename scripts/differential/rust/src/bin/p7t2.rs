//! Rust half of the `p7t2` differential pair — `scripts/differential/java/P7T2.java`.
//!
//! Plan 7 Task 9, pass level: one whole autoroute pass —
//! `AutoroutePassRunner::run_single_thread` (`AutoroutePassRunner.java:151-336`) — over a real DSN
//! board.
//!
//! Usage: `p7t2 <dsn> [passNo] [maxItems|all]`. See `P7T2.java`'s class comment for the two
//! halves, for why the `[real]` half is what makes the `[transcript]` half evidence, and for the
//! budget note. The shared ladder is [`p7t_common`].
//!
//! # The board comparison
//!
//! Java compares its two boards with `BasicBoard.getHash()`, an MD5 over `serialize(true)`; this
//! side compares with [`fr_board::Board::structural_hash`], a `u64` over a Rust-native input. The
//! two are **not** comparable by value and neither is printed — what crosses the diff is the
//! **decision** `equalsTranscript=<bool>`, which is controller ruling AH's rule and what Task 3
//! audited the port's hash against.

use std::io::{BufWriter, Write};

#[path = "../p7t_common.rs"]
mod p7t_common;

use p7t_common::PassState;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p7t2 <dsn> [passNo] [maxItems|all]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let pass_no: i32 = args.get(1).map_or(1, |a| a.parse().expect("passNo"));
    let max_items_arg: &str = args
        .get(2)
        .filter(|a| !a.is_empty())
        .map_or("all", String::as_str);
    let max_items: Option<i32> = if max_items_arg == "all" {
        None
    } else {
        Some(max_items_arg.parse().expect("maxItems"))
    };

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let (jar, bytes, mtime) = p7t_common::jar_identity();
    writeln!(
        out,
        "HEADER jar={jar} bytes={bytes} mtime={mtime} fixture={} passNo={pass_no} \
         maxItems={max_items_arg}",
        dsn.file_name().expect("a file name").to_string_lossy(),
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
    }

    // ---- half one: the transcription ---------------------------------------------------------
    let mut board = p7t_common::load_board(&dsn);
    let mut settings = p7t_common::build_settings(&board);
    settings.max_items = max_items;
    let mut router = p7t_common::new_router(&board, &settings);
    let mut state = PassState::default();
    for previous in 1..pass_no {
        p7t_common::run_pass_with(&mut board, &mut router, &mut state, previous);
    }
    writeln!(out, "[transcript]").expect("write");
    p7t_common::dump_autoroute_items(&mut out, &router, &board);
    let transcript_returned = p7t_common::transcribe_run_single_thread(
        &mut out,
        &mut board,
        &mut router,
        &mut state,
        pass_no,
    );
    writeln!(
        out,
        "TRANSCRIPT returned={transcript_returned} {} totalItemsRouted={}",
        p7t_common::board_shape(&mut board),
        router.total_items_routed
    )
    .expect("write");
    let transcript_hash = board.structural_hash();

    // ---- half two: the real method -----------------------------------------------------------
    let mut real_board = p7t_common::load_board(&dsn);
    let mut real_settings = p7t_common::build_settings(&real_board);
    real_settings.max_items = max_items;
    let mut real_router = p7t_common::new_router(&real_board, &real_settings);
    let mut real_state = PassState::default();
    for previous in 1..pass_no {
        p7t_common::run_pass_with(&mut real_board, &mut real_router, &mut real_state, previous);
    }
    writeln!(out, "[real]").expect("write");
    let real_returned =
        p7t_common::run_pass_with(&mut real_board, &mut real_router, &mut real_state, pass_no);
    let real_hash = real_board.structural_hash();
    writeln!(
        out,
        "REAL returned={real_returned} {} totalItemsRouted={} equalsTranscript={}",
        p7t_common::board_shape(&mut real_board),
        real_router.total_items_routed,
        real_hash == transcript_hash
    )
    .expect("write");
    out.flush().expect("flush");
}
