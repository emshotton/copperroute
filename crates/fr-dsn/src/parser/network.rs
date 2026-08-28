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
// added in Task 11: Rule.writeScope, Rule.writeDefaultRule, Rule.writeLayerRule, Rule.writeItemClearanceClass — the rule writers, which belong to the DSN writer half. `writeItemClearanceClass` must emit the 2.3.0 literal `"(clearance_class "`, never HEAD's `"(clearanceClass "` (plan ruling 1).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::error::DsnError;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, LexicalState, Token};
use crate::parser::dsn_file::{
    CLASS_CLEARANCE_SEPARATOR, read_on_off_scope, read_string_list_scope, read_string_scope,
};
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};
use crate::parser::structure::read_via_padstacks;

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
/// Deriving `Ord` over `(component_name, pin_name)` in this order reproduces `Pin.compareTo`
/// (Net.java:115-122) exactly.
// renamed: Net.Pin -> PinRef (`Pin` is `fr_board`'s board item, which this module also names).
// renamed: Net.Pin.compareTo -> the derived `Ord`.
// renamed: Net.Pin.toString -> the `Display` impl below.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PinRef {
    /// `Net.Pin.componentName` (Net.java:107).
    pub component_name: String,
    /// `Net.Pin.pinName` (Net.java:108).
    pub pin_name: String,
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
/// Field order matters: deriving `Ord` over `(name, subnet_no)` in this order reproduces
/// `Id.compareTo` exactly — `this.name.compareTo(other.name)` (plain, **not**
/// `compareToIgnoreCase` — unlike `rules.Net.compareTo`, this one is case-sensitive), falling
/// back to `this.subnetNumber - other.subnetNumber` only when the names are equal.
// renamed: Net.Id.compareTo -> the derived `Ord`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetId {
    /// `Net.Id.name` (Net.java:86).
    pub name: String,
    /// `Net.Id.subnetNumber` (Net.java:87).
    pub subnet_no: i32,
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
// added in Task 11: Net.writeScope, Net.writeNetId, Net.writePin — the `(net …)` writers, which
// belong with `Network.writeScope` in the DSN writer half.
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
    pub fn values(&self) -> std::collections::btree_map::Values<'_, NetId, DsnNet> {
        self.nets.values()
    }

    /// `nets.isEmpty()` — not a Java method (Java's callers reach the private field directly).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nets.is_empty()
    }
}

// --------------------------------------------------------------------------- Network.java
// added in Plan 3: Network.readScope
/// Stub for `Network.readScope` (Network.java) — replaced with the real reader by a later task;
/// for now this just discards the scope's body.
pub fn read_network_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(text: &str) -> DsnScanner {
        DsnScanner::new(text).expect("fits the lexer buffer")
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
