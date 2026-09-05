use fr_board::{Board, FixedState, Item, ItemId};
use fr_geometry::TileShape;

use crate::error::DsnError;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, Token};
use crate::parser::scope_parameter::skip_scope;

pub const CLASS_CLEARANCE_SEPARATOR: char = '-';

pub fn read_on_off_scope(scanner: &mut DsnScanner) -> Result<bool, DsnError> {
    let next_token = scanner.next_token()?;
    let result = matches!(next_token, Some(Token::Kw(Keyword::On)));
    skip_scope(scanner)?;
    Ok(result)
}

pub fn read_integer_scope(scanner: &mut DsnScanner) -> Result<Option<i32>, DsnError> {
    let value = match scanner.next_token()? {
        Some(Token::Int(i)) => i as i32,
        // Java: `FRLogger.warn(...); return 0;` (DsnFile.java:141-146) — no second token read.
        Some(offending) => {
            skip_rest_of_scope(scanner, &offending)?;
            return Ok(None);
        }
        None => return Ok(None),
    };
    match scanner.next_token()? {
        Some(Token::Close) => Ok(Some(value)),
        // Java: `FRLogger.warn(...); return 0;` (DsnFile.java:150-154) — the wrong token here
        Some(offending) => {
            skip_rest_of_scope(scanner, &offending)?;
            Ok(None)
        }
        None => Ok(None),
    }
}

fn skip_rest_of_scope(scanner: &mut DsnScanner, offending: &Token) -> Result<(), DsnError> {
    match offending {
        Token::Close => {}
        Token::Open => {
            skip_scope(scanner)?;
            skip_scope(scanner)?;
        }
        _ => {
            skip_scope(scanner)?;
        }
    }
    Ok(())
}

pub fn read_float_scope(scanner: &mut DsnScanner) -> Result<Option<f64>, DsnError> {
    let value = match scanner.next_token()? {
        Some(Token::Float(f)) => f,
        Some(Token::Int(i)) => i as f64,
        Some(offending) => {
            skip_rest_of_scope(scanner, &offending)?;
            return Ok(None);
        }
        None => return Ok(None),
    };
    match scanner.next_token()? {
        Some(Token::Close) => Ok(Some(value)),
        Some(offending) => {
            skip_rest_of_scope(scanner, &offending)?;
            Ok(None)
        }
        None => Ok(None),
    }
}

pub fn read_string_scope(scanner: &mut DsnScanner) -> Result<String, DsnError> {
    let result = scanner.next_string_ignoring_newline(true);
    let mut next_token = scanner.next_token()?;
    if next_token != Some(Token::Close) {
        // Java: `FRLogger.warn(...)`, dropped.
        while next_token.is_some() && next_token != Some(Token::Close) {
            next_token = scanner.next_token()?;
        }
    }
    Ok(result)
}

pub fn read_string_list_scope(scanner: &mut DsnScanner) -> Result<Option<Vec<String>>, DsnError> {
    let result = scanner.next_string_list();
    if !scanner.next_closing_bracket()? {
        return Ok(None);
    }
    Ok(Some(result))
}

