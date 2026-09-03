//! `io/specctra/parser/{Net,NetList,Rule,Circuit,NetClass,Network}.java` — the `network` scope,
//! and the sub-scopes it and the `structure` scope share.
//!
//! # `Rule.java` lives here
//!
//! `Rule.java` has no scope of its own: a `(rule …)` is always nested inside a `structure`, a
//! `class` or a `layer_rule`, and its `WidthRule`/`ClearanceRule` results are consumed by
//! `Structure.updateBoardRules`/`setClearanceRule` and by `Network.insertNetClass`. Plan Task 6
//! landed its three `Structure`-facing readers in `parser/structure.rs` because the structure
//! scope needed them first; Task 8 moved them here unchanged, to the file the plan's File
//! Structure names, and added `Rule.readLayerRuleScope` next to its only caller
//! (`NetClass.readScope`).
//!
//! # `Circuit.java` and `NetClass.java` live here too
//!
//! Neither is a `ScopeKeyword`: `(circuit …)` only ever appears inside a `(class …)`, and
//! `(class …)`/`(class_class …)` only inside a `(network …)`. Their readers are plain functions
//! over the scanner, called from `Network.readScope` (Task 9).
//!
//! # The writers
//!
//! `Rule.java`'s six writers, `Net.java`'s three and `Network.java`'s six live at the foot of
//! this file. Per plan ruling 1 they emit the 2.3.0 literals `"(clearance_class "`,
//! `"via_rule"`/`"(via_rule "`, `"(use_layer"`, `"(pull_tight off)"` and `"(shove_fixed on)"`,
//! never HEAD's camelCase spellings.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Write;

use fr_board::{
    Board, BoardRules, FixedState, Item, ItemClass, ItemId, Keepout, NetClass, NetClassId,
    PadstackId, PartPin, ViaInfo, ViaInfoId, ViaRule,
};
use fr_geometry::{Area, Point, ShapeOps, Vector};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::DsnError;
use crate::format::{IdentifierType, IndentFileWriter, java_double_to_string, java_round_to_int};
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::{
    CLASS_CLEARANCE_SEPARATOR, read_on_off_scope, read_string_list_scope, read_string_scope,
};
use crate::parser::geometry::DsnLayerStructure;
use crate::parser::library::strip_dot_digits;
use crate::parser::part_library::{DsnLogicalPart, DsnLogicalPartMapping, java_string_cmp};
use crate::parser::placement::ComponentLocation;
use crate::parser::scope_parameter::{ReadScopeParameter, WriteScopeParameter, skip_scope};
use crate::parser::structure::{contains_wire_clearance_pair, read_via_padstacks};

// ------------------------------------------------------------------------------ Rule.java

/// `io/specctra/parser/Rule.java`'s two concrete rule classes, as one enum (Rule is an abstract
/// class whose subclasses carry no shared state, and every consumer is an `instanceof` chain —
/// Structure.java:614,627,653,678).
// renamed: the public nested `Rule.WidthRule(double)` constructor (Rule.java:317-319) -> the
// `DsnRule::Width` variant.
// renamed: the public nested `Rule.ClearanceRule(double, Collection)` constructor
// (Rule.java:327-330) -> `DsnClearanceRule`'s struct literal.
#[derive(Debug, Clone, PartialEq)]
pub enum DsnRule {
    /// `Rule.WidthRule` (Rule.java:313-320): a trace width, in DSN units.
    Width(f64),
    /// `Rule.ClearanceRule` (Rule.java:322-331).
    Clearance(DsnClearanceRule),
}

/// `Rule.ClearanceRule` (Rule.java:322-331).
#[derive(Debug, Clone, PartialEq)]
pub struct DsnClearanceRule {
    /// `ClearanceRule.value`, in DSN units.
    pub value: f64,
    /// `ClearanceRule.clearanceClassPairs`, the `(type …)` list. Empty means "the default
    /// clearance", which [`set_clearance_rule`] handles separately.
    pub clearance_class_pairs: Vec<String>,
}

/// `Rule.readScope` (Rule.java:24-64): the `(rule …)` scope's body, a list of `width`/`clearance`
/// rules. `None` is Java's `null` (end of file).
// renamed: Rule.readScope -> read_rule_scope (see this module's docs on why it lives here).
pub fn read_rule_scope(scanner: &mut DsnScanner) -> Result<Option<Vec<DsnRule>>, DsnError> {
    let mut result = Vec::new();
    let mut prev_was_open = false;
    loop {
        let Some(current_token) = scanner.next_token()? else {
            // "unexpected end of file" (Rule.java:35-39).
            return Ok(None);
        };
        if current_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = current_token == Token::Open;
        if prev_was_open {
            // every rule starts with a "("
            let current_rule = match current_token {
                Token::Kw(Keyword::Width) => read_width_rule(scanner)?,
                Token::Kw(Keyword::Clearance) => read_clearance_rule(scanner)?,
                _ => {
                    let _ = skip_scope(scanner)?;
                    None
                }
            };
            if let Some(rule) = current_rule {
                result.push(rule);
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(result))
}

/// `Rule.readWidthRule` (Rule.java:109-117).
///
// totalized: Rule.readWidthRule — Java's `double value = scanner.nextDouble()` unboxes a
// `Double` that `nextDouble` returns as `null` for a non-numeric token (IJFlexScanner.java:32),
// so `(width abc)` throws a `NullPointerException` that nothing between here and
// `DsnReader.readBoard` catches. The port answers `None`, which is the value Java's *other*
// failure branch on the very next line (a missing closing bracket) already returns and every
// caller already handles by dropping the rule (Rule.java:58-60).
pub fn read_width_rule(scanner: &mut DsnScanner) -> Result<Option<DsnRule>, DsnError> {
    let value = scanner.next_double();
    if !scanner.next_closing_bracket()? {
        return Ok(None);
    }
    Ok(value.map(DsnRule::Width))
}

/// `Rule.readClearanceRule` (Rule.java:257-303).
///
// totalized: Rule.readClearanceRule — same `nextDouble` unboxing NPE as `readWidthRule`, and the
// same totalization to the `null` its callers already handle.
pub fn read_clearance_rule(scanner: &mut DsnScanner) -> Result<Option<DsnRule>, DsnError> {
    let Some(value) = scanner.next_double() else {
        return Ok(None);
    };
    let mut class_pairs: Vec<String> = Vec::new();
    let next_token = scanner.next_token()?;
    if next_token != Some(Token::Close) {
        // look for "(type"
        if next_token != Some(Token::Open) {
            // "( expected" (Rule.java:265-269).
            return Ok(None);
        }
        if scanner.next_token()? != Some(Token::Kw(Keyword::Type)) {
            // "type expected" (Rule.java:271-275).
            return Ok(None);
        }
        class_pairs.extend(scanner.next_string_list_sep(CLASS_CLEARANCE_SEPARATOR));
        // check the closing ")" of "(type"
        if !scanner.next_closing_bracket()? {
            return Ok(None);
        }
        // check the closing ")" of "(clear"
        if !scanner.next_closing_bracket()? {
            return Ok(None);
        }
    }
    Ok(Some(DsnRule::Clearance(DsnClearanceRule {
        value,
        clearance_class_pairs: class_pairs,
    })))
}

/// `Rule.LayerRule` (Rule.java:333-342): a set of layer names and the rules that apply on them.
// renamed: the nested `Rule.LayerRule(Collection, Collection)` constructor (Rule.java:338-341)
// -> this struct's literal; it is package-private in Java.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnLayerRule {
    /// `LayerRule.layerNames` (Rule.java:335).
    pub layer_names: Vec<String>,
    /// `LayerRule.rules` (Rule.java:336).
    pub rules: Vec<DsnRule>,
}

/// `Rule.readLayerRuleScope` (Rule.java:67-107): a `(layer_rule <layer>+ (rule …))` scope. Its
/// only caller is `NetClass.readScope`/`readClassClassScope` — no `structure` scope reaches it.
///
/// Java's shape is worth spelling out, because it is stricter than it looks: the first loop eats
/// layer names in the `LAYER_NAME` lexical state until it meets `(`, and the second loop then
/// expects the token right after that bracket to be `rule`. `Rule.readScope` consumes the `)`
/// that closes its own scope, so a *second* `(rule …)` inside the same `layer_rule` presents the
/// second loop with a `(` instead of `rule` and Java warns and returns `null`. JVM-checked (see
/// the `NetProbe` runs recorded in the Task 8 report).
///
// totalized: Rule.readLayerRuleScope — `ruleList.addAll(readScope(scanner))` (Rule.java:100)
// dereferences a `null` from `readScope`, which happens only at end of file. Reproducing the NPE
// would not change what the function answers: the very next iteration of the enclosing loop
// reads `null` too, which is neither `)` nor `rule`, so Java returns `null` from there anyway.
// The port treats the `null` rule list as empty and lets that next iteration do the returning.
pub fn read_layer_rule_scope(scanner: &mut DsnScanner) -> Result<Option<DsnLayerRule>, DsnError> {
    let mut layer_names: Vec<String> = Vec::new();
    let mut rule_list: Vec<DsnRule> = Vec::new();
    loop {
        scanner.yybegin(LexicalState::LayerName);
        match scanner.next_token()? {
            Some(Token::Open) => break,
            Some(Token::Str(name)) => layer_names.push(name),
            // "string expected" (Rule.java:77-84).
            _ => return Ok(None),
        }
    }
    loop {
        match scanner.next_token()? {
            Some(Token::Close) => break,
            Some(Token::Kw(Keyword::Rule)) => {
                rule_list.extend(read_rule_scope(scanner)?.unwrap_or_default());
            }
            // "rule expected" (Rule.java:92-99).
            _ => return Ok(None),
        }
    }
    Ok(Some(DsnLayerRule {
        layer_names,
        rules: rule_list,
    }))
}

// --------------------------------------------------------------------------- Circuit.java

/// `Circuit.ReadScopeResult` (Circuit.java:113-130): what a `(circuit …)` scope contributes to
/// the net class around it.
///
/// Ruling 8 in full: `Circuit.readScope` keeps the length-matching rule *and* the `use_via` and
/// `use_layer` lists (Circuit.java:52-55), and `NetClass.readScope` consumes all four
/// (NetClass.java:102-109). The Task 8 brief said "min/max trace length only"; Java wins.
///
/// "A maxLength of -1 indicates that no maximum length is defined" (Circuit.java:112). The two
/// lengths are stored in the order `(length <max> <min>)`, which is the order
/// `Network.writeCircuit` writes them back out in (Network.java:169-186) — not a transposition
/// bug.
// renamed: the nested `Circuit.ReadScopeResult(double, double, Collection, Collection)`
// constructor (Circuit.java:120-129) -> this struct's literal.
// renamed: the private nested `Circuit.LengthMatchingRule(double, double)` class
// (Circuit.java:133-142) -> `read_length_scope`'s `[f64; 2]` return, which is all it ever held.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnCircuit {
    /// `ReadScopeResult.maxLength` (Circuit.java:115).
    pub max_length: f64,
    /// `ReadScopeResult.minLength` (Circuit.java:116).
    pub min_length: f64,
    /// `ReadScopeResult.useVia` (Circuit.java:117).
    pub use_via: Vec<String>,
    /// `ReadScopeResult.useLayer` (Circuit.java:118).
    pub use_layer: Vec<String>,
}

/// `Circuit.readScope` (Circuit.java:22-62): "currently only the length matching rule is read
/// from a circuit scope" — plus `use_via` and `use_layer`. Everything else is `skipScope`d.
/// `None` is Java's `null` (end of file).
pub fn read_circuit_scope(scanner: &mut DsnScanner) -> Result<Option<DsnCircuit>, DsnError> {
    let mut min_trace_length = 0.0;
    let mut max_trace_length = 0.0;
    let mut use_via: Vec<String> = Vec::new();
    let mut use_layer: Vec<String> = Vec::new();
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            // "unexpected end of file" (Circuit.java:36-40).
            return Ok(None);
        };
        if next_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Length) => {
                    if let Some(length_rule) = read_length_scope(scanner)? {
                        max_trace_length = length_rule[0];
                        min_trace_length = length_rule[1];
                    }
                }
                // `Structure.readViaPadstacks` can answer `null`, which Java's `addAll`
                // dereferences; the same totalization `Structure.readScope` already applies.
                Token::Kw(Keyword::UseVia) => {
                    use_via.extend(read_via_padstacks(scanner)?.unwrap_or_default());
                }
                // `DsnFile.readStringListScope` likewise (`Arrays.stream(null)`).
                Token::Kw(Keyword::UseLayer) => {
                    use_layer.extend(read_string_list_scope(scanner)?.unwrap_or_default());
                }
                _ => {
                    let _ = skip_scope(scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(DsnCircuit {
        max_length: max_trace_length,
        min_length: min_trace_length,
        use_via,
        use_layer,
    }))
}

/// `Circuit.readLengthScope` (Circuit.java:64-110): the two numbers of a `(length <max> <min>)`
/// scope, then every remaining sub-scope skipped. Java packs them into a `LengthMatchingRule`
/// whose `maxLength` is the *first* number; this port returns them as `[max, min]`.
fn read_length_scope(scanner: &mut DsnScanner) -> Result<Option<[f64; 2]>, DsnError> {
    let mut length_arr = [0.0_f64; 2];
    for slot in &mut length_arr {
        #[allow(clippy::cast_precision_loss)]
        match scanner.next_token()? {
            Some(Token::Float(value)) => *slot = value,
            Some(Token::Int(value)) => *slot = value as f64,
            // "number expected" (Circuit.java:79-83).
            _ => return Ok(None),
        }
    }
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            // "unexpected end of file" (Circuit.java:94-100).
            return Ok(None);
        };
        if next_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            let _ = skip_scope(scanner)?;
        }
        prev_was_open = is_open;
    }
    Ok(Some(length_arr))
}

// -------------------------------------------------------------------------- NetClass.java

/// `io/specctra/parser/NetClass.java`: "contains the information of a Specctra Class scope" —
/// a `(class <name> <net>* …)` scope as read, before `Network.insertNetClass` turns it into a
/// `rules.NetClass` on the board.
// renamed: the `NetClass(String, String, Collection, …)` constructor (NetClass.java:28-53) ->
// this struct's literal.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnNetClass {
    /// `NetClass.name` (NetClass.java:14).
    pub name: String,
    /// `NetClass.traceClearanceClass` (NetClass.java:15); `None` is Java's `null`.
    pub trace_clearance_class: Option<String>,
    /// `NetClass.netList` (NetClass.java:16): the names of the nets in the class.
    pub net_list: Vec<String>,
    /// `NetClass.rules` (NetClass.java:17).
    pub rules: Vec<DsnRule>,
    /// `NetClass.layerRules` (NetClass.java:18).
    pub layer_rules: Vec<DsnLayerRule>,
    /// `NetClass.useVia` (NetClass.java:19), filled from the nested `circuit` scope.
    pub use_via: Vec<String>,
    /// `NetClass.useLayer` (NetClass.java:20), likewise.
    pub use_layer: Vec<String>,
    /// `NetClass.viaRule` (NetClass.java:21).
    pub via_rule: Option<String>,
    /// `NetClass.shoveFixed` (NetClass.java:22); Java's local default is `false`.
    pub shove_fixed: bool,
    /// `NetClass.pullTight` (NetClass.java:23); Java's local default is **`true`**.
    pub pull_tight: bool,
    /// `NetClass.minTraceLength` (NetClass.java:24), in DSN units.
    pub min_trace_length: f64,
    /// `NetClass.maxTraceLength` (NetClass.java:25), in DSN units.
    pub max_trace_length: f64,
}

