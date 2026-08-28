//! Small, closely related header/global-settings scopes with no dedicated file of their own in
//! the plan's file structure: `io/specctra/parser/{Parser,Resolution,Unit,PlaceControl}.java`.
//!
//! `Parser` reads the `parser` scope (`host_cad`/`host_version`/`string_quote`/
//! `write_resolution`, i.e. the file's own metadata about the tool that wrote it); `Resolution`
//! reads the `resolution` scope (`unit`, `resolution`); `Unit` writes the `unit` scope;
//! `PlaceControl` reads the `place_control` scope (`flip_style`/`rotate_first`). All four are
//! tiny compared to `structure`/`network`/`wiring`/`library`/`placement`, hence the shared file.
//!
//! **Writer literals are the 2.3.0 ones** (plan ruling 1): `Parser.writeScope` at the Java
//! clone's HEAD writes `"(stringQuote "`, `"(hostCad "`, `"(hostVersion "` and
//! `"(writeResolution "`; the pinned `tools/freerouting-2.3.0.jar` writes `"(string_quote "`,
//! `"(host_cad "`, `"(host_version "` and `"(write_resolution "`, and those are what this port
//! emits. Verified with `javap -c` on `app/freerouting/io/specctra/parser/Parser.class` out of
//! that jar, and against `tests/reference/Issue143-rpi_splitter/roundtrip.dsn:3-7`.

use std::io::Write;

use fr_board::{Communication, Unit, WriteResolution};

use crate::error::DsnError;
use crate::format::{IdentifierType, IndentFileWriter};
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::read_string_scope;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

/// `Parser.readWriteSolution` (Parser.java:18-51) — the `(write_resolution <name> <int>)` scope.
/// (Java's own spelling of the method name; it reads a *resolution*.)
fn read_write_solution(
    scope_parameter: &mut ReadScopeParameter<'_>,
) -> Result<Option<WriteResolution>, DsnError> {
    let Some(Token::Str(resolution_string)) = scope_parameter.scanner.next_token()? else {
        return Ok(None);
    };
    let Some(Token::Int(resolution_value)) = scope_parameter.scanner.next_token()? else {
        return Ok(None);
    };
    if scope_parameter.scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    Ok(Some(WriteResolution::new(
        resolution_string,
        resolution_value as i32,
    )))
}

/// `Parser.readConstant` (Parser.java:53-89) — the `(constant <name> <value>)` scope. Both
/// tokens are read in the `NAME` lexical state, so a value like `123abc` arrives as one string.
fn read_constant(
    scope_parameter: &mut ReadScopeParameter<'_>,
) -> Result<Option<Vec<String>>, DsnError> {
    let mut result = Vec::with_capacity(2);
    for _ in 0..2 {
        scope_parameter.scanner.yybegin(LexicalState::Name);
        let Some(Token::Str(value)) = scope_parameter.scanner.next_token()? else {
            return Ok(None);
        };
        result.push(value);
    }
    if scope_parameter.scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    Ok(Some(result))
}

/// `Parser.readQuoteChar` (Parser.java:147-168) — the `(string_quote <char>)` scope. The lexer
/// enters `IGNORE_QUOTE` when it recognises the `string_quote` keyword, so the quote character
/// itself comes back as an ordinary string rather than opening a quoted token.
fn read_quote_char(scanner: &mut DsnScanner) -> Result<Option<String>, DsnError> {
    let Some(Token::Str(result)) = scanner.next_token()? else {
        return Ok(None);
    };
    if scanner.next_token()? != Some(Token::Close) {
        return Ok(None);
    }
    Ok(Some(result))
}

/// `Parser.writeScope` (Parser.java:92-145), with the 2.3.0 literals — see the module docs.
///
/// `(space_in_quoted_tokens on)` is emitted unconditionally in non-reduced mode and has no
/// read-side counterpart at all: [`read_parser_scope`] meets it as an unknown scope and skips
/// it.
// renamed: Parser.writeScope -> write_parser_scope (this file holds three classes' scopes).
pub fn write_parser_scope<W: Write>(
    file: &mut IndentFileWriter<W>,
    parser_info: &Communication,
    identifier_type: &IdentifierType,
    reduced: bool,
) {
    file.start_scope_nl();
    file.write("parser");
    if !reduced {
        file.new_line();
        file.write("(string_quote ");
        file.write(&parser_info.string_quote);
        file.write(")");
        file.new_line();
        file.write("(space_in_quoted_tokens on)");
    }
    if let Some(host_cad) = &parser_info.host_cad {
        file.new_line();
        file.write("(host_cad ");
        identifier_type.write(host_cad, file);
        file.write(")");
    }
    if let Some(host_version) = &parser_info.host_version {
        file.new_line();
        file.write("(host_version ");
        identifier_type.write(host_version, file);
        file.write(")");
    }
    for current_constant in &parser_info.constants {
        file.new_line();
        file.write("(constant ");
        for part in current_constant {
            identifier_type.write(part, file);
            file.write(" ");
        }
        file.write(")");
    }
    if let Some(write_resolution) = &parser_info.write_resolution {
        file.new_line();
        file.write("(write_resolution ");
        // `charName.substring(0, 1)` (Parser.java:134) — the unit's initial letter only.
        // totalized: Parser.writeScope throws `StringIndexOutOfBoundsException` on an empty
        // `charName`; the port's `chars().next()` writes nothing instead. No reachable caller
        // observes it: the lexer cannot return a zero-length `Str` for this scope, so
        // `read_write_solution` cannot build such a `WriteResolution`. See docs/java-quirks.md.
        if let Some(first) = write_resolution.char_name.chars().next() {
            file.write(&first.to_string());
        }
        file.write(" ");
        file.write(&write_resolution.positive_int.to_string());
        file.write(")");
    }
    if !reduced {
        file.new_line();
        file.write("(generated_by_freerouting)");
    }
    file.end_scope();
}

