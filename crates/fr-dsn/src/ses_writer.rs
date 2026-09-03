//! `io/specctra/SesWriter.java` — the single entry point that serialises a [`Board`]'s routing
//! result to Specctra session (`.ses`) text.
//!
//! # Deviation from Java: the coordinate transform is a parameter
//!
//! `SesWriter.writeSessionScope` (SesWriter.java:80-82) derives the session-file transform from
//! the board: `board.communication.coordinateTransform.dsnToBoard(1) / board.communication
//! .resolution`. This port's `fr-board` `Communication` has no `coordinateTransform` field —
//! Plan 3 ruling A keeps [`CoordinateTransform`] in `fr-dsn`, where the rest of the Specctra
//! layer lives, and [`crate::read_board`] hands it back on the [`crate::BoardReadResult`]
//! instead. [`write`] therefore takes the *DSN* transform as an explicit argument and derives
//! the SES one from it exactly as Java does. The bytes are identical; only the plumbing differs.
//! Same deviation, same reason as [`crate::dsn_writer::write`].
//!
//! # Why SES coordinates are integers and DSN coordinates are not
//!
//! The scale factor above is the DSN transform's divided by the board resolution, which puts
//! every SES coordinate in the DSN `resolution` grid rather than in board units. Every
//! coordinate and width on this path then goes through [`java_round_to_int`] (Java's `(int)
//! Math.round`), and shapes are written with `writeScopeInt` rather than `writeScope`. The one
//! exception is [`crate::parser::geometry::DsnShape::write_hole_scope`], which Java reaches from
//! `writeConductionArea` — see the `// Java bug:` marker there.

use std::collections::BTreeSet;
use std::io::{self, Write};

use fr_board::{Board, FixedState, Item, ItemId, Padstack};
use fr_geometry::{Area, FloatPoint, Shape, ShapeOps};

use crate::coordinate_transform::CoordinateTransform;
use crate::format::{
    IdentifierType, IndentFileWriter, SES_RESERVED, format_placement_rotation, java_round_to_int,
};
use crate::parser::geometry::DsnLayer;
use crate::parser::header::{write_parser_scope, write_resolution_scope};

/// `SesWriter.write(BasicBoard, OutputStream, String)` (SesWriter.java:55-67).
///
/// The sink is **flushed** but not closed, exactly as Java documents. `ct` is the DSN transform
/// Java reads off `board.communication` — see the module docs.
///
/// Java's `if (out == null) throw new IOException(…)` guard (SesWriter.java:57-59) has no
/// counterpart: `&mut impl Write` cannot be null.
///
/// # Errors
///
/// Returns the first I/O error any write hit, surfaced by [`IndentFileWriter::flush`]. Java has
/// no equivalent: `IndentFileWriter` swallows every `IOException` into an `FRLogger` call, so a
/// full disk silently truncates the file there.
// renamed: SesWriter.write -> the free function `write` (the class is a private-constructor
// static holder, which Rust spells as a module).
pub fn write<W: Write>(
    board: &Board,
    ct: &CoordinateTransform,
    out: &mut W,
    design_name: &str,
) -> io::Result<()> {
    let mut output_file = IndentFileWriter::new(out);
    // A plain substring replace, not a suffix strip — `a.dsn.b.dsn` becomes `a.ses.b.ses`
    // (SesWriter.java:61).
    let session_name = design_name.replace(".dsn", ".ses");
    let identifier_type = IdentifierType::new(
        SES_RESERVED.iter().map(|s| (*s).to_string()).collect(),
        board.communication.string_quote.clone(),
    );
    write_session_scope(
        board,
        ct,
        &identifier_type,
        &mut output_file,
        &session_name,
        design_name,
    )?;
    output_file.flush()
}

