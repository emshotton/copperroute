//! `io/specctra/parser/{ReadScopeParameter,WriteScopeParameter,ScopeKeyword}.java` — the state
//! threaded through every scope reader/writer, and the generic scope dispatch loop.

use std::collections::BTreeMap;
use std::io::Write;

use fr_board::{AngleRestriction, Board, Unit};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::{DSN_RESERVED, IdentifierType, IndentFileWriter};
use crate::keyword::ScopeKeyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::DsnRouterSettings;
use crate::parser::network::{DsnNet, NetId};
use crate::parser::placement::ComponentPlacement;
use crate::parser::structure::{DsnLayerStructure, DsnPlane};
use crate::parser::{header, library, network, part_library, placement, structure, wiring};

/// Per-read options threaded through [`ReadScopeParameter`] as `options`.
///
/// Placeholder: not a Java class (Java's `DsnReader.readBoard` takes no options object at all).
/// Plan ruling 4 (Task 10) adds `normalize_time_limit`, the `StopCheck`-backed limit on
/// `Board::normalize_all_traces_checked` that `Wiring.readScope`'s final
/// `board.normalizeAllTraces()` call (Wiring.java:346) runs under.
// added in Plan 3 Task 10: normalize_time_limit (plan ruling 4)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DsnReadOptions {}

/// `io/specctra/parser/ReadScopeParameter.java`: the state every scope reader shares while
/// reading a DSN file.
///
/// Defaults (`ReadScopeParameter::new`) match the Java field initialisers: `resolution = 100`
/// (ReadScopeParameter.java, 2.3.0: `public int resolution = 100;`), `snap_angle =
/// FortyFiveDegree` (`AngleRestriction.FORTYFIVE_DEGREE`), `string_quote = "\""`,
/// `board_outline_ok = true`, `dsn_file_generated_by_host = true`, `unit = Unit::Mil`
/// (`Unit.MIL`).
///
/// Java's `boardHandling` is a `BoardParserCallback` (`not ported:` — GUI-adjacent indirection;
/// this port constructs the board directly instead, so `board` is a plain `Option<Board>`, not a
/// callback). Java's `observers`/`idGenerator`/`viaAtSmdAllowed`/`logicalPartMappings`/
/// `logicalParts` fields are not carried here; a later task adds them if and when the scope that
/// needs them lands.
///
/// Every Java `Collection` field here (`LinkedList`, insertion order) is a `Vec`; `netlist` is
/// Java's `TreeMap<Net.Id, Net>` (deterministic order), so it is a `BTreeMap<NetId, DsnNet>`
/// with [`NetId`]'s `Ord` matching `Net.Id.compareTo` exactly.
#[derive(Debug)]
pub struct ReadScopeParameter<'a> {
    /// `ReadScopeParameter.scanner`.
    pub scanner: DsnScanner,
    // not ported: createBoard, getRoutingBoard, getCurrentRoutingJob — `BoardParserCallback`'s
    // three methods (and its only implementation, `ReadScopeParameter`'s private nested
    // `MinimalBoardManager`) are GUI-adjacent indirection this port drops: `board` below is
    // constructed and held directly instead of behind a callback interface.
    // renamed: getBoard -> the public field `board` (a plain accessor method has no reason to
    // exist once there is no callback to hide).
    /// `ReadScopeParameter.boardHandling.getRoutingBoard()`, held directly rather than behind
    /// Java's `BoardParserCallback`.
    pub board: Option<Board>,
    /// `ReadScopeParameter.netlist` (`NetList`, itself a `TreeMap<Net.Id, Net>` wrapper).
    pub netlist: BTreeMap<NetId, DsnNet>,
    /// `ReadScopeParameter.planeList`.
    pub plane_list: Vec<DsnPlane>,
    /// `ReadScopeParameter.placementList`.
    pub placement_list: Vec<ComponentPlacement>,
    /// `ReadScopeParameter.constants` (`Collection<String[]>`).
    pub constants: Vec<Vec<String>>,
    /// `ReadScopeParameter.viaPadstackNames`.
    pub via_padstack_names: Vec<String>,
    /// `ReadScopeParameter.stringQuote`.
    pub string_quote: String,
    /// `ReadScopeParameter.hostCad`.
    pub host_cad: Option<String>,
    /// `ReadScopeParameter.hostVersion`.
    pub host_version: Option<String>,
    /// `ReadScopeParameter.dsnFileGeneratedByHost`.
    pub dsn_file_generated_by_host: bool,
    /// `ReadScopeParameter.boardOutlineOk`.
    pub board_outline_ok: bool,
    /// `ReadScopeParameter.coordinateTransform`.
    pub coordinate_transform: Option<CoordinateTransform>,
    /// `ReadScopeParameter.layerStructure`.
    pub layer_structure: Option<DsnLayerStructure>,
    /// `ReadScopeParameter.autorouteSettings`.
    pub autoroute_settings: Option<DsnRouterSettings>,
    /// `ReadScopeParameter.unit`.
    pub unit: Unit,
    /// `ReadScopeParameter.resolution`.
    pub resolution: i32,
    /// `ReadScopeParameter.snapAngle`.
    pub snap_angle: AngleRestriction,
    // renamed: getWarnings -> the public field `warnings` (Java's getter wraps it in
    // `Collections.unmodifiableList`; the port has no such wrapper to offer, so the field is
    // exposed directly).
    /// `ReadScopeParameter.warnings`.
    pub warnings: Vec<String>,
    /// Not a Java field — see [`DsnReadOptions`].
    pub options: &'a DsnReadOptions,
}

