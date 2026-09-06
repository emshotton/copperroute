use std::io::Read;

use fr_board::Board;

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::DsnRouterSettings;
use crate::parser::autoroute_settings::read_autoroute_settings_scope;
use crate::parser::geometry::{DsnLayer, DsnLayerStructure};
use crate::parser::library::read_padstack_scope;
use crate::parser::network::{
    DsnRule, add_via_rule, insert_net_class, read_net_class_scope, read_rule_scope, read_via_info,
    read_via_rule,
};
use crate::parser::scope_parameter::skip_scope;
use crate::parser::structure::{RuleLayerScope, read_snap_angle, set_clearance_rule};

pub fn read(
    input: impl Read,
    design_name: &str,
    board: &mut Board,
    ct: &CoordinateTransform,
    mut target_settings: Option<&mut DsnRouterSettings>,
) -> Result<bool, DsnError> {
    let _ = design_name;
    let text = read_to_string(input)?;
    let mut scanner = DsnScanner::new(&text);

    if !read_rules_header(&mut scanner)? {
        return Ok(false);
    }

    let layer_structure = DsnLayerStructure::from_board(board.layer_structure());
    let string_quote = board.communication.string_quote.clone();

    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            return Ok(false);
        };
        if next_token == Token::Close {
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Rule) => {
                    let rules = read_rule_scope(&mut scanner)?;
                    apply_rules(
                        rules.as_deref(),
                        board,
                        ct,
                        &string_quote,
                        RuleLayerScope::AllLayers,
                    );
                }
                Token::Kw(Keyword::Layer) => {
                    apply_layer_rules(&mut scanner, board, ct, &string_quote)?;
                }
                Token::Kw(Keyword::Padstack) => {
                    read_padstack_scope(
                        &mut scanner,
                        &layer_structure,
                        ct,
                        &mut board.library.padstacks,
                    )?;
                }
                Token::Kw(Keyword::Via) => apply_via_info(&mut scanner, board)?,
                Token::Kw(Keyword::ViaRule) => apply_via_rule(&mut scanner, board)?,
                Token::Kw(Keyword::Class) => {
                    apply_net_class(&mut scanner, &layer_structure, board, ct)?;
                }
                Token::Kw(Keyword::SnapAngle) => {
                    if let Some(snap_angle) = read_snap_angle(&mut scanner)? {
                        board.rules.trace_angle_restriction = snap_angle;
                    }
                }
                Token::Kw(Keyword::AutorouteSettings) => {
                    let parsed = read_autoroute_settings_scope(&mut scanner, &layer_structure)?;
                    if let (Some(target), Some(parsed)) = (target_settings.as_mut(), parsed) {
                        target.apply_new_values_from(&parsed);
                    }
                }
                _ => {
                    skip_scope(&mut scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }
    Ok(true)
}

pub fn read_router_settings(input: impl Read) -> Result<Option<DsnRouterSettings>, DsnError> {
    let text = read_to_string(input)?;
    if text.is_empty() {
        return Ok(None);
    }

    let layer_structure = discover_layer_structure(&text)?;
    let mut scanner = DsnScanner::new(&text);
    if !read_rules_header(&mut scanner)? {
        return Ok(None);
    }

    let mut prev_was_open = false;
    loop {
        let next_token = scanner.next_token()?;
        if next_token.is_none() || next_token == Some(Token::Close) {
            break;
        }
        let is_open = next_token == Some(Token::Open);
        if prev_was_open {
            if next_token == Some(Token::Kw(Keyword::AutorouteSettings)) {
                return read_autoroute_settings_scope(&mut scanner, &layer_structure);
            }
            skip_scope(&mut scanner)?;
        }
        prev_was_open = is_open;
    }
    Ok(None)
}

