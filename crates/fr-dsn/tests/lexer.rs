//! `DsnScanner` — the Specctra DSN lexer (`io/specctra/parser/SpecctraDsnStreamReader.java`).
//!
//! Every expectation here was taken from the Java source (the DFA tables and the action switch)
//! or from a JVM run recorded in the Task 3 report; the `p3t3` differential driver checks the
//! same scanner against the real Java one over the whole fixture corpus.

use fr_dsn::keyword::Keyword;
use fr_dsn::lexer::{DsnScanner, LexicalState, Token};

/// Collects the whole token stream of `input`.
fn tokens(input: &str) -> Vec<Token> {
    let mut scanner = DsnScanner::new(input).expect("input fits the buffer");
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
    let mut scanner = DsnScanner::new("(path 007)").expect("fits");
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
    let mut scanner = DsnScanner::new("1e5").expect("fits");
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
    let mut scanner = DsnScanner::new("(net 123abc)").expect("fits");
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
    let mut scanner = DsnScanner::new("  foo)").expect("fits");
    assert_eq!(scanner.next_string(), "foo");
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Close));
}

#[test]
fn next_string_reads_a_quoted_string() {
    let mut scanner = DsnScanner::new("\"a b\" rest)").expect("fits");
    assert_eq!(scanner.next_string(), "a b");
    assert_eq!(scanner.next_string(), "rest");
}

#[test]
fn next_string_list_drops_a_leading_empty_string() {
    // The KiCad 8 workaround at `SpecctraDsnStreamReader.java:1801-1804`.
    let mut scanner = DsnScanner::new("\"\" A B )").expect("fits");
    assert_eq!(scanner.next_string_list(), vec!["A", "B"]);
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Close));
}

#[test]
fn next_string_skips_backspace_but_not_tab() {
    // `stringSkipTrailing = {8, 32}` and `stringStopAt = {8, 10, 13, 32, 40, 41}`: backspace is
    // skippable and a tab is not, despite the Java comment saying "spaces, tabs".
    let mut scanner = DsnScanner::new("\u{8}fo\to)").expect("fits");
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
        let mut scanner = DsnScanner::new(text).expect("fits");
        assert_eq!(scanner.next_double(), expected, "next_double({text})");
    }
}

#[test]
fn input_larger_than_the_java_buffer_is_rejected() {
    // Java's `zzBuffer` is a fixed `char[16 * 1024 * 1024]` that `nextString` indexes with no
    // refill; the port refuses such an input instead of mis-lexing it.
    let too_big = "x".repeat(16 * 1024 * 1024 + 1);
    assert!(DsnScanner::new(&too_big).is_err());
    let just_fits = "x".repeat(16 * 1024 * 1024);
    assert!(DsnScanner::new(&just_fits).is_ok());
}
