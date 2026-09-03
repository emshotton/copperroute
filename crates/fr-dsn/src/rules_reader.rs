//! `io/specctra/RulesReader.java` — applies a Specctra `.rules` file to an **existing**
//! [`Board`].
//!
//! # What this reader is not
//!
//! Like [`crate::ses_reader`], it never builds a board. It knows eight top-level scopes —
//! `rule`, `layer`, `padstack`, `via`, `via_rule`, `class`, `snap_angle` and
//! `autoroute_settings` — and hands everything else to [`skip_scope`]. The board it mutates must
//! already carry the nets, the padstack library and the layer structure the file refers to, which
//! is why every caller reads the design's `.dsn` first.
//!
//! # Deviation from Java: the coordinate transform is a parameter
//!
//! `RulesReader.read` reads `board.communication.coordinateTransform` (RulesReader.java:113) and
//! threads it through `applyRules`, `Library.readPadstackScope` and `Network.insertNetClass`.
//! This port's `fr-board` [`Board`] has no such field — Plan 3 ruling A keeps
//! [`CoordinateTransform`] in `fr-dsn`, and [`crate::read_board`] hands it back on the
//! [`crate::BoardReadResult`] instead — so [`read`] takes it explicitly. Same deviation, same
//! reason as [`crate::ses_reader::read`] and [`crate::dsn_writer::write`].
//!
//! # Three of these entry points are clone-HEAD-only, so their tests are not jar-pinned
//!
//! `javap -p` on `tools/freerouting-2.3.0.jar` shows exactly one public method on
//! `io.specctra.RulesReader`: `read(InputStream, String, BasicBoard)`. The four-argument `read`
//! (with `RouterSettings`), [`read_router_settings`] and `discoverLayerStructure` are **clone-HEAD
//! additions** and do not exist in the pinned jar. So the JVM goldens in
//! `tests/rules_round_trip.rs` pin the three-argument [`read`] only; the tests for the other
//! three are read from `RulesReader.java` at HEAD and are *not* jar-verified. The same holds for
//! [`DsnRouterSettings::apply_new_values_from`] — see
//! [`crate::parser::autoroute_settings`]'s module docs.
//!
//! # `read` and `read_router_settings` are two different parsers
//!
//! [`read`] builds its layer structure from the **board** (`new LayerStructure(board.layerStructure)`,
//! RulesReader.java:112). [`read_router_settings`] has no board, so it runs
//! [`discover_layer_structure`] over the whole buffer first and parses a second time
//! (RulesReader.java:198-199). The pre-pass is not shared with [`read`] and must not be fused
//! into either main loop.

use std::io::Read;

use fr_board::Board;

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::java_round_to_int;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::DsnRouterSettings;
use crate::parser::autoroute_settings::read_autoroute_settings_scope;
use crate::parser::geometry::{DsnLayer, DsnLayerStructure};
use crate::parser::library::read_padstack_scope;
use crate::parser::network::{
    DsnRule, add_via_rule, insert_net_class, read_net_class_scope, read_rule_scope, read_via_info,
    read_via_rule,
};
use crate::parser::scope_parameter::skip_scope;
use crate::parser::structure::{RuleLayerScope, read_snap_angle, set_clearance_rule};

