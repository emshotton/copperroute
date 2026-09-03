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

/// `DsnFile.readIntegerScope` (DsnFile.java:134-160): reads one integer token followed by the
/// closing bracket.
///
/// `None` is "this scope named no usable value" — the caller leaves its field alone.
///
// Java bug: (#90) DsnFile.readIntegerScope warns and returns `0` for a token of the wrong kind,
// and its *first* failure branch (DsnFile.java:141-146) returns without reading a second token,
// so the scope's own closing bracket is left unread. Two observable consequences, both repaired
// here. (a) `AutorouteSettings.java:53,55,57` feed the totalized `0` straight into
// `RouterSettings.setViaCosts`/`setPlaneViaCosts`/`setStartRipupCosts`, which
// `AutorouteSettings.writeScope` writes back out and `BoardMetadata` carries — a malformed field
// silently becomes a *decision*, and one that outranks every lower-priority settings source.
// (b) The unread bracket desyncs `AutorouteSettings.readScope`'s flat, depth-unaware loop, which
// misreads it as ending its own scope one field early. (Java's *second* failure branch,
// :150-154, has already consumed the offending token, so the asymmetry is Java's, not a porting
// artefact.)
//
// fixed: T4 (#90) — both failure branches now consume the rest of the scope through its matching
// closing bracket (`skip_scope`, whose bracket count starts at 1 for exactly this position), and
// the value becomes `Option`: a malformed scope reports "no value", so the field keeps whatever
// a higher-priority settings source put there instead of being overwritten with `0`. The
// caller-side half of this repair is in `read_autoroute_settings_scope`/`read_layer_rule`, which
// land in the same commit — see the note there for why one without the other is not a fix.
///
/// Only a genuine scanner error (`DsnScanner::next_token`'s `Err`) propagates as `Err` here;
/// Java's dropped `FRLogger.warn` calls are simply not ported (no `tracing` in `fr-dsn`).
pub fn read_integer_scope(scanner: &mut DsnScanner) -> Result<Option<i32>, DsnError> {
    let value = match scanner.next_token()? {
        Some(Token::Int(i)) => i as i32,
        // Java: `FRLogger.warn(...); return 0;` (DsnFile.java:141-146) — no second token read.
        Some(_) => {
            skip_scope(scanner)?;
            return Ok(None);
        }
        None => return Ok(None),
    };
    match scanner.next_token()? {
        Some(Token::Close) => Ok(Some(value)),
        // Java: `FRLogger.warn(...); return 0;` (DsnFile.java:150-154) — the wrong token here
        // has already been consumed, unlike the branch above.
        Some(_) => {
            skip_scope(scanner)?;
            Ok(None)
        }
        None => Ok(None),
    }
}