/// `NetClass.readScope` (NetClass.java:55-143): a `(class <name> <net>* …)` scope.
///
/// Java's `rulesMissing` local (NetClass.java:63, :80) is initialised to `false` and never
/// assigned, so the `if (!rulesMissing)` guard around the whole token loop is dead; not ported.
///
/// Java's `traceClearanceClass == null` bail-out (NetClass.java:112-114) is unreachable here:
/// `DsnFile.readStringScope` returns a `String` in this port (see its docs), never `null`.
// renamed: NetClass.readScope -> read_net_class_scope.
pub fn read_net_class_scope(scanner: &mut DsnScanner) -> Result<Option<DsnNetClass>, DsnError> {
    // read the class name
    scanner.yybegin(LexicalState::Name);
    let class_name = scanner.next_string();

    // read the nets belonging to the class
    let net_list = scanner.next_string_list();

    let mut rules: Vec<DsnRule> = Vec::new();
    let mut layer_rules: Vec<DsnLayerRule> = Vec::new();
    let mut use_via: Vec<String> = Vec::new();
    let mut use_layer: Vec<String> = Vec::new();
    let mut via_rule: Option<String> = None;
    let mut trace_clearance_class: Option<String> = None;
    let mut pull_tight = true;
    let mut shove_fixed = false;
    let mut min_trace_length = 0.0;
    let mut max_trace_length = 0.0;

    // Java reads one token here and immediately makes it `prevToken`, without inspecting it
    // (NetClass.java:79-81).
    let mut prev_was_open = scanner.next_token()? == Some(Token::Open);
    loop {
        let Some(next_token) = scanner.next_token()? else {
            // "unexpected end of file" (NetClass.java:84-90).
            return Ok(None);
        };
        if next_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match &next_token {
                // `Rule.readScope`/`readLayerRuleScope` can answer `null`, which Java's
                // `addAll`/`add` respectively dereference or store; see
                // `read_layer_rule_scope`'s totalization note.
                Token::Kw(Keyword::Rule) => {
                    rules.extend(read_rule_scope(scanner)?.unwrap_or_default());
                }
                // totalized: NetClass.readScope stores a `null` `LayerRule`
                // (NetClass.java:99) that `Network.insertNetClass` (Network.java:497-521)
                // dereferences; the port drops it.
                Token::Kw(Keyword::LayerRule) => {
                    if let Some(layer_rule) = read_layer_rule_scope(scanner)? {
                        layer_rules.push(layer_rule);
                    }
                }
                Token::Kw(Keyword::ViaRule) => via_rule = Some(read_string_scope(scanner)?),
                Token::Kw(Keyword::Circuit) => {
                    if let Some(current_rule) = read_circuit_scope(scanner)? {
                        max_trace_length = current_rule.max_length;
                        min_trace_length = current_rule.min_length;
                        use_via.extend(current_rule.use_via);
                        use_layer.extend(current_rule.use_layer);
                    }
                }
                Token::Kw(Keyword::ClearanceClass) => {
                    trace_clearance_class = Some(read_string_scope(scanner)?);
                }
                Token::Kw(Keyword::ShoveFixed) => shove_fixed = read_on_off_scope(scanner)?,
                Token::Kw(Keyword::PullTight) => pull_tight = read_on_off_scope(scanner)?,
                _ => {
                    let _ = skip_scope(scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(DsnNetClass {
        name: class_name,
        trace_clearance_class,
        net_list,
        rules,
        layer_rules,
        use_via,
        use_layer,
        via_rule,
        shove_fixed,
        pull_tight,
        min_trace_length,
        max_trace_length,
    }))
}

/// `NetClass.ClassClass` (NetClass.java:182-196): a `(class_class …)` scope — the rules that
/// apply *between* two net classes.
// renamed: the nested `NetClass.ClassClass(Collection, Collection, Collection)` constructor
// (NetClass.java:188-195) -> this struct's literal.
#[derive(Debug, Clone, PartialEq)]
pub struct DsnClassClass {
    /// `ClassClass.classNames` (NetClass.java:184).
    pub class_names: Vec<String>,
    /// `ClassClass.rules` (NetClass.java:185).
    pub rules: Vec<DsnRule>,
    /// `ClassClass.layerRules` (NetClass.java:186).
    pub layer_rules: Vec<DsnLayerRule>,
}

/// `NetClass.readClassClassScope` (NetClass.java:145-180).
///
/// Unlike [`read_net_class_scope`], the token loop here has **no `else` branch**: an unknown
/// sub-scope is not `skipScope`d, so its contents are re-scanned as if they were part of this
/// scope (NetClass.java:164-172). Reproduced as written.
// renamed: NetClass.readClassClassScope -> read_class_class_scope.
pub fn read_class_class_scope(scanner: &mut DsnScanner) -> Result<Option<DsnClassClass>, DsnError> {
    let mut classes: Vec<String> = Vec::new();
    let mut rules: Vec<DsnRule> = Vec::new();
    let mut layer_rules: Vec<DsnLayerRule> = Vec::new();
    let mut prev_was_open = false;
    loop {
        let Some(next_token) = scanner.next_token()? else {
            // "unexpected end of file" (NetClass.java:153-159).
            return Ok(None);
        };
        if next_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match &next_token {
                Token::Kw(Keyword::Classes) => {
                    classes.extend(read_string_list_scope(scanner)?.unwrap_or_default());
                }
                Token::Kw(Keyword::Rule) => {
                    rules.extend(read_rule_scope(scanner)?.unwrap_or_default());
                }
                // totalized: same `null` `LayerRule` as `read_net_class_scope`.
                Token::Kw(Keyword::LayerRule) => {
                    if let Some(layer_rule) = read_layer_rule_scope(scanner)? {
                        layer_rules.push(layer_rule);
                    }
                }
                _ => {}
            }
        }
        prev_was_open = is_open;
    }
    Ok(Some(DsnClassClass {
        class_names: classes,
        rules,
        layer_rules,
    }))
}

// ------------------------------------------------------------------------------- Net.java

/// `Net.Pin` (Net.java:105-128): "sorted tuple of component name and pin name" — the DSN
/// parser's own pin reference, resolved against the board only in `Network.insertNets` (Task 9).
///
/// `Pin.compareTo` (Net.java:115-122) compares `componentName` and then `pinName` with
/// `String.compareTo`, which orders by UTF-16 code units — *not* by Unicode scalar value, which
/// is what a derived `Ord` over two `String`s would give. The two disagree only when a
/// supplementary character meets one in U+E000..U+FFFF, but the order is observable (the pin set
/// is a `TreeSet`), so the impl below goes through [`java_string_cmp`].
// renamed: Net.Pin -> PinRef (`Pin` is `fr_board`'s board item, which this module also names).
// renamed: Net.Pin.compareTo -> the `Ord` impl below.
// renamed: Net.Pin.toString -> the `Display` impl below.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PinRef {
    /// `Net.Pin.componentName` (Net.java:107).
    pub component_name: String,
    /// `Net.Pin.pinName` (Net.java:108).
    pub pin_name: String,
}

impl Ord for PinRef {
    /// `Net.Pin.compareTo` (Net.java:115-122).
    fn cmp(&self, other: &PinRef) -> std::cmp::Ordering {
        java_string_cmp(&self.component_name, &other.component_name)
            .then_with(|| java_string_cmp(&self.pin_name, &other.pin_name))
    }
}

impl PartialOrd for PinRef {
    fn partial_cmp(&self, other: &PinRef) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PinRef {
    /// `Net.Pin(String, String)` (Net.java:110-113).
    #[must_use]
    pub fn new(component_name: impl Into<String>, pin_name: impl Into<String>) -> PinRef {
        PinRef {
            component_name: component_name.into(),
            pin_name: pin_name.into(),
        }
    }
}

impl fmt::Display for PinRef {
    /// `Net.Pin.toString` (Net.java:124-127): `Pin{<component>-<pin>}`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pin{{{}-{}}}", self.component_name, self.pin_name)
    }
}

/// `Net.Id` (Net.java:84-102 — the DSN-parser's own `Net`, not `rules.Net`): the `TreeMap`/
/// `BTreeMap` key for [`NetList`].
///
/// `Id.compareTo` is `this.name.compareTo(other.name)` (plain, **not** `compareToIgnoreCase` —
/// unlike `rules.Net.compareTo`, this one is case-sensitive) falling back to
/// `this.subnetNumber - other.subnetNumber` when the names are equal. `String.compareTo` orders
/// by UTF-16 code units, so the impl below goes through [`java_string_cmp`] rather than deriving
/// `Ord`: this type keys the [`NetList`] `BTreeMap`, and `NetList.getNets`'s iteration order
/// decides which net a multi-net pin reports first (`Network.insertComponent`,
/// Network.java:1046).
// renamed: Net.Id.compareTo -> the `Ord` impl below.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetId {
    /// `Net.Id.name` (Net.java:86).
    pub name: String,
    /// `Net.Id.subnetNumber` (Net.java:87).
    pub subnet_no: i32,
}

impl Ord for NetId {
    /// `Net.Id.compareTo` (Net.java:94-101).
    fn cmp(&self, other: &NetId) -> std::cmp::Ordering {
        java_string_cmp(&self.name, &other.name).then_with(|| self.subnet_no.cmp(&other.subnet_no))
    }
}

impl PartialOrd for NetId {
    fn partial_cmp(&self, other: &NetId) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl NetId {
    /// `Net.Id(String, int)` (Net.java:89-92).
    #[must_use]
    pub fn new(name: impl Into<String>, subnet_no: i32) -> NetId {
        NetId {
            name: name.into(),
            subnet_no,
        }
    }
}

/// `io/specctra/parser/Net.java` (the DSN-parser's `Net`, distinct from `rules.Net`): a net as
/// read from a `network` scope, before it is resolved against `rules.Nets`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnNet {
    /// `Net.id` (Net.java:17).
    pub id: NetId,
    /// `Net.pinList` (Net.java:20). Java declares it `Set<Pin>` where `Pin` resolves to the
    /// *member* type `Net.Pin`, not the imported `board.model.items.Pin`, and `setPins` wraps
    /// the caller's collection in a `TreeSet` — hence `BTreeSet<PinRef>`. `None` is Java's
    /// `null`: `setPins` has not been called yet, a state `NetList.getNets` explicitly tests for
    /// (NetList.java:50).
    pins: Option<BTreeSet<PinRef>>,
}

impl DsnNet {
    /// `Net(Id)` (Net.java:23-25).
    #[must_use]
    pub fn new(id: NetId) -> DsnNet {
        DsnNet { id, pins: None }
    }

    /// `Net.getPins` (Net.java:76-78).
    #[must_use]
    pub fn get_pins(&self) -> Option<&BTreeSet<PinRef>> {
        self.pins.as_ref()
    }

    /// `Net.setPins(Collection<Pin>)` (Net.java:80-82): `new TreeSet<>(pinList)`, so duplicates
    /// collapse and the order becomes `PinRef`'s.
    pub fn set_pins(&mut self, pin_list: impl IntoIterator<Item = PinRef>) {
        self.pins = Some(pin_list.into_iter().collect());
    }
}

// --------------------------------------------------------------------------- NetList.java

/// `io/specctra/parser/NetList.java`: "describes a list of nets sorted by its names. The net
/// number is generated internally."
///
/// Java's `TreeMap<Net.Id, Net>` is a [`BTreeMap`], so iteration order is [`NetId`]'s `Ord` —
/// `(name, subnet_no)` — and not insertion order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetList {
    /// `NetList.nets` (NetList.java:13).
    nets: BTreeMap<NetId, DsnNet>,
}

impl NetList {
    /// A fresh, empty net list — Java's field initialiser (NetList.java:13).
    #[must_use]
    pub fn new() -> NetList {
        NetList::default()
    }

    /// `NetList.contains` (NetList.java:16-18).
    #[must_use]
    pub fn contains(&self, net_id: &NetId) -> bool {
        self.nets.contains_key(net_id)
    }

    /// `NetList.addNet` (NetList.java:24-33): `None` (Java's `null`) when a net with that id is
    /// already present, in which case nothing is added.
    pub fn add_net(&mut self, net_id: NetId) -> Option<&mut DsnNet> {
        if self.nets.contains_key(&net_id) {
            return None;
        }
        Some(
            self.nets
                .entry(net_id.clone())
                .or_insert_with(|| DsnNet::new(net_id)),
        )
    }

    /// `NetList.getNet` (NetList.java:39-41).
    #[must_use]
    pub fn get_net(&self, net_id: &NetId) -> Option<&DsnNet> {
        self.nets.get(net_id)
    }

    /// [`Self::get_net`] for the callers that mutate — Java hands back the live object from
    /// `getNet`, which has no `&mut`/`&` distinction to make.
    pub fn get_net_mut(&mut self, net_id: &NetId) -> Option<&mut DsnNet> {
        self.nets.get_mut(net_id)
    }

    /// `NetList.getNets(String, String)` (NetList.java:44-55): every net containing that pin, in
    /// `NetId` order.
    #[must_use]
    pub fn get_nets(&self, component_name: &str, pin_name: &str) -> Vec<&DsnNet> {
        let search_pin = PinRef::new(component_name, pin_name);
        self.nets
            .values()
            .filter(|net| {
                net.get_pins()
                    .is_some_and(|pins| pins.contains(&search_pin))
            })
            .collect()
    }

    /// The nets in `NetId` order — Java reaches `nets.values()` directly from inside the class
    /// (NetList.java:47) and `Network`/`Structure` iterate the map the same way.
    ///
    /// Returns an opaque iterator rather than `btree_map::Values`, so the backing collection
    /// stays an implementation detail.
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &DsnNet> + ExactSizeIterator {
        self.nets.values()
    }

    /// `nets.isEmpty()` — not a Java method (Java's callers reach the private field directly).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nets.is_empty()
    }
}

// --------------------------------------------------------------------------- Network.java

/// `Network.readScope` (Network.java:1197-1323): the `(network …)` scope, and — in its tail —
/// the point at which almost every board item is created.
///
/// The loop itself only *collects*: `(net …)` goes straight to `board.rules.nets` through
/// [`read_net_scope`], while `(via …)`, `(via_rule …)`, `(class …)` and `(class_class …)` pile
/// up in four local lists. Everything else happens after the closing bracket, in exactly this
/// order (Network.java:1274-1322) — and because item ids are handed out in insertion order,
/// this order *is* the id assignment for every item after the board outline and its holes:
///
/// 1. merge the net classes' `use_via` names into `ReadScopeParameter.via_padstack_names` and
///    `BoardLibrary::set_via_padstacks` the resolved list (:1274-1313);
/// 2. [`insert_via_infos`] (:1315);
/// 3. [`insert_via_rules`] (:1317);
/// 4. [`insert_net_classes`] (:1318);
/// 5. [`insert_class_pairs`] (:1319);
/// 6. [`insert_components`] (:1320) — per component: one `insert_pin` per package pin
///    (:1035), then the package/via/place keepouts (:1082/:1093/:1104), then the component
///    outlines (:1192);
/// 7. [`insert_logical_parts`] (:1321).
///
/// Plan 2 obligation, **phase 2 of 2**: `BoardLibrary::via_padstacks` is set to the empty list
/// by `Structure.createBoard` (see `parser/structure.rs`, "phase 1 of 2") because the padstacks
/// the structure scope names do not exist until the `library` scope has been read; step 1 above
/// is where the real list finally lands, merged from both scopes. Between the two phases the
/// list is `Some(vec![])`, never `None`, so the two `BoardLibrary` methods that reproduce Java's
/// NPE on a `null` list (quirks #42-43) cannot be reached with a `None`.
pub fn read_network_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    let mut classes: Vec<DsnNetClass> = Vec::new();
    let mut class_class_list: Vec<DsnClassClass> = Vec::new();
    let mut via_infos: Vec<ViaInfo> = Vec::new();
    let mut via_rules: Vec<Vec<String>> = Vec::new();

    let mut prev_was_open = false;
    loop {
        let Some(next_token) = p.scanner.next_token()? else {
            // "unexpected end of file" (Network.java:1211-1216).
            return Ok(false);
        };
        if next_token == Token::Close {
            // end of scope
            break;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            match next_token {
                Token::Kw(Keyword::Net) => {
                    // Java discards `readNetScope`'s boolean (Network.java:1220-1226).
                    read_net_scope(p)?;
                }
                Token::Kw(Keyword::Via) => {
                    let board = p.board.as_mut().expect(BOARD_EXPECTED);
                    match read_via_info(&mut p.scanner, board)? {
                        Some(via_info) => via_infos.push(via_info),
                        None => return Ok(false),
                    }
                }
                Token::Kw(Keyword::ViaRule) => match read_via_rule(&mut p.scanner)? {
                    Some(rule) => via_rules.push(rule),
                    None => return Ok(false),
                },
                Token::Kw(Keyword::Class) => match read_net_class_scope(&mut p.scanner)? {
                    Some(class) => classes.push(class),
                    None => return Ok(false),
                },
                Token::Kw(Keyword::ClassClass) => match read_class_class_scope(&mut p.scanner)? {
                    Some(class_class) => class_class_list.push(class_class),
                    None => return Ok(false),
                },
                _ => {
                    skip_scope(&mut p.scanner)?;
                }
            }
        }
        prev_was_open = is_open;
    }

    // Add any vias defined in the Netclasses to the list of vias to be instantiated
    // (Network.java:1274-1280).
    //
    // Java bug: Network.readScope — the `else` branch assigns `n.useVia` **by reference**
    // (:1278), so when the structure scope named no via padstacks at all the first net class's
    // own `useVia` list becomes the merged list, and every later class's `addAll` mutates it.
    // `Network.insertNetClass` then reads that same mutated list back out
    // (`createViaRule(netClass.useVia, …)`, :539), so the first net class gets a via rule
    // holding every class's vias. Reproduced below by copying the merged list back into the
    // aliased class after the loop. See `docs/java-quirks.md`.
    let mut aliased_class: Option<usize> = None;
    for (index, net_class) in classes.iter().enumerate() {
        match &mut p.via_padstack_names {
            Some(names) => names.extend(net_class.use_via.iter().cloned()),
            None => {
                p.via_padstack_names = Some(net_class.use_via.clone());
                aliased_class = Some(index);
            }
        }
    }
    if let Some(index) = aliased_class {
        classes[index].use_via = p.via_padstack_names.clone().unwrap_or_default();
    }

    // Set the via padstacks after network parsing, so that named vias from both structure and
    // network DSN sections are properly instantiated (Network.java:1282-1313).
    if let Some(names) = p.via_padstack_names.clone() {
        let board = p.board.as_mut().expect(BOARD_EXPECTED);
        let mut via_padstacks: Vec<PadstackId> = Vec::with_capacity(names.len());
        for current_padstack_name in &names {
            let cleaned_name = strip_dot_digits(current_padstack_name);
            // Java writes into `viaPadstacks[foundPadstackCount]`, i.e. it **compacts**: a name
            // with no padstack leaves no hole, and every later via padstack moves down one index
            // (Network.java:1291-1295, and the `System.arraycopy` shrink at :1306-1311, which is
            // what pushing only the found ones already achieves). The name that misses is a
            // "Library.read_scope: via padstack with name '…' not found" `FRLogger.warn` and
            // nothing more (:1296-1303): it is not pushed onto `ReadScopeParameter.warnings`, so
            // it leaves no trace in this port.
            if let Some(padstack) = board.library.padstacks.get_by_name(&cleaned_name) {
                via_padstacks.push(PadstackId(padstack.no));
            }
        }
        board.library.set_via_padstacks(via_padstacks);
    }

    let via_at_smd_allowed = p.via_at_smd_allowed;
    {
        let board = p.board.as_mut().expect(BOARD_EXPECTED);
        insert_via_infos(via_infos, board, via_at_smd_allowed);
        insert_via_rules(&via_rules, board);
    }
    insert_net_classes(&classes, p);
    insert_class_pairs(&class_class_list, p);
    insert_components(p);
    insert_logical_parts(p);
    Ok(true)
}

