//! The part of KiCad's custom design rules (`.kicad_dru`) that maps onto net classes: minimum
//! clearance, track width, via diameter and hole size, by net class, plus board edge clearance.

use std::collections::BTreeSet;

use super::dto::{CustomClearanceRuleJson, KiCadBoardJson};
use super::pcb::PcbError;
use super::sexpr::{self, Node, Value};

const SECTION: &str = "custom rules";

#[derive(Debug, Clone, PartialEq)]
enum Scope {
    Everything,
    Class(String),
    Pair(String, String),
}

fn length_mm(text: &str) -> Option<f64> {
    let text = text.trim();
    let (number, factor) = if let Some(value) = text.strip_suffix("mm") {
        (value, 1.0)
    } else if let Some(value) = text.strip_suffix("mil") {
        (value, 0.0254)
    } else if let Some(value) = text.strip_suffix("in") {
        (value, 25.4)
    } else if let Some(value) = text.strip_suffix("um") {
        (value, 0.001)
    } else {
        (text, 1.0)
    };
    number
        .trim()
        .parse::<f64>()
        .ok()
        .map(|value| value * factor)
}

fn class_term(term: &str) -> Option<(char, String)> {
    let side = term.chars().next().filter(|c| *c == 'A' || *c == 'B')?;
    let rest = term.get(2..)?;
    let quoted = rest
        .strip_prefix("NetClass==")
        .or_else(|| rest.strip_prefix("hasNetclass(")?.strip_suffix(')'))?;
    let name = quoted
        .strip_prefix('\'')
        .and_then(|q| q.strip_suffix('\''))
        .or_else(|| quoted.strip_prefix('"').and_then(|q| q.strip_suffix('"')))?;
    Some((side, name.to_string()))
}

/// The condition forms KiCad users write for net-class rules, with either quote style and
/// either side first. Anything else is not understood and yields `None`.
fn scope_of(condition: Option<&str>) -> Option<Scope> {
    let Some(condition) = condition else {
        return Some(Scope::Everything);
    };
    let compact: String = condition.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some((left, right)) = compact.split_once("&&") {
        let (a, b) = (class_term(left)?, class_term(right)?);
        return (a.0 != b.0).then_some(Scope::Pair(a.1, b.1));
    }
    if let Some((left, right)) = compact.split_once("||") {
        let (a, b) = (class_term(left)?, class_term(right)?);
        return (a.1 == b.1).then_some(Scope::Class(a.1));
    }
    class_term(&compact).map(|(_, name)| Scope::Class(name))
}

fn min_of(constraint: &Node) -> Option<f64> {
    constraint.value("min").and_then(length_mm)
}

fn atoms(node: &Node) -> impl Iterator<Item = &str> {
    node.values.iter().filter_map(|value| match value {
        Value::Atom(text) => Some(text.as_str()),
        Value::Node(_) => None,
    })
}