/// `DsnFile.readFloatScope` (DsnFile.java:162-188): reads one numeric token followed by the
/// closing bracket, **widening an `Int` token to `f64`** — the direction [`read_integer_scope`]
/// does not accept.
///
// Java bug: (#90) the float twin of `readIntegerScope`'s failure branches (DsnFile.java:171-175,
// :179-183): a non-numeric token warns and returns `0.0` without consuming the scope's closing
// bracket. // fixed: T4 (#90) — same repair, same commit: the scope is consumed to its matching
// bracket and the answer is `None`, so a malformed `(preferred_direction_trace_costs …)` leaves
// the layer's cost at whatever it already was rather than zeroing it.
pub fn read_float_scope(scanner: &mut DsnScanner) -> Result<Option<f64>, DsnError> {
    let value = match scanner.next_token()? {
        Some(Token::Float(f)) => f,
        Some(Token::Int(i)) => i as f64,
        Some(_) => {
            skip_scope(scanner)?;
            return Ok(None);
        }
        None => return Ok(None),
    };
    match scanner.next_token()? {
        Some(Token::Close) => Ok(Some(value)),
        Some(_) => {
            skip_scope(scanner)?;
            Ok(None)
        }
        None => Ok(None),
    }
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
/// branch (DsnFile.java:33-35) is unreachable and not ported: `DsnReader.readBoard` only reaches
/// this call when its own `readOk` is `true`, and — per `Structure.createBoard`
/// (Structure.java:1139ff) — every path that leaves `scopeParameter.getBoard()` null also either
/// returns `false` from `Structure.readScope` (making `readOk` false) or the file has no
/// `(structure ...)` scope at all; the port's calling convention (a later task's
/// `DsnReader::read_board`) is expected to gate this call on `board.is_some()` the same way, so
/// the precondition here is simply "call this with a real board".
///
/// # Two crash-vs-totalize calls (fix round 1)
///
/// Java's own body is uneven about null-checking its two `splitToConvex()` results:
/// - `boardOutline.getShape(i).splitToConvex()` (`:67`) **is** null-checked (`:68`).
/// - `currentConductionArea.getArea().splitToConvex()` (`:86`) is **not** — the loop straight
///   after it (`:88`) would NPE on a `null`.
///
// totalized: `routingBoard.getOutline()` (DsnFile.java:64) is itself never null when
// `routingBoard` isn't: `Structure.createBoard` never calls `boardHandling.createBoard(...)`
// (the only thing that can produce a non-null board) without first guaranteeing at least one
// outline shape (Structure.java:1219-1225, "construct an outline from the boundingShape, if the
// outline is missing"), and `BasicBoard`'s constructor inserts that outline unconditionally
// (mirrored by `Board::new`, which "is never empty: the outline takes id 1"). So
// `board.get_outline()` returning `None` here is unreachable in the port too; this function
// simply leaves `board_area` at `0.0` in that unreachable case rather than crashing, which is a
// harmless totalization of an impossible branch, not an observable behavioural choice.
///
/// The **other** `splitToConvex()` — a conduction area's own shape — genuinely can return `None`
/// for a degenerate enough copper-pour polygon, and that failure is reachable and observable
/// (an uncaught NPE would abort the whole `DsnReader.readBoard` call, since nothing between it
/// and this function catches anything). The port surfaces that one as
/// [`DsnError::UnsplittableConductionArea`] instead of silently skipping the area.
///
/// Also note: when `board.get_outline()` genuinely has no shapes (or, per the unreachable case
/// above, no outline at all), `board_area` stays `0.0`, so `current_area < 0.5 * board_area`
/// (comparing against `0.0`) can never be true — every conduction area's `current_area` (which
/// is `>= 0.0`) passes that gate trivially. Java has the identical consequence (`boardArea`
/// stays `0` in the same circumstance), so this is not a divergence, just worth flagging.
///
/// Java's `FRLogger.info` per newly-recognised plane layer (DsnFile.java:100-107) is dropped —
/// no `tracing` in `fr-dsn`.
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
    // totalized: `Board::get_outline` returning `None` (DsnFile.java:64's NPE if
    // `getOutline()` were null) is unreachable per this function's own doc comment above; the
    // `if let` here is the totalization of that unreachable branch (no board area contribution),
    // not an observable behaviour choice.
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
            // DsnFile.java:86-88: no null check here in Java either — reachable, so `Err`,
            // not a silently skipped area. See the doc comment above.
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

    Ok(!nothing_changed)
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
        assert_eq!(read_integer_scope(&mut scanner).expect("integer"), Some(5));
    }

    #[test]
    fn read_integer_scope_reports_no_value_for_a_float() {
        // fixed: T4 (#90) — the jar answers `0` here and writes it back out; the port answers
        // "no value read", which leaves the field for a higher-priority settings source.
        let mut scanner = scan("5.0)");
        assert_eq!(
            read_integer_scope(&mut scanner).expect("no scan error"),
            None
        );
    }

    #[test]
    fn read_integer_scope_consumes_the_closing_bracket_after_a_bad_first_token() {
        // fixed: T4 (#90) — DsnFile.java:141-146 returns `0` immediately, leaving the scope's own
        // `)` for the caller's loop to misread as the end of *its* scope. The scope is now
        // consumed to its matching bracket, so nothing is left behind.
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
        // The bracket count starts at 1, so a malformed body carrying its own `(...)` is
        // consumed whole rather than leaving two brackets behind.
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
        // fixed: T4 (#90) — was Java's totalized `0.0`, and the `)` stayed unread.
        let mut scanner = scan("on) tail");
        assert_eq!(read_float_scope(&mut scanner).expect("no scan error"), None);
        assert_eq!(
            scanner.next_token().unwrap(),
            Some(Token::Str("tail".to_string()))
        );
    }
}
