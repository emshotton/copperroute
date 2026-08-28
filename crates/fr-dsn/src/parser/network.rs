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
// added in Task 11: Rule.writeScope, Rule.writeDefaultRule, Rule.writeLayerRule, Rule.writeItemClearanceClass — the rule writers, which belong to the DSN writer half. `writeItemClearanceClass` must emit the 2.3.0 literal `"(clearance_class "`, never HEAD's `"(clearanceClass "` (plan ruling 1).

use crate::error::DsnError;
use crate::keyword::Keyword;
use crate::lexer::{DsnScanner, Token};
use crate::parser::dsn_file::CLASS_CLEARANCE_SEPARATOR;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

// ------------------------------------------------------------------------------ Rule.java

/// `io/specctra/parser/Rule.java`'s two concrete rule classes, as one enum (Rule is an abstract
/// class whose subclasses carry no shared state, and every consumer is an `instanceof` chain —
/// Structure.java:614,627,653,678).
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

/// `Net.Id` (Net.java:86-104 — the DSN-parser's own `Net`, not `rules.Net`): the `TreeMap`/
/// `BTreeMap` key for [`ReadScopeParameter::netlist`](crate::parser::scope_parameter::ReadScopeParameter).
///
/// Field order matters: deriving `Ord` over `(name, subnet_no)` in this order reproduces
/// `Id.compareTo` exactly — `this.name.compareTo(other.name)` (plain, **not**
/// `compareToIgnoreCase` — unlike `rules.Net.compareTo`, this one is case-sensitive), falling
/// back to `this.subnetNumber - other.subnetNumber` only when the names are equal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetId {
    /// `Net.Id.name` (Net.java:88).
    pub name: String,
    /// `Net.Id.subnetNumber` (Net.java:89).
    pub subnet_no: i32,
}

/// `io/specctra/parser/Net.java` (the DSN-parser's `Net`, distinct from `rules.Net`): a net as
/// read from a `network` scope, before it is resolved against `rules.Nets`.
///
/// Placeholder — body (the pin set, `Net.setPins`/`getPins`) arrives with `Network.readScope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnNet {
    /// `Net.id` (Net.java:16).
    pub id: NetId,
}

impl DsnNet {
    /// `Net(Id)` (Net.java:22-24).
    #[must_use]
    pub fn new(id: NetId) -> DsnNet {
        DsnNet { id }
    }
}

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
}
