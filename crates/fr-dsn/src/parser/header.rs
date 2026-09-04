//! `Parser` reads the `parser` scope (`host_cad`/`host_version`/`string_quote`/
use std::io::Write;

use fr_board::{Communication, Unit, WriteResolution};

use crate::error::DsnError;
use crate::format::{IdentifierType, IndentFileWriter};
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::read_string_scope;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

fn read_write_solution(
    scope_parameter: &mut ReadScopeParameter<'_>,
) -> Result<Option<WriteResolution>, DsnError> {
    let Some(Token::Str(resolution_string)) = scope_parameter.scanner.next_token()? else {
        return Ok(None);
    };
    let Some(Token::Int(resolution_value)) = scope_parameter.scanner.next_token()? else {
        return Ok(None);
    };
    if scope_parameter.scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    Ok(Some(WriteResolution::new(
        resolution_string,
        resolution_value as i32,
    )))
}

fn read_constant(
    scope_parameter: &mut ReadScopeParameter<'_>,
) -> Result<Option<Vec<String>>, DsnError> {
    let mut result = Vec::with_capacity(2);
    for _ in 0..2 {
        scope_parameter.scanner.yybegin(LexicalState::Name);
        let Some(Token::Str(value)) = scope_parameter.scanner.next_token()? else {
            return Ok(None);
        };
        result.push(value);
    }
    if scope_parameter.scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    Ok(Some(result))
}

fn read_quote_char(scanner: &mut DsnScanner) -> Result<Option<String>, DsnError> {
    let Some(Token::Str(result)) = scanner.next_token()? else {
        return Ok(None);
    };
    if scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    Ok(Some(result))
}

pub fn write_parser_scope<W: Write>(
    file: &mut IndentFileWriter<W>,
    parser_info: &Communication,
    identifier_type: &IdentifierType,
    reduced: bool,
) {
    file.start_scope_nl();
    file.write("parser");
    if !reduced {
        file.new_line();
        file.write("(string_quote ");
        file.write(&parser_info.string_quote);
        file.write(")");
        file.new_line();
        file.write("(space_in_quoted_tokens on)");
    }
    if let Some(host_cad) = &parser_info.host_cad {
        file.new_line();
        file.write("(host_cad ");
        identifier_type.write(host_cad, file);
        file.write(")");
    }
    if let Some(host_version) = &parser_info.host_version {
        file.new_line();
        file.write("(host_version ");
        identifier_type.write(host_version, file);
        file.write(")");
    }
    for current_constant in &parser_info.constants {
        file.new_line();
        file.write("(constant ");
        for part in current_constant {
            identifier_type.write(part, file);
            file.write(" ");
        }
        file.write(")");
    }
    if let Some(write_resolution) = &parser_info.write_resolution {
        file.new_line();
        file.write("(write_resolution ");
        if let Some(first) = write_resolution.char_name.chars().next() {
            file.write(&first.to_string());
        }
        file.write(" ");
        file.write(&write_resolution.positive_int.to_string());
        file.write(")");
    }
    if !reduced {
        file.new_line();
        file.write("(generated_by_freerouting)");
    }
    file.end_scope();
}

pub fn read_parser_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
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
                Token::Kw(Keyword::StringQuote) => {
                    let Some(quote_char) = read_quote_char(&mut p.scanner)? else {
                        return Ok(false);
                    };
                    p.string_quote = quote_char;
                }
                Token::Kw(Keyword::HostCad) => {
                    p.host_cad = Some(read_string_scope(&mut p.scanner)?);
                }
                Token::Kw(Keyword::HostVersion) => {
                    p.host_version = Some(read_string_scope(&mut p.scanner)?);
                }
                Token::Kw(Keyword::Constant) => {
                    if let Some(current_constant) = read_constant(p)? {
                        p.constants.push(current_constant);
                    }
                }
                Token::Kw(Keyword::WriteResolution) => {
                    p.write_resolution = read_write_solution(p)?;
                }
                Token::Kw(Keyword::GeneratedByFreerouting) => {
                    p.dsn_file_generated_by_host = false;
                    let _ = skip_scope(&mut p.scanner)?;
                }
                _ => {
                    let _ = skip_scope(&mut p.scanner)?;
                }
            }
        }
    }
    Ok(true)
}

pub fn write_resolution_scope<W: Write>(
    file: &mut IndentFileWriter<W>,
    board_communication: &Communication,
) {
    file.new_line();
    file.write("(resolution ");
    file.write(&board_communication.unit.to_string());
    file.write(" ");
    file.write(&board_communication.resolution.to_string());
    file.write(")");
}

pub fn read_resolution_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let Some(Token::Str(unit_name)) = p.scanner.next_token()? else {
        return Ok(false);
    };
    let Some(unit) = Unit::from_string(&unit_name) else {
        return Ok(false);
    };
    p.unit = unit;
    let Some(Token::Int(resolution)) = p.scanner.next_token()? else {
        return Ok(false);
    };
    p.resolution = resolution as i32;
    if p.scanner.next_token()? != Some(Token::Close) {
        return Ok(false);
    }
    Ok(true)
}

pub fn write_unit_scope<W: Write>(file: &mut IndentFileWriter<W>, unit: Unit) {
    file.new_line();
    file.write("(unit ");
    file.write(&unit.to_string());
    file.write(")");
}

pub fn read_unit_scope(_p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    Ok(false)
}

pub fn read_flip_style_rotate_first(scanner: &mut DsnScanner) -> Result<bool, DsnError> {
    let result = scanner.next_token()? == Some(Token::Kw(Keyword::RotateFirst));
    if scanner.next_token()? != Some(Token::Close) {
        return Ok(false);
    }
    Ok(result)
}

pub fn read_place_control_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut flip_style_rotate_first = false;
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = p.scanner.next_token()? else {
            return Ok(false);
        };
        if next_token == Token::Close {
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open && next_token == Token::Kw(Keyword::FlipStyle) {
            flip_style_rotate_first = read_flip_style_rotate_first(&mut p.scanner)?;
        }
        prev_was_open = is_open;
    }
    if flip_style_rotate_first && let Some(board) = p.board.as_mut() {
        board.components.set_flip_style_rotate_first(true);
    }
    Ok(true)
}