/// Raises the board's net-class rules to meet `text`'s constraints and returns a warning for
/// each rule or constraint left out.
pub fn apply_custom_rules(board: &mut KiCadBoardJson, text: &str) -> Result<Vec<String>, PcbError> {
    let root = sexpr::parse(&format!("(rules {text})"))
        .map_err(|error| PcbError::new(SECTION, &error.to_string()))?;
    let mut warnings = Vec::new();
    let mut ignored_constraints = BTreeSet::new();
    let classes = board.netClasses.get_or_insert_with(Vec::new);
    let class_names: Vec<String> = classes.iter().filter_map(|c| c.name.clone()).collect();
    let mut pair_rules: Vec<(String, String, f64)> = Vec::new();
    let mut edge_clearance: Option<f64> = None;

    for rule in root.children("rule") {
        let name = atoms(rule).nth(1).unwrap_or("unnamed").to_string();
        let Some(scope) = scope_of(rule.value("condition")) else {
            warnings.push(format!(
                "Custom rule \"{name}\" has a condition other than net classes and was not applied."
            ));
            continue;
        };
        if rule.child("layer").is_some() {
            warnings.push(format!(
                "Custom rule \"{name}\" is limited to some layers; it was applied on every layer."
            ));
        }
        for constraint in rule.children("constraint") {
            let kind = constraint.atom(1).unwrap_or_default();
            let Some(minimum) = min_of(constraint) else {
                if !matches!(kind, "") {
                    ignored_constraints.insert(kind.to_string());
                }
                continue;
            };
            let targets: Vec<&mut super::dto::NetClassJson> = match &scope {
                Scope::Everything => classes.iter_mut().collect(),
                Scope::Class(class) => classes
                    .iter_mut()
                    .filter(|c| c.name.as_deref() == Some(class.as_str()))
                    .collect(),
                Scope::Pair(..) => Vec::new(),
            };
            match (kind, &scope) {
                ("clearance", Scope::Everything) => {
                    for class in targets {
                        class.clearance = class.clearance.max(minimum);
                    }
                }
                ("clearance", Scope::Class(class)) => {
                    for other in &class_names {
                        pair_rules.push((class.clone(), other.clone(), minimum));
                    }
                }
                ("clearance", Scope::Pair(a, b)) => {
                    pair_rules.push((a.clone(), b.clone(), minimum))
                }
                ("track_width", Scope::Everything | Scope::Class(_)) => {
                    let width = constraint
                        .value("opt")
                        .and_then(length_mm)
                        .unwrap_or(minimum)
                        .max(minimum);
                    for class in targets {
                        class.traceWidth = class.traceWidth.max(width);
                    }
                }
                ("via_diameter", Scope::Everything | Scope::Class(_)) => {
                    for class in targets {
                        class.viaDiameter = class.viaDiameter.max(minimum);
                    }
                }
                ("hole_size", Scope::Everything | Scope::Class(_)) => {
                    for class in targets {
                        class.viaDrill = class.viaDrill.max(minimum);
                        class.viaDiameter = class.viaDiameter.max(class.viaDrill + 0.1);
                    }
                }
                ("edge_clearance", Scope::Everything) => {
                    edge_clearance = Some(edge_clearance.unwrap_or(0.0).max(minimum));
                }
                _ => {
                    ignored_constraints.insert(kind.to_string());
                }
            }
        }
    }

    let class_clearance = |name: &str| {
        board
            .netClasses
            .iter()
            .flatten()
            .find(|c| c.name.as_deref() == Some(name))
            .map_or(0.0, |c| c.clearance)
    };
    let mut resolved: Vec<CustomClearanceRuleJson> = Vec::new();
    for (a, b, minimum) in pair_rules {
        if !class_names.contains(&a) || !class_names.contains(&b) {
            warnings.push(format!(
                "Custom clearance rule names a net class this board does not have: {a} / {b}."
            ));
            continue;
        }
        let value = minimum.max(class_clearance(&a)).max(class_clearance(&b));
        match resolved.iter_mut().find(|rule| {
            let (x, y) = (rule.classA.as_deref(), rule.classB.as_deref());
            (x, y) == (Some(a.as_str()), Some(b.as_str()))
                || (x, y) == (Some(b.as_str()), Some(a.as_str()))
        }) {
            Some(rule) => rule.clearance = rule.clearance.max(value),
            None => resolved.push(CustomClearanceRuleJson {
                classA: Some(a),
                classB: Some(b),
                clearance: value,
            }),
        }
    }
    board
        .clearanceRules
        .get_or_insert_with(Vec::new)
        .extend(resolved);
    if let (Some(edge), Some(outline)) = (edge_clearance, board.outline.as_mut()) {
        outline.clearance = outline.clearance.max(edge);
    }
    if !ignored_constraints.is_empty() {
        warnings.push(format!(
            "Custom rule constraints the router does not use were left to KiCad's DRC: {}.",
            ignored_constraints
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kicad::dto::{NetClassJson, OutlineJson};

    fn board() -> KiCadBoardJson {
        let class = |name: &str, clearance: f64| NetClassJson {
            name: Some(name.to_string()),
            clearance,
            traceWidth: 0.2,
            viaDiameter: 0.6,
            viaDrill: 0.3,
            ..NetClassJson::default()
        };
        KiCadBoardJson {
            netClasses: Some(vec![class("Default", 0.2), class("HV", 0.3)]),
            outline: Some(OutlineJson {
                clearance: 0.2,
                ..OutlineJson::default()
            }),
            ..KiCadBoardJson::default()
        }
    }

    #[test]
    fn net_class_rules_raise_the_matching_rules() {
        let mut board = board();
        let warnings = apply_custom_rules(
            &mut board,
            r#"(version 1)
            (rule "hv" (constraint clearance (min 1.5mm)) (condition "A.NetClass == 'HV' && B.NetClass != 'HV'"))
            (rule "hv pair" (constraint clearance (min 1mm)) (condition "A.NetClass == 'HV' && B.NetClass == 'Default'"))
            (rule "power" (constraint track_width (min 0.4mm) (opt 0.5mm)) (condition "A.hasNetclass('HV')"))
            (rule "global" (constraint clearance (min 8mil)))
            (rule "edge" (constraint edge_clearance (min 0.5mm)))
            (rule "silk" (constraint silk_clearance (min 0.1mm)))"#,
        )
        .expect("the rules parse");
        let classes = board.netClasses.as_ref().unwrap();
        assert!((classes[0].clearance - 0.2032).abs() < 1e-9);
        assert_eq!(classes[1].traceWidth, 0.5);
        let rules = board.clearanceRules.as_ref().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].clearance, 1.0);
        assert_eq!(board.outline.as_ref().unwrap().clearance, 0.5);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("\"hv\" has a condition"))
        );
        assert!(warnings.iter().any(|w| w.contains("silk_clearance")));
    }

    #[test]
    fn lengths_accept_every_kicad_unit() {
        assert_eq!(length_mm("0.5mm"), Some(0.5));
        assert_eq!(length_mm("10mil"), Some(0.254));
        assert_eq!(length_mm("0.25"), Some(0.25));
        assert_eq!(length_mm("100um"), Some(0.1));
    }
}