/// The message on every `p.board` unwrap in this module. Java reaches the board through
/// `scopeParameter.boardHandling.getRoutingBoard()`, which returns `null` — and NPEs — until
/// `Structure.createBoard` has run; a `(network …)` scope before `(structure …)` is the only
/// way to get there, and the DSN format does not allow it.
const BOARD_EXPECTED: &str =
    "Network.readScope: the structure scope must have created the board (Java NPEs here too)";

/// `String.split("_")` (Network.java:651,777), whose trailing empty strings Java drops and
/// Rust's `str::split` keeps. Java also returns the whole input when the separator never
/// occurs, which is why `""` splits to `[""]` and not to `[]`.
fn java_split_underscore(text: &str) -> Vec<&str> {
    if !text.contains('_') {
        return vec![text];
    }
    let mut parts: Vec<&str> = text.split('_').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

// -------------------------------------------------------------------- KiCadNetClassNames.java

/// `io/KiCadNetClassNames.KICAD_DSN_DEFAULT` (KiCadNetClassNames.java:15): "KiCad renames its
/// `Default` net class to `kicad_default` in Specctra DSN files to avoid colliding with
/// Freerouting's reserved internal `default` class."
pub const KICAD_DSN_DEFAULT: &str = "kicad_default";

/// `KiCadNetClassNames.isKiCadDefaultNetClassName` (KiCadNetClassNames.java:24-30). `null` and
/// the empty string are both false; the two matches are case-insensitive.
// renamed: KiCadNetClassNames.isKiCadDefaultNetClassName -> is_kicad_default_net_class_name; the audit script's mechanical snake_case of the Java name is `is_ki_cad_default_net_class_name`, which no Rust reader would write.
#[must_use]
pub fn is_kicad_default_net_class_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case(KICAD_DSN_DEFAULT)
}

/// `KiCadNetClassNames.resolveNetClass` (KiCadNetClassNames.java:39-45): the board's *default*
/// net class for either KiCad spelling, otherwise the class with exactly that name.
#[must_use]
pub fn resolve_net_class(rules: &mut BoardRules, name: &str) -> Option<NetClassId> {
    if is_kicad_default_net_class_name(name) {
        return Some(rules.get_default_net_class());
    }
    rules.net_classes.get_no(name)
}

// ------------------------------------------------------------------ the `(net …)` sub-scope

/// `Network.readNetScope` (Network.java:1325-1465): one `(net <name> [<subnet>] …)` scope,
/// creating the `rules.Net`s it names on the board as it goes.
fn read_net_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    // read the net name
    let net_name = p.scanner.next_string();

    let mut subnet_number = 1;
    let mut next_token = p.scanner.next_token()?;
    let scope_is_empty = next_token == Some(Token::Close);
    if let Some(Token::Int(value)) = next_token {
        // Java's `Integer` token is an `int`; the port's is an `i64` (the lexer widens), so the
        // narrowing Java did at parse time happens here instead.
        subnet_number = value as i32;
    }
    let mut pin_order_found = false;
    let mut pin_list: Vec<PinRef> = Vec::new();
    let mut net_rules: Vec<DsnRule> = Vec::new();
    let mut subnet_pin_lists: Vec<Vec<PinRef>> = Vec::new();
    if !scope_is_empty {
        let mut prev_was_open = next_token == Some(Token::Open);
        loop {
            next_token = p.scanner.next_token()?;
            let Some(token) = next_token else {
                // "unexpected end of file" (Network.java:1362-1367).
                return Ok(false);
            };
            if token == Token::Close {
                // end of scope
                break;
            }
            let is_open = token == Token::Open;
            if prev_was_open {
                match token {
                    Token::Kw(Keyword::Pins) => {
                        if !read_net_pins(&mut p.scanner, &mut pin_list)? {
                            return Ok(false);
                        }
                    }
                    Token::Kw(Keyword::Order) => {
                        pin_order_found = true;
                        if !read_net_pins(&mut p.scanner, &mut pin_list)? {
                            return Ok(false);
                        }
                    }
                    Token::Kw(Keyword::Fromto) => {
                        let mut current_subnet_pin_list: Vec<PinRef> = Vec::new();
                        if !read_net_pins(&mut p.scanner, &mut current_subnet_pin_list)? {
                            return Ok(false);
                        }
                        // Java's `Set<Net.Pin> currentSubnetPinList = new TreeSet<>()`
                        // (Network.java:1378) sorts and deduplicates before `setPins` would.
                        current_subnet_pin_list.sort();
                        current_subnet_pin_list.dedup();
                        subnet_pin_lists.push(current_subnet_pin_list);
                    }
                    Token::Kw(Keyword::Rule) => {
                        // totalized: Rule.readScope's `null` — Java's `addAll(null)` NPEs
                        // (Network.java:1385).
                        if let Some(rules) = read_rule_scope(&mut p.scanner)? {
                            net_rules.extend(rules);
                        }
                    }
                    // "layer_rule not yet implemented" — an `FRLogger.warn` and a `skipScope`
                    // (Network.java:1386-1392).
                    _ => {
                        skip_scope(&mut p.scanner)?;
                    }
                }
            }
            prev_was_open = is_open;
        }
    }
    if subnet_pin_lists.is_empty() {
        if pin_order_found {
            subnet_pin_lists = create_ordered_subnets(&pin_list);
        } else {
            subnet_pin_lists.push(pin_list);
        }
    }
    let coordinate_transform = p.coordinate_transform.expect(TRANSFORM_EXPECTED);
    let contains_plane = p
        .layer_structure
        .as_ref()
        .expect(LAYER_STRUCTURE_EXPECTED)
        .contains_plane(&net_name);
    for current_pin_list in subnet_pin_lists {
        let net_id = NetId::new(net_name.clone(), subnet_number);
        if !p.netlist.contains(&net_id) {
            let board = p.board.as_mut().expect(BOARD_EXPECTED);
            let default_class = board.rules.get_default_net_class();
            if p.netlist.add_net(net_id.clone()).is_some() {
                let board = p.board.as_mut().expect(BOARD_EXPECTED);
                board.rules.nets.add(
                    net_id.name.clone(),
                    net_id.subnet_no,
                    contains_plane,
                    default_class,
                );
            }
        }
        let Some(current_subnet) = p.netlist.get_net_mut(&net_id) else {
            // "net not found in netlist" (Network.java:1408-1414).
            return Ok(false);
        };
        current_subnet.set_pins(current_pin_list);
        if !net_rules.is_empty() {
            // Evaluate the net rules.
            let board = p.board.as_mut().expect(BOARD_EXPECTED);
            let Some(board_net_number) = board
                .rules
                .nets
                .get_by_name_and_subnet(&net_id.name, net_id.subnet_no)
                .map(|net| net.net_number)
            else {
                // "board net not found" (Network.java:1420-1426).
                return Ok(false);
            };
            for current_object in &net_rules {
                // "Rule not yet implemented" for anything but a width rule
                // (Network.java:1450-1455).
                if let DsnRule::Width(wire_width) = current_object {
                    let default_net_rule = board.rules.get_default_net_class();
                    // Note the division by 2 happens **after** `dsnToBoard` here, unlike
                    // `insertNetClass` (Network.java:1441 vs :485).
                    let trace_half_width =
                        java_round_to_int(coordinate_transform.dsn_to_board(*wire_width) / 2.0);
                    let default_trace_clearance_class = board
                        .rules
                        .net_classes
                        .get(default_net_rule)
                        .get_trace_clearance_class();
                    // Java passes `defaultNetRule.getViaRule()`, the object itself
                    // (Network.java:1445); the port clones so the `&mut` borrow below can run.
                    let default_via_rule = board
                        .rules
                        .net_classes
                        .get(default_net_rule)
                        .get_via_rule()
                        .cloned();
                    let net_rule = board
                        .rules
                        .net_classes
                        .find(
                            trace_half_width,
                            default_trace_clearance_class,
                            default_via_rule.as_ref(),
                        )
                        // create a new net rule
                        .unwrap_or_else(|| board.rules.get_new_net_class());
                    board
                        .rules
                        .net_classes
                        .get_mut(net_rule)
                        .set_trace_half_width_on_all_layers(trace_half_width);
                    board
                        .rules
                        .nets
                        .get_mut(board_net_number)
                        .expect("looked up above")
                        .set_class(net_rule);
                }
            }
        }
        subnet_number += 1;
    }
    Ok(true)
}

/// The message on every `p.layer_structure` unwrap in this module. `Structure.readScope` builds
/// it before `Structure.createBoard` returns (Structure.java:975-978, :985-987), and Java
/// dereferences it unguarded — `layerStructure.containsPlane` (Network.java:1394) and
/// `layerStructure.layers.length` (:711, via `insertNetClass`) both NPE on a `null`.
const LAYER_STRUCTURE_EXPECTED: &str =
    "Network: the structure scope must have built the layer structure (Java NPEs here too)";

/// The message on every `p.coordinate_transform` unwrap in this module — `Structure.createBoard`
/// assigns it, and Java NPEs on the `null` a `(network …)` before a `(structure …)` would leave.
const TRANSFORM_EXPECTED: &str =
    "Network: the structure scope must have set the coordinate transform (Java NPEs here too)";

/// `Network.createOrderedSubnets` (Network.java:192-208): "creates a sequence of subnets with 2
/// pins from pinList".
fn create_ordered_subnets(pin_list: &[PinRef]) -> Vec<Vec<PinRef>> {
    let mut result: Vec<Vec<PinRef>> = Vec::new();
    let mut it = pin_list.iter();
    let Some(mut prev_pin) = it.next() else {
        return result;
    };
    for next_pin in it {
        // Java's `TreeSet` (Network.java:200) sorts the two pins and collapses a duplicate.
        let mut current_subnet_pin_list = vec![prev_pin.clone(), next_pin.clone()];
        current_subnet_pin_list.sort();
        current_subnet_pin_list.dedup();
        result.push(current_subnet_pin_list);
        prev_pin = next_pin;
    }
    result
}

/// `Network.readNetPins` (Network.java:210-260): the `(pins …)`/`(order …)`/`(fromto …)` bodies,
/// each entry a `<component>-<pin>` pair read through the scanner's hyphen-aware string bypass.
fn read_net_pins(scanner: &mut DsnScanner, pin_list: &mut Vec<PinRef>) -> Result<bool, DsnError> {
    loop {
        let component_name = scanner.next_string_with(true, '-');
        if component_name.is_empty() {
            break;
        }
        scanner.yybegin(LexicalState::SpecChar);
        // overread the hyphen
        scanner.next_token()?;
        let pin_name = scanner.next_string_ignoring_newline(true);
        pin_list.push(PinRef::new(component_name, pin_name));
    }

    let next_token = scanner.next_token()?;
    if next_token.is_none() {
        // "unexpected end of file" (Network.java:239-245).
        return Ok(false);
    }
    // A missing closing bracket is only an `FRLogger.warn` here — Java carries on and returns
    // true (Network.java:246-253).
    Ok(true)
}

// ------------------------------------------------------------------ the `(via …)` sub-scope

/// `Network.readViaInfo` (Network.java:250-321): `(via <name> <padstack> <clearance-class>
/// [attach])`.
///
/// Side effect worth naming: a padstack that is in `library.padstacks` but not yet in the via
/// padstack list is **appended to it** here (:274), i.e. before the tail's
/// `set_via_padstacks` overwrites the whole list. `None` is Java's `null`.
pub(crate) fn read_via_info(
    scanner: &mut DsnScanner,
    board: &mut Board,
) -> Result<Option<ViaInfo>, DsnError> {
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(name)) = scanner.next_token()? else {
        // "string expected" (Network.java:254-259).
        return Ok(None);
    };
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(padstack_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    scanner.set_scope_identifier(&padstack_name);
    let via_padstack = match board.library.get_via_padstack_by_name(&padstack_name) {
        Some(padstack) => padstack,
        None => {
            // The padstack may not yet be inserted into the list of via padstacks.
            let Some(padstack) = board.library.padstacks.get_by_name(&padstack_name) else {
                // "padstack not found" (Network.java:270-276).
                return Ok(None);
            };
            let padstack = PadstackId(padstack.no);
            board.library.add_via_padstack(padstack);
            padstack
        }
    };
    scanner.yybegin(LexicalState::Name);
    let Some(Token::Str(clearance_class_name)) = scanner.next_token()? else {
        return Ok(None);
    };
    // Clearance class not stored, because it is identical to the default clearance netClass.
    let clearance_class = board
        .rules
        .clearance_matrix
        .get_no(&clearance_class_name)
        .unwrap_or_else(BoardRules::default_clearance_class);
    let mut attach_allowed = false;
    let mut next_token = scanner.next_token()?;
    if next_token != Some(Token::Close) {
        if next_token != Some(Token::Kw(Keyword::Attach)) {
            // "Keyword.ATTACH expected" (Network.java:300-306).
            return Ok(None);
        }
        attach_allowed = true;
        next_token = scanner.next_token()?;
        if next_token != Some(Token::Close) {
            // "closing bracket expected" (Network.java:309-315).
            return Ok(None);
        }
    }
    Ok(Some(ViaInfo::new(
        name,
        via_padstack,
        clearance_class,
        attach_allowed,
    )))
}

/// `Network.readViaRule` (Network.java:323-345): `(via_rule <name> <via-name>*)`, as a plain
/// list of names — the first is the rule's, the rest are via-info names. `None` is Java's
/// `null`.
pub(crate) fn read_via_rule(scanner: &mut DsnScanner) -> Result<Option<Vec<String>>, DsnError> {
    let mut result: Vec<String> = Vec::new();
    loop {
        scanner.yybegin(LexicalState::Name);
        match scanner.next_token()? {
            Some(Token::Close) => break,
            Some(Token::Str(name)) => result.push(name),
            // "string expected" (Network.java:333-338), and end of file, where Java's loop
            // would spin: both answer `null` here.
            _ => return Ok(None),
        }
    }
    Ok(Some(result))
}

// --------------------------------------------------------------------- the insertion tail

/// `Network.insertViaInfos` (Network.java:347-357): the via infos the file declared, or — when
/// it declared none — one per via padstack, from [`create_default_via_infos`].
fn insert_via_infos(via_infos: Vec<ViaInfo>, board: &mut Board, attach_allowed: bool) {
    if via_infos.is_empty() {
        // No via infos found; create default via infos from the via padstacks.
        let default_net_class = board.rules.get_default_net_class();
        create_default_via_infos(board, default_net_class, attach_allowed);
        return;
    }
    for current_info in via_infos {
        board.rules.via_infos.add(current_info);
    }
}

