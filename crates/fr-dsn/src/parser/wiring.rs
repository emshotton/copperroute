//! `io/specctra/parser/Wiring.java` — the `wiring` scope (`wire`/`via` entries).
//!
//! This is the one scope reader that produces machine-readable warnings: the eight
//! `scopeParameter.warnings.add(msg)` sites at Wiring.java:351, 432, 468, 508, 544, 669, 682 and
//! 703 are the **only** ones in the whole DSN reader (`grep -rn "warnings.add"
//! io/specctra/` finds nothing else), so every string below is transcribed verbatim —
//! em dashes included — because `RulesRoundTripTest.loadingProducesWarningsForDegenerateWires`
//! and `BoardReadResult.warnings` consumers match on them. Every other `FRLogger` call in the
//! file vanishes, per the plan's Global Constraints.
//!
//! The scope's tail (Wiring.java:345-352) is plan ruling 4: Java ends every DSN read with
//! `board.normalizeAllTraces()` (:347) inside a `try`/`catch (Exception)`, and quirk #76 makes
//! that call non-terminating on a four-rung ladder. The port runs it under a `TimeLimit`-backed
//! [`fr_board::datastructures::StopCheck`] and lands a trip in the branch Java already has,
//! with Java's own message.

use std::io::Write;

use fr_board::datastructures::TimeLimit;
use fr_board::rules::ItemClass;
use fr_board::{Board, BoardError, FixedState, Item, ItemId, NetClassId, PadstackId};
use fr_geometry::{Area, FloatPoint, Line, Point, Polygon, Polyline, TileShape};

use crate::error::DsnError;
use crate::format::{IdentifierType, IndentFileWriter, java_double_to_string, java_round_to_int};
use crate::keyword::Keyword;
use crate::lexer::Token;
use crate::parser::geometry::{self as shape, DsnLayer, DsnPolygonPath, DsnPolylinePath, DsnShape};
use crate::parser::network::{
    NetId, clearance_class_name, write_item_clearance_class, write_net_id,
};
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};
use crate::parser::{dsn_file, library};

// renamed: Wiring.readScope -> read_wiring_scope.
/// `Wiring.readScope` (Wiring.java:306-354).
///
/// The loop is Java's exactly: only a token that directly follows `(` is dispatched, `wire`'s
/// return value is discarded (:334) while `via`'s is not (:336), and anything else is skipped.
/// End of file and a scanner error both answer `false` (:313-326); the `FRLogger.warn` texts
/// that accompany them are dropped.
pub fn read_wiring_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        // Wiring.java:311-326: Java answers `false` for an IO error and for end of file alike.
        // Only a genuine scanner `Error` propagates here (see `skip_scope`'s docs).
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // "Wiring.read_scope: unexpected end of file at '…'" (:320-326).
            return Ok(false);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        let mut read_ok = true;
        if prev_token == Some(Token::Open) {
            match token {
                // Wiring.java:334: the returned `Item` is discarded.
                Token::Kw(Keyword::Wire) => {
                    read_wire_scope(p)?;
                }
                Token::Kw(Keyword::Via) => read_ok = read_via_scope(p)?,
                _ => {
                    skip_scope(&mut p.scanner)?;
                }
            }
        }
        if !read_ok {
            return Ok(false);
        }
    }

    // Wiring.java:345-352, plan ruling 4. Java: `try { board.normalizeAllTraces(); } catch
    // (Exception e) { FRLogger.debug(msg); scopeParameter.warnings.add(msg); }` — the call is
    // :347, the message literal :349 and the `warnings.add` :351.
    let limit_ms = p.options.normalize_time_limit_ms();
    let deadline = TimeLimit::new(limit_ms);
    // A zero budget trips before the first check. Java's `TimeLimit.isExceeded` compares
    // *strictly* greater than the deadline (TimeLimit.java:20), so `TimeLimit::new(0)` alone
    // would only trip once a millisecond had elapsed inside the walk — which makes
    // `DsnReadOptions { normalize_time_limit: Duration::ZERO }` a race rather than an opt-out.
    // The explicit test gives it a defined meaning and makes the trip deterministic.
    let stop = move || limit_ms <= 0 || deadline.is_exceeded();
    // totalized: Wiring.readScope dereferences `boardHandling.getRoutingBoard()` unguarded
    // (Wiring.java:345), so a DSN whose `structure` scope produced no board — the
    // `boardOutlineOk == false` case — NPEs straight out of `DsnReader.readBoard`, which has no
    // `catch`. The port answers `false` instead, which `read_board` turns into the
    // `OutlineMissing` variant it would have produced anyway had the file ended one scope
    // earlier: a defined outcome in place of a crash, and the only outcome the caller can act on.
    let Some(board) = p.board.as_mut() else {
        return Ok(false);
    };
    if board.normalize_all_traces_checked(&stop).is_err() {
        // Wiring.java:349 — Java's own message for the `catch (Exception)` around
        // `normalizeAllTraces`. `BoardError::Stopped` (the ruling-4 time-limit trip) and a real
        // normalisation failure both land here, exactly as Java's `catch (Exception e)` catches
        // both.
        p.warnings
            .push("Wiring: normalization of traces failed".to_string());
    }
    Ok(true)
}

