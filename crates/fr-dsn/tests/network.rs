use fr_dsn::keyword::{Keyword, ScopeKeyword};
use fr_dsn::lexer::{DsnScanner, Token};
use fr_dsn::parser::scope_parameter::{DsnReadOptions, ReadScopeParameter, read_scope};

fn read_pcb<T>(text: &str, f: impl FnOnce(&mut ReadScopeParameter<'_>) -> T) -> T {
    let options = DsnReadOptions::default();
    let scanner = DsnScanner::new(text);
    let mut p = ReadScopeParameter::new(scanner, &options);
    assert_eq!(p.scanner.next_token().expect("scan"), Some(Token::Open));
    assert_eq!(
        p.scanner.next_token().expect("scan"),
        Some(Token::Kw(Keyword::PcbScope))
    );
    assert!(read_scope(ScopeKeyword::Pcb, &mut p).expect("no scan error"));
    f(&mut p)
}

fn board_text(control_scope: &str) -> String {
    format!(
        "(pcb via_at_smd.dsn
  (parser
    (string_quote \")
    (space_in_quoted_tokens on)
  )
  (resolution um 10)
  (unit um)
  (structure
    (layer F.Cu (type signal) (property (index 0)))
    (layer B.Cu (type signal) (property (index 1)))
    (boundary
      (path pcb 0  0 0  100000 0  100000 100000  0 100000  0 0)
    )
    (rule (width 250) (clearance 200))
    {control_scope}
  )
  (library
    (padstack VA (shape (circle F.Cu 800)) (shape (circle B.Cu 800)))
  )
  (network
    (net N1)
    (class Alpha N1
      (circuit (use_via VA))
      (rule (width 300) (clearance 300))
    )
  )
)"
    )
}

/// #104: `Network.createViaRule`'s `attachAllowed` parameter now gates the `attach_smd_allowed`
/// of every via the net class's `use_via` list pulls in, the same way `createDefaultViaInfos`
/// already gates its own vias — so `(via_at_smd on)` reaches a class's `use_via` rule and not
/// only the vias `createDefaultViaInfos` builds.
#[test]
fn via_at_smd_reaches_the_net_class_use_via_rule() {
    read_pcb(&board_text(""), |p| {
        let board = p.board.as_ref().expect("board built");
        let via_rule = board.rules.get_via_rule("Alpha").expect("Alpha via rule");
        let rule = &board.rules.via_rules[via_rule.0];
        assert_eq!(rule.via_count(), 1);
        assert!(
            !rule.get_via(0).attach_smd_allowed(),
            "via_at_smd is off by default, so the class's use_via rule must not allow SMD attach"
        );
    });

    read_pcb(&board_text("(control (via_at_smd on))"), |p| {
        let board = p.board.as_ref().expect("board built");
        let via_rule = board.rules.get_via_rule("Alpha").expect("Alpha via rule");
        let rule = &board.rules.via_rules[via_rule.0];
        assert_eq!(rule.via_count(), 1);
        assert!(
            rule.get_via(0).attach_smd_allowed(),
            "(via_at_smd on) must reach the class's use_via rule, not just createDefaultViaInfos"
        );
    });
}