/// `RulesReader.read(InputStream, String, BasicBoard, RouterSettings)`
/// (RulesReader.java:65-170): reads `input` and applies its rules to `board`.
///
/// `design_name` is the name the `(rules PCB <name>` header is expected to carry. A mismatch is
/// **not** fatal — Java logs it and reads on (RulesReader.java:100-110) — so this port ignores it
/// entirely, having no logger.
///
/// # Note
///
/// `design_name` is therefore an **inert parameter**: nothing in this function reads it, and no
/// input can make the result depend on it. It is kept so the signature matches Java's and so a
/// host that wants the mismatch message can compare the header itself; `let _ = design_name;` in
/// the body is deliberate, not an oversight.
///
/// `target_settings`, when given, receives the file's `(autoroute_settings …)` through
/// [`DsnRouterSettings::apply_new_values_from`] (RulesReader.java:154-157). Java's three-argument
/// overload (:47-49) is `target_settings: None`.
///
/// `ct` is the transform Java reads off `board.communication` — see the module docs.
///
/// # Returns
///
/// `Ok(true)` for Java's `true`: the `(rules …)` scope was closed cleanly. `Ok(false)` for every
/// one of Java's `false`s — a bad header token, end of file before the closing bracket, or an
/// `IOException` anywhere in the body (:121-124, :164-166). Nothing inside the scope can make the
/// read fail: an unparseable rule, an unknown layer, a missing padstack are each dropped and the
/// loop carries on, exactly as Java's `FRLogger.warn`-and-return helpers do.
///
/// # Errors
///
/// [`DsnError`] only for what Java does *not* catch: a scanner error (Java's
/// `zzScanError` throws an `Error`, which its `catch (IOException)` blocks do not stop) and the
/// initial read of the stream.
//
// renamed: RulesReader.read -> the free function `read` (the class is a private-constructor
// static holder, which Rust spells as a module).
// not ported: the `in == null` / `board == null` guards (RulesReader.java:67-75) — neither `impl
// Read` nor `&mut Board` can be null.
// not ported: `closeQuietly(in)` in the `finally` (:167-169, :369-375) — the stream is consumed
// by value and dropped, which closes a `File` the same way.
// not ported: the `designName` mismatch warning (:100-110); `fr-dsn` has no logger and Java
// continues regardless, so the parameter reaches nothing. It is kept so a host that wants the
// message can compare the header itself.
pub fn read(
    input: impl Read,
    design_name: &str,
    board: &mut Board,
    ct: &CoordinateTransform,
    mut target_settings: Option<&mut DsnRouterSettings>,
) -> Result<bool, DsnError> {
    let _ = design_name;
    let text = read_to_string(input)?;
    let mut scanner = DsnScanner::new(&text)?;

    // The "(rules PCB <name>" header (RulesReader.java:80-110). The name token is consumed but
    // never validated — a mismatch is non-fatal in Java too.
    if !read_rules_header(&mut scanner)? {
        return Ok(false);
    }

    // `new LayerStructure(board.layerStructure)` (RulesReader.java:112). Not
    // `discover_layer_structure`: only `read_router_settings` needs that.
    let layer_structure = DsnLayerStructure::from_board(board.layer_structure());
    let string_quote = board.communication.string_quote.clone();

    // RulesReader.java:116-162 — the top-level scope loop.
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            // "unexpected end of file" (RulesReader.java:125-129).
            return Ok(false);
        };
        if next_token == Token::Close {
            // end of the `(rules …)` scope — success (:130-133).
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Rule) => {
                    let rules = read_rule_scope(&mut scanner)?;
                    apply_rules(
                        rules.as_deref(),
                        board,
                        ct,
                        &string_quote,
                        RuleLayerScope::AllLayers,
                    );
                }
                Token::Kw(Keyword::Layer) => {
                    apply_layer_rules(&mut scanner, board, ct, &string_quote)?;
                }
                Token::Kw(Keyword::Padstack) => {
                    read_padstack_scope(
                        &mut scanner,
                        &layer_structure,
                        ct,
                        &mut board.library.padstacks,
                    )?;
                }
                Token::Kw(Keyword::Via) => apply_via_info(&mut scanner, board)?,
                Token::Kw(Keyword::ViaRule) => apply_via_rule(&mut scanner, board)?,
                Token::Kw(Keyword::Class) => {
                    apply_net_class(&mut scanner, &layer_structure, board, ct)?;
                }
                Token::Kw(Keyword::SnapAngle) => {
                    if let Some(snap_angle) = read_snap_angle(&mut scanner)? {
                        board.rules.trace_angle_restriction = snap_angle;
                    }
                }
                Token::Kw(Keyword::AutorouteSettings) => {
                    let parsed = read_autoroute_settings_scope(&mut scanner, &layer_structure)?;
                    if let (Some(target), Some(parsed)) = (target_settings.as_mut(), parsed) {
                        target.apply_new_values_from(&parsed);
                    }
                }
                _ => {
                    skip_scope(&mut scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }
    Ok(true)
}

