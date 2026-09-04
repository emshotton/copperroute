use std::path::PathBuf;

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
            || head.starts_with("fixed: T")
            || head.starts_with("removed before T");
        assert!(
            ok,
            "row #{id}'s status is {status:?}, which is none of pinned / fixed: T<n> / totalized \
             / candidate / keep"
        );
    }
}
