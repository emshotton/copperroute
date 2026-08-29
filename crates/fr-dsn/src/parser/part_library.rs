//! `io/specctra/parser/PartLibrary.java` — the `part_library` scope (`logical_part` and
//! `logical_part_mapping` entries used for pin/gate swap).
//!
//! Plan ruling 7: ported only as far as DSN read/write needs. `PartLibrary.readScope` fills
//! [`ReadScopeParameter::logical_parts`] and [`ReadScopeParameter::logical_part_mappings`] —
//! *not* `BoardLibrary::logical_parts` directly; `Network.insertLogicalParts`
//! (Network.java:844-897) is what turns those two lists into `LogicalParts` entries and hangs
//! them off the board's components, and that is Task 9's. `PartLibrary.writeScope` is ported in
//! full because `DsnWriter.writePcbScope` calls it unconditionally.
//!
//! Everything else Java does with logical parts (the pin/gate-swap consumers) already lives in
//! `fr-board`.
// not ported: PartLibrary.LogicalPartMapping, PartLibrary.PartPin, PartLibrary.LogicalPart's
// private constructors — Rust struct literals, with the three records renamed below.

use std::cmp::Ordering;

use crate::error::DsnError;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};

/// `PartLibrary.LogicalPartMapping` (PartLibrary.java:302-314): the components a logical part is
/// mapped onto.
// renamed: PartLibrary.LogicalPartMapping -> DsnLogicalPartMapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnLogicalPartMapping {
    /// `LogicalPartMapping.name`.
    pub name: String,
    /// `LogicalPartMapping.components`, Java's `SortedSet<String>` (a `TreeSet`, i.e. sorted by
    /// `String.compareTo` and duplicate-free). Kept as a sorted, deduplicated `Vec<String>`:
    /// `Network.searchLibPackage` reads `components.getFirst()`, so the order is observable.
    pub components: Vec<String>,
}

/// `PartLibrary.PartPin` (PartLibrary.java:316-336): one `(pin …)` entry of a `logical_part`.
///
/// Distinct from `fr_board::PartPin`, which carries the resolved `pin_index` this record does
/// not have yet.
// renamed: PartLibrary.PartPin -> DsnPartPin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnPartPin {
    /// `PartPin.pinName`.
    pub pin_name: String,
    /// `PartPin.gateName`.
    pub gate_name: String,
    /// `PartPin.gateSwapCode`.
    pub gate_swap_code: i32,
    /// `PartPin.gatePinName`.
    pub gate_pin_name: String,
    /// `PartPin.gatePinSwapCode`.
    pub gate_pin_swap_code: i32,
}

/// `PartLibrary.LogicalPart` (PartLibrary.java:338-350): one `logical_part` scope, as read.
// renamed: PartLibrary.LogicalPart -> DsnLogicalPart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnLogicalPart {
    /// `LogicalPart.name`.
    pub name: String,
    /// `LogicalPart.partPins`, in file order (a `LinkedList` in Java).
    pub part_pins: Vec<DsnPartPin>,
}

/// `PartLibrary.readScope` (PartLibrary.java:83-124).
// renamed: PartLibrary.readScope -> read_part_library_scope.
pub fn read_part_library_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut next_token: Option<Token> = None;
    loop {
        let prev_token = next_token;
        next_token = p.scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // "PartLibrary.read_scope: unexpected end of file".
            return Ok(false);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        if prev_token == Some(Token::Open) {
            match token {
                Token::Kw(Keyword::LogicalPartMapping) => {
                    let Some(next_mapping) = read_logical_part_mapping(&mut p.scanner)? else {
                        return Ok(false);
                    };
                    p.logical_part_mappings.push(next_mapping);
                }
                Token::Kw(Keyword::LogicalPart) => {
                    let Some(next_part) = read_logical_part(&mut p.scanner)? else {
                        return Ok(false);
                    };
                    p.logical_parts.push(next_part);
                }
                _ => {
                    skip_scope(&mut p.scanner)?;
                }
            }
        }
    }
    Ok(true)
}

