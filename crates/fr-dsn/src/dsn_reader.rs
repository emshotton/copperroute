//! `io/specctra/DsnReader.java` — the two entry points that turn a Specctra DSN stream into a
//! [`BoardReadResult`].
//!
//! [`read_board`] hand-checks the `(pcb <name>` header and then runs the generic
//! [`ScopeKeyword::Pcb`] loop; [`read_metadata`] re-implements the PCB-level loop so it can stop
//! at the end of the `structure` scope and never touch `library`, `placement`, `network` or
//! `wiring` (`DsnReaderMetadataTest.readMetadataCompletesWithinReasonableTimeOnLargeDsn` is an
//! assertion about exactly that shortcut, so the two must not be collapsed into one).
//!
//! **There is no line/column tracking anywhere in Java**, so every
//! [`BoardReadResult::ParseError`] this module produces carries the literal location `"(pcb"`,
//! as Java's four `new BoardReadResult.ParseError("(pcb", …)` calls do.

use std::io::Read;

use fr_board::ItemIdGenerator;

use crate::error::{BoardMetadata, BoardReadResult, DsnError};
use crate::keyword::{Keyword, ScopeKeyword};
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::scope_parameter::{DsnReadOptions, ReadScopeParameter, read_scope, skip_scope};
use crate::parser::{dsn_file, header, structure};

/// The location every `ParseError` carries (DsnReader.java:65,106,159,184,211).
const PCB: &str = "(pcb";

/// `DsnReader.readBoard(InputStream, BoardObservers, IdGenerator, String)`
/// (DsnReader.java:58-161).
///
/// * `id_generator` is Java's nullable `IdGenerator`; `None` is Java's `new ItemIdGenerator()`
///   default (:73-75). It becomes the board's `Communication.idGenerator`
///   (Structure.java:1252). **Taken by value, and `ItemIdGenerator` is `Copy`**, so — unlike
///   Java, which shares the caller's object by reference and leaves it advanced by however many
///   ids the read consumed — the caller's generator is unchanged when this returns. Read the
///   board's `communication.id_gen` to see where it got to.
/// * `design_name` is Java's nullable filename hint. Java uses it for **one thing only** — the
///   `"DSN file '<name>' was loaded with N warning(s)."` log line (:139-145) — so in this port
///   it feeds nothing; see the `not ported:` note below.
/// * `options` is not a Java parameter: it carries plan ruling 4's normalisation time limit (see
///   [`DsnReadOptions`]).
///
/// # Which variant comes back
///
/// | Java | here |
/// |---|---|
/// | `readScope` true | [`BoardReadResult::Success`], with **`metadata: None`** — only [`read_metadata`] fills it (:146) |
/// | `readScope` true, but the input ended inside an unclosed scope | [`BoardReadResult::Partial`] — quirk #91, fixed by Plan 9 Task 4. Java has no such row: it answers `Success` here too |
/// | `readScope` false, `boardOutlineOk` false | [`BoardReadResult::OutlineMissing`] (:147-157) |
/// | `readScope` false, `boardOutlineOk` true | [`BoardReadResult::ParseError`], detail `"DSN structure parsing failed"` (:159) |
/// | header check failed | [`BoardReadResult::ParseError`], detail `"Not a Specctra DSN file: expected '(pcb <name>' header"` (:106-107) |
/// | `IOException` from a header-token read | [`BoardReadResult::IoError`] (:87-90) |
///
/// Deeper IO errors never reach the caller in Java either: `ScopeKeyword.readScope` and every
/// scope reader below it catch `IOException` and answer `false`, which lands in the
/// `ParseError`/`OutlineMissing` rows above.
//
// not ported: `observers` (`BoardObservers`, DsnReader.java:59) — GUI/host embedding, dropped
// with `BoardParserCallback` in Plan 3 Task 4.
// not ported: `effectiveDesignName` (DsnReader.java:111-120) and the two identical
// `FRLogger.warn("DSN file '…' was loaded with N warning(s).")` calls it exists for (:139-145,
// :150-155). `fr-dsn` has no logger, so the name resolution has no consumer; the parameter is
// kept so a host that wants the message can build it from the warnings this returns.
// not ported: the `inputStream == null` guard (DsnReader.java:64-66) — `impl Read` cannot be
// null; the nearest reachable input, an empty stream, fails the header check instead and gives
// the same `ParseError`.
pub fn read_board(
    input: impl Read,
    id_generator: Option<ItemIdGenerator>,
    design_name: Option<&str>,
    options: &DsnReadOptions,
) -> BoardReadResult {
    let _ = design_name;

    let text = match read_to_string(input) {
        Ok(text) => text,
        Err(error) => return BoardReadResult::IoError(error),
    };
    let scanner = DsnScanner::new(&text);
    let mut p = ReadScopeParameter::new(scanner, options);
    p.id_generator = id_generator.unwrap_or_default();

    // DsnReader.java:79-109 — the "(pcb <name>" header, an identical check to `DsnFile.read`.
    // The pcb-name token (`i == 2`) is captured into `pcbTokenName` and never validated: Java's
    // `ok` stays true for that iteration whatever the token is.
    match read_pcb_header(&mut p) {
        Ok(HeaderResult::Ok) => {}
        Ok(HeaderResult::NotDsn) => return not_a_dsn_file(),
        Err(error) => return parse_error(&error),
    }

    // DsnReader.java:125-126.
    let read_ok = match read_scope(ScopeKeyword::Pcb, &mut p) {
        Ok(read_ok) => read_ok,
        // totalized: a scanner `Error` (`SpecctraDsnStreamReader.zzScanError`, "could not match
        // input") is an `Error`, not an `IOException`, so Java's `catch (IOException)` blocks do
        // not stop it and it propagates out of `readBoard` uncaught. The port reports it as the
        // parse failure it is, which is the row Java's *other* unreadable inputs already take.
        Err(error) => return parse_error(&error),
    };

    let coordinate_transform = p.coordinate_transform;
    let mut board = p.board.take();
    let warnings = std::mem::take(&mut p.warnings);

    if read_ok {
        // DsnReader.java:132-136: apply power-plane autoroute settings if the DSN had no
        // `(autoroute …)` scope. `DsnFile.adjustPlaneAutorouteSettings` null-checks its argument
        // (DsnFile.java:33-35), which is why a `None` board simply skips it here.
        if p.autoroute_settings.is_none()
            && let Some(board) = board.as_mut()
            && let Err(error) = dsn_file::adjust_plane_autoroute_settings(board)
        {
            return parse_error(&error);
        }
        // fixed: T4 (#91) — Java has no branch here: `readScope` answered `true` and
        // `readBoard` returns `Success` whether or not the file actually ended. The port asks
        // the reader whether it ran out of input inside an unclosed scope (see
        // `read_scope_generic`) and says so, without withholding the board.
        if let Some(diagnostic) = p.truncation.take() {
            return BoardReadResult::Partial {
                board: board.map(Box::new),
                metadata: None,
                warnings,
                coordinate_transform,
                diagnostic,
            };
        }
        // DsnReader.java:146: `metadata` is **null** on this path.
        BoardReadResult::Success {
            board: board.map(Box::new),
            metadata: None,
            warnings,
            coordinate_transform,
        }
    } else if !p.board_outline_ok {
        // DsnReader.java:147-157.
        BoardReadResult::OutlineMissing {
            board: board.map(Box::new),
            metadata: None,
            warnings,
            coordinate_transform,
        }
    } else {
        // DsnReader.java:159.
        BoardReadResult::ParseError {
            location: PCB.to_string(),
            detail: "DSN structure parsing failed".to_string(),
        }
    }
}

