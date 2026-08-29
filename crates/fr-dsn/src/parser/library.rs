//! `io/specctra/parser/Library.java` and `io/specctra/parser/Package.java` — the `library` scope
//! (`padstack` and `image` definitions) and the DSN-side `Package`/`PinInfo` records it builds
//! before handing them to `fr_board::{Padstacks, Packages}`.
//!
//! The two Java classes share one Rust module because `Package` is not a scope of its own:
//! `Library.readScope` is the only caller of `Package.readScope`, and `Library.writeScope` the
//! only caller of `Package.writeScope`. Java's `Package` (the DSN record) and
//! `core.library.Package` (the board record) are two different types with the same simple name;
//! the port keeps the board one as `fr_board::Package` and renames the DSN one.
// renamed: Package -> DsnPackage, Package.PinInfo -> DsnPinInfo (the plain names belong to
// `fr_board`'s board-side records, which this module also uses).

use fr_board::{Keepout, PackagePin, Padstack, PadstackId, Padstacks, equals_ignore_case};
use fr_geometry::{Area, Circle, IntVector, Shape, ShapeOps, TileShape, Vector};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::{java_double_to_string, java_round_to_int};
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::read_on_off_scope;
use crate::parser::geometry::{
    self as shape, DsnLayer, DsnLayerStructure, DsnShape, ReadAreaScopeResult,
};
use crate::parser::placement::write_component_scope;
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};

// ------------------------------------------------------------------------ Package.java

/// `Package.PinInfo` (Package.java:404-424): one `(pin …)` entry of an `image` scope, before its
/// coordinates are scaled to board units.
// renamed: Package.PinInfo -> DsnPinInfo.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnPinInfo {
    /// `PinInfo.padstackName`.
    pub padstack_name: String,
    /// `PinInfo.pinName`.
    pub pin_name: String,
    /// `PinInfo.relCoor`, Java's `double[2]`.
    pub rel_coor: [f64; 2],
    /// `PinInfo.rotation`.
    pub rotation: f64,
}

/// `io/specctra/parser/Package.java`: one `(image …)` scope, as read.
// renamed: Package -> DsnPackage.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnPackage {
    /// `Package.name`.
    pub name: String,
    /// `Package.pinInfoArr`.
    pub pin_info_arr: Vec<DsnPinInfo>,
    /// `Package.outline`.
    pub outline: Vec<DsnShape>,
    /// `Package.keepouts`.
    pub keepouts: Vec<ReadAreaScopeResult>,
    /// `Package.viaKeepouts`.
    pub via_keepouts: Vec<ReadAreaScopeResult>,
    /// `Package.placeKeepouts`.
    pub place_keepouts: Vec<ReadAreaScopeResult>,
    /// `Package.isFront` — false when the package sits on the back of the board.
    pub is_front: bool,
}

/// `Package.readScope` (Package.java:57-150): one `(image <name> …)` scope.
///
/// `None` is Java's `null` return — every one of its `FRLogger.warn`/`error` calls is dropped
/// (`Package.readScope` has no `ReadScopeParameter` to push a warning onto).
// renamed: Package.readScope -> read_image_scope (the scope's keyword is `image`, and this
// module already has a `read_library_scope`/`read_padstack_scope` pair named after their
// keywords).
pub fn read_image_scope(
    scanner: &mut DsnScanner,
    layer_structure: Option<&DsnLayerStructure>,
) -> Result<Option<DsnPackage>, DsnError> {
    let mut is_front = true;
    let mut outline: Vec<DsnShape> = Vec::new();
    let mut keepouts: Vec<ReadAreaScopeResult> = Vec::new();
    let mut via_keepouts: Vec<ReadAreaScopeResult> = Vec::new();
    let mut place_keepouts: Vec<ReadAreaScopeResult> = Vec::new();
    let mut next_token = scanner.next_token()?;
    let Some(Token::Str(package_name)) = next_token.clone() else {
        // "Package.read_scope: String expected".
        return Ok(None);
    };
    scanner.set_scope_identifier(&package_name);
    let mut pin_info_list: Vec<DsnPinInfo> = Vec::new();
    loop {
        let prev_token = next_token;
        next_token = scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // "Package.read_scope: unexpected end of file".
            return Ok(None);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        if prev_token == Some(Token::Open) {
            match token {
                Token::Kw(Keyword::Pin) => {
                    let Some(next_pin) = read_pin_info(scanner)? else {
                        return Ok(None);
                    };
                    pin_info_list.push(next_pin);
                }
                Token::Kw(Keyword::Side) => is_front = read_placement_side(scanner)?,
                Token::Kw(Keyword::Outline) => {
                    if let Some(current_shape) = shape::read_scope(scanner, layer_structure)? {
                        outline.push(current_shape);
                    }
                    // overread closing bracket
                    next_token = scanner.next_token()?;
                    if next_token != Some(Token::Close) {
                        // "Package.read_scope: closed bracket expected".
                        return Ok(None);
                    }
                }
                Token::Kw(Keyword::Keepout) => {
                    if let Some(keepout_area) =
                        shape::read_area_scope(scanner, layer_structure, false)?
                    {
                        keepouts.push(keepout_area);
                    }
                }
                Token::Kw(Keyword::ViaKeepout) => {
                    if let Some(keepout_area) =
                        shape::read_area_scope(scanner, layer_structure, false)?
                    {
                        via_keepouts.push(keepout_area);
                    }
                }
                Token::Kw(Keyword::PlaceKeepout) => {
                    if let Some(keepout_area) =
                        shape::read_area_scope(scanner, layer_structure, false)?
                    {
                        place_keepouts.push(keepout_area);
                    }
                }
                _ => {
                    // Java discards `skipScope`'s boolean (Package.java:135).
                    skip_scope(scanner)?;
                }
            }
        }
    }
    Ok(Some(DsnPackage {
        name: package_name,
        pin_info_arr: pin_info_list,
        outline,
        keepouts,
        via_keepouts,
        place_keepouts,
        is_front,
    }))
}

