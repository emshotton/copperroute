use super::PcbError;
use crate::kicad::sexpr::Node;

pub(super) fn number(section: &str, text: &str) -> Result<f64, PcbError> {
    let value: f64 = text.parse().unwrap_or(f64::NAN);
    if !value.is_finite() || value.abs() > 100_000.0 {
        return Err(PcbError::new(
            section,
            "Invalid or excessive board coordinate.",
        ));
    }
    Ok(value)
}

pub(super) fn xy(section: &str, node: Option<&Node>) -> Result<(f64, f64), PcbError> {
    let node = node.ok_or_else(|| PcbError::new(section, "Missing coordinate."))?;
    let x = number(section, node.atom(1).unwrap_or(""))?;
    let y = number(section, node.atom(2).unwrap_or(""))?;
    Ok((x, y))
}

pub(super) fn point(section: &str, node: &Node, key: &str) -> Result<(f64, f64), PcbError> {
    xy(section, node.child(key))
}

pub(super) fn optional_number(
    section: &str,
    node: &Node,
    key: &str,
    fallback: f64,
) -> Result<f64, PcbError> {
    match node.value(key) {
        Some(text) => number(section, text),
        None => Ok(fallback),
    }
}

pub(super) fn stroke_margin(section: &str, node: &Node) -> Result<f64, PcbError> {
    let width_source = node.child("stroke").unwrap_or(node);
    Ok(optional_number(section, width_source, "width", 0.0)? / 2.0)
}
