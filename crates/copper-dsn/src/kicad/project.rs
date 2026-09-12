use std::collections::HashSet;
use std::fmt;

use serde_json::Value;

use crate::kicad::dto::{KiCadBoardJson, NetClassJson};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetClassProjectError(pub String);

impl fmt::Display for NetClassProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NetClassProjectError {}

pub fn apply_net_classes(
    board: &mut KiCadBoardJson,
    project_json: &str,
) -> Result<(), NetClassProjectError> {
    if project_json.is_empty() {
        return Ok(());
    }
    let project: Value = serde_json::from_str(project_json)
        .map_err(|error| NetClassProjectError(format!("Invalid KiCad project JSON: {error}")))?;
    if project.pointer("/board/design_settings").is_none() {
        return Err(NetClassProjectError(
            "Project has no board.design_settings.".to_string(),
        ));
    }
    let minimum = project
        .pointer("/board/design_settings/rules")
        .cloned()
        .unwrap_or(Value::Null);
    let fallback = board
        .netClasses
        .get_or_insert_with(Vec::new)
        .first()
        .cloned()
        .unwrap_or_default();

    let source = project
        .pointer("/net_settings/classes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut classes: Vec<NetClassJson> = source
        .iter()
        .map(|class| NetClassJson {
            viaInPadAllowed: None,
            name: class
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string),
            clearance: floor(
                class,
                "clearance",
                fallback.clearance,
                &minimum,
                "min_clearance",
            ),
            traceWidth: floor(
                class,
                "track_width",
                fallback.traceWidth,
                &minimum,
                "min_track_width",
            ),
            viaDiameter: floor(
                class,
                "via_diameter",
                fallback.viaDiameter,
                &minimum,
                "min_via_diameter",
            ),
            viaDrill: floor(
                class,
                "via_drill",
                fallback.viaDrill,
                &minimum,
                "min_through_hole_diameter",
            ),
            netNames: Some(Vec::new()),
        })
        .collect();

    if !classes
        .iter()
        .any(|class| class.name.as_deref() == Some("Default"))
    {
        classes.insert(
            0,
            NetClassJson {
                netNames: Some(Vec::new()),
                ..fallback
            },
        );
    }

    for class in &classes {
        let name_ok = class.name.as_deref().is_some_and(|name| !name.is_empty());
        let dims_ok = [
            class.clearance,
            class.traceWidth,
            class.viaDiameter,
            class.viaDrill,
        ]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0);
        if !name_ok || !dims_ok || class.viaDrill >= class.viaDiameter {
            let shown = class.name.as_deref().unwrap_or("undefined");
            return Err(NetClassProjectError(format!(
                "Invalid dimensions for project net class {shown}"
            )));
        }
    }

    let assignments = project
        .pointer("/net_settings/netclass_assignments")
        .and_then(Value::as_object)
        .cloned();
    let patterns = project
        .pointer("/net_settings/netclass_patterns")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for net in board.nets.get_or_insert_with(Vec::new).iter_mut() {
        let net_name = net.name.clone().unwrap_or_default();
        let raw = match assignments.as_ref().and_then(|map| map.get(&net_name)) {
            None | Some(Value::Null) => pattern_names(&patterns, &net_name)?,
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| item.as_str().map(str::to_string))
                .collect(),
            Some(Value::String(name)) => vec![Some(name.clone())],
            Some(_) => vec![None],
        };

        let mut seen = HashSet::new();
        let mut names = Vec::new();
        for candidate in raw.into_iter().flatten() {
            if !candidate.is_empty() && seen.insert(candidate.clone()) {
                names.push(candidate);
            }
        }
        if names.len() > 1 {
            return Err(NetClassProjectError(format!(
                "Composite net classes for {net_name} are not supported yet."
            )));
        }
        let name = names
            .into_iter()
            .next()
            .unwrap_or_else(|| "Default".to_string());
        let Some(class) = classes
            .iter_mut()
            .find(|class| class.name.as_deref() == Some(name.as_str()))
        else {
            return Err(NetClassProjectError(format!(
                "Unknown project net class: {name}"
            )));
        };
        net.className = Some(name);
        class.netNames.get_or_insert_with(Vec::new).push(net_name);
    }

    board.netClasses = Some(classes);
    Ok(())
}

fn floor(class: &Value, key: &str, fallback: f64, minimum: &Value, minimum_key: &str) -> f64 {
    let base = match class.get(key) {
        None | Some(Value::Null) => fallback,
        Some(value) => dimension_number(value),
    };
    let floor = minimum
        .get(minimum_key)
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if base.is_nan() { base } else { base.max(floor) }
}

fn dimension_number(value: &Value) -> f64 {
    match value {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        _ => f64::NAN,
    }
}

fn pattern_names(
    patterns: &[Value],
    net_name: &str,
) -> Result<Vec<Option<String>>, NetClassProjectError> {
    let mut result = Vec::new();
    for pattern in patterns {
        let text = pattern.get("pattern").and_then(Value::as_str).unwrap_or("");
        if glob_matches(text, net_name)? {
            result.push(
                pattern
                    .get("netclass")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            );
        }
    }
    Ok(result)
}

/// KiCad's own net-class pattern syntax is richer than this; only whole-name wildcards
/// (`*` and `?`) are supported, matching `web/project.js`'s `matches`.
fn glob_matches(pattern: &str, name: &str) -> Result<bool, NetClassProjectError> {
    if pattern.chars().any(|c| matches!(c, '[' | ']' | '{' | '}')) {
        return Err(NetClassProjectError(format!(
            "Unsupported project net pattern: {pattern}"
        )));
    }
    Ok(glob_match(pattern, name))
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut star_ti = 0usize;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(star_pi) = star {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_matches_any_run_including_empty() {
        assert!(glob_match("USB_*", "USB_"));
        assert!(glob_match("USB_*", "USB_D+"));
        assert!(!glob_match("USB_*", "VCC"));
    }

    #[test]
    fn question_mark_matches_exactly_one_character() {
        assert!(glob_match("D?", "D0"));
        assert!(!glob_match("D?", "D"));
        assert!(!glob_match("D?", "D00"));
    }

    #[test]
    fn brackets_and_braces_are_rejected() {
        assert!(glob_matches("D[0-9]", "D0").is_err());
        assert!(glob_matches("D{0,1}", "D0").is_err());
    }
}