/// `Package.readPinInfo` (Package.java:249-330): one `(pin <padstack> [(rotate …)] <name> <x>
/// <y> …)` entry.
///
/// The two `yybegin(NAME)` calls before the padstack and pin names are load-bearing: without
/// them a name starting with a digit (`1`, `2A`) does not lex as a single string.
fn read_pin_info(scanner: &mut DsnScanner) -> Result<Option<DsnPinInfo>, DsnError> {
    // Read the padstack name.
    scanner.yybegin(LexicalState::Name);
    let mut next_token = scanner.next_token()?;
    let padstack_name = match &next_token {
        Some(Token::Str(s)) => s.clone(),
        // `nextToken.toString()` on an `Integer` (Package.java:261).
        Some(Token::Int(i)) => i.to_string(),
        // "Package.read_pin_info: String or Integer expected".
        _ => return Ok(None),
    };
    let mut rotation = 0.0;

    // to be able to handle pin names starting with a digit.
    scanner.yybegin(LexicalState::Name);
    next_token = scanner.next_token()?;
    if next_token == Some(Token::Open) {
        // read the padstack rotation
        next_token = scanner.next_token()?;
        if next_token == Some(Token::Kw(Keyword::Rotate)) {
            rotation = read_rotation(scanner)?;
        } else {
            skip_scope(scanner)?;
        }
        scanner.yybegin(LexicalState::Name);
        next_token = scanner.next_token()?;
    }
    // Read the pin name.
    let pin_name = match &next_token {
        Some(Token::Str(s)) => s.clone(),
        Some(Token::Int(i)) => i.to_string(),
        // "Package.read_pin_info: String or Integer expected".
        _ => return Ok(None),
    };

    let mut pin_coor = [0.0f64; 2];
    for coordinate in &mut pin_coor {
        next_token = scanner.next_token()?;
        match next_token {
            Some(Token::Float(f)) => *coordinate = f,
            #[allow(clippy::cast_precision_loss)]
            Some(Token::Int(i)) => *coordinate = i as f64,
            // "Package.read_pin_info: number expected".
            _ => return Ok(None),
        }
    }
    // Handle scopes at the end of the pin scope.
    loop {
        let prev_token = next_token;
        next_token = scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // "Package.read_pin_info: unexpected end of file".
            return Ok(None);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        if prev_token == Some(Token::Open) {
            if token == Token::Kw(Keyword::Rotate) {
                rotation = read_rotation(scanner)?;
            } else {
                skip_scope(scanner)?;
            }
        }
    }
    Ok(Some(DsnPinInfo {
        padstack_name,
        pin_name,
        rel_coor: pin_coor,
        rotation,
    }))
}

/// `Package.readRotation` (Package.java:332-352): the body of a `(rotate <number>)` scope.
///
/// Java uses `Double.parseDouble` here, not the scanner's `NumberFormat` — the difference shows
/// on a locale-decimal-comma input, which `parseDouble` rejects and `NumberFormat` accepts.
// totalized: Package.readRotation — Java lets `Double.parseDouble`'s `NumberFormatException`
// escape uncaught (only `IOException` is caught, Package.java:347), aborting the entire DSN
// read; the port answers `0.0`, the same value the method returns for every other failure it
// does handle. `f64::from_str` accepts a slightly different grammar than `Double.parseDouble`
// (`inf` vs `Infinity`, no trailing `d`/`f` suffix, no hex literals); no fixture in the corpus
// writes a rotation in any of those forms.
fn read_rotation(scanner: &mut DsnScanner) -> Result<f64, DsnError> {
    let next_string = scanner.next_string();
    let result = next_string.trim().parse::<f64>().unwrap_or(0.0);

    // Overread The closing bracket.
    // "Package.read_rotation: closing bracket expected" — Java warns and carries on.
    let _ = scanner.next_token()?;

    Ok(result)
}

/// `Package.readPlacementSide` (Package.java:389-401): `(side front|back)`; anything that is not
/// the `back` keyword counts as front.
fn read_placement_side(scanner: &mut DsnScanner) -> Result<bool, DsnError> {
    let next_token = scanner.next_token()?;
    let result = next_token != Some(Token::Kw(Keyword::Back));

    // "Package.read_placement_side: closing bracket expected" — Java warns and carries on.
    let _ = scanner.next_token()?;
    Ok(result)
}