/// `Network.createDefaultViaInfos` (Network.java:359-378): one [`ViaInfo`] per via padstack,
/// named after the padstack for the default net class and `<padstack>-<class>` for any other
/// (Network.java:371 — `getName()` site 4 of 4).
fn create_default_via_infos(board: &mut Board, net_class: NetClassId, attach_allowed: bool) {
    let clearance_class_index = board
        .rules
        .net_classes
        .get(net_class)
        .default_item_clearance_classes
        .get(ItemClass::Via);
    let is_default_class = net_class == board.rules.get_default_net_class();
    let net_class_name = board
        .rules
        .net_classes
        .get(net_class)
        .get_name()
        .to_string();
    for i in 0..board.library.via_padstack_count() {
        // `i < viaPadstackCount()`, so Java's `getViaPadstack(i)` cannot be `null` either
        // (BoardLibrary.java:44-50 returns `null` only out of range or on an unset list).
        let current_padstack = board
            .library
            .get_via_padstack(i)
            .expect("i < via_padstack_count()");
        let (padstack_name, padstack_attach_allowed) = board
            .library
            .get_padstack(current_padstack)
            .map(|p| (p.name.clone(), p.attach_allowed))
            // Java dereferences the `Padstack` reference `getViaPadstack` handed back, so it
            // cannot be missing there; an unresolvable id is a bug in this port's id plumbing.
            .expect("a via padstack id resolves in library.padstacks");
        let via_attach_allowed = attach_allowed && padstack_attach_allowed;
        let via_name = if is_default_class {
            padstack_name
        } else {
            format!("{padstack_name}{CLASS_CLEARANCE_SEPARATOR}{net_class_name}")
        };
        board.rules.via_infos.add(ViaInfo::new(
            via_name,
            current_padstack,
            clearance_class_index,
            via_attach_allowed,
        ));
    }
}

/// `Network.insertViaRules` (Network.java:380-392): the file's via rules, or a generated
/// `"default"` one; then **every** net class is pointed at the *first* via rule, whatever it
/// set itself (`getDefaultViaRule`, BoardRules.java:242-247).
fn insert_via_rules(via_rules: &[Vec<String>], board: &mut Board) {
    let mut rule_found = false;
    for current_list in via_rules {
        if current_list.len() < 2 {
            continue;
        }
        if add_via_rule(current_list, board) {
            rule_found = true;
        }
    }
    if !rule_found {
        let default_net_class = board.rules.get_default_net_class();
        board
            .rules
            .create_default_via_rule(default_net_class, "default", &board.library.padstacks);
    }
    // Network.java:392-394 — every net class is pointed at the *same* `getDefaultViaRule()`
    // object. The port's net class owns a copy of it (Plan 7 Task 11); they start identical and
    // nothing on the reader path mutates a rule afterwards.
    let default_via_rule = board.rules.get_default_via_rule().cloned();
    for i in 0..board.rules.net_classes.count() {
        board
            .rules
            .net_classes
            .get_mut(NetClassId(i))
            .set_via_rule(default_via_rule.clone());
    }
}

/// `Network.addViaRule` (Network.java:394-419): "inserts a via rule into the board. Replaces an
/// already existing via rule with the same" [name]. Returns false — and inserts nothing — when
/// any of the named via infos is missing.
///
/// ~~Port hazard, not a Java one: removing the replaced rule shifts every later `ViaRuleId`~~ —
/// **gone since Plan 7 Task 11.** `io/specctra/RulesReader.java:352-357` calls this on a board
/// whose net classes already have via rules, and a `NetClass` now **owns** its
/// [`ViaRule`](fr_board::ViaRule) rather than indexing `board.rules.via_rules` (ruling H's
/// via-rule half, controller ruling AN) — so the removal shifts nothing a net class can see, and
/// [`BoardRules::replace_via_rule`] is `Network.addViaRule:413-417`'s two lines and nothing else.
/// A class that held the replaced rule keeps the **detached original**, exactly as Java's object
/// reference does.
///
/// (Plan 3 Task 14 had routed the removal through
/// `BoardRules::replace_via_rule_renumbering_net_classes`, which re-pointed every net class at
/// the replacement; `crates/fr-router/tests/data/p7t11-ruling-h-viarule.txt` is the measurement
/// that closed it.)
pub fn add_via_rule(name_list: &[String], board: &mut Board) -> bool {
    let mut it = name_list.iter();
    let rule_name = it
        .next()
        // Java's `it.next()` on an empty list throws `NoSuchElementException`; the only caller
        // guarantees at least two entries (Network.java:383).
        .expect("Network.addViaRule: the name list is never empty");
    let existing_rule = board.rules.get_via_rule(rule_name);
    let mut current_rule = ViaRule::new(rule_name.clone());
    let mut rule_ok = true;
    for via_name in it {
        match board.rules.via_infos.get_by_name(via_name) {
            // Java appends the `ViaInfo` object (Network.java:408); the rule owns a copy.
            Some(current_via) => current_rule.append_via(current_via.clone()),
            // "viaInfo not found" (Network.java:409).
            None => rule_ok = false,
        }
    }
    if rule_ok {
        match existing_rule {
            // Replace already existing rule (Network.java:414-416).
            Some(existing) => {
                board.rules.replace_via_rule(existing, current_rule);
            }
            None => board.rules.via_rules.push(current_rule),
        }
    }
    rule_ok
}

/// `Network.insertNetClasses` (Network.java:421-433).
fn insert_net_classes(classes: &[DsnNetClass], p: &mut ReadScopeParameter<'_>) {
    let layer_structure = p.layer_structure.clone().expect(LAYER_STRUCTURE_EXPECTED);
    let coordinate_transform = p.coordinate_transform.expect(TRANSFORM_EXPECTED);
    let via_at_smd_allowed = p.via_at_smd_allowed;
    let board = p.board.as_mut().expect(BOARD_EXPECTED);
    for current_class in classes {
        insert_net_class(
            current_class,
            &layer_structure,
            board,
            &coordinate_transform,
            via_at_smd_allowed,
        );
    }
}

/// `Network.insertNetClass` (Network.java:435-546): turns one parsed `(class …)` into a board
/// net class — including the KiCad merge (plan ruling 8): a class named `default` or
/// `kicad_default`, in any case, updates freerouting's own default class instead of appending a
/// second one.
pub fn insert_net_class(
    net_class: &DsnNetClass,
    layer_structure: &DsnLayerStructure,
    board: &mut Board,
    coordinate_transform: &CoordinateTransform,
    via_at_smd_allowed: bool,
) {
    let board_net_class = if is_kicad_default_net_class_name(&net_class.name) {
        board.rules.get_default_net_class()
    } else {
        board.rules.append_net_class_named(&net_class.name)
    };
    if let Some(trace_clearance_class) = &net_class.trace_clearance_class {
        // "clearance class not found" is an `FRLogger.warn` only (Network.java:449-455).
        if let Some(no) = board.rules.clearance_matrix.get_no(trace_clearance_class) {
            board
                .rules
                .net_classes
                .get_mut(board_net_class)
                .set_trace_clearance_class(no);
        }
    }
    if let Some(via_rule_name) = &net_class.via_rule {
        // "via rule not found" is an `FRLogger.warn` only (Network.java:461-465).
        if let Some(via_rule) = board.rules.get_via_rule(via_rule_name) {
            let via_rule = board.rules.via_rules[via_rule.0].clone();
            board
                .rules
                .net_classes
                .get_mut(board_net_class)
                .set_via_rule(Some(via_rule));
        }
    }
    if net_class.max_trace_length > 0.0 {
        let value = coordinate_transform.dsn_to_board(net_class.max_trace_length);
        board
            .rules
            .net_classes
            .get_mut(board_net_class)
            .set_maximum_trace_length(value);
    }
    if net_class.min_trace_length > 0.0 {
        let value = coordinate_transform.dsn_to_board(net_class.min_trace_length);
        board
            .rules
            .net_classes
            .get_mut(board_net_class)
            .set_minimum_trace_length(value);
    }
    for current_net_name in &net_class.net_list {
        let net_numbers: Vec<i32> = board
            .rules
            .nets
            .get_by_name(current_net_name)
            .iter()
            .map(|net| net.net_number)
            .collect();
        for net_number in net_numbers {
            board
                .rules
                .nets
                .get_mut(net_number)
                .expect("listed by get_by_name")
                .set_class(board_net_class);
        }
    }

    // read the trace width and clearance rules.

    let mut clearance_rule_found = false;

    for current_rule in &net_class.rules {
        match current_rule {
            DsnRule::Width(value) => {
                // Note the division by 2 happens **before** `dsnToBoard` here, unlike
                // `readNetScope` (Network.java:485 vs :1441).
                let trace_half_width =
                    java_round_to_int(coordinate_transform.dsn_to_board(value / 2.0));
                board
                    .rules
                    .net_classes
                    .get_mut(board_net_class)
                    .set_trace_half_width_on_all_layers(trace_half_width);
            }
            DsnRule::Clearance(rule) => {
                add_clearance_rule(board, board_net_class, rule, None, coordinate_transform);
                clearance_rule_found = true;
            }
        }
    }

    // read the layer dependent rules.

    for current_layer_rule in &net_class.layer_rules {
        for current_layer_name in &current_layer_rule.layer_names {
            // The **board's** layer structure, not the DSN one (Network.java:502).
            let Some(layer_index) = board.layer_structure().get_no(current_layer_name) else {
                // "layer not found" (Network.java:504-508).
                continue;
            };
            for current_rule in &current_layer_rule.rules {
                match current_rule {
                    DsnRule::Width(value) => {
                        let trace_half_width =
                            java_round_to_int(coordinate_transform.dsn_to_board(value / 2.0));
                        board
                            .rules
                            .net_classes
                            .get_mut(board_net_class)
                            .set_trace_half_width(layer_index, trace_half_width);
                    }
                    DsnRule::Clearance(rule) => {
                        add_clearance_rule(
                            board,
                            board_net_class,
                            rule,
                            Some(layer_index),
                            coordinate_transform,
                        );
                        clearance_rule_found = true;
                    }
                }
            }
        }
    }

    board
        .rules
        .net_classes
        .get_mut(board_net_class)
        .set_pull_tight(net_class.pull_tight);
    board
        .rules
        .net_classes
        .get_mut(board_net_class)
        .set_shove_fixed(net_class.shove_fixed);
    let mut via_infos_created = false;

    if clearance_rule_found && board_net_class != board.rules.get_default_net_class() {
        create_default_via_infos(board, board_net_class, via_at_smd_allowed);
        via_infos_created = true;
    }

    if net_class.use_via.is_empty() {
        if via_infos_created {
            let name = board
                .rules
                .net_classes
                .get(board_net_class)
                .get_name()
                .to_string();
            board
                .rules
                .create_default_via_rule(board_net_class, name, &board.library.padstacks);
        }
    } else {
        create_via_rule(&net_class.use_via, board_net_class, board);
    }
    if !net_class.use_layer.is_empty() {
        create_active_trace_layers(
            &net_class.use_layer,
            layer_structure,
            board,
            board_net_class,
        );
    }
}

/// `Network.insertClassPairs` (Network.java:548-576).
///
/// Java bug: Network.insertClassPairs — the inner loop reuses the **outer** iterator
/// (`Iterator<String> it2 = it1;`, :557), so it drains it. A `(class_class (classes A B C) …)`
/// therefore produces the pairs `(A,B)` and `(A,C)` and stops: `(B,C)` is never written, and the
/// outer loop never sees `B` or `C` as a first class. Reproduced verbatim below — the inner
/// `for` borrows the same iterator the `while let` is driving. See `docs/java-quirks.md`.
fn insert_class_pairs(class_classes: &[DsnClassClass], p: &mut ReadScopeParameter<'_>) {
    let coordinate_transform = p.coordinate_transform.expect(TRANSFORM_EXPECTED);
    let board = p.board.as_mut().expect(BOARD_EXPECTED);
    for current_class_class in class_classes {
        let mut it1 = current_class_class.class_names.iter();
        while let Some(first_name) = it1.next() {
            let Some(first_class) = resolve_net_class(&mut board.rules, first_name) else {
                // "first class not found" (Network.java:559).
                continue;
            };
            for second_name in it1.by_ref() {
                let Some(second_class) = resolve_net_class(&mut board.rules, second_name) else {
                    // "second class not found" (Network.java:564).
                    continue;
                };
                insert_class_pair_info(
                    current_class_class,
                    first_class,
                    second_class,
                    board,
                    &coordinate_transform,
                );
            }
        }
    }
}

/// `Network.insertClassPairInfo` (Network.java:578-619): every clearance rule of a
/// `(class_class …)`, applied to the pair of classes.
fn insert_class_pair_info(
    class_class: &DsnClassClass,
    first_class: NetClassId,
    second_class: NetClassId,
    board: &mut Board,
    coordinate_transform: &CoordinateTransform,
) {
    for current_rule in &class_class.rules {
        // "unexpected rule" for anything but a clearance rule (Network.java:594).
        if let DsnRule::Clearance(current_clearance_rule) = current_rule {
            add_mixed_clearance_rule(
                board,
                first_class,
                second_class,
                current_clearance_rule,
                None,
                coordinate_transform,
            );
        }
    }
    for current_layer_rule in &class_class.layer_rules {
        for current_layer_name in &current_layer_rule.layer_names {
            let Some(layer_index) = board.layer_structure().get_no(current_layer_name) else {
                // "layer not found" (Network.java:600-604).
                continue;
            };
            for current_rule in &current_layer_rule.rules {
                if let DsnRule::Clearance(rule) = current_rule {
                    add_mixed_clearance_rule(
                        board,
                        first_class,
                        second_class,
                        rule,
                        Some(layer_index),
                        coordinate_transform,
                    );
                }
            }
        }
    }
}

/// `Network.addMixedClearanceRule` (Network.java:621-676): the clearance between two *different*
/// net classes, written into both halves of the matrix — `(i, j)` **and** `(j, i)`.
fn add_mixed_clearance_rule(
    board: &mut Board,
    first_class: NetClassId,
    second_class: NetClassId,
    clearance_rule: &DsnClearanceRule,
    layer_index: Option<usize>,
    coordinate_transform: &CoordinateTransform,
) {
    let current_clearance =
        java_round_to_int(coordinate_transform.dsn_to_board(clearance_rule.value));
    let first_class_name = board
        .rules
        .net_classes
        .get(first_class)
        .get_name()
        .to_string();
    let first_class_no = match board.rules.clearance_matrix.get_no(&first_class_name) {
        Some(no) => no,
        None => {
            board.rules.clearance_matrix.append_class(&first_class_name);
            board
                .rules
                .clearance_matrix
                .get_no(&first_class_name)
                .expect("appendClass leaves the class present")
        }
    };
    let second_class_name = board
        .rules
        .net_classes
        .get(second_class)
        .get_name()
        .to_string();
    let second_class_no = match board.rules.clearance_matrix.get_no(&second_class_name) {
        Some(no) => no,
        None => {
            board
                .rules
                .clearance_matrix
                .append_class(&second_class_name);
            board
                .rules
                .clearance_matrix
                .get_no(&second_class_name)
                .expect("appendClass leaves the class present")
        }
    };
    if clearance_rule.clearance_class_pairs.is_empty() {
        set_clearance_both_ways(
            board,
            first_class_no,
            second_class_no,
            layer_index,
            current_clearance,
        );
        return;
    }
    for current_string in &clearance_rule.clearance_class_pairs {
        let current_pair = java_split_underscore(current_string);
        if current_pair.len() != 2 {
            continue;
        }
        for i in 0..2 {
            let (current_first_class_no, current_second_class_no) = if i == 0 {
                (
                    get_clearance_class(board, first_class, current_pair[0]),
                    get_clearance_class(board, second_class, current_pair[1]),
                )
            } else {
                (
                    get_clearance_class(board, second_class, current_pair[0]),
                    get_clearance_class(board, first_class, current_pair[1]),
                )
            };
            set_clearance_both_ways(
                board,
                current_first_class_no,
                current_second_class_no,
                layer_index,
                current_clearance,
            );
        }
    }
}

/// The `setValue(i, j, …); setValue(j, i, …)` pair that both `Network.addMixedClearanceRule` and
/// `Network.addClearanceRule` write (Network.java:640-643,655-661,796-801). Not a Java method.
fn set_clearance_both_ways(
    board: &mut Board,
    first_class_no: usize,
    second_class_no: usize,
    layer_index: Option<usize>,
    value: i32,
) {
    match layer_index {
        None => {
            board.rules.clearance_matrix.set_value_on_all_layers(
                first_class_no,
                second_class_no,
                value,
            );
            board.rules.clearance_matrix.set_value_on_all_layers(
                second_class_no,
                first_class_no,
                value,
            );
        }
        Some(layer) => {
            board
                .rules
                .clearance_matrix
                .set_value(first_class_no, second_class_no, layer, value);
            board
                .rules
                .clearance_matrix
                .set_value(second_class_no, first_class_no, layer, value);
        }
    }
}