/// `SesWriter.writeSessionScope` (SesWriter.java:73-94).
fn write_session_scope<W: Write>(
    board: &Board,
    ct: &CoordinateTransform,
    identifier_type: &IdentifierType,
    file: &mut IndentFileWriter<W>,
    session_name: &str,
    design_name: &str,
) -> io::Result<()> {
    // SesWriter.java:80-82. See the module docs for why `ct` is a parameter here.
    let scale_factor = ct.dsn_to_board(1.0) / f64::from(board.communication.resolution);
    // The incoming `ct`'s own scale factor is finite and non-zero (`CoordinateTransform::new`
    // refuses anything else — quirk #89), so the only way this quotient is not is a board whose
    // `communication.resolution` is `0`, which no `(resolution …)` scope the reader accepts can
    // produce. Answered rather than unwrapped because the constructor has an error channel and a
    // writer that emitted `Infinity` coordinates is precisely what #89 is about.
    let coordinate_transform =
        CoordinateTransform::new(scale_factor, 0.0, 0.0).map_err(io::Error::other)?;
    file.start_scope(false);
    file.write("session ");
    identifier_type.write(session_name, file);
    file.new_line();
    file.write("(base_design ");
    identifier_type.write(design_name, file);
    file.write(")");
    write_placement(board, identifier_type, &coordinate_transform, file);
    write_was_is(board, identifier_type, file);
    write_routes(board, identifier_type, &coordinate_transform, file);
    file.end_scope();
    Ok(())
}

/// `SesWriter.writePlacement` (SesWriter.java:96-114).
fn write_placement<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("placement");
    write_resolution_scope(file, &board.communication);
    // Java guards the loop with `board.library.packages != null`, on a field this port makes
    // non-nullable (as `Library.writeScope` does).
    for i in 1..=board.library.packages.count() {
        write_components(board, identifier_type, ct, file, i);
    }
    file.end_scope();
}

/// `SesWriter.writeComponents` (SesWriter.java:117-151): "writes all components with the given
/// package to the session file".
///
/// Java takes the `Package` object and compares `currentComponent.getPackage() == pkg` by
/// *identity*; [`fr_board::Component::get_package`] is exactly that package's number, so this
/// port takes the number (the Plan 2 "no object references between model objects" rule).
///
/// Note the guard differs from `Package.writePlacementScope`'s (Package.java:372), which the DSN
/// path uses: this one is `undeletedItemFound` alone, with no `|| !currentComponent.isPlaced()`.
/// An unplaced component with no undeleted items is therefore written by the DSN writer and
/// skipped by this one (a placed one with no undeleted items is skipped by both).
fn write_components<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
    package_no: usize,
) {
    let mut component_found = false;
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    for i in 1..=board.components.count() as i32 {
        let current_component = board.components.get(i);
        if current_component.get_package() != package_no {
            continue;
        }
        // check that not all items of the component are deleted
        let undeleted_item_found = board
            .get_items()
            .any(|item| item.component_id() == current_component.id);
        if !undeleted_item_found {
            continue;
        }
        if !component_found {
            let package_name = board.library.packages.get(package_no).name.clone();
            file.start_scope_nl();
            file.write("component ");
            identifier_type.write(&package_name, file);
            component_found = true;
        }
        write_component(identifier_type, ct, file, current_component);
    }
    if component_found {
        file.end_scope();
    }
}

/// `SesWriter.writeComponent` (SesWriter.java:153-181): one `(place <name> <x> <y> front|back
/// <rotation> [(lock_type position)])` line.
fn write_component<W: Write>(
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
    component: &fr_board::Component,
) {
    file.new_line();
    file.write("(place ");
    identifier_type.write(&component.name, file);
    // totalized: SesWriter.writeComponent dereferences `component.getLocation().toFloat()`
    // (SesWriter.java:163) with no null check, so an unplaced component with undeleted items
    // NPEs there — after the `(place <name>` above has already been written. The port writes
    // the same prefix and then skips the location group, exactly as `Component.writeScope`
    // (the DSN path) does for an unplaced component. Unreachable from `read_board`: every
    // component the `placement` scope creates is placed. See docs/java-quirks.md.
    if let Some(location) = component.get_location() {
        let location = ct.board_to_dsn_point(&location.to_float());
        let xcoordinate = java_round_to_int(location[0]);
        let ycoordinate = java_round_to_int(location[1]);
        file.write(" ");
        file.write(&xcoordinate.to_string());
        file.write(" ");
        file.write(&ycoordinate.to_string());
        if component.placed_on_front() {
            file.write(" front ");
        } else {
            file.write(" back ");
        }
        // The only `String.format` in the Specctra package (SesWriter.java:259-269).
        file.write(&format_placement_rotation(
            component.get_rotation_in_degree(),
        ));
    }
    if component.position_fixed {
        file.new_line();
        file.write(" (lock_type position)");
    }
    file.write(")");
}

