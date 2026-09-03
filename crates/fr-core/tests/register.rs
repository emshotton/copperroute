//! The quirk register's two gates (Plan 9 Task 0, Global Constraint
//! "Quirk-register status-column discipline", recommendation 13).
//!
//! `docs/java-quirks.md` is the port's description of the **Java** program: every deliberate
//! reproduction of a Java defect has a row there and a `// Java bug:` marker at the site. Plan 9
//! fixes 121 of those defects, and the two directions of that relationship each get a test here:
//!
//! * [`every_java_bug_marker_has_a_register_row`] — code → register. A marker that names an id
//!   the register does not carry is a marker pointing at nothing.
//! * [`a_fixed_row_has_a_fixed_marker`] — register → code. A row whose `status` says
//!   `fixed: T<n>` must have a `// fixed: T<n> (#id)` marker **naming that row**, and one per
//!   site wherever the site inventory is knowable, or the register is claiming a fix the code
//!   does not carry. **Vacuously true at Task 0; it is the gate every later task arms**, and it
//!   is what makes "edit the status cell in the same commit as the fix" checkable rather than a
//!   convention nobody enforces.
//!
//!   **Why the id is in the marker.** Keyed on the task alone, one `// fixed: T9` anywhere would
//!   satisfy every row Task 9 closes — a task fixing ten rows would need one comment, and the
//!   assertion message would be claiming a check the code did not make. The id makes the marker
//!   point at its own row. The site count then comes free wherever the neighbouring
//!   `// Java bug:` markers carry the id too (26 lines name 24 ids today): if `n` of them name
//!   `#id`, at least `n` `// fixed:` markers must name it back.
//! * [`the_register_is_contiguous`] — the register's own arithmetic note, executed. The note at
//!   the bottom of `docs/java-quirks.md` says the ids are contiguous from 1 and names the next
//!   free id; a plan that allocates ids by reading a prose sentence needs that sentence checked.
//!
//! **Where the census is taken, and why it is `crates/` and not `crates/*/src`.** Survey §9.1's
//! figure of record is "165 markers across 8 crates", and that is exactly
//! `grep -rn '// Java bug:' crates` at the Plan 8 handoff — **every** file, not only `.rs`. It
//! decomposes as **153 code markers** in `.rs` files plus **12 prose mentions** in the seven
//! crate `README.md`s, which describe the convention rather than mark a site. The same grep
//! restricted to `crates/*/src` answers 148, because five markers sit in `tests/` (a reproduced
//! defect whose only site is the assertion that pins it). This file counts what §9.1 counted, so
//! [`MARKER_BASELINE`] is comparable to the number the survey, the plan and Task 25's completion
//! gate all quote, and [`CODE_MARKER_BASELINE`] is the half a fix could actually delete. **This
//! file itself is excluded from both counts**: it discusses the marker in prose, and a test that
//! counted its own documentation would drift with its own comments (the "marker gate is two
//! greps, not one" note in `docs/java-quirks.md`, applied to itself). See the Task 0 report.
//!
//! These are `fr-core` tests only because `fr-core` is the crate that sits above the whole port
//! and already depends on `parity` for its workspace-root helper; nothing in this file touches
//! `fr_core` itself.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Survey §9.1's count of record: 165 `// Java bug:` markers across the eight crates. The
/// constraint is `>=`, never `==`: a later plan may add a marker, and **no plan may delete one**
/// (survey §9.1 — the marker is the provenance that makes the register navigable from the code,
/// and it is what a future reader compares the jar against).
const MARKER_BASELINE: usize = 165;

/// The `.rs` half of [`MARKER_BASELINE`]: 153 markers at actual code sites. This is the number a
/// fix could delete by accident; the other 12 are README prose about the convention.
const CODE_MARKER_BASELINE: usize = 153;

/// The register's row count: 292 rows from Plans 1-8, plus #293-#296 (Plan 9 Task 0, ruling
/// BL6 — R1/R2/I1/I2 out of `benchmark/reports/java-regressions-2026-09.md`), plus #297 (Plan 9
/// Task 17's review, ruling BY — the `reduceTraceShapesAtTiePins` no-op investigation owned by
/// Task 10).
const REGISTER_ROWS: usize = 297;

fn workspace_root() -> PathBuf {
    parity::workspace_root()
}

fn register_text() -> String {
    let path = workspace_root().join("docs/java-quirks.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// One row of the numbered table: its id and the text of its **last** cell, which is the `status`
/// column Task 0 added.
///
/// The parse is deliberately shallow — a line that starts with `| <digits> |` is a row of the
/// numbered table and nothing else in the document has that shape (the totalization table's rows
/// start with a Java expression, the candidate table's with prose, and the KEEP table's ids are
/// bolded as `| **#62** |`). Cells are not split on `|`, because several rows carry a literal
/// `|` inside a code span; only the trailing cell is needed, and `rsplit` finds it whatever the
/// arity of the row.
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

/// Every file under `crates/`, sorted, so a failure names the same file on every machine. This
/// file itself is never in the list — see the module doc.
fn all_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(&workspace_root().join("crates"), &mut out);
    let this = workspace_root().join(file!());
    out.retain(|p| *p != this);
    out.sort();
    out
}

/// [`all_sources`], restricted to Rust.
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
            // `target/` is a build directory a `cargo test` in a crate subdirectory can leave
            // behind; it holds no port source and its vendored copies would double every count.
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Every `// Java bug:` line under `crates/`, as `(path, line number, text)` — `grep -rn`'s
/// answer, every file type included.
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

/// The register ids a marker line names, as `#nnn`. Most markers carry none — they name the Java
/// method and the defect in prose, and the register row is found by that description rather than
/// by a number — so this returns an empty set for the majority and the test asserts only over
/// what is actually claimed.
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
    // Every row carries a status, and it is one of the five the rules header names. A row whose
    // status is `pinned (keep in part — …)` reads as `pinned` here: the leading word is what a
    // gate acts on and the parenthesis is what a reader acts on.
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
        // Task 0's state. Not an early return that hides a broken parse: the contiguity test
        // above proves the rows parse and carry statuses, so an empty set here means the plan has
        // not fixed anything yet, which is true at Task 0 and false from Task 1 on.
        return;
    }

    // Every `// fixed: T<n> (#id)` marker in the tree, counted **per (task, id)**. Counting
    // rather than collecting a set is what makes the "at every site" clause below possible: a set
    // would let one comment satisfy ten rows, which is what this test used to do and what its own
    // message already denied.
    let mut fixed_markers: BTreeMap<(String, u32), usize> = BTreeMap::new();
    // And every `// Java bug:` marker that names an id, counted the same way — the site inventory
    // the "at every site" clause is measured against.
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
        // `fixed: T9` / `fixed: T9 — one clause`: the task token is the first word after the colon.
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
        // **At every site**, wherever the site inventory is knowable. A `// Java bug:` marker that
        // names its id is a site this register row describes, so the fix must have left a
        // `// fixed:` marker beside each of them. Where the neighbouring markers carry no id — the
        // majority, which name the Java method in prose instead — the inventory is not machine
        // readable and the `>= 1` above is the whole check; that is a limit of the older markers,
        // not a softening of the rule, and it shrinks every time a task touches one.
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