/// `Network.createDefaultClearanceClasses` (Network.java:678-684) — *not* the same method as
/// `Structure.createDefaultClearanceClasses` (Structure.java:819-824), which takes no net class
/// and names the classes `via`/`smd`/`pin`/`area` outright rather than `<class>-via` and so on.
fn create_default_clearance_classes(board: &mut Board, net_class: NetClassId) {
    get_clearance_class(board, net_class, "via");
    get_clearance_class(board, net_class, "smd");
    get_clearance_class(board, net_class, "pin");
    get_clearance_class(board, net_class, "area");
}

/// `Network.createViaRule` (Network.java:686-708): a via rule named after the net class, holding
/// every via info whose padstack name is one of `use_via` **and** whose clearance class is the
/// class's own default via clearance class.
///
/// not ported: the `boolean attachAllowed` parameter (Network.java:690) — Java never reads it.
fn create_via_rule(use_via: &[String], net_class: NetClassId, board: &mut Board) {
    let net_class_name = board
        .rules
        .net_classes
        .get(net_class)
        .get_name()
        .to_string();
    let mut new_via_rule = ViaRule::new(net_class_name);
    let default_via_cl_class = board
        .rules
        .net_classes
        .get(net_class)
        .default_item_clearance_classes
        .get(ItemClass::Via);
    for current_via_name in use_via {
        for i in 0..board.rules.via_infos.count() {
            let info = board.rules.via_infos.get(ViaInfoId(i));
            if info.get_clearance_class_index() != default_via_cl_class {
                continue;
            }
            // `Padstack.name.equals` — case-sensitive, and against the *raw* `use_via` name,
            // not the `\.\d+`-stripped one the padstack lookup used (Network.java:697).
            let padstack_name = board
                .library
                .get_padstack(info.get_padstack())
                .map(|p| p.name.as_str());
            if padstack_name == Some(current_via_name.as_str()) {
                new_via_rule.append_via(info.clone());
            }
        }
    }
    // Network.java:702-704 — `viaRules.add(newViaRule); netClass.setViaRule(newViaRule)`, one
    // object in two places; the port's net class owns a copy (Plan 7 Task 11).
    board
        .rules
        .net_classes
        .get_mut(net_class)
        .set_via_rule(Some(new_via_rule.clone()));
    board.rules.via_rules.push(new_via_rule);
}

/// `Network.createActiveTraceLayers` (Network.java:710-727): only the named layers stay active,
/// and "currently all inactive layers have tracewidth 0".
fn create_active_trace_layers(
    use_layer: &[String],
    layer_structure: &DsnLayerStructure,
    board: &mut Board,
    net_class: NetClassId,
) {
    let layer_count = layer_structure.layers.len();
    let board_net_class = board.rules.net_classes.get_mut(net_class);
    for i in 0..layer_count {
        board_net_class.set_active_routing_layer(i, false);
    }
    for cur_layer_name in use_layer {
        // totalized: `LayerStructure.getNo` returns -1 for an unknown layer and Java hands that
        // straight to `setActiveRoutingLayer`, which throws `ArrayIndexOutOfBoundsException`
        // (Network.java:717-718). The port skips the layer instead.
        let Some(current_no) = layer_structure.get_no(cur_layer_name) else {
            continue;
        };
        board_net_class.set_active_routing_layer(current_no, true);
    }
    // currently all inactive layers have tracewidth 0.
    for i in 0..layer_count {
        if !board_net_class.is_active_routing_layer(i) {
            board_net_class.set_trace_half_width(i, 0);
        }
    }
}

/// `Network.addClearanceRule` (Network.java:729-803): a net class's own clearance rule. When the
/// class has no clearance class yet, one is appended and seeded with the *maximum* of the new
/// clearance and every existing value, then made the class's default for all four item classes.
fn add_clearance_rule(
    board: &mut Board,
    net_class: NetClassId,
    rule: &DsnClearanceRule,
    layer_index: Option<usize>,
    coordinate_transform: &CoordinateTransform,
) {
    let current_clearance = java_round_to_int(coordinate_transform.dsn_to_board(rule.value));
    let class_name = board
        .rules
        .net_classes
        .get(net_class)
        .get_name()
        .to_string();
    let class_no = match board.rules.clearance_matrix.get_no(&class_name) {
        Some(no) => no,
        None => {
            // class not yet existing, create a new class
            board.rules.clearance_matrix.append_class(&class_name);
            let class_no = board
                .rules
                .clearance_matrix
                .get_no(&class_name)
                .expect("appendClass leaves the class present");
            // set the clearance values of the new class to the maximum of currentClearance and
            // the existing values.
            for i in 1..board.rules.clearance_matrix.get_class_count() {
                for j in 0..board.rules.clearance_matrix.get_layer_count() {
                    let current_value = board
                        .rules
                        .clearance_matrix
                        .get_value(class_no, i, j, false)
                        .max(current_clearance);
                    board
                        .rules
                        .clearance_matrix
                        .set_value(class_no, i, j, current_value);
                    board
                        .rules
                        .clearance_matrix
                        .set_value(i, class_no, j, current_value);
                }
            }
            board
                .rules
                .net_classes
                .get_mut(net_class)
                .default_item_clearance_classes
                .set_all(class_no);
            class_no
        }
    };
    board
        .rules
        .net_classes
        .get_mut(net_class)
        .set_trace_clearance_class(class_no);
    if rule.clearance_class_pairs.is_empty() {
        match layer_index {
            None => board.rules.clearance_matrix.set_value_on_all_layers(
                class_no,
                class_no,
                current_clearance,
            ),
            Some(layer) => {
                board.rules.clearance_matrix.set_value(
                    class_no,
                    class_no,
                    layer,
                    current_clearance,
                );
            }
        }
        return;
    }
    if contains_wire_clearance_pair(&rule.clearance_class_pairs) {
        create_default_clearance_classes(board, net_class);
    }
    for current_string in &rule.clearance_class_pairs {
        let current_pair = java_split_underscore(current_string);
        if current_pair.len() != 2 {
            continue;
        }
        let first_class_no = get_clearance_class(board, net_class, current_pair[0]);
        let second_class_no = get_clearance_class(board, net_class, current_pair[1]);
        set_clearance_both_ways(
            board,
            first_class_no,
            second_class_no,
            layer_index,
            current_clearance,
        );
    }
}

/// `Network.getClearanceClass` (Network.java:805-865): "gets the number of the clearance class
/// with name combined of netClassName and itemClassName. Creates a new class, if that class is
/// not yet existing."
fn get_clearance_class(board: &mut Board, net_class: NetClassId, item_class_name: &str) -> usize {
    let net_class_name = board
        .rules
        .net_classes
        .get(net_class)
        .get_name()
        .to_string();
    let new_class_name = if item_class_name == "wire" {
        net_class_name.clone()
    } else {
        format!("{net_class_name}{CLASS_CLEARANCE_SEPARATOR}{item_class_name}")
    };
    if let Some(found_class_no) = board.rules.clearance_matrix.get_no(&new_class_name) {
        return found_class_no;
    }
    board.rules.clearance_matrix.append_class(&new_class_name);
    let result = board
        .rules
        .clearance_matrix
        .get_no(&new_class_name)
        .expect("appendClass leaves the class present");
    let Some(net_class_no) = board.rules.clearance_matrix.get_no(&net_class_name) else {
        // "clearance class not found" (Network.java:844-849) — Java returns the (valid) new
        // class number without initialising it.
        return result;
    };
    // initialize the clearance values of newClassName from netClassName
    for i in 1..board.rules.clearance_matrix.get_class_count() {
        for j in 0..board.rules.clearance_matrix.get_layer_count() {
            let current_value = board
                .rules
                .clearance_matrix
                .get_value(net_class_no, i, j, false);
            board
                .rules
                .clearance_matrix
                .set_value(result, i, j, current_value);
            board
                .rules
                .clearance_matrix
                .set_value(i, result, j, current_value);
        }
    }
    let default_item_clearance_classes = &mut board
        .rules
        .net_classes
        .get_mut(net_class)
        .default_item_clearance_classes;
    match item_class_name {
        "via" => default_item_clearance_classes.set(ItemClass::Via, result),
        "pin" => default_item_clearance_classes.set(ItemClass::Pin, result),
        "smd" => default_item_clearance_classes.set(ItemClass::Smd, result),
        "area" => default_item_clearance_classes.set(ItemClass::Area, result),
        // Ignore unsupported item classes.
        _ => {}
    }
    result
}

// ------------------------------------------------------------- components and logical parts

/// `Network.insertComponents` (Network.java:867-873): every placed component, in the order the
/// `placement` scope read them.
///
/// The list is moved out of `ReadScopeParameter` and put straight back: Java iterates the live
/// `placementList` while `insertComponent` mutates the board, which the borrow checker will not
/// allow through a shared `&mut ReadScopeParameter`. Nothing here mutates the list itself.
fn insert_components(p: &mut ReadScopeParameter<'_>) {
    let placement_list = std::mem::take(&mut p.placement_list);
    for next_lib_component in &placement_list {
        for next_component in &next_lib_component.locations {
            // fixed: T4 (#103) — a rejected component says so on `ReadScopeParameter.warnings`,
            // the same channel `Wiring.readScope`'s degenerate-wire complaints already use, so a
            // caller that reports warnings reports this one too.
            if let Some(diagnostic) =
                insert_component(next_component, &next_lib_component.lib_name, p)
            {
                p.warnings.push(diagnostic);
            }
        }
    }
    p.placement_list = placement_list;
}

/// `Network.insertComponent` (Network.java:932-1195): "inserts all board components belonging to
/// the input library component" — and, with them, the pins, keepouts and outlines whose ids this
/// task exists to get right.
///
/// The order inside one component is fixed and load-bearing: every package pin
/// (`insertPin`, :1035), then the package keepouts (`insertObstacle`, :1082), the via keepouts
/// (`insertViaObstacle`, :1093) and the place keepouts (`insertComponentObstacle`, :1104), then
/// every package outline (`insertComponentOutline`, :1192).
///
/// `Some(diagnostic)` means the component was **rejected whole** and nothing of it reached the
/// board — see the `#103` marker on the padstack pre-scan below. `None` is the ordinary path,
/// including Java's two silent early returns (package not found, component not placed), which
/// are not this row's business.
fn insert_component(
    location: &ComponentLocation,
    lib_key: &str,
    p: &mut ReadScopeParameter<'_>,
) -> Option<String> {
    let coordinate_transform = p.coordinate_transform.expect(TRANSFORM_EXPECTED);
    let netlist = &p.netlist;
    let board = p.board.as_mut().expect(BOARD_EXPECTED);

    let current_front_package = board
        .library
        .packages
        .get_by_name(lib_key, true)
        .map(|pkg| pkg.no);
    let current_back_package = board
        .library
        .packages
        .get_by_name(lib_key, false)
        .map(|pkg| pkg.no);
    let (Some(current_front_package), Some(current_back_package)) =
        (current_front_package, current_back_package)
    else {
        // "component package not found" (Network.java:940-947).
        return None;
    };

    // Java bug: (#103) `Network.insertComponent`'s pin loop `return`s — it does not `continue` —
    // when a pin names a padstack the library does not have (Network.java:1013-1019). By then
    // the component has been added and its first *n-1* pins inserted, so the board keeps a
    // half-built component and loses **all** of its keepouts, via keepouts, place keepouts and
    // outlines; and because item ids are handed out in insertion order, every later item id on
    // the board shifts.
    //
    // fixed: T4 (#103) — the component is rejected **whole**: the padstacks are checked before
    // anything is inserted, and a miss returns a diagnostic with the board untouched, so no
    // later id moves. (The register offers `continue` as an equal alternative; it is not — a
    // component silently missing one pad is a board that routes to the wrong place.) Hard to
    // reach from a DSN, because `Library.readScope` refuses the whole file for the same
    // condition (Library.java:326-331); live for the KiCad-JSON reader, which has no such guard.
    //
    // `get_package()` is `on_front ? front : back` (Component.java:237-246) and `on_front` is
    // `location.is_front`, so this is the same package the pin loop below reads.
    let package_no = if location.is_front {
        current_front_package
    } else {
        current_back_package
    };
    let package = board.library.packages.get(package_no);
    for i in 0..package.pin_count() {
        let pin = package.get_pin(i as i32).expect("i < pinCount");
        if board.library.padstacks.get(pin.padstack_no).is_none() {
            return Some(format!(
                "component {}: pin {} of package {lib_key} names padstack {:?}, which the \
                 library does not have — the whole component is rejected",
                location.name, pin.name, pin.padstack_no
            ));
        }
    }

    let component_location = location
        .coor
        .map(|coor| coordinate_transform.dsn_to_board_point(&coor).round());
    let rotation_in_degree = location.rotation;

    let new_component_id = board
        .components
        .add(
            location.name.clone(),
            component_location.map(Point::Int),
            rotation_in_degree,
            location.is_front,
            current_front_package,
            current_back_package,
            location.position_fixed,
            location.part_number.clone(),
        )
        .id;

    let Some(component_location) = component_location else {
        // component is not yet placed.
        return None;
    };
    let component_translation = Point::Int(component_location).difference_by(&Point::ZERO);
    let fixed_state = if location.position_fixed {
        FixedState::SystemFixed
    } else {
        FixedState::Unfixed
    };
    let current_package = board.components.get(new_component_id).get_package();
    let pin_count = board.library.packages.get(current_package).pin_count();
    for i in 0..pin_count {
        let current_pin = board
            .library
            .packages
            .get(current_package)
            .get_pin(i as i32)
            .expect("i < pinCount")
            .clone();
        let current_padstack = board
            .library
            .padstacks
            .get(current_pin.padstack_no)
            // fixed: T4 (#103) — Java's "pin padstack not found" `return` (Network.java:1013-1019)
            // was here. Every pin of this package was checked against the library before the
            // component was added (see the pre-scan above), and nothing between that check and
            // this line touches `library.padstacks`, so the miss is not reachable any more.
            .expect("every pin's padstack was checked before the component was added");
        let padstack_is_smd = current_padstack.from_layer() == current_padstack.to_layer();
        let pin_nets: Vec<(String, i32)> = netlist
            .get_nets(&location.name, &current_pin.name)
            .iter()
            .map(|net| (net.id.name.clone(), net.id.subnet_no))
            .collect();
        let mut net_number_array: Vec<i32> = Vec::with_capacity(pin_nets.len());
        for (net_name, subnet_no) in &pin_nets {
            // "board net not found" is an `FRLogger.warn` only (Network.java:1023-1030).
            if let Some(current_board_net) = board
                .rules
                .nets
                .get_by_name_and_subnet(net_name, *subnet_no)
            {
                net_number_array.push(current_board_net.net_number);
            }
        }
        let board_net_class = net_number_array
            .first()
            .and_then(|no| board.rules.nets.get(*no))
            .map(fr_board::Net::get_net_class);
        let net_class = board_net_class.unwrap_or_else(|| board.rules.get_default_net_class());
        let mut clearance_class = location
            .pin_infos
            .get(&current_pin.name)
            .and_then(|pin_info| {
                board
                    .rules
                    .clearance_matrix
                    .get_no(&pin_info.clearance_class)
            });
        if clearance_class.is_none() {
            let default_item_clearance_classes = &board
                .rules
                .net_classes
                .get(net_class)
                .default_item_clearance_classes;
            clearance_class = Some(if padstack_is_smd {
                default_item_clearance_classes.get(ItemClass::Smd)
            } else {
                default_item_clearance_classes.get(ItemClass::Pin)
            });
        }
        board.insert_pin(
            new_component_id,
            i as i32,
            net_number_array,
            clearance_class.expect("assigned above"),
            fixed_state,
        );
    }

    // insert the keepouts belonging to the package (k = 1 for via keepouts)
    for k in 0..=2 {
        let package = board.library.packages.get(current_package);
        let (keepouts, current_keepout_infos) = match k {
            0 => (package.keepouts.clone(), &location.keepout_infos),
            1 => (package.via_keepouts.clone(), &location.via_keepout_infos),
            _ => (
                package.place_keepouts.clone(),
                &location.place_keepout_infos,
            ),
        };
        for current_keepout in &keepouts {
            let mut layer = current_keepout.layer;
            if layer >= board.get_layer_count() as i32 {
                // "keepout layer is to big" (Network.java:1054-1060).
                continue;
            }
            if layer >= 0 && !location.is_front {
                layer = board.get_layer_count() as i32 - current_keepout.layer - 1;
            }
            let default_net_class = board.rules.get_default_net_class();
            let mut clearance_class = board
                .rules
                .net_classes
                .get(default_net_class)
                .default_item_clearance_classes
                .get(ItemClass::Area);
            if let Some(keepout_info) = current_keepout_infos.get(&current_keepout.name) {
                // Note the `> 0`, not `>= 0`: clearance class 0 does not override the default
                // (Network.java:1073-1076).
                if let Some(current_clearance_class) = board
                    .rules
                    .clearance_matrix
                    .get_no(&keepout_info.clearance_class)
                    && current_clearance_class > 0
                {
                    clearance_class = current_clearance_class;
                }
            }
            if let Ok(layer) = usize::try_from(layer) {
                insert_package_keepout(
                    board,
                    k,
                    current_keepout,
                    layer,
                    &component_translation,
                    rotation_in_degree,
                    !location.is_front,
                    clearance_class,
                    new_component_id,
                    fixed_state,
                );
            } else {
                // insert the obstacle on all signal layers
                for j in 0..board.layer_structure().count() {
                    if board.layer_structure().layers[j].is_signal {
                        insert_package_keepout(
                            board,
                            k,
                            current_keepout,
                            j,
                            &component_translation,
                            rotation_in_degree,
                            !location.is_front,
                            clearance_class,
                            new_component_id,
                            fixed_state,
                        );
                    }
                }
            }
        }
    }

    // insert the outline as component keepout
    let package = board.library.packages.get(current_package);
    let outline = package.outline.clone();
    let outline_widths = package.outline_widths.clone();
    let outline_is_closed = package.outline_is_closed.clone();
    let mut courtyard_idx: i32 = -1;
    if let Some(outline) = outline.as_ref()
        && outline.len() > 1
    {
        let mut max_area = -1.0;
        for (i, shape) in outline.iter().enumerate() {
            let area = shape.bounding_box().area();
            if area > max_area {
                max_area = area;
                courtyard_idx = i as i32;
            }
        }
    }
    if let Some(outline) = outline {
        for (i, shape) in outline.iter().enumerate() {
            let mut is_courtyard = i as i32 == courtyard_idx;
            if let Some(widths) = outline_widths.as_ref()
                && i < widths.len()
                && widths[i] == 0.0
            {
                is_courtyard = true;
            }
            let mut is_fabrication = false;
            if !is_courtyard
                && let Some(widths) = outline_widths.as_ref()
                && i < widths.len()
                && widths[i] <= 110.0
            {
                is_fabrication = true;
            }
            let mut is_closed = false;
            if let Some(closed) = outline_is_closed.as_ref()
                && i < closed.len()
            {
                is_closed = closed[i];
            }
            board.insert_component_outline(
                Area::Shape(shape.clone()),
                location.is_front,
                component_translation.clone(),
                rotation_in_degree,
                new_component_id,
                is_courtyard,
                is_fabrication,
                is_closed,
                fixed_state,
            );
        }
    }
    None
}