/// `SesWriter.writeWasIs` (SesWriter.java:183-218): one `(pins <cmp>-<pin> <cmp>-<pin>)` line per
/// pin that was swapped with another.
///
/// Both of Java's `component not found` branches are `FRLogger.warn` + *keep going*: the
/// half-written `(pins ` line still gets its separating space and its closing bracket. Ported
/// exactly. Java reaches the branch through `board.components.get(id)` returning `null`; this
/// port's `Components::get` panics on an out-of-range id instead (matching Java's own
/// `Vector.elementAt`), so the range check is explicit here.
fn write_was_is<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("was_is");
    for pin_id in board.get_pins() {
        let Some(Item::Pin(current_pin)) = board.get_item(pin_id) else {
            continue;
        };
        let swapped_with = current_pin.get_changed_to();
        // Java compares object identity: `currentPin.getChangedTo() != currentPin`
        // (SesWriter.java:190).
        if swapped_with == pin_id {
            continue;
        }
        file.new_line();
        file.write("(pins ");
        write_swapped_pin(board, identifier_type, file, pin_id);
        file.write(" ");
        write_swapped_pin(board, identifier_type, file, swapped_with);
        file.write(")");
    }
    file.end_scope();
}

/// One `<component>-<packagePin>` half of a `(pins …)` line (SesWriter.java:193-199 and :204-210,
/// which are the same six statements twice).
// renamed: the two inlined halves of SesWriter.writeWasIs -> write_swapped_pin.
fn write_swapped_pin<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    file: &mut IndentFileWriter<W>,
    pin_id: ItemId,
) {
    let Some(item) = board.get_item(pin_id) else {
        return;
    };
    let Item::Pin(pin) = item else {
        return;
    };
    let component_id = item.component_id();
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    if component_id < 1 || component_id > board.components.count() as i32 {
        // "SesWriter.writeWasIs: component not found" — an `FRLogger.warn` this port drops.
        return;
    }
    let component = board.components.get(component_id);
    let component_name = component.name.clone();
    // totalized: SesWriter.writeWasIs dereferences `getPin(...).name` (SesWriter.java:198, :209)
    // with no null check, so a pin index past the package's pin count would NPE. The port writes the component name and
    // the separating `-` and then stops, which is the closest total behaviour. Unreachable: a
    // pin's index always comes from its own package.
    let package_pin_name = board
        .library
        .packages
        .get(component.get_package())
        .get_pin(pin.get_pin_index())
        .map(|p| p.name.clone());
    identifier_type.write(&component_name, file);
    file.write("-");
    if let Some(package_pin_name) = package_pin_name {
        identifier_type.write(&package_pin_name, file);
    }
}

/// `SesWriter.writeRoutes` (SesWriter.java:220-233).
///
/// `Parser.writeScope` is called with `reduced = true` here (SesWriter.java:229) — the SES
/// `(parser …)` scope carries only `host_cad`/`host_version`/`constant`/`write_resolution`, never
/// `(string_quote …)`, `(space_in_quoted_tokens on)` or `(generated_by_freerouting)`.
fn write_routes<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("routes ");
    write_resolution_scope(file, &board.communication);
    write_parser_scope(file, &board.communication, identifier_type, true);
    write_library(board, identifier_type, ct, file);
    write_network(board, identifier_type, ct, file);
    file.end_scope();
}

/// `SesWriter.writeLibrary` (SesWriter.java:235-252): the via padstacks, de-duplicated by name.
///
/// Java's `LinkedHashSet<String>` (SesWriter.java:243) is only ever used through `add`'s boolean
/// return — it is never iterated — so its insertion order is unobservable and a [`BTreeSet`]
/// guard reproduces it exactly. (The brief asks for an insertion-ordered `Vec` alongside; that
/// `Vec` would have no reader.)
fn write_library<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("library_out ");
    let mut written_padstack_names: BTreeSet<String> = BTreeSet::new();
    for i in 0..board.library.via_padstack_count() {
        // `viaPadstack == null` (SesWriter.java:245) — an id the library does not know.
        let Some(via_padstack) = board
            .library
            .get_via_padstack(i)
            .and_then(|id| board.library.get_padstack(id))
        else {
            continue;
        };
        if !written_padstack_names.insert(via_padstack.name.clone()) {
            continue;
        }
        write_padstack(via_padstack, board, identifier_type, ct, file);
    }
    file.end_scope();
}