/// `RulesReader.readRouterSettings(InputStream)` (RulesReader.java:180-236): the lighter variant
/// that extracts only the file's `(autoroute_settings …)` and needs no board.
///
/// `None` is Java's `null`: an empty stream, a bad header, no `autoroute_settings` scope, or an
/// `IOException` while scanning (:232-234).
///
/// # Errors
///
/// As [`read`]: only a scanner error or the initial stream read.
// renamed: RulesReader.readRouterSettings -> read_router_settings.
// not ported: the `in == null` guard (:181-183).
pub fn read_router_settings(input: impl Read) -> Result<Option<DsnRouterSettings>, DsnError> {
    let text = read_to_string(input)?;
    if text.is_empty() {
        // `data.length == 0` (RulesReader.java:194-196).
        return Ok(None);
    }

    // The pre-pass over the whole buffer, then a second scan of the same bytes (:198-199).
    let layer_structure = discover_layer_structure(&text)?;
    let mut scanner = DsnScanner::new(&text)?;
    if !read_rules_header(&mut scanner)? {
        return Ok(None);
    }

    let mut prev_was_open = false;
    loop {
        let next_token = scanner.next_token()?;
        // Java breaks on end of file *and* on the closing bracket (:221-223).
        if next_token.is_none() || next_token == Some(Token::Close) {
            break;
        }
        let is_open = next_token == Some(Token::Open);
        if prev_was_open {
            if next_token == Some(Token::Kw(Keyword::AutorouteSettings)) {
                return read_autoroute_settings_scope(&mut scanner, &layer_structure);
            }
            skip_scope(&mut scanner)?;
        }
        prev_was_open = is_open;
    }
    Ok(None)
}

/// `RulesReader.discoverLayerStructure(byte[])` (RulesReader.java:238-274): a full extra pass
/// over the buffer, collecting every name that follows a `(layer_rule` or `(layer` token into an
/// insertion-ordered unique set (Java's `LinkedHashSet`), so [`read_router_settings`] has a layer
/// structure to resolve `layer_rule` names against without a board.
///
/// With no names found at all it falls back to KiCad's two-layer stack, `F.Cu` / `B.Cu`
/// (:263-266). Every layer it builds is marked `is_signal = true` (:271).
///
/// # Errors
///
/// Only a construction failure of the scanner. Java swallows the `IOException` its loop can
/// raise (:259-261) and uses whatever it collected before the failure; this port does the same,
/// since [`DsnScanner::next_token`]'s only failure is the scanner error Java does not catch
/// either.
// renamed: discoverLayerStructure -> discover_layer_structure, taking the decoded text rather
// than Java's `byte[]` (the port decodes the stream once, in `read_to_string`), and `pub` where
// Java is `private static` (RulesReader.java:238). Widened deliberately: it is the only way to
// test the pre-pass in isolation (its Java caller returns `null` for most of the inputs that
// exercise it), and it is a useful standalone answer to "what layers does this rules file name?".
// It exists only at the clone's HEAD, so nothing in the pinned 2.3.0 jar can contradict the
// wider visibility.
pub fn discover_layer_structure(text: &str) -> Result<DsnLayerStructure, DsnError> {
    let mut layer_names: Vec<String> = Vec::new();
    let mut scanner = DsnScanner::new(text)?;
    let mut prev_was_open = false;
    loop {
        let Some(token) = scanner.next_token()? else {
            break;
        };
        let is_open = token == Token::Open;
        if prev_was_open
            && (token == Token::Kw(Keyword::LayerRule) || token == Token::Kw(Keyword::Layer))
        {
            scanner.yybegin(LexicalState::Name);
            // Java's `String.isBlank()` (:253) tests `Character.isWhitespace` over the whole
            // string; Rust's `char::is_whitespace` is the near-identical Unicode `White_Space`
            // property. The two disagree only on characters no Specctra layer name can hold.
            if let Some(Token::Str(name)) = scanner.next_token()?
                && !name.trim().is_empty()
                && !layer_names.contains(&name)
            {
                // `LinkedHashSet.add` — insertion-ordered, deduplicated (:239, :254).
                layer_names.push(name);
            }
        }
        prev_was_open = is_open;
    }

    if layer_names.is_empty() {
        layer_names.push("F.Cu".to_string());
        layer_names.push("B.Cu".to_string());
    }
    Ok(DsnLayerStructure::new(
        layer_names
            .into_iter()
            .enumerate()
            .map(|(i, name)| DsnLayer::new(name, i32::try_from(i).unwrap_or(i32::MAX), true))
            .collect(),
    ))
}

