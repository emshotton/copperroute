use fr_dsn::keyword::Keyword;
use fr_dsn::lexer::{DsnScanner, LexicalState, Token};

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
    assert_eq!(tokens("007"), vec![Token::Float(7.0)]);
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
    let mut scanner = DsnScanner::new("\"\" A B )");
    assert_eq!(scanner.next_string_list(), vec!["A", "B"]);
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Close));
}

#[test]
fn next_string_skips_backspace_but_not_tab() {
    let mut scanner = DsnScanner::new("\u{8}fo\to)");
    assert_eq!(scanner.next_string(), "fo\to");
}

#[test]
fn next_double_is_javas_lenient_number_format() {
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

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn a_twenty_mebibyte_input_lexes_like_a_fifteen_mebibyte_one() {
        const UNIT: &str = "(wire (path F.Cu 250 1 2 3 4))";
    let unit_len = UNIT.len() + 1;

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