impl<'a> ReadScopeParameter<'a> {
    /// `ReadScopeParameter(IJFlexScanner, BoardObservers, IdGenerator)`
    /// (ReadScopeParameter.java), minus the two host-embedding parameters this port drops.
    #[must_use]
    pub fn new(scanner: DsnScanner, options: &'a DsnReadOptions) -> ReadScopeParameter<'a> {
        ReadScopeParameter {
            scanner,
            board: None,
            netlist: BTreeMap::new(),
            plane_list: Vec::new(),
            placement_list: Vec::new(),
            constants: Vec::new(),
            via_padstack_names: Vec::new(),
            string_quote: "\"".to_string(),
            host_cad: None,
            host_version: None,
            dsn_file_generated_by_host: true,
            board_outline_ok: true,
            coordinate_transform: None,
            layer_structure: None,
            autoroute_settings: None,
            unit: Unit::Mil,
            resolution: 100,
            snap_angle: AngleRestriction::FortyFiveDegree,
            warnings: Vec::new(),
            options,
        }
    }
}

/// `io/specctra/parser/WriteScopeParameter.java`: the state every scope writer shares while
/// writing a DSN file.
pub struct WriteScopeParameter<'a> {
    /// `WriteScopeParameter.board`.
    pub board: &'a Board,
    /// `WriteScopeParameter.file`.
    pub file: IndentFileWriter<&'a mut dyn Write>,
    /// `WriteScopeParameter.identifierType`, built from the DSN reserved-character set
    /// (`{"(", ")", " ", ";", "-", "_"}`, WriteScopeParameter.java:33) and the caller's quote
    /// character.
    pub identifier_type: IdentifierType,
    /// `WriteScopeParameter.coordinateTransform`.
    pub coordinate_transform: &'a CoordinateTransform,
    /// `WriteScopeParameter.compatMode`.
    pub compat_mode: bool,
}

impl<'a> WriteScopeParameter<'a> {
    /// `WriteScopeParameter(BasicBoard, RouterSettings, IndentFileWriter, String,
    /// CoordinateTransform, boolean)` (WriteScopeParameter.java:26-36) — minus the
    /// `autorouteSettings` parameter, which this port's callers pass separately rather than
    /// storing on the parameter (plan Task 4 brief).
    #[must_use]
    pub fn new(
        board: &'a Board,
        file: IndentFileWriter<&'a mut dyn Write>,
        string_quote: &str,
        coordinate_transform: &'a CoordinateTransform,
        compat_mode: bool,
    ) -> WriteScopeParameter<'a> {
        let identifier_type = IdentifierType::new(
            DSN_RESERVED.iter().map(|s| (*s).to_string()).collect(),
            string_quote.to_string(),
        );
        WriteScopeParameter {
            board,
            file,
            identifier_type,
            coordinate_transform,
            compat_mode,
        }
    }
}

