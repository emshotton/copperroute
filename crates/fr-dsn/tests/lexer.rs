//! `DsnScanner` — the Specctra DSN lexer (`io/specctra/parser/SpecctraDsnStreamReader.java`).
//!
//! Every expectation here was taken from the Java source (the DFA tables and the action switch)
//! or from a JVM run recorded in the Task 3 report; the `p3t3` differential driver checks the
//! same scanner against the real Java one over the whole fixture corpus.

use fr_dsn::keyword::Keyword;
use fr_dsn::lexer::{DsnScanner, LexicalState, Token};

/// Collects the whole token stream of `input`.
fn tokens(input: &str) -> Vec<Token> {
    let mut scanner = DsnScanner::new(input);
    let mut out = Vec::new();
    while let Some(token) = scanner.next_token().expect("no scan error") {
        out.push(token);
    }
    out
}

#[test]
fn leading_zero_is_a_float_not_an_integer() {
    // `DecIntegerLiteral = [+-]?(0|[1-9][0-9]*)` does not match `007` — but `DecFloatLiteral =
    // [+-]?[0-9]+(\.[0-9]+)?([Ee][+-]?int)?` does, so the token is a `Double`, not a string.
    // JVM-verified against the 2.3.0 jar: `007` scans as `DBL 7.0`. (The Task 3 brief predicted
    // `Str("007")`; Java wins.)
    assert_eq!(tokens("007"), vec![Token::Float(7.0)]);
    // In `NAME` it *is* a string, which is what the brief's example was about.
    assert_eq!(
        tokens("(net 007)"),
        vec![
            Token::Open,
            Token::Kw(Keyword::Net),
            Token::Str("007".to_string()),
            Token::Close,
        ]
    );
}

#[test]
fn layer_name_state_also_stringifies_numbers() {
    // `path` -> `LAYER_NAME` (action 31, `SpecctraDsnStreamReader.java:961`), where the same
    // rule applies. JVM-verified: `(path 007)` scans as `OPEN KW POLYGON_PATH STR 007 CLOSE`.
    let mut scanner = DsnScanner::new("(path 007)");
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Open));
    assert_eq!(
        scanner.next_token().unwrap(),
        Some(Token::Kw(Keyword::PolygonPath))
    );
    assert_eq!(scanner.yystate(), LexicalState::LayerName);
    assert_eq!(
        scanner.next_token().unwrap(),
        Some(Token::Str("007".to_string()))
    );
}

#[test]
fn plus_sign_integer_is_an_integer() {
    assert_eq!(tokens("+5"), vec![Token::Int(5)]);
}

#[test]
fn exponent_float_through_the_dfa() {
    assert_eq!(tokens("1e5"), vec![Token::Float(100_000.0)]);
}

#[test]
fn next_double_has_no_exponents() {
    // The hand-rolled `nextDouble` uses `NumberFormat.getInstance(Locale.US)`, whose lenient
    // parse stops at the lowercase `e` — a different number grammar from the DFA's.
    let mut scanner = DsnScanner::new("1e5");
    assert_eq!(scanner.next_double(), Some(1.0));
}

#[test]
fn net_scope_tokens() {
    assert_eq!(
        tokens("(net \"a b\")"),
        vec![
            Token::Open,
            Token::Kw(Keyword::Net),
            Token::Str("a b".to_string()),
            Token::Close,
        ]
    );
}

#[test]
fn net_switches_to_name_state_where_digits_lex_as_a_string() {
    let mut scanner = DsnScanner::new("(net 123abc)");
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Open));
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Kw(Keyword::Net)));
    assert_eq!(scanner.yystate(), LexicalState::Name);
    assert_eq!(
        scanner.next_token().unwrap(),
        Some(Token::Str("123abc".to_string()))
    );
    assert_eq!(scanner.yystate(), LexicalState::YyInitial);
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Close));
}

#[test]
fn line_comment_is_skipped() {
    assert_eq!(
        tokens("# comment\n(pcb"),
        vec![Token::Open, Token::Kw(Keyword::PcbScope)]
    );
}

#[test]
fn block_comment_is_skipped() {
    assert_eq!(
        tokens("/* c */(pcb"),
        vec![Token::Open, Token::Kw(Keyword::PcbScope)]
    );
}

#[test]
fn backslash_in_a_quoted_string_is_literal() {
    // Action 12 (`SpecctraDsnStreamReader.java:1477`) appends a backslash verbatim; there is no
    // escape processing at all.
    assert_eq!(tokens("\"a\\b\""), vec![Token::Str("a\\b".to_string())]);
}

#[test]
fn single_quoted_string() {
    assert_eq!(tokens("'x'"), vec![Token::Str("x".to_string())]);
}

#[test]
fn next_string_stops_at_a_bracket_and_leaves_it() {
    let mut scanner = DsnScanner::new("  foo)");
    assert_eq!(scanner.next_string(), "foo");
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Close));
}

#[test]
fn next_string_reads_a_quoted_string() {
    let mut scanner = DsnScanner::new("\"a b\" rest)");
    assert_eq!(scanner.next_string(), "a b");
    assert_eq!(scanner.next_string(), "rest");
}