/// `Package.writeScope` (Package.java:152-207): one `(image …)` scope.
// renamed: Package.writeScope -> write_package_scope.
pub fn write_package_scope(p: &mut WriteScopeParameter<'_>, board_package: &fr_board::Package) {
    p.file.start_scope_nl();
    p.file.write("image ");
    p.identifier_type.write(&board_package.name, &mut p.file);
    // write the placement side of the package
    p.file.new_line();
    p.file.write("(side ");
    if board_package.is_front {
        p.file.write("front)");
    } else {
        p.file.write("back)");
    }
    // write the pins of the package
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    for i in 0..board_package.pin_count() as i32 {
        let Some(current_pin) = board_package.get_pin(i) else {
            continue;
        };
        p.file.new_line();
        p.file.write("(pin ");
        // Java dereferences `padstacks.get(padstackId)` unchecked (Package.java:171-172).
        // totalized: Package.writeScope — a pin whose padstack id is out of range NPEs in Java;
        // the port writes an empty identifier instead. Unreachable from a DSN read, where every
        // pin's padstack id came out of the same `Padstacks` this looks it up in.
        let padstack_name = p
            .board
            .library
            .padstacks
            .get(current_pin.padstack_no)
            .map_or(String::new(), |padstack| padstack.name.clone());
        p.identifier_type.write(&padstack_name, &mut p.file);
        p.file.write(" ");
        let pin_name = current_pin.name.clone();
        p.identifier_type.write(&pin_name, &mut p.file);
        let rel_coor = p
            .coordinate_transform
            .board_to_dsn_vector(&current_pin.relative_location);
        for coordinate in rel_coor {
            p.file.write(" ");
            p.file.write(&java_double_to_string(coordinate));
        }
        let rotation = java_round_to_int(current_pin.rotation_in_degree);
        if rotation != 0 {
            p.file.write("(rotate ");
            p.file.write(&rotation.to_string());
            p.file.write(")");
        }
        p.file.write(")");
    }
    // write the keepouts belonging to the package.
    for keepout in &board_package.keepouts {
        write_package_keepout(keepout, p, false);
    }
    for keepout in &board_package.via_keepouts {
        write_package_keepout(keepout, p, true);
    }
    // write the package outline.
    if let Some(outline) = board_package.outline.as_ref() {
        for board_shape in outline {
            p.file.start_scope_nl();
            p.file.write("outline");
            // totalized: Package.writeScope — Java calls `currentOutline.writeScope(...)` on
            // `boardToDsnRel`'s result with no null check (Package.java:200-202), one of the few
            // sites in the package that does not. The port writes an empty `(outline)` scope
            // instead of NPEing. Unreachable: `board_to_dsn_rel_shape` never answers `None` for
            // a shape `fr-board` holds.
            if let Some(current_outline) = p
                .coordinate_transform
                .board_to_dsn_rel_shape(board_shape, DsnLayer::signal())
            {
                current_outline.write_scope(&mut p.file, &p.identifier_type);
            }
            p.file.end_scope();
        }
    }
    p.file.end_scope();
}

/// `Package.writePackageKeepout` (Package.java:209-246).
fn write_package_keepout(keepout: &Keepout, p: &mut WriteScopeParameter<'_>, is_via_keepout: bool) {
    let keepout_layer = if keepout.layer >= 0 {
        #[allow(clippy::cast_sign_loss)]
        let board_layer = &p.board.layer_structure().layers[keepout.layer as usize];
        DsnLayer::new(
            board_layer.name.clone(),
            keepout.layer,
            board_layer.is_signal,
        )
    } else {
        DsnLayer::signal()
    };
    let (boundary_shape, holes) = match &keepout.area {
        Area::Shape(s) => (s.clone(), Vec::new()),
        area => (area.get_border(), area.get_holes()),
    };
    p.file.start_scope_nl();
    if is_via_keepout {
        p.file.write("via_keepout");
    } else {
        p.file.write("keepout");
    }
    if let Some(dsn_shape) = p
        .coordinate_transform
        .board_to_dsn_shape(&boundary_shape, keepout_layer.clone())
    {
        dsn_shape.write_scope(&mut p.file, &p.identifier_type);
    }
    for hole in &holes {
        // totalized: Package.writePackageKeepout — Java dereferences `boardToDsn`'s result for a
        // hole without the `null` check it applies to the border one line above
        // (Package.java:242-243); the port skips a hole it cannot transform. Only a
        // `PolylineShape`-free, non-`IntBox`, non-`Circle` shape reaches that branch, and
        // nothing in `fr-geometry` produces one.
        if let Some(dsn_hole) = p
            .coordinate_transform
            .board_to_dsn_shape(hole, keepout_layer.clone())
        {
            dsn_hole.write_hole_scope(&mut p.file, &p.identifier_type);
        }
    }
    p.file.end_scope();
}

