use crate::FileFormat;
use copper_router::score::BoardStatistics;

pub trait BoardStatisticsExt {
    fn from_bytes(data: &[u8], format: FileFormat) -> BoardStatistics;
}

impl BoardStatisticsExt for BoardStatistics {
    fn from_bytes(data: &[u8], format: FileFormat) -> BoardStatistics {
        let mut stats = BoardStatistics::default();

        match format {
            FileFormat::Ses => ses_branch(&decode_utf8(data), &mut stats),
            FileFormat::Dsn => dsn_branch(&decode_utf8(data), &mut stats),
            FileFormat::KicadDesignJson => kicad_design_json_branch(&decode_utf8(data), &mut stats),
            FileFormat::Unknown
            | FileFormat::Frb
            | FileFormat::Rules
            | FileFormat::Scr
            | FileFormat::DrcJson
            | FileFormat::KicadSessionJson => {}
        }

        stats
    }
}

fn decode_utf8(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}

pub fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut index = 0;
    while let Some(found) = haystack[index..].find(needle) {
        count += 1;
        index += found + needle.len();
    }
    count
}

fn split_dropping_trailing_empty<'a>(s: &'a str, separator: &str) -> Vec<&'a str> {
    if !s.contains(separator) {
        return vec![s];
    }
    let mut parts: Vec<&str> = s.split(separator).collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

fn ses_branch(content: &str, stats: &mut BoardStatistics) {
    let lines = split_dropping_trailing_empty(content, "(path ");

    let mut layers: Vec<&str> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let words = split_dropping_trailing_empty(line, " ");
        if i > 0 && words.len() >= 2 {
            let layer = words[0];
            if !layers.contains(&layer) {
                layers.push(layer);
            }
        }
    }

    stats.layers.total_count = Some(layers.len() as i32);
    stats.components.total_count = Some(count_occurrences(content, "(component") as i32);
    stats.nets.total_count = Some(count_occurrences(content, "(net") as i32);
    stats.traces.total_count = Some(count_occurrences(content, "(wire") as i32);
    stats.vias.total_count = Some(count_occurrences(content, "(via") as i32);
}

fn dsn_branch(content: &str, stats: &mut BoardStatistics) {
    let mut host_cad: Option<String> = None;
    let mut host_version: Option<String> = None;

    if let Some(parser_index) = content.find("(parser") {
        let mut search_limit = match content[parser_index..].find(')') {
            Some(offset) => (content.len()).min(parser_index + offset + 1),
            None => (content.len()).min(parser_index + 1000),
        };
        while !content.is_char_boundary(search_limit) {
            search_limit += 1;
        }
        let parser_scope = &content[parser_index..search_limit];

        if let Some(hc_idx) = parser_scope.find("(hostCad")
            && let Some(hc_end) = parser_scope[hc_idx..].find(')').map(|o| hc_idx + o)
        {
            let value = slice_totalized(parser_scope, hc_idx + 9, hc_end);
            host_cad = Some(remove_quotes((value).trim()).to_string());
        }
        if let Some(hv_idx) = parser_scope.find("(hostVersion")
            && let Some(hv_end) = parser_scope[hv_idx..].find(')').map(|o| hv_idx + o)
        {
            let value = slice_totalized(parser_scope, hv_idx + 13, hv_end);
            host_version = Some(remove_quotes((value).trim()).to_string());
        }
    }

    match (host_cad, host_version) {
        (Some(cad), Some(version)) => stats.host = format!("{cad},{version}"),
        (Some(cad), None) => stats.host = cad,
        _ => {}
    }

    stats.layers.total_count = Some(count_occurrences(content, "(layer") as i32);
    stats.components.total_count = Some(count_occurrences(content, "(component") as i32);
    stats.nets.class_count = Some(count_occurrences(content, "(class") as i32);
    stats.nets.total_count = Some(count_occurrences(content, "(net") as i32);
    stats.traces.total_count = Some(count_occurrences(content, "(wire") as i32);
    stats.vias.total_count = Some(count_occurrences(content, "(via") as i32);
}