#[test]
fn next_string_list_drops_a_leading_empty_string() {
    // The KiCad 8 workaround at `SpecctraDsnStreamReader.java:1801-1804`.
    let mut scanner = DsnScanner::new("\"\" A B )");
    assert_eq!(scanner.next_string_list(), vec!["A", "B"]);
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Close));
}

#[test]
fn next_string_skips_backspace_but_not_tab() {
    // `stringSkipTrailing = {8, 32}` and `stringStopAt = {8, 10, 13, 32, 40, 41}`: backspace is
    // skippable and a tab is not, despite the Java comment saying "spaces, tabs".
    let mut scanner = DsnScanner::new("\u{8}fo\to)");
    assert_eq!(scanner.next_string(), "fo\to");
}

#[test]
fn next_double_is_javas_lenient_number_format() {
    // JVM-verified against `NumberFormat.getInstance(Locale.US)` (see the Task 3 report): the
    // exponent separator is an uppercase `E` only, `+` is not a sign, `,` groups, and a leading
    // numeric prefix is enough.
    for (text, expected) in [
        ("1e5)", Some(1.0)),
        ("1E5)", Some(100_000.0)),
        ("1.5)", Some(1.5)),
        ("+5)", None),
        ("1,234)", Some(1234.0)),
        ("abc)", None),
        ("007)", Some(7.0)),
    ] {
        let mut scanner = DsnScanner::new(text);
        assert_eq!(scanner.next_double(), expected, "next_double({text})");
    }
}

/// fixed: T4 (#86) — replaces `input_larger_than_the_java_buffer_is_rejected`, whose assertion
/// was `DsnScanner::new(&"x".repeat(16 * 1024 * 1024 + 1)).is_err()`.
///
/// Java's `zzBuffer` is a fixed `char[16 * 1024 * 1024]` (`SpecctraDsnStreamReader.java:40`) that
/// the hand-rolled `nextString` indexes with no refill, so a design over 16 MiB cannot be scanned
/// at all. The port's lexer was always correct — it converts the whole input once, exactly sized
/// — and reproduced the ceiling **on purpose**, as `DsnError::InputTooLarge`. That deliberate
/// limit is deleted, and `DsnScanner::new` is infallible.
///
/// The claim is not merely "20 MiB is accepted" but "20 MiB lexes like 15 MiB": both inputs are
/// the same DSN body repeated, so the token stream of the larger must be the smaller's stream
/// with more repetitions of the same unit and nothing mis-lexed at the 16 MiB mark — which is
/// exactly where Java's buffer ends and its `nextString` would have started reading rubbish.
///
/// Slow lane: the two inputs are 35 MiB of text together, so this runs in release
/// (`cargo test -p fr-dsn --release --test lexer`) or with `-- --ignored`.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn a_twenty_mebibyte_input_lexes_like_a_fifteen_mebibyte_one() {
    /// One repetition unit of a DSN body, plus the newline that separates two of them.
    const UNIT: &str = "(wire (path F.Cu 250 1 2 3 4))";
    let unit_len = UNIT.len() + 1;

    /// Scans a whole input, returning the token count and the last `tail` tokens.
    fn scan(text: &str, tail_len: usize) -> (usize, Vec<Token>) {
        let mut scanner = DsnScanner::new(text);
        let mut n = 0;
        let mut tail: Vec<Token> = Vec::new();
        while let Some(token) = scanner.next_token().expect("no scan error") {
            n += 1;
            tail.push(token);
            if tail.len() > tail_len {
                tail.remove(0);
            }
        }
        (n, tail)
    }

    // The tokens one unit lexes to, derived rather than asserted as a literal: what is binding
    // here is that the 20 MiB stream is the 15 MiB stream with proportionally more of the *same*
    // tokens, not any particular grammar decision.
    let (per_unit, _) = scan(UNIT, 0);
    assert!(per_unit >= 10, "the unit lexes to a real token run");

    let build = |mib: usize| -> (String, usize) {
        let units = (mib * 1024 * 1024).div_ceil(unit_len);
        let mut text = String::with_capacity(units * unit_len);
        for _ in 0..units {
            text.push_str(UNIT);
            text.push('\n');
        }
        (text, units)
    };
    let (small, small_units) = build(15);
    let (large, large_units) = build(20);
    assert!(small.len() >= 15 * 1024 * 1024, "{}", small.len());
    assert!(
        large.len() > 16 * 1024 * 1024,
        "the larger input must be past Java's 16 MiB ceiling: {}",
        large.len()
    );

    let (small_tokens, small_tail) = scan(&small, per_unit);
    let (large_tokens, large_tail) = scan(&large, per_unit);
    assert_eq!(small_tokens, small_units * per_unit);
    assert_eq!(
        large_tokens,
        large_units * per_unit,
        "every token past the 16 MiB mark is lexed, none dropped or mangled"
    );
    assert_eq!(
        small_tail, large_tail,
        "the same repetition unit lexes to the same tokens at 20 MiB as at 15 MiB"
    );
}