/// `PartLibrary.readLogicalPartMapping` (PartLibrary.java:127-182): `(logical_part_mapping <name>
/// (comp <component>*))`. `None` is Java's `null`.
fn read_logical_part_mapping(
    scanner: &mut DsnScanner,
) -> Result<Option<DsnLogicalPartMapping>, DsnError> {
    let Some(Token::Str(name)) = scanner.next_token()? else {
        // "PartLibrary.read_logical_part_mapping: string expected".
        return Ok(None);
    };
    if scanner.next_token()? != Some(Token::Open) {
        // "PartLibrary.read_logical_part_mapping: open bracket expected".
        return Ok(None);
    }
    if scanner.next_token()? != Some(Token::Kw(Keyword::ComponentScope)) {
        // "PartLibrary.read_logical_part_mapping: Keyword.COMPONENT_SCOPE expected".
        return Ok(None);
    }
    let mut result: Vec<String> = Vec::new();
    loop {
        scanner.yybegin(LexicalState::Name);
        let next_token = scanner.next_token()?;
        if next_token == Some(Token::Close) {
            break;
        }
        let Some(Token::Str(component)) = next_token else {
            // "PartLibrary.read_logical_part_mapping: string expected". Java's `null` at end of
            // file also lands here, so the port needs no extra end-of-file branch.
            return Ok(None);
        };
        sorted_set_add(&mut result, component);
    }
    if scanner.next_token()? != Some(Token::Close) {
        // "PartLibrary.read_logical_part_mapping: closing bracket expected".
        return Ok(None);
    }
    Ok(Some(DsnLogicalPartMapping {
        name,
        components: result,
    }))
}

/// `PartLibrary.readLogicalPart` (PartLibrary.java:184-237): `(logical_part <name> (pin …)*)`.
// Java bug: PartLibrary.readLogicalPart declares `boolean readOk = true;` inside the loop and
// tests `if (!readOk) return null;` at its end (PartLibrary.java:220,232-234) without ever
// assigning `false` — dead code that the compiler cannot warn about. Reproduced by omitting it,
// which is observably identical; see docs/java-quirks.md.
fn read_logical_part(scanner: &mut DsnScanner) -> Result<Option<DsnLogicalPart>, DsnError> {
    let mut part_pins: Vec<DsnPartPin> = Vec::new();
    let mut next_token = scanner.next_token()?;
    let Some(Token::Str(part_name)) = next_token.clone() else {
        // "PartLibrary.read_logical_part: string expected".
        return Ok(None);
    };
    scanner.set_scope_identifier(&part_name);
    loop {
        let prev_token = next_token;
        next_token = scanner.next_token()?;
        let Some(token) = next_token.clone() else {
            // "PartLibrary.read_logical_part: unexpected end of file".
            return Ok(None);
        };
        if token == Token::Close {
            // end of scope
            break;
        }
        if prev_token == Some(Token::Open) {
            if token == Token::Kw(Keyword::Pin) {
                let Some(current_part_pin) = read_part_pin(scanner)? else {
                    return Ok(None);
                };
                part_pins.push(current_part_pin);
            } else {
                skip_scope(scanner)?;
            }
        }
    }
    Ok(Some(DsnLogicalPart {
        name: part_name,
        part_pins,
    }))
}

