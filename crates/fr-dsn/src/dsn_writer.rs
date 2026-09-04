use std::io::{self, Write};

use fr_board::Board;

use crate::coordinate_transform::CoordinateTransform;
use crate::format::IndentFileWriter;
use crate::parser::scope_parameter::WriteScopeParameter;
use crate::parser::{header, library, network, part_library, placement, structure, wiring};

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