/// `ScopeKeyword.skipScope` (ScopeKeyword.java:20-38): consumes tokens until the bracket that
/// matches the scope's own opening bracket (the caller has already consumed `(` and the scope
/// keyword; `open_bracket_count` therefore starts at 1, exactly as Java's does).
///
/// `scanner.yybegin(LexicalState::Name)` runs before **every** token read here, including the
/// first — this is load-bearing (see the lexer docs on `NAME`: a bare number like `123abc` only
/// lexes as one `Str` token in that state) and easy to lose in a refactor.
///
/// Java returns `false` (after a dropped `FRLogger.error`/`.warn`) on end-of-file or a scan
/// error, and every one of its callers (`ScopeKeyword.readScope`, `DsnFile.readOnOffScope`, …)
/// discards that boolean. The port instead surfaces premature end-of-file as
/// [`DsnError::UnexpectedEof`] — a deliberate, documented divergence from Java's silent
/// swallow-and-continue, on the view that a caller should see this rather than have it silently
/// absorbed into an apparently-successful read of a truncated file.
pub fn skip_scope(scanner: &mut DsnScanner) -> Result<(), DsnError> {
    let mut open_bracket_count: i32 = 1;
    while open_bracket_count > 0 {
        scanner.yybegin(LexicalState::Name);
        let token = scanner.next_token()?;
        let Some(token) = token else {
            return Err(DsnError::UnexpectedEof {
                scope: scanner.scope_identifier().to_string(),
            });
        };
        match token {
            Token::Open => open_bracket_count += 1,
            Token::Close => open_bracket_count -= 1,
            _ => {}
        }
    }
    Ok(())
}

/// The generic `ScopeKeyword.readScope` loop (ScopeKeyword.java:46-73) — used directly for
/// [`ScopeKeyword::Pcb`], which has no Java subclass (`Keyword.PCB_SCOPE = new
/// ScopeKeyword("pcb")`, `Keyword.java:66`) and so never overrides it, and recursively, from
/// within this same loop, whenever it meets a nested scope keyword (via [`read_scope`]).
///
/// Tracks only "was the previous token `(`" rather than the previous token's full value —
/// Java's `prevToken` is compared exactly once, against `Keyword.OPEN_BRACKET`
/// (ScopeKeyword.java:59), so that is everything the loop needs to remember.
fn read_scope_generic(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut prev_was_open = false;
    loop {
        let token = p.scanner.next_token()?;
        let Some(token) = token else {
            // End of file (ScopeKeyword.java:53-55).
            return Ok(true);
        };
        if token == Token::Close {
            break;
        }
        let is_open = token == Token::Open;
        if prev_was_open {
            match token {
                Token::Kw(kw) => match ScopeKeyword::from_keyword(kw) {
                    Some(next_scope) => {
                        if !read_scope(next_scope, p)? {
                            return Ok(false);
                        }
                    }
                    None => skip_scope(&mut p.scanner)?,
                },
                _ => skip_scope(&mut p.scanner)?,
            }
        }
        prev_was_open = is_open;
    }
    Ok(true)
}

/// `ScopeKeyword.readScope`, dispatched by identity rather than by Java's virtual call
/// (ScopeKeyword.java:46, overridden per subclass by later Plan 3 tasks). `scope` names which
/// scope's body comes next.
///
/// Every variant except [`ScopeKeyword::Pcb`] has a same-named Java scope class with its own
/// `readScope` override; those are one stub function per module for now (`// added in Plan 3:`
/// markers on each), replaced with real bodies as later tasks land. `Pcb` has no Java subclass,
/// so it is [`read_scope_generic`] itself.
pub fn read_scope(scope: ScopeKeyword, p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    match scope {
        ScopeKeyword::Pcb => read_scope_generic(p),
        ScopeKeyword::Structure => structure::read_structure_scope(p),
        ScopeKeyword::Plane => structure::read_plane_scope(p),
        ScopeKeyword::Network => network::read_network_scope(p),
        ScopeKeyword::Wiring => wiring::read_wiring_scope(p),
        ScopeKeyword::Library => library::read_library_scope(p),
        ScopeKeyword::PartLibrary => part_library::read_part_library_scope(p),
        ScopeKeyword::Placement => placement::read_placement_scope(p),
        ScopeKeyword::Component => placement::read_component_scope(p),
        ScopeKeyword::Parser => header::read_parser_scope(p),
        ScopeKeyword::Resolution => header::read_resolution_scope(p),
        ScopeKeyword::PlaceControl => header::read_place_control_scope(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_scope_parameter_defaults_match_java() {
        let options = DsnReadOptions::default();
        let p = ReadScopeParameter::new(DsnScanner::new("").expect("fits"), &options);
        assert_eq!(p.resolution, 100);
        assert_eq!(p.snap_angle, AngleRestriction::FortyFiveDegree);
        assert_eq!(p.string_quote, "\"");
        assert!(p.board_outline_ok);
        assert!(p.dsn_file_generated_by_host);
        assert_eq!(p.unit, Unit::Mil);
        assert!(p.board.is_none());
        assert!(p.netlist.is_empty());
    }
}
