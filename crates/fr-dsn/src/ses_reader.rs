//! `io/specctra/SesReader.java` — re-imports the routing result of a Specctra session (`.ses`)
//! file into an **existing** [`Board`], plus `io/specctra/SesImportSummary.java`.
//!
//! # What this reader is not
//!
//! It is not a second DSN reader. It never builds a board, never touches `library`, `placement`,
//! `structure` or `network`, and knows exactly five scopes: `session` → `routes` → `network_out`
//! → `net` → `wire` | `via`. Everything else, `placement` and `was_is` included, goes to
//! [`skip_scope`]. The board it mutates must already carry the nets, the padstack library and
//! the layer structure the session refers to — which is why every caller reads the design's
//! `.dsn` first.
//!
//! # Deviation from Java: the coordinate transform is a parameter
//!
//! `SesReader.read` derives the session-file scale from the board:
//! `board.communication.coordinateTransform.dsnToBoard(1) / board.communication.resolution`
//! (SesReader.java:71-72) — the same expression `SesWriter.writeSessionScope` uses, so a file
//! this port writes reads back on the same grid it was written on. This port's `fr-board`
//! [`Board`] has no `coordinateTransform` field (Plan 3 ruling A keeps [`CoordinateTransform`]
//! in `fr-dsn`), so [`read`] takes the *DSN* transform explicitly and recomputes the SES one from
//! it exactly as Java does. Same deviation, same reason as [`crate::ses_writer::write`].
//!
//! # Layer mapping is by name
//!
//! `new LayerStructure(board.layerStructure)` (SesReader.java:42) numbers the **board's own**
//! layers 0, 1, … and `Shape.readPolygonPathScope` looks the session's layer name up in it. A
//! session written for a different layer order therefore lands on the right layers, and one
//! naming a layer the board does not have produces a `null` path, which
//! [`SesReader::process_wire_scope`] cannot tell apart from a conduction area — see its note.

use std::io::Read;

use fr_board::{Board, FixedState, ItemClass, PadstackId};
use fr_geometry::{IntPoint, Point, Polyline};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::java_round_to_int;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::geometry::{DsnLayerStructure, DsnPolygonPath, read_polygon_path_scope};
use crate::parser::library::strip_dot_digits;
use crate::parser::scope_parameter::skip_scope;

/// `io/specctra/SesImportSummary.java`, a three-`int` record: "a non-zero `errorsEncountered`
/// value indicates that one or more wires or vias could not be imported (e.g. an unknown net
/// name or a malformed coordinate). The board may still be partially populated in that case".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SesImportSummary {
    /// `SesImportSummary.wiresImported`: the number of wire traces successfully imported.
    pub wires_imported: usize,
    /// `SesImportSummary.viasImported`: the number of vias successfully imported.
    pub vias_imported: usize,
    /// `SesImportSummary.errorsEncountered`: the number of non-fatal errors during import.
    pub errors_encountered: usize,
}

