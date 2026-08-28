//! `Keyword`/`ScopeKeyword` names, `skip_scope`/`read_scope` dispatch, `DsnFile` scalar helpers.
//!
//! Java authority: `io/specctra/parser/{Keyword,ScopeKeyword,DsnFile}.java`. The fifteen
//! `Keyword::name()` values below were reflected off `tools/freerouting-2.3.0.jar`
//! (`Keyword.get_name()`) while writing this test — plan ruling 1: the 2.3.0 snake_case
//! Specctra tokens, not the clone HEAD's camelCased regression (`Keyword.AUTOROUTE_SETTINGS =
//! new Keyword("autorouteSettings")` at HEAD).

use fr_dsn::keyword::{Keyword, ScopeKeyword};
use fr_dsn::lexer::{DsnScanner, LexicalState, Token};
use fr_dsn::parser::dsn_file::{read_float_scope, read_integer_scope, read_on_off_scope};
use fr_dsn::parser::scope_parameter::skip_scope;

#[test]
fn keyword_name_matches_2_3_0_for_every_ruling_1_keyword() {
    let cases: &[(Keyword, &str)] = &[
        (Keyword::AutorouteSettings, "autoroute_settings"),
        (Keyword::ClearanceClass, "clearance_class"),
        (Keyword::HostCad, "host_cad"),
        (Keyword::HostVersion, "host_version"),
        (Keyword::LogicalPart, "logical_part"),
        (Keyword::PullTight, "pull_tight"),
        (Keyword::ShoveFixed, "shove_fixed"),
        (Keyword::SnapAngle, "snap_angle"),
        (Keyword::StartRipupCosts, "start_ripup_costs"),
        (Keyword::StringQuote, "string_quote"),
        (Keyword::UseLayer, "use_layer"),
        (Keyword::UseVia, "use_via"),
        (Keyword::ViaCosts, "via_costs"),
        (Keyword::ViaRule, "via_rule"),
        (Keyword::WriteResolution, "write_resolution"),
    ];
    for (kw, expected) in cases {
        assert_eq!(kw.name(), *expected, "{kw:?}");
    }
}

#[test]
fn keyword_jumper_name_is_jumper() {
    // Never produced by the lexer's DFA (see `keyword.rs`'s module docs), but `Structure.java:349`
    // still calls `Keyword.JUMPER.getName()`.
    assert_eq!(Keyword::Jumper.name(), "jumper");
}

#[test]
fn scope_keyword_name_matches_2_3_0_for_all_twelve() {
    let cases: &[(ScopeKeyword, &str)] = &[
        (ScopeKeyword::Component, "component"),
        (ScopeKeyword::Library, "library"),
        (ScopeKeyword::Network, "network"),
        (ScopeKeyword::Parser, "parser"),
        (ScopeKeyword::PartLibrary, "part_library"),
        (ScopeKeyword::Pcb, "pcb"),
        (ScopeKeyword::PlaceControl, "place_control"),
        (ScopeKeyword::Placement, "placement"),
        (ScopeKeyword::Plane, "plane"),
        (ScopeKeyword::Resolution, "resolution"),
        (ScopeKeyword::Structure, "structure"),
        (ScopeKeyword::Wiring, "wiring"),
    ];
    for (kw, expected) in cases {
        assert_eq!(kw.name(), *expected, "{kw:?}");
    }
}

#[test]
fn scope_keyword_from_keyword_round_trips() {
    assert_eq!(
        ScopeKeyword::from_keyword(Keyword::StructureScope),
        Some(ScopeKeyword::Structure)
    );
    assert_eq!(ScopeKeyword::from_keyword(Keyword::Via), None);
}

/// `ScopeKeyword.skipScope` (ScopeKeyword.java:20ff): given a scanner positioned right after `(`
/// and the scope keyword (as every real caller is), consumes exactly to the matching `)` and
/// leaves the following token untouched.
#[test]
fn skip_scope_consumes_exactly_the_matching_bracket() {
    let mut scanner = DsnScanner::new("(foo (bar 1 2) baz) tail").expect("fits the buffer");
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Open));
    assert_eq!(
        scanner.next_token().unwrap(),
        Some(Token::Str("foo".to_string()))
    );
    skip_scope(&mut scanner).expect("balanced input");
    assert_eq!(
        scanner.next_token().unwrap(),
        Some(Token::Str("tail".to_string()))
    );
}