/// `SesWriter.writePadstack` (SesWriter.java:271-313): one `(padstack <name> (shape …)*
/// [(attach off)])` scope, covering the layer range between the first and last layer the padstack
/// actually has a shape on.
///
/// Unlike `Library.writePadstackScope` (the DSN path) this one writes `writeScopeInt` shapes and
/// never emits `(absolute on)`.
fn write_padstack<W: Write>(
    padstack: &Padstack,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    // determine the layer range covered by this padstack
    let layer_count = board.get_layer_count();
    let mut first_layer_no = 0usize;
    while first_layer_no < layer_count
        && padstack
            .get_shape(i32::try_from(first_layer_no).unwrap_or(i32::MAX))
            .is_none()
    {
        first_layer_no += 1;
    }
    let mut last_layer_no = i64::try_from(layer_count).unwrap_or(i64::MAX) - 1;
    while last_layer_no >= 0
        && padstack
            .get_shape(i32::try_from(last_layer_no).unwrap_or(i32::MAX))
            .is_none()
    {
        last_layer_no -= 1;
    }
    if first_layer_no >= layer_count || last_layer_no < 0 {
        // "SesWriter.writePadstack: padstack shape not found" — the whole scope is skipped.
        return;
    }

    file.start_scope_nl();
    file.write("padstack ");
    let padstack_name = padstack.name.clone();
    identifier_type.write(&padstack_name, file);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    for i in first_layer_no..=(last_layer_no as usize) {
        let Some(current_board_shape) = padstack.get_shape(i32::try_from(i).unwrap_or(i32::MAX))
        else {
            continue;
        };
        let board_layer = &board.layer_structure().layers[i];
        let current_layer = DsnLayer::new(
            board_layer.name.clone(),
            i32::try_from(i).unwrap_or(i32::MAX),
            board_layer.is_signal,
        );
        let current_shape = ct.board_to_dsn_rel_shape(current_board_shape, current_layer);
        file.start_scope_nl();
        file.write("shape");
        // totalized: SesWriter.writePadstack calls `currentShape.writeScopeInt` without checking
        // `boardToDsnRel`'s `null` (SesWriter.java:305); the port writes an empty `(shape …)`
        // scope rather than crashing. Unreachable: every shape `fr-geometry` can hold is one
        // `boardToDsnRel` handles.
        if let Some(current_shape) = current_shape {
            current_shape.write_scope_int(file, identifier_type);
        }
        file.end_scope();
    }
    if !padstack.attach_allowed {
        file.new_line();
        file.write("(attach off)");
    }
    file.end_scope();
}

/// `SesWriter.writeNetwork` (SesWriter.java:315-327): every net number `1..=maxNetNumber()`, in
/// net-number order.
fn write_network<W: Write>(
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("network_out ");
    for i in 1..=board.rules.nets.max_net_number() {
        write_net(i, board, identifier_type, ct, file);
    }
    file.end_scope();
}