/// `Package.writePlacementScope` (Package.java:355-387): the `(component <image> (place …)*)`
/// scope for one library package — written only if at least one component uses that package
/// *and* survives the "not all items of the component are deleted" test.
///
/// The scope header is emitted lazily, on the first component that passes, so a package with no
/// live components produces nothing at all (Java's `componentFound` flag).
///
/// Java takes the `Package` object; this port takes its `Package.no`, because
/// `currentComponent.getPackage() == boardPackage` is an *identity* comparison in Java and
/// `Component::get_package` returns exactly that number here (the Plan 2 "no object references
/// between model objects" rule).
// renamed: Package.writePlacementScope -> write_component_placement_scope (it writes a
// `component` scope, not a `package`/`image` one, and the name has to survive next to
// `write_package_scope` above).
pub fn write_component_placement_scope(p: &mut WriteScopeParameter<'_>, package_no: usize) {
    let board = p.board;
    let mut component_found = false;
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    for i in 1..=board.components.count() as i32 {
        let current_component = board.components.get(i);
        if current_component.get_package() != package_no {
            continue;
        }
        // check, if not all items of the component are deleted
        let undeleted_item_found = board
            .get_items()
            .any(|item| item.component_id() == current_component.id);
        // Java: `if (undeletedItemFound || !currentComponent.isPlaced())` (Package.java:372).
        if !undeleted_item_found && current_component.is_placed() {
            continue;
        }
        if !component_found {
            // write the scope header
            let package_name = board.library.packages.get(package_no).name.clone();
            p.file.start_scope_nl();
            p.file.write("component ");
            p.identifier_type.write(&package_name, &mut p.file);
            component_found = true;
        }
        write_component_scope(p, current_component);
    }
    if component_found {
        p.file.end_scope();
    }
}

// ------------------------------------------------------------------------ Library.java

/// `Library.writeScope` (Library.java:35-52): every package, then every padstack.
// renamed: Library.writeScope -> write_library_scope.
pub fn write_library_scope(p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("library");

    let board = p.board;
    // Java guards both loops with a `!= null` check on a field this port makes non-nullable.
    for i in 1..=board.library.packages.count() {
        let board_package = board.library.packages.get(i);
        write_package_scope(p, board_package);
    }

    for i in 1..=board.library.padstacks.count() {
        // totalized: Library.writeScope — Java hands `padstacks.get(i)` straight to
        // `writePadstackScope` (Library.java:46-47), and `Padstacks.get(int)` warns and returns
        // `null` for an index outside `1..=count`, which `writePadstackScope` then NPEs on. The
        // port skips the entry. Unreachable: the loop bounds are the collection's own count.
        let Some(padstack) = board.library.padstacks.get(PadstackId(i)) else {
            continue;
        };
        write_padstack_scope(p, padstack);
    }

    p.file.end_scope();
}

/// `Library.writePadstackScope` (Library.java:54-99): one `(padstack …)` scope, covering the
/// layer range between the first and last layer the padstack actually has a shape on.
// renamed: Library.writePadstackScope -> write_padstack_scope.
pub fn write_padstack_scope(p: &mut WriteScopeParameter<'_>, padstack: &Padstack) {
    // search the layer range of the padstack
    let layer_count = p.board.get_layer_count();
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
        // "Library.write_padstack_scope: padstack shape not found" — the whole scope is skipped.
        return;
    }

    p.file.start_scope_nl();
    p.file.write("padstack ");
    let padstack_name = padstack.name.clone();
    p.identifier_type.write(&padstack_name, &mut p.file);
    #[allow(clippy::cast_possible_truncation)]
    for i in first_layer_no..=(last_layer_no as usize) {
        let Some(current_board_shape) = padstack.get_shape(i32::try_from(i).unwrap_or(i32::MAX))
        else {
            continue;
        };
        let board_layer = &p.board.layer_structure().layers[i];
        let current_layer = DsnLayer::new(
            board_layer.name.clone(),
            i32::try_from(i).unwrap_or(i32::MAX),
            board_layer.is_signal,
        );
        let current_shape = p
            .coordinate_transform
            .board_to_dsn_rel_shape(current_board_shape, current_layer);
        p.file.start_scope_nl();
        p.file.write("shape");
        // totalized: Library.writePadstackScope — Java calls `currentShape.writeScope` without
        // checking `boardToDsnRel`'s `null` (Library.java:84-87); the port writes an empty
        // `(shape …)` scope rather than crashing. Unreachable: every shape `fr-geometry` can
        // hold is one `boardToDsnRel` handles.
        if let Some(current_shape) = current_shape {
            current_shape.write_scope(&mut p.file, &p.identifier_type);
        }
        p.file.end_scope();
    }
    if !padstack.attach_allowed {
        p.file.new_line();
        p.file.write("(attach off)");
    }
    if padstack.placed_absolute {
        p.file.new_line();
        p.file.write("(absolute on)");
    }
    p.file.end_scope();
}