/// `PartLibrary.readPartPin` (PartLibrary.java:239-300): `(pin <pinName> <int> <gateName>
/// <gateSwapCode> <gatePinName> <gatePinSwapCode> …)`.
///
/// The integer between the pin name and the gate name is read and discarded, exactly as Java
/// does — the writer emits a literal `0` there (PartLibrary.java:66).
fn read_part_pin(scanner: &mut DsnScanner) -> Result<Option<DsnPartPin>, DsnError> {
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(pin_name)) = scanner.next_token()? else {
        // "PartLibrary.read_part_pin: string expected".
        return Ok(None);
    };
    scanner.set_scope_identifier(&pin_name);
    let Some(Token::Int(_)) = scanner.next_token()? else {
        // "PartLibrary.read_part_pin: integer expected".
        return Ok(None);
    };
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(gate_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.set_scope_identifier(&gate_name);
    let Some(Token::Int(gate_swap_code)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(gate_pin_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.set_scope_identifier(&gate_pin_name);
    let Some(Token::Int(gate_pin_swap_code)) = scanner.next_token()? else {
        return Ok(None);
    };
    // overread subgates
    loop {
        match scanner.next_token()? {
            Some(Token::Close) | None => break,
            _ => {}
        }
    }
    Ok(Some(DsnPartPin {
        pin_name,
        gate_name,
        // Java's tokens are `Integer`s here, so the values already fit an `i32`.
        gate_swap_code: i32::try_from(gate_swap_code).unwrap_or(i32::MAX),
        gate_pin_name,
        gate_pin_swap_code: i32::try_from(gate_pin_swap_code).unwrap_or(i32::MAX),
    }))
}

/// `PartLibrary.writeScope` (PartLibrary.java:23-81): the mappings first, then the parts.
///
/// The `logical_part` literal is 2.3.0's (plan ruling 1); the clone's HEAD writes `logicalPart `
/// there (PartLibrary.java:58), which its own lexer cannot read back.
// renamed: PartLibrary.writeScope -> write_part_library_scope.
pub fn write_part_library_scope(p: &mut WriteScopeParameter<'_>) {
    if p.board.library.logical_parts.count() == 0 {
        return;
    }
    p.file.start_scope_nl();
    p.file.write("part_library");

    // write the logical part mappings

    for i in 1..=p.board.library.logical_parts.count() {
        let current_part_name = p.board.library.logical_parts.get(i).name.clone();
        p.file.start_scope_nl();
        p.file.write("logical_part_mapping ");
        p.identifier_type.write(&current_part_name, &mut p.file);
        p.file.new_line();
        p.file.write("(comp");
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        for j in 1..=p.board.components.count() as i32 {
            let current_component = p.board.components.get(j);
            if current_component
                .get_logical_part()
                .is_some_and(|part| part == i)
            {
                let name = current_component.name.clone();
                p.file.write(" ");
                p.file.write(&name);
            }
        }
        p.file.write(")");
        p.file.end_scope();
    }

    // write the logical parts.

    for i in 1..=p.board.library.logical_parts.count() {
        let current_part = p.board.library.logical_parts.get(i).clone();

        p.file.start_scope_nl();
        p.file.write("logical_part ");
        p.identifier_type.write(&current_part.name, &mut p.file);
        p.file.new_line();
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        for j in 0..current_part.pin_count() as i32 {
            p.file.new_line();
            // totalized: PartLibrary.writeScope — Java dereferences `currentPart.getPin(j)`
            // unchecked (PartLibrary.java:63-65), and `LogicalPart.getPin(int)` warns and
            // returns `null` out of range. The port skips the pin. Unreachable: the loop bound
            // is `currentPart.pinCount()`.
            let Some(current_pin) = current_part.get_pin(j) else {
                continue;
            };
            p.file.write("(pin ");
            p.identifier_type.write(&current_pin.pin_name, &mut p.file);
            p.file.write(" 0 ");
            p.identifier_type.write(&current_pin.gate_name, &mut p.file);
            p.file.write(" ");
            p.file.write(&current_pin.gate_swap_code.to_string());
            p.file.write(" ");
            p.identifier_type
                .write(&current_pin.gate_pin_name, &mut p.file);
            p.file.write(" ");
            p.file.write(&current_pin.gate_pin_swap_code.to_string());
            p.file.write(")");
        }
        p.file.end_scope();
    }
    p.file.end_scope();
}

/// `java.util.TreeSet<String>.add`: insert in `String.compareTo` order, ignoring duplicates.
fn sorted_set_add(set: &mut Vec<String>, value: String) {
    match set.binary_search_by(|probe| java_string_cmp(probe, &value)) {
        Ok(_) => {}
        Err(index) => set.insert(index, value),
    }
}

/// `String.compareTo`: lexicographic over UTF-16 **code units**, not Unicode scalar values.
///
/// The two orders differ only when a supplementary character (U+10000 and above, encoded as a
/// surrogate pair whose lead unit is in U+D800..U+DBFF) is compared against a character in
/// U+E000..U+FFFF; `str`'s own `Ord` would sort those the other way round.
///
/// `pub(crate)` since Task 9: `Net.Id.compareTo` and `Net.Pin.compareTo` are `String.compareTo`
/// chains too, and their `TreeMap`/`TreeSet` order is observable (see `parser/network.rs`).
pub(crate) fn java_string_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_set_add_sorts_and_deduplicates_like_a_treeset() {
        let mut set = Vec::new();
        for name in ["U10", "U2", "U1", "U2"] {
            sorted_set_add(&mut set, name.to_string());
        }
        // `String.compareTo` is lexicographic, so "U10" sorts before "U2".
        assert_eq!(set, ["U1", "U10", "U2"]);
    }

    #[test]
    fn java_string_cmp_orders_by_utf16_code_units() {
        // U+FFFD (one code unit, 0xFFFD) vs U+10000 (surrogate pair, lead unit 0xD800):
        // Java sorts the surrogate pair *first*; Rust's `str` `Ord` would not.
        assert_eq!(java_string_cmp("\u{10000}", "\u{FFFD}"), Ordering::Less);
        assert!("\u{10000}" > "\u{FFFD}");
    }
}
