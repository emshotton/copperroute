# Native KiCad Board Input Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `copperroute route board.kicad_pcb -o out.ses` route a KiCad board directly, with no Specctra DSN involved.

**Architecture:** A new `copper-dsn::kicad::pcb` module parses `.kicad_pcb` s-expressions into the existing `KiCadBoardJson` DTO, which `kicad::reader` already turns into a `Board`. The module is a port of the import half of `web/kicad.js`, which stays in place as the reference implementation and the differential test oracle. A new `FileFormat::KicadPcb` routes the new extension through the existing load path.

**Tech Stack:** Rust 2024 edition, `serde`/`serde_json`, `cargo test`. Node.js for the differential test only, which skips when absent.

**Spec:** `docs/superpowers/specs/2026-09-09-native-kicad-board-input-design.md`

## Global Constraints

- Comments only ever indicate unexpected behaviour. No comment may duplicate what the code says, describe past or future states of the code, or refer to plans, tickets or design documents. This is the repository's rule in `CLAUDE.md` and it overrides habit.
- Error messages for unsupported constructs must reproduce the wording in `web/kicad.js` verbatim, so the command line and the web app refuse the same board for the same stated reason.
- The reference implementation for every geometric and structural decision is `web/kicad.js` plus `web/geometry.js`. Where this plan and that code disagree, the JS wins — it is in production.
- Coordinates in a `.kicad_pcb` are millimetres. `KiCadBoardJson` carries `resolution` and `unit`; match what `web/kicad.js` emits so the reader is fed identical input from both front ends.
- No routing behaviour changes in this plan. Solder mask expansion and the pad keepout are stage two.
- `cargo clippy --workspace -- -D warnings` has eight pre-existing `collapsible_if` failures in `copper-router` that arrived with upstream commits. Do not fix them here; do not add new ones.

---

### Task 1: S-expression reader

**Files:**
- Create: `crates/copper-dsn/src/kicad/sexpr.rs`
- Modify: `crates/copper-dsn/src/kicad/mod.rs`
- Test: `crates/copper-dsn/tests/kicad_sexpr.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `Node`, `Value`, `SexprError`, `parse(text: &str) -> Result<Node, SexprError>`, and the accessors `Node::name`, `Node::children`, `Node::child`, `Node::atom`, `Node::value`, `Node::number`, `Node::point`. Every later task in this crate uses these.

The KiCad dialect is not the DSN dialect, so this does not reuse `crates/copper-dsn/src/lexer`. It is the direct analogue of `parse` in `web/kicad.js:4-52`.

- [ ] **Step 1: Write the failing test**

```rust
// crates/copper-dsn/tests/kicad_sexpr.rs
use copper_dsn::kicad::sexpr::{parse, Value};

#[test]
fn it_reads_nested_nodes_and_atoms() {
    let node = parse("(kicad_pcb (version 20241229) (generator \"pcbnew\"))").expect("it parses");
    assert_eq!(node.name(), "kicad_pcb");
    assert_eq!(node.value("version"), Some("20241229"));
    assert_eq!(node.value("generator"), Some("pcbnew"));
}