/// `SesWriter.writeNet` (SesWriter.java:329-370): the wires, vias and signal-layer conduction
/// areas of one net, in `getConnectableItems` order (descending item id — quirk #63 — which is
/// Java's `TreeSet<Item>` order too).
///
/// The `(net …)` header is written **lazily**, on the first item that is actually going to be
/// written, so a net whose items are all `FixedState::SystemFixed` (or all pins) produces no
/// scope at all — which is why all four reference SES files have an empty `(network_out )`.
fn write_net<W: Write>(
    net_number: i32,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    let ctx = board.ctx();
    let mut header_written = false;
    for item_id in board.get_connectable_items(net_number) {
        let Some(current_item) = board.get_item(item_id) else {
            continue;
        };
        if current_item.get_fixed_state() == FixedState::SystemFixed {
            continue;
        }
        let is_wire = matches!(current_item, Item::Trace(_));
        let is_via = matches!(current_item, Item::Via(_));
        // Java's `&&` short-circuits, so `firstLayer()` is only reached for a conduction area.
        let is_conduction_area = matches!(current_item, Item::ConductionArea(_))
            && board.layer_structure().layers[current_item.first_layer(&ctx)].is_signal;
        if !header_written && (is_wire || is_via || is_conduction_area) {
            file.start_scope_nl();
            file.write("net ");
            match board.rules.nets.get(net_number) {
                // "SesWriter.writeNet: net not found" — an `FRLogger.warn` this port drops.
                // Java still writes the `(net ` header, with no name after it.
                None => {}
                Some(current_net) => {
                    let name = current_net.name.clone();
                    identifier_type.write(&name, file);
                }
            }
            header_written = true;
        }
        if is_wire {
            write_wire(item_id, board, identifier_type, ct, file);
        } else if is_via {
            write_via(item_id, board, identifier_type, ct, file);
        } else if is_conduction_area {
            write_conduction_area(item_id, board, identifier_type, ct, file);
        }
    }
    if header_written {
        file.end_scope();
    }
}

/// `SesWriter.writeWire` (SesWriter.java:372-415): a trace as a `(wire (path …) [(type …)])`
/// scope, with consecutive corners that round to the same integer pair collapsed.
fn write_wire<W: Write>(
    wire_id: ItemId,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    let Some(item) = board.get_item(wire_id) else {
        return;
    };
    let Item::Trace(wire) = item else {
        return;
    };
    let layer_index = wire.get_layer();
    let board_layer_name = board.layer_structure().layers[layer_index].name.clone();
    let wire_width = java_round_to_int(ct.board_to_dsn(f64::from(2 * wire.get_half_width())));

    let corners = wire.polyline().corners();
    let mut coors: Vec<i32> = Vec::with_capacity(2 * corners.len());
    let mut prev_coors: Option<[i32; 2]> = None;
    for (i, corner) in corners.iter().enumerate() {
        let mut corner_point = corner.to_float();
        if (i == 0 || i == corners.len() - 1)
            && let Some(snapped) = snapped_endpoint(board, wire_id, i == 0)
        {
            corner_point = snapped;
        }
        let current_float_coors = ct.board_to_dsn_point(&corner_point);
        let current_coors = [
            java_round_to_int(current_float_coors[0]),
            java_round_to_int(current_float_coors[1]),
        ];
        if prev_coors != Some(current_coors) {
            coors.push(current_coors[0]);
            coors.push(current_coors[1]);
            prev_coors = Some(current_coors);
        }
    }

    file.start_scope_nl();
    file.write("wire");
    write_path(&board_layer_name, wire_width, &coors, identifier_type, file);
    write_fixed_state(file, item.get_fixed_state());
    file.end_scope();
}