/// `Wiring.readWireScope` (Wiring.java:356-577): one `(wire …)` entry — a trace when it carries
/// a `path`/`polyline_path`, a conduction area when it carries a `rect`/`polygon`/`circle`.
///
/// Java answers the inserted `Item` or `null`; the port answers its [`ItemId`]. The result is
/// discarded by the only caller (:334) — it exists solely for the `tryCorrectNet` tail
/// (:573-575).
#[allow(clippy::too_many_lines)]
fn read_wire_scope(p: &mut ReadScopeParameter<'_>) -> Result<Option<ItemId>, DsnError> {
    let mut net_id: Option<NetId> = None;
    let mut clearance_class_name: Option<String> = None;
    let mut fixed = FixedState::Unfixed;
    // Used, if a trace is read.
    let mut path: Option<DsnShape> = None;
    // Used, if a conduction area is read.
    let mut border_shape: Option<DsnShape> = None;
    let mut hole_list: Vec<Option<DsnShape>> = Vec::new();

    // `ReadScopeParameter.layerStructure` is set once, by `Structure.readScope`, and cannot
    // change while a `wiring` scope is being read — so this is cloned once here rather than once
    // per sub-scope token (fix round 1; the shape readers below need it while `p.scanner` is
    // mutably borrowed, which is why it is a clone at all).
    let layer_structure = p.layer_structure.clone();
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // "Wiring.read_wire_scope: unexpected end of file at '…'" (:372-378).
            return Ok(None);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        if prev_token != Some(Token::Open) {
            continue;
        }
        match token {
            // Wiring.java:384-387.
            Token::Kw(Keyword::PolygonPath) => {
                path = shape::read_polygon_path_scope(&mut p.scanner, layer_structure.as_ref())?
                    .map(DsnShape::Path);
            }
            Token::Kw(Keyword::PolylinePath) => {
                path = shape::read_polyline_path_scope(&mut p.scanner, layer_structure.as_ref())?
                    .map(DsnShape::PolylinePath);
            }
            // Wiring.java:388-399.
            Token::Kw(Keyword::Rectangle) => {
                border_shape =
                    shape::read_rectangle_scope(&mut p.scanner, layer_structure.as_ref())?
                        .map(DsnShape::Rect);
            }
            Token::Kw(Keyword::Polygon) => {
                border_shape = shape::read_polygon_scope(&mut p.scanner, layer_structure.as_ref())?
                    .map(DsnShape::Polygon);
            }
            Token::Kw(Keyword::Circle) => {
                border_shape = shape::read_circle_scope(&mut p.scanner, layer_structure.as_ref())?
                    .map(DsnShape::Circle);
            }
            // Wiring.java:400-416: a hole. Java appends the shape even when it is `null`
            // (`holeList.add(holeShape)`), which is what makes `transformAreaToBoard` NPE on a
            // hole it could not read — see `Shape::transform_area_to_board`'s totalization.
            Token::Kw(Keyword::Window) => {
                let hole_shape = shape::read_scope(&mut p.scanner, layer_structure.as_ref())?;
                hole_list.push(hole_shape);
                // overread the closing bracket
                next_token = p.scanner.next_token()?;
                if next_token != Some(Token::Close) {
                    // "Wiring.read_wire_scope: closing bracket expected at '…'" (:410-416).
                    return Ok(None);
                }
            }
            // Wiring.java:417-423.
            Token::Kw(Keyword::Net) => net_id = read_net_id(p),
            Token::Kw(Keyword::ClearanceClass) => {
                clearance_class_name = Some(dsn_file::read_string_scope(&mut p.scanner)?);
            }
            Token::Kw(Keyword::Type) => fixed = calc_fixed(p)?,
            _ => {
                skip_scope(&mut p.scanner)?;
            }
        }
    }

    // Wiring.java:428-434.
    if path.is_none() && border_shape.is_none() {
        let msg = format!(
            "Wiring: wire has no shape at '{}'",
            p.scanner.scope_identifier()
        );
        p.warnings.push(msg);
        return Ok(None);
    }

    // totalized: `Wiring.readWireScope` dereferences the board (:436) and the coordinate
    // transform (:454) unguarded; both are `null` when the `structure` scope produced no board.
    // See `read_wiring_scope`'s note — the port skips the wire and lets the tail answer `false`.
    let Some(coordinate_transform) = p.coordinate_transform else {
        return Ok(None);
    };
    let Some(board) = p.board.as_mut() else {
        return Ok(None);
    };

    // Wiring.java:437-445. `netClass` ends up as the *last* found net's class, and `netNumbers`
    // is filled in iteration order.
    let mut net_class: NetClassId = board.rules.get_default_net_class();
    let found_nets = get_subnets(net_id.as_ref(), board);
    let mut net_numbers: Vec<i32> = Vec::with_capacity(found_nets.len());
    for (net_number, found_class) in &found_nets {
        net_numbers.push(*net_number);
        net_class = *found_class;
    }

    // Wiring.java:446-449: Java's `ClearanceMatrix.getNo` answers `-1` for an unknown name.
    let mut clearance_class_index: i32 = -1;
    if let Some(name) = &clearance_class_name {
        clearance_class_index = board
            .rules
            .clearance_matrix
            .get_no(name)
            .map_or(-1, |no| no as i32);
    }

    // Wiring.java:450-457.
    let (layer_index, half_width) = match (&path, &border_shape) {
        (Some(path), _) => (
            path.layer().no,
            // Wiring.java:454: `Math.round(coordinateTransform.dsnToBoard(path.width / 2))`.
            java_round_to_int(coordinate_transform.dsn_to_board(path_width(path) / 2.0)),
        ),
        (None, Some(border_shape)) => (border_shape.layer().no, 0),
        (None, None) => unreachable!("the `path == null && borderShape == null` exit is above"),
    };

    // Wiring.java:459-470.
    if layer_index < 0 || layer_index >= board.get_layer_count() as i32 {
        let layer_name = match (&path, &border_shape) {
            (Some(path), _) => path.layer().name.clone(),
            (None, Some(border_shape)) => border_shape.layer().name.clone(),
            (None, None) => unreachable!(),
        };
        let msg = format!(
            "Wiring: wire ignored — unknown layer '{layer_name}' at '{}'",
            p.scanner.scope_identifier()
        );
        p.warnings.push(msg);
        return Ok(None);
    }
    let layer_index = layer_index as usize;

    // Wiring.java:472.
    let bounding_box = TileShape::from(board.get_bounding_box());

    let mut result: Option<ItemId> = None;
    if let Some(border_shape) = border_shape {
        // Wiring.java:475-490: a conduction area.
        if clearance_class_index < 0 {
            clearance_class_index = board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes
                .get(ItemClass::Area) as i32;
        }
        let mut area: Vec<Option<DsnShape>> = Vec::with_capacity(1 + hole_list.len());
        area.push(Some(border_shape));
        area.extend(hole_list);
        // totalized: `Shape.transformAreaToBoard` can answer `null` (see its own note); Java
        // then NPEs inside `insertConductionArea`. The port skips the area.
        let Some(conduction_area) = shape::transform_area_to_board(&area, &coordinate_transform)
        else {
            return Ok(None);
        };
        result = Some(board.insert_conduction_area(
            conduction_area,
            layer_index,
            net_numbers.clone(),
            clearance_class_index as usize,
            false,
            fixed,
        ));
    } else if let Some(DsnShape::Path(path)) = &path {
        // Wiring.java:491-545: a polygon path — a trace given by its corners.
        if clearance_class_index < 0 {
            clearance_class_index = board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes
                .get(ItemClass::Trace) as i32;
        }
        let coordinate_arr = &path.coordinate_arr;
        let mut corners: Vec<Point> = Vec::with_capacity(coordinate_arr.len() / 2);
        for i in 0..coordinate_arr.len() / 2 {
            let current_point = [coordinate_arr[2 * i], coordinate_arr[2 * i + 1]];
            let current_corner = coordinate_transform.dsn_to_board_point(&current_point);
            if !bounding_box.contains_float(&current_corner) {
                // Wiring.java:498-510. The coordinates are the **DSN** ones, truncated by a Java
                // `(int)` cast (which saturates, exactly like Rust's `as i32`).
                let msg = format!(
                    "Wiring: wire corner ({},{}) is outside board bounds at '{}'",
                    current_point[0] as i32,
                    current_point[1] as i32,
                    p.scanner.scope_identifier()
                );
                p.warnings.push(msg);
                return Ok(None);
            }
            corners.push(Point::Int(current_corner.round()));
        }

        let polygon = Polygon::new(corners);
        let polygon_corners = polygon.corner_array();
        // Wiring.java:517-529: a wire is degenerate if it has fewer than 2 corners, or if all
        // corners map to the same point.
        let has_distinct_corner = polygon_corners
            .iter()
            .skip(1)
            .any(|corner| *corner != polygon_corners[0]);
        let is_degenerate = polygon_corners.len() < 2 || !has_distinct_corner;
        if is_degenerate {
            // Wiring.java:536-545.
            let msg = format!(
                "Wiring: degenerate wire trace skipped (all {} corners are identical — \
                 zero-length trace) on layer '{}'. This is likely a DSN export issue in your EDA \
                 tool.",
                polygon_corners.len(),
                path.layer.name
            );
            p.warnings.push(msg);
        } else {
            // Traces are not yet normalized here because cycles may be removed premature.
            let trace_polyline = Polyline::from_polygon(&polygon);
            result = board.insert_trace_without_cleaning(
                trace_polyline,
                layer_index,
                half_width,
                net_numbers.clone(),
                clearance_class_index as usize,
                fixed,
            );
        }
    } else if let Some(DsnShape::PolylinePath(path)) = &path {
        // Wiring.java:546-570: a polyline path — a trace given by its lines.
        if clearance_class_index < 0 {
            clearance_class_index = board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes
                .get(ItemClass::Trace) as i32;
        }
        let coordinate_arr = &path.coordinate_arr;
        let mut lines: Vec<Line> = Vec::with_capacity(coordinate_arr.len() / 4);
        for i in 0..coordinate_arr.len() / 4 {
            let a = coordinate_transform
                .dsn_to_board_point(&[coordinate_arr[4 * i], coordinate_arr[4 * i + 1]]);
            let b = coordinate_transform
                .dsn_to_board_point(&[coordinate_arr[4 * i + 2], coordinate_arr[4 * i + 3]]);
            lines.push(Line::new(a.round(), b.round()));
        }
        // totalized: Wiring.readWireScope — Java's `new Polyline(Line[])` (Wiring.java:562)
        // cannot fail: `Polyline(Line[])` answers an empty `lines` array for anything it cannot
        // normalise, and `insertTraceWithoutCleaning` then returns `null` for it
        // (BasicBoard.java:185-187). The port's `Polyline::from_lines` answers
        // `Err(PolylineError::NormalizationIndexUnderflow)` on the one input class where Java's
        // constructor *throws* instead (Plan 1: `remove_overlaps`' index underflow), which no
        // Java caller can observe as a value — so it propagates as `DsnError::Board` and fails
        // the read rather than silently inserting nothing. See docs/java-quirks.md quirk #109.
        let trace_polyline = Polyline::from_lines(lines).map_err(BoardError::from)?;
        result = board.insert_trace_without_cleaning(
            trace_polyline,
            layer_index,
            half_width,
            net_numbers.clone(),
            clearance_class_index as usize,
            fixed,
        );
    } else {
        // "Wiring.read_wire_scope: unexpected Path subclass at '…'" (:566-572). Unreachable
        // here: `path` is one of the two `Path` subclasses or `None`, and `None` with no
        // `borderShape` already returned above.
        return Ok(None);
    }

    // Wiring.java:573-575.
    if let Some(id) = result
        && board
            .items
            .get(&id)
            .is_some_and(|item| item.header().net_count() == 0)
    {
        try_correct_net(board, id);
    }
    Ok(result)
}