fn kicad_design_json_branch(content: &str, stats: &mut BoardStatistics) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return;
    };
    let Some(object) = value.as_object() else {
        return;
    };

    for (key, field) in [
        ("layers", ArrayField::Layers),
        ("components", ArrayField::Components),
        ("netClasses", ArrayField::NetClasses),
        ("nets", ArrayField::Nets),
        ("traces", ArrayField::Traces),
        ("vias", ArrayField::Vias),
    ] {
        if let Some(member) = object.get(key) {
            let Some(array) = member.as_array() else {
                return;
            };
            let count = Some(array.len() as i32);
            match field {
                ArrayField::Layers => stats.layers.total_count = count,
                ArrayField::Components => stats.components.total_count = count,
                ArrayField::NetClasses => stats.nets.class_count = count,
                ArrayField::Nets => stats.nets.total_count = count,
                ArrayField::Traces => stats.traces.total_count = count,
                ArrayField::Vias => stats.vias.total_count = count,
            }
        }
    }

    if object.contains_key("designName") {
        let Some(raw) = raw_member(content, "designName") else {
            return;
        };
        let Some(text) = get_as_string(raw) else {
            return;
        };
        stats.host = format!("KiCad JSON,{text}");
    }
}

#[derive(Clone, Copy)]
enum ArrayField {
    Layers,
    Components,
    NetClasses,
    Nets,
    Traces,
    Vias,
}

fn get_as_string(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let first = *raw.as_bytes().first()?;
    match first {
        b'"' => serde_json::from_str::<String>(raw).ok(),
        b'[' => match raw_array_elements(raw)?.as_slice() {
            [only] => get_as_string(only),
            _ => None,
        },
        b'{' => None,
        _ if raw == "null" => None,
        _ => Some(raw.to_string()),
    }
}

fn raw_member<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    let bytes = content.as_bytes();
    let mut i = skip_whitespace(bytes, 0);
    if bytes.get(i) != Some(&b'{') {
        return None;
    }
    i += 1;
    let mut found = None;
    loop {
        i = skip_whitespace(bytes, i);
        match bytes.get(i) {
            Some(b',') => {
                i += 1;
                continue;
            }
            Some(b'"') => {}
            _ => return found,
        }
        let key_end = scan_value_end(bytes, i)?;
        let name = serde_json::from_str::<String>(&content[i..key_end]).ok()?;
        i = skip_whitespace(bytes, key_end);
        if bytes.get(i) != Some(&b':') {
            return found;
        }
        i = skip_whitespace(bytes, i + 1);
        let value_end = scan_value_end(bytes, i)?;
        if name == key {
            found = Some(&content[i..value_end]);
        }
        i = value_end;
    }
}

fn raw_array_elements(raw: &str) -> Option<Vec<&str>> {
    let bytes = raw.as_bytes();
    let mut i = skip_whitespace(bytes, 0);
    if bytes.get(i) != Some(&b'[') {
        return None;
    }
    i += 1;
    let mut elements = Vec::new();
    loop {
        i = skip_whitespace(bytes, i);
        match bytes.get(i) {
            Some(b',') => {
                i += 1;
                continue;
            }
            Some(b']') | None => return Some(elements),
            Some(_) => {}
        }
        let end = scan_value_end(bytes, i)?;
        elements.push(&raw[i..end]);
        i = end;
    }
}

fn scan_value_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    match *bytes.get(i)? {
        b'"' => {
            i += 1;
            while let Some(&b) = bytes.get(i) {
                match b {
                    b'\\' => i += 2,
                    b'"' => return Some(i + 1),
                    _ => i += 1,
                }
            }
            None
        }
        open @ (b'[' | b'{') => {
            let close = if open == b'[' { b']' } else { b'}' };
            let mut depth = 0_usize;
            while let Some(&b) = bytes.get(i) {
                if b == b'"' {
                    i = scan_value_end(bytes, i)?;
                } else if b == open {
                    depth += 1;
                    i += 1;
                } else if b == close {
                    depth -= 1;
                    i += 1;
                    if depth == 0 {
                        return Some(i);
                    }
                } else {
                    i += 1;
                }
            }
            None
        }
        _ => {
            while let Some(&b) = bytes.get(i) {
                if b.is_ascii_whitespace() || matches!(b, b',' | b']' | b'}' | b':') {
                    break;
                }
                i += 1;
            }
            (i > start).then_some(i)
        }
    }
}

fn skip_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while matches!(bytes.get(i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        i += 1;
    }
    i
}

fn remove_quotes(text: &str) -> &str {
    if text.len() < 2 {
        return text;
    }
    if text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

fn slice_totalized(text: &str, begin: usize, end: usize) -> &str {
    let mut begin = (begin).min(end);
    while !text.is_char_boundary(begin) {
        begin += 1;
    }
    &text[begin..end]
}