pub fn discover_layer_structure(text: &str) -> Result<DsnLayerStructure, DsnError> {
    let mut layer_names: Vec<String> = Vec::new();
    let mut scanner = DsnScanner::new(text);
    let mut prev_was_open = false;
    loop {
        let Some(token) = scanner.next_token()? else {
            break;
        };
        let is_open = token == Token::Open;
        if prev_was_open
            && (token == Token::Kw(Keyword::LayerRule) || token == Token::Kw(Keyword::Layer))
        {
            scanner.yybegin(LexicalState::Name);
            if let Some(Token::Str(name)) = scanner.next_token()?
                && !name.trim().is_empty()
                && !layer_names.contains(&name)
            {
                layer_names.push(name);
            }
        }
        prev_was_open = is_open;
    }

    if layer_names.is_empty() {
        layer_names.push("F.Cu".to_string());
        layer_names.push("B.Cu".to_string());
    }
    Ok(DsnLayerStructure::new(
        layer_names
            .into_iter()
            .enumerate()
            .map(|(i, name)| DsnLayer::new(name, i32::try_from(i).unwrap_or(i32::MAX), true))
            .collect(),
    ))
}

fn read_rules_header(scanner: &mut DsnScanner) -> Result<bool, DsnError> {
    if scanner.next_token()? != Some(Token::Open) {
        return Ok(false);
    }
    if scanner.next_token()? != Some(Token::Kw(Keyword::Rules)) {
        return Ok(false);
    }
    if scanner.next_token()? != Some(Token::Kw(Keyword::PcbScope)) {
        return Ok(false);
    }
    scanner.yybegin(LexicalState::Name);
    scanner.next_token()?;
    Ok(true)
}

fn apply_rules(
    rules: Option<&[DsnRule]>,
    board: &mut Board,
    ct: &CoordinateTransform,
    string_quote: &str,
    scope: RuleLayerScope,
) {
    let Some(rules) = rules else {
        return;
    };
    for rule in rules {
        match rule {
            DsnRule::Width(value) => {
                let trace_half_width = (ct.dsn_to_board(*value) / 2.0).round() as i32;
                match scope {
                    RuleLayerScope::AllLayers => {
                        board.rules.set_default_trace_half_widths(trace_half_width);
                    }
                    RuleLayerScope::One(layer) => board
                        .rules
                        .set_default_trace_half_width(layer, trace_half_width),
                }
            }
            DsnRule::Clearance(clearance_rule) => {
                set_clearance_rule(clearance_rule, scope, ct, &mut board.rules, string_quote);
            }
        }
    }
}

fn apply_layer_rules(
    scanner: &mut DsnScanner,
    board: &mut Board,
    ct: &CoordinateTransform,
    string_quote: &str,
) -> Result<(), DsnError> {
    let Some(Token::Str(layer_string)) = scanner.next_token()? else {
        return Ok(());
    };
    let layer_scope = board
        .layer_structure()
        .get_no(&layer_string)
        .map(RuleLayerScope::One);
    let mut next_token = scanner.next_token()?;
    while next_token != Some(Token::Close) {
        if next_token != Some(Token::Open) {
            return Ok(());
        }
        next_token = scanner.next_token()?;
        if next_token == Some(Token::Kw(Keyword::Rule)) {
            let rules = read_rule_scope(scanner)?;
            if let Some(scope) = layer_scope {
                apply_rules(rules.as_deref(), board, ct, string_quote, scope);
            }
        } else {
            skip_scope(scanner)?;
        }
        next_token = scanner.next_token()?;
    }
    Ok(())
}

fn apply_via_info(scanner: &mut DsnScanner, board: &mut Board) -> Result<(), DsnError> {
    let Some(via_info) = read_via_info(scanner, board)? else {
        return Ok(());
    };
    match board.rules.via_infos.get_no(via_info.get_name()) {
        Some(old_id) => {
            board.rules.replace_via_info(old_id, via_info);
        }
        None => {
            board.rules.via_infos.add(via_info);
        }
    }
    Ok(())
}

fn apply_via_rule(scanner: &mut DsnScanner, board: &mut Board) -> Result<(), DsnError> {
    if let Some(via_rule) = read_via_rule(scanner)?
        && !via_rule.is_empty()
    {
        add_via_rule(&via_rule, board);
    }
    Ok(())
}

fn apply_net_class(
    scanner: &mut DsnScanner,
    layer_structure: &DsnLayerStructure,
    board: &mut Board,
    ct: &CoordinateTransform,
) -> Result<(), DsnError> {
    let Some(net_class) = read_net_class_scope(scanner)? else {
        return Ok(());
    };
    insert_net_class(&net_class, layer_structure, board, ct, false);
    Ok(())
}

fn read_to_string(mut input: impl Read) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