/// The three-token `(rules pcb` check plus the design-name token, shared by [`read`]
/// (RulesReader.java:80-99) and [`read_router_settings`] (:202-215), which run it identically.
///
/// `scanner.yybegin(NAME)` after the `pcb` keyword is load-bearing, exactly as in
/// [`crate::dsn_reader`]'s `(pcb <name>` check: without it a design name like `123abc` does not
/// lex as a single token.
fn read_rules_header(scanner: &mut DsnScanner) -> Result<bool, DsnError> {
    if scanner.next_token()? != Some(Token::Open) {
        return Ok(false);
    }
    if scanner.next_token()? != Some(Token::Kw(Keyword::Rules)) {
        return Ok(false);
    }
    if scanner.next_token()? != Some(Token::Kw(Keyword::PcbScope)) {
        return Ok(false);
    }
    scanner.yybegin(LexicalState::Name);
    // The design name itself; consumed, never validated (see `read`'s `not ported:` note).
    scanner.next_token()?;
    Ok(true)
}

/// `RulesReader.applyRules(Collection<Rule>, BasicBoard, String)` (RulesReader.java:280-306):
/// turns one `(rule …)` scope's width and clearance entries into board rules.
///
/// `scope` says which layers the rules land on. It is [`RuleLayerScope::AllLayers`] for the
/// file-level `(rule …)`, which genuinely applies to every layer, and
/// [`RuleLayerScope::One`] for a `(layer <name> (rule …))` whose name the board carries.
///
// Java bug: (#112) `RulesReader.applyRules` resolves the layer name itself and, when the board
// does not have that layer, warns "layer not found" and **does not return** (:286-290): the
// `int layerIndex` stays `-1`, which is exactly the sentinel the two branches below read as
// "all layers", so a stale `.rules` file silently overwrites the default trace width and the
// clearance matrix on the whole board.
//
// fixed: T4 (#112) — this function no longer resolves anything and cannot represent the
// failure: it takes a [`RuleLayerScope`], whose two variants are "all layers" and "this one".
// The lookup moved to [`apply_layer_rules`], the one caller that has a name to resolve, and a
// name the board does not carry makes it drop that scope's rules instead of widening them.
// renamed: RulesReader.applyRules -> apply_rules.
fn apply_rules(
    rules: Option<&[DsnRule]>,
    board: &mut Board,
    ct: &CoordinateTransform,
    string_quote: &str,
    scope: RuleLayerScope,
) {
    let Some(rules) = rules else {
        // `rules == null` (RulesReader.java:281-283).
        return;
    };
    for rule in rules {
        match rule {
            DsnRule::Width(value) => {
                let trace_half_width = java_round_to_int(ct.dsn_to_board(*value) / 2.0);
                match scope {
                    RuleLayerScope::AllLayers => {
                        board.rules.set_default_trace_half_widths(trace_half_width);
                    }
                    RuleLayerScope::One(layer) => board
                        .rules
                        .set_default_trace_half_width(layer, trace_half_width),
                }
            }
            DsnRule::Clearance(clearance_rule) => {
                set_clearance_rule(clearance_rule, scope, ct, &mut board.rules, string_quote);
            }
        }
    }
}

/// `RulesReader.applyLayerRules(IJFlexScanner, BasicBoard)` (RulesReader.java:308-338): the
/// `(layer <name> (rule …)* …)` scope, whose `(` and `layer` keyword the caller has consumed.
///
/// Anything inside that is not a `(rule …)` is skipped. A malformed body — a missing layer name,
/// a sub-scope that does not start with `(` — makes Java `return` without consuming the rest of
/// the scope, which leaves the outer loop mid-scope; reproduced verbatim, because the outer loop
/// then reads the stray tokens as top-level ones and is what makes a mangled `.rules` file still
/// answer `true`.
///
/// This is where the layer name is resolved (quirk #112, fixed in Plan 9 Task 4): a name the
/// board does not carry drops this scope's rules. The scope is still read to its end, so a stale
/// `.rules` file is still a `true` read of everything else in it — the rules that named a real
/// layer, the padstacks, the via rules and the net classes all land as before.
// renamed: RulesReader.applyLayerRules -> apply_layer_rules.
fn apply_layer_rules(
    scanner: &mut DsnScanner,
    board: &mut Board,
    ct: &CoordinateTransform,
    string_quote: &str,
) -> Result<(), DsnError> {
    let Some(Token::Str(layer_string)) = scanner.next_token()? else {
        // "String expected" (RulesReader.java:311-317).
        return Ok(());
    };
    // fixed: T4 (#112) — the resolution that Java left to `applyRules`, where a miss became
    // "all layers". `None` here means the board has no such layer, and every `(rule …)` inside
    // this scope is then read (so the outer loop stays in sync) and **dropped**.
    let layer_scope = board
        .layer_structure()
        .get_no(&layer_string)
        .map(RuleLayerScope::One);
    let mut next_token = scanner.next_token()?;
    while next_token != Some(Token::Close) {
        if next_token != Some(Token::Open) {
            // "'(' expected" (RulesReader.java:320-326).
            return Ok(());
        }
        next_token = scanner.next_token()?;
        if next_token == Some(Token::Kw(Keyword::Rule)) {
            let rules = read_rule_scope(scanner)?;
            if let Some(scope) = layer_scope {
                apply_rules(rules.as_deref(), board, ct, string_quote, scope);
            }
        } else {
            skip_scope(scanner)?;
        }
        next_token = scanner.next_token()?;
    }
    Ok(())
}

