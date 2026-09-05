//! Rust twin of `scripts/differential/java/P3T3.java` (Plan 3 Task 3).
//!
//! Dumps the token stream `fr_dsn::lexer::DsnScanner` produces for a file, in exactly the format
//! the Java driver dumps the one `SpecctraDsnStreamReader` produces. See
//! `scripts/differential/README.md`.
//!
//! The dump itself lives in `src/token_dump.rs`, shared with `p3t15` mode 4 (Plan 3 Task 15),
//! whose Java side delegates to `P3T3.main`.

use std::io::BufWriter;

#[path = "../token_dump.rs"]
mod token_dump;

fn main() {
    let path = match std::env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("usage: p3t3 <file>");
            std::process::exit(2);
        }
    };

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    token_dump::dump(&mut out, &path);
}
