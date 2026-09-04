use std::io::Read;

use fr_board::ItemIdGenerator;

use crate::error::{BoardMetadata, BoardReadResult, DsnError};
use crate::keyword::{Keyword, ScopeKeyword};
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::scope_parameter::{DsnReadOptions, ReadScopeParameter, read_scope, skip_scope};
use crate::parser::{dsn_file, header, structure};

const PCB: &str = "(pcb";

// `FRLogger.warn("DSN file '…' was loaded with N warning(s).")` calls it exists for (:139-145,
pub fn read_board(
    input: impl Read,
    id_generator: Option<ItemIdGenerator>,
    design_name: Option<&str>,
    options: &DsnReadOptions,
) -> BoardReadResult {
    let _ = design_name;

    let text = match read_to_string(input) {
        Ok(text) => text,
        Err(error) => return BoardReadResult::IoError(error),
    };
    let scanner = DsnScanner::new(&text);
    let mut p = ReadScopeParameter::new(scanner, options);
    p.id_generator = id_generator.unwrap_or_default();

    match read_pcb_header(&mut p) {
        Ok(HeaderResult::Ok) => {}
        Ok(HeaderResult::NotDsn) => return not_a_dsn_file(),
        Err(error) => return parse_error(&error),
    }

    let read_ok = match read_scope(ScopeKeyword::Pcb, &mut p) {
        Ok(read_ok) => read_ok,
        Err(error) => return parse_error(&error),
    };

    let coordinate_transform = p.coordinate_transform;
    let mut board = p.board.take();
    let warnings = std::mem::take(&mut p.warnings);

    if read_ok {
        if p.autoroute_settings.is_none()
            && let Some(board) = board.as_mut()
            && let Err(error) = dsn_file::adjust_plane_autoroute_settings(board)
        {
            return parse_error(&error);
        }
        if let Some(diagnostic) = p.truncation.take() {
            return BoardReadResult::Partial {
                board: board.map(Box::new),
                metadata: None,
                warnings,
                coordinate_transform,
                diagnostic,
            };
        }
        BoardReadResult::Success {
            board: board.map(Box::new),
            metadata: None,
            warnings,
            coordinate_transform,
        }
    } else if !p.board_outline_ok {
        BoardReadResult::OutlineMissing {
            board: board.map(Box::new),
            metadata: None,
            warnings,
            coordinate_transform,
        }
    } else {
        BoardReadResult::ParseError {
            location: PCB.to_string(),
            detail: "DSN structure parsing failed".to_string(),
        }
    }
}

pub fn read_metadata(input: impl Read) -> BoardReadResult {
    let options = DsnReadOptions::default();
    let text = match read_to_string(input) {
        Ok(text) => text,
        Err(error) => return BoardReadResult::IoError(error),
    };
    let scanner = DsnScanner::new(&text);
    let mut p = ReadScopeParameter::new(scanner, &options);

    match read_pcb_header(&mut p) {
        Ok(HeaderResult::Ok) => {}
        Ok(HeaderResult::NotDsn) => return not_a_dsn_file(),
        Err(error) => return parse_error(&error),
    }

    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        next_token = match p.scanner.next_token() {
            Ok(token) => token,
            Err(error) => return parse_error(&error),
        };
        if next_token.is_none() || next_token == Some(Token::Close) {
            break;
        }
        if prev_token != Some(Token::Open) {
            continue;
        }
        let result = match next_token {
            Some(Token::Kw(Keyword::ParserScope)) => header::read_parser_scope(&mut p),
            Some(Token::Kw(Keyword::ResolutionScope)) => header::read_resolution_scope(&mut p),
            Some(Token::Kw(Keyword::StructureScope)) => {
                let result = structure::read_structure_scope(&mut p);
                if let Err(error) = result {
                    return parse_error(&error);
                }
                break;
            }
            _ => skip_scope(&mut p.scanner).map(|_| true),
        };
        if let Err(error) = result {
            return parse_error(&error);
        }
    }

    let layer_count = match (&p.layer_structure, &p.board) {
        (Some(layer_structure), _) => layer_structure.layers.len(),
        (None, Some(board)) => board.get_layer_count(),
        (None, None) => 0,
    };
    let metadata = BoardMetadata {
        host_cad: p.host_cad.clone(),
        host_version: p.host_version.clone(),
        layer_count,
        unit: p.unit,
        resolution: p.resolution,
        snap_angle: p.snap_angle,
        router_settings: p.autoroute_settings.clone(),
    };
    BoardReadResult::Success {
        board: p.board.take().map(Box::new),
        metadata: Some(metadata),
        warnings: std::mem::take(&mut p.warnings),
        coordinate_transform: p.coordinate_transform,
    }
}

enum HeaderResult {
        Ok,
        NotDsn,
}

fn read_pcb_header(p: &mut ReadScopeParameter<'_>) -> Result<HeaderResult, DsnError> {
    for i in 0..3 {
        let token = p.scanner.next_token()?;
        let ok = match i {
            0 => token == Some(Token::Open),
            1 => {
                let ok = token == Some(Token::Kw(Keyword::PcbScope));
                p.scanner.yybegin(LexicalState::Name);
                ok
            }
            _ => true,
        };
        if !ok {
            return Ok(HeaderResult::NotDsn);
        }
    }
    Ok(HeaderResult::Ok)
}

fn not_a_dsn_file() -> BoardReadResult {
    BoardReadResult::ParseError {
        location: PCB.to_string(),
        detail: "Not a Specctra DSN file: expected '(pcb <name>' header".to_string(),
    }
}

fn parse_error(error: &DsnError) -> BoardReadResult {
    BoardReadResult::ParseError {
        location: PCB.to_string(),
        detail: error.to_string(),
    }
}

fn read_to_string(mut input: impl Read) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
