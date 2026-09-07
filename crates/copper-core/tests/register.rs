//! The port is a total fork of the Java original; git history is the record of that history,
//! not the source tree. This is the absence gate: it fails if parity-era provenance scaffolding
//! (comments citing Java source lines, task/plan/ruling/quirk markers, or the retired
//! `docs/java-quirks.md` register) reappears in `crates/*/src`.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    testkit::workspace_root()
}

/// Literal needles checked against every comment line (`//`, `///`, `//!`) in `crates/*/src`.
/// A needle here is provenance shorthand, not prose that could show up in a legitimate comment.
const NEEDLES: &[&str] = &[
    "Java",
    ".java:",
    "fixed: T",
    "pinned: #",
    "not ported:",
    "totalized:",
    "renamed:",
    "obligation:",
    "pub seam:",
    "not reachable:",
    "docs/java-quirks",
    "java_quirks",
    "quirk",
    "AGENTS.md",
    "global-constraints.md",
    "controller ruling",
    "the register",
    "HEAD's",
    "HEAD is the authority",
    "survey group",
];

/// `word` followed by whitespace and a digit, e.g. `"Task 12"` or `"Plan 3"`.
fn contains_word_then_digit(line: &str, word: &str) -> bool {
    let mut rest = line;
    while let Some(idx) = rest.find(word) {
        let after = &rest[idx + word.len()..];
        let trimmed = after.trim_start_matches(' ');
        if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return true;
        }
        rest = &rest[idx + word.len()..];
    }
    false
}

fn flag(line: &str) -> Option<&'static str> {
    for needle in NEEDLES {
        if line.contains(needle) {
            return Some(needle);
        }
    }
    if contains_word_then_digit(line, "Task ") {
        return Some("Task <n>");
    }
    if contains_word_then_digit(line, "Plan ") {
        return Some("Plan <n>");
    }
    if line.contains("ruling ") {
        return Some("ruling <id>");
    }
    None
}

fn rs_files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rs_files_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_provenance_markers_remain_in_crate_sources() {
    let root = workspace_root();
    let crates_dir = root.join("crates");
    let mut src_dirs = Vec::new();
    for entry in std::fs::read_dir(&crates_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", crates_dir.display()))
        .flatten()
    {
        let src = entry.path().join("src");
        if src.is_dir() {
            src_dirs.push(src);
        }
    }
    assert!(
        !src_dirs.is_empty(),
        "expected at least one crates/*/src directory under {}",
        crates_dir.display()
    );

    let mut files = Vec::new();
    for dir in &src_dirs {
        rs_files_under(dir, &mut files);
    }
    assert!(
        !files.is_empty(),
        "expected to find .rs files under crates/*/src"
    );

    let mut violations = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for (i, line) in text.lines().enumerate() {
            if !line.trim_start().starts_with("//") {
                continue;
            }
            if let Some(needle) = flag(line) {
                violations.push(format!(
                    "{}:{}: {:?} (matched {needle:?})",
                    path.display(),
                    i + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "parity-era provenance markers found in crates/*/src comments (the port is a total \
         fork; put this history in the commit message, not the source):\n{}",
        violations.join("\n")
    );
}