/// `Wiring.tryCorrectNet` (Wiring.java:583-599): "Maybe trace of type turret without net in
/// Mentor design. Try to assign the net by calculating the overlaps."
///
/// Java calls `getNormalContacts(corner, true)` — `ignoreNet = true` — at both ends (:587-588),
/// merges them into one set and takes the first contact with exactly one net (:590-595).
///
/// # Iteration order is load-bearing (fix round 1)
///
/// `Trace.getNormalContacts` answers a `TreeSet<Item>`, which iterates by `Item.compareTo` —
/// **descending** id (quirk #44) — and `addAll` merges the second end into the same set, so
/// Java's `break` takes the **highest-id** single-net contact of either end. The port's
/// `BTreeSet<ItemId>` iterates ascending, so the walk is `.rev()`ed. A netless trace whose two
/// ends touch pins of different nets is where the two directions disagree; the test
/// `try_correct_net_takes_the_highest_id_contact` pins it.
///
/// A `null` first or last corner makes Java's `getNormalContacts(null, true)` answer the *empty*
/// set for that end (Trace.java:174-176) while the other end is still gathered — hence two
/// independent `Option` arms here rather than one early return.
fn try_correct_net(board: &mut Board, id: ItemId) {
    let Some(fr_board::Item::Trace(trace)) = board.items.get(&id) else {
        return;
    };
    let (first_corner, last_corner) = (trace.first_corner(), trace.last_corner());
    let mut contacts = std::collections::BTreeSet::new();
    if let Some(corner) = first_corner {
        contacts.extend(board.trace_normal_contacts_at(id, &corner, true));
    }
    if let Some(corner) = last_corner {
        contacts.extend(board.trace_normal_contacts_at(id, &corner, true));
    }
    let mut corrected_net_no = 0;
    // `.rev()`: descending id, matching Java's `TreeSet` walk (quirk #44).
    for contact in contacts.into_iter().rev() {
        let Some(item) = board.items.get(&contact) else {
            continue;
        };
        if item.header().net_count() == 1 {
            corrected_net_no = item.header().get_net_number(0);
            break;
        }
    }
    if corrected_net_no != 0 {
        let nets = board.rules.nets.clone();
        if let Some(item) = board.items.get_mut(&id) {
            item.assign_net_no(corrected_net_no, &nets);
        }
    }
}