/// `Library.readPadstackScope` (Library.java:101-224): one `(padstack <name> (shape …)* [(attach
/// on|off)] [(absolute on|off)])` scope, inserted straight into `board_padstacks`.
///
/// Every `FRLogger.warn` here is dropped (this method has no `ReadScopeParameter`).
pub fn read_padstack_scope(
    scanner: &mut DsnScanner,
    layer_structure: &DsnLayerStructure,
    coordinate_transform: &CoordinateTransform,
    board_padstacks: &mut Padstacks,
) -> Result<bool, DsnError> {
    let mut is_drilllable = true;
    let mut placed_absolute = false;
    let mut shape_list: Vec<DsnShape> = Vec::new();
    let mut next_token = scanner.next_token()?;
    let Some(Token::Str(raw_name)) = next_token.clone() else {
        // "Library.read_padstack_scope: unexpected padstack identifier".
        return Ok(false);
    };
    let padstack_name = strip_dot_digits(&raw_name);
    scanner.set_scope_identifier(&padstack_name);

    while next_token != Some(Token::Close) {
        let prev_token = next_token;
        next_token = scanner.next_token()?;
        if next_token.is_none() {
            // totalized: Library.readPadstackScope — Java's `while (nextToken !=
            // Keyword.CLOSED_BRACKET)` never terminates once `nextToken` is `null` (end of
            // file): neither branch inside the loop consumes anything further, so the reader
            // spins forever. The port stops and reports failure, the same answer Java gives for
            // every other malformed padstack scope. Same shape as `Structure.readBoundaryScope`.
            return Ok(false);
        }
        if prev_token == Some(Token::Open) {
            match next_token {
                Some(Token::Kw(Keyword::Shape)) => {
                    if let Some(current_shape) = shape::read_scope(scanner, Some(layer_structure))?
                    {
                        shape_list.push(current_shape);
                    }
                    // overread the closing bracket and unknown scopes.
                    let mut current_next_token = scanner.next_token()?;
                    while current_next_token == Some(Token::Open) {
                        skip_scope(scanner)?;
                        current_next_token = scanner.next_token()?;
                    }
                    if current_next_token != Some(Token::Close) {
                        // "Library.read_padstack_scope: closing bracket expected".
                        return Ok(false);
                    }
                    // Java leaves `nextToken` at the shape keyword here, so the outer loop's
                    // `prevToken` for the next round is that keyword, not the bracket it just
                    // over-read — reproduced by not reassigning `next_token`.
                }
                Some(Token::Kw(Keyword::Attach)) => is_drilllable = read_on_off_scope(scanner)?,
                Some(Token::Kw(Keyword::Absolute)) => placed_absolute = read_on_off_scope(scanner)?,
                _ => {
                    skip_scope(scanner)?;
                }
            }
        }
    }

    if board_padstacks.get_by_name(&padstack_name).is_some() {
        // Padstack exists already
        return Ok(true);
    }
    if shape_list.is_empty() {
        // "Library.read_padstack_scope: shape not found for padstack".
        return Ok(true);
    }
    let mut padstack_shapes: Vec<Option<Shape>> = vec![None; layer_structure.layers.len()];
    for pad_shape in &shape_list {
        // totalized: Library.readPadstackScope — `transformToBoardRel` is `null` for a
        // `PolylinePath` (Shape.java's `PolylinePath.transformToBoardRel` warns and returns
        // `null`), and Java then calls `splitToConvex()` on it, which NPEs and aborts the read.
        // The port treats it as "no copper on this layer", which is what the surrounding array
        // already means.
        let mut padstack_shape = pad_shape
            .transform_to_board_rel(coordinate_transform)
            .and_then(to_convex_shape);
        if let Some(shape) = &padstack_shape
            && shape.dimension() < 2
        {
            // "the shape of padstack '…' is not an area. We will enlarge it as a workaround"
            // — enlarge the shape a little bit, so that it is an area.
            let enlarged = offset_convex(shape, 1.0);
            padstack_shape = if enlarged.dimension() < 2 {
                None
            } else {
                Some(enlarged)
            };
        }

        let layer = pad_shape.layer();
        if *layer == DsnLayer::pcb() || *layer == DsnLayer::signal() {
            padstack_shapes.fill(padstack_shape);
        } else {
            let Some(shape_layer) = layer_structure.get_no(&layer.name) else {
                // "Library.read_padstack_scope: layer number found".
                return Ok(false);
            };
            if shape_layer >= padstack_shapes.len() {
                return Ok(false);
            }
            padstack_shapes[shape_layer] = padstack_shape;
        }
    }
    board_padstacks.add(
        padstack_name,
        padstack_shapes,
        is_drilllable,
        placed_absolute,
    );
    Ok(true)
}

/// Java's `ConvexShape` narrowing at Library.java:171-191: a `TileShape` or a `Circle` is
/// already convex; a `PolygonShape` is replaced by the single convex piece of its convex hull.
///
/// `None` where Java would have thrown — see the `totalized:` markers on the two call sites.
// totalized: Library.readPadstackScope — Java indexes `convexShapes[0]` with no length check
// (Library.java:187) right after warning that the length is not 1, so an empty split throws
// `ArrayIndexOutOfBoundsException`; the port drops the shape instead.
fn to_convex_shape(shape: Shape) -> Option<Shape> {
    match shape {
        Shape::Tile(_) | Shape::Circle(_) => Some(shape),
        Shape::Polygon(polygon) => {
            let hull = Shape::Polygon(polygon.convex_hull());
            // "Library.read_padstack_scope: convex shape expected" when the split is not a
            // single piece — a warning only; Java takes the first piece regardless.
            let convex_shapes = hull.split_to_convex()?;
            let first = convex_shapes.first()?;
            Some(Shape::Tile(match first {
                // `if (convexShape instanceof Simplex simplex) convexShape = simplex.simplify();`
                TileShape::Simplex(simplex) => simplex.simplify(),
                other => other.clone(),
            }))
        }
    }
}