/// The three-way `k` switch inside `Network.insertComponent`'s keepout loop
/// (Network.java:1080-1110 and its all-signal-layers twin at :1114-1152). Not a Java method —
/// Java writes the same `if (k == 0) … else if (k == 1) … else …` chain twice.
#[allow(clippy::too_many_arguments)]
fn insert_package_keepout(
    board: &mut Board,
    k: i32,
    keepout: &Keepout,
    layer: usize,
    translation: &Vector,
    rotation_in_degree: f64,
    side_changed: bool,
    clearance_class: usize,
    component_id: i32,
    fixed_state: FixedState,
) {
    let area = keepout.area.clone();
    let name = Some(keepout.name.clone());
    match k {
        0 => {
            board.insert_obstacle_of_component(
                area,
                layer,
                translation.clone(),
                rotation_in_degree,
                side_changed,
                clearance_class,
                component_id,
                name,
                fixed_state,
            );
        }
        1 => {
            board.insert_via_obstacle_of_component(
                area,
                layer,
                translation.clone(),
                rotation_in_degree,
                side_changed,
                clearance_class,
                component_id,
                name,
                fixed_state,
            );
        }
        _ => {
            board.insert_component_obstacle_of_component(
                area,
                layer,
                translation.clone(),
                rotation_in_degree,
                side_changed,
                clearance_class,
                component_id,
                name,
                fixed_state,
            );
        }
    }
}

/// `Network.insertLogicalParts` (Network.java:875-930): "create the part library on the board.
/// Can be called after the components are inserted. Returns false, if an error occurred."
fn insert_logical_parts(p: &mut ReadScopeParameter<'_>) -> bool {
    let logical_parts = std::mem::take(&mut p.logical_parts);
    let logical_part_mappings = std::mem::take(&mut p.logical_part_mappings);
    let result = insert_logical_parts_inner(&logical_parts, &logical_part_mappings, p);
    p.logical_parts = logical_parts;
    p.logical_part_mappings = logical_part_mappings;
    result
}

fn insert_logical_parts_inner(
    logical_parts: &[DsnLogicalPart],
    logical_part_mappings: &[DsnLogicalPartMapping],
    p: &mut ReadScopeParameter<'_>,
) -> bool {
    let board = p.board.as_mut().expect(BOARD_EXPECTED);
    for next_part in logical_parts {
        let Some(lib_package) = search_lib_package(&next_part.name, logical_part_mappings, board)
        else {
            return false;
        };
        let mut board_part_pins: Vec<PartPin> = Vec::with_capacity(next_part.part_pins.len());
        for current_part_pin in &next_part.part_pins {
            let Some(pin_index) = board
                .library
                .packages
                .get(lib_package)
                .get_pin_index(&current_part_pin.pin_name)
            else {
                // "package pin not found" (Network.java:889-895).
                return false;
            };
            board_part_pins.push(PartPin::new(
                pin_index as i32,
                current_part_pin.pin_name.clone(),
                current_part_pin.gate_name.clone(),
                current_part_pin.gate_swap_code,
                current_part_pin.gate_pin_name.clone(),
                current_part_pin.gate_pin_swap_code,
            ));
        }
        board
            .library
            .logical_parts
            .add(next_part.name.clone(), board_part_pins);
    }

    for next_mapping in logical_part_mappings {
        // "logical part not found" is an `FRLogger.warn` only, and the `null` is then assigned
        // to every component in the mapping (Network.java:912-919).
        let current_logical_part = board
            .library
            .logical_parts
            .get_by_name(&next_mapping.name)
            .map(|part| part.no);
        for current_cmp_name in &next_mapping.components {
            // "board component not found" is an `FRLogger.warn` only (Network.java:925-929).
            if let Some(component_id) = board
                .components
                .get_by_name(current_cmp_name)
                .map(|component| component.id)
            {
                board
                    .components
                    .get_mut(component_id)
                    .set_logical_part(current_logical_part);
            }
        }
    }
    true
}

/// `Network.searchLibPackage` (Network.java:900-930): "calculates the library package belonging
/// to the logical part with name partName. Returns null, if the package was not found."
fn search_lib_package(
    part_name: &str,
    logical_part_mappings: &[DsnLogicalPartMapping],
    board: &Board,
) -> Option<usize> {
    for current_mapping in logical_part_mappings {
        if current_mapping.name == part_name {
            // "component list empty" (Network.java:910-913, and the redundant `null` check at
            // :915-918 that a non-empty `List<String>` cannot reach).
            let component_name = current_mapping.components.first()?;
            // "component not found" (Network.java:922-926).
            let current_component = board.components.get_by_name(component_name)?;
            return Some(current_component.get_package());
        }
    }
    // "library package '…' not found" (Network.java:928).
    None
}

// ======================================================================= the `network` writers
//
// `Rule.java`'s six writers, `Net.java`'s three and `Network.java`'s six — the DSN write half of
// this file, ported in Plan 3 Task 11. The module-head markers above named them.

// -------------------------------------------------------------------------- Rule.java writers

/// `Rule.writeScope(NetClass, WriteScopeParameter)` (Rule.java:119-137): a net class's `(rule
/// (width …))` scope, followed by one `(layer_rule …)` per layer whose trace width differs.
// renamed: Rule.writeScope -> write_rule_scope (the read half is `read_rule_scope` above).
pub fn write_rule_scope(net_class: &NetClass, p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("rule");

    // write the trace width
    let default_trace_half_width = net_class.get_trace_half_width(0);
    let trace_width = 2.0
        * p.coordinate_transform
            .board_to_dsn(f64::from(default_trace_half_width));
    p.file.new_line();
    p.file.write("(width ");
    p.file.write(&java_double_to_string(trace_width));
    p.file.write(")");
    p.file.end_scope();
    for i in 1..p.board.layer_structure().count() {
        if net_class.get_trace_half_width(i) != default_trace_half_width {
            write_layer_rule(net_class, i, p);
        }
    }
}

/// `Rule.writeLayerRule` (Rule.java:139-161). Note the layer name goes out **raw**, not through
/// the identifier writer (Rule.java:147), and the closing `") "` (:158) carries a trailing
/// space.
fn write_layer_rule(net_class: &NetClass, layer_index: usize, p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    p.file.start_scope_nl();
    p.file.write("layer_rule ");

    let current_board_layer_name = &board.layer_structure().layers[layer_index].name;

    p.file.write(current_board_layer_name);
    p.file.start_scope_nl();
    p.file.write("rule ");

    let current_trace_half_width = net_class.get_trace_half_width(layer_index);

    // write the trace width
    let trace_width = 2.0
        * p.coordinate_transform
            .board_to_dsn(f64::from(current_trace_half_width));
    p.file.new_line();
    p.file.write("(width ");
    p.file.write(&java_double_to_string(trace_width));
    p.file.write(") ");
    p.file.end_scope();
    p.file.end_scope();
}

/// `Rule.writeDefaultRule` (Rule.java:164-201): "writes the default rule as a scope to an output
/// dsn-file" — the width, the default clearance, the smd-to-turn gap and the named clearance
/// rules.
///
/// The trace width is always the **layer-0** width, even when `layer` is not 0 (Rule.java:172);
/// only the clearances are read on `layer`.
// renamed: Rule.writeDefaultRule -> write_default_rule.
pub fn write_default_rule(p: &mut WriteScopeParameter<'_>, layer: usize) {
    p.file.start_scope_nl();
    p.file.write("rule");
    // write the trace width
    let default_half_width =
        crate::parser::structure::default_net_class_trace_half_width(&p.board.rules, 0);
    let trace_width = 2.0
        * p.coordinate_transform
            .board_to_dsn(f64::from(default_half_width));
    p.file.new_line();
    p.file.write("(width ");
    p.file.write(&java_double_to_string(trace_width));
    p.file.write(")");
    // write the default clearance rule
    let default_cl_no = BoardRules::default_clearance_class();
    let default_board_clearance =
        p.board
            .rules
            .clearance_matrix
            .get_value(default_cl_no, default_cl_no, layer, false);
    let default_clearance = p
        .coordinate_transform
        .board_to_dsn(f64::from(default_board_clearance));
    p.file.new_line();
    // write the default clearance
    p.file.write("(clearance ");
    p.file.write(&java_double_to_string(default_clearance));
    p.file.write(")");
    // write the smd_to_turn_gap
    let smd_to_turn_dist = p
        .coordinate_transform
        .board_to_dsn(p.board.rules.get_pin_edge_to_turn_dist());
    p.file.new_line();
    p.file.write("(clearance ");
    p.file.write(&java_double_to_string(smd_to_turn_dist));
    p.file.write(" (type smd_to_turn_gap))");

    // write the named clearance rules from the clearance matrix
    write_named_clearance_rules(p, layer);
    // write_non_default_clearance_rules(scopeParameter, layer, defaultBoardClearance);

    p.file.end_scope();
}

/// [`CLASS_CLEARANCE_SEPARATOR`] as the `&str` [`IndentFileWriter::write`] takes.
///
/// Java's `DsnFile.CLASS_CLEARANCE_SEPARATOR` is a `char` (DsnFile.java:20) that
/// `Rule.writeNonDefaultClearanceRules` passes to `Writer.write(int)` (Rule.java:225); this port
/// keeps the `char` for the readers that split on it and adds this one-character `&str` for the
/// writer, rather than allocating a `String` per loop iteration. `separator_str_matches_char`
/// pins the two together.
const CLASS_CLEARANCE_SEPARATOR_STR: &str = "-";

/// `Rule.writeNonDefaultClearanceRules` (Rule.java:204-230): "write the clearance rules, which
/// are different from the default clearance."
///
/// **Dead code in Java, ported anyway.** Its only call site is the commented-out line in
/// [`write_default_rule`] (Rule.java:198), so nothing in the DSN write path reaches it and no
/// fixture's output depends on it. Kept because the plan asks for all six `Rule` writers and
/// because the outer loop's bound is a live Java bug worth carrying: `i` runs to `clCount`
/// **inclusive** (`i <= clCount`, Rule.java:210) where `j` stops at `clCount - 1`, so the last
/// `i` iteration reads `clMatrix.getValue(clCount, …)` — out of range. Java's `getValue` clamps
/// and returns 0 rather than throwing, and so does this port's.
// Java bug: Rule.writeNonDefaultClearanceRules — `for (int i = 1; i <= clCount; i++)` walks one
// class past the end of the clearance matrix (quirk table).
#[allow(dead_code, reason = "dead in Java too — see the doc comment")]
fn write_non_default_clearance_rules(
    p: &mut WriteScopeParameter<'_>,
    layer: usize,
    default_clearance: i32,
) {
    let cl_count = p.board.rules.clearance_matrix.get_class_count();

    for i in 1..=cl_count {
        for j in i..cl_count {
            let current_board_clearance =
                p.board.rules.clearance_matrix.get_value(i, j, layer, false);

            if current_board_clearance == default_clearance {
                continue;
            }

            let current_clearance = p
                .coordinate_transform
                .board_to_dsn(f64::from(current_board_clearance));
            let name_i = clearance_class_name(&p.board.rules, i).to_string();
            let name_j = clearance_class_name(&p.board.rules, j).to_string();
            p.file.new_line();
            p.file.write("(clearance ");
            p.file.write(&java_double_to_string(current_clearance));
            p.file.write(" (type ");
            p.identifier_type.write(&name_i, &mut p.file);
            p.file.write(CLASS_CLEARANCE_SEPARATOR_STR);
            p.identifier_type.write(&name_j, &mut p.file);
            p.file.write("))");
        }
    }
}

/// `Rule.writeNamedClearanceRules` (Rule.java:233-255): "write the clearance rules for the named
/// classes in the clearance matrix" — the diagonal entry of every class except `default`.
fn write_named_clearance_rules(p: &mut WriteScopeParameter<'_>, layer: usize) {
    let cl_count = p.board.rules.clearance_matrix.get_class_count();

    for i in 1..cl_count {
        if clearance_class_name(&p.board.rules, i) == "default" {
            continue;
        }

        let current_board_clearance = p.board.rules.clearance_matrix.get_value(i, i, layer, false);
        let current_clearance = p
            .coordinate_transform
            .board_to_dsn(f64::from(current_board_clearance));
        let name_i = clearance_class_name(&p.board.rules, i).to_string();

        p.file.new_line();
        p.file.write("(clearance ");
        p.file.write(&java_double_to_string(current_clearance));
        p.file.write(" (type ");
        p.identifier_type.write(&name_i, &mut p.file);
        p.file.write("))");
    }
}

/// `Rule.writeItemClearanceClass` (Rule.java:305-311).
///
/// The literal is 2.3.0's `"(clearance_class "` (plan ruling 1); the clone's HEAD writes
/// `"(clearanceClass "` (:308), which its own lexer cannot read back.
// renamed: Rule.writeItemClearanceClass -> write_item_clearance_class.
pub fn write_item_clearance_class<W: Write>(
    name: &str,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    file.new_line();
    file.write("(clearance_class ");
    identifier_type.write(name, file);
    file.write(")");
}

/// `ClearanceMatrix.getName(int)` (ClearanceMatrix.java:68-74) as the writers use it.
///
/// totalized: Java's `getName` warns and returns `null` for an out-of-range index, and every
/// writer here hands that straight to `IdentifierType.write`, which would NPE. This port writes
/// the empty string instead. No reachable caller sees the difference: every index the writers
/// pass comes from the matrix itself or from an item whose clearance class the reader validated.
pub(crate) fn clearance_class_name(rules: &BoardRules, index: usize) -> &str {
    rules.clearance_matrix.get_name(index).unwrap_or("")
}

// --------------------------------------------------------------------------- Net.java writers