/// `Wiring.readViaScope` (Wiring.java:601-714): one `(via <padstack> <x> <y> …)` entry.
#[allow(clippy::too_many_lines)]
fn read_via_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut fixed = FixedState::Unfixed;
    // read the padstack name (Wiring.java:604-612)
    let mut next_token = p.scanner.next_token()?;
    let Some(Token::Str(padstack_name)) = next_token.clone() else {
        // "Wiring.read_via_scope: padstack name expected at '…'" (:606-612).
        return Ok(false);
    };
    p.scanner.set_scope_identifier(&padstack_name);

    // read the location (Wiring.java:614-629)
    let mut location = [0.0f64; 2];
    for slot in &mut location {
        next_token = p.scanner.next_token()?;
        match next_token {
            Some(Token::Float(value)) => *slot = value,
            Some(Token::Int(value)) => *slot = value as f64,
            _ => {
                // "Wiring.read_via_scope: number expected at '…'" (:622-628).
                return Ok(false);
            }
        }
    }

    let mut net_id: Option<NetId> = None;
    let mut clearance_class_name: Option<String> = None;
    loop {
        let prev_token = next_token;
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // "Wiring.read_via_scope: unexpected end of file at '…'" (:635-641).
            return Ok(false);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        if prev_token != Some(Token::Open) {
            continue;
        }
        match token {
            Token::Kw(Keyword::Net) => net_id = read_net_id(p),
            Token::Kw(Keyword::ClearanceClass) => {
                clearance_class_name = Some(dsn_file::read_string_scope(&mut p.scanner)?);
            }
            Token::Kw(Keyword::Type) => fixed = calc_fixed(p)?,
            _ => {
                skip_scope(&mut p.scanner)?;
            }
        }
    }

    // totalized: the board and the coordinate transform are dereferenced unguarded at
    // Wiring.java:659 and :698 — see `read_wiring_scope`'s note.
    let Some(coordinate_transform) = p.coordinate_transform else {
        return Ok(false);
    };
    let Some(board) = p.board.as_mut() else {
        return Ok(false);
    };

    // Wiring.java:659-671. The lookup strips every `.<digits>` run; the *warning* keeps the
    // original name.
    let cleaned_name = library::strip_dot_digits(&padstack_name);
    let Some(current_padstack) = board
        .library
        .padstacks
        .get_by_name(&cleaned_name)
        .map(|padstack| PadstackId(padstack.no))
    else {
        let msg = format!(
            "Wiring: via padstack '{padstack_name}' not found at '{}'",
            p.scanner.scope_identifier()
        );
        p.warnings.push(msg);
        return Ok(false);
    };

    // Wiring.java:672-683.
    let mut net_class: NetClassId = board.rules.get_default_net_class();
    let found_nets = get_subnets(net_id.as_ref(), board);
    if let Some(net_id) = &net_id
        && found_nets.is_empty()
    {
        let msg = format!(
            "Wiring: via net '{}' not found at '{}'",
            net_id.name,
            p.scanner.scope_identifier()
        );
        p.warnings.push(msg);
    }
    // Java bug: Wiring.readViaScope — Wiring.java:684-690 declares `int currentIndex = 0` and writes
    // `netNumbers[currentIndex] = currentNet.netNumber` inside the loop **without ever
    // incrementing it** — unlike the identical loop in `readWireScope` (:440-445), which does.
    // A via on a net name with several subnets therefore gets `netNumbers = [lastSubnet, 0, 0,
    // …]`: one real net number followed by `foundNets.size() - 1` zeros, and `Nets.isNormalNetNumber`
    // treats 0 as "no net". Reproduced, zeros and all. See docs/java-quirks.md quirk #105.
    let mut net_numbers: Vec<i32> = vec![0; found_nets.len()];
    for (net_number, found_class) in &found_nets {
        if let Some(slot) = net_numbers.first_mut() {
            *slot = *net_number;
        }
        net_class = *found_class;
    }

    // Wiring.java:691-697.
    let mut clearance_class_index: i32 = -1;
    if let Some(name) = &clearance_class_name {
        clearance_class_index = board
            .rules
            .clearance_matrix
            .get_no(name)
            .map_or(-1, |no| no as i32);
    }
    if clearance_class_index < 0 {
        clearance_class_index = board
            .rules
            .net_classes
            .get(net_class)
            .default_item_clearance_classes
            .get(ItemClass::Via) as i32;
    }

    // Wiring.java:698-712.
    let board_location = coordinate_transform.dsn_to_board_point(&location).round();
    if via_exists(board, &board_location, current_padstack, &net_numbers) {
        let msg = format!(
            "Wiring: duplicate via skipped at ({}, {})",
            board_location.x, board_location.y
        );
        p.warnings.push(msg);
    } else {
        let attach_allowed = p.via_at_smd_allowed
            && board
                .library
                .padstacks
                .get(current_padstack)
                .is_some_and(|padstack| padstack.attach_allowed);
        let board = p.board.as_mut().expect("checked above");
        // obligation: Plan 8 — BasicBoard.insertVia (Wiring.java:706) walks
        // `fromLayer..toLayer` calling `splitTraces` -> `PolylineTrace.split`, i.e. the same
        // machinery quirk #76 does not terminate in, and it is **outside** the
        // `try`/`catch` Java wraps `normalizeAllTraces` in (:346-352). Plan ruling 4's
        // `StopCheck` therefore does not reach it: a DSN whose vias sit on a four-rung ladder can
        // still wedge the reader. **The seam now exists**: plan-6 ruling 6 (Task 10b) added
        // `Board::insert_via_checked`, for `ForcedViaInserter::insert`'s sake, and left
        // `insert_via` as a `|| false` wrapper. What is left here is to pass this reader's own
        // `normalize_time_limit`-backed check to it, which changes DSN-reader behaviour under the
        // 105-file corpus and so was not folded into a router task — see docs/java-quirks.md's
        // obligation register.
        // totalized: Wiring.readViaScope — Java's `board.insertVia` (:706) cannot fail, and
        // `readViaScope`'s `catch` only covers `IOException`; the port's returns
        // `Result<ItemId, BoardError>` because `split_traces` can surface a `Polyline`
        // normalisation failure (and, once the obligation above is closed, a stop). Propagated
        // as `DsnError::Board`, which fails the read rather than inserting a corrupt via. See
        // docs/java-quirks.md quirk #109.
        board.insert_via(
            current_padstack,
            Point::Int(board_location),
            net_numbers,
            clearance_class_index as usize,
            fixed,
            attach_allowed,
        )?;
    }
    Ok(true)
}