/// `ConvexShape.offset(double)` over the two shape kinds that reach it (`TileShape` and
/// `Circle`), Java's covariant overloads collapsed into one helper.
fn offset_convex(shape: &Shape, distance: f64) -> Shape {
    match shape {
        Shape::Tile(tile) => Shape::Tile(tile.offset(distance)),
        Shape::Circle(circle) => Shape::Circle(Circle::offset(circle, distance)),
        // `to_convex_shape` never returns a `PolygonShape`.
        Shape::Polygon(_) => shape.clone(),
    }
}

/// `Library.arePackagePinsIdentical` (Library.java:226-259): the deduplication predicate for a
/// repeated `image` name.
///
/// Java's leading `pkg1 == null || p2 == null` branch is dropped — neither argument is nullable
/// at the single call site (Library.java:430).
fn are_package_pins_identical(pkg1: &fr_board::Package, p2: &[PackagePin]) -> bool {
    if pkg1.pin_count() != p2.len() {
        return false;
    }
    for (i, pin2) in p2.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let Some(pin1) = pkg1.get_pin(i as i32) else {
            // Java: `pin1 == null` while `pin2 != null` -> not identical.
            return false;
        };
        if pin1.name != pin2.name {
            return false;
        }
        if pin1.padstack_no != pin2.padstack_no {
            return false;
        }
        let loc1 = pin1.relative_location.to_float();
        let loc2 = pin2.relative_location.to_float();
        if (loc1.x - loc2.x).abs() > 0.001 || (loc1.y - loc2.y).abs() > 0.001 {
            return false;
        }
        if (pin1.rotation_in_degree - pin2.rotation_in_degree).abs() > 0.001 {
            return false;
        }
    }
    true
}