/// `DsnReader.readMetadata(InputStream)` (DsnReader.java:182-280): parse only `(parser …)`,
/// `(resolution …)` and `(structure …)`, then stop.
///
/// This deliberately **re-implements** the PCB-level loop rather than calling [`read_board`]:
/// `break outer` after the `structure` scope (:249) is what makes it fast on a large DSN, and
/// `DsnReaderMetadataTest.readMetadataCompletesWithinReasonableTimeOnLargeDsn` asserts it.
///
/// Java answers a whole [`BoardReadResult`], not a bare metadata record — a `Success` whose
/// `board` may be `None` (no valid outline) and whose `metadata` is always `Some`.
///
/// **This function never answers [`BoardReadResult::Partial`]**, and that is deliberate rather
/// than an oversight. #91's truncation flag is set by `read_scope_generic`'s end-of-file branch,
/// and this loop does not use `read_scope_generic` at all: it is the hand-written PCB-level loop
/// above, whose whole point is to `break` at the end of the `(structure …)` scope with most of
/// the file **deliberately** unread. "The input ended early" and "we stopped early on purpose"
/// would be indistinguishable here, so a `Partial` from this path would say nothing. A caller
/// that needs to know whether the file is whole reads it with [`read_board`].
///
/// `layer_count` prefers `layerStructure.layers.length` and falls back to
/// `board.getLayerCount()` (:261-266); with neither it stays 0.
pub fn read_metadata(input: impl Read) -> BoardReadResult {
    let options = DsnReadOptions::default();
    let text = match read_to_string(input) {
        Ok(text) => text,
        Err(error) => return BoardReadResult::IoError(error),
    };
    let scanner = DsnScanner::new(&text);
    let mut p = ReadScopeParameter::new(scanner, &options);

    // DsnReader.java:194-214 — the same three-token check as `readBoard`.
    match read_pcb_header(&mut p) {
        Ok(HeaderResult::Ok) => {}
        Ok(HeaderResult::NotDsn) => return not_a_dsn_file(),
        Err(error) => return parse_error(&error),
    }

    // DsnReader.java:221-254: the custom PCB-level loop.
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        next_token = match p.scanner.next_token() {
            Ok(token) => token,
            Err(error) => return parse_error(&error),
        };
        // EOF or end of the `(pcb …)` scope (:232-234).
        if next_token.is_none() || next_token == Some(Token::Close) {
            break;
        }
        if prev_token != Some(Token::Open) {
            continue;
        }
        // Java ignores every one of these three `readScope` return values (DsnReader.java:239,
        // 242, 248 — ":247 Return value is ignored — we extract whatever was populated"), and so
        // does this port: an `Ok(false)` falls through and the metadata is built from whatever
        // the scope managed to fill in. Only a Rust `Err` — a scanner `Error`, which Java lets
        // propagate uncaught, or a `BoardError` out of `createBoard`, which Java cannot produce —
        // becomes a `ParseError` here. So the port reports a failure in exactly the cases where
        // Java either crashes or has no equivalent, never where Java carries on.
        let result = match next_token {
            // Populates `hostCad`, `hostVersion`, `stringQuote` (:236-239).
            Some(Token::Kw(Keyword::ParserScope)) => header::read_parser_scope(&mut p),
            // Populates `unit`, `resolution` (:240-242).
            Some(Token::Kw(Keyword::ResolutionScope)) => header::read_resolution_scope(&mut p),
            // Populates `layerStructure`, `snapAngle`, `autorouteSettings` and creates the board
            // (:243-249). The return value is ignored — whatever was populated is extracted.
            Some(Token::Kw(Keyword::StructureScope)) => {
                let result = structure::read_structure_scope(&mut p);
                if let Err(error) = result {
                    return parse_error(&error);
                }
                // stop here — skip library, placement, network, wiring
                break;
            }
            _ => skip_scope(&mut p.scanner).map(|_| true),
        };
        if let Err(error) = result {
            return parse_error(&error);
        }
    }

    // DsnReader.java:261-279.
    let layer_count = match (&p.layer_structure, &p.board) {
        (Some(layer_structure), _) => layer_structure.layers.len(),
        (None, Some(board)) => board.get_layer_count(),
        (None, None) => 0,
    };
    let metadata = BoardMetadata {
        host_cad: p.host_cad.clone(),
        host_version: p.host_version.clone(),
        layer_count,
        unit: p.unit,
        resolution: p.resolution,
        snap_angle: p.snap_angle,
        router_settings: p.autoroute_settings.clone(),
    };
    BoardReadResult::Success {
        board: p.board.take().map(Box::new),
        metadata: Some(metadata),
        warnings: std::mem::take(&mut p.warnings),
        coordinate_transform: p.coordinate_transform,
    }
}

