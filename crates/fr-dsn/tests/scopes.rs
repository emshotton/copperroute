//! `Keyword`/`ScopeKeyword` names, `skip_scope`/`read_scope` dispatch, `DsnFile` scalar helpers.
//!
//! Java authority: `io/specctra/parser/{Keyword,ScopeKeyword,DsnFile}.java`. The fifteen
//! `Keyword::name()` values below were reflected off `tools/freerouting-2.3.0.jar`
//! (`Keyword.get_name()`) while writing this test — plan ruling 1: the 2.3.0 snake_case
//! Specctra tokens, not the clone HEAD's camelCased regression (`Keyword.AUTOROUTE_SETTINGS =
//! new Keyword("autorouteSettings")` at HEAD).

use fr_dsn::keyword::{Keyword, ScopeKeyword};
use fr_dsn::lexer::{DsnScanner, LexicalState, Token};
use fr_dsn::parser::autoroute_settings::read_autoroute_settings_scope;
use fr_dsn::parser::dsn_file::{read_float_scope, read_integer_scope, read_on_off_scope};
use fr_dsn::parser::geometry::{DsnLayer, DsnLayerStructure};
use fr_dsn::parser::scope_parameter::{DsnReadOptions, ReadScopeParameter, read_scope, skip_scope};

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
    let mut scanner = DsnScanner::new("(foo (bar 1 2) baz) tail");
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Open));
    assert_eq!(
        scanner.next_token().unwrap(),
        Some(Token::Str("foo".to_string()))
    );
    assert!(skip_scope(&mut scanner).expect("no scan error"));
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
    let mut scanner = DsnScanner::new("(foo 123abc (456def 1) 789ghi) tail");
    assert_eq!(scanner.next_token().unwrap(), Some(Token::Open));
    assert_eq!(
        scanner.next_token().unwrap(),
        Some(Token::Str("foo".to_string()))
    );
    assert!(
        skip_scope(&mut scanner).expect("no scan error"),
        "balanced input, however NAME-mode tokens split"
    );
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
    let mut initial = DsnScanner::new("123abc");
    assert_eq!(
        initial.next_token().unwrap(),
        Some(Token::Int(123)),
        "YyInitial splits the digits off as their own token"
    );

    let mut name = DsnScanner::new("123abc");
    name.yybegin(LexicalState::Name);
    assert_eq!(
        name.next_token().unwrap(),
        Some(Token::Str("123abc".to_string())),
        "NAME lexes the whole run as one string"
    );
}

#[test]
fn read_on_off_scope_reads_on_and_off() {
    let mut on = DsnScanner::new("on)");
    assert!(read_on_off_scope(&mut on).expect("no scan error"));

    let mut off = DsnScanner::new("off)");
    assert!(!read_on_off_scope(&mut off).expect("no scan error"));
}

#[test]
fn read_integer_scope_accepts_an_integer_and_reports_no_value_for_a_float() {
    // fixed: T4 (#90) — `DsnFile.readIntegerScope` (DsnFile.java:134-160) warns and returns `0`
    // for a non-integer token, and `AutorouteSettings.java:53,55,57` feeds that `0` straight into
    // `RouterSettings`, where it is written back out and carried on `BoardMetadata`. The port now
    // answers "no value read" so the caller leaves its field alone.
    let mut int_scanner = DsnScanner::new("5)");
    assert_eq!(
        read_integer_scope(&mut int_scanner).expect("integer"),
        Some(5)
    );

    let mut float_scanner = DsnScanner::new("5.0)");
    assert_eq!(
        read_integer_scope(&mut float_scanner).expect("no scan error"),
        None
    );
}

#[test]
fn read_float_scope_widens_an_integer_token() {
    let mut scanner = DsnScanner::new("5)");
    assert_eq!(
        read_float_scope(&mut scanner).expect("number"),
        Some(5.0_f64)
    );
}

/// #90's binding test: a malformed integer scope must not end its caller's scope.
///
/// The jar's answer for this exact input, from `DsnFile.readIntegerScope`'s first failure branch
/// (DsnFile.java:141-146) plus `AutorouteSettings.readScope`'s flat loop: `via_costs` totalizes
/// to `0`, the `)` that closes `(via_costs 5.0)` is read as the end of the **whole**
/// `autoroute_settings` scope, and everything after it — here `(vias off)` and
/// `(start_ripup_costs 13)` — is never seen by this reader at all. So the jar reports
/// `viasAllowed = true` (the default) and `startRipupCosts = 1` (the default), with
/// `viaCosts = 0`.
///
/// fixed: T4 (#90): the scope is consumed to its own bracket, `via_costs` keeps its unset state
/// (`via_costs_raw() == None`, so a higher-priority settings source still wins the merge), and
/// the two fields after it are read.
///
/// # The three shapes a malformed scalar scope comes in
///
/// The offending token is already consumed by the time the scope has to be resynchronised, and
/// how many brackets are still open depends on **what** it was — `)` closed this scope, `(`
/// opened a nested one, anything else left just this one. All three are exercised here; the
/// `(via_costs (5))` row is the fix round's own case (a first cut resynchronised all three as if
/// they were the third, which left the outer bracket behind on the second and over-consumed on
/// the first).
#[test]
fn a_malformed_integer_scope_does_not_desync_its_caller() {
    let layer_structure = DsnLayerStructure::new(vec![
        DsnLayer::new("F.Cu".to_string(), 0, true),
        DsnLayer::new("B.Cu".to_string(), 1, true),
    ]);

    // `malformed` is the whole `(via_costs …)` scope, ill-formed in three different ways; the two
    // well-formed fields after it are what a desync would swallow.
    for malformed in [
        "(via_costs 5.0)",        // a value of the wrong kind: one bracket still open
        "(via_costs (5))",        // a nested scope where a scalar belongs: two brackets still open
        "(via_costs)",            // no value at all: the scope's bracket already consumed
        "(via_costs 5 junk)",     // a good value, then a stray token: one bracket still open
        "(via_costs 5 (junk 1))", // a good value, then a nested scope: two brackets still open
    ] {
        let text = format!("{malformed} (vias off) (start_ripup_costs 13)) tail");
        let mut scanner = DsnScanner::new(&text);
        let settings = read_autoroute_settings_scope(&mut scanner, &layer_structure)
            .expect("no scan error")
            .unwrap_or_else(|| panic!("{malformed}: the scope must close on its own bracket"));

        assert_eq!(
            settings.via_costs_raw(),
            None,
            "{malformed}: a malformed value leaves the field unset instead of writing Java's 0"
        );
        assert!(
            !settings.vias_allowed(),
            "{malformed}: (vias off) is past the desync and is read now"
        );
        assert_eq!(
            settings.start_ripup_costs(),
            13,
            "{malformed}: (start_ripup_costs 13) is past the desync and is read now"
        );
        assert_eq!(
            scanner.next_token().unwrap(),
            Some(Token::Str("tail".to_string())),
            "{malformed}: the scope ended on its own closing bracket, not one field early"
        );
    }
}

