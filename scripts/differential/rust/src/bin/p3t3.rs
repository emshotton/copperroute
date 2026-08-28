//! Rust twin of `scripts/differential/java/P3T3.java` (Plan 3 Task 3).
//!
//! Dumps the token stream `fr_dsn::lexer::DsnScanner` produces for a file, in exactly the format
//! the Java driver dumps the one `SpecctraDsnStreamReader` produces. See
//! `scripts/differential/README.md`.

use std::io::{BufWriter, Write};

use fr_dsn::format::double::java_double_to_string;
use fr_dsn::keyword::Keyword;
use fr_dsn::lexer::{DsnScanner, Token};

fn main() {
    let path = match std::env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("usage: p3t3 <file>");
            std::process::exit(2);
        }
    };

    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    // Java wraps the `InputStream` in an `InputStreamReader` with the default charset, which is
    // UTF-8 since JDK 18 and maps malformed input to U+FFFD, exactly like `from_utf8_lossy`.
    let text = String::from_utf8_lossy(&bytes);

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let mut scanner = match DsnScanner::new(&text) {
        Ok(scanner) => scanner,
        Err(e) => {
            writeln!(out, "0 ERROR {e}").expect("write");
            return;
        }
    };

    let mut index = 0u64;
    loop {
        match scanner.next_token() {
            Err(_) => {
                // Java's `zzScanError` throws `java.lang.Error`; `Integer.valueOf`/
                // `Double.valueOf` throw `java.lang.NumberFormatException`. The Rust twin cannot
                // name the Java class, so a mismatch here shows up as a diff, which is what we
                // want: no fixture is expected to reach it.
                writeln!(out, "{index} ERROR java.lang.Error").expect("write");
                return;
            }
            Ok(None) => {
                writeln!(out, "{index} EOF - {}", scanner.yystate() as u8).expect("write");
                return;
            }
            Ok(Some(token)) => {
                let described = describe(&token);
                writeln!(out, "{index} {described} {}", scanner.yystate() as u8).expect("write");
                index += 1;
            }
        }
    }
}

fn describe(token: &Token) -> String {
    match token {
        Token::Open => "OPEN -".to_string(),
        Token::Close => "CLOSE -".to_string(),
        Token::Kw(keyword) => format!("KW {}", java_constant_name(*keyword)),
        Token::Str(s) => format!("STR {}", escape(s)),
        Token::Int(i) => format!("INT {i}"),
        Token::Float(d) => format!("DBL {}", java_double_to_string(*d)),
    }
}

/// The name of the Java `Keyword` field holding this keyword's singleton, which is what the Java
/// driver prints: the enum's `Debug` spelling is the CamelCase of exactly that name.
fn java_constant_name(keyword: Keyword) -> String {
    let debug = format!("{keyword:?}");
    let mut out = String::with_capacity(debug.len() + 8);
    for (i, ch) in debug.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

/// Mirrors `P3T3.escape`: one token is always exactly one output line.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    // Java escapes per UTF-16 code unit, but a surrogate pair is reassembled by the
    // `StringBuilder` and printed as one character, so iterating Rust `char`s is equivalent.
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}
