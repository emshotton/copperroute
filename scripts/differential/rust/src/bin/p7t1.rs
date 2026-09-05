//! Rust half of the `p7t1` differential pair — `scripts/differential/java/P7T1.java`.
//!
//! Plan 7 Task 9, item-selection level: `BatchAutorouter::autoroute_items`
//! (`BatchAutorouter.java:345-409`) over a real DSN board, printed **before any routing of the
//! pass being reported**, with the `handledItems` set traced step by step.
//!
//! Usage: `p7t1 <dsn> [passNo]`. See `P7T1.java`'s class comment for the output format, for why
//! `passNo` is an argument at all, and for the budget note. Everything the two sides share —
//! loading, settings, the router, the pass warm-up and the dump itself — lives in
//! [`p7t_common`], the twin of `P7T2.java`'s shared helpers, so the two drivers cannot describe
//! different boards.

use std::io::{BufWriter, Write};

#[path = "../p7t_common.rs"]
mod p7t_common;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p7t1 <dsn> [passNo]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let pass_no: i32 = args.get(1).map_or(1, |a| a.parse().expect("passNo"));

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    // `P7T1.main`'s header line, field for field.
    let (jar, bytes, mtime) = p7t_common::jar_identity();
    writeln!(
        out,
        "HEADER jar={jar} bytes={bytes} mtime={mtime} fixture={} passNo={pass_no}",
        dsn.file_name().expect("a file name").to_string_lossy(),
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
    }

    let mut board = p7t_common::load_board(&dsn);
    let settings = p7t_common::build_settings(&board);
    let mut router = p7t_common::new_router(&board, &settings);

    // The board this pass sees is the board the previous passes left; see `P7T1.java`.
    for previous in 1..pass_no {
        p7t_common::run_pass(&mut board, &mut router, previous);
    }

    p7t_common::dump_autoroute_items(&mut out, &router, &board);
    out.flush().expect("flush");
}