/// `Wiring.viaExists` (Wiring.java:238-255): is there already a via of exactly these nets, at
/// exactly this point, spanning exactly this padstack's layer range?
fn via_exists(
    board: &Board,
    location: &fr_geometry::IntPoint,
    padstack: PadstackId,
    net_numbers: &[i32],
) -> bool {
    let Some(padstack) = board.library.padstacks.get(padstack) else {
        return false;
    };
    let from_layer = padstack.from_layer();
    let to_layer = padstack.to_layer();
    let Ok(pick_layer) = usize::try_from(from_layer) else {
        // A padstack with no shape on any layer: `fromLayer()` is `shapes.len()` and
        // `toLayer()` is -1, so Java's `pickItems` finds nothing on that out-of-range layer.
        return false;
    };
    let point = Point::Int(*location);
    let ctx = board.ctx();
    for id in board.pick_items(&point, Some(pick_layer)) {
        // Java's `ItemSelectionFilter(VIAS)` — the cast at :245 relies on it.
        let Some(fr_board::Item::Via(via)) = board.items.get(&id) else {
            continue;
        };
        if via.hdr.nets_equal(net_numbers)
            && via.get_center() == point
            && via.first_layer(&ctx) as i32 == from_layer
            && via.last_layer(&ctx) as i32 == to_layer
        {
            return true;
        }
    }
    false
}