/// `Net.writeScope(WriteScopeParameter, rules.Net, Collection<Pin>)` (Net.java:27-44): one
/// `(net <name> <subnet> (pins …))` scope.
// renamed: Net.writeScope -> write_net_scope.
pub fn write_net_scope(p: &mut WriteScopeParameter<'_>, net_number: i32, pin_list: &[ItemId]) {
    let board = p.board;
    // totalized: Java dereferences `nets.get(i)` in `Network.writeScope` without a null check
    // (Network.java:50); the port skips a missing net. `1..=maxNetNumber` is dense on every
    // board the reader builds, so no reachable caller sees the difference.
    let Some(net) = board.rules.nets.get(net_number) else {
        return;
    };
    let net_name = net.name.clone();
    let subnet_number = net.subnet_number;
    p.file.start_scope_nl();
    write_net_id_parts(&net_name, subnet_number, &mut p.file, &p.identifier_type);
    // write the pins scope
    p.file.start_scope_nl();
    p.file.write("pins");
    for pin_id in pin_list {
        let Some(pin) = board.items.get(pin_id) else {
            continue;
        };
        if pin.contains_net(net_number) {
            write_pin(p, *pin_id);
        }
    }
    p.file.end_scope();
    p.file.end_scope();
}

/// `Net.writeNetId(rules.Net, IndentFileWriter, IdentifierType)` (Net.java:46-54).
// renamed: Net.writeNetId -> write_net_id (the port takes the net's two written fields rather
// than the `rules.Net` object, so `Wiring.writeNet` can call it without a second lookup).
pub fn write_net_id<W: Write>(
    net: &fr_board::Net,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    write_net_id_parts(&net.name, net.subnet_number, file, identifier_type);
}

fn write_net_id_parts<W: Write>(
    name: &str,
    subnet_number: i32,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    file.write("net ");
    identifier_type.write(name, file);
    file.write(" ");
    file.write(&subnet_number.to_string());
}

/// `Net.writePin(WriteScopeParameter, board.model.items.Pin)` (Net.java:56-74): one
/// `<component>-<pin>` entry inside a `(pins …)` scope.
// renamed: Net.writePin -> write_pin.
pub fn write_pin(p: &mut WriteScopeParameter<'_>, pin_id: ItemId) {
    let board = p.board;
    let Some(item) = board.items.get(&pin_id) else {
        return;
    };
    let Item::Pin(pin) = item else {
        return;
    };
    // Java's "component not found" branch (Net.java:61-64) dereferences the very reference it
    // just found to be null, so it can only ever throw; the port simply returns, as the branch
    // was plainly meant to.
    // Java bug: Net.writePin — `FRLogger.warn("… at '" + currentComponent.name + "'")` inside the
    // `currentComponent == null` guard (Net.java:62) NPEs instead of warning.
    let component_id = item.component_id();
    if component_id < 1
        || component_id > i32::try_from(board.components.count()).unwrap_or(i32::MAX)
    {
        // `Components::get` would panic here, mirroring Java's `elementAt(no - 1)` throw
        // (quirk #49); this is the `currentComponent == null` branch, taken as written.
        return;
    }
    let current_component = board.components.get(component_id);
    let component_name = current_component.name.clone();
    let package_no = current_component.get_package();
    let Some(lib_pin) = board
        .library
        .packages
        .get(package_no)
        .get_pin(pin.get_pin_index())
    else {
        // "Net.write_scope: pin number out of range" (Net.java:66-69) — an `FRLogger.warn`
        // this port drops.
        return;
    };
    let lib_pin_name = lib_pin.name.clone();
    p.file.new_line();
    p.identifier_type.write(&component_name, &mut p.file);
    p.file.write("-");
    p.identifier_type.write(&lib_pin_name, &mut p.file);
}

// ----------------------------------------------------------------------- Network.java writers

/// `Network.writeScope` (Network.java:45-56): every net by number, then the via infos, the via
/// rules and the net classes.
// renamed: Network.writeScope -> write_network_scope.
pub fn write_network_scope(p: &mut WriteScopeParameter<'_>) {
    p.file.start_scope_nl();
    p.file.write("network");
    let board_pins = p.board.get_pins();
    for i in 1..=p.board.rules.nets.max_net_number() {
        write_net_scope(p, i, &board_pins);
    }
    write_via_infos(
        &p.board.rules,
        &p.board.library.padstacks,
        &mut p.file,
        &p.identifier_type,
    );
    write_via_rules(&p.board.rules, &mut p.file, &p.identifier_type);
    write_net_classes(p);
    p.file.end_scope();
}

/// `Network.writeViaInfos(BoardRules, IndentFileWriter, IdentifierType)` (Network.java:58-76).
///
/// The extra `padstacks` parameter is this port's: Java's `ViaInfo.getPadstack()` returns the
/// `Padstack` object, where `fr-board`'s returns a [`PadstackId`] (Plan 2's "no object references
/// between model objects" rule), so the name needs a lookup the Java signature does not.
// renamed: Network.writeViaInfos -> write_via_infos.
pub fn write_via_infos<W: Write>(
    rules: &BoardRules,
    padstacks: &fr_board::Padstacks,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    for i in 0..rules.via_infos.count() {
        let current_via = rules.via_infos.get(ViaInfoId(i));
        file.start_scope_nl();
        file.write("via ");
        file.new_line();
        identifier_type.write(current_via.get_name(), file);
        file.write(" ");
        // totalized: Java writes `currentVia.getPadstack().name` unconditionally
        // (Network.java:67); a via info whose padstack id is not in the library would NPE there.
        // The port writes the empty string. Unreachable: `Network.readViaInfo` refuses to build a
        // `ViaInfo` without a padstack.
        let padstack_name = padstacks
            .get(current_via.get_padstack())
            .map_or("", |padstack| padstack.name.as_str());
        identifier_type.write(padstack_name, file);
        file.write(" ");
        identifier_type.write(
            clearance_class_name(rules, current_via.get_clearance_class_index()),
            file,
        );
        if current_via.attach_smd_allowed() {
            file.write(" attach");
        }
        file.end_scope();
    }
}

/// `Network.writeViaRules(BoardRules, IndentFileWriter, IdentifierType)` (Network.java:78-91).
///
/// The literal is 2.3.0's `"via_rule"` (plan ruling 1); the clone's HEAD writes `"viaRule"`.
// renamed: Network.writeViaRules -> write_via_rules.
pub fn write_via_rules<W: Write>(
    rules: &BoardRules,
    file: &mut IndentFileWriter<W>,
    identifier_type: &IdentifierType,
) {
    for current_rule in &rules.via_rules {
        file.start_scope_nl();
        file.write("via_rule");
        file.new_line();
        identifier_type.write(&current_rule.name, file);
        for i in 0..current_rule.via_count() {
            file.write(" ");
            identifier_type.write(current_rule.get_via(i).get_name(), file);
        }
        file.end_scope();
    }
}

/// `Network.writeNetClasses` (Network.java:93-97).
// renamed: Network.writeNetClasses -> write_net_classes.
pub fn write_net_classes(p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    for i in 0..board.rules.net_classes.count() {
        write_net_class(board.rules.net_classes.get(NetClassId(i)), NetClassId(i), p);
    }
}

/// `Network.writeNetClass(rules.NetClass, WriteScopeParameter)` (Network.java:99-150): the
/// `(class …)` scope — the class name, its nets **eight per line**, the trace clearance class, an
/// optional via rule, the width rules, the circuit and the two flags.
///
/// The extra `net_class_id` parameter replaces Java's `nets.get(i).getNetClass() == netClass`
/// object-identity test (Network.java:108), which `fr-board`'s [`NetClassId`]-valued
/// `Net::get_net_class` expresses as an index comparison.
///
/// The two flag literals are 2.3.0's `"(pull_tight off)"` and `"(shove_fixed on)"` (plan
/// ruling 1); the clone's HEAD writes `"(pullTight off)"`/`"(shoveFixed on)"`. So is the
/// `"(via_rule "` on the via-rule line.
// renamed: Network.writeNetClass -> write_net_class.
pub fn write_net_class(
    net_class: &NetClass,
    net_class_id: NetClassId,
    p: &mut WriteScopeParameter<'_>,
) {
    let board = p.board;
    p.file.start_scope_nl();
    p.file.write("class ");
    p.identifier_type.write(net_class.get_name(), &mut p.file);
    const NETS_PER_ROW: usize = 8;
    let mut net_counter = 0usize;
    for i in 1..=board.rules.nets.max_net_number() {
        let Some(net) = board.rules.nets.get(i) else {
            continue;
        };
        if net.get_net_class() == net_class_id {
            if net_counter.is_multiple_of(NETS_PER_ROW) {
                p.file.new_line();
            } else {
                p.file.write(" ");
            }
            p.identifier_type.write(&net.name, &mut p.file);
            net_counter += 1;
        }
    }

    // write the trace clearance class
    let trace_clearance_name =
        clearance_class_name(&board.rules, net_class.get_trace_clearance_class()).to_string();
    write_item_clearance_class(&trace_clearance_name, &mut p.file, &p.identifier_type);

    if let Some(via_rule) = net_class.get_via_rule() {
        // write the via rule — `netClass.getViaRule().name` (Network.java:130), read straight
        // off the rule the class owns since Plan 7 Task 11. (Before that the port held an index
        // and had to `totalized:` the out-of-range case; a net class cannot hold a dangling rule
        // any more, so the guard is gone rather than relaxed.)
        let via_rule_name = via_rule.name.clone();
        p.file.new_line();
        p.file.write("(via_rule ");
        p.identifier_type.write(&via_rule_name, &mut p.file);
        p.file.write(")");
    }

    // write the rules, if they are different from the default rule.
    write_rule_scope(net_class, p);

    write_circuit(net_class, p);

    if !net_class.get_pull_tight() {
        p.file.new_line();
        p.file.write("(pull_tight off)");
    }

    if net_class.is_shove_fixed() {
        p.file.new_line();
        p.file.write("(shove_fixed on)");
    }

    p.file.end_scope();
}

/// `Network.writeCircuit(rules.NetClass, WriteScopeParameter)` (Network.java:152-190): the
/// `(circuit (use_layer …) [(length <max> <min>)])` sub-scope of a `(class …)`.
///
/// The literal is 2.3.0's `"(use_layer"` (plan ruling 1); the clone's HEAD writes `"(useLayer"`.
/// Note the layer names go out **raw**, not through the identifier writer (Network.java:165).
// renamed: Network.writeCircuit -> write_circuit.
fn write_circuit(net_class: &NetClass, p: &mut WriteScopeParameter<'_>) {
    let board = p.board;
    let min_trace_length = net_class.get_minimum_trace_length();
    let max_trace_length = net_class.get_maximum_trace_length();
    p.file.start_scope_nl();
    p.file.write("circuit ");
    p.file.new_line();
    p.file.write("(use_layer");
    let layer_count = net_class.layer_count();
    for i in 0..layer_count {
        if net_class.is_active_routing_layer(i) {
            let name = &board.layer_structure().layers[i].name;
            p.file.write(" ");
            p.file.write(name);
        }
    }
    p.file.write(")");
    if min_trace_length > 0.0 || max_trace_length > 0.0 {
        p.file.new_line();
        p.file.write("(length ");
        let transformed_max_length = if max_trace_length <= 0.0 {
            -1.0
        } else {
            p.coordinate_transform.board_to_dsn(max_trace_length)
        };
        p.file.write(&java_double_to_string(transformed_max_length));
        p.file.write(" ");
        let transformed_min_length = if min_trace_length <= 0.0 {
            0.0
        } else {
            p.coordinate_transform.board_to_dsn(min_trace_length)
        };
        p.file.write(&java_double_to_string(transformed_min_length));
        p.file.write(")");
    }
    p.file.end_scope();
}

#[cfg(test)]
mod tests {
    /// `CLASS_CLEARANCE_SEPARATOR_STR` is the writer's spelling of the reader's
    /// [`CLASS_CLEARANCE_SEPARATOR`]; the two must never drift apart (DsnFile.java:20).
    #[test]
    fn separator_str_matches_char() {
        assert_eq!(
            CLASS_CLEARANCE_SEPARATOR_STR,
            CLASS_CLEARANCE_SEPARATOR.to_string()
        );
    }

    use super::*;

    fn scan(text: &str) -> DsnScanner {
        DsnScanner::new(text)
    }