/// `Library.readScope` (Library.java:261-452): the `library` scope — `padstack` and `image`
/// entries, then the whole `image` list turned into board packages in one pass at the end.
///
/// **Pin coordinates are rounded here** (`java_round_to_int`, Library.java:317,319), not at
/// insertion time.
// renamed: Library.readScope -> read_library_scope.
pub fn read_library_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    // totalized: Library.readScope — Java dereferences `scopeParameter.layerStructure` and
    // `.coordinateTransform` (both `null` until `Structure.createBoard` has run) and
    // `boardHandling.getRoutingBoard()` unchecked; a `library` scope that precedes `structure`
    // therefore NPEs out of the whole read. The port reports failure instead, which is the same
    // "this file did not read" outcome without the crash.
    let (Some(layer_structure), Some(coordinate_transform)) =
        (p.layer_structure.clone(), p.coordinate_transform)
    else {
        return Ok(false);
    };
    if p.board.is_none() {
        return Ok(false);
    }
    {
        let board_layer_structure = p
            .board
            .as_ref()
            .expect("checked above")
            .layer_structure()
            .clone();
        let board = p.board.as_mut().expect("checked above");
        board.library.padstacks = Padstacks::new(board_layer_structure);
    }
    let mut package_list: Vec<DsnPackage> = Vec::new();
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // "Library.read_scope: unexpected end of file".
            return Ok(false);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        if prev_token == Some(Token::Open) {
            match token {
                Token::Kw(Keyword::Padstack) => {
                    let board = p.board.as_mut().expect("checked above");
                    if !read_padstack_scope(
                        &mut p.scanner,
                        &layer_structure,
                        &coordinate_transform,
                        &mut board.library.padstacks,
                    )? {
                        return Ok(false);
                    }
                }
                Token::Kw(Keyword::Image) => {
                    let Some(current_package) =
                        read_image_scope(&mut p.scanner, Some(&layer_structure))?
                    else {
                        return Ok(false);
                    };
                    package_list.push(current_package);
                }
                _ => {
                    skip_scope(&mut p.scanner)?;
                }
            }
        }
    }

    // Create the library packages on the board
    let board = p.board.as_mut().expect("checked above");
    board.library.packages = fr_board::Packages::new();
    for mut current_package in package_list {
        let mut pins: Vec<PackagePin> = Vec::with_capacity(current_package.pin_info_arr.len());
        for pin_info in &current_package.pin_info_arr {
            let rel_x = java_round_to_int(coordinate_transform.dsn_to_board(pin_info.rel_coor[0]));
            let rel_y = java_round_to_int(coordinate_transform.dsn_to_board(pin_info.rel_coor[1]));
            // `new IntVector(relX, relY)` (Library.java:320) — a direct `IntVector`, never
            // `Vector.getInstance`'s promotion to a `RationalVector`.
            let rel_coor = Vector::Int(IntVector::new(rel_x, rel_y));
            let cleaned_lookup_name = strip_dot_digits(&pin_info.padstack_name);
            let Some(board_padstack) = board.library.padstacks.get_by_name(&cleaned_lookup_name)
            else {
                // "Library.read_scope: board padstack '…' not found".
                return Ok(false);
            };
            pins.push(PackagePin::new(
                pin_info.pin_name.clone(),
                PadstackId(board_padstack.no),
                rel_coor,
                pin_info.rotation,
            ));
        }
        let mut outlines: Vec<Shape> = Vec::with_capacity(current_package.outline.len());
        let mut outline_widths: Vec<f64> = Vec::with_capacity(current_package.outline.len());
        let mut outline_is_closed: Vec<bool> = Vec::with_capacity(current_package.outline.len());

        for current_shape in &current_package.outline {
            // totalized: Library.readScope — `transformToBoardRel` is `null` for a
            // `PolylinePath` outline, and Java stores that `null` in the package's `Shape[]`,
            // where `Package.writeScope` later NPEs on it (Package.java:200-202). `fr-board`'s
            // outline array holds no `Option`, so the port drops the entry — together with its
            // width and closed flag, keeping the three arrays in step.
            let Some(board_shape) = current_shape.transform_to_board_rel(&coordinate_transform)
            else {
                continue;
            };
            outlines.push(board_shape);
            // Java's `instanceof Path` (Library.java:349) matches **both** subclasses —
            // `PolygonPath` and `PolylinePath` share `Path` as their base, and both carry
            // `width`/`coordinateArr` — so both arms below are the faithful transcription.
            //
            // The `PolylinePath` arm is nonetheless **unreachable from here**, in this port and
            // in Java alike: `PolylinePath.transformToBoardRel` is an unconditional
            // `return null` (PolylinePath.java:56-60), so the `continue` five lines above always
            // fires first. In Java the same shape reaches `outlineWidths[i]` only because the
            // `null` is *stored* rather than skipped, and `Package.writeScope` then NPEs on it
            // (the `totalized:` note above). The arm is kept rather than replaced by `_ => None`
            // so that the day `transformToBoardRel` is implemented, this branch is already right
            // instead of silently classifying a polyline path as a closed non-path shape.
            let path = match current_shape {
                DsnShape::Path(path) => Some((path.width, path.coordinate_arr.as_slice())),
                DsnShape::PolylinePath(path) => Some((path.width, path.coordinate_arr.as_slice())),
                _ => None,
            };
            if let Some((width, coords)) = path {
                outline_widths.push(width);
                outline_is_closed.push(path_is_closed(coords));
            } else {
                outline_widths.push(0.0);
                // Non-path shapes (polygons/rects) are closed
                outline_is_closed.push(true);
            }
        }
        generate_missing_keepout_names("keepout_", &mut current_package.keepouts);
        generate_missing_keepout_names("via_keepout_", &mut current_package.via_keepouts);
        generate_missing_keepout_names("place_keepout_", &mut current_package.place_keepouts);
        let keepouts = board_keepouts(&current_package.keepouts, &coordinate_transform);
        let via_keepouts = board_keepouts(&current_package.via_keepouts, &coordinate_transform);
        let place_keepout_arr =
            board_keepouts(&current_package.place_keepouts, &coordinate_transform);

        let base_package_name = strip_side_suffix(&current_package.name);
        let mut suffix = 0u32;
        loop {
            let test_name = if suffix == 0 {
                base_package_name.clone()
            } else {
                format!("{base_package_name}::{suffix}")
            };
            // Java's `try`/`catch (Exception e)` fallback around this block
            // (Library.java:434-447) is unreachable: neither `Packages.get(String, boolean)` nor
            // `Packages.add` throws. Rust has no exceptions, so it is not ported.
            let existing_matches = board
                .library
                .packages
                .get_by_name(&test_name, current_package.is_front)
                .filter(|existing| equals_ignore_case(&existing.name, &test_name))
                .map(|existing| are_package_pins_identical(existing, &pins));
            match existing_matches {
                None => {
                    board.library.packages.add(
                        test_name,
                        pins,
                        Some(outlines),
                        Some(outline_widths),
                        Some(outline_is_closed),
                        keepouts,
                        via_keepouts,
                        place_keepout_arr,
                        current_package.is_front,
                    );
                    break;
                }
                Some(true) => break,
                Some(false) => suffix += 1,
            }
        }
    }
    Ok(true)
}

/// `Library.readScope`'s outline-closed test (Library.java:351-355): a path whose first corner
/// equals its last.
fn path_is_closed(coords: &[f64]) -> bool {
    if coords.len() < 4 {
        // Java leaves the array's `false` default in place.
        return false;
    }
    coords[0] == coords[coords.len() - 2] && coords[1] == coords[coords.len() - 1]
}