/// `Wiring.getSubnets` (Wiring.java:223-236), flattened to the two things both callers read off
/// each found net: its number and its net class.
///
/// A `subnetNumber > 0` selects exactly one net; a `subnetNumber` of 0 (which is what
/// [`read_net_id`] answers when the DSN gave none) selects **every** subnet of that name.
fn get_subnets(net_id: Option<&NetId>, board: &Board) -> Vec<(i32, NetClassId)> {
    let Some(net_id) = net_id else {
        return Vec::new();
    };
    if net_id.subnet_no > 0 {
        return board
            .rules
            .nets
            .get_by_name_and_subnet(&net_id.name, net_id.subnet_no)
            .map(|net| vec![(net.net_number, net.get_net_class())])
            .unwrap_or_default();
    }
    board
        .rules
        .nets
        .get_by_name(&net_id.name)
        .into_iter()
        .map(|net| (net.net_number, net.get_net_class()))
        .collect()
}

/// `Wiring.readNetId` (Wiring.java:281-304): "Reads a netId. The subnetNumber of the netId will
/// be 0, if no subnetNumber was found."
///
/// Java answers `null` only on an `IOException`; the port's scanner has no IO to fail at (the
/// whole input is already in the buffer), so the only `None` here is a scan error, which the
/// caller cannot distinguish from Java's `null` either way.
fn read_net_id(p: &mut ReadScopeParameter<'_>) -> Option<NetId> {
    let mut subnet_number = 0;
    let net_name = p.scanner.next_string();
    p.scanner.set_scope_identifier(&net_name);
    let mut next_token = p.scanner.next_token().ok()?;
    if let Some(Token::Int(value)) = next_token {
        // The lexer widens Java's `Integer` to `i64`; narrow it back here.
        subnet_number = value as i32;
        next_token = p.scanner.next_token().ok()?;
    }
    // "Wiring.read_net_id: closing bracket expected at '…'" (:293-298) — a warning only; Java
    // returns the id regardless.
    let _ = next_token;
    Some(NetId::new(net_name, subnet_number))
}

/// `Wiring.calcFixed` (Wiring.java:257-278): the body of a `(type …)` scope.
///
/// Anything that is not `shove_fixed`, `fix` or `normal` counts as `USER_FIXED` (:261-267) —
/// including end of file, whose `None` token is not `NORMAL`. A missing closing bracket resets
/// the answer to `UNFIXED` (:268-272).
fn calc_fixed(p: &mut ReadScopeParameter<'_>) -> Result<FixedState, DsnError> {
    let mut result = FixedState::Unfixed;
    let next_token = p.scanner.next_token()?;
    match next_token {
        Some(Token::Kw(Keyword::ShoveFixed)) => result = FixedState::ShoveFixed,
        Some(Token::Kw(Keyword::Fix)) => result = FixedState::SystemFixed,
        Some(Token::Kw(Keyword::Normal)) => {}
        _ => result = FixedState::UserFixed,
    }
    let next_token = p.scanner.next_token()?;
    if next_token != Some(Token::Close) {
        // "Wiring.is_fixed: ) expected at '…'" (:269-272).
        return Ok(FixedState::Unfixed);
    }
    Ok(result)
}

