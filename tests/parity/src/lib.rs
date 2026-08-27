//! Parity-test helpers: locate the Java clone, fixtures and reference outputs,
//! and compare text outputs modulo whitespace.

use similar::{ChangeTag, TextDiff};
use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

pub fn java_dir() -> PathBuf {
    std::env::var_os("FREEROUTING_JAVA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("..").join("freerouting"))
}

pub fn fixture(name: &str) -> PathBuf {
    java_dir().join("fixtures").join(name)
}

pub fn example(name: &str) -> PathBuf {
    java_dir().join("examples").join(name)
}

pub fn reference(stem: &str, file: &str) -> PathBuf {
    workspace_root()
        .join("tests")
        .join("reference")
        .join(stem)
        .join(file)
}

pub fn normalize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0usize;
    for line in s.replace("\r\n", "\n").split('\n') {
        let t = line.trim_end();
        if t.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(t);
        out.push('\n');
    }
    // trim trailing blank lines to exactly one '\n'
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Returns false and prints a skip message when a reference file is absent.
pub fn require_reference(path: &Path) -> bool {
    if path.exists() {
        true
    } else {
        eprintln!(
            "SKIP: reference {} missing — run scripts/gen-reference.sh",
            path.display()
        );
        false
    }
}

pub fn assert_text_parity(actual: &str, reference_path: &Path) {
    let expected = std::fs::read_to_string(reference_path)
        .unwrap_or_else(|e| panic!("cannot read reference {}: {e}", reference_path.display()));
    let a = normalize_whitespace(actual);
    let e = normalize_whitespace(&expected);
    if a == e {
        return;
    }
    // Save actual for inspection.
    if let Ok(rel) = reference_path.strip_prefix(workspace_root().join("tests").join("reference")) {
        let scratch = workspace_root()
            .join("tests")
            .join("reference")
            .join("_scratch")
            .join(rel);
        let _ = std::fs::create_dir_all(scratch.parent().unwrap());
        let _ = std::fs::write(&scratch, &a);
    }
    let diff = TextDiff::from_lines(&e, &a);
    let mut msg = format!("parity mismatch vs {}\n", reference_path.display());
    let mut shown = 0;
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => continue,
        };
        msg.push_str(sign);
        msg.push_str(change.value());
        shown += 1;
        if shown > 200 {
            msg.push_str("… (diff truncated)\n");
            break;
        }
    }
    panic!("{msg}");
}