pub fn adjust_plane_autoroute_settings(board: &mut Board) -> Result<bool, DsnError> {
    let layer_count = board.layer_structure().layers.len();
    if layer_count <= 2 {
        return Ok(false);
    }
    if board.layer_structure().layers.iter().any(|l| !l.is_signal) {
        return Ok(false);
    }

    let mut layer_contains_wires = vec![false; layer_count];
    let mut conduction_area_ids: Vec<ItemId> = Vec::new();
    for (id, item) in &board.items {
        match item {
            Item::Trace(trace) => layer_contains_wires[trace.get_layer()] = true,
            Item::ConductionArea(_) => conduction_area_ids.push(*id),
            _ => {}
        }
    }

    let mut board_area = 0.0_f64;
    if let Some(outline_id) = board.get_outline()
        && let Some(Item::BoardOutline(outline)) = board.get_item(outline_id)
    {
        for i in 0..outline.shape_count() {
            if let Some(pieces) = outline.get_shape(i).and_then(|s| s.split_to_convex()) {
                board_area += pieces.iter().map(TileShape::area).sum::<f64>();
            }
        }
    }

    struct PlaneChange {
        id: ItemId,
        net_numbers: Vec<i32>,
        bump_fixed_state: bool,
    }

    let mut changes: Vec<PlaneChange> = Vec::new();
    {
        let ctx = board.ctx();
        for id in &conduction_area_ids {
            let Some(Item::ConductionArea(area)) = board.get_item(*id) else {
                continue;
            };
            let layer_index = area.area.get_layer();
            if layer_contains_wires[layer_index] {
                continue;
            }
            if !board.layer_structure().layers[layer_index].is_signal
                || layer_index == 0
                || layer_index == layer_count - 1
            {
                continue;
            }
            let Some(pieces) = area.area.split_to_convex(&ctx) else {
                return Err(DsnError::UnsplittableConductionArea { item: *id });
            };
            let current_area: f64 = pieces.iter().map(TileShape::area).sum();
            if current_area < 0.5 * board_area {
                continue;
            }
            let net_numbers: Vec<i32> = (0..area.hdr.net_count())
                .map(|i| area.hdr.get_net_number(i))
                .collect();
            let bump_fixed_state = area.hdr.get_fixed_state() < FixedState::UserFixed;
            changes.push(PlaneChange {
                id: *id,
                net_numbers,
                bump_fixed_state,
            });
        }
    }

    let nothing_changed = changes.iter().all(|c| c.net_numbers.is_empty());

    for change in &changes {
        for net_number in &change.net_numbers {
            if let Some(net) = board.rules.nets.get_mut(*net_number) {
                net.set_contains_plane(true);
            }
        }
        if change.bump_fixed_state
            && let Some(item) = board.get_item_mut(change.id)
        {
            item.header_mut().set_fixed_state(FixedState::UserFixed);
        }
    }

    Ok(!nothing_changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::DsnScanner;

    fn scan(input: &str) -> DsnScanner {
        DsnScanner::new(input)
    }

    #[test]
    fn class_clearance_separator_is_a_hyphen() {
        assert_eq!(CLASS_CLEARANCE_SEPARATOR, '-');
    }

    #[test]
    fn read_on_off_scope_reads_on() {
        let mut scanner = scan("on)");
        assert!(read_on_off_scope(&mut scanner).expect("no scan error"));
    }

    #[test]
    fn read_on_off_scope_reads_off() {
        let mut scanner = scan("off)");
        assert!(!read_on_off_scope(&mut scanner).expect("no scan error"));
    }

    #[test]
    fn read_integer_scope_reads_an_integer() {
        let mut scanner = scan("5)");
        assert_eq!(read_integer_scope(&mut scanner).expect("integer"), Some(5));
    }

    #[test]
    fn read_integer_scope_reports_no_value_for_a_float() {
        let mut scanner = scan("5.0)");
        assert_eq!(
            read_integer_scope(&mut scanner).expect("no scan error"),
            None
        );
    }

    #[test]
    fn read_integer_scope_consumes_the_closing_bracket_after_a_bad_first_token() {
        let mut scanner = scan("5.0) tail");
        assert_eq!(
            read_integer_scope(&mut scanner).expect("no scan error"),
            None
        );
        assert_eq!(
            scanner.next_token().unwrap(),
            Some(Token::Str("tail".to_string()))
        );
    }

    #[test]
    fn read_integer_scope_consumes_a_nested_scope_in_a_malformed_body() {
        let mut scanner = scan("5.0 (junk 1 2)) tail");
        assert_eq!(
            read_integer_scope(&mut scanner).expect("no scan error"),
            None
        );
        assert_eq!(
            scanner.next_token().unwrap(),
            Some(Token::Str("tail".to_string()))
        );
    }

    #[test]
    fn a_malformed_scalar_scope_is_resynchronised_whatever_the_offending_token() {
        for body in [
            "5.0) tail",
            "(5)) tail",
            ") tail",
            "on) tail",
            "5 junk) tail",
            "5 (junk 1)) tail",
            "5 (a (b c)) ) tail",
        ] {
            let mut scanner = scan(body);
            let integer = read_integer_scope(&mut scanner).expect("no scan error");
            assert_eq!(
                scanner.next_token().unwrap(),
                Some(Token::Str("tail".to_string())),
                "read_integer_scope({body:?}) left the scanner in the wrong place (value {integer:?})"
            );

            let mut scanner = scan(body);
            let float = read_float_scope(&mut scanner).expect("no scan error");
            assert_eq!(
                scanner.next_token().unwrap(),
                Some(Token::Str("tail".to_string())),
                "read_float_scope({body:?}) left the scanner in the wrong place (value {float:?})"
            );
        }
    }

    #[test]
    fn read_float_scope_widens_an_integer() {
        let mut scanner = scan("5)");
        assert_eq!(read_float_scope(&mut scanner).expect("number"), Some(5.0));
    }

    #[test]
    fn read_float_scope_reads_a_float() {
        let mut scanner = scan("5.5)");
        assert_eq!(read_float_scope(&mut scanner).expect("number"), Some(5.5));
    }

    #[test]
    fn read_float_scope_reports_no_value_for_a_non_numeric_token() {
        let mut scanner = scan("on) tail");
        assert_eq!(read_float_scope(&mut scanner).expect("no scan error"), None);
        assert_eq!(
            scanner.next_token().unwrap(),
            Some(Token::Str("tail".to_string()))
        );
    }
}
