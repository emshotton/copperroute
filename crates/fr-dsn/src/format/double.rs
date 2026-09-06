pub fn format_double(d: f64) -> String {
    if d.is_nan() {
        return "NaN".to_string();
    }
    if d.is_infinite() {
        return if d < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    format!("{d}")
}

pub fn format_float(f: f32) -> String {
    if f.is_nan() {
        return "NaN".to_string();
    }
    if f.is_infinite() {
        return if f < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    format!("{f}")
}

pub fn format_fixed(x: f64, precision: usize) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    format!("{x:.precision$}")
}

pub fn format_placement_rotation(degrees: f64) -> String {
    let rounded = (degrees * 1000.0).round_ties_even() / 1000.0;
    if (rounded - (rounded).round_ties_even()).abs() < 1e-9 {
        return format_fixed(rounded, 0);
    }
    let formatted = format_fixed(rounded, 3);
    if formatted.contains('.') {
        let trimmed = formatted.trim_end_matches('0');
        return trimmed.strip_suffix('.').unwrap_or(trimmed).to_string();
    }
    formatted
}
