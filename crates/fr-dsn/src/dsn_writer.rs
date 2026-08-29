//! `io/specctra/DsnWriter.java` — the single entry point that serialises a [`Board`] to Specctra
//! DSN text.
//!
//! # Deviation from Java: the coordinate transform is a parameter
//!
//! `DsnWriter.writePcbScope` (DsnWriter.java:58-91) reads the transform straight off the board at :68:
//! `board.communication.coordinateTransform`. This port's `fr-board` `Communication` has no such
//! field — Plan 3 ruling A keeps [`CoordinateTransform`] in `fr-dsn`, where the rest of the
//! Specctra layer lives, and [`crate::read_board`] hands it back on the
//! [`crate::BoardReadResult`] instead. [`write`] therefore takes it as an explicit argument. The
//! bytes are identical; only the plumbing differs.
//!
//! # `Parser::write_scope` is called with `reduced = false` even in compat mode
//!
//! DsnWriter.java:75-79 passes the literal `false` for `reduced`, so `(string_quote …)` and
//! `(space_in_quoted_tokens on)` are emitted whatever `compat_mode` says. That is not a bug this
//! port fixes — `compat_mode` reaches the writers that actually branch on it
//! (`Wiring::write_wire_scope`, which writes a `path` instead of a `polyline_path`).

use std::io::{self, Write};

use fr_board::Board;

use crate::coordinate_transform::CoordinateTransform;
use crate::format::IndentFileWriter;
use crate::parser::scope_parameter::WriteScopeParameter;
use crate::parser::{header, library, network, part_library, placement, structure, wiring};

/// `DsnWriter.write(BasicBoard, OutputStream, String, boolean)` (DsnWriter.java:45-52).
///
/// The sink is **flushed** but not closed, exactly as Java documents. `ct` is the transform Java
/// reads off `board.communication` — see the module docs.
///
/// # Errors
///
/// Returns the first I/O error any write hit, surfaced by [`IndentFileWriter::flush`]. Java has
/// no equivalent: `IndentFileWriter` swallows every `IOException` into an `FRLogger` call, so a
/// full disk silently truncates the file there.
// renamed: DsnWriter.write -> the free function `write` (the class is a private-constructor
// static holder, which Rust spells as a module).
pub fn write<W: Write>(
    board: &Board,
    ct: &CoordinateTransform,
    out: &mut W,
    design_name: &str,
    compat_mode: bool,
) -> io::Result<()> {
    let output_file = IndentFileWriter::new(out as &mut dyn Write);
    write_pcb_scope(board, ct, output_file, design_name, compat_mode)
}

/// `DsnWriter.writePcbScope` (DsnWriter.java:58-91).
///
/// `WriteScopeParameter` is built with `autorouteSettings = null` (DsnWriter.java:65), so the
/// `(autoroute_settings …)` scope is **never** written on this path; the argument is threaded
/// through [`structure::write_structure_scope`] anyway, to keep that function a faithful port of
/// `Structure.writeScope` for the day another caller passes settings.
fn write_pcb_scope<'a>(
    board: &'a Board,
    ct: &'a CoordinateTransform,
    output_file: IndentFileWriter<&'a mut dyn Write>,
    design_name: &str,
    compat_mode: bool,
) -> io::Result<()> {
    let string_quote = board.communication.string_quote.clone();
    let mut p = WriteScopeParameter::new(board, output_file, &string_quote, ct, compat_mode);

    p.file.start_scope(false);
    p.file.write("pcb ");
    p.identifier_type.write(design_name, &mut p.file);

    header::write_parser_scope(
        &mut p.file,
        &p.board.communication,
        &p.identifier_type,
        false,
    );

    header::write_resolution_scope(&mut p.file, &board.communication);
    header::write_unit_scope(&mut p.file, board.communication.unit);
    structure::write_structure_scope(&mut p, None);
    placement::write_placement_scope(&mut p);
    library::write_library_scope(&mut p);
    part_library::write_part_library_scope(&mut p);
    network::write_network_scope(&mut p);
    wiring::write_wiring_scope(&mut p);

    p.file.end_scope();
    p.file.flush()
}