/// `Parser.readScope` (Parser.java:170-227): the `parser` scope.
///
/// Java's `readOk` local is dead — it is initialised to `true` inside the loop and never
/// assigned, so its `if (!readOk) return false;` (Parser.java:222-224) can never fire; the port
/// drops it rather than transcribing an unreachable branch.
// renamed: Parser.readScope -> read_parser_scope (this file holds three classes' scopes).
pub fn read_parser_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // Java: "unexpected end of file".
            return Ok(false);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        if prev_token == Some(Token::Open) {
            match token {
                Token::Kw(Keyword::StringQuote) => {
                    let Some(quote_char) = read_quote_char(&mut p.scanner)? else {
                        return Ok(false);
                    };
                    p.string_quote = quote_char;
                }
                Token::Kw(Keyword::HostCad) => {
                    p.host_cad = Some(read_string_scope(&mut p.scanner)?);
                }
                Token::Kw(Keyword::HostVersion) => {
                    p.host_version = Some(read_string_scope(&mut p.scanner)?);
                }
                Token::Kw(Keyword::Constant) => {
                    if let Some(current_constant) = read_constant(p)? {
                        p.constants.push(current_constant);
                    }
                }
                Token::Kw(Keyword::WriteResolution) => {
                    p.write_resolution = read_write_solution(p)?;
                }
                // Java bug: this arm is dead. The generated DFA never returns
                // `Keyword.GENERATED_BY_FREEROUTING` for the lexeme `generated_by_freerouting` —
                // JVM-verified against `tools/freerouting-2.3.0.jar`, where `next_token()` over
                // `(generated_by_freerouting)` yields `OPEN_BRACKET`, the **String**
                // `"generated_by_freerouting"`, `CLOSED_BRACKET`. So the scope falls through to
                // the `skipScope` arm below and `dsnFileGeneratedByHost` stays `true` even for a
                // file freerouting itself wrote. Reproduced, not fixed. See docs/java-quirks.md.
                Token::Kw(Keyword::GeneratedByFreerouting) => {
                    p.dsn_file_generated_by_host = false;
                    // skip the closing bracket
                    let _ = skip_scope(&mut p.scanner)?;
                }
                _ => {
                    let _ = skip_scope(&mut p.scanner)?;
                }
            }
        }
    }
    Ok(true)
}

/// `Resolution.writeScope` (Resolution.java:17-25).
// renamed: Resolution.writeScope -> write_resolution_scope.
pub fn write_resolution_scope<W: Write>(
    file: &mut IndentFileWriter<W>,
    board_communication: &Communication,
) {
    file.new_line();
    file.write("(resolution ");
    file.write(&board_communication.unit.to_string());
    file.write(" ");
    file.write(&board_communication.resolution.to_string());
    file.write(")");
}

/// `Resolution.readScope` (Resolution.java:27-72): `(resolution <unit> <int>)`, writing both
/// into the shared [`ReadScopeParameter`].
///
/// Assignment order differs harmlessly from Java's: Java assigns `scopeParameter.unit =
/// Unit.fromString(...)` and *then* tests it for `null` (Resolution.java:39-47), so a bad unit
/// name leaves `scopeParameter.unit` **null** before the `false` return; the port validates
/// first and leaves the field at its previous value. No caller observes it — every one of Java's
/// `readScope` callers abandons the read on `false` (`ScopeKeyword.readScope`
/// propagates it straight out), and Java's `unit` would be a `null` nothing may read anyway.
// renamed: Resolution.readScope -> read_resolution_scope.
pub fn read_resolution_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    // read the unit
    let Some(Token::Str(unit_name)) = p.scanner.next_token()? else {
        return Ok(false);
    };
    let Some(unit) = Unit::from_string(&unit_name) else {
        return Ok(false);
    };
    p.unit = unit;
    // read the scale factor
    let Some(Token::Int(resolution)) = p.scanner.next_token()? else {
        return Ok(false);
    };
    p.resolution = resolution as i32;
    // overread the closing bracket
    if p.scanner.next_token()? != Some(Token::Close) {
        return Ok(false);
    }
    Ok(true)
}

/// `Unit.writeScope` (Unit.java:15-21).
// renamed: Unit.writeScope -> write_unit_scope.
pub fn write_unit_scope<W: Write>(file: &mut IndentFileWriter<W>, unit: Unit) {
    file.new_line();
    file.write("(unit ");
    file.write(&unit.to_string());
    file.write(")");
}

/// `Unit.readScope` (Unit.java:23-26): `return false;`, unconditionally.
///
/// `Unit` extends `ScopeKeyword` but is never assigned to a `Keyword` constant and the DSN
/// scanner has no `unit` lexeme, so the `(unit …)` scope is skipped as an unknown scope and this
/// override is unreachable in Java. It is ported anyway, verbatim, because it is a `public`
/// method of an in-scope class.
// renamed: Unit.readScope -> read_unit_scope.
pub fn read_unit_scope(_p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    Ok(false)
}

// added in Plan 3: PlaceControl.readScope
/// Stub for `PlaceControl.readScope` (PlaceControl.java) — replaced with the real reader by a
/// later task; for now this just discards the scope's body.
pub fn read_place_control_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let _ = skip_scope(&mut p.scanner)?;
    Ok(true)
}
