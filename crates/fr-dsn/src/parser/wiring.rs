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
//! The scope's tail (Wiring.java:344-351) is plan ruling 4: Java ends every DSN read with
//! `board.normalizeAllTraces()` inside a `try`/`catch (Exception)`, and quirk #76 makes that
//! call non-terminating on a four-rung ladder. The port runs it under a `TimeLimit`-backed
//! [`fr_board::datastructures::StopCheck`] and lands a trip in the branch Java already has,
//! with Java's own message.

use fr_board::datastructures::TimeLimit;
use fr_board::rules::ItemClass;
use fr_board::{Board, BoardError, FixedState, ItemId, NetClassId, PadstackId};
use fr_geometry::{Line, Point, Polygon, Polyline, TileShape};

use crate::error::DsnError;
use crate::format::java_round_to_int;
use crate::keyword::Keyword;
use crate::lexer::Token;
use crate::parser::geometry::{self as shape, DsnShape};
use crate::parser::network::NetId;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};
use crate::parser::{dsn_file, library};

// renamed: Wiring.readScope -> read_wiring_scope.
/// `Wiring.readScope` (Wiring.java:311-355).
///
/// The loop is Java's exactly: only a token that directly follows `(` is dispatched, `wire`'s
/// return value is discarded (:337) while `via`'s is not (:339), and anything else is skipped.
/// End of file and a scanner error both answer `false` (:319-333); the `FRLogger.warn` texts
/// that accompany them are dropped.
pub fn read_wiring_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        // Wiring.java:317-333: Java answers `false` for an IO error and for end of file alike.
        // Only a genuine scanner `Error` propagates here (see `skip_scope`'s docs).
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // "Wiring.read_scope: unexpected end of file at '…'" (:327-331).
            return Ok(false);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        let mut read_ok = true;
        if prev_token == Some(Token::Open) {
            match token {
                // Wiring.java:337: the returned `Item` is discarded.
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

    // Wiring.java:344-351, plan ruling 4. Java: `try { board.normalizeAllTraces(); } catch
    // (Exception e) { FRLogger.debug(msg); scopeParameter.warnings.add(msg); }`.
    let deadline = TimeLimit::new(p.options.normalize_time_limit_ms());
    let stop = move || deadline.is_exceeded();
    // totalized: Wiring.readScope dereferences `boardHandling.getRoutingBoard()` unguarded
    // (Wiring.java:344), so a DSN whose `structure` scope produced no board — the
    // `boardOutlineOk == false` case — NPEs straight out of `DsnReader.readBoard`, which has no
    // `catch`. The port answers `false` instead, which `read_board` turns into the
    // `OutlineMissing` variant it would have produced anyway had the file ended one scope
    // earlier: a defined outcome in place of a crash, and the only outcome the caller can act on.
    let Some(board) = p.board.as_mut() else {
        return Ok(false);
    };
    if board.normalize_all_traces_checked(&stop).is_err() {
        // Wiring.java:348 — Java's own message for the `catch (Exception)` around
        // `normalizeAllTraces`. `BoardError::Stopped` (the ruling-4 time-limit trip) and a real
        // normalisation failure both land here, exactly as Java's `catch (Exception e)` catches
        // both.
        p.warnings
            .push("Wiring: normalization of traces failed".to_string());
    }
    Ok(true)
}

/// `Wiring.readWireScope` (Wiring.java:357-582): one `(wire …)` entry — a trace when it carries
/// a `path`/`polyline_path`, a conduction area when it carries a `rect`/`polygon`/`circle`.
///
/// Java answers the inserted `Item` or `null`; the port answers its [`ItemId`]. The result is
/// discarded by the only caller (:337) — it exists solely for the `tryCorrectNet` tail (:575).
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
        let layer_structure = p.layer_structure.clone();
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
            // Wiring.java:388-400.
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
            // Wiring.java:401-416: a hole. Java appends the shape even when it is `null`
            // (`holeList.add(holeShape)`), which is what makes `transformAreaToBoard` NPE on a
            // hole it could not read — see `Shape::transform_area_to_board`'s totalization.
            Token::Kw(Keyword::Window) => {
                let hole_shape = shape::read_scope(&mut p.scanner, layer_structure.as_ref())?;
                hole_list.push(hole_shape);
                // overread the closing bracket
                next_token = p.scanner.next_token()?;
                if next_token != Some(Token::Close) {
                    // "Wiring.read_wire_scope: closing bracket expected at '…'" (:411-415).
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

    // totalized: `Wiring.readWireScope` dereferences the board (:435) and the coordinate
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
        // "Wiring.read_wire_scope: unexpected Path subclass at '…'" (:571-576). Unreachable
        // here: `path` is one of the two `Path` subclasses or `None`, and `None` with no
        // `borderShape` already returned above.
        return Ok(None);
    }

    // Wiring.java:577-580.
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

/// `Wiring.tryCorrectNet` (Wiring.java:586-604): "Maybe trace of type turret without net in
/// Mentor design. Try to assign the net by calculating the overlaps."
///
/// Java calls `getNormalContacts(corner, true)` — `ignoreNet = true` — at both ends and takes
/// the first contact that has exactly one net.
fn try_correct_net(board: &mut Board, id: ItemId) {
    let Some(fr_board::Item::Trace(trace)) = board.items.get(&id) else {
        return;
    };
    let (Some(first_corner), Some(last_corner)) = (trace.first_corner(), trace.last_corner())
    else {
        return;
    };
    let mut contacts = board.trace_normal_contacts_at(id, &first_corner, true);
    contacts.extend(board.trace_normal_contacts_at(id, &last_corner, true));
    let mut corrected_net_no = 0;
    for contact in contacts {
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

/// `Wiring.readViaScope` (Wiring.java:606-714): one `(via <padstack> <x> <y> …)` entry.
#[allow(clippy::too_many_lines)]
fn read_via_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut fixed = FixedState::Unfixed;
    // read the padstack name (Wiring.java:610-618)
    let mut next_token = p.scanner.next_token()?;
    let Some(Token::Str(padstack_name)) = next_token.clone() else {
        // "Wiring.read_via_scope: padstack name expected at '…'" (:612-617).
        return Ok(false);
    };
    p.scanner.set_scope_identifier(&padstack_name);

    // read the location (Wiring.java:620-633)
    let mut location = [0.0f64; 2];
    for slot in &mut location {
        next_token = p.scanner.next_token()?;
        match next_token {
            Some(Token::Float(value)) => *slot = value,
            Some(Token::Int(value)) => *slot = value as f64,
            _ => {
                // "Wiring.read_via_scope: number expected at '…'" (:627-632).
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
            // "Wiring.read_via_scope: unexpected end of file at '…'" (:640-645).
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
    // Wiring.java:657 and :698 — see `read_wiring_scope`'s note.
    let Some(coordinate_transform) = p.coordinate_transform else {
        return Ok(false);
    };
    let Some(board) = p.board.as_mut() else {
        return Ok(false);
    };

    // Wiring.java:658-671. The lookup strips every `.<digits>` run; the *warning* keeps the
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

/// `Wiring.viaExists` (Wiring.java:230-249): is there already a via of exactly these nets, at
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
        // Java's `ItemSelectionFilter(VIAS)` — the cast at :237 relies on it.
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

/// `Wiring.getSubnets` (Wiring.java:216-228), flattened to the two things both callers read off
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

/// `Wiring.readNetId` (Wiring.java:280-305): "Reads a netId. The subnetNumber of the netId will
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
    // "Wiring.read_net_id: closing bracket expected at '…'" (:295-299) — a warning only; Java
    // returns the id regardless.
    let _ = next_token;
    Some(NetId::new(net_name, subnet_number))
}

/// `Wiring.calcFixed` (Wiring.java:252-277): the body of a `(type …)` scope.
///
/// Anything that is not `shove_fixed`, `fix` or `normal` counts as `USER_FIXED` (:259-265) —
/// including end of file, whose `None` token is not `NORMAL`. A missing closing bracket resets
/// the answer to `UNFIXED` (:267-272).
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
        // "Wiring.is_fixed: ) expected at '…'" (:270).
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

// not ported: Wiring.writeScope / writeWireScope / writeViaScope / writeConductionAreaScope /
// writeNet / writeFixedState (Wiring.java:47-215) — the DSN write path, which Plan 3 Task 11
// owns.
