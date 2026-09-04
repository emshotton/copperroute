use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const MARKER_BASELINE: usize = 165;

const CODE_MARKER_BASELINE: usize = 153;

const REGISTER_ROWS: usize = 297;

fn workspace_root() -> PathBuf {
    parity::workspace_root()
}

fn register_text() -> String {
    let path = workspace_root().join("docs/java-quirks.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn register_rows(text: &str) -> Vec<(u32, String)> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix('|') else {
            continue;
        };
        let Some((first, _)) = rest.split_once('|') else {
            continue;
        };
        let Ok(id) = first.trim().parse::<u32>() else {
            continue;
        };
        let trimmed = line.trim_end();
        let body = trimmed.strip_suffix('|').unwrap_or(trimmed);
        let status = body
            .rsplit('|')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        rows.push((id, status));
    }
    rows
}

fn all_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(&workspace_root().join("crates"), &mut out);
    let this = workspace_root().join(file!());
    out.retain(|p| *p != this);
    out.sort();
    out
}

fn rust_sources() -> Vec<PathBuf> {
    let mut out = all_sources();
    out.retain(|p| p.extension().is_some_and(|e| e == "rs"));
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn java_bug_markers() -> Vec<(PathBuf, usize, String)> {
    let mut out = Vec::new();
    for path in all_sources() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if line.contains("// Java bug:") {
                out.push((path.clone(), i + 1, line.trim().to_string()));
            }
        }
    }
    out
}

fn ids_in(line: &str) -> BTreeSet<u32> {
    let mut ids = BTreeSet::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > start {
                if let Ok(n) = line[start..end].parse::<u32>() {
                    ids.insert(n);
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    ids
}

#[test]
fn the_register_is_contiguous() {
    let text = register_text();
    let rows = register_rows(&text);
    let ids: Vec<u32> = rows.iter().map(|(id, _)| *id).collect();
    let expected: Vec<u32> = (1..=REGISTER_ROWS as u32).collect();
    assert_eq!(
        ids, expected,
        "docs/java-quirks.md's numbered table must be contiguous 1..{REGISTER_ROWS} in write \
         order with no gap and no duplicate; it is the arithmetic note at the bottom of the file, \
         and ruling BL6 allocates from it"
    );
    for (id, status) in &rows {
        let head = {
            let h = status.split_once(" (").map_or(status.as_str(), |(h, _)| h);
            h.split_once(" — ").map_or(h, |(h, _)| h).trim()
        };
        let ok = head == "pinned"
            || head == "totalized"
            || head == "candidate"
            || head == "keep"
            || head.starts_with("fixed: T");
        assert!(
            ok,
            "row #{id}'s status is {status:?}, which is none of pinned / fixed: T<n> / totalized \
             / candidate / keep"
        );
    }
}

#[test]
fn every_java_bug_marker_has_a_register_row() {
    let text = register_text();
    let known: BTreeSet<u32> = register_rows(&text).into_iter().map(|(id, _)| id).collect();
    let markers = java_bug_markers();
    let code = markers
        .iter()
        .filter(|(p, _, _)| p.extension().is_some_and(|e| e == "rs"))
        .count();
    assert!(
        markers.len() >= MARKER_BASELINE && code >= CODE_MARKER_BASELINE,
        "the `// Java bug:` census under crates/ is {} ({code} of them at code sites) and survey \
         §9.1's figures of record are {MARKER_BASELINE} / {CODE_MARKER_BASELINE}; a marker may be \
         added but **never deleted** — a Plan 9 fix adds `// fixed: T<n>` beside the marker and \
         leaves the marker itself alone",
        markers.len()
    );
    let mut orphans = Vec::new();
    for (path, line_no, line) in &markers {
        for id in ids_in(line) {
            if !known.contains(&id) {
                orphans.push(format!("{}:{line_no} names #{id}", path.display()));
            }
        }
    }
    assert!(
        orphans.is_empty(),
        "these `// Java bug:` markers name a register id docs/java-quirks.md does not carry:\n{}",
        orphans.join("\n")
    );
}

#[test]
fn a_fixed_row_has_a_fixed_marker() {
    let text = register_text();
    let fixed: Vec<(u32, String)> = register_rows(&text)
        .into_iter()
        .filter(|(_, status)| status.starts_with("fixed: T"))
        .collect();
    if fixed.is_empty() {
        return;
    }

    let mut fixed_markers: BTreeMap<(String, u32), usize> = BTreeMap::new();
    let mut bug_sites: BTreeMap<u32, usize> = BTreeMap::new();
    for (_, _, line) in java_bug_markers() {
        for id in ids_in(&line) {
            *bug_sites.entry(id).or_default() += 1;
        }
    }
    for path in rust_sources() {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in source.lines() {
            let Some(rest) = line.split("// fixed: T").nth(1) else {
                continue;
            };
            let task: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if task.is_empty() {
                continue;
            }
            for id in ids_in(rest) {
                *fixed_markers.entry((format!("T{task}"), id)).or_default() += 1;
            }
        }
    }

    let mut missing = Vec::new();
    for (id, status) in &fixed {
        let task: String = status["fixed: ".len()..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        let marked = fixed_markers
            .get(&(task.clone(), *id))
            .copied()
            .unwrap_or_default();
        if marked == 0 {
            missing.push(format!(
                "row #{id} says {status:?} but no `// fixed: {task} (#{id})` marker exists (a marker naming another row's id does not close this one)"
            ));
            continue;
        }
        let sites = bug_sites.get(id).copied().unwrap_or_default();
        if marked < sites {
            missing.push(format!(
                "row #{id} says {status:?} and {sites} `// Java bug:` markers name #{id}, but                  only {marked} of them has a `// fixed: {task} (#{id})` beside it — the rule is                  one marker per site, not one per row"
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "a register row may only claim `fixed: T<n>` once the fix is in the code, with a `// fixed: T<n> (#id)` marker naming that row beside the `// Java bug:` one at every site:\n{}",
        missing.join("\n")
    );
}