/// fixed: T4 (#91) — replaces
/// `read_scope_generic_returns_ok_true_when_truncated_inside_an_unknown_scope`.
///
/// `ScopeKeyword.readScope`'s own end-of-file check (ScopeKeyword.java:55-58) fires even when
/// the file was truncated *inside* a nested, unrecognised scope that `skipScope` could not
/// close: `skipScope` answers `false` at end of file (:32-33), the caller discards that, and the
/// very next `nextToken()` call — now genuinely at end of file — is what makes the generic loop
/// return `true`. So `DsnReader.readBoard` reports a truncated file as a **`Success`** carrying a
/// partial board, indistinguishable from a complete read.
///
/// The `Ok(true)` stays: a caller may well want whatever routing data survived, and the roadmap's
/// correction of the register's binary framing says the honest answer is a third state rather
/// than a refusal. What changes is that the reader now *records* the truncation, and `read_board`
/// answers `BoardReadResult::Partial` with the board **and** the diagnostic. `ScopeKeyword::Pcb`
/// dispatches straight to the generic loop (`Keyword.PCB_SCOPE` has no Java subclass), so it
/// stands in for it here.
#[test]
fn a_truncation_inside_an_unknown_scope_reports_partial() {
    // `(foo 1 2` — an unrecognised nested scope keyword ("foo" is not a DSN keyword) whose body
    // is never closed before the input simply ends.
    let scanner = DsnScanner::new("(foo 1 2");
    let options = DsnReadOptions::default();
    let mut p = ReadScopeParameter::new(scanner, &options);
    assert!(
        matches!(read_scope(ScopeKeyword::Pcb, &mut p), Ok(true)),
        "still Java's `true`: the surviving board is handed over, not withheld"
    );
    let truncation = p.truncation.as_deref().expect("the truncation is recorded");
    assert!(
        truncation.contains("(pcb)") && truncation.contains("end of file"),
        "the diagnostic names the scope left open: {truncation}"
    );

    // A complete file records nothing, so the flag is evidence and not noise.
    let scanner = DsnScanner::new("(foo 1 2))");
    let mut complete = ReadScopeParameter::new(scanner, &options);
    assert!(matches!(
        read_scope(ScopeKeyword::Pcb, &mut complete),
        Ok(true)
    ));
    assert_eq!(complete.truncation, None);
}

/// The whole-reader half of #91: `read_board` on a DSN truncated inside an unrecognised scope
/// answers `Partial`, not `Success`.
///
/// The jar's answer for this input is `Success` with the same partial board — that is the bug.
#[test]
fn a_truncated_dsn_reads_as_partial_not_success() {
    let text = "(pcb truncated.dsn\n  (parser\n    (string_quote \")\n  )\n  (resolution um \
                10)\n  (unit um)\n  (structure\n    (layer F.Cu\n      (type signal)\n    )\n    \
                (layer B.Cu\n      (type signal)\n    )\n    (boundary\n      (rect pcb 0 0 \
                100000 50000)\n    )\n  )\n  (some_unknown_scope 1 2";
    let options = DsnReadOptions::default();
    let result = fr_dsn::read_board(text.as_bytes(), None, Some("truncated"), &options);
    let fr_dsn::BoardReadResult::Partial {
        board, diagnostic, ..
    } = result
    else {
        panic!("a truncated DSN must not read as Success");
    };
    assert!(
        board.is_some(),
        "the board the file did contain is still handed over — this is not a refusal"
    );
    assert!(
        diagnostic.contains("end of file"),
        "diagnostic: {diagnostic}"
    );

    // The control: the same file, closed. `Success`, not `Partial`.
    let closed = format!("{text})\n)\n");
    assert!(matches!(
        fr_dsn::read_board(closed.as_bytes(), None, Some("truncated"), &options),
        fr_dsn::BoardReadResult::Success { .. }
    ));
}

/// Direct pin of `skip_scope`'s own end-of-file answer (fix round 1: `Ok(false)`, not `Err`,
/// mirroring `ScopeKeyword.skipScope`'s `false` — ScopeKeyword.java:32-33).
#[test]
fn skip_scope_returns_ok_false_at_end_of_file() {
    let mut scanner = DsnScanner::new("no closing bracket here");
    assert!(matches!(skip_scope(&mut scanner), Ok(false)));
}
