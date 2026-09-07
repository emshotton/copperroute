use std::io::{self, Write};

use copper_board::{Board, PadstackId};

use crate::coordinate_transform::CoordinateTransform;
use crate::format::IndentFileWriter;
use crate::parser::DsnRouterSettings;
use crate::parser::autoroute_settings::write_autoroute_settings_scope;
use crate::parser::library::write_padstack_scope;
use crate::parser::network::{
    write_default_rule, write_net_classes, write_via_infos, write_via_rules,
};
use crate::parser::scope_parameter::WriteScopeParameter;
use crate::parser::structure::write_snap_angle;

pub fn write<W: Write>(
    board: &Board,
    ct: &CoordinateTransform,
    settings: Option<&DsnRouterSettings>,
    out: &mut W,
    design_name: &str,
) -> io::Result<()> {
    let output_file = IndentFileWriter::new(out as &mut dyn Write);
    write_rules(board, ct, output_file, settings, design_name)
}

fn write_rules<'a>(
    board: &'a Board,
    ct: &'a CoordinateTransform,
    output_file: IndentFileWriter<&'a mut dyn Write>,
    settings: Option<&DsnRouterSettings>,
    design_name: &str,
) -> io::Result<()> {
    let string_quote = board.communication.string_quote.clone();
    let mut p = WriteScopeParameter::new(board, output_file, &string_quote, ct, false);

    p.file.start_scope_nl();
    p.file.write("rules PCB ");
    p.file.write(design_name);

    write_snap_angle(&mut p.file, board.rules.trace_angle_restriction);

    if let Some(settings) = settings {
        write_autoroute_settings_scope(
            &mut p.file,
            settings,
            board.layer_structure(),
            &p.identifier_type,
        );
    }

    write_default_rule(&mut p, 0);

    for i in 1..=board.library.padstacks.count() {
        let Some(current_padstack) = board.library.padstacks.get(PadstackId(i)) else {
            continue;
        };
        if board
            .library
            .get_via_padstack_by_name(&current_padstack.name)
            .is_none()
        {
            continue;
        }
        write_padstack_scope(&mut p, current_padstack);
    }

    write_via_infos(
        &p.board.rules,
        &p.board.library.padstacks,
        &mut p.file,
        &p.identifier_type,
    );
    write_via_rules(&p.board.rules, &mut p.file, &p.identifier_type);
    write_net_classes(&mut p);

    p.file.end_scope();
    p.file.flush()
}