/// `SesReader.read(InputStream, BasicBoard)` (SesReader.java:60-90): reads `input` and imports
/// its wires and vias into `board`.
///
/// `ct` is the DSN transform Java reads off `board.communication` — see the module docs.
///
/// # Errors
///
/// [`DsnError::NotASessionFile`] when the stream does not start with a `(session <name>` header
/// (Java's `IOException`, SesReader.java:129-132), plus the usual I/O and lexer failures. A
/// **content** problem is never an error: a wire whose layer is unknown, a via whose padstack is
/// missing, a net the board does not have — each of those bumps
/// [`SesImportSummary::errors_encountered`] and the read carries on, which is the whole point of
/// the summary.
//
// renamed: SesReader.read -> the free function `read` (the class is a private-constructor holder
// with three mutable counters, which Rust spells as a module plus a private struct).
// not ported: the `in == null` / `board == null` guards (SesReader.java:61-66) — neither `impl
// Read` nor `&mut Board` can be null, so `nullInputStreamThrowsIoException` and
// `nullBoardThrowsIoException` have no Rust counterpart.
// not ported: `closeQuietly(in)` in the `finally` (SesReader.java:87-89, :96-102) — the stream is
// consumed by value and dropped, which closes a `File` the same way.
// not ported: the `FRLogger.info("SES file import complete: …")` line (:78-84); the same numbers
// are the return value.
// not ported: `SesReader.saveSpecctraSessionSesAsEagleScriptScr` (:105-109), a one-line delegate
// to `parser/SessionToEagle.java`, which Plan 3 does not port (spec §Scope: no Eagle output).
pub fn read(
    input: impl Read,
    board: &mut Board,
    ct: &CoordinateTransform,
) -> Result<SesImportSummary, DsnError> {
    let text = read_to_string(input)?;
    let scanner = DsnScanner::new(&text);

    // SES files use the DSN-to-board scale factor: dsn_to_board(1) / resolution
    // (SesReader.java:71-72). See the module docs for why `ct` is a parameter here.
    let scale_factor = ct.dsn_to_board(1.0) / f64::from(board.communication.resolution);

    // `new LayerStructure(board.layerStructure)` (SesReader.java:42).
    let specctra_layer_structure = DsnLayerStructure::from_board(board.layer_structure());

    let mut reader = SesReader {
        scanner,
        board,
        specctra_layer_structure,
        session_file_scale_denominator: scale_factor,
        wires_imported: 0,
        vias_imported: 0,
        errors_encountered: 0,
    };
    reader.process_session_scope()?;
    Ok(SesImportSummary {
        wires_imported: reader.wires_imported,
        vias_imported: reader.vias_imported,
        errors_encountered: reader.errors_encountered,
    })
}

/// The four fields of Java's `SesReader` instance plus its three counters
/// (SesReader.java:31-37).
struct SesReader<'a> {
    scanner: DsnScanner,
    board: &'a mut Board,
    specctra_layer_structure: DsnLayerStructure,
    session_file_scale_denominator: f64,
    wires_imported: usize,
    vias_imported: usize,
    errors_encountered: usize,
}

