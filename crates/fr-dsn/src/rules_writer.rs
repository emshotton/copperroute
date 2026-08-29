//! `io/specctra/RulesWriter.java` — the single entry point that serialises a [`Board`]'s design
//! rules to a Specctra `.rules` file.
//!
//! # Deviation from Java: the coordinate transform is a parameter
//!
//! `RulesWriter.write` builds its `WriteScopeParameter` from `board.communication`
//! (RulesWriter.java:57-65), including `coordinateTransform`. This port's `fr-board`
//! [`Board`] has no such field — Plan 3 ruling A keeps [`CoordinateTransform`] in `fr-dsn` —
//! so [`write`] takes it explicitly, exactly as [`crate::dsn_writer::write`] does. It is not an
//! optional argument: `Rule::write_default_rule` and `Library::write_padstack_scope` both convert
//! board units back to DSN units through it.

use std::io::{self, Write};

use fr_board::{Board, PadstackId};

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

/// `RulesWriter.write(BasicBoard, RouterSettings, OutputStream, String)`
/// (RulesWriter.java:54-68): writes `board`'s design rules — and, when `settings` is given, the
/// router settings — to `out`.
///
/// The sink is **flushed** but not closed, exactly as Java documents (:44-45). `settings: None`
/// is Java's three-argument overload (:36-39), which passes `null` and so never writes an
/// `(autoroute_settings …)` scope.
///
/// `ct` is the transform Java reads off `board.communication` — see the module docs.
///
/// # Errors
///
/// Returns the first I/O error any write hit, surfaced by [`IndentFileWriter::flush`]. Java has
/// no equivalent: `IndentFileWriter` swallows every `IOException` into an `FRLogger` call, so a
/// full disk silently truncates the file there.
// renamed: RulesWriter.write -> the free function `write` (the class is a private-constructor
// static holder, which Rust spells as a module).
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

/// `RulesWriter.writeRules(WriteScopeParameter, String)` (RulesWriter.java:74-103).
///
/// The scope order is Java's and is load-bearing for byte parity: snap angle → autoroute settings
/// (only when present) → the default rule *on layer 0* → the via padstacks → the via infos → the
/// via rules → the net classes.
fn write_rules<'a>(
    board: &'a Board,
    ct: &'a CoordinateTransform,
    output_file: IndentFileWriter<&'a mut dyn Write>,
    settings: Option<&DsnRouterSettings>,
    design_name: &str,
) -> io::Result<()> {
    let string_quote = board.communication.string_quote.clone();
    // `compatMode` is the literal `false` Java passes (RulesWriter.java:64).
    let mut p = WriteScopeParameter::new(board, output_file, &string_quote, ct, false);

    p.file.start_scope_nl();
    p.file.write("rules PCB ");
    // Java bug: RulesWriter.writeRules — the design name goes through the raw
    // `file.write(designName)` (RulesWriter.java:78), **not** `identifierType.write`, so it is
    // never quoted however many reserved characters it holds. `DsnWriter.writePcbScope`
    // (DsnWriter.java:70) quotes the very same string, so a design named `Issue593-BBD_Mars-64`
    // is `"Issue593-BBD_Mars-64"` in the `.dsn` and bare in the `.rules`. Reproduced verbatim;
    // see `docs/java-quirks.md`.
    p.file.write(design_name);

    write_snap_angle(&mut p.file, p.board.rules.trace_angle_restriction);

    if let Some(settings) = settings {
        let layer_structure = p.board.layer_structure().clone();
        write_autoroute_settings_scope(&mut p.file, settings, &layer_structure, &p.identifier_type);
    }

    // write the default rule using 0 as default layer
    write_default_rule(&mut p, 0);

    // write the via padstacks
    for i in 1..=p.board.library.padstacks.count() {
        let Some(current_padstack) = p.board.library.padstacks.get(PadstackId(i)) else {
            continue;
        };
        // Java's `getViaPadstack(name) != null` filter (RulesWriter.java:93): only padstacks the
        // board also lists as *via* padstacks are written. The via-padstack list may name the
        // same padstack twice (`Network.readViaInfo` appends without checking), which this loop
        // is immune to — it iterates the library, not the via list.
        if p.board
            .library
            .get_via_padstack_by_name(&current_padstack.name)
            .is_none()
        {
            continue;
        }
        let current_padstack = current_padstack.clone();
        write_padstack_scope(&mut p, &current_padstack);
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