/// `Path.width` (Path.java:19), the field both `Path` subclasses carry.
fn path_width(shape: &DsnShape) -> f64 {
    match shape {
        DsnShape::Path(path) => path.width,
        DsnShape::PolylinePath(path) => path.width,
        _ => 0.0,
    }
}

// ======================================================================== the `wiring` writers
//
// `Wiring.writeScope` (Wiring.java:47-77) and the five helpers it calls (:79-221), ported in
// Plan 3 Task 11; they replace the `// not ported:` marker that stood here while the reader was
// the only half of this file.

/// `Wiring.writeScope` (Wiring.java:47-77): all traces, then all vias, then the conduction areas
/// on **signal** layers (the non-signal ones belong to the `structure` scope).
///
/// The three walks are Java's three walks: `BasicBoard.getTraces` and `.getVias`, then
/// `board.itemList` directly — all three in `UndoableObjects` order, which Plan 2 established is
/// **descending item id** (quirk #63). `Board::get_traces`/`get_vias`/`items_in_board_order` all
/// answer in that order already.
// renamed: Wiring.writeScope -> write_wiring_scope.
pub fn write_wiring_scope(p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("wiring");
    // write the wires
    let board_wires = p.board.get_traces();
    for current_board_wire in board_wires {
        write_wire_scope(p, current_board_wire);
    }
    let board_vias = p.board.get_vias();
    for current_via in board_vias {
        write_via_scope(p, current_via);
    }
    // write the conduction areas
    let board = p.board;
    for id in board.items_in_board_order() {
        let Some(Item::ConductionArea(area)) = board.items.get(&id) else {
            continue;
        };
        if !board.layer_structure().layers[area.get_layer()].is_signal {
            // This conduction area is written in the structure scope.
            continue;
        }
        write_conduction_area_scope(p, id);
    }
    p.file.end_scope();
}

/// `Wiring.writeViaScope` (Wiring.java:79-109): one `(via <padstack> <x> <y> [(net …)]
/// (clearance_class …) [(type …)])` scope.
fn write_via_scope(p: &mut WriteScopeParameter<'_>, via_id: ItemId) {
    let board = p.board;
    let ctx = board.ctx();
    let Some(item) = board.items.get(&via_id) else {
        return;
    };
    let Item::Via(via) = item else {
        return;
    };
    // totalized: Java writes `via.getPadstack().name` unconditionally (Wiring.java:81); a via
    // whose padstack id is not in the library would NPE. Unreachable — every via is inserted
    // with a padstack the library holds.
    let Some(via_padstack) = via.get_padstack(&ctx) else {
        return;
    };
    let via_padstack_name = via_padstack.name.clone();
    let via_location = via.get_center().to_float();
    let via_coor = p.coordinate_transform.board_to_dsn_point(&via_location);
    let via_net = if item.net_count() > 0 {
        board.rules.nets.get(item.get_net_number(0)).cloned()
    } else {
        None
    };
    p.file.start_scope_nl();
    p.file.write("via ");
    p.identifier_type.write(&via_padstack_name, &mut p.file);
    for coor in via_coor {
        p.file.write(" ");
        p.file.write(&java_double_to_string(coor));
    }
    if let Some(via_net) = &via_net {
        write_net(via_net, &mut p.file, &p.identifier_type);
    }
    let clearance_name = clearance_class_name(&board.rules, item.clearance_class()).to_string();
    write_item_clearance_class(&clearance_name, &mut p.file, &p.identifier_type);
    write_fixed_state(&mut p.file, item.get_fixed_state());
    p.file.end_scope();
}