/// `RulesReader.applyViaInfo(IJFlexScanner, BasicBoard)` (RulesReader.java:340-350) — **ruling
/// H's site**.
///
/// Java is three lines: look the name up, remove the hit if there is one, append the new info.
/// The removal costs Java nothing because its `ViaRule`s hold `ViaInfo` object references
/// (ViaRule.java:21) — a rule that held the removed object keeps it, **detached** from
/// `viaInfos`. Since Plan 7 Task 0 a [`fr_board::ViaRule`] holds owned copies, so the port does
/// the same three lines and no rule moves:
/// [`BoardRules::replace_via_info`](fr_board::BoardRules::replace_via_info), next to
/// `ViaInfos::remove`'s note.
// renamed: RulesReader.applyViaInfo -> apply_via_info.
fn apply_via_info(scanner: &mut DsnScanner, board: &mut Board) -> Result<(), DsnError> {
    let Some(via_info) = read_via_info(scanner, board)? else {
        // `viaInfo == null` (RulesReader.java:342-344).
        return Ok(());
    };
    match board.rules.via_infos.get_no(via_info.get_name()) {
        Some(old_id) => {
            board.rules.replace_via_info(old_id, via_info);
        }
        // `viaInfos.add(viaInfo)` with nothing to remove (:349).
        None => {
            board.rules.via_infos.add(via_info);
        }
    }
    Ok(())
}

/// `RulesReader.applyViaRule(IJFlexScanner, BasicBoard)` (RulesReader.java:352-357).
///
/// Unlike `Network.insertViaRules`, which filters lists shorter than two entries
/// (Network.java:383), this path hands whatever it read straight to `add_via_rule` — so a
/// `(via_rule foo)` with no vias inserts an **empty** rule named `foo`, replacing any existing
/// rule of that name.
//
// totalized: RulesReader.applyViaRule — a `(via_rule)` with no names at all makes
// `Network.addViaRule`'s `it.next()` (Network.java:396) throw `NoSuchElementException`, which
// `RulesReader.read`'s `catch (IOException)` does not stop, so the whole read dies. The port
// drops the empty rule instead, which is the answer Java's own `Network.insertViaRules` already
// gives a short list (Network.java:383) and leaves the board untouched either way.
// renamed: RulesReader.applyViaRule -> apply_via_rule.
fn apply_via_rule(scanner: &mut DsnScanner, board: &mut Board) -> Result<(), DsnError> {
    if let Some(via_rule) = read_via_rule(scanner)?
        && !via_rule.is_empty()
    {
        add_via_rule(&via_rule, board);
    }
    Ok(())
}

/// `RulesReader.applyNetClass(IJFlexScanner, LayerStructure, BasicBoard)`
/// (RulesReader.java:359-367).
///
/// `viaAtSmdAllowed` is the literal `false` Java passes (:366) — a `.rules` file carries no
/// `(control (via_at_smd …))` scope to read one from.
// renamed: RulesReader.applyNetClass -> apply_net_class.
fn apply_net_class(
    scanner: &mut DsnScanner,
    layer_structure: &DsnLayerStructure,
    board: &mut Board,
    ct: &CoordinateTransform,
) -> Result<(), DsnError> {
    let Some(net_class) = read_net_class_scope(scanner)? else {
        // `netClass == null` (RulesReader.java:362-364).
        return Ok(());
    };
    insert_net_class(&net_class, layer_structure, board, ct, false);
    Ok(())
}

/// Java's `new InputStreamReader(in)` — see [`crate::dsn_reader`]'s note, which this shares
/// verbatim.
fn read_to_string(mut input: impl Read) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