    #[test]
    fn net_id_orders_by_name_then_subnet() {
        let a = NetId {
            name: "GND".to_string(),
            subnet_no: 1,
        };
        let b = NetId {
            name: "GND".to_string(),
            subnet_no: 2,
        };
        let c = NetId {
            name: "VCC".to_string(),
            subnet_no: 0,
        };
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn read_rule_scope_reads_width_and_clearance() {
        let mut scanner = scan("(width 152.4) (clearance 200 (type via_smd)))");
        let rules = read_rule_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0], DsnRule::Width(152.4));
        let DsnRule::Clearance(clearance) = &rules[1] else {
            panic!("expected a clearance rule");
        };
        assert!((clearance.value - 200.0).abs() < f64::EPSILON);
        assert_eq!(clearance.clearance_class_pairs, vec!["via_smd".to_string()]);
    }

    /// Reads a scope body the way `Network.readScope` would: with the enclosing `(` and the
    /// scope keyword already consumed. The JVM probe (`NetProbe.java`, Task 8 report) does the
    /// same by calling `nextToken()` twice before handing the scanner over.
    fn scan_body(text: &str) -> DsnScanner {
        let mut scanner = scan(text);
        assert_eq!(scanner.next_token().expect("scan"), Some(Token::Open));
        scanner.next_token().expect("scan");
        scanner
    }

    #[test]
    fn read_net_class_scope_reads_every_field_java_keeps() {
        // NetProbe class '(class Power GND VCC (rule (width 200) (clearance 300 (type via_smd)))
        //   (via_rule vr1) (clearance_class cc1) (shove_fixed on) (pull_tight off)
        //   (circuit (length 5000 1000) (use_via v1 v2) (use_layer L1 L2))
        //   (layer_rule L1 (rule (width 150))) (junk x))':
        //   name=Power traceClearanceClass=cc1 nets=[GND, VCC] viaRule=vr1 shoveFixed=true
        //   pullTight=false minTraceLength=1000.0 maxTraceLength=5000.0 useVia=[v1, v2]
        //   useLayer=[L1, L2]
        //     rule width 200.0 / rule clearance 300.0 pairs=[via_smd]
        //     layer_rule layers=[L1] rules=[width 150.0;]
        let mut scanner = scan_body(concat!(
            "(class Power GND VCC (rule (width 200) (clearance 300 (type via_smd))) ",
            "(via_rule vr1) (clearance_class cc1) (shove_fixed on) (pull_tight off) ",
            "(circuit (length 5000 1000) (use_via v1 v2) (use_layer L1 L2)) ",
            "(layer_rule L1 (rule (width 150))) (junk x))",
        ));
        let net_class = read_net_class_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(net_class.name, "Power");
        assert_eq!(net_class.trace_clearance_class.as_deref(), Some("cc1"));
        assert_eq!(
            net_class.net_list,
            vec!["GND".to_string(), "VCC".to_string()]
        );
        assert_eq!(net_class.via_rule.as_deref(), Some("vr1"));
        assert!(net_class.shove_fixed);
        assert!(!net_class.pull_tight);
        assert!((net_class.min_trace_length - 1000.0).abs() < f64::EPSILON);
        assert!((net_class.max_trace_length - 5000.0).abs() < f64::EPSILON);
        assert_eq!(net_class.use_via, vec!["v1".to_string(), "v2".to_string()]);
        assert_eq!(
            net_class.use_layer,
            vec!["L1".to_string(), "L2".to_string()]
        );
        assert_eq!(net_class.rules.len(), 2);
        assert_eq!(net_class.rules[0], DsnRule::Width(200.0));
        assert_eq!(net_class.layer_rules.len(), 1);
        assert_eq!(net_class.layer_rules[0].layer_names, vec!["L1".to_string()]);
        assert_eq!(net_class.layer_rules[0].rules, vec![DsnRule::Width(150.0)]);
    }

    #[test]
    fn a_net_class_with_no_nets_gets_javas_defaults() {
        // NetProbe class '(class Empty (rule (width 1)))':
        //   name=Empty traceClearanceClass=null nets=[] viaRule=null shoveFixed=false
        //   pullTight=true minTraceLength=0.0 maxTraceLength=0.0 useVia=[] useLayer=[]
        //     rule width 1.0
        // `pullTight` defaults to **true** and `shoveFixed` to false (NetClass.java:74-75).
        let mut scanner = scan_body("(class Empty (rule (width 1)))");
        let net_class = read_net_class_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(net_class.name, "Empty");
        assert!(net_class.net_list.is_empty());
        assert!(net_class.trace_clearance_class.is_none());
        assert!(net_class.via_rule.is_none());
        assert!(!net_class.shove_fixed);
        assert!(net_class.pull_tight);
        assert_eq!(net_class.rules, vec![DsnRule::Width(1.0)]);
    }

    #[test]
    fn read_circuit_scope_keeps_the_length_rule_and_the_use_lists() {
        // NetProbe circuit ' (length 5000 1000) (use_via v1 v2) (use_layer L1 L2))':
        //   maxLength=5000.0 minLength=1000.0 useVia=[v1, v2] useLayer=[L1, L2]
        // The first number of `(length …)` is the *maximum* — the order Network.writeCircuit
        // writes them back out in (Network.java:169-186).
        let mut scanner = scan("(length 5000 1000) (use_via v1 v2) (use_layer L1 L2))");
        let circuit = read_circuit_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert!((circuit.max_length - 5000.0).abs() < f64::EPSILON);
        assert!((circuit.min_length - 1000.0).abs() < f64::EPSILON);
        assert_eq!(circuit.use_via, vec!["v1".to_string(), "v2".to_string()]);
        assert_eq!(circuit.use_layer, vec!["L1".to_string(), "L2".to_string()]);
    }

    #[test]
    fn a_circuit_scope_skips_what_java_skips() {
        // NetProbe circuit ' (length 5000 1000 (unit mil)))':
        //   maxLength=5000.0 minLength=1000.0 useVia=[] useLayer=[]
        // NetProbe circuit ' (something else))':
        //   maxLength=0.0 minLength=0.0 useVia=[] useLayer=[]
        let mut scanner = scan("(length 5000 1000 (unit mil)))");
        let circuit = read_circuit_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert!((circuit.max_length - 5000.0).abs() < f64::EPSILON);
        assert!((circuit.min_length - 1000.0).abs() < f64::EPSILON);

        let mut scanner = scan("(something else))");
        let circuit = read_circuit_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(circuit.max_length, 0.0);
        assert_eq!(circuit.min_length, 0.0);
        assert!(circuit.use_via.is_empty());
        assert!(circuit.use_layer.is_empty());
    }

    #[test]
    fn read_class_class_scope_reads_classes_rules_and_layer_rules() {
        // NetProbe classclass ' (classes A B) (rule (width 10))
        //   (layer_rule L1 (rule (width 20))))':
        //   classNames=[A, B]
        //     rule width 10.0
        //     layer_rule layers=[L1] rules=[width 20.0;]
        let mut scanner =
            scan("(classes A B) (rule (width 10)) (layer_rule L1 (rule (width 20))))");
        let class_class = read_class_class_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(
            class_class.class_names,
            vec!["A".to_string(), "B".to_string()]
        );
        assert_eq!(class_class.rules, vec![DsnRule::Width(10.0)]);
        assert_eq!(class_class.layer_rules.len(), 1);
        assert_eq!(class_class.layer_rules[0].rules, vec![DsnRule::Width(20.0)]);
    }

    #[test]
    fn a_layer_rule_with_two_rule_scopes_is_rejected_the_way_java_rejects_it() {
        // NetProbe layerrule ' L1 L2 (rule (width 150)) (rule (clearance 20)))':
        //   WARN Rule.read_layer_rule_scope: rule expected at ''
        //   null
        // `Rule.readScope` eats the `)` that closes its own scope, so the second `(` lands where
        // the loop expects the bare keyword `rule` (Rule.java:87-101).
        let mut scanner = scan("L1 L2 (rule (width 150)) (rule (clearance 20)))");
        assert_eq!(read_layer_rule_scope(&mut scanner).expect("scan"), None);

        // One rule scope is what the format actually supports.
        let mut scanner = scan("L1 L2 (rule (width 150)))");
        let layer_rule = read_layer_rule_scope(&mut scanner)
            .expect("scan")
            .expect("not null");
        assert_eq!(
            layer_rule.layer_names,
            vec!["L1".to_string(), "L2".to_string()]
        );
        assert_eq!(layer_rule.rules, vec![DsnRule::Width(150.0)]);
    }

    #[test]
    fn net_list_iterates_in_net_id_order_not_insertion_order() {
        // `NetList.nets` is a `TreeMap<Net.Id, Net>` (NetList.java:13), so `values()` follows
        // `Net.Id.compareTo` — name first, then subnet number — whatever order `addNet` ran in.
        let mut netlist = NetList::new();
        assert!(netlist.is_empty());
        for id in [
            NetId::new("VCC", 1),
            NetId::new("GND", 2),
            NetId::new("GND", 1),
        ] {
            assert!(netlist.add_net(id).is_some());
        }
        assert_eq!(
            netlist
                .values()
                .map(|net| (net.id.name.clone(), net.id.subnet_no))
                .collect::<Vec<_>>(),
            vec![
                ("GND".to_string(), 1),
                ("GND".to_string(), 2),
                ("VCC".to_string(), 1),
            ]
        );
        // "Returns null, if a net with name already exists in the net list. In this case no new
        // net is added." (NetList.java:21-23)
        assert!(netlist.add_net(NetId::new("GND", 1)).is_none());
        assert!(netlist.contains(&NetId::new("GND", 1)));
        assert!(!netlist.contains(&NetId::new("GND", 3)));
    }

    #[test]
    fn net_id_and_pin_ref_order_by_utf16_code_units_like_string_compare_to() {
        // `Net.Id.compareTo` / `Net.Pin.compareTo` are `String.compareTo` chains, which order by
        // UTF-16 code units: a supplementary character sorts **before** U+E000..U+FFFF, the
        // opposite of `str`'s own `Ord`.
        assert!(NetId::new("\u{10000}", 1) < NetId::new("\u{FFFD}", 1));
        assert!("\u{10000}" > "\u{FFFD}");
        assert!(PinRef::new("\u{10000}", "1") < PinRef::new("\u{FFFD}", "1"));
        assert!(PinRef::new("U1", "\u{10000}") < PinRef::new("U1", "\u{FFFD}"));
        // …and the ordinary cases are unchanged.
        assert!(NetId::new("GND", 1) < NetId::new("GND", 2));
        assert!(NetId::new("GND", 9) < NetId::new("VCC", 1));
        assert!(PinRef::new("R1", "2") < PinRef::new("U1", "1"));
    }

    #[test]
    fn java_split_underscore_drops_trailing_empties_the_way_javas_split_does() {
        // `String.split("_")` with the default limit removes trailing empty strings, and
        // returns the whole input when the separator never occurs.
        assert_eq!(java_split_underscore("via_smd"), vec!["via", "smd"]);
        assert_eq!(java_split_underscore("smd_via_same_net").len(), 4);
        assert_eq!(java_split_underscore("via_"), vec!["via"]);
        assert_eq!(java_split_underscore("_via"), vec!["", "via"]);
        assert_eq!(java_split_underscore("a__b"), vec!["a", "", "b"]);
        assert!(java_split_underscore("_").is_empty());
        assert_eq!(java_split_underscore(""), vec![""]);
        assert_eq!(java_split_underscore("wire"), vec!["wire"]);
    }

    #[test]
    fn kicad_default_net_class_names_are_matched_case_insensitively() {
        // KiCadNetClassNames.java:24-30 (plan ruling 8).
        assert!(is_kicad_default_net_class_name("default"));
        assert!(is_kicad_default_net_class_name("Default"));
        assert!(is_kicad_default_net_class_name("DEFAULT"));
        assert!(is_kicad_default_net_class_name("kicad_default"));
        assert!(is_kicad_default_net_class_name("KiCad_Default"));
        assert!(!is_kicad_default_net_class_name(""));
        assert!(!is_kicad_default_net_class_name("Power"));
        assert!(!is_kicad_default_net_class_name("default2"));
    }

    #[test]
    fn create_ordered_subnets_pairs_consecutive_pins() {
        // "Creates a sequence of subnets with 2 pins from pinList" (Network.java:192-208).
        let pins = vec![
            PinRef::new("U1", "1"),
            PinRef::new("R1", "2"),
            PinRef::new("U1", "3"),
        ];
        let subnets = create_ordered_subnets(&pins);
        assert_eq!(subnets.len(), 2);
        // Each subnet is a `TreeSet`, so the pair comes out in `PinRef` order.
        assert_eq!(
            subnets[0],
            vec![PinRef::new("R1", "2"), PinRef::new("U1", "1")]
        );
        assert_eq!(
            subnets[1],
            vec![PinRef::new("R1", "2"), PinRef::new("U1", "3")]
        );
        assert!(create_ordered_subnets(&[]).is_empty());
        // One pin makes no subnet at all.
        assert!(create_ordered_subnets(&pins[..1]).is_empty());
        // A repeated pin collapses to a one-element subnet (the `TreeSet` again).
        let repeated = vec![PinRef::new("U1", "1"), PinRef::new("U1", "1")];
        assert_eq!(
            create_ordered_subnets(&repeated),
            vec![vec![PinRef::new("U1", "1")]]
        );
    }

    #[test]
    fn set_pins_sorts_and_deduplicates_and_get_nets_searches_every_net() {
        let mut netlist = NetList::new();
        netlist.add_net(NetId::new("GND", 1)).expect("new net");
        netlist.add_net(NetId::new("VCC", 1)).expect("new net");
        // `Net.setPins` wraps the collection in a `TreeSet` (Net.java:81).
        netlist
            .get_net_mut(&NetId::new("GND", 1))
            .expect("present")
            .set_pins([
                PinRef::new("U1", "2"),
                PinRef::new("R1", "1"),
                PinRef::new("U1", "2"),
            ]);
        let pins: Vec<String> = netlist
            .get_net(&NetId::new("GND", 1))
            .expect("present")
            .get_pins()
            .expect("set")
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(pins, vec!["Pin{R1-1}".to_string(), "Pin{U1-2}".to_string()]);

        // A net whose pins were never set is skipped, not dereferenced (NetList.java:50).
        assert!(
            netlist
                .get_net(&NetId::new("VCC", 1))
                .expect("present")
                .get_pins()
                .is_none()
        );
        assert_eq!(netlist.get_nets("U1", "2").len(), 1);
        assert!(netlist.get_nets("U1", "3").is_empty());
    }
}

#[cfg(test)]
mod component_rejection_tests {
    //! #103's directed test. It lives beside the code rather than in `tests/network_scope.rs`
    //! because `insert_component`/`insert_components` are private and the defect is **not
    //! reachable through the DSN reader at all**: `Library.readScope` refuses the whole file
    //! when a package pin names a padstack the library lacks (`library.rs`, "board padstack '…'
    //! not found"), which is exactly what the register row says makes this branch hard to reach
    //! from a DSN. The dangling padstack id is therefore installed directly on the board, which
    //! is the state the KiCad-JSON reader — which has no such guard — can produce.

    use fr_board::{PackagePin, Packages, PadstackId};
    use fr_geometry::{IntVector, Vector};

    use super::*;
    use crate::coordinate_transform::CoordinateTransform;
    use crate::error::BoardReadResult;
    use crate::parser::placement::{ComponentLocation, ComponentPlacement};
    use crate::parser::scope_parameter::DsnReadOptions;

    /// A two-layer board with one padstack in its library and no components at all.
    const DSN: &str = "(pcb t103.dsn\n  (parser\n    (string_quote \")\n  )\n  (resolution um \
                       10)\n  (unit um)\n  (structure\n    (layer F.Cu (type signal))\n    \
                       (layer B.Cu (type signal))\n    (boundary\n      (path pcb 0  0 0  \
                       100000 0  100000 100000  0 100000  0 0)\n    )\n  )\n  (library\n    \
                       (padstack PAD (shape (circle F.Cu 800)) (shape (circle B.Cu 800)) \
                       (attach off))\n  )\n)\n";

    fn board_and_transform() -> (Board, CoordinateTransform) {
        let options = DsnReadOptions::default();
        match crate::dsn_reader::read_board(DSN.as_bytes(), None, Some("t103"), &options) {
            BoardReadResult::Success {
                board,
                coordinate_transform,
                ..
            } => (
                *board.expect("a board"),
                coordinate_transform.expect("a transform"),
            ),
            other => panic!("the fixture must read cleanly: {other:?}"),
        }
    }

    fn pin(name: &str, padstack: PadstackId) -> PackagePin {
        PackagePin::new(name, padstack, Vector::Int(IntVector::new(0, 0)), 0.0)
    }

    /// Both sides of one library package, front and back, as `Library.readScope` builds them.
    ///
    /// The pins are given in order, so `BAD`'s **second** pin is the one that dangles: Java
    /// inserts the first before it returns, which is what shifts every later item id.
    fn package_pair(packages: &mut Packages, name: &str, padstacks: &[PadstackId]) {
        for is_front in [true, false] {
            let pins = padstacks
                .iter()
                .enumerate()
                .map(|(i, p)| pin(&(i + 1).to_string(), *p))
                .collect();
            packages.add(
                name,
                pins,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                is_front,
            );
        }
    }

    fn placed(lib_name: &str, component_name: &str) -> ComponentPlacement {
        ComponentPlacement {
            lib_name: lib_name.to_string(),
            locations: vec![ComponentLocation {
                name: component_name.to_string(),
                coor: Some([1000.0, 1000.0]),
                is_front: true,
                rotation: 0.0,
                position_fixed: false,
                pin_infos: BTreeMap::new(),
                keepout_infos: BTreeMap::new(),
                via_keepout_infos: BTreeMap::new(),
                place_keepout_infos: BTreeMap::new(),
                part_number: None,
            }],
        }
    }

    struct Outcome {
        components: Vec<String>,
        pin_ids: Vec<fr_board::ItemId>,
        warnings: Vec<String>,
    }

    /// Runs `insert_components` over `placement_list` on a fresh board whose `BAD` package
    /// carries `bad_padstack` (a dangling id, in the defect's case) and reports what reached the
    /// board.
    fn run(bad_padstack: Option<PadstackId>, placement_list: Vec<ComponentPlacement>) -> Outcome {
        let (mut board, ct) = board_and_transform();
        let good = PadstackId(
            board
                .library
                .padstacks
                .get_by_name("PAD")
                .expect("the fixture's padstack")
                .no,
        );
        board.library.packages = Packages::new();
        package_pair(
            &mut board.library.packages,
            "BAD",
            &[good, bad_padstack.unwrap_or(good)],
        );
        package_pair(&mut board.library.packages, "GOOD", &[good]);

        let options = DsnReadOptions::default();
        let mut p = ReadScopeParameter::new(DsnScanner::new(""), &options);
        p.board = Some(board);
        p.coordinate_transform = Some(ct);
        p.placement_list = placement_list;
        insert_components(&mut p);

        let board = p.board.take().expect("the board");
        Outcome {
            components: (0..board.components.count())
                .map(|i| board.components.get(i as i32 + 1).name.clone())
                .collect(),
            pin_ids: board
                .items
                .iter()
                .filter(|(_, item)| matches!(item, Item::Pin(_)))
                .map(|(id, _)| *id)
                .collect(),
            warnings: p.warnings,
        }
    }

    /// Java bug #103: `Network.insertComponent` `return`s — not `continue`s — when a pin names a
    /// padstack the library lacks (Network.java:1013-1019), **after** the component has been
    /// added and its first *n-1* pins inserted. The board keeps a half-built component, loses all
    /// of its keepouts and outlines, and — because item ids are handed out in insertion order —
    /// every later item id shifts.
    ///
    /// fixed: T4 (#103): the padstacks are checked before anything is inserted, so a miss rejects
    /// the component whole, with a diagnostic and with the board untouched.
    #[test]
    fn a_component_with_an_absent_padstack_is_rejected_whole() {
        let both = vec![placed("BAD", "C1"), placed("GOOD", "C2")];

        // The control: nothing wrong with either package. Both components land.
        let control = run(None, both.clone());
        assert_eq!(control.components, vec!["C1", "C2"]);
        assert_eq!(control.pin_ids.len(), 3, "C1's two pins and C2's one");
        assert!(control.warnings.is_empty());

        // The reference: `C1` simply absent from the placement list. This is what "unshifted"
        // means — the ids `C2` gets when nothing was inserted before it.
        let alone = run(None, vec![placed("GOOD", "C2")]);
        assert_eq!(alone.components, vec!["C2"]);

        // The defect's input: `C1`'s package names a padstack the library does not have.
        let rejected = run(Some(PadstackId(9999)), both);
        assert_eq!(
            rejected.components,
            vec!["C2"],
            "the rejected component is not added at all — Java adds it and then abandons it"
        );
        assert_eq!(
            rejected.pin_ids, alone.pin_ids,
            "the following component's item ids are unshifted"
        );
        assert_eq!(rejected.warnings.len(), 1);
        assert!(
            rejected.warnings[0].contains("C1") && rejected.warnings[0].contains("rejected"),
            "the diagnostic names the component: {}",
            rejected.warnings[0]
        );
    }
}
