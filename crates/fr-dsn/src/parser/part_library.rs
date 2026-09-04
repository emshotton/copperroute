
use std::cmp::Ordering;

use crate::error::DsnError;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnLogicalPartMapping {
        pub name: String,
                pub components: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnPartPin {
        pub pin_name: String,
        pub gate_name: String,
        pub gate_swap_code: i32,
        pub gate_pin_name: String,
        pub gate_pin_swap_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnLogicalPart {
        pub name: String,
        pub part_pins: Vec<DsnPartPin>,
}

pub fn read_part_library_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            return Ok(false);
        };
        if token == Token::Close {
            break;
        }
        if prev_token == Some(Token::Open) {
            match token {
                Token::Kw(Keyword::LogicalPartMapping) => {
                    let Some(next_mapping) = read_logical_part_mapping(&mut p.scanner)? else {
                        return Ok(false);
                    };
                    p.logical_part_mappings.push(next_mapping);
                }
                Token::Kw(Keyword::LogicalPart) => {
                    let Some(next_part) = read_logical_part(&mut p.scanner)? else {
                        return Ok(false);
                    };
                    p.logical_parts.push(next_part);
                }
                _ => {
                    skip_scope(&mut p.scanner)?;
                }
            }
        }
    }
    Ok(true)
}

fn read_logical_part_mapping(
    scanner: &mut DsnScanner,
) -> Result<Option<DsnLogicalPartMapping>, DsnError> {
    let Some(Token::Str(name)) = scanner.next_token()? else {
        return Ok(None);
    };
    if scanner.next_token()? != Some(Token::Open) {
        return Ok(None);
    }
    if scanner.next_token()? != Some(Token::Kw(Keyword::ComponentScope)) {
        return Ok(None);
    }
    let mut result: Vec<String> = Vec::new();
    loop {
        scanner.yybegin(LexicalState::Name);
        let next_token = scanner.next_token()?;
        if next_token == Some(Token::Close) {
            break;
        }
        let Some(Token::Str(component)) = next_token else {
            return Ok(None);
        };
        sorted_set_add(&mut result, component);
    }
    if scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    Ok(Some(DsnLogicalPartMapping {
        name,
        components: result,
    }))
}

fn read_logical_part(scanner: &mut DsnScanner) -> Result<Option<DsnLogicalPart>, DsnError> {
    let mut part_pins: Vec<DsnPartPin> = Vec::new();
    let mut next_token = scanner.next_token()?;
    let Some(Token::Str(part_name)) = next_token.clone() else {
        return Ok(None);
    };
    scanner.set_scope_identifier(&part_name);
    loop {
        let prev_token = next_token;
        next_token = scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            return Ok(None);
        };
        if token == Token::Close {
            break;
        }
        if prev_token == Some(Token::Open) {
            if token == Token::Kw(Keyword::Pin) {
                let Some(current_part_pin) = read_part_pin(scanner)? else {
                    return Ok(None);
                };
                part_pins.push(current_part_pin);
            } else {
                skip_scope(scanner)?;
            }
        }
    }
    Ok(Some(DsnLogicalPart {
        name: part_name,
        part_pins,
    }))
}

fn read_part_pin(scanner: &mut DsnScanner) -> Result<Option<DsnPartPin>, DsnError> {
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(pin_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.set_scope_identifier(&pin_name);
    let Some(Token::Int(_)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(gate_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.set_scope_identifier(&gate_name);
    let Some(Token::Int(gate_swap_code)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(gate_pin_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.set_scope_identifier(&gate_pin_name);
    let Some(Token::Int(gate_pin_swap_code)) = scanner.next_token()? else {
        return Ok(None);
    };
    loop {
        match scanner.next_token()? {
            Some(Token::Close) | None => break,
            _ => {}
        }
    }
    Ok(Some(DsnPartPin {
        pin_name,
        gate_name,
        gate_swap_code: i32::try_from(gate_swap_code).unwrap_or(i32::MAX),
        gate_pin_name,
        gate_pin_swap_code: i32::try_from(gate_pin_swap_code).unwrap_or(i32::MAX),
    }))
}

pub fn write_part_library_scope(p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    if board.library.logical_parts.count() == 0 {
        return;
    }
    p.file.start_scope_nl();
    p.file.write("part_library");


    for i in 1..=board.library.logical_parts.count() {
        let current_part_name = &board.library.logical_parts.get(i).name;
        p.file.start_scope_nl();
        p.file.write("logical_part_mapping ");
        p.identifier_type.write(current_part_name, &mut p.file);
        p.file.new_line();
        p.file.write("(comp");
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        for j in 1..=board.components.count() as i32 {
            let current_component = board.components.get(j);
            if current_component
                .get_logical_part()
                .is_some_and(|part| part == i)
            {
                let name = current_component.name.clone();
                p.file.write(" ");
                p.file.write(&name);
            }
        }
        p.file.write(")");
        p.file.end_scope();
    }


    for i in 1..=board.library.logical_parts.count() {
        let current_part = board.library.logical_parts.get(i);

        p.file.start_scope_nl();
        p.file.write("logical_part ");
        p.identifier_type.write(&current_part.name, &mut p.file);
        p.file.new_line();
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        for j in 0..current_part.pin_count() as i32 {
            p.file.new_line();
            let Some(current_pin) = current_part.get_pin(j) else {
                continue;
            };
            p.file.write("(pin ");
            p.identifier_type.write(&current_pin.pin_name, &mut p.file);
            p.file.write(" 0 ");
            p.identifier_type.write(&current_pin.gate_name, &mut p.file);
            p.file.write(" ");
            p.file.write(&current_pin.gate_swap_code.to_string());
            p.file.write(" ");
            p.identifier_type
                .write(&current_pin.gate_pin_name, &mut p.file);
            p.file.write(" ");
            p.file.write(&current_pin.gate_pin_swap_code.to_string());
            p.file.write(")");
        }
        p.file.end_scope();
    }
    p.file.end_scope();
}

fn sorted_set_add(set: &mut Vec<String>, value: String) {
    match set.binary_search_by(|probe| java_string_cmp(probe, &value)) {
        Ok(_) => {}
        Err(index) => set.insert(index, value),
    }
}

pub(crate) fn java_string_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_set_add_sorts_and_deduplicates_like_a_treeset() {
        let mut set = Vec::new();
        for name in ["U10", "U2", "U1", "U2"] {
            sorted_set_add(&mut set, name.to_string());
        }
        assert_eq!(set, ["U1", "U10", "U2"]);
    }

    #[test]
    fn java_string_cmp_orders_by_utf16_code_units() {
        assert_eq!(java_string_cmp("\u{10000}", "\u{FFFD}"), Ordering::Less);
        assert!("\u{10000}" > "\u{FFFD}");
    }
}