/// `skip_scope` must not choke on a body containing `123abc`-shaped tokens (a leading digit run
/// glued to letters) at any position, including immediately after a nested `(...)` — a
/// regression guard for the `scanner.yybegin(NAME)` call ScopeKeyword.java:23 makes before
/// **every** token, not just once before the loop.
///
/// This can only be a smoke test, not a discriminating one: `NAME` is a genuinely one-shot state
/// (confirmed empirically — matching *any* single token while in `NAME`, including `(`/`)`
/// themselves, reverts the scanner to `YyInitial` before the next `next_token()` call, by
/// construction of the underlying JFlex grammar), and `123abc` vs. `123`+`abc` never changes
/// which characters are brackets — so no input can make `skip_scope`'s `Ok`/`Err` outcome or
/// final buffer position depend on whether `NAME` was re-entered before each token. The
/// behaviour this guards is instead pinned directly against the lexer below
/// (`name_state_lexes_a_leading_digit_run_as_one_string`) and by reading `skip_scope`'s source.
#[test]
fn skip_scope_handles_glued_digit_letter_tokens_throughout() {
    // `(` and `foo` simulate the opening bracket and scope keyword a real caller would already
    // have consumed before ever calling `skip_scope` (see the `ScopeKeyword.readScope` docs);
    // everything from `123abc` onward is the scope's body, which is what `skip_scope` itself
    // reads, each token preceded by its own `yybegin(NAME)`.
    let mut scanner =
        DsnScanner::new("(foo 123abc (456def 1) 789ghi) tail").expect("fits the buffer");
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Open));
    assert_eq!(
        scanner.next_token().unwrap(),
        Some(Token::Str("foo".to_string()))
    );
    skip_scope(&mut scanner).expect("balanced input, however NAME-mode tokens split");
    assert_eq!(
        scanner.next_token().unwrap(),
        Some(Token::Str("tail".to_string()))
    );
}

/// The actual claim behind `NAME`'s existence, pinned directly against the lexer:
/// `next_token`, started from `YyInitial`, splits `123abc` into an `Int` and a trailing `Str`
/// (the numeric-literal rule intercepts the leading digits); started from `Name` (what
/// `skip_scope` sets before every read), the whole run is one `Str`.
#[test]
fn name_state_lexes_a_leading_digit_run_as_one_string() {
    let mut initial = DsnScanner::new("123abc").expect("fits the buffer");
    assert_eq!(
        initial.next_token().unwrap(),
        Some(Token::Int(123)),
        "YyInitial splits the digits off as their own token"
    );

    let mut name = DsnScanner::new("123abc").expect("fits the buffer");
    name.yybegin(LexicalState::Name);
    assert_eq!(
        name.next_token().unwrap(),
        Some(Token::Str("123abc".to_string())),
        "NAME lexes the whole run as one string"
    );
}

#[test]
fn read_on_off_scope_reads_on_and_off() {
    let mut on = DsnScanner::new("on)").expect("fits");
    assert!(read_on_off_scope(&mut on).expect("no scan error"));

    let mut off = DsnScanner::new("off)").expect("fits");
    assert!(!read_on_off_scope(&mut off).expect("no scan error"));
}

#[test]
fn read_integer_scope_accepts_an_integer_and_rejects_a_float() {
    let mut int_scanner = DsnScanner::new("5)").expect("fits");
    assert_eq!(read_integer_scope(&mut int_scanner).expect("integer"), 5);

    let mut float_scanner = DsnScanner::new("5.0)").expect("fits");
    assert!(read_integer_scope(&mut float_scanner).is_err());
}

#[test]
fn read_float_scope_widens_an_integer_token() {
    let mut scanner = DsnScanner::new("5)").expect("fits");
    assert_eq!(read_float_scope(&mut scanner).expect("number"), 5.0_f64);
}