/// The three identical keepout loops of `Library.readScope` (Library.java:370-408).
// totalized: Library.readScope — Java dereferences `shapeList.iterator().next().layer` (a
// `null` first shape is possible: `Shape.readAreaScope` stores `null` for a border it could not
// read) and `Shape.transformAreaToBoardRel`'s result, both unchecked. Either NPEs out of the
// whole read; the port drops the keepout, the same outcome `Structure.insertKeepout` already
// has for a keepout it cannot build.
fn board_keepouts(
    keepout_list: &[ReadAreaScopeResult],
    coordinate_transform: &CoordinateTransform,
) -> Vec<Keepout> {
    let mut result = Vec::with_capacity(keepout_list.len());
    for current_keepout in keepout_list {
        let Some(current_layer) = current_keepout
            .shape_list
            .first()
            .and_then(Option::as_ref)
            .map(DsnShape::layer)
        else {
            continue;
        };
        let layer_no = current_layer.no;
        let Some(current_area) =
            shape::transform_area_to_board_rel(&current_keepout.shape_list, coordinate_transform)
        else {
            continue;
        };
        result.push(Keepout::new(
            current_keepout.area_name.clone().unwrap_or_default(),
            current_area,
            layer_no,
        ));
    }
    result
}

/// `Library.generateMissingKeepoutNames` (Library.java:454-472): if *any* keepout of the list is
/// unnamed, **every** keepout of the list is renamed `<prefix><1-based index>`.
fn generate_missing_keepout_names(keepout_type: &str, keepout_list: &mut [ReadAreaScopeResult]) {
    let all_names_existing = keepout_list.iter().all(|k| k.area_name.is_some());
    if all_names_existing {
        return;
    }
    // generate names
    for (i, current_keepout) in keepout_list.iter_mut().enumerate() {
        current_keepout.area_name = Some(format!("{keepout_type}{}", i + 1));
    }
}

/// Java's `name.replaceAll("\\.\\d+", "")` (Library.java:113,322): removes **every** `.` followed
/// by one or more ASCII digits, anywhere in the name — not just a trailing one.
///
/// The same expression appears in `Network.readScope` (Network.java:1292), which normalises via
/// padstack names before looking them up, and in `SesReader.processViaScope`
/// (SesReader.java:384). `pub(crate)` for the first of those (Task 9).
pub(crate) fn strip_dot_digits(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        // Java's `\d` without `UNICODE_CHARACTER_CLASS` is ASCII `[0-9]` only.
        if c == '.' && chars.peek().is_some_and(char::is_ascii_digit) {
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Java's `name.replaceAll("::\\d+$", "")` (Library.java:410): strips a trailing `"::<digits>"`.
/// Same reasoning as `fr_board`'s `strip_side_suffix`: only the last `"::"` can start an
/// all-digit run to the end.
fn strip_side_suffix(name: &str) -> String {
    if let Some(idx) = name.rfind("::") {
        let suffix = &name[idx + 2..];
        if !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
            return name[..idx].to_string();
        }
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_dot_digits_removes_every_dotted_number() {
        assert_eq!(strip_dot_digits("Pad_1358_um"), "Pad_1358_um");
        assert_eq!(strip_dot_digits("Pad.12"), "Pad");
        assert_eq!(strip_dot_digits("a.1b.23c"), "abc");
        // A dot not followed by a digit survives.
        assert_eq!(strip_dot_digits("F.Cu"), "F.Cu");
        assert_eq!(strip_dot_digits("a."), "a.");
        assert_eq!(strip_dot_digits(""), "");
        // Network.java:1292's call site, on a real fixture name.
        assert_eq!(
            strip_dot_digits("Via[0-1]_1016:485.7_um"),
            "Via[0-1]_1016:485_um"
        );
        // `\d` is ASCII-only without UNICODE_CHARACTER_CLASS, so an Arabic-Indic digit stays.
        assert_eq!(strip_dot_digits("a.\u{0661}"), "a.\u{0661}");
        assert_eq!(strip_dot_digits("PAD."), "PAD.");
        assert_eq!(strip_dot_digits("µ.7x"), "µx");
    }

    #[test]
    fn strip_side_suffix_only_strips_a_trailing_run() {
        assert_eq!(strip_side_suffix("IMG"), "IMG");
        assert_eq!(strip_side_suffix("IMG::1"), "IMG");
        assert_eq!(strip_side_suffix("IMG::1::2"), "IMG::1");
        assert_eq!(strip_side_suffix("IMG::a"), "IMG::a");
    }

    #[test]
    fn generate_missing_keepout_names_renames_all_or_none() {
        let mut list = vec![
            ReadAreaScopeResult {
                shape_list: Vec::new(),
                clearance_class_name: None,
                area_name: Some("kept".to_string()),
            },
            ReadAreaScopeResult {
                shape_list: Vec::new(),
                clearance_class_name: None,
                area_name: None,
            },
        ];
        generate_missing_keepout_names("keepout_", &mut list);
        assert_eq!(list[0].area_name.as_deref(), Some("keepout_1"));
        assert_eq!(list[1].area_name.as_deref(), Some("keepout_2"));

        let mut all_named = vec![ReadAreaScopeResult {
            shape_list: Vec::new(),
            clearance_class_name: None,
            area_name: Some("kept".to_string()),
        }];
        generate_missing_keepout_names("keepout_", &mut all_named);
        assert_eq!(all_named[0].area_name.as_deref(), Some("kept"));
    }
}