impl SesReader<'_> {
    /// `SesReader.processSessionScope` (SesReader.java:116-157): the `(session <name>` header
    /// check, then the session scope's direct subscopes.
    fn process_session_scope(&mut self) -> Result<(), DsnError> {
        // Validate the "(session <name>" header (SesReader.java:118-133). Note `keywordOk` is
        // left `true` for `i == 2`: the session name itself is never validated, exactly as
        // `DsnReader`'s `(pcb <name>` check leaves the design name alone.
        let mut next_token: Option<Token> = None;
        for i in 0..3 {
            next_token = self.scanner.next_token()?;
            let keyword_ok = match i {
                0 => next_token == Some(Token::Open),
                1 => {
                    let ok = next_token == Some(Token::Kw(Keyword::Session));
                    // consume the session name
                    self.scanner.yybegin(LexicalState::Name);
                    ok
                }
                _ => true,
            };
            if !keyword_ok {
                return Err(DsnError::NotASessionFile {
                    got: describe_token(next_token.as_ref()),
                });
            }
        }

        // Read the direct subscopes of the session scope (SesReader.java:136-156).
        loop {
            let prev_token = next_token;
            next_token = self.scanner.next_token()?;
            let Some(token) = next_token.clone() else {
                // end of file
                return Ok(());
            };
            if token == Token::Close {
                // end of session scope
                break;
            }
            if prev_token == Some(Token::Open) {
                if token == Token::Kw(Keyword::Routes) {
                    self.process_routes_scope()?;
                } else {
                    // skip placement, was_is, and any other scopes we don't need
                    skip_scope(&mut self.scanner)?;
                }
            }
        }
        Ok(())
    }

    /// `SesReader.processRoutesScope` (SesReader.java:160-184): the `(routes …)` scope, whose
    /// only interesting child is `network_out`. `resolution`, `parser` and `library_out` are all
    /// skipped — the board already has them.
    // not ported: the `FRLogger.warn("… unexpected end of file at '…'")` at :166-170.
    fn process_routes_scope(&mut self) -> Result<(), DsnError> {
        self.process_container_scope(Keyword::NetworkOut, Self::process_network_scope)
    }

    /// `SesReader.processNetworkScope` (SesReader.java:187-211): the `(network_out …)` scope.
    // not ported: the `FRLogger.warn` at :193-197.
    fn process_network_scope(&mut self) -> Result<(), DsnError> {
        self.process_container_scope(Keyword::Net, Self::process_net_scope)
    }

    /// The body `processRoutesScope` and `processNetworkScope` share verbatim
    /// (SesReader.java:162-183 == :189-210, differing only in the keyword they recurse on and in
    /// the method name inside their warning text, which this port drops).
    // renamed: the two identical loops -> one `process_container_scope`.
    fn process_container_scope(
        &mut self,
        wanted: Keyword,
        mut on_match: impl FnMut(&mut Self) -> Result<(), DsnError>,
    ) -> Result<(), DsnError> {
        let mut next_token: Option<Token> = None;
        loop {
            let prev_token = next_token;
            next_token = self.scanner.next_token()?;
            let Some(token) = next_token.clone() else {
                // unexpected end of file
                return Ok(());
            };
            if token == Token::Close {
                break;
            }
            if prev_token == Some(Token::Open) {
                if token == Token::Kw(wanted) {
                    on_match(self)?;
                } else {
                    skip_scope(&mut self.scanner)?;
                }
            }
        }
        Ok(())
    }

    /// `SesReader.processNetScope` (SesReader.java:214-260): one `(net <name> (wire …)* (via …)*)`
    /// scope.
    ///
    /// The net is looked up by name **with subnet number 1** (`nets.get(netName, 1)`, :226) —
    /// unlike `Wiring.readWireScope`, which collects every subnet of the name. A session naming a
    /// multi-subnet net therefore lands all of its wires on subnet 1.
    // not ported: the `FRLogger.warn` calls at :217-220 and :228.
    fn process_net_scope(&mut self) -> Result<(), DsnError> {
        let mut next_token = self.scanner.next_token()?;
        let Some(Token::Str(net_name)) = next_token.clone() else {
            // "SesReader.processNetScope: String expected at '…'" (:217-222).
            self.errors_encountered += 1;
            return Ok(());
        };
        self.scanner.set_scope_identifier(&net_name);

        let Some(net) = self.board.rules.nets.get_by_name_and_subnet(&net_name, 1) else {
            // "SesReader: net not found: '…' — skipping" (:227-231).
            self.errors_encountered += 1;
            skip_scope(&mut self.scanner)?;
            return Ok(());
        };
        let net_numbers = vec![net.net_number];

        loop {
            let prev_token = next_token;
            next_token = self.scanner.next_token()?;
            let Some(token) = next_token.clone() else {
                // SesReader.java:239-241: a bare `return`, with neither a warning nor an error
                // count — unlike every sibling loop.
                return Ok(());
            };
            if token == Token::Close {
                break;
            }
            if prev_token == Some(Token::Open) {
                if token == Token::Kw(Keyword::Wire) {
                    if !self.process_wire_scope(&net_numbers)? {
                        self.errors_encountered += 1;
                    }
                } else if token == Token::Kw(Keyword::Via) {
                    if !self.process_via_scope(&net_numbers)? {
                        self.errors_encountered += 1;
                    }
                } else {
                    skip_scope(&mut self.scanner)?;
                }
            }
        }
        Ok(())
    }

    /// `SesReader.processWireScope` (SesReader.java:270-333): a `(wire (path …))` becomes one
    /// `USER_FIXED` trace. Answers Java's `boolean` — `false` makes the caller count an error.
    ///
    /// The trace's clearance class is the **default net class's** `TRACE` class (:316-321), not
    /// the class of the net the wire is on and not anything the session said. That is what Java
    /// does; it is not an oversight to fix here.
    // not ported: the two `FRLogger.warn` calls at :277-280 and :330.
    fn process_wire_scope(&mut self, net_numbers: &[i32]) -> Result<bool, DsnError> {
        let mut wire_path: Option<DsnPolygonPath> = None;
        let mut next_token: Option<Token> = None;
        loop {
            let prev_token = next_token;
            next_token = self.scanner.next_token()?;
            let Some(token) = next_token.clone() else {
                // unexpected end of file
                return Ok(false);
            };
            if token == Token::Close {
                break;
            }
            if prev_token == Some(Token::Open) {
                if token == Token::Kw(Keyword::PolygonPath) {
                    wire_path = read_polygon_path_scope(
                        &mut self.scanner,
                        Some(&self.specctra_layer_structure),
                    )?;
                } else {
                    skip_scope(&mut self.scanner)?;
                }
            }
        }

        // SesReader.java:295-298: "conduction areas have no polygon_path — silently skip", and
        // the wire counts as a *success*. `readPolygonPathScope` also answers `null` for a path
        // on an unknown layer, for a single-point path and for a path with a non-numeric corner
        // (Shape.java:455-514), so those three are silently swallowed here too.
        let Some(wire_path) = wire_path else {
            return Ok(true);
        };

        // Everything below is Java's `try` block (:300-332); its `catch (Exception)` answers
        // `false`, which is what each guarded early return here does.
        let denominator = self.session_file_scale_denominator;
        let coordinate_arr = &wire_path.coordinate_arr;
        let mut points: Vec<Point> = Vec::with_capacity(coordinate_arr.len() / 2);
        for i in 0..coordinate_arr.len() / 2 {
            points.push(Point::Int(IntPoint::new(
                java_round_to_int(coordinate_arr[2 * i] / denominator),
                java_round_to_int(coordinate_arr[2 * i + 1] / denominator),
            )));
        }

        let polyline = Polyline::from_points(&points);
        let half_width = java_round_to_int(wire_path.width / (2.0 * denominator));

        // SesReader.processWireScope passes `wirePath.layer.no` (:301) straight to `insertTrace`
        // with no range check, and it does not need one: `Trace`'s constructor clamps the layer
        // on both sides — `Math.min(Math.max(p_layer, 0), board.getLayerCount() - 1)`
        // (Trace.java:45-47). The two shared pseudo-layers `Layer.PCB` and `Layer.SIGNAL` carry
        // `no == -1`, and `Shape.getLayer` hands either of them back for a path on layer
        // `pcb`/`signal`, so such a wire lands on **layer 0** and counts as imported. JVM-verified
        // against the 2.3.0 jar: relabelling one `(path B.Cu …)` of `Issue026-J2_reference.ses`
        // to `(path pcb …)` still gives `wires=89 vias=10 errors=0`, with that one trace moved
        // from layer 1 to layer 0 (`pcb_layer_path_lands_on_layer_0_and_counts_as_a_wire`).
        //
        // [`fr_board::PolylineTrace::new`] applies the *upper* half of that clamp; the lower half
        // is this `unwrap_or(0)`, because a `usize` layer cannot be negative in the first place.
        let layer_index = usize::try_from(wire_path.layer.no).unwrap_or(0);

        let clearance_class = self.default_clearance_class(ItemClass::Trace);

        // The return value is discarded, so a polyline Java's `insertTrace` rejects (fewer than
        // two distinct corners, or a closed trace) still counts as an imported wire.
        self.board.insert_trace(
            polyline,
            layer_index,
            half_width,
            net_numbers.to_vec(),
            clearance_class,
            FixedState::UserFixed,
        );
        self.wires_imported += 1;
        Ok(true)
    }

    /// `SesReader.processViaScope` (SesReader.java:341-412): a `(via <padstack> <x> <y> …)`
    /// becomes one `USER_FIXED` via, with `attach_allowed` hard-coded `true` (:403) — unlike
    /// `Wiring.readViaScope`, which consults the padstack and the `via_at_smd_allowed` setting.
    // not ported: the four `FRLogger.warn` calls at :344-348, :360-364, :376-380 and :409.
    fn process_via_scope(&mut self, net_numbers: &[i32]) -> Result<bool, DsnError> {
        let mut next_token = self.scanner.next_token()?;
        let Some(Token::Str(padstack_name)) = next_token.clone() else {
            // "padstack name expected at '…'" (:343-349).
            return Ok(false);
        };
        self.scanner.set_scope_identifier(&padstack_name);

        // SesReader.java:352-366.
        let mut location = [0.0f64; 2];
        for slot in &mut location {
            next_token = self.scanner.next_token()?;
            match next_token {
                Some(Token::Float(value)) => *slot = value,
                #[allow(clippy::cast_precision_loss)]
                Some(Token::Int(value)) => *slot = value as f64,
                // "number expected at '…'" (:359-365).
                _ => return Ok(false),
            }
        }

        // Skip any additional sub-scopes (e.g. type, lock_type) — SesReader.java:368-381.
        next_token = self.scanner.next_token()?;
        while next_token == Some(Token::Open) {
            skip_scope(&mut self.scanner)?;
            next_token = self.scanner.next_token()?;
        }
        if next_token != Some(Token::Close) {
            // "closing bracket expected at '…'" (:375-381).
            return Ok(false);
        }

        // Java's `try` block (:383-411).
        let cleaned_name = strip_dot_digits(&padstack_name);
        let Some(via_padstack) = self
            .board
            .library
            .padstacks
            .get_by_name(&cleaned_name)
            .map(|padstack| PadstackId(padstack.no))
        else {
            // "via padstack not found: …" (:386-389) — the *warning* keeps the original name.
            return Ok(false);
        };

        let denominator = self.session_file_scale_denominator;
        let via_location = Point::Int(IntPoint::new(
            java_round_to_int(location[0] / denominator),
            java_round_to_int(location[1] / denominator),
        ));

        let clearance_class = self.default_clearance_class(ItemClass::Via);

        // totalized: `BasicBoard.insertVia` (:402-403) cannot fail in Java, and `processViaScope`'s
        // `catch (Exception)` would swallow it if it did; the port's answers a `Result` because
        // `split_traces` can surface a `Polyline` normalisation failure (quirk #109). Mapped to
        // the `false` Java's catch produces rather than propagated, so one bad via costs one
        // `errorsEncountered` instead of the whole import. The via is on the board either way —
        // `Board::insert_via` calls `insert_item` before the `split_traces` loop that can fail,
        // exactly as `BasicBoard.insertVia` does — so an `Err` here leaves an *uncounted* via,
        // which is the state Java's caught exception leaves behind too.
        if self
            .board
            .insert_via(
                via_padstack,
                via_location,
                net_numbers.to_vec(),
                clearance_class,
                FixedState::UserFixed,
                true,
            )
            .is_err()
        {
            return Ok(false);
        }
        self.vias_imported += 1;
        Ok(true)
    }

    /// `board.rules.getDefaultNetClass().defaultItemClearanceClasses.get(…)`
    /// (SesReader.java:316-321, :395-400), the two occurrences of which differ only in the
    /// `ItemClass`.
    // renamed: the two inlined lookups -> `default_clearance_class`.
    fn default_clearance_class(&mut self, item_class: ItemClass) -> usize {
        let default_net_class = self.board.rules.get_default_net_class();
        self.board
            .rules
            .net_classes
            .get(default_net_class)
            .default_item_clearance_classes
            .get(item_class)
    }
}

/// The token in Java's header-failure message (SesReader.java:130-131), which is a bare string
/// concatenation of an `Object`.
//
// not ported: Java's `Keyword` has no `toString`, so a keyword token renders there as
// `app.freerouting.io.specctra.parser.Keyword@<identity hash>` — a different string on every
// JVM run, which nothing can assert against. This port prints the keyword's Specctra name
// instead. Strings and `null` are spelled exactly as Java spells them.
fn describe_token(token: Option<&Token>) -> String {
    match token {
        None => "null".to_string(),
        Some(Token::Open) => "(".to_string(),
        Some(Token::Close) => ")".to_string(),
        Some(Token::Kw(keyword)) => keyword.name().to_string(),
        Some(Token::Str(value)) => value.clone(),
        Some(Token::Int(value)) => value.to_string(),
        Some(Token::Float(value)) => crate::format::java_double_to_string(*value),
    }
}

/// `new SpecctraDsnStreamReader(in)`'s `InputStreamReader` (SesReader.java:68) — see
/// `dsn_reader`'s copy of this function for why the whole stream is read up front.
fn read_to_string(mut input: impl Read) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