/// `SesWriter.snappedEndpoint` (SesWriter.java:426-453): "returns the exact pad/via center to use
/// for a wire endpoint, or null to keep the corner as-is".
///
/// Java's `getStartContacts()`/`getEndContacts()` return a `TreeSet<Item>`, whose iteration order
/// is descending item id (quirk #63); this port's `BTreeSet<ItemId>` is ascending, so the walk is
/// reversed to match. The order decides which contacted drill item wins when a corner touches
/// more than one.
///
/// `board`/`wire_id` replace Java's `PolylineTrace` receiver: the contact sets live on
/// [`Board`] in this port, not on the trace.
// renamed: SesWriter.snappedEndpoint -> snapped_endpoint (a free function taking the board).
// `pub` because Java's is package-visible and `SesRoundTripTest.endpointSnappingIsStableAndRoundTrips`
// calls it directly; `crates/fr-dsn/tests/ses_round_trip.rs` is that test.
pub fn snapped_endpoint(board: &Board, wire_id: ItemId, start_side: bool) -> Option<FloatPoint> {
    let ctx = board.ctx();
    let Some(Item::Trace(wire)) = board.get_item(wire_id) else {
        return None;
    };
    let corner = if start_side {
        wire.first_corner()
    } else {
        wire.last_corner()
    };
    // totalized: SesWriter.snappedEndpoint's `corner.toFloat()` (SesWriter.java:428) NPEs on an
    // empty polyline, which `Polyline`'s invariants make unreachable.
    let corner_float = corner?.to_float();
    let contacts = if start_side {
        board.trace_start_contacts(wire_id)
    } else {
        board.trace_end_contacts(wire_id)
    };
    let layer = wire.get_layer();
    for contact_id in contacts.into_iter().rev() {
        let Some(contact) = board.get_item(contact_id) else {
            continue;
        };
        // `!(contact instanceof DrillItem drill)` (SesWriter.java:432-434).
        let pad_shape = match contact {
            Item::Via(via) => {
                if layer < contact.first_layer(&ctx) || layer > contact.last_layer(&ctx) {
                    continue;
                }
                via.get_shape(layer - contact.first_layer(&ctx), &ctx)
            }
            Item::Pin(pin) => {
                if layer < contact.first_layer(&ctx) || layer > contact.last_layer(&ctx) {
                    continue;
                }
                pin.get_shape(layer - contact.first_layer(&ctx), &ctx)
            }
            _ => continue,
        };
        let Some(pad_shape) = pad_shape else {
            continue;
        };
        let Some(center) = board.drill_center(contact_id) else {
            continue;
        };
        let center = center.to_float();
        let center_distance = corner_float.distance(&center);
        if center_distance <= 0.5 {
            // Already at the center (within rounding) — nothing to fix.
            return None;
        }
        if center_distance <= pad_shape.border_distance(&center) {
            return Some(center);
        }
    }
    None
}

/// `SesWriter.writeVia` (SesWriter.java:455-476): `(via <padstack> <x> <y> [(type …)])`.
fn write_via<W: Write>(
    via_id: ItemId,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    let ctx = board.ctx();
    let Some(item) = board.get_item(via_id) else {
        return;
    };
    let Item::Via(via) = item else {
        return;
    };
    // totalized: SesWriter.writeVia writes `via.getPadstack().name` unconditionally
    // (SesWriter.java:462); a via
    // whose padstack id is not in the library would NPE. Unreachable — every via is inserted with
    // a padstack the library holds.
    let Some(via_padstack) = via.get_padstack(&ctx) else {
        return;
    };
    let via_padstack_name = via_padstack.name.clone();
    let via_location = via.get_center().to_float();
    file.start_scope_nl();
    file.write("via ");
    identifier_type.write(&via_padstack_name, file);
    file.write(" ");
    let location = ct.board_to_dsn_point(&via_location);
    file.write(&java_round_to_int(location[0]).to_string());
    file.write(" ");
    file.write(&java_round_to_int(location[1]).to_string());
    write_fixed_state(file, item.get_fixed_state());
    file.end_scope();
}

/// `SesWriter.writeFixedState` (SesWriter.java:478-490): nothing at all for an item that is at
/// most `SHOVE_FIXED`, else `(type fix)` or `(type protect)`.
///
/// Note this is **not** `Wiring.writeFixedState`: the DSN one emits `(type shove_fixed)` too, and
/// this one does not. `FixedState`'s declaration order is its ordinal order in both languages
/// (`FixedState.java:3`), so `<=` here is Java's `ordinal() <= SHOVE_FIXED.ordinal()`.
///
/// The `fix)` arm is unreachable from [`write_net`], which skips every `SystemFixed` item before
/// it gets here; ported anyway, because Java's branch is there.
fn write_fixed_state<W: Write>(file: &mut IndentFileWriter<W>, fixed_state: FixedState) {
    if fixed_state <= FixedState::ShoveFixed {
        return;
    }
    file.new_line();
    file.write("(type ");
    if fixed_state == FixedState::SystemFixed {
        file.write("fix)");
    } else {
        file.write("protect)");
    }
}

/// `SesWriter.writePath` (SesWriter.java:492-512): `(path <layer> <width> <x0> <y0> …)`, one
/// corner pair per line.
fn write_path<W: Write>(
    layer_name: &str,
    width: i32,
    coors: &[i32],
    identifier_type: &IdentifierType,
    file: &mut IndentFileWriter<W>,
) {
    file.start_scope_nl();
    file.write("path ");
    identifier_type.write(layer_name, file);
    file.write(" ");
    file.write(&width.to_string());
    for pair in coors.chunks_exact(2) {
        file.new_line();
        file.write(&pair[0].to_string());
        file.write(" ");
        file.write(&pair[1].to_string());
    }
    file.end_scope();
}

