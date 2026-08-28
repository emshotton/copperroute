//! `io/specctra/parser/DsnFile.java` — scalar scope helpers shared by many scope readers, plus
//! `adjustPlaneAutorouteSettings`.

use fr_board::{Board, FixedState, Item, ItemId};
use fr_geometry::TileShape;

use crate::error::DsnError;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, Token};
use crate::parser::scope_parameter::skip_scope;

/// `DsnFile.CLASS_CLEARANCE_SEPARATOR` (DsnFile.java:20): joins a via padstack name to a net
/// class name when writing a per-class via (`<padstack>-<netclass>`).
pub const CLASS_CLEARANCE_SEPARATOR: char = '-';

/// `DsnFile.readOnOffScope` (DsnFile.java:120-133): reads one `on`/`off` token, then consumes the
/// rest of the scope with [`skip_scope`] regardless (Java calls `ScopeKeyword.skipScope`
/// unconditionally, ignoring its own return value — this port propagates `skip_scope`'s error
/// instead of silently discarding it, a deliberate, documented divergence: see
/// [`skip_scope`]'s docs).
///
/// Java's `FRLogger.warn` for "neither on nor off" is dropped (no `tracing` in `fr-dsn`); the
/// result is simply `false`, exactly as Java's totalized return already is.
pub fn read_on_off_scope(scanner: &mut DsnScanner) -> Result<bool, DsnError> {
    let next_token = scanner.next_token()?;
    let result = matches!(next_token, Some(Token::Kw(Keyword::On)));
    skip_scope(scanner)?;
    Ok(result)
}

/// `DsnFile.readIntegerScope` (DsnFile.java:135-157): reads one integer token followed by the
/// closing bracket.
///
/// Rejects a `Float` token — the int/float asymmetry the plan Task 4 brief calls out: `(x 5.0)`
/// is an error here, unlike [`read_float_scope`], which widens an `Int` token to `f64`.
pub fn read_integer_scope(scanner: &mut DsnScanner) -> Result<i32, DsnError> {
    let value = match scanner.next_token()? {
        Some(Token::Int(i)) => i as i32,
        other => {
            return Err(DsnError::UnexpectedScalar {
                context: "DsnFile::read_integer_scope",
                expected: "integer",
                found: format!("{other:?}"),
            });
        }
    };
    match scanner.next_token()? {
        Some(Token::Close) => {}
        other => {
            return Err(DsnError::UnexpectedScalar {
                context: "DsnFile::read_integer_scope",
                expected: ")",
                found: format!("{other:?}"),
            });
        }
    }
    Ok(value)
}

/// `DsnFile.readFloatScope` (DsnFile.java:159-181): reads one numeric token followed by the
/// closing bracket, **widening an `Int` token to `f64`** — the direction [`read_integer_scope`]
/// does not accept.
pub fn read_float_scope(scanner: &mut DsnScanner) -> Result<f64, DsnError> {
    let value = match scanner.next_token()? {
        Some(Token::Float(f)) => f,
        Some(Token::Int(i)) => i as f64,
        other => {
            return Err(DsnError::UnexpectedScalar {
                context: "DsnFile::read_float_scope",
                expected: "number",
                found: format!("{other:?}"),
            });
        }
    };
    match scanner.next_token()? {
        Some(Token::Close) => {}
        other => {
            return Err(DsnError::UnexpectedScalar {
                context: "DsnFile::read_float_scope",
                expected: ")",
                found: format!("{other:?}"),
            });
        }
    }
    Ok(value)
}

/// `DsnFile.readStringScope` (DsnFile.java:183-203): reads one bypass-lexed string, then
/// consumes tokens up to (not including any further skip past) the closing bracket.
///
/// Java's `nextString(true)` can return `null` on a Java-side buffer overrun; the port's
/// [`DsnScanner::next_string_ignoring_newline`] always returns a `String` (see its docs), so
/// that branch is unreachable here and is not ported.
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

/// `DsnFile.readStringListScope` (DsnFile.java:205-210): `None` (Java: `null`) when the scope
/// does not end with the closing bracket `nextStringList` expects.
pub fn read_string_list_scope(scanner: &mut DsnScanner) -> Result<Option<Vec<String>>, DsnError> {
    let result = scanner.next_string_list();
    if !scanner.next_closing_bracket()? {
        return Ok(None);
    }
    Ok(Some(result))
}

/// `DsnFile.adjustPlaneAutorouteSettings` (DsnFile.java:32-113): called from
/// `DsnReader.readBoard` when the DSN file contains no `(autoroute ...)` scope, to retroactively
/// mark large conduction areas as planes.
///
/// Java takes a nullable `BasicBoard`; `&mut Board` is never null, so the `routingBoard == null`
/// branch (DsnFile.java:33-35) is unreachable and not ported. Java's `FRLogger.info` per
/// newly-recognised plane layer (DsnFile.java:100-107) is dropped — no `tracing` in `fr-dsn`.
#[must_use]
pub fn adjust_plane_autoroute_settings(board: &mut Board) -> bool {
    let layer_count = board.layer_structure().layers.len();
    if layer_count <= 2 {
        return false;
    }
    if board.layer_structure().layers.iter().any(|l| !l.is_signal) {
        return false;
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

    /// One conduction area's outcome, computed under an immutable borrow of `board` (via
    /// `ItemCtx`) and applied afterwards under a mutable one — Java's single pass is split in
    /// two here because `board.ctx()` borrows the whole board.
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
                continue;
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

    // Java: `nothingChanged` only flips to `false` inside the net-number loop, so a conduction
    // area with zero nets (never actually reachable — every real plane has exactly one) would
    // still leave it `true` even though it passed every other gate.
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

    !nothing_changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::DsnScanner;

    fn scan(input: &str) -> DsnScanner {
        DsnScanner::new(input).expect("fits the buffer")
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
        assert_eq!(read_integer_scope(&mut scanner).expect("integer"), 5);
    }

    #[test]
    fn read_integer_scope_rejects_a_float() {
        let mut scanner = scan("5.0)");
        assert!(read_integer_scope(&mut scanner).is_err());
    }

    #[test]
    fn read_float_scope_widens_an_integer() {
        let mut scanner = scan("5)");
        assert_eq!(read_float_scope(&mut scanner).expect("number"), 5.0);
    }

    #[test]
    fn read_float_scope_reads_a_float() {
        let mut scanner = scan("5.5)");
        assert_eq!(read_float_scope(&mut scanner).expect("number"), 5.5);
    }
}
