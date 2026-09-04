//! Java authority: `io/specctra/parser/{Keyword,ScopeKeyword,DsnFile}.java`. The fifteen
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

#[test]
fn skip_scope_handles_glued_digit_letter_tokens_throughout() {
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

#[test]
fn a_malformed_integer_scope_does_not_desync_its_caller() {
    let layer_structure = DsnLayerStructure::new(vec![
        DsnLayer::new("F.Cu".to_string(), 0, true),
        DsnLayer::new("B.Cu".to_string(), 1, true),
    ]);

    for malformed in [
        "(via_costs 5.0)",        
        "(via_costs (5))",        
        "(via_costs)",            
        "(via_costs 5 junk)",     
        "(via_costs 5 (junk 1))", 
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

#[test]
fn a_truncation_inside_an_unknown_scope_reports_partial() {
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

    let scanner = DsnScanner::new("(foo 1 2))");
    let mut complete = ReadScopeParameter::new(scanner, &options);
    assert!(matches!(
        read_scope(ScopeKeyword::Pcb, &mut complete),
        Ok(true)
    ));
    assert_eq!(complete.truncation, None);
}

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

    let closed = format!("{text})\n)\n");
    assert!(matches!(
        fr_dsn::read_board(closed.as_bytes(), None, Some("truncated"), &options),
        fr_dsn::BoardReadResult::Success { .. }
    ));
}

#[test]
fn skip_scope_returns_ok_false_at_end_of_file() {
    let mut scanner = DsnScanner::new("no closing bracket here");
    assert!(matches!(skip_scope(&mut scanner), Ok(false)));
}