#[test]
fn it_unescapes_quoted_strings() {
    let node = parse(r#"(net 1 "Net-(J1\\CC1)")"#).expect("it parses");
    assert_eq!(node.atom(2), Some(r"Net-(J1\CC1)"));
}

#[test]
fn it_collects_repeated_children() {
    let node = parse("(pcb (net 1 A) (net 2 B) (net 3 C))").expect("it parses");
    let names: Vec<&str> = node.children("net").filter_map(|n| n.atom(2)).collect();
    assert_eq!(names, ["A", "B", "C"]);
}

#[test]
fn it_reads_numbers_and_points() {
    let node = parse("(pad (at 1.5 -2.25) (size 0.8 0.95))").expect("it parses");
    assert_eq!(node.number("at"), Some(1.5));
    assert_eq!(node.point("size"), Some((0.8, 0.95)));
    assert_eq!(node.point("at"), Some((1.5, -2.25)));
}

#[test]
fn it_rejects_an_unclosed_expression() {
    let error = parse("(kicad_pcb (version 3)").expect_err("it fails");
    assert_eq!(error.to_string(), "Unclosed expression.");
}

#[test]
fn it_rejects_a_non_expression() {
    let error = parse("kicad_pcb").expect_err("it fails");
    assert_eq!(error.to_string(), "Expected a KiCad S-expression.");
}

#[test]
fn it_rejects_excessive_nesting() {
    let deep = format!("{}{}", "(a ".repeat(120), ")".repeat(120));
    let error = parse(&deep).expect_err("it fails");
    assert_eq!(error.to_string(), "File nesting is too deep.");
}

#[test]
fn it_keeps_source_spans() {
    let text = "(pcb (net 1 A))";
    let node = parse(text).expect("it parses");
    let net = node.child("net").expect("a net");
    assert_eq!(&text[net.start..net.end], "(net 1 A)");
    let _ = Value::Atom(String::new());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p copper-dsn --test kicad_sexpr`
Expected: FAIL to compile, `unresolved import copper_dsn::kicad::sexpr`.

- [ ] **Step 3: Write the implementation**

```rust
// crates/copper-dsn/src/kicad/sexpr.rs
use std::fmt;

const MAX_DEPTH: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Atom(String),
    Node(Node),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub values: Vec<Value>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SexprError(&'static str);

impl fmt::Display for SexprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for SexprError {}

impl Node {
    #[must_use]
    pub fn name(&self) -> &str {
        self.atom(0).unwrap_or_default()
    }

    #[must_use]
    pub fn atom(&self, index: usize) -> Option<&str> {
        match self.values.get(index) {
            Some(Value::Atom(text)) => Some(text.as_str()),
            _ => None,
        }
    }

    pub fn children<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a Node> {
        self.values.iter().filter_map(move |value| match value {
            Value::Node(node) if node.name() == key => Some(node),
            _ => None,
        })
    }

    #[must_use]
    pub fn child(&self, key: &str) -> Option<&Node> {
        self.children(key).next()
    }

    #[must_use]
    pub fn value(&self, key: &str) -> Option<&str> {
        self.child(key)?.atom(1)
    }

    #[must_use]
    pub fn number(&self, key: &str) -> Option<f64> {
        self.value(key)?.parse().ok()
    }

    #[must_use]
    pub fn point(&self, key: &str) -> Option<(f64, f64)> {
        let node = self.child(key)?;
        Some((node.atom(1)?.parse().ok()?, node.atom(2)?.parse().ok()?))
    }
}

pub fn parse(text: &str) -> Result<Node, SexprError> {
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    let node = read_node(text, bytes, &mut cursor, 0)?;
    skip_space(bytes, &mut cursor);
    if cursor != bytes.len() {
        return Err(SexprError("Expected one kicad_pcb board."));
    }
    Ok(node)
}

fn skip_space(bytes: &[u8], cursor: &mut usize) {
    while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
        *cursor += 1;
    }
}

fn read_node(
    text: &str,
    bytes: &[u8],
    cursor: &mut usize,
    depth: usize,
) -> Result<Node, SexprError> {
    if depth > MAX_DEPTH {
        return Err(SexprError("File nesting is too deep."));
    }
    skip_space(bytes, cursor);
    let start = *cursor;
    if bytes.get(start) != Some(&b'(') {
        return Err(SexprError("Expected a KiCad S-expression."));
    }
    *cursor += 1;
    let mut values = Vec::new();
    loop {
        skip_space(bytes, cursor);
        match bytes.get(*cursor) {
            None => return Err(SexprError("Unclosed expression.")),
            Some(b')') => {
                *cursor += 1;
                return Ok(Node {
                    values,
                    start,
                    end: *cursor,
                });
            }
            Some(b'(') => values.push(Value::Node(read_node(text, bytes, cursor, depth + 1)?)),
            Some(b'"') => values.push(Value::Atom(read_quoted(text, bytes, cursor))),
            Some(_) => values.push(Value::Atom(read_bare(text, bytes, cursor))),
        }
    }
}

fn read_quoted(text: &str, bytes: &[u8], cursor: &mut usize) -> String {
    *cursor += 1;
    let mut out = String::new();
    while *cursor < bytes.len() {
        let start = *cursor;
        match bytes[start] {
            b'"' => {
                *cursor += 1;
                break;
            }
            b'\\' => {
                *cursor += 1;
                if *cursor < bytes.len() {
                    let escaped = next_char(text, *cursor);
                    out.push_str(escaped);
                    *cursor += escaped.len();
                }
            }
            _ => {
                let ch = next_char(text, start);
                out.push_str(ch);
                *cursor += ch.len();
            }
        }
    }
    out
}

fn read_bare(text: &str, bytes: &[u8], cursor: &mut usize) -> String {
    let start = *cursor;
    while *cursor < bytes.len() {
        let byte = bytes[*cursor];
        if byte.is_ascii_whitespace() || byte == b'(' || byte == b')' {
            break;
        }
        *cursor += 1;
    }
    text[start..*cursor].to_string()
}

fn next_char(text: &str, index: usize) -> &str {
    let rest = &text[index..];
    let width = rest.chars().next().map_or(0, char::len_utf8);
    &rest[..width]
}
```

`web/kicad.js:52` raises `Expected one kicad_pcb board.` when trailing text follows the root, and checks the root name separately. This reader raises the same message for trailing text; the root name check belongs to Task 4, which knows it wants a board.

- [ ] **Step 4: Export the module**

In `crates/copper-dsn/src/kicad/mod.rs`, add `pub mod sexpr;` above `pub mod writer;`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p copper-dsn --test kicad_sexpr`
Expected: PASS, 7 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/copper-dsn/src/kicad/sexpr.rs crates/copper-dsn/src/kicad/mod.rs crates/copper-dsn/tests/kicad_sexpr.rs
git commit -m "Read KiCad s-expressions"
```

---

### Task 2: Split the reader so it accepts a parsed DTO

**Files:**
- Modify: `crates/copper-dsn/src/kicad/reader.rs:19-33`
- Modify: `crates/copper-dsn/src/kicad/mod.rs`
- Test: `crates/copper-dsn/tests/kicad_reader.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `read_board_json(board_json: KiCadBoardJson, id_generator: Option<ItemIdGenerator>) -> BoardReadResult`. Task 7 calls it with a DTO built in memory, avoiding a serialise/parse round trip. `read_board(json: &str, id_generator: Option<ItemIdGenerator>) -> BoardReadResult` keeps its signature for the wasm path.

- [ ] **Step 1: Write the failing test**

Append to `crates/copper-dsn/tests/kicad_reader.rs`:

```rust
#[test]
fn it_reads_a_board_from_a_parsed_dto() {
    use copper_dsn::kicad::{read_board_json, KiCadBoardJson};

    let json = minimal_board_json();
    let dto: KiCadBoardJson = serde_json::from_str(&json).expect("the DTO parses");
    let from_dto = read_board_json(dto, None);
    let from_text = copper_dsn::kicad::read_board(&json, None);

    let layers = |result: &copper_dsn::error::BoardReadResult| match result {
        copper_dsn::error::BoardReadResult::Success { board: Some(b), .. } => b.get_layer_count(),
        other => panic!("expected a loaded board, got {other:?}"),
    };
    assert_eq!(layers(&from_dto), layers(&from_text));
}
```

If `kicad_reader.rs` has no `minimal_board_json` helper, add one that returns the smallest board JSON already used by a passing test in that file. Reuse the existing fixture rather than inventing a new board.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p copper-dsn --test kicad_reader it_reads_a_board_from_a_parsed_dto`
Expected: FAIL to compile, `read_board_json` not found.

- [ ] **Step 3: Perform the split**

In `crates/copper-dsn/src/kicad/reader.rs`, change the head of `read_board` to delegate, and move its body into the new function. The existing body from the `board_json.validate()` call at line 34 onward becomes the body of `read_board_json` unchanged.

```rust
#[must_use]
pub fn read_board(json: &str, id_generator: Option<ItemIdGenerator>) -> BoardReadResult {
    let board_json: KiCadBoardJson = if json.trim().is_empty() {
        return parse_error("json_root", "JSON payload is empty or invalid");
    } else {
        match serde_json::from_str::<Option<KiCadBoardJson>>(json) {
            Ok(Some(board_json)) => board_json,
            Ok(None) => return parse_error("json_root", "JSON payload is empty or invalid"),
            Err(error) => {
                return parse_error("json_payload", &format!("Exception occurred: {error}"));
            }
        }
    };
    read_board_json(board_json, id_generator)
}

#[allow(clippy::too_many_lines)]
#[must_use]
pub fn read_board_json(
    board_json: KiCadBoardJson,
    id_generator: Option<ItemIdGenerator>,
) -> BoardReadResult {
    let id_generator = id_generator.unwrap_or_default();

    if let Err(malformed) = board_json.validate() {
        // ... the rest of the existing body, unchanged
```

Move the `#[allow(clippy::too_many_lines)]` attribute from `read_board` to `read_board_json`; the wrapper is short now.

- [ ] **Step 4: Export the new function**

In `crates/copper-dsn/src/kicad/mod.rs`, change the reader re-export to:

```rust
pub use reader::{import_session, read_board, read_board_json};
```

- [ ] **Step 5: Run the whole crate's tests**

Run: `cargo test -p copper-dsn`
Expected: PASS. This is a pure refactor; every existing KiCad reader test must still pass. If any fails, the body was moved incorrectly — revert and redo the move without editing the body.

- [ ] **Step 6: Commit**

```bash
git add crates/copper-dsn/src/kicad/reader.rs crates/copper-dsn/src/kicad/mod.rs crates/copper-dsn/tests/kicad_reader.rs
git commit -m "Let the KiCad reader take a parsed board DTO"
```

---

### Task 3: Board outline geometry

**Files:**
- Create: `crates/copper-dsn/src/kicad/pcb/outline.rs`
- Create: `crates/copper-dsn/src/kicad/pcb/mod.rs`
- Modify: `crates/copper-dsn/src/kicad/mod.rs`
- Test: `crates/copper-dsn/tests/kicad_pcb_outline.rs`

**Interfaces:**
- Consumes: `sexpr::{Node, parse}` from Task 1.
- Produces: `PcbError { section: String, message: String }`; `OUTLINE_TOLERANCE: f64`; `footprint_point(fp: &Node, x: f64, y: f64) -> (f64, f64)`; `arc_points(a, m, b) -> Result<Vec<(f64, f64)>, PcbError>`; `outline_paths(root: &Node) -> Result<OutlinePaths, PcbError>` where `OutlinePaths { paths: Vec<Vec<(f64, f64)>>, curved: bool }`; `assemble_outline(paths: &OutlinePaths) -> Result<Outline, PcbError>` where `Outline { boundary: Vec<(f64, f64)>, cutouts: Vec<Vec<(f64, f64)>> }`. Tasks 4, 5 and 6 use `PcbError` and `footprint_point`.

This is a direct port of `web/geometry.js` in full, plus the loop assembly in `web/kicad.js:210-239`.

- [ ] **Step 1: Write the failing test**

```rust
// crates/copper-dsn/tests/kicad_pcb_outline.rs
use copper_dsn::kicad::pcb::outline::{assemble_outline, outline_paths, OUTLINE_TOLERANCE};
use copper_dsn::kicad::sexpr::parse;

fn board(body: &str) -> String {
    format!("(kicad_pcb (version 20241229) {body})")
}

#[test]
fn it_assembles_a_rectangular_outline() {
    let text = board(
        r#"(gr_rect (start 0 0) (end 40 30) (layer "Edge.Cuts"))"#,
    );
    let root = parse(&text).expect("it parses");
    let paths = outline_paths(&root).expect("outline paths");
    assert!(!paths.curved);
    let outline = assemble_outline(&paths).expect("an outline");
    assert!(outline.cutouts.is_empty());
    let xs: Vec<f64> = outline.boundary.iter().map(|p| p.0).collect();
    assert_eq!(xs.iter().cloned().fold(f64::MIN, f64::max), 40.0);
}

#[test]
fn it_assembles_four_lines_into_one_loop() {
    let text = board(concat!(
        r#"(gr_line (start 0 0) (end 10 0) (layer "Edge.Cuts"))"#,
        r#"(gr_line (start 10 0) (end 10 10) (layer "Edge.Cuts"))"#,
        r#"(gr_line (start 10 10) (end 0 10) (layer "Edge.Cuts"))"#,
        r#"(gr_line (start 0 10) (end 0 0) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let outline = assemble_outline(&outline_paths(&root).expect("paths")).expect("an outline");
    assert_eq!(outline.boundary.len(), 4);
    assert!(outline.cutouts.is_empty());
}

#[test]
fn it_treats_the_smaller_loop_as_a_cutout() {
    let text = board(concat!(
        r#"(gr_rect (start 0 0) (end 40 30) (layer "Edge.Cuts"))"#,
        r#"(gr_rect (start 10 10) (end 20 20) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let outline = assemble_outline(&outline_paths(&root).expect("paths")).expect("an outline");
    assert_eq!(outline.cutouts.len(), 1);
}

#[test]
fn it_marks_an_arc_outline_curved() {
    let text = board(
        r#"(gr_arc (start 0 0) (mid 5 5) (end 10 0) (layer "Edge.Cuts"))"#,
    );
    let root = parse(&text).expect("it parses");
    let paths = outline_paths(&root).expect("paths");
    assert!(paths.curved);
    let sampled = &paths.paths[0];
    let radius = 5.0_f64;
    for point in sampled {
        let error = ((point.0 - 5.0).powi(2) + point.1.powi(2)).sqrt() - radius;
        assert!(error.abs() <= OUTLINE_TOLERANCE * 2.0, "{error} off the arc");
    }
}

#[test]
fn it_rejects_an_open_outline() {
    let text = board(
        r#"(gr_line (start 0 0) (end 10 0) (layer "Edge.Cuts"))"#,
    );
    let root = parse(&text).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths")).expect_err("it fails");
    assert_eq!(error.message, "The Edge.Cuts outline is not closed.");
}

#[test]
fn it_rejects_a_board_with_no_outline() {
    let root = parse(&board("")).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths")).expect_err("it fails");
    assert_eq!(error.message, "A closed Edge.Cuts outline is required.");
}

#[test]
fn it_rejects_separate_outlines() {
    let text = board(concat!(
        r#"(gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))"#,
        r#"(gr_rect (start 50 50) (end 60 60) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths")).expect_err("it fails");
    assert_eq!(error.message, "Separate board outlines are not supported yet.");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p copper-dsn --test kicad_pcb_outline`
Expected: FAIL to compile, `copper_dsn::kicad::pcb` not found.

- [ ] **Step 3: Create the module skeleton and the error type**

```rust
// crates/copper-dsn/src/kicad/pcb/mod.rs
pub mod outline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcbError {
    pub section: String,
    pub message: String,
}

impl PcbError {
    pub fn new(section: &str, message: &str) -> PcbError {
        PcbError {
            section: section.to_string(),
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for PcbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PcbError {}
```

In `crates/copper-dsn/src/kicad/mod.rs`, add `pub mod pcb;`.

- [ ] **Step 4: Port the geometry**

Write `crates/copper-dsn/src/kicad/pcb/outline.rs` as a line-by-line port of `web/geometry.js`, keeping the same function names in snake case:

| JS | Rust | Notes |
|---|---|---|
| `OUTLINE_TOLERANCE` | `pub const OUTLINE_TOLERANCE: f64 = 0.005;` | millimetres |
| `num` | private `num(text) -> Result<f64, PcbError>` | rejects non-finite and `abs > 100000` with `"Invalid outline coordinate."` |
| `xy` | private `xy(node) -> Result<(f64, f64), PcbError>` | missing node is `"Missing outline coordinate."` |
| `footprintPoint` | `pub fn footprint_point(fp: &Node, x: f64, y: f64) -> (f64, f64)` | PCB Y-down rotation, exactly as the JS |
| `sample` | private `sample(center, radius, start, sweep)` | `"Degenerate outline arc."`, `"Outline curve is too large to import."` at count > 4096 |
| `arcPoints` | `pub fn arc_points(a, m, b)` | |
| `nativeArcPoints` | private `native_arc_points(node)` | |
| `outlinePaths` | `pub fn outline_paths(root: &Node) -> Result<OutlinePaths, PcbError>` | strips a leading `fp_` to fold footprint edges into the `gr_` cases |

Then port the loop assembly from `web/kicad.js:210-239` into:

```rust
pub struct Outline {
    pub boundary: Vec<(f64, f64)>,
    pub cutouts: Vec<Vec<(f64, f64)>>,
}

pub fn assemble_outline(paths: &OutlinePaths) -> Result<Outline, PcbError>;
```

It builds the edge list, walks loops until closed, sorts loops by absolute shoelace area descending, takes the largest as the boundary, and requires every remaining loop to lie inside it by the even-odd test in `web/kicad.js:230-237`. Point equality uses the JS `equal` helper's tolerance; read it from `web/kicad.js` and match it exactly.

Section names for `PcbError::new` in this file are `"outline"`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p copper-dsn --test kicad_pcb_outline`
Expected: PASS, 7 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/copper-dsn/src/kicad/pcb crates/copper-dsn/src/kicad/mod.rs crates/copper-dsn/tests/kicad_pcb_outline.rs
git commit -m "Assemble KiCad board outlines from Edge.Cuts geometry"
```

---

### Task 4: Layers, nets and net classes

**Files:**
- Create: `crates/copper-dsn/src/kicad/pcb/structure.rs`
- Modify: `crates/copper-dsn/src/kicad/pcb/mod.rs`
- Test: `crates/copper-dsn/tests/kicad_pcb_structure.rs`

**Interfaces:**
- Consumes: `sexpr::Node`, `PcbError`.
- Produces: `Layers { entries: Vec<LayerJson> }` with `Layers::read(root) -> Result<Layers, PcbError>` and `Layers::index_of(name: &str) -> Result<i32, PcbError>`; `NetTable` with `NetTable::read(root) -> Result<NetTable, PcbError>`, `NetTable::name_of(node: &Node) -> Result<String, PcbError>` (the `netName` closure, handling the `version >= 20260101` named form and registering unseen names), and `NetTable::finish(net_classes) -> (Vec<NetJson>, Vec<NetClassJson>)`; `read_net_classes(root, defaults) -> Result<Vec<NetClassJson>, PcbError>`. Tasks 5, 6 and 7 use all of these.

Ported from `web/kicad.js:101-138` (layers and nets), `:139-152` (`netName`), and `:440-462` (net classes), plus `embeddedNetClasses` in the same file.

- [ ] **Step 1: Write the failing test**

```rust
// crates/copper-dsn/tests/kicad_pcb_structure.rs
use copper_dsn::kicad::pcb::structure::{read_net_classes, Layers, NetTable};
use copper_dsn::kicad::sexpr::parse;

const LAYERS: &str = r#"(layers (0 "F.Cu" signal) (2 "B.Cu" signal) (1 "F.Mask" user))"#;

fn board(body: &str) -> String {
    format!("(kicad_pcb (version 20241229) {LAYERS} {body})")
}

#[test]
fn it_keeps_only_copper_layers_in_order() {
    let root = parse(&board("")).expect("it parses");
    let layers = Layers::read(&root).expect("layers");
    assert_eq!(layers.entries.len(), 2);
    assert_eq!(layers.entries[0].name.as_deref(), Some("F.Cu"));
    assert_eq!(layers.index_of("B.Cu").expect("an index"), 1);
}

#[test]
fn it_maps_a_power_layer_to_a_plane() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal) (2 "B.Cu" power)))"#;
    let root = parse(text).expect("it parses");
    let layers = Layers::read(&root).expect("layers");
    assert_eq!(layers.entries[1].r#type.as_deref(), Some("plane"));
}

#[test]
fn it_rejects_an_unknown_copper_layer_type() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" jumper)))"#;
    let root = parse(text).expect("it parses");
    let error = Layers::read(&root).expect_err("it fails");
    assert_eq!(error.message, "Unsupported copper layer type: jumper");
}

#[test]
fn it_resolves_a_track_net_by_number() {
    let root = parse(&board(r#"(net 1 "GND") (segment (net 1))"#)).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    assert_eq!(nets.name_of(segment).expect("a name"), "GND");
}

#[test]
fn it_rejects_an_unknown_net_number() {
    let root = parse(&board(r#"(net 1 "GND") (segment (net 7))"#)).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    assert_eq!(nets.name_of(segment).expect_err("it fails").message, "Unknown net 7");
}

#[test]
fn it_resolves_a_track_net_by_name_on_new_boards() {
    let text = format!(
        r#"(kicad_pcb (version 20260101) {LAYERS} (net 1 "GND") (segment (net "VCC")))"#
    );
    let root = parse(&text).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    assert_eq!(nets.name_of(segment).expect("a name"), "VCC");
}

#[test]
fn it_synthesises_a_default_class_when_the_board_has_none() {
    let root = parse(&board("")).expect("it parses");
    let defaults = copper_dsn::kicad::NetClassJson {
        name: Some("Default".to_string()),
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        ..Default::default()
    };
    let classes = read_net_classes(&root, &defaults).expect("classes");
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name.as_deref(), Some("Default"));
    assert!((classes[0].clearance - 0.2).abs() < f64::EPSILON);
}

#[test]
fn it_rejects_a_net_in_two_classes() {
    let body = concat!(
        r#"(net_class "A" "" (clearance 0.2) (trace_width 0.25) (via_dia 0.6) (via_drill 0.3) (add_net "GND"))"#,
        r#"(net_class "B" "" (clearance 0.3) (trace_width 0.25) (via_dia 0.6) (via_drill 0.3) (add_net "GND"))"#,
    );
    let root = parse(&board(body)).expect("it parses");
    let defaults = copper_dsn::kicad::NetClassJson::default();
    let error = read_net_classes(&root, &defaults).expect_err("it fails");
    assert_eq!(error.message, "Net GND belongs to multiple embedded net classes.");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p copper-dsn --test kicad_pcb_structure`
Expected: FAIL to compile, `pcb::structure` not found.

- [ ] **Step 3: Write the implementation**

Create `crates/copper-dsn/src/kicad/pcb/structure.rs` and add `pub mod structure;` to `pcb/mod.rs`.

`Layers::read` filters `(layers ...)` entries whose name ends in `.Cu`, assigns sequential indices from zero, maps `power` to `"plane"` and `signal`/`mixed` to `"signal"`, rejects any other type with `Unsupported copper layer type: {kind}`, and rejects a count outside 1 to 32 with `Expected 1–32 copper layers.` — note the en dash, matching `web/kicad.js:109`.

`NetTable::read` collects `(net id name)` entries. `NetTable::name_of` is the `netName` closure: when the board `version` is at least 20260101 it reads `(net "NAME")` and registers a name it has not seen, otherwise it reads `(net ID)`, returns an empty string for id zero, and errors `Unknown net {id}` for an id absent from the table. `name_of` needs interior mutability to register names; use `RefCell<Vec<NetJson>>` inside `NetTable` and expose `finish` to take the accumulated nets out.

`read_net_classes` ports `embeddedNetClasses` plus `web/kicad.js:440-462`: validate that names are nonempty and unique (`Embedded net class names must be nonempty and unique.`), that `clearance >= 0`, `traceWidth > 0`, `viaDrill > 0` and `viaDiameter > viaDrill` (`Invalid embedded routing rules for {name}.`), reject a net claimed by two classes (`Net {name} belongs to multiple embedded net classes.`), prepend a synthesised `Default` from `defaults` when absent, and otherwise sort `Default` first.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p copper-dsn --test kicad_pcb_structure`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/copper-dsn/src/kicad/pcb crates/copper-dsn/tests/kicad_pcb_structure.rs
git commit -m "Read KiCad copper layers, nets and embedded net classes"
```

---

### Task 5: Footprints and pads

**Files:**
- Create: `crates/copper-dsn/src/kicad/pcb/footprints.rs`
- Modify: `crates/copper-dsn/src/kicad/pcb/mod.rs`
- Test: `crates/copper-dsn/tests/kicad_pcb_footprints.rs`

**Interfaces:**
- Consumes: `sexpr::Node`, `PcbError`, `outline::footprint_point`, `structure::{Layers, NetTable}`.
- Produces: `read_components(root: &Node, layers: &Layers, nets: &NetTable, warnings: &mut Vec<String>) -> Result<(Vec<ComponentJson>, Vec<ConductionAreaJson>), PcbError>`. Task 7 calls it. The conduction areas returned are the footprint copper rectangles reserved as obstacles.

Ported from `web/kicad.js:240-391`. This is the densest part of the adapter; keep it in its own file.

- [ ] **Step 1: Write the failing test**

```rust
// crates/copper-dsn/tests/kicad_pcb_footprints.rs
use copper_dsn::kicad::pcb::footprints::read_components;
use copper_dsn::kicad::pcb::structure::{Layers, NetTable};
use copper_dsn::kicad::sexpr::parse;

const LAYERS: &str = r#"(layers (0 "F.Cu" signal) (2 "B.Cu" signal))"#;

fn read(body: &str) -> (Vec<copper_dsn::kicad::ComponentJson>, Vec<String>) {
    let text = format!("(kicad_pcb (version 20241229) {LAYERS} (net 1 \"GND\") {body})");
    let root = parse(&text).expect("it parses");
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    let (components, _) = read_components(&root, &layers, &nets, &mut warnings).expect("components");
    (components, warnings)
}

fn read_err(body: &str) -> String {
    let text = format!("(kicad_pcb (version 20241229) {LAYERS} (net 1 \"GND\") {body})");
    let root = parse(&text).expect("it parses");
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    read_components(&root, &layers, &nets, &mut warnings)
        .expect_err("it fails")
        .message
}

#[test]
fn it_gives_each_pad_its_own_component_at_absolute_position() {
    let (components, _) = read(concat!(
        r#"(footprint "R" (layer "F.Cu") (at 10 20)"#,
        r#" (property "Reference" "R1")"#,
        r#" (pad "1" smd rect (at 1 0) (size 0.8 0.9) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(components.len(), 1);
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert_eq!(pad.netName.as_deref(), Some("GND"));
    let position = components[0].position.as_ref().expect("a position");
    assert!((position.x - 11.0).abs() < 1e-9, "got {}", position.x);
    assert!((position.y - 20.0).abs() < 1e-9, "got {}", position.y);
}

#[test]
fn it_rotates_a_pad_offset_about_the_footprint_origin() {
    let (components, _) = read(concat!(
        r#"(footprint "R" (layer "F.Cu") (at 10 20 90)"#,
        r#" (pad "1" smd rect (at 1 0) (size 0.8 0.9) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    let position = components[0].position.as_ref().expect("a position");
    assert!((position.x - 10.0).abs() < 1e-9, "got {}", position.x);
    assert!((position.y - 19.0).abs() < 1e-9, "got {}", position.y);
}

#[test]
fn it_expands_a_wildcard_layer_span() {
    let (components, _) = read(concat!(
        r#"(footprint "J" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" thru_hole circle (at 0 0) (size 1 1) (drill 0.5) (layers "*.Cu") (net 1 "GND")))"#,
    ));
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert_eq!(pad.layers.as_ref().expect("layers").len(), 2);
}

#[test]
fn it_skips_a_paste_only_aperture() {
    let (components, _) = read(concat!(
        r#"(footprint "A" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Paste")))"#,
    ));
    assert!(components.is_empty());
}

#[test]
fn it_warns_that_rounded_pads_route_as_rectangles() {
    let (_, warnings) = read(concat!(
        r#"(footprint "R" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd roundrect (at 0 0) (size 1 1) (roundrect_rratio 0.25)"#,
        r#" (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert!(warnings.iter().any(|w| w.contains("corner radius for hole DRC")));
}

#[test]
fn it_rejects_net_ties() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0) (net_tie_pad_groups "1,2")"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Net ties are not supported yet.");
}

#[test]
fn it_rejects_a_footprint_zone() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0) (zone (net 1))"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Footprint zones are not supported yet.");
}

#[test]
fn it_rejects_an_unsupported_pad_shape() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd trapezoid (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Unsupported pad U.1: smd/trapezoid");
}

#[test]
fn it_rejects_a_nonpositive_pad_size() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd rect (at 0 0) (size 0 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Pad sizes must be positive.");
}

#[test]
fn it_rejects_an_unequal_circular_pad() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd circle (at 0 0) (size 1 2) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Circular pads must have equal dimensions.");
}
```

The `Unsupported pad U.1` message interpolates the footprint reference, which falls back through `(property "Reference" ...)`, then `(fp_text reference ...)`, then `FP{index}`. In these fixtures no reference property is present, so the reference is the footprint's own name only if the JS does that — read `web/kicad.js:271-277` and match it exactly; adjust the two expected strings above to whatever the JS actually produces for a footprint with no reference property. Do not change the JS.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p copper-dsn --test kicad_pcb_footprints`
Expected: FAIL to compile, `pcb::footprints` not found.

- [ ] **Step 3: Write the implementation**

Create `crates/copper-dsn/src/kicad/pcb/footprints.rs`, add `pub mod footprints;` to `pcb/mod.rs`, and port `web/kicad.js:240-391` in order:

1. Reject `net_tie_pad_groups` and footprint `zone` children.
2. Walk footprint children on a `.Cu` layer: `fp_rect` becomes an obstacle `ConductionAreaJson` with `isObstacle: true`, `netName: ""` and its stroke half-width added to each side; anything other than `pad` or `layer` on copper raises `Footprint copper graphics are not supported yet.`
3. Resolve the reference as described above.
4. For each `(pad ...)`: validate type against `smd`, `connect`, `thru_hole`, `np_thru_hole` and shape against `circle`, `rect`, `oval`, `roundrect`, `custom`.
5. Custom pads: exactly one filled `gr_poly` primitive, convex, with a circular anchor and equal size, covering its anchor; sample the stroke radius into a convex hull. Port `web/kicad.js:290-314` including the `count` formula and the monotone chain. Push the custom pad warning.
6. Drills: `(drill oval w h)` is a slot taking the smaller dimension and setting `drillEstimated: true`; otherwise `(drill d)`. Slots must be `thru_hole` with positive dimensions.
7. Position: rotate the pad's local `(at x y)` by the footprint rotation about the footprint origin, PCB Y-down, exactly as `web/kicad.js:337-347`.
8. Layers: expand `*.Cu` and `F&B.Cu` to every copper layer, keep `.Cu` names, drop the rest; a pad left with no copper layers is skipped as a paste-only aperture.
9. Emit one `ComponentJson` per pad with `reference` formatted `{reference}:{footprint_index}:{pad_index}`, `layer: "F.Cu"`, `rotation: -angle`, and a single `PadJson` carrying `name` as the pad index, `shape`, `roundRectRatio` for roundrect, `size`, `copperPolygon` for custom, `shapeOffset` from a drill offset, `offset` of zero, `position`, `drill`, `nonPlated`, `drillEstimated` and `layers`.

Both `(footprint ...)` and legacy `(module ...)` nodes are read, footprints first, matching `web/kicad.js:241-244`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p copper-dsn --test kicad_pcb_footprints`
Expected: PASS, 10 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/copper-dsn/src/kicad/pcb crates/copper-dsn/tests/kicad_pcb_footprints.rs
git commit -m "Read KiCad footprints and pads"
```

---

### Task 6: Tracks, vias, zones and copper obstacles

**Files:**
- Create: `crates/copper-dsn/src/kicad/pcb/routing.rs`
- Modify: `crates/copper-dsn/src/kicad/pcb/mod.rs`
- Test: `crates/copper-dsn/tests/kicad_pcb_routing.rs`

**Interfaces:**
- Consumes: `sexpr::Node`, `PcbError`, `structure::{Layers, NetTable}`.
- Produces: `read_traces(root, layers, nets) -> Result<Vec<TraceJson>, PcbError>`; `read_vias(root, layers, nets) -> Result<Vec<ViaJson>, PcbError>`; `check_zones(root, nets, warnings) -> Result<(), PcbError>`; `read_copper_text(root, layers, warnings) -> Result<Vec<ConductionAreaJson>, PcbError>`. Task 7 calls all four.

Ported from `web/kicad.js:96-100` and `:139-198` (zones and copper text) and `:392-438` (tracks and vias). Rip-up options are not ported: the command line always routes the board as given, so `rippedUp` is always false and `options` has no analogue.

- [ ] **Step 1: Write the failing test**

```rust
// crates/copper-dsn/tests/kicad_pcb_routing.rs
use copper_dsn::kicad::pcb::routing::{check_zones, read_copper_text, read_traces, read_vias};
use copper_dsn::kicad::pcb::structure::{Layers, NetTable};
use copper_dsn::kicad::sexpr::parse;

const LAYERS: &str = r#"(layers (0 "F.Cu" signal) (2 "B.Cu" signal))"#;

fn root_of(body: &str) -> copper_dsn::kicad::sexpr::Node {
    let text = format!("(kicad_pcb (version 20241229) {LAYERS} (net 1 \"GND\") {body})");
    parse(&text).expect("it parses")
}

#[test]
fn it_reads_a_track() {
    let root = root_of(r#"(segment (start 1 2) (end 3 4) (width 0.25) (layer "F.Cu") (net 1))"#);
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let traces = read_traces(&root, &layers, &nets).expect("traces");
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].netName.as_deref(), Some("GND"));
    assert_eq!(traces[0].layerIndex, 0);
}

#[test]
fn it_rejects_a_locked_track() {
    let root = root_of(r#"(segment (start 1 2) (end 3 4) (width 0.25) (layer "F.Cu") (net 1) (locked yes))"#);
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let error = read_traces(&root, &layers, &nets).expect_err("it fails");
    assert_eq!(error.message, "Locked tracks are not supported yet.");
}

#[test]
fn it_reads_a_through_via() {
    let root = root_of(r#"(via (at 5 6) (size 0.6) (drill 0.3) (layers "F.Cu" "B.Cu") (net 1))"#);
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let vias = read_vias(&root, &layers, &nets).expect("vias");
    assert_eq!(vias.len(), 1);
    assert_eq!(vias[0].startLayerIndex, 0);
    assert_eq!(vias[0].endLayerIndex, 1);
}

#[test]
fn it_rejects_a_blind_via() {
    let root = root_of(r#"(via blind (at 5 6) (size 0.6) (drill 0.3) (layers "F.Cu" "B.Cu") (net 1))"#);
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let error = read_vias(&root, &layers, &nets).expect_err("it fails");
    assert_eq!(error.message, "Locked, blind, and micro vias are not supported yet.");
}

#[test]
fn it_warns_about_copper_zones_without_routing_against_them() {
    let root = root_of(r#"(zone (net 1) (layer "F.Cu"))"#);
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    check_zones(&root, &nets, &mut warnings).expect("zones are accepted");
    assert!(warnings.iter().any(|w| w.contains("fill cache removed")));
}

#[test]
fn it_rejects_a_netless_copper_zone() {
    let root = root_of(r#"(zone (layer "F.Cu"))"#);
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    let error = check_zones(&root, &nets, &mut warnings).expect_err("it fails");
    assert_eq!(error.message, "Netless copper zones are not supported for routing yet.");
}

#[test]
fn it_rejects_a_keepout_that_restricts_tracks() {
    let root = root_of(r#"(zone (layer "F.Cu") (keepout (tracks not_allowed) (vias allowed)))"#);
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    let error = check_zones(&root, &nets, &mut warnings).expect_err("it fails");
    assert_eq!(
        error.message,
        "Zone keepouts that restrict tracks or vias are not supported for routing yet."
    );
}

#[test]
fn it_reserves_copper_text_as_an_obstacle() {
    let root = root_of(
        r#"(gr_text "HI" (at 5 5 0) (layer "F.Cu") (effects (font (size 1 1) (thickness 0.15))))"#,
    );
    let layers = Layers::read(&root).expect("layers");
    let mut warnings = Vec::new();
    let areas = read_copper_text(&root, &layers, &mut warnings).expect("areas");
    assert_eq!(areas.len(), 1);
    assert!(areas[0].isObstacle);
    assert_eq!(areas[0].polygon.as_ref().expect("a polygon").len(), 4);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p copper-dsn --test kicad_pcb_routing`
Expected: FAIL to compile, `pcb::routing` not found.

- [ ] **Step 3: Write the implementation**

Create `crates/copper-dsn/src/kicad/pcb/routing.rs` and add `pub mod routing;` to `pcb/mod.rs`.

`read_traces` maps each `(segment ...)` to a `TraceJson` with a sequential `id`, its net name, width, layer index and its two points. A `locked` atom or child, or an empty net name, raises the messages above. Curved tracks are rejected in Task 7, where `(arc ...)` is checked.

`read_vias` maps each `(via ...)` to a `ViaJson`. Reject any via carrying `locked`, `blind` or `micro`; require a two-entry layer span running from the first to the last copper layer, and a nonempty net, with `Only through vias assigned to a net are supported.`

`check_zones` walks `(zone ...)`. A zone with a `keepout` child must allow both tracks and vias; count it as preserved. A zone without one must have a net; count it as copper. Push the two warnings from `web/kicad.js:160-168` when the respective counts are nonzero, with the same wording and the counts interpolated.

`read_copper_text` ports `web/kicad.js:176-192`: for each top-level `gr_text` on a `.Cu` layer, require an `(effects (font ...))` without a `(face ...)` — otherwise `Custom copper text fonts are not supported yet.` — then build the conservative rotated rectangle from the font size, line count, longest line and justification, and push the copper text warning. Match the width and height formulas exactly.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p copper-dsn --test kicad_pcb_routing`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/copper-dsn/src/kicad/pcb crates/copper-dsn/tests/kicad_pcb_routing.rs
git commit -m "Read KiCad tracks, vias, zones and copper obstacles"
```

---

### Task 7: Assemble the board DTO

**Files:**
- Modify: `crates/copper-dsn/src/kicad/pcb/mod.rs`
- Modify: `crates/copper-dsn/src/kicad/mod.rs`
- Test: `crates/copper-dsn/tests/kicad_pcb.rs`

**Interfaces:**
- Consumes: everything from Tasks 1 and 3 through 6.
- Produces: `ImportedPcb { board: KiCadBoardJson, warnings: Vec<String> }` and `read_pcb(text: &str, defaults: &NetClassJson) -> Result<ImportedPcb, PcbError>`. Tasks 8, 9 and 10 call `read_pcb`.

- [ ] **Step 1: Write the failing test**

```rust
// crates/copper-dsn/tests/kicad_pcb.rs
use copper_dsn::kicad::pcb::read_pcb;
use copper_dsn::kicad::{read_board_json, NetClassJson};
use copper_dsn::error::BoardReadResult;

fn defaults() -> NetClassJson {
    NetClassJson {
        name: Some("Default".to_string()),
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        ..Default::default()
    }
}

fn example() -> String {
    let path = testkit::workspace_root().join("web/example.kicad_pcb");
    std::fs::read_to_string(&path).expect("the example board reads")
}

#[test]
fn it_imports_the_example_board() {
    let imported = read_pcb(&example(), &defaults()).expect("the board imports");
    assert!(!imported.board.components.as_ref().expect("components").is_empty());
    assert!(imported.board.outline.is_some());
    assert!(!imported.board.nets.as_ref().expect("nets").is_empty());
}

#[test]
fn the_imported_board_loads_through_the_reader() {
    let imported = read_pcb(&example(), &defaults()).expect("the board imports");
    match read_board_json(imported.board, None) {
        BoardReadResult::Success { board: Some(board), .. } => {
            assert!(board.get_layer_count() >= 1);
        }
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

#[test]
fn it_rejects_a_file_that_is_not_a_board() {
    let error = read_pcb("(kicad_sch (version 1))", &defaults()).expect_err("it fails");
    assert_eq!(error.message, "Expected one kicad_pcb board.");
}

#[test]
fn it_rejects_a_board_with_no_pads() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts")))"#;
    let error = read_pcb(text, &defaults()).expect_err("it fails");
    assert_eq!(error.message, "No pads were found.");
}

#[test]
fn it_rejects_curved_tracks() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal)) (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))
        (arc (start 1 1) (mid 2 2) (end 3 3) (width 0.25) (layer "F.Cu") (net 1)))"#;
    let error = read_pcb(text, &defaults()).expect_err("it fails");
    assert_eq!(error.message, "Curved tracks are not supported yet.");
}
```

Add `testkit` to `crates/copper-dsn/Cargo.toml` under `[dev-dependencies]` if it is not already there; check first, since other tests in this crate may already use it.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p copper-dsn --test kicad_pcb`
Expected: FAIL to compile, `read_pcb` not found.

- [ ] **Step 3: Write the assembly**

In `crates/copper-dsn/src/kicad/pcb/mod.rs`:

```rust
pub struct ImportedPcb {
    pub board: KiCadBoardJson,
    pub warnings: Vec<String>,
}

pub fn read_pcb(text: &str, defaults: &NetClassJson) -> Result<ImportedPcb, PcbError> {
```

In order: parse the text, require the root name to be `kicad_pcb` with `Expected one kicad_pcb board.`, read layers, read nets, check zones, read the outline, reject any top-level copper object outside the allowed set from `web/kicad.js:194-204` with `Unsupported copper object: {name}`, read copper text obstacles, read components, require at least one component with `No pads were found.`, reject `(arc ...)` with `Curved tracks are not supported yet.`, read traces and vias, read net classes, assign each net its class, and push the curved-outline warning when the outline is curved.

Fill `KiCadBoardJson` to match what `web/kicad.js` returns: `resolution` and `unit` as the JS sets them, `layers`, `nets`, `netClasses`, `components`, `outline` from the boundary with cutouts, `traces`, `vias` and `conductionAreas` combining the copper text and footprint rectangle obstacles. Read the JS return block and mirror every field it sets; leave the rest at the DTO's defaults.

Export from `crates/copper-dsn/src/kicad/mod.rs`:

```rust
pub use pcb::{read_pcb, ImportedPcb, PcbError};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p copper-dsn --test kicad_pcb`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/copper-dsn/src/kicad crates/copper-dsn/tests/kicad_pcb.rs crates/copper-dsn/Cargo.toml
git commit -m "Assemble a KiCad board DTO from a .kicad_pcb file"
```

---

### Task 8: Differential test against the JavaScript adapter

**Files:**
- Create: `crates/copper-dsn/tests/kicad_pcb_parity.rs`
- Create: `crates/copper-dsn/tests/data/kicad_pcb_parity.mjs`

**Interfaces:**
- Consumes: `read_pcb` from Task 7.
- Produces: nothing consumed by later tasks.

This is the primary correctness argument for the port. It runs the JS adapter over each example board and compares its `KiCadBoardJson` to the Rust parser's.

- [ ] **Step 1: Write the Node harness**

```javascript
// crates/copper-dsn/tests/data/kicad_pcb_parity.mjs
// Emits the JS adapter's KiCadBoardJson for one board so the Rust port can be compared to it.
import { readFileSync } from "node:fs";
import { importBoard } from "../../../../web/kicad.js";

const rules = { clearance: 0.2, traceWidth: 0.25, viaDiameter: 0.6, viaDrill: 0.3 };
const text = readFileSync(process.argv[2], "utf8");
const { board } = importBoard(text, "parity", rules, { rebuildZones: true });
process.stdout.write(JSON.stringify(board));
```

Check what `importBoard` actually returns before writing this — `web/worker.js:45` and `:59` call it and destructure the result. Match that shape; if the board DTO is returned under a different key, use that key.

- [ ] **Step 2: Write the failing test**

```rust
// crates/copper-dsn/tests/kicad_pcb_parity.rs
use copper_dsn::kicad::pcb::read_pcb;
use copper_dsn::kicad::NetClassJson;
use std::path::{Path, PathBuf};
use std::process::Command;

const TOLERANCE: f64 = 0.005;

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn boards() -> Vec<PathBuf> {
    let root = testkit::workspace_root();
    let mut paths = vec![root.join("web/example.kicad_pcb")];
    let examples = root.join("web/examples");
    if let Ok(entries) = std::fs::read_dir(&examples) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if let Ok(inner) = std::fs::read_dir(&dir) {
                for file in inner.flatten() {
                    let path = file.path();
                    if path.extension().is_some_and(|e| e == "kicad_pcb") {
                        paths.push(path);
                    }
                }
            }
        }
    }
    paths.retain(|p| p.exists());
    paths
}

fn javascript_board(path: &Path) -> serde_json::Value {
    let harness = testkit::workspace_root()
        .join("crates/copper-dsn/tests/data/kicad_pcb_parity.mjs");
    let out = Command::new("node")
        .arg(&harness)
        .arg(path)
        .output()
        .expect("node runs");
    assert!(
        out.status.success(),
        "the JS adapter failed on {}: {}",
        path.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("the JS emits JSON")
}

fn compare(path: &str, want: &serde_json::Value, got: &serde_json::Value, failures: &mut Vec<String>) {
    use serde_json::Value;
    match (want, got) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap_or_default(), b.as_f64().unwrap_or_default());
            if (a - b).abs() > TOLERANCE {
                failures.push(format!("{path}: js {a}, rust {b}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                failures.push(format!("{path}: js has {} entries, rust {}", a.len(), b.len()));
                return;
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                compare(&format!("{path}[{i}]"), x, y, failures);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            for (key, x) in a {
                match b.get(key) {
                    Some(y) => compare(&format!("{path}.{key}"), x, y, failures),
                    None => failures.push(format!("{path}.{key}: missing from rust")),
                }
            }
        }
        _ if want != got => failures.push(format!("{path}: js {want}, rust {got}")),
        _ => {}
    }
}

#[test]
fn the_port_matches_the_javascript_adapter() {
    if !node_available() {
        eprintln!("skipping: node is not available");
        return;
    }
    let defaults = NetClassJson {
        name: Some("Default".to_string()),
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        ..Default::default()
    };
    let mut failures = Vec::new();
    for path in boards() {
        let text = std::fs::read_to_string(&path).expect("the board reads");
        let imported = read_pcb(&text, &defaults).expect("the board imports");
        let rust = serde_json::to_value(&imported.board).expect("the DTO serialises");
        let js = javascript_board(&path);
        let stem = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        compare(&stem, &js, &rust, &mut failures);
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
```

- [ ] **Step 3: Run the test**

Run: `cargo test -p copper-dsn --test kicad_pcb_parity -- --nocapture`
Expected: FAIL on the first run, listing concrete field differences.

- [ ] **Step 4: Fix the port until it agrees**

Work down the failure list, changing the Rust to match the JS. The JS is the reference; do not change `web/kicad.js` to make a test pass. If a difference is genuinely a floating-point artefact rather than a logic error, widen only that comparison and note why in the commit message, not in a code comment.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p copper-dsn --test kicad_pcb_parity`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/copper-dsn/tests/kicad_pcb_parity.rs crates/copper-dsn/tests/data/kicad_pcb_parity.mjs crates/copper-dsn/src/kicad
git commit -m "Check the KiCad board port against the JavaScript adapter"
```

---

### Task 9: A file format for KiCad boards

**Files:**
- Modify: `crates/copper-core/src/job.rs:104-115`, `:119-131`, `:133-147`, `:164-176`, `:222-236`
- Modify: `crates/copper-core/src/load.rs:171-193`, `:197-217`
- Test: `crates/copper-core/tests/` — add to the existing file-format test file if one exists, otherwise create `crates/copper-core/tests/kicad_pcb_format.rs`

**Interfaces:**
- Consumes: `read_pcb` from Task 7.
- Produces: `FileFormat::KicadPcb`. Task 10 accepts it in the command line's guard.

- [ ] **Step 1: Write the failing test**

```rust
// crates/copper-core/tests/kicad_pcb_format.rs
use copper_core::FileFormat;
use std::path::Path;

#[test]
fn it_recognises_the_extension() {
    assert_eq!(
        FileFormat::from_path(Path::new("board.kicad_pcb")),
        FileFormat::KicadPcb
    );
}

#[test]
fn it_sniffs_a_board_header() {
    assert_eq!(
        FileFormat::sniff_bytes(b"(kicad_pcb (version 20241229)"),
        FileFormat::KicadPcb
    );
}

#[test]
fn it_sniffs_a_board_header_after_newlines() {
    assert_eq!(
        FileFormat::sniff_bytes(b"\n\n(kicad_pcb (version 20241229)"),
        FileFormat::KicadPcb
    );
}

#[test]
fn it_still_sniffs_a_dsn() {
    assert_eq!(FileFormat::sniff_bytes(b"(pcb board.dsn"), FileFormat::Dsn);
}

#[test]
fn it_round_trips_its_name() {
    assert_eq!(
        FileFormat::from_name(FileFormat::KicadPcb.name()),
        Some(FileFormat::KicadPcb)
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p copper-core --test kicad_pcb_format`
Expected: FAIL to compile, no variant `KicadPcb`.

- [ ] **Step 3: Add the variant**

In `crates/copper-core/src/job.rs`, add `KicadPcb` to `FileFormat`, `"KICAD_PCB"` to `name()` and `from_name()`, and `"kicad_pcb" => FileFormat::KicadPcb` to the extension match at line 231. Leave `default_extension` alone; the format is never a default output.

In `sniff_bytes_inner`, add a fourth s-expression check beside the existing ones, matching `(kic` case-insensitively on the second, third and fourth bytes:

```rust
if buffer[0] == 0x28
    && (buffer[1] == 0x6B || buffer[1] == 0x4B)
    && (buffer[2] == 0x69 || buffer[2] == 0x49)
    && (buffer[3] == 0x63 || buffer[3] == 0x43)
{
    return (FileFormat::KicadPcb, hangs);
}
```

Place it before the `Dsn` check is irrelevant — the prefixes are disjoint — but keep it adjacent to the other three so the group reads together.

- [ ] **Step 4: Accept the format when loading**

In `crates/copper-core/src/load.rs`, both `load_board_if_needed` (line 177) and `parse_board_if_needed` (line 204) guard on the format. Widen each guard to admit `FileFormat::KicadPcb` and update the message to name it. In `parse_board_if_needed`, add the third arm:

```rust
let parsed = if format == FileFormat::KicadPcb {
    let text = String::from_utf8_lossy(&data);
    match copper_dsn::kicad::read_pcb(&text, &default_net_class()) {
        Ok(imported) => {
            let mut result = parse_board_result(copper_dsn::kicad::read_board_json(
                imported.board,
                None,
            ))?;
            result.warnings.extend(imported.warnings);
            Ok(result)
        }
        Err(error) => Err(Error::Load(format!("Failed to load board: {error}"))),
    }
} else if format == FileFormat::KicadDesignJson {
```

Do the same for `load_board_if_needed`, mirroring how it calls `load_from_kicad_json`.

`default_net_class()` is a new private helper in `load.rs` returning the `NetClassJson` the web app passes as `rules`: read the defaults `web/app.js` or `web/worker.js` supplies and use the same values, so a board with no embedded `Default` class routes identically from both front ends.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p copper-core`
Expected: PASS, including the five new tests and every existing one.

- [ ] **Step 6: Commit**

```bash
git add crates/copper-core/src/job.rs crates/copper-core/src/load.rs crates/copper-core/tests/kicad_pcb_format.rs
git commit -m "Recognise .kicad_pcb as a board input format"
```

---

### Task 10: Route a KiCad board from the command line

**Files:**
- Modify: `crates/copperroute/src/ops/load.rs:71-77`
- Modify: `crates/copperroute/src/cli.rs:41-42`
- Test: `crates/copperroute/tests/kicad_pcb_input.rs`

**Interfaces:**
- Consumes: `FileFormat::KicadPcb` from Task 9.
- Produces: the finished command line behaviour. Nothing consumes it.

- [ ] **Step 1: Write the failing test**

```rust
// crates/copperroute/tests/kicad_pcb_input.rs
use std::process::Command;

fn binary() -> std::path::PathBuf {
    let mut path = std::env::current_exe().expect("a test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("copperroute")
}

#[test]
fn it_reports_a_board_summary_for_a_kicad_board() {
    let board = testkit::workspace_root().join("web/example.kicad_pcb");
    let out = Command::new(binary())
        .arg("info")
        .arg(&board)
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "info failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let summary: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("info emits JSON");
    assert!(summary.get("layers").is_some(), "got {summary}");
}

#[test]
fn it_routes_a_kicad_board_to_a_session() {
    let board = testkit::workspace_root().join("web/example.kicad_pcb");
    let dir = tempfile::tempdir().expect("a temp dir");
    let output = dir.path().join("out.ses");
    let out = Command::new(binary())
        .arg("route")
        .arg(&board)
        .arg("-o")
        .arg(&output)
        .arg("--max-passes")
        .arg("1")
        .arg("--timeout")
        .arg("120")
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "route failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let session = std::fs::read_to_string(&output).expect("a session was written");
    assert!(session.starts_with("(session"), "got {}", &session[..40.min(session.len())]);
}
```

Check `crates/copperroute/tests/` for an existing helper that locates the binary and builds it; reuse it rather than the `binary()` above if one exists. Add `tempfile` to `[dev-dependencies]` only if the crate does not already have it.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p copperroute --test kicad_pcb_input`
Expected: FAIL — `info` and `route` reject the input, reporting that only Specctra DSN and KiCad board JSON are accepted.

- [ ] **Step 3: Widen the command line guard**

In `crates/copperroute/src/ops/load.rs:72`:

```rust
if !matches!(
    input.format,
    FileFormat::Dsn | FileFormat::KicadDesignJson | FileFormat::KicadPcb
) {
    return Err(OpError::Input(format!(
        "'{}' is not a board: only Specctra DSN, KiCad board JSON and KiCad boards are accepted, got {}",
        input.get_filename(),
        input.format.name()
    )));
}
```

In `crates/copperroute/src/cli.rs:41`, update the `input` doc comment on `RouteArgs` to `/// A Specctra DSN, a KiCad board JSON, or a KiCad .kicad_pcb board.` Do the same for `DrcArgs` and `InfoArgs` at lines 87 and 105.

`read_scheduler_rules` at `ops/load.rs:150` looks for a `.rules` file beside a DSN input only. Leave it as is: a KiCad board carries its own net classes, so there is nothing to discover beside it.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p copperroute --test kicad_pcb_input`
Expected: PASS, 2 tests.

- [ ] **Step 5: Run the whole suite**

Run: `cargo test --workspace`
Expected: PASS. Then `cargo clippy -p copper-dsn -p copper-core -p copperroute -- -D warnings`, which must be clean; the eight known `collapsible_if` failures are in `copper-router` and are not in scope.

- [ ] **Step 6: Document the new input**

Add `.kicad_pcb` to the input formats listed in `README.md` and in `crates/copperroute/README.md` if it names them. Describe the crates as they are; no history, no comparison to other tools.

- [ ] **Step 7: Commit**

```bash
git add crates/copperroute crates/copper-dsn README.md
git commit -m "Route a KiCad board from the command line"
```

---

### Task 11: Validate the parser against the corpus boards

**Files:**
- Create: `crates/copper-dsn/tests/kicad_pcb_corpus.rs`
- Create: `scripts/kicad-pcb-import-survey.sh`

**Interfaces:**
- Consumes: `read_pcb` from Task 7.
- Produces: a survey of which real boards the parser accepts. Nothing consumes it.

The five boards under `web/examples` were chosen to work with the web app. They do not
represent the corpus, which is mostly legacy KiCad 4 and 5 boards. These eleven cells carry
the largest `solder_mask_bridge` counts and are the stage two measurement set; here they
serve as realistic parser input.

```
LPC2148_Stick_LPC2148_stick
avr-fuser-32_adapter
NRC2016_banked_ram
MixSID_mixsid
kitspace_dropbot-front-panel
NRC2016_usb_sio
Own-Mailbox-Hardware_mailbox
Own-Mailbox-Hardware_eth
Librecalc-Hardware__autosave-calculator
FRM16_Relay_Module_I2C_Controller_relay_controller
8bit-cpu_programming_interface
```

Each corpus cell holds a `stripped.kicad_pcb` beside its `unrouted.dsn`. The corpus lives on
em@workbench, not in this checkout, so both artefacts here are opt-in and skip when the
boards are absent.

- [ ] **Step 1: Write the opt-in corpus test**

```rust
// crates/copper-dsn/tests/kicad_pcb_corpus.rs
use copper_dsn::kicad::pcb::read_pcb;
use copper_dsn::kicad::{read_board_json, NetClassJson};
use copper_dsn::error::BoardReadResult;

const BOARDS: [&str; 11] = [
    "LPC2148_Stick_LPC2148_stick",
    "avr-fuser-32_adapter",
    "NRC2016_banked_ram",
    "MixSID_mixsid",
    "kitspace_dropbot-front-panel",
    "NRC2016_usb_sio",
    "Own-Mailbox-Hardware_mailbox",
    "Own-Mailbox-Hardware_eth",
    "Librecalc-Hardware__autosave-calculator",
    "FRM16_Relay_Module_I2C_Controller_relay_controller",
    "8bit-cpu_programming_interface",
];

#[test]
fn it_imports_the_solder_mask_validation_boards() {
    let Ok(root) = std::env::var("COPPERROUTE_PCBENCH") else {
        eprintln!("skipping: set COPPERROUTE_PCBENCH to the corpus root");
        return;
    };
    let defaults = NetClassJson {
        name: Some("Default".to_string()),
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        ..Default::default()
    };
    let mut failures = Vec::new();
    for stem in BOARDS {
        let path = std::path::Path::new(&root).join(stem).join("stripped.kicad_pcb");
        if !path.exists() {
            failures.push(format!("{stem}: no stripped.kicad_pcb at {}", path.display()));
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("the board reads");
        match read_pcb(&text, &defaults) {
            Ok(imported) => match read_board_json(imported.board, None) {
                BoardReadResult::Success { board: Some(_), .. } => {}
                other => failures.push(format!("{stem}: the reader rejected it: {other:?}")),
            },
            Err(error) => failures.push(format!("{stem}: {}", error.message)),
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
```

- [ ] **Step 2: Run it where the corpus is present**

Run: `COPPERROUTE_PCBENCH=<corpus root> cargo test -p copper-dsn --test kicad_pcb_corpus -- --nocapture`
Expected: it either passes, or names each board and the construct that stopped it.

- [ ] **Step 3: Triage what it reports**

Every failure is one of two things, and they are handled differently.

A **porting bug** — the JS accepts the board and the Rust does not — is fixed here. Confirm
by running the Task 8 harness on the same board: `node crates/copper-dsn/tests/data/kicad_pcb_parity.mjs <board>`.

A **shared limitation** — both the JS and the Rust refuse it, for a construct neither
supports, such as copper zones without `rebuildZones`, net ties, or concave custom pads — is
not fixed here. It is a pre-existing limit of the KiCad adapter that this plan inherits
rather than introduces. Record it in step 4 and leave it.

- [ ] **Step 4: Write the survey script and record the outcome**

```bash
#!/usr/bin/env bash
# Report which corpus boards the native KiCad reader accepts.
set -euo pipefail
root="${1:?usage: kicad-pcb-import-survey.sh <corpus root>}"
cargo build --release -p copperroute
for board in "$root"/*/stripped.kicad_pcb; do
    stem=$(basename "$(dirname "$board")")
    if ./target/release/copperroute info "$board" >/dev/null 2>&1; then
        printf 'ok    %s\n' "$stem"
    else
        printf 'fail  %s: %s\n' "$stem" \
            "$(./target/release/copperroute info "$board" 2>&1 | tail -1)"
    fi
done
```

Run it across the whole corpus, not just the eleven, and append a short section to
`docs/solder-mask-bridge-investigation.md` giving the accept rate and the constructs behind
the most common refusals. That number decides whether stage two can measure on the native
path or must use `--kicad-board` beside a DSN, so it is worth having before stage two starts.

- [ ] **Step 5: Commit**

```bash
git add crates/copper-dsn/tests/kicad_pcb_corpus.rs scripts/kicad-pcb-import-survey.sh docs/solder-mask-bridge-investigation.md
git commit -m "Survey the native KiCad reader against the corpus boards"
```

---

## Self-review

**Spec coverage.** Every component in the spec's table maps to a task: `sexpr.rs` to Task 1, the reader split to Task 2, `pcb.rs` to Tasks 3 through 7 (split into four files, as the spec's risk section anticipated), `copper-core/job.rs` and `load.rs` to Task 9, `copperroute/ops/load.rs` and `cli.rs` to Task 10. All three verification layers appear: differential in Task 8, round trip in Task 7, rejection cases spread across Tasks 3 through 7, command line in Task 10. The spec's `--kicad-board` exclusion is respected — no task adds it.

**Deviation from the spec, deliberate.** The spec proposed one `pcb.rs` of roughly 700 lines and flagged splitting it if it grew. This plan splits it up front into `outline`, `structure`, `footprints` and `routing`, because those four have clean boundaries and each carries its own test file. The spec's risk section anticipated exactly this.

**Known unknowns, flagged in the tasks rather than guessed.** Three places tell the implementer to read the JS and match it rather than trusting a value written here: the footprint reference fallback in Task 5, the `importBoard` return shape in Task 8, and the default net class values in Task 9. Each is a fact that lives in code I did not fully read; inventing a value would be worse than a one-line lookup.

**The risk Task 11 exists to expose.** The KiCad adapter refuses several constructs outright,
including copper zones when zones are not being rebuilt. The corpus is legacy boards, which
use pours heavily. If the native path refuses most of them, stage two cannot measure on it
and must reach the mask settings through `--kicad-board` beside a DSN instead. That would not
waste this plan — the parser is what `--kicad-board` needs either way — but it changes stage
two's shape, so Task 11 runs before stage two is planned rather than after.