/// `Wiring.writeWireScope` (Wiring.java:111-155): one `(wire (polyline_path …) …)` scope — or a
/// `(path …)` one in compat mode, which writes the trace's corners rather than its lines.
fn write_wire_scope(p: &mut WriteScopeParameter<'_>, wire_id: ItemId) {
    let board = p.board;
    let Some(item) = board.items.get(&wire_id) else {
        return;
    };
    // "Wiring.write_wire_scope: trace type not yet implemented" (Wiring.java:114) — Java's
    // non-`PolylineTrace`
    // branch, unreachable here because `Trace` has exactly one variant in this port.
    let Item::Trace(current_wire) = item else {
        return;
    };
    let layer_index = current_wire.get_layer();
    let board_layer = &board.layer_structure().layers[layer_index];
    let current_layer = DsnLayer::new(
        board_layer.name.clone(),
        i32::try_from(layer_index).unwrap_or(i32::MAX),
        board_layer.is_signal,
    );
    let wire_width = p
        .coordinate_transform
        .board_to_dsn(f64::from(2 * current_wire.get_half_width()));
    let wire_net = if item.net_count() > 0 {
        board.rules.nets.get(item.get_net_number(0))
    } else {
        None
    };
    let Some(wire_net) = wire_net else {
        // "Wiring.write_wire_scope: net not found" (Wiring.java:128) — an `FRLogger.warn` this
        // port drops.
        return;
    };
    p.file.start_scope_nl();
    p.file.write("wire");

    if p.compat_mode {
        let corners = current_wire.polyline().corners();
        let float_corner_arr: Vec<FloatPoint> = corners.iter().map(Point::to_float).collect();
        let coors = p
            .coordinate_transform
            .board_to_dsn_points(&float_corner_arr);
        let current_path = DsnPolygonPath::new(current_layer, wire_width, coors);
        current_path.write_scope(&mut p.file, &p.identifier_type);
    } else {
        let coors = p
            .coordinate_transform
            .board_to_dsn_lines(current_wire.polyline().lines());
        let current_path = DsnPolylinePath::new(current_layer, wire_width, coors);
        current_path.write_scope(&mut p.file, &p.identifier_type);
    }
    write_net(wire_net, &mut p.file, &p.identifier_type);
    let clearance_name = clearance_class_name(&board.rules, item.clearance_class()).to_string();
    write_item_clearance_class(&clearance_name, &mut p.file, &p.identifier_type);
    write_fixed_state(&mut p.file, item.get_fixed_state());
    p.file.end_scope();
}

/// `Wiring.writeConductionAreaScope` (Wiring.java:157-196): a signal-layer conduction area as a
/// `(wire <shape> (window …)* (net …) (clearance_class …))` scope.
///
/// Unlike [`crate::parser::structure::write_plane_scope`], which writes the non-signal-layer ones
/// as `(plane …)`, this one carries the net and the clearance class but no fixed state.
fn write_conduction_area_scope(p: &mut WriteScopeParameter<'_>, conduction_id: ItemId) {
    let board = p.board;
    let ctx = board.ctx();
    let Some(item) = board.items.get(&conduction_id) else {
        return;
    };
    let Item::ConductionArea(conduction_area) = item else {
        return;
    };
    let net_count = item.net_count();
    if net_count != 1 {
        // "Plane.write_scope: unexpected net count" — Java's message, in `Wiring`
        // (Wiring.java:161); an `FRLogger.warn` this port drops.
        return;
    }
    // totalized: Java dereferences `rules.nets.get(...)` without a null check
    // (Wiring.java:164-165).
    let Some(current_net) = board.rules.nets.get(item.get_net_number(0)).cloned() else {
        return;
    };
    let current_area = conduction_area.get_area(&ctx);
    let layer_index = conduction_area.get_layer();
    let board_layer = &board.layer_structure().layers[layer_index];
    let conduction_layer = DsnLayer::new(
        board_layer.name.clone(),
        i32::try_from(layer_index).unwrap_or(i32::MAX),
        board_layer.is_signal,
    );
    let (boundary_shape, holes) = match current_area {
        Area::Shape(s) => (s.clone(), Vec::new()),
        area => (area.get_border(), area.get_holes()),
    };
    p.file.start_scope_nl();
    p.file.write("wire ");
    if let Some(dsn_shape) = p
        .coordinate_transform
        .board_to_dsn_shape(&boundary_shape, conduction_layer.clone())
    {
        dsn_shape.write_scope(&mut p.file, &p.identifier_type);
    }
    for hole in &holes {
        if let Some(dsn_hole) = p
            .coordinate_transform
            .board_to_dsn_shape(hole, conduction_layer.clone())
        {
            dsn_hole.write_hole_scope(&mut p.file, &p.identifier_type);
        }
    }
    write_net(&current_net, &mut p.file, &p.identifier_type);
    let clearance_name = clearance_class_name(&board.rules, item.clearance_class()).to_string();
    write_item_clearance_class(&clearance_name, &mut p.file, &p.identifier_type);
    p.file.end_scope();
}

/// `Wiring.writeNet(rules.Net, IndentFileWriter, IdentifierType)` (Wiring.java:198-205): the
/// `(net <name> <subnet>)` line every wire, via and conduction area carries.
fn write_net<W: Write>(
    net: &fr_board::Net,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    file.new_line();
    file.write("(");
    write_net_id(net, file, identifier_type);
    file.write(")");
}

/// `Wiring.writeFixedState(IndentFileWriter, FixedState)` (Wiring.java:207-221): nothing at all
/// for an unfixed item, else `(type shove_fixed|fix|protect)`.
///
/// The `shove_fixed)` literal is 2.3.0's (plan ruling 1); the clone's HEAD writes `shoveFixed)`,
/// which its own lexer cannot read back.
fn write_fixed_state<W: Write>(file: &mut IndentFileWriter<W>, fixed_state: FixedState) {
    if fixed_state == FixedState::Unfixed {
        return;
    }
    file.new_line();
    file.write("(type ");
    match fixed_state {
        FixedState::ShoveFixed => file.write("shove_fixed)"),
        FixedState::SystemFixed => file.write("fix)"),
        _ => file.write("protect)"),
    }
}