/// `SesWriter.writeConductionArea` (SesWriter.java:514-551): a signal-layer conduction area as a
/// `(wire <shape> (window …)*)` scope. Unlike the DSN path's
/// [`crate::parser::wiring`] counterpart it carries neither the net nor the clearance class.
//
// Java bug: SesWriter.writeConductionArea sends the boundary out through `writeScopeInt`
// (SesWriter.java:544) but each hole goes
// through `writeHoleScope` (:548), which calls plain `writeScope` — so a conduction area with
// holes emits integer boundary coordinates and `Double.toString` hole coordinates in the same
// SES scope. Reproduced, not fixed. See docs/java-quirks.md.
fn write_conduction_area<W: Write>(
    conduction_id: ItemId,
    board: &Board,
    identifier_type: &IdentifierType,
    ct: &CoordinateTransform,
    file: &mut IndentFileWriter<W>,
) {
    let ctx = board.ctx();
    let Some(item) = board.get_item(conduction_id) else {
        return;
    };
    let Item::ConductionArea(conduction_area) = item else {
        return;
    };
    if item.net_count() != 1 {
        // "SesWriter.writeConductionArea: unexpected net count" — an `FRLogger.warn` this port
        // drops.
        return;
    }
    let current_area = conduction_area.get_area(&ctx);
    let layer_index = conduction_area.get_layer();
    let board_layer = &board.layer_structure().layers[layer_index];
    let conduction_layer = DsnLayer::new(
        board_layer.name.clone(),
        i32::try_from(layer_index).unwrap_or(i32::MAX),
        board_layer.is_signal,
    );
    let (boundary_shape, holes): (Shape, Vec<Shape>) = match current_area {
        Area::Shape(s) => (s.clone(), Vec::new()),
        area => (area.get_border(), area.get_holes()),
    };
    file.start_scope_nl();
    file.write("wire ");
    if let Some(dsn_shape) = ct.board_to_dsn_shape(&boundary_shape, conduction_layer.clone()) {
        dsn_shape.write_scope_int(file, identifier_type);
    }
    for hole in &holes {
        // totalized: SesWriter.writeConductionArea calls `dsnHole.writeHoleScope` without checking
        // `boardToDsn`'s `null`
        // (SesWriter.java:547-548); the port skips the hole rather than crashing. Unreachable,
        // for the same reason as in `write_padstack`.
        if let Some(dsn_hole) = ct.board_to_dsn_shape(hole, conduction_layer.clone()) {
            dsn_hole.write_hole_scope(file, identifier_type);
        }
    }
    file.end_scope();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The session name is a plain substring replace, not a suffix strip (SesWriter.java:61).
    #[test]
    fn session_name_replaces_every_dsn_occurrence() {
        assert_eq!("board.ses", "board.dsn".replace(".dsn", ".ses"));
        assert_eq!("a.ses.b.ses", "a.dsn.b.dsn".replace(".dsn", ".ses"));
        // No `.dsn` at all: the design name is used verbatim, which is what
        // `scripts/gen-reference/RefWriter.java` produces (it strips the suffix first).
        assert_eq!("tutorial_board", "tutorial_board".replace(".dsn", ".ses"));
    }

    /// `SesWriter.writeFixedState` writes nothing below `USER_FIXED` and never `shove_fixed`.
    #[test]
    fn fixed_state_is_written_only_above_shove_fixed() {
        fn rendered(state: FixedState) -> String {
            let mut out: Vec<u8> = Vec::new();
            let mut file = IndentFileWriter::new(&mut out);
            write_fixed_state(&mut file, state);
            file.flush().expect("Vec never fails");
            String::from_utf8(out).expect("UTF-8")
        }
        assert_eq!(rendered(FixedState::Unfixed), "");
        assert_eq!(rendered(FixedState::ShoveFixed), "");
        assert_eq!(rendered(FixedState::UserFixed), "\n(type protect)");
        assert_eq!(rendered(FixedState::SystemFixed), "\n(type fix)");
    }
}