/// Whether the three-token header check passed.
enum HeaderResult {
    /// `(`, `pcb`, then whatever token follows.
    Ok,
    /// One of the first two tokens was wrong.
    NotDsn,
}

/// The three-token `(pcb <name>` check both entry points run (DsnReader.java:83-109 ==
/// :194-214, character for character apart from `readBoard`'s `pcbTokenName` capture, which
/// feeds only the not-ported log line).
///
/// `scanner.yybegin(NAME)` after the `pcb` keyword is load-bearing: without it a design name
/// like `123abc` does not lex as a single token.
fn read_pcb_header(p: &mut ReadScopeParameter<'_>) -> Result<HeaderResult, DsnError> {
    for i in 0..3 {
        let token = p.scanner.next_token()?;
        let ok = match i {
            0 => token == Some(Token::Open),
            1 => {
                let ok = token == Some(Token::Kw(Keyword::PcbScope));
                // switch the scanner to NAME mode so the pcb-name token is consumed cleanly
                p.scanner.yybegin(LexicalState::Name);
                ok
            }
            // i == 2: the design name string immediately following "(pcb". Never validated.
            _ => true,
        };
        if !ok {
            return Ok(HeaderResult::NotDsn);
        }
    }
    Ok(HeaderResult::Ok)
}

/// DsnReader.java:106-107 / :211-212, verbatim.
fn not_a_dsn_file() -> BoardReadResult {
    BoardReadResult::ParseError {
        location: PCB.to_string(),
        detail: "Not a Specctra DSN file: expected '(pcb <name>' header".to_string(),
    }
}

/// A [`DsnError`] the Java code either could not produce or did not catch, reported as the parse
/// failure it is. The location is Java's constant `"(pcb"`; the detail is the error's own
/// message, since Java has no string for these.
fn parse_error(error: &DsnError) -> BoardReadResult {
    BoardReadResult::ParseError {
        location: PCB.to_string(),
        detail: error.to_string(),
    }
}

/// Java's `new InputStreamReader(inputStream)` (SpecctraDsnStreamReader.java:613), which decodes
/// with the default charset (UTF-8 since JDK 18) and replaces malformed input rather than
/// failing — which is what [`String::from_utf8_lossy`] does too.
///
/// Java refills its 16 MiB `zzBuffer` lazily; the port reads the stream once (see
/// `DsnScanner::new`'s note and docs/java-quirks.md). The observable consequence is *where* an
/// IO error surfaces: here, rather than at the token that first needed the bytes. Both land in
/// [`BoardReadResult::IoError`] for `read_board`'s first three tokens, which is the only place
/// Java surfaces one at all.
fn read_to_string(mut input: impl Read) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
