# Plan 1 — Foundation & `fr-geometry` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the `freerouting-rs` Cargo workspace with a working CLI/MCP skeleton, a parity-test harness that can produce Java reference outputs, and a complete, unit-tested port of freerouting's `geometry/planar` package as the `fr-geometry` crate.

**Architecture:** Bottom-up port (spec §15). This plan covers build-order steps 1–2: the shell every later crate plugs into, and the geometry layer everything else depends on. Geometry is ported class-by-class from `../freerouting/src/main/java/app/freerouting/geometry/planar/`, keeping exact integer/rational arithmetic; Java's abstract `Point`/`Vector`/`Direction`/`TileShape`/`Shape` hierarchies become Rust enums with `match`-dispatch.

**Tech Stack:** Rust edition 2024 (stable), `clap` 4, `serde`/`serde_json`, `num-bigint`, `num-traits`, `num-integer`, `thiserror`, `tracing`/`tracing-subscriber`, `similar` (diffs in the harness). Java 25 + the pinned `freerouting-2.3.0.jar` release for reference generation.

**Spec:** `docs/superpowers/specs/2026-08-27-freerouting-rust-port-design.md`

**Later plans** (written after this one lands): Plan 2 `fr-board`, Plan 3 `fr-dsn`, Plan 4 `fr-settings`, Plan 5 `fr-drc`, Plan 6 `fr-router` (maze/expansion/path), Plan 7 `fr-router` (batch/optimizer), Plan 8 `fr-core` + full CLI/MCP.

## Global Constraints

- Rust edition **2024**; MSRV = current stable. No nightly features.
- Coordinates are `i32`; intermediates `i64`/`i128`; overflow beyond `CRIT_INT = 33_554_432` promotes to `BigInt` rationals exactly where Java does (spec §5).
- **No `f64` in intersection/containment/side-of paths.** Where Java uses `double` only because the value provably fits in 53 bits (products of two ≤2^25 numbers), use `i64` — results are identical.
- Java `Math.round(x)` is `floor(x + 0.5)` (rounds −1.5 to −1). Rust `f64::round` rounds half away from zero. Always use the `java_round` helper from Task 5; never call `.round()` on a coordinate.
- Java integer division truncates toward zero, same as Rust `/` on signed ints. Java `%` matches Rust `%`. `BigInteger.mod` is always non-negative (use `num_integer::Integer::mod_floor` semantics only where Java uses `.mod`; Java `.remainder` = Rust `%`).
- The port is **behavioral**: when a test expectation written in this plan disagrees with what the Java source does, **Java wins** — fix the test, and say so in the commit message.
- Every task ends with `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all clean, then a commit.
- Reference Java sources live at `../freerouting/src/main/java/app/freerouting/` relative to the workspace root (sibling clone). Do not modify the Java clone.
- Commit messages end with:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_015f1ubhBBpiWHAooDsso97Z
  ```

## File Structure (this plan)

```
freerouting-rs/
  Cargo.toml                          workspace: members = crates/*, tests/parity
  rust-toolchain.toml                 channel = "stable"
  .gitignore                          target/, tools/*.jar, tests/reference/_scratch/
  crates/
    fr-geometry/
      Cargo.toml
      src/lib.rs                      pub mod list + re-exports
      src/limits.rs                   CRIT_INT, CRIT_DOUBLE, SQRT2, java_round
      src/side.rs                     Side
      src/signum.rs                   Signum
      src/bigint_aux.rs               determinant, add_rational_coordinates, binary_gcd
      src/int_vector.rs               IntVector
      src/int_direction.rs            IntDirection + constants + Ord
      src/int_point.rs                IntPoint
      src/rational_vector.rs          RationalVector
      src/rational_point.rs           RationalPoint
      src/bigint_direction.rs         BigIntDirection
      src/point.rs                    enum Point
      src/vector.rs                   enum Vector
      src/direction.rs                enum Direction
      src/float_point.rs              FloatPoint
      src/float_line.rs               FloatLine
      src/line.rs                     Line
      src/int_box.rs                  IntBox
      src/int_octagon.rs              IntOctagon
      src/simplex.rs                  Simplex
      src/tile_shape.rs               enum TileShape + shared TileShape.java algorithms
      src/regular_tile_shape.rs       enum RegularTileShape (Box | Octagon)
      src/bounding_directions.rs      enum ShapeBoundingDirections, FortyfiveDegreeDirection
      src/line_segment.rs             LineSegment
      src/polygon.rs                  Polygon
      src/polyline.rs                 Polyline
      src/polyline_shape.rs           trait PolylineShapeOps (PolylineShape.java)
      src/polygon_shape.rs            PolygonShape
      src/polyline_area.rs            PolylineArea
      src/circle.rs                   Circle
      src/ellipse.rs                  Ellipse
      src/shape.rs                    trait ShapeOps, enum Shape, enum Area
    freerouting/
      Cargo.toml                      [[bin]] name = "freerouting"
      src/main.rs                     entry: legacy rewrite → clap → dispatch
      src/cli.rs                      clap definitions (Cli, Command, RouteArgs, DrcArgs, InfoArgs)
      src/legacy.rs                   rewrite(argv) -> Result<Vec<String>, LegacyError>
      src/commands/mod.rs
      src/commands/route.rs           stub (exit 2) until Plan 8
      src/commands/drc.rs             stub
      src/commands/info.rs            stub
      src/mcp/mod.rs
      src/mcp/jsonrpc.rs              Request/Response/Error types, parse/serialize
      src/mcp/server.rs               State, handle(&mut State, Request) -> Option<Response>
      src/mcp/stdio.rs                run_stdio() loop
      tests/legacy_cli.rs             spawn binary with -de/-do, check rewrite
      tests/mcp_stdio.rs              spawn binary `mcp`, drive initialize/ping/tools/list
  tests/parity/
      Cargo.toml                      crate `parity` (lib + tests)
      src/lib.rs                      paths, normalize_whitespace, assert_text_parity
      tests/harness.rs                self-tests of the helpers
  tests/reference/
      fixtures.txt                    one fixture stem per line
      README.md                       how references are produced
  scripts/gen-reference.sh            downloads jar, runs Java on fixtures, writes tests/reference/<stem>/
  tools/                              (gitignored) freerouting-2.3.0.jar
```

---

### Task 1: Cargo workspace skeleton

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `crates/fr-geometry/Cargo.toml`, `crates/fr-geometry/src/lib.rs`, `crates/freerouting/Cargo.toml`, `crates/freerouting/src/main.rs`, `tests/parity/Cargo.toml`, `tests/parity/src/lib.rs`

**Interfaces:**
- Produces: workspace members `fr-geometry`, `freerouting`, `parity`. Shared `[workspace.dependencies]` versions used by all later tasks.

- [ ] **Step 1: Write the workspace manifest**

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["crates/*", "tests/parity"]

[workspace.package]
edition = "2024"
version = "0.1.0"
license = "GPL-3.0-or-later"
repository = "https://github.com/freerouting/freerouting"

[workspace.dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
num-bigint = "0.4"
num-traits = "0.2"
num-integer = "0.1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
similar = "2"
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

`.gitignore`:
```
/target
/tools/*.jar
/tests/reference/_scratch/
```

- [ ] **Step 2: Create the three crates**

`crates/fr-geometry/Cargo.toml`:
```toml
[package]
name = "fr-geometry"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
num-bigint.workspace = true
num-traits.workspace = true
num-integer.workspace = true
```

`crates/fr-geometry/src/lib.rs`:
```rust
//! Planar geometry for the freerouting port. Faithful port of
//! `app.freerouting.geometry.planar` — exact integer/rational arithmetic.
```

`crates/freerouting/Cargo.toml`:
```toml
[package]
name = "freerouting"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "freerouting"
path = "src/main.rs"

[dependencies]
fr-geometry = { path = "../fr-geometry" }
clap.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

`crates/freerouting/src/main.rs`:
```rust
fn main() {
    println!("freerouting-rs");
}
```

`tests/parity/Cargo.toml`:
```toml
[package]
name = "parity"
version.workspace = true
edition.workspace = true
license.workspace = true
publish = false

[dependencies]
similar.workspace = true
```

`tests/parity/src/lib.rs`:
```rust
//! Parity-test helpers: locate the Java clone, fixtures and reference outputs.
```

- [ ] **Step 3: Verify the workspace builds and is clean**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: builds; 0 tests run; no warnings.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: cargo workspace skeleton (fr-geometry, freerouting bin, parity)"
```

---

### Task 2: CLI skeleton with legacy `-de/-do` shim

**Files:**
- Create: `crates/freerouting/src/cli.rs`, `crates/freerouting/src/legacy.rs`, `crates/freerouting/src/commands/{mod.rs,route.rs,drc.rs,info.rs}`, `crates/freerouting/tests/legacy_cli.rs`
- Modify: `crates/freerouting/src/main.rs`

**Interfaces:**
- Produces:
  - `cli::Cli { verbose: u8, log_level: Option<String>, command: Command }`
  - `cli::Command::{Route(RouteArgs), Drc(DrcArgs), Info(InfoArgs), Mcp}`
  - `cli::RouteArgs { input: PathBuf, output: PathBuf, rules: Option<PathBuf>, ses: Option<PathBuf>, max_passes: Option<u32>, timeout: Option<u64>, threads: Option<u32>, result_json: Option<PathBuf>, optimizer_improvement_threshold: Option<f64>, ignore_net_classes: Option<String>, update_strategy: Option<String>, hybrid_ratio: Option<String>, item_selection: Option<String>, set: Vec<String> }`
  - `cli::DrcArgs { input: PathBuf, ses: Option<PathBuf>, rules: Option<PathBuf>, output: Option<PathBuf> }`
  - `cli::InfoArgs { input: PathBuf }`
  - `legacy::rewrite(argv: &[String]) -> Result<Vec<String>, LegacyError>` — returns argv unchanged when no legacy flag present.
  - Exit codes: subcommand stubs exit **2** ("not implemented"); usage errors exit 64 (clap default is 2 — override with `Cli::command().error(...)`? No: keep clap's 2 for usage, use **3** for not-implemented to keep them distinct).

- [ ] **Step 1: Write failing unit tests for the legacy rewrite**

`crates/freerouting/src/legacy.rs` (tests at bottom; implementation comes in Step 3):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn passthrough_when_no_legacy_flags() {
        let argv = s(&["route", "a.dsn", "-o", "b.ses"]);
        assert_eq!(rewrite(&argv).unwrap(), argv);
    }

    #[test]
    fn de_do_become_route() {
        let argv = s(&["-de", "a.dsn", "-do", "b.ses"]);
        assert_eq!(rewrite(&argv).unwrap(), s(&["route", "a.dsn", "-o", "b.ses"]));
    }

    #[test]
    fn de_splits_plus_delimited_inputs() {
        let argv = s(&["-de", "a.dsn+a.ses+a.rules", "-do", "b.ses"]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&["route", "a.dsn", "--ses", "a.ses", "--rules", "a.rules", "-o", "b.ses"])
        );
    }

    #[test]
    fn numeric_and_strategy_flags_map() {
        let argv = s(&[
            "-de", "a.dsn", "-do", "b.ses", "-mp", "5", "-mt", "2", "-oit", "0.5",
            "-inc", "GND,VCC", "-us", "hybrid", "-hr", "1:1", "-is", "random", "-dr", "x.rules",
        ]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&[
                "route", "a.dsn", "-o", "b.ses", "--max-passes", "5", "--threads", "2",
                "--optimizer-improvement-threshold", "0.5", "--ignore-net-classes", "GND,VCC",
                "--update-strategy", "hybrid", "--hybrid-ratio", "1:1", "--item-selection", "random",
                "--rules", "x.rules",
            ])
        );
    }

    #[test]
    fn generic_section_field_overrides_become_set() {
        let argv = s(&["-de", "a.dsn", "-do", "b.ses", "--router.max_passes=7"]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&["route", "a.dsn", "-o", "b.ses", "--set", "router.max_passes=7"])
        );
    }

    #[test]
    fn drc_flag_becomes_drc_subcommand() {
        let argv = s(&["-de", "a.dsn+a.ses", "-drc", "r.json"]);
        assert_eq!(
            rewrite(&argv).unwrap(),
            s(&["drc", "a.dsn", "--ses", "a.ses", "-o", "r.json"])
        );
    }

    #[test]
    fn ignored_flags_are_dropped() {
        // -l, -da, -dl, -ll, -host, -dct, -im take a value; they are GUI/telemetry/binary-snapshot
        // options the port does not have.
        let argv = s(&["-de", "a.dsn", "-do", "b.ses", "-l", "en", "-da", "-dl", "-host", "x", "-im", "y"]);
        assert_eq!(rewrite(&argv).unwrap(), s(&["route", "a.dsn", "-o", "b.ses"]));
    }

    #[test]
    fn di_is_unsupported() {
        let argv = s(&["-di", "some/dir"]);
        assert!(matches!(rewrite(&argv), Err(LegacyError::Unsupported(f)) if f == "-di"));
    }

    #[test]
    fn de_without_do_or_drc_is_error() {
        let argv = s(&["-de", "a.dsn"]);
        assert!(matches!(rewrite(&argv), Err(LegacyError::MissingOutput)));
    }
}
```

Note on `-da` and `-dl`: in Java these are boolean switches (no value); `-l`, `-host`, `-dct`, `-im`, `-ll` take one value. Encode that in the implementation.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p freerouting legacy`
Expected: compile error — `rewrite` / `LegacyError` not defined.

- [ ] **Step 3: Implement `legacy.rs`**

```rust
//! Rewrites Java-freerouting command lines (`-de in.dsn -do out.ses -mp 100 …`)
//! into the subcommand form (`route in.dsn -o out.ses --max-passes 100`).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LegacyError {
    #[error("legacy flag {0} is not supported by freerouting-rs")]
    Unsupported(String),
    #[error("legacy flag {0} requires a value")]
    MissingValue(String),
    #[error("-de given without -do or -drc")]
    MissingOutput,
    #[error("-de input must include a .dsn file")]
    MissingDsn,
}

const LEGACY_TRIGGERS: &[&str] = &["-de", "-do", "-drc", "-di", "-dr", "-mp", "-mt", "-oit", "-inc", "-us", "-hr", "-is"];

/// Returns `argv` unchanged if it contains no legacy flags.
pub fn rewrite(argv: &[String]) -> Result<Vec<String>, LegacyError> {
    let has_legacy = argv.iter().any(|a| LEGACY_TRIGGERS.contains(&a.as_str()) || is_generic_override(a));
    if !has_legacy {
        return Ok(argv.to_vec());
    }

    let mut dsn: Option<String> = None;
    let mut ses: Option<String> = None;
    let mut rules: Option<String> = None;
    let mut output: Option<String> = None;
    let mut drc_output: Option<String> = None;
    let mut extra: Vec<String> = Vec::new();

    let mut i = 0;
    let take_value = |i: &mut usize, flag: &str| -> Result<String, LegacyError> {
        *i += 1;
        argv.get(*i).cloned().ok_or_else(|| LegacyError::MissingValue(flag.to_string()))
    };
    while i < argv.len() {
        let a = argv[i].as_str();
        match a {
            "-de" => {
                let v = take_value(&mut i, a)?;
                for part in v.split('+') {
                    let lower = part.to_ascii_lowercase();
                    if lower.ends_with(".dsn") {
                        dsn = Some(part.to_string());
                    } else if lower.ends_with(".ses") {
                        ses = Some(part.to_string());
                    } else if lower.ends_with(".rules") {
                        rules = Some(part.to_string());
                    } else {
                        dsn = Some(part.to_string());
                    }
                }
            }
            "-do" => output = Some(take_value(&mut i, a)?),
            "-drc" => drc_output = Some(take_value(&mut i, a)?),
            "-dr" => rules = Some(take_value(&mut i, a)?),
            "-di" => return Err(LegacyError::Unsupported(a.to_string())),
            "-mp" => push_pair(&mut extra, "--max-passes", take_value(&mut i, a)?),
            "-mt" => push_pair(&mut extra, "--threads", take_value(&mut i, a)?),
            "-oit" => push_pair(&mut extra, "--optimizer-improvement-threshold", take_value(&mut i, a)?),
            "-inc" => push_pair(&mut extra, "--ignore-net-classes", take_value(&mut i, a)?),
            "-us" => push_pair(&mut extra, "--update-strategy", take_value(&mut i, a)?),
            "-hr" => push_pair(&mut extra, "--hybrid-ratio", take_value(&mut i, a)?),
            "-is" => push_pair(&mut extra, "--item-selection", take_value(&mut i, a)?),
            // Value-taking flags the port ignores.
            "-l" | "-host" | "-dct" | "-im" | "-ll" => {
                let _ = take_value(&mut i, a)?;
            }
            // Boolean switches the port ignores.
            "-da" | "-dl" => {}
            _ if is_generic_override(a) => {
                push_pair(&mut extra, "--set", a.trim_start_matches("--").to_string());
            }
            _ => extra.push(a.to_string()),
        }
        i += 1;
    }

    let dsn = dsn.ok_or(LegacyError::MissingDsn)?;
    let mut out: Vec<String> = Vec::new();
    if let Some(report) = drc_output {
        out.push("drc".into());
        out.push(dsn);
        if let Some(s) = ses { push_pair(&mut out, "--ses", s); }
        if let Some(r) = rules { push_pair(&mut out, "--rules", r); }
        push_pair(&mut out, "-o", report);
    } else {
        let output = output.ok_or(LegacyError::MissingOutput)?;
        out.push("route".into());
        out.push(dsn);
        if let Some(s) = ses { push_pair(&mut out, "--ses", s); }
        if let Some(r) = rules { push_pair(&mut out, "--rules", r); }
        push_pair(&mut out, "-o", output);
    }
    out.extend(extra);
    Ok(out)
}

/// `--section.field=value` (Java generic settings override).
fn is_generic_override(a: &str) -> bool {
    a.starts_with("--") && a.contains('.') && a.contains('=')
}

fn push_pair(v: &mut Vec<String>, flag: &str, value: String) {
    v.push(flag.to_string());
    v.push(value);
}
```

Check the test `numeric_and_strategy_flags_map`: `-dr x.rules` arrives after the numeric flags but `--rules` is emitted before `-o`. Adjust the expected vector in the test to `["route","a.dsn","--rules","x.rules","-o","b.ses","--max-passes",…]` — fixed positions come first, `extra` last. (This is the deliberate order; update the test, not the code.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p freerouting legacy`
Expected: all 9 pass.

- [ ] **Step 5: Write `cli.rs`**

```rust
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "freerouting", version, about = "Headless PCB autorouter (Rust port of freerouting)")]
pub struct Cli {
    /// Increase log verbosity (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
    /// Explicit log level (error|warn|info|debug|trace); overrides -v
    #[arg(long, global = true)]
    pub log_level: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Route a Specctra DSN board and write a SES session file
    Route(RouteArgs),
    /// Run the design rule checker and emit a KiCad-schema JSON report
    Drc(DrcArgs),
    /// Print board summary (layers, nets, components) as JSON
    Info(InfoArgs),
    /// Run as an MCP server over stdio
    Mcp,
}

#[derive(Args, Debug)]
pub struct RouteArgs {
    /// Input .dsn file
    pub input: PathBuf,
    /// Output .ses (or .dsn) file
    #[arg(short, long)]
    pub output: PathBuf,
    #[arg(long)]
    pub rules: Option<PathBuf>,
    /// Previous session to load before routing (incremental)
    #[arg(long)]
    pub ses: Option<PathBuf>,
    #[arg(long)]
    pub max_passes: Option<u32>,
    /// Routing timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,
    #[arg(long)]
    pub threads: Option<u32>,
    #[arg(long)]
    pub result_json: Option<PathBuf>,
    #[arg(long)]
    pub optimizer_improvement_threshold: Option<f64>,
    #[arg(long)]
    pub ignore_net_classes: Option<String>,
    #[arg(long)]
    pub update_strategy: Option<String>,
    #[arg(long)]
    pub hybrid_ratio: Option<String>,
    #[arg(long)]
    pub item_selection: Option<String>,
    /// Generic settings override, `section.field=value` (repeatable)
    #[arg(long = "set")]
    pub set: Vec<String>,
}

#[derive(Args, Debug)]
pub struct DrcArgs {
    pub input: PathBuf,
    #[arg(long)]
    pub ses: Option<PathBuf>,
    #[arg(long)]
    pub rules: Option<PathBuf>,
    /// Report path; stdout when omitted
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    pub input: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_route() {
        let cli = Cli::try_parse_from(["freerouting", "route", "a.dsn", "-o", "b.ses", "--max-passes", "3", "--set", "x.y=1"]).unwrap();
        match cli.command {
            Command::Route(r) => {
                assert_eq!(r.input, PathBuf::from("a.dsn"));
                assert_eq!(r.max_passes, Some(3));
                assert_eq!(r.set, vec!["x.y=1".to_string()]);
            }
            _ => panic!("expected route"),
        }
    }
    #[test]
    fn parses_mcp() {
        let cli = Cli::try_parse_from(["freerouting", "mcp"]).unwrap();
        assert!(matches!(cli.command, Command::Mcp));
    }
}
```

- [ ] **Step 6: Write command stubs and `main.rs`**

`crates/freerouting/src/commands/mod.rs`:
```rust
pub mod drc;
pub mod info;
pub mod route;

/// Exit code for subcommands not yet ported.
pub const EXIT_NOT_IMPLEMENTED: i32 = 3;
```

`crates/freerouting/src/commands/route.rs`:
```rust
use crate::cli::RouteArgs;

pub fn run(args: &RouteArgs) -> i32 {
    tracing::error!(input = %args.input.display(), "route: not implemented yet (Plan 8)");
    super::EXIT_NOT_IMPLEMENTED
}
```
`drc.rs` and `info.rs`: identical shape with `DrcArgs` / `InfoArgs`.

`crates/freerouting/src/main.rs`:
```rust
mod cli;
mod commands;
mod legacy;
mod mcp;

use clap::Parser;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let argv = match legacy::rewrite(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let cli = cli::Cli::parse_from(std::iter::once("freerouting".to_string()).chain(argv));
    init_logging(cli.verbose, cli.log_level.as_deref());
    let code = match &cli.command {
        cli::Command::Route(a) => commands::route::run(a),
        cli::Command::Drc(a) => commands::drc::run(a),
        cli::Command::Info(a) => commands::info::run(a),
        cli::Command::Mcp => mcp::stdio::run(),
    };
    std::process::exit(code);
}

fn init_logging(verbose: u8, level: Option<&str>) {
    let filter = match (level, verbose) {
        (Some(l), _) => l.to_string(),
        (None, 0) => "warn".into(),
        (None, 1) => "info".into(),
        (None, 2) => "debug".into(),
        (None, _) => "trace".into(),
    };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_writer(std::io::stderr)
        .init();
}
```
Until Task 3 exists, add a temporary `mcp/mod.rs` containing `pub mod stdio { pub fn run() -> i32 { 3 } }`.

- [ ] **Step 7: Write the end-to-end legacy test**

`crates/freerouting/tests/legacy_cli.rs`:
```rust
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_freerouting"))
}

#[test]
fn legacy_de_do_reaches_route_stub() {
    let out = bin().args(["-de", "a.dsn", "-do", "b.ses", "-mp", "1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("route: not implemented"));
}

#[test]
fn legacy_di_is_rejected() {
    let out = bin().args(["-di", "dir"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not supported"));
}

#[test]
fn subcommand_help_works() {
    let out = bin().args(["route", "--help"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("--max-passes"));
}
```

- [ ] **Step 8: Run everything**

Run: `cargo test -p freerouting && cargo clippy -p freerouting --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(cli): clap subcommands with Java-compatible -de/-do legacy shim"
```

---

### Task 3: MCP stdio skeleton

**Files:**
- Create: `crates/freerouting/src/mcp/{mod.rs,jsonrpc.rs,server.rs,stdio.rs}`, `crates/freerouting/tests/mcp_stdio.rs`
- Modify: `crates/freerouting/src/main.rs` (remove the temporary stub)

**Interfaces:**
- Produces:
  - `jsonrpc::Request { jsonrpc: String, id: Option<serde_json::Value>, method: String, params: Option<serde_json::Value> }`
  - `jsonrpc::Response` (either `result` or `error`), `jsonrpc::RpcError { code: i64, message: String, data: Option<Value> }`
  - `server::State::new() -> State`; `server::handle(&mut State, Request) -> Option<Response>` (None for notifications)
  - `server::ToolDef { name, description, input_schema }` and `State::register_tool(ToolDef, handler)` where `handler: Box<dyn Fn(Value) -> Result<Value, RpcError>>` — Plan 8 registers the real tools here.
  - `stdio::run() -> i32`
  - Protocol version string constant `PROTOCOL_VERSION = "2025-06-18"`.

- [ ] **Step 1: Write failing unit tests for `server::handle`**

In `crates/freerouting/src/mcp/server.rs` test module:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn req(id: i64, method: &str, params: serde_json::Value) -> Request {
        Request { jsonrpc: "2.0".into(), id: Some(json!(id)), method: method.into(), params: Some(params) }
    }

    #[test]
    fn initialize_returns_server_info_and_tools_capability() {
        let mut st = State::new();
        let resp = handle(&mut st, req(1, "initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}))).unwrap();
        let r = resp.result.unwrap();
        assert_eq!(r["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(r["serverInfo"]["name"], "freerouting");
        assert!(r["capabilities"]["tools"].is_object());
    }

    #[test]
    fn ping_returns_empty_object() {
        let mut st = State::new();
        let resp = handle(&mut st, req(2, "ping", json!({}))).unwrap();
        assert_eq!(resp.result.unwrap(), json!({}));
    }

    #[test]
    fn notifications_get_no_response() {
        let mut st = State::new();
        let n = Request { jsonrpc: "2.0".into(), id: None, method: "notifications/initialized".into(), params: None };
        assert!(handle(&mut st, n).is_none());
    }

    #[test]
    fn tools_list_is_empty_by_default_and_lists_registered() {
        let mut st = State::new();
        let resp = handle(&mut st, req(3, "tools/list", json!({}))).unwrap();
        assert_eq!(resp.result.unwrap()["tools"], json!([]));
        st.register_tool(
            ToolDef { name: "echo".into(), description: "echo".into(), input_schema: json!({"type": "object"}) },
            Box::new(|v| Ok(v)),
        );
        let resp = handle(&mut st, req(4, "tools/list", json!({}))).unwrap();
        assert_eq!(resp.result.unwrap()["tools"][0]["name"], "echo");
    }

    #[test]
    fn tools_call_dispatches_and_wraps_content() {
        let mut st = State::new();
        st.register_tool(
            ToolDef { name: "echo".into(), description: "echo".into(), input_schema: json!({"type": "object"}) },
            Box::new(|v| Ok(v)),
        );
        let resp = handle(&mut st, req(5, "tools/call", json!({"name": "echo", "arguments": {"a": 1}}))).unwrap();
        let r = resp.result.unwrap();
        assert_eq!(r["isError"], false);
        assert_eq!(r["structuredContent"], json!({"a": 1}));
        assert_eq!(r["content"][0]["type"], "text");
    }

    #[test]
    fn unknown_method_and_unknown_tool_error() {
        let mut st = State::new();
        let resp = handle(&mut st, req(6, "nope", json!({}))).unwrap();
        assert_eq!(resp.error.unwrap().code, -32601);
        let resp = handle(&mut st, req(7, "tools/call", json!({"name": "missing", "arguments": {}}))).unwrap();
        assert_eq!(resp.error.unwrap().code, -32602);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p freerouting mcp`
Expected: compile errors (types missing).

- [ ] **Step 3: Implement `jsonrpc.rs`**

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0", id, result: Some(result), error: None }
    }
    pub fn err(id: Value, code: i64, message: impl Into<String>) -> Self {
        Self { jsonrpc: "2.0", id, result: None, error: Some(RpcError { code, message: message.into(), data: None }) }
    }
}

impl RpcError {
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self { code: INVALID_PARAMS, message: msg.into(), data: None }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { code: INTERNAL_ERROR, message: msg.into(), data: None }
    }
}
```

- [ ] **Step 4: Implement `server.rs`**

```rust
use super::jsonrpc::{METHOD_NOT_FOUND, Request, Response, RpcError};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub type ToolHandler = Box<dyn Fn(Value) -> Result<Value, RpcError> + Send>;

#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct State {
    tools: BTreeMap<String, (ToolDef, ToolHandler)>,
    pub initialized: bool,
}

impl State {
    pub fn new() -> Self {
        Self { tools: BTreeMap::new(), initialized: false }
    }
    pub fn register_tool(&mut self, def: ToolDef, handler: ToolHandler) {
        self.tools.insert(def.name.clone(), (def, handler));
    }
}

pub fn handle(state: &mut State, req: Request) -> Option<Response> {
    let id = req.id.clone()?; // notifications: no id → no response
    let params = req.params.unwrap_or(Value::Null);
    let resp = match req.method.as_str() {
        "initialize" => {
            state.initialized = true;
            Response::ok(id, json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "freerouting", "version": env!("CARGO_PKG_VERSION") },
            }))
        }
        "ping" => Response::ok(id, json!({})),
        "tools/list" => {
            let tools: Vec<Value> = state.tools.values().map(|(d, _)| json!({
                "name": d.name, "description": d.description, "inputSchema": d.input_schema,
            })).collect();
            Response::ok(id, json!({ "tools": tools }))
        }
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match state.tools.get(name) {
                None => Response::err(id, super::jsonrpc::INVALID_PARAMS, format!("unknown tool: {name}")),
                Some((_, h)) => match h(args) {
                    Ok(v) => Response::ok(id, json!({
                        "content": [{ "type": "text", "text": v.to_string() }],
                        "structuredContent": v,
                        "isError": false,
                    })),
                    Err(e) => Response::ok(id, json!({
                        "content": [{ "type": "text", "text": e.message }],
                        "isError": true,
                    })),
                },
            }
        }
        m => Response::err(id, METHOD_NOT_FOUND, format!("method not found: {m}")),
    };
    Some(resp)
}
```

Note: per the MCP spec, tool *execution* failures are reported inside the result with `isError: true`, while protocol errors (unknown tool, bad params) are JSON-RPC errors. Keep that split.

- [ ] **Step 5: Implement `stdio.rs` and `mod.rs`**

`mod.rs`:
```rust
pub mod jsonrpc;
pub mod server;
pub mod stdio;
```

`stdio.rs`:
```rust
use super::jsonrpc::{PARSE_ERROR, Request, Response};
use super::server::{State, handle};
use serde_json::Value;
use std::io::{BufRead, Write};

/// Newline-delimited JSON-RPC over stdin/stdout. Returns the process exit code.
pub fn run() -> i32 {
    let mut state = State::new();
    run_with(&mut state, std::io::stdin().lock(), std::io::stdout().lock())
}

pub fn run_with<R: BufRead, W: Write>(state: &mut State, reader: R, mut writer: W) -> i32 {
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handle(state, req),
            Err(e) => Some(Response::err(Value::Null, PARSE_ERROR, format!("parse error: {e}"))),
        };
        if let Some(resp) = response {
            let text = serde_json::to_string(&resp).expect("response serializes");
            if writeln!(writer, "{text}").and_then(|_| writer.flush()).is_err() {
                break;
            }
        }
    }
    0
}
```

- [ ] **Step 6: Write the spawned-process e2e test**

`crates/freerouting/tests/mcp_stdio.rs`:
```rust
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn initialize_ping_and_list_over_pipes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let mut send = |s: &str| {
        writeln!(stdin, "{s}").unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str::<serde_json::Value>(&line).unwrap()
    };

    let init = send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#);
    assert_eq!(init["result"]["serverInfo"]["name"], "freerouting");
    writeln!(stdin, r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#).unwrap();
    let ping = send(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
    assert_eq!(ping["result"], serde_json::json!({}));
    let list = send(r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#);
    assert!(list["result"]["tools"].as_array().unwrap().is_empty());

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}
```

- [ ] **Step 7: Run all tests + lint**

Run: `cargo test -p freerouting && cargo clippy -p freerouting --all-targets -- -D warnings`
Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(mcp): native stdio JSON-RPC server skeleton (initialize/ping/tools)"
```

---

### Task 4: Parity harness and Java reference generation

**Files:**
- Create: `tests/parity/src/lib.rs` (replace stub), `tests/parity/tests/harness.rs`, `tests/reference/fixtures.txt`, `tests/reference/README.md`, `scripts/gen-reference.sh`

**Interfaces:**
- Produces:
  - `parity::workspace_root() -> PathBuf` (from `CARGO_MANIFEST_DIR` of the parity crate, two levels up)
  - `parity::java_dir() -> PathBuf` — `$FREEROUTING_JAVA_DIR` or `<root>/../freerouting`
  - `parity::fixture(name: &str) -> PathBuf` — `<java_dir>/fixtures/<name>`; `parity::example(name)` → `<java_dir>/examples/<name>`
  - `parity::reference(stem: &str, file: &str) -> PathBuf` — `<root>/tests/reference/<stem>/<file>`
  - `parity::normalize_whitespace(&str) -> String` — CRLF→LF, strip trailing spaces per line, collapse runs of blank lines, ensure single trailing `\n`
  - `parity::assert_text_parity(actual: &str, reference_path: &Path)` — panics with unified diff; writes `actual` to `<root>/tests/reference/_scratch/<stem>/<file>` for inspection
  - `parity::require_reference(path: &Path) -> bool` — returns false (and prints a skip note) if the reference file is missing, so tests can `return` rather than fail when references haven't been generated locally.

- [ ] **Step 1: Write failing tests for the helpers**

`tests/parity/tests/harness.rs`:
```rust
use parity::*;

#[test]
fn normalize_collapses_crlf_trailing_spaces_and_blank_runs() {
    let s = "a  \r\n\r\n\r\nb\n\n\n";
    assert_eq!(normalize_whitespace(s), "a\n\nb\n");
}

#[test]
fn normalize_adds_single_trailing_newline() {
    assert_eq!(normalize_whitespace("x"), "x\n");
    assert_eq!(normalize_whitespace("x\n\n"), "x\n");
}

#[test]
fn workspace_root_contains_cargo_toml() {
    assert!(workspace_root().join("Cargo.toml").exists());
}

#[test]
fn reference_path_layout() {
    let p = reference("tutorial_board", "roundtrip.dsn");
    assert!(p.ends_with("tests/reference/tutorial_board/roundtrip.dsn"));
}

#[test]
#[should_panic(expected = "parity mismatch")]
fn assert_text_parity_panics_on_difference() {
    let dir = std::env::temp_dir().join("fr-parity-test");
    std::fs::create_dir_all(&dir).unwrap();
    let r = dir.join("ref.txt");
    std::fs::write(&r, "one\ntwo\n").unwrap();
    assert_text_parity("one\nthree\n", &r);
}

#[test]
fn assert_text_parity_passes_modulo_whitespace() {
    let dir = std::env::temp_dir().join("fr-parity-test2");
    std::fs::create_dir_all(&dir).unwrap();
    let r = dir.join("ref.txt");
    std::fs::write(&r, "one\r\ntwo  \r\n").unwrap();
    assert_text_parity("one\ntwo\n\n", &r);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p parity`
Expected: compile errors.

- [ ] **Step 3: Implement `tests/parity/src/lib.rs`**

```rust
//! Parity-test helpers: locate the Java clone, fixtures and reference outputs,
//! and compare text outputs modulo whitespace.

use similar::{ChangeTag, TextDiff};
use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
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
    workspace_root().join("tests").join("reference").join(stem).join(file)
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
        eprintln!("SKIP: reference {} missing — run scripts/gen-reference.sh", path.display());
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
    if let Some(rel) = reference_path.strip_prefix(workspace_root().join("tests").join("reference")).ok() {
        let scratch = workspace_root().join("tests").join("reference").join("_scratch").join(rel);
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p parity`
Expected: 6 pass.

- [ ] **Step 5: Write `tests/reference/fixtures.txt` and README**

`fixtures.txt` (stem → source path, `|`-separated; examples first because they are small):
```
tutorial_board|examples/tutorial_board/tutorial_board.dsn
Issue026-J2_reference|fixtures/Issue026-J2_reference.dsn
Issue103-Board-Unrouted|fixtures/Issue103-Board-Unrouted.dsn
Issue143-rpi_splitter|fixtures/Issue143-rpi_splitter.dsn
```
(Later plans append more stems. Keep this list short in Plan 1; the script is idempotent.)

`README.md`:
```
# Java reference outputs

Generated by `scripts/gen-reference.sh` from the pinned freerouting 2.3.0 jar
(the parity baseline per the Java repo's AGENTS.md). For each stem in
`fixtures.txt` the script produces:

- `<stem>/roundtrip.dsn` — Java: read DSN, write DSN unchanged (`-do <stem>.dsn`, `-mp 0`)
- `<stem>/unrouted.ses` — Java: read DSN, write SES without routing (`-do <stem>.ses`, `-mp 0`)
- `<stem>/java.log`     — stderr/stdout of the Java run, for debugging

Reference files are committed. `_scratch/` holds the Rust side of a failing
comparison and is gitignored.

Requirements: Java 25 (`brew install openjdk@25` on macOS), network access on
first run to download the jar into `tools/`.
```

- [ ] **Step 6: Write `scripts/gen-reference.sh`**

```bash
#!/usr/bin/env bash
# Generate Java reference outputs for parity tests.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAR_VERSION="2.3.0"
JAR="$ROOT/tools/freerouting-$JAR_VERSION.jar"
JAR_URL="https://github.com/freerouting/freerouting/releases/download/v$JAR_VERSION/freerouting-$JAR_VERSION.jar"
REF="$ROOT/tests/reference"
JAVA_BIN="${JAVA:-java}"

# --- Java version check (jar targets JDK 25) ---
ver="$("$JAVA_BIN" -version 2>&1 | head -1 | sed -E 's/.*"([0-9]+).*/\1/')"
if [[ "${ver:-0}" -lt 25 ]]; then
  echo "error: Java 25+ required (found ${ver:-none}). On macOS: brew install openjdk@25" >&2
  echo "       then: export JAVA=/opt/homebrew/opt/openjdk@25/bin/java" >&2
  exit 1
fi

mkdir -p "$ROOT/tools"
if [[ ! -f "$JAR" ]]; then
  echo "downloading $JAR_URL"
  curl -fL --retry 3 -o "$JAR" "$JAR_URL"
fi

run_java() { # args...
  "$JAVA_BIN" -jar "$JAR" -da -dl "$@"
}

while IFS='|' read -r stem src; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  in="$JAVA_DIR/$src"
  out="$REF/$stem"
  mkdir -p "$out"
  echo "== $stem"
  # DSN round-trip (no routing passes) and unrouted SES.
  run_java -de "$in" -do "$out/roundtrip.dsn" -mp 0 > "$out/java.log" 2>&1 || {
    echo "   java failed for roundtrip.dsn; see $out/java.log" >&2; }
  run_java -de "$in" -do "$out/unrouted.ses" -mp 0 >> "$out/java.log" 2>&1 || {
    echo "   java failed for unrouted.ses; see $out/java.log" >&2; }
done < "$REF/fixtures.txt"

echo "done. References in $REF"
```

`chmod +x scripts/gen-reference.sh`.

**Executor note:** verify empirically that `-mp 0` produces an *unrouted* SES (open `unrouted.ses`; its `(routes …)` section must contain only wiring already present in the DSN). If the jar routes anyway, switch the flag to `--router.enabled=false` (documented generic override) and record which one worked in `tests/reference/README.md`. If Java 25 is not installable in this environment, still commit the script and README, leave `tests/reference/<stem>/` empty, and say so in the commit message — parity tests use `require_reference` and will skip.

- [ ] **Step 7: Run the script once (if Java 25 available) and inspect**

Run: `scripts/gen-reference.sh && ls tests/reference/tutorial_board && head -5 tests/reference/tutorial_board/roundtrip.dsn`
Expected: `roundtrip.dsn` begins with `(pcb`; `unrouted.ses` begins with `(session`.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "test(parity): harness helpers and Java reference generation script"
```

---

## Geometry port — conventions for Tasks 5–19

Every geometry task ports one or more Java files from
`../freerouting/src/main/java/app/freerouting/geometry/planar/` (and two from
`datastructures/`). The procedure is the same each time:

1. Read the Java file(s) end to end **before** writing Rust.
2. Write the tests in the task first (they encode a handful of hand-checked values); run them to see them fail.
3. Translate **method by method, in source order**, keeping Java's method names in `snake_case` (`sideOf` → `side_of`, `turn45Degree` → `turn_45_degree`, `isMultipleOf45Degree` → `is_multiple_of_45_degree`). Keep Java's comments where they explain intent.
4. Java `double` arithmetic used for *signs* of products of ≤2^25 values → `i64`. Java `double` used for actual approximations (`…Approx`, `distance`, `area`, `length`) → `f64`.
5. Java `null` returns → `Option<T>`. Java arrays → `Vec<T>` (or `[T; N]` where the length is fixed: IntOctagon has 8 border lines, IntBox 4).
6. Java `Comparable`/`compareTo` → `impl Ord` (only when it is a total order; `IntDirection.compareTo` is a total angular order) or an explicit `fn compare_xy(&self, other) -> Ordering`.
7. Java `equals`/`hashCode` → `#[derive(PartialEq, Eq, Hash)]` when structural, or manual `impl PartialEq` when Java compares by value across representations (RationalPoint).
8. Anything Java marks `@Deprecated`, GUI-only, or unused by `board/`, `autoroute/`, `drc/`, `io/` (check with `grep -rn "methodName(" ../freerouting/src/main/java/app/freerouting --include=*.java | grep -v geometry/`) may be **skipped**; write `// not ported: unused outside geometry` in the Rust file so the omission is deliberate and greppable.
9. `FRLogger.warn/debug` calls → `tracing::warn!/debug!`? **No** — `fr-geometry` must not depend on `tracing`. Replace with `debug_assert!` where Java is guarding an invariant, or drop when it is only a diagnostic.
10. Finish with `cargo fmt`, `cargo clippy -p fr-geometry --all-targets -- -D warnings`, `cargo test -p fr-geometry`, commit.

Type mapping (fixed for all tasks; later crates depend on these names):

| Java | Rust |
|---|---|
| `Point` (abstract) | `enum Point { Int(IntPoint), Rational(RationalPoint) }` |
| `Vector` (abstract) | `enum Vector { Int(IntVector), Rational(RationalVector) }` |
| `Direction` (abstract) | `enum Direction { Int(IntDirection), Big(BigIntDirection) }` |
| `TileShape` (abstract) | `enum TileShape { Box(IntBox), Octagon(IntOctagon), Simplex(Simplex) }` |
| `RegularTileShape` | `enum RegularTileShape { Box(IntBox), Octagon(IntOctagon) }` |
| `Shape` (interface) | `trait ShapeOps` + `enum Shape { Tile(TileShape), Polygon(PolygonShape), Circle(Circle) }` |
| `Area` (interface) | `enum Area { Shape(Shape), Polyline(PolylineArea) }` |
| `PolylineShape` (abstract) | `trait PolylineShapeOps` (implemented by `TileShape`, `IntBox`, `IntOctagon`, `Simplex`, `PolygonShape`) |
| `ShapeBoundingDirections` + 2 impls | `enum ShapeBoundingDirections { Orthogonal, FortyfiveDegree }` |
| `FortyfiveDegreeDirection` | `enum FortyfiveDegreeDirection { Right, Right45, Up, Up45, Left, Left45, Down, Down45 }` |
| `Side`, `Signum` | enums |
| `BigInteger` | `num_bigint::BigInt` |

All geometry structs derive `Debug, Clone, PartialEq` (plus `Copy, Eq, Hash` for the plain-integer ones: `IntPoint`, `IntVector`, `IntDirection`, `IntBox`, `IntOctagon`, `Line`, `Side`, `Signum`, `FloatPoint` is `Copy, PartialEq` only).

---

### Task 5: Primitives — `limits`, `side`, `signum`, `bigint_aux`

**Files:**
- Create: `crates/fr-geometry/src/{limits.rs,side.rs,signum.rs,bigint_aux.rs}`
- Modify: `crates/fr-geometry/src/lib.rs`
- Java sources: `Limits.java`, `Side.java`, `../datastructures/Signum.java`, `../datastructures/BigIntAux.java`

**Interfaces:**
- Produces:
  - `limits::{CRIT_INT: i32 = 33_554_432, CRIT_DOUBLE: f64 = 9007199254740992.0, SQRT2: f64}`, `limits::crit_int_big() -> BigInt`, `limits::java_round(x: f64) -> i64`
  - `enum Side { OnTheLeft, OnTheRight, Collinear }` with `Side::of_i64(i64)`, `Side::of_f64(f64)`, `negate()`
  - `enum Signum { Positive, Negative, Zero }` with `of_i64`, `of_f64`, `as_int_i64(i64) -> i32`, `as_int_f64(f64) -> i32`, `negate()`
  - `bigint_aux::determinant(x1,y1,x2,y2: &BigInt) -> BigInt`, `add_rational_coordinates(&[BigInt;3], &[BigInt;3]) -> [BigInt;3]`, `binary_gcd(a: i32, b: i32) -> i32`

- [ ] **Step 1: Write failing tests**

`crates/fr-geometry/src/limits.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_round_rounds_half_up_not_away_from_zero() {
        assert_eq!(java_round(1.5), 2);
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-1.5), -1); // Java Math.round(-1.5) == -1
        assert_eq!(java_round(-2.5), -2);
        assert_eq!(java_round(-0.4), 0);
        assert_eq!(java_round(0.49999999999999994), 0);
    }
    #[test]
    fn constants() {
        assert_eq!(CRIT_INT, 1 << 25);
        assert_eq!(crit_int_big(), num_bigint::BigInt::from(CRIT_INT));
    }
}
```
`side.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn of_and_negate() {
        assert_eq!(Side::of_i64(5), Side::OnTheLeft);
        assert_eq!(Side::of_i64(-5), Side::OnTheRight);
        assert_eq!(Side::of_i64(0), Side::Collinear);
        assert_eq!(Side::OnTheLeft.negate(), Side::OnTheRight);
        assert_eq!(Side::Collinear.negate(), Side::Collinear);
        assert_eq!(Side::of_f64(-0.0), Side::Collinear);
    }
}
```
`signum.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn of_and_as_int() {
        assert_eq!(Signum::of_i64(3), Signum::Positive);
        assert_eq!(Signum::of_f64(-0.5), Signum::Negative);
        assert_eq!(Signum::as_int_f64(0.0), 0);
        assert_eq!(Signum::as_int_i64(-9), -1);
        assert_eq!(Signum::Positive.negate(), Signum::Negative);
    }
}
```
`bigint_aux.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;
    fn b(v: i64) -> BigInt { BigInt::from(v) }
    #[test]
    fn gcd_matches_euclid() {
        assert_eq!(binary_gcd(12, 18), 6);
        assert_eq!(binary_gcd(0, 7), 7);
        assert_eq!(binary_gcd(7, 0), 7);
        assert_eq!(binary_gcd(1, 1), 1);
        assert_eq!(binary_gcd(1 << 20, 3 << 10), 1 << 10);
        assert_eq!(binary_gcd(33_554_432, 33_554_432), 33_554_432);
    }
    #[test]
    fn determinant_and_rational_add() {
        assert_eq!(determinant(&b(1), &b(2), &b(3), &b(4)), b(-2));
        let r = add_rational_coordinates(&[b(1), b(2), b(3)], &[b(1), b(1), b(3)]);
        assert_eq!(r, [b(2), b(3), b(3)]);
        let r = add_rational_coordinates(&[b(1), b(2), b(3)], &[b(1), b(1), b(2)]);
        assert_eq!(r, [b(5), b(7), b(6)]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fr-geometry`
Expected: compile errors.

- [ ] **Step 3: Implement**

`limits.rs`:
```rust
use num_bigint::BigInt;

/// Coordinates with |value| above this promote to rational (BigInt) arithmetic.
pub const CRIT_INT: i32 = 33_554_432; // 2^25
pub const CRIT_DOUBLE: f64 = 9_007_199_254_740_992.0; // 2^53
pub const SQRT2: f64 = std::f64::consts::SQRT_2;

pub fn crit_int_big() -> BigInt {
    BigInt::from(CRIT_INT)
}

/// Java `Math.round(double)`: floor(x + 0.5), so -1.5 → -1.
pub fn java_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}
```
(`java_round(0.49999999999999994)`: `0.49999999999999994 + 0.5` rounds to `1.0` in f64, so `floor` gives 1 — but Java's `Math.round` has special handling and returns 0. Implement it as Java does since JDK 7: `if x.is_nan() {0} else { let f = x.floor(); if x - f >= 0.5 { f as i64 + 1 } else { f as i64 } }`. Use that form; the test above checks it.)

`side.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    OnTheLeft,
    OnTheRight,
    Collinear,
}

impl Side {
    pub fn of_i64(value: i64) -> Side {
        match value.signum() {
            1 => Side::OnTheLeft,
            -1 => Side::OnTheRight,
            _ => Side::Collinear,
        }
    }
    pub fn of_f64(value: f64) -> Side {
        if value > 0.0 { Side::OnTheLeft } else if value < 0.0 { Side::OnTheRight } else { Side::Collinear }
    }
    pub fn negate(self) -> Side {
        match self {
            Side::OnTheLeft => Side::OnTheRight,
            Side::OnTheRight => Side::OnTheLeft,
            Side::Collinear => Side::Collinear,
        }
    }
}
```
`signum.rs`: same shape with `Positive/Negative/Zero`, `as_int_*` returning `1/-1/0`.

`bigint_aux.rs`: `determinant` = `x1*y2 - x2*y1`; `add_rational_coordinates` exactly as Java (same-denominator fast path, else cross-multiply); `binary_gcd(a, b)`: inputs are non-negative in all call sites (Java callers pass `Math.abs`), so implement with `num_integer::Integer::gcd` on `i32` after `debug_assert!(a >= 0 && b >= 0)` — mathematically identical to Java's binary GCD.

- [ ] **Step 4: Wire `lib.rs`**

```rust
pub mod bigint_aux;
pub mod limits;
pub mod side;
pub mod signum;

pub use limits::{CRIT_INT, java_round};
pub use side::Side;
pub use signum::Signum;
```

- [ ] **Step 5: Run tests, lint, commit**

Run: `cargo test -p fr-geometry && cargo clippy -p fr-geometry --all-targets -- -D warnings`

```bash
git add -A
git commit -m "feat(geometry): limits, Side, Signum, bigint helpers"
```

---

### Task 6: `IntVector` and `IntDirection`

**Files:**
- Create: `crates/fr-geometry/src/int_vector.rs`, `crates/fr-geometry/src/int_direction.rs`
- Modify: `lib.rs`
- Java: `IntVector.java`, `IntDirection.java`, and the `IntDirection`-relevant parts of `Direction.java` (constants, `turn45Degree`, `opposite`, `compareFrom`, `middleApprox`, `angleApprox`, `toString`)

**Interfaces:**
- Produces:
  - `IntVector { pub x: i32, pub y: i32 }` — `new`, `ZERO`, `is_zero`, `negate`, `is_orthogonal`, `is_diagonal`, `is_multiple_of_45_degree`, `determinant(&IntVector) -> i64`, `turn_90_degree(i32)`, `mirror_at_x_axis`, `mirror_at_y_axis`, `add(&IntVector) -> IntVector`, `side_of(&IntVector) -> Side`, `projection(&IntVector) -> Signum`, `scalar_product(&IntVector) -> f64`, `to_float() -> FloatPoint` (**defer** until Task 9 — add then), `length_approx() -> f64`, `cos_angle`, `angle_approx_to(&IntVector) -> f64`, `angle_approx() -> f64`, `to_normalized_direction() -> IntDirection`
  - `IntDirection { pub x: i32, pub y: i32 }` — consts `NULL, RIGHT, RIGHT45, UP, UP45, LEFT, LEFT45, DOWN, DOWN45`; `get_vector() -> IntVector`, `is_orthogonal`, `is_diagonal`, `is_multiple_of_45_degree`, `turn_45_degree(i32)`, `opposite`, `determinant(&IntDirection) -> i64`, `side_of(&IntDirection) -> Side`, `projection(&IntDirection) -> Signum`, `compare_from(&IntDirection, &IntDirection) -> Ordering`, `middle_approx(&IntDirection) -> IntDirection` (needs FloatPoint → defer body to Task 9 with `todo!()` **not allowed**: instead implement using plain `f64` math inline — `size = (x²+y²).sqrt()`), `angle_approx() -> f64`; `impl Ord` = Java `compareTo` angular order; `impl Display` = Java `toString`.
  - `IntVector::new(x, y)` does **not** check `CRIT_INT` (Java doesn't); promotion happens in `Vector::new` (Task 8).

- [ ] **Step 1: Write failing tests**

`int_direction.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Side;
    use std::cmp::Ordering;

    #[test]
    fn angular_order_is_counterclockwise_from_right() {
        let order = [IntDirection::RIGHT, IntDirection::RIGHT45, IntDirection::UP, IntDirection::UP45,
                     IntDirection::LEFT, IntDirection::LEFT45, IntDirection::DOWN, IntDirection::DOWN45];
        for w in order.windows(2) {
            assert_eq!(w[0].cmp(&w[1]), Ordering::Less, "{} < {}", w[0], w[1]);
        }
        assert_eq!(IntDirection::DOWN45.cmp(&IntDirection::RIGHT), Ordering::Greater);
        assert_eq!(IntDirection::new(3, 1).cmp(&IntDirection::new(1, 3)), Ordering::Less);
    }

    #[test]
    fn turn_45_and_opposite() {
        assert_eq!(IntDirection::RIGHT.turn_45_degree(1), IntDirection::RIGHT45);
        assert_eq!(IntDirection::RIGHT.turn_45_degree(2), IntDirection::UP);
        assert_eq!(IntDirection::RIGHT.turn_45_degree(6), IntDirection::DOWN);
        assert_eq!(IntDirection::new(2, 1).turn_45_degree(1), IntDirection::new(1, 3));
        assert_eq!(IntDirection::UP.opposite(), IntDirection::DOWN);
        assert_eq!(IntDirection::new(1, -3).turn_45_degree(-1), IntDirection::new(1, -3).turn_45_degree(7));
    }

    #[test]
    fn side_and_projection() {
        assert_eq!(IntDirection::RIGHT.side_of(&IntDirection::UP), Side::OnTheLeft);
        assert_eq!(IntDirection::UP.side_of(&IntDirection::RIGHT), Side::OnTheRight);
        assert_eq!(IntDirection::RIGHT.side_of(&IntDirection::LEFT), Side::Collinear);
        assert_eq!(IntDirection::RIGHT.projection(&IntDirection::RIGHT45), crate::Signum::Positive);
    }

    #[test]
    fn compare_from_orders_relative_to_self() {
        // From RIGHT45, UP comes before RIGHT (RIGHT wraps to the end).
        assert_eq!(IntDirection::RIGHT45.compare_from(&IntDirection::UP, &IntDirection::RIGHT), Ordering::Less);
        assert_eq!(IntDirection::RIGHT45.compare_from(&IntDirection::RIGHT, &IntDirection::UP), Ordering::Greater);
    }

    #[test]
    fn display_names() {
        assert_eq!(IntDirection::UP45.to_string(), "UP-LEFT");
        assert_eq!(IntDirection::new(2, 2).to_string(), "UP-RIGHT"); // equal under angular compare
        assert_eq!(IntDirection::new(5, 1).to_string(), "UNKNOWN");
    }

    #[test]
    fn angle_approx_of_up_is_half_pi() {
        assert!((IntDirection::UP.angle_approx() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }
}
```
Note the Java quirk in `turn45Degree`: `n = factor % 8` can be negative for negative factors and then falls to `default → (0,0)`. The test `turn_45_degree(-1)` asserts the *Java* behaviour is reproduced: Java returns `IntDirection(0,0)` for `-1`, so the last assertion in `turn_45_and_opposite` must be `assert_eq!(IntDirection::new(1,-3).turn_45_degree(-1), IntDirection::NULL)`. Fix the test to that; do **not** "improve" the modulo.

`int_vector.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Side, Signum};

    #[test]
    fn basics() {
        let v = IntVector::new(3, -4);
        assert!(!v.is_zero());
        assert_eq!(v.negate(), IntVector::new(-3, 4));
        assert!(IntVector::new(0, 5).is_orthogonal());
        assert!(IntVector::new(-5, 5).is_diagonal());
        assert!(!v.is_multiple_of_45_degree());
        assert_eq!(v.determinant(&IntVector::new(1, 2)), 3 * 2 - (-4) * 1);
        assert_eq!(v.add(&IntVector::new(1, 1)), IntVector::new(4, -3));
    }

    #[test]
    fn turns_and_mirrors() {
        let v = IntVector::new(1, 2);
        assert_eq!(v.turn_90_degree(1), IntVector::new(-2, 1));
        assert_eq!(v.turn_90_degree(-1), IntVector::new(2, -1));
        assert_eq!(v.turn_90_degree(5), v.turn_90_degree(1));
        assert_eq!(v.mirror_at_x_axis(), IntVector::new(1, -2));
        assert_eq!(v.mirror_at_y_axis(), IntVector::new(-1, 2));
    }

    #[test]
    fn side_projection_scalar() {
        let right = IntVector::new(1, 0);
        let up = IntVector::new(0, 1);
        assert_eq!(right.side_of(&up), Side::OnTheLeft);
        assert_eq!(up.side_of(&right), Side::OnTheRight);
        assert_eq!(right.projection(&IntVector::new(-1, 7)), Signum::Negative);
        assert_eq!(right.scalar_product(&IntVector::new(4, 9)), 4.0);
    }

    #[test]
    fn normalized_direction_divides_by_gcd() {
        assert_eq!(IntVector::new(6, -4).to_normalized_direction(), IntDirection::new(3, -2));
        assert_eq!(IntVector::new(0, -8).to_normalized_direction(), IntDirection::DOWN);
    }

    #[test]
    fn large_determinant_does_not_overflow() {
        let a = IntVector::new(crate::CRIT_INT, crate::CRIT_INT);
        let b = IntVector::new(-crate::CRIT_INT, crate::CRIT_INT);
        assert_eq!(a.determinant(&b), 2_i64 * (crate::CRIT_INT as i64) * (crate::CRIT_INT as i64));
        assert_eq!(a.side_of(&b), Side::OnTheLeft);
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p fr-geometry int_`

- [ ] **Step 3: Implement both files**

Key translations:
- `IntVector::side_of(other)`: Java `sideOf(IntVector other)` computes `(double) other.x * y - (double) other.y * x` then `Side.of` — write `Side::of_i64(other.x as i64 * self.y as i64 - other.y as i64 * self.x as i64)`. Note the *operand order* (it is `other × self`), and that the public `sideOf(Vector)` in Java negates the result of `other.sideOf(this)` — the two negations cancel, so the public `side_of` is exactly the formula above. Test `right.side_of(&up) == OnTheLeft` pins this down: `other=(0,1), self=(1,0)`: `0*0 - 1*1 = -1` → OnTheRight?! Recompute: `other.x*self.y - other.y*self.x = 0*0 - 1*1 = -1` → `OnTheRight`. But geometrically "up is on the left of right"… Java's public `sideOf(Vector other)` returns `other.sideOf(this).negate()` where the inner package-private `sideOf(IntVector)` is called with the roles swapped, so: `up.sideOf_pkg(right) = right.x*up.y - right.y*up.x = 1*1 - 0*0 = 1 → OnTheLeft`, negated → `OnTheRight`. So Java says `right.sideOf(up) == ON_THE_RIGHT`. **Java wins**: flip the two expectations in `side_projection_scalar` and in `int_direction::side_and_projection` accordingly (`RIGHT.side_of(UP) == OnTheRight`, `UP.side_of(RIGHT) == OnTheLeft`). Implement the public method as `Side::of_i64(self.x as i64 * other.y as i64 - self.y as i64 * other.x as i64).negate()` — i.e. literally the Java double-dispatch collapsed — and leave a comment explaining the sign convention (a point is `ON_THE_LEFT` of a directed line when it lies to the left walking along it; `Vector.sideOf` is defined so that `Point.sideOf(Line)` works out). Do the same care for `IntDirection::side_of`, which is `self.get_vector().side_of(&other.get_vector())`.
- `IntDirection` `Ord`: port `compareTo(IntDirection)` verbatim (the nested `y > 0 / y < 0 / y == 0` cases), with the final `determinant` in `i64`. `PartialOrd` delegates to `Ord`. **`Eq` must agree with `Ord`**: Java's `equals` is angular (collinear + same sense), so `IntDirection::new(2,2) == RIGHT45` is `true`. Implement `PartialEq` manually as `self.cmp(other) == Ordering::Equal` and `Hash` on the normalised (gcd-divided) pair so `Eq`/`Hash` stay consistent.
- `middle_approx`, `angle_approx`: inline `f64` (no `FloatPoint` yet): `let (l1, l2) = (hypot(x,y), hypot(ox,oy)); let x = x/l1 + ox/l2; …; IntVector::new(java_round(x*1000.0) as i32, java_round(y*1000.0) as i32).to_normalized_direction()`.
- `to_float()` on `IntVector`: **add in Task 9** when `FloatPoint` exists; `length_approx`, `cos_angle`, `angle_approx*` can be written now with `hypot`.

- [ ] **Step 4: Run tests, lint**

Run: `cargo test -p fr-geometry && cargo clippy -p fr-geometry --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(geometry): IntVector and IntDirection with Java angular ordering"
```

---

### Task 7: `IntPoint`

**Files:**
- Create: `crates/fr-geometry/src/int_point.rs`
- Modify: `lib.rs`
- Java: `IntPoint.java`; the `IntPoint`-relevant defaults of `Point.java` (`sideOf(p1,p2)`, `compareXY`, `turn90Degree`, `mirrorVertical/Horizontal`, `perpendicularDirection`)

**Interfaces:**
- Produces: `IntPoint { pub x: i32, pub y: i32 }` — `new`, `ZERO`, `translate_by(&IntVector) -> IntPoint`, `difference_by(&IntPoint) -> IntVector`, `determinant(&IntPoint) -> i64`, `signed_area(&IntPoint, &IntPoint) -> i64` (Java returns the `long` determinant as `double`; keep `i64`), `distance_square(&IntPoint) -> f64`, `distance(&IntPoint) -> f64`, `orthogonal_projection(&IntPoint) -> IntPoint`, `fortyfive_degree_projection(&IntPoint) -> IntPoint`, `fortyfive_degree_corner(&IntPoint, left_turn: bool) -> Option<IntPoint>`, `ninety_degree_corner(&IntPoint, bool) -> Option<IntPoint>`, `compare_x/compare_y/compare_xy(&IntPoint) -> Ordering`, `side_of(&IntPoint, &IntPoint) -> Side`, `turn_90_degree(factor, pole: &IntPoint) -> IntPoint`, `mirror_vertical(pole)`, `mirror_horizontal(pole)`, `impl Display` → `(x,y)`.
- Deferred to later tasks (they need types not yet defined): `surrounding_box` (Task 11), `surrounding_octagon` (Task 12), `is_contained_in(&IntBox)` (Task 11), `side_of_line(&Line)` and `perpendicular_projection(&Line) -> Point` and `perpendicular_direction(&Line)` (Task 10), `to_float` (Task 9). Each of those tasks adds an `impl IntPoint { … }` block in *its own* file (Rust allows inherent impls across modules of one crate).

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_vector::IntVector;
    use crate::Side;
    use std::cmp::Ordering;

    #[test]
    fn translate_and_difference() {
        let p = IntPoint::new(2, 3);
        assert_eq!(p.translate_by(&IntVector::new(-5, 1)), IntPoint::new(-3, 4));
        assert_eq!(IntPoint::new(7, 7).difference_by(&p), IntVector::new(5, 4));
        assert_eq!(p.to_string(), "(2,3)");
    }

    #[test]
    fn determinant_and_area() {
        assert_eq!(IntPoint::new(1, 2).determinant(&IntPoint::new(3, 4)), -2);
        // signed_area of (0,0) relative to p1=(0,0)... use a real triangle:
        // p=(0,1), p1=(0,0), p2=(1,0): d21=(1,0), d01=(0,1): det = 1*1 - 0*0 = 1
        assert_eq!(IntPoint::new(0, 1).signed_area(&IntPoint::new(0, 0), &IntPoint::new(1, 0)), 1);
        assert_eq!(IntPoint::new(0, 0).distance_square(&IntPoint::new(3, 4)), 25.0);
        assert_eq!(IntPoint::new(0, 0).distance(&IntPoint::new(3, 4)), 5.0);
    }

    #[test]
    fn projections() {
        // horizontal distance 2 <= vertical 5 → snap x to other
        assert_eq!(IntPoint::new(0, 0).orthogonal_projection(&IntPoint::new(2, 5)), IntPoint::new(2, 0));
        assert_eq!(IntPoint::new(0, 0).orthogonal_projection(&IntPoint::new(5, 2)), IntPoint::new(0, 2));
        // 45°: from (0,0) to (10,1): dx=-10, dy=-1 → distArr=[10,1,4.5,5.5] → min=dy → (this.x, other.y)
        assert_eq!(IntPoint::new(0, 0).fortyfive_degree_projection(&IntPoint::new(10, 1)), IntPoint::new(0, 1));
    }

    #[test]
    fn corners() {
        // (0,0)->(10,4): dy>0 && dy<dx → left: (to.x - dy, this.y) = (6,0); right: (this.x+dy, to.y) = (4,4)
        let a = IntPoint::new(0, 0);
        let b = IntPoint::new(10, 4);
        assert_eq!(a.fortyfive_degree_corner(&b, true), Some(IntPoint::new(6, 0)));
        assert_eq!(a.fortyfive_degree_corner(&b, false), Some(IntPoint::new(4, 4)));
        assert_eq!(a.fortyfive_degree_corner(&IntPoint::new(5, 5), true), None); // already diagonal
        assert_eq!(a.ninety_degree_corner(&b, true), Some(IntPoint::new(10, 0)));
        assert_eq!(a.ninety_degree_corner(&b, false), Some(IntPoint::new(0, 4)));
        assert_eq!(a.ninety_degree_corner(&IntPoint::new(0, 9), true), None);
    }

    #[test]
    fn comparisons_and_side() {
        assert_eq!(IntPoint::new(1, 9).compare_xy(&IntPoint::new(2, 0)), Ordering::Less);
        assert_eq!(IntPoint::new(1, 9).compare_xy(&IntPoint::new(1, 0)), Ordering::Greater);
        // Point.sideOf(p1, p2): v1 = this - p1, v2 = p2 - p1, v1.sideOf(v2)
        let s = IntPoint::new(0, 1).side_of(&IntPoint::new(0, 0), &IntPoint::new(1, 0));
        assert!(matches!(s, Side::OnTheLeft | Side::OnTheRight)); // pinned precisely in Step 3
    }

    #[test]
    fn turn_and_mirror_about_pole() {
        let pole = IntPoint::new(1, 1);
        assert_eq!(IntPoint::new(2, 1).turn_90_degree(1, &pole), IntPoint::new(1, 2));
        assert_eq!(IntPoint::new(3, 5).mirror_vertical(&pole), IntPoint::new(-1, 5));
        assert_eq!(IntPoint::new(3, 5).mirror_horizontal(&pole), IntPoint::new(3, -3));
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p fr-geometry int_point`

- [ ] **Step 3: Implement, then pin the `side_of` expectation**

After porting, compute `IntPoint::new(0,1).side_of(&(0,0), &(1,0))` with the convention established in Task 6 (`v1=(0,1)`, `v2=(1,0)`, `v1.side_of(v2)`), replace the `matches!` in the test with the exact `Side`, and add a comment `// Java: IntVector(0,1).sideOf(IntVector(1,0))`.

`fortyfive_degree_corner`/`ninety_degree_corner`: port the if-chains verbatim; the final `else → null` becomes `None`.

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): IntPoint"
```

---

### Task 8: Rational types and the `Point` / `Vector` / `Direction` enums

**Files:**
- Create: `crates/fr-geometry/src/{rational_vector.rs,rational_point.rs,bigint_direction.rs,point.rs,vector.rs,direction.rs}`
- Modify: `lib.rs`
- Java: `RationalVector.java`, `RationalPoint.java`, `BigIntDirection.java`, `Vector.java`, `Point.java`, `Direction.java`

**Interfaces:**
- Produces:
  - `RationalVector { x: BigInt, y: BigInt, z: BigInt }` (z > 0 invariant kept as in Java) — `from_int(&IntVector)`, `new(x,y,z)`, `is_zero`, `negate`, `add_int(&IntVector) -> Vector`, `add_rational(&RationalVector) -> Vector`, `side_of_int`, `side_of_rational`, `projection_*`, `scalar_product_*`, `is_orthogonal`, `is_diagonal`, `turn_90_degree`, `mirror_at_x_axis`, `mirror_at_y_axis`, `to_normalized_direction() -> Direction`, `determinant(&RationalVector) -> BigInt`
  - `RationalPoint { x, y, z: BigInt }` — `from_int(&IntPoint)`, `new(x,y,z)`, `is_infinite`, `translate_by_int`, `translate_by_rational`, `difference_by_int`, `difference_by_rational`, `compare_x/y` vs both representations; `impl PartialEq` by cross-multiplication (`x1*z2 == x2*z1 && y1*z2 == y2*z1`; all infinite points equal); `impl Hash` consistent with that (hash the reduced form)
  - `BigIntDirection { x: BigInt, y: BigInt }` — `from_rational_vector`, `get_vector() -> Vector`, `is_orthogonal`, `is_diagonal`, `turn_45_degree`, `opposite`, `compare_to_int(&IntDirection) -> Ordering`, `compare_to_big(&BigIntDirection) -> Ordering`
  - `enum Vector { Int(IntVector), Rational(RationalVector) }` — `Vector::new(x: i32, y: i32)` (Java `getInstance(int,int)`: promotes when `|x|>CRIT_INT || |y|>CRIT_INT`), `Vector::from_big(x, y, z: BigInt)` (Java `getInstance(BigInteger×3)`: normalises sign of z, reduces when divisible, demotes to `Int` when it fits), `ZERO`, and every abstract method of `Vector.java` dispatched by `match`: `is_zero, negate, add, side_of, is_orthogonal, is_diagonal, is_multiple_of_45_degree, projection, scalar_product, turn_90_degree, mirror_at_x_axis, mirror_at_y_axis, length_approx, cos_angle, angle_approx, change_length_approx, to_normalized_direction, add_to(&Point) -> Point`. `to_float` added in Task 9.
  - `enum Point { Int(IntPoint), Rational(RationalPoint) }` — `Point::new(x, y)`, `Point::from_big(x,y,z)`, `ZERO`, `translate_by(&Vector) -> Point`, `difference_by(&Point) -> Vector`, `is_infinite`, `compare_x/compare_y/compare_xy -> Ordering`, `side_of(&Point, &Point) -> Side`, `turn_90_degree(i32, &Point) -> Point`, `mirror_vertical/mirror_horizontal(&Point) -> Point`. `impl From<IntPoint> for Point`. Deferred (need `Line`/`IntBox`/`IntOctagon`): `side_of_line`, `perpendicular_projection`, `perpendicular_direction`, `surrounding_box`, `surrounding_octagon`, `is_contained_in`, `to_float` — added in Tasks 9–12.
  - `enum Direction { Int(IntDirection), Big(BigIntDirection) }` — `Direction::from_vector(&Vector)`, `Direction::between(&Point, &Point) -> Option<Direction>` (Java `getInstance(from,to)`, `null` when equal), `Direction::from_angle_approx(f64)`, `get_vector() -> Vector`, `is_orthogonal`, `is_diagonal`, `is_multiple_of_45_degree`, `turn_45_degree`, `opposite`, `side_of`, `projection`, `middle_approx`, `compare_from`, `angle_approx`; `impl Ord` (dispatch on the four pairings); `impl PartialEq` = `cmp == Equal` (Java `equals` semantics). `impl From<IntDirection> for Direction`.

- [ ] **Step 1: Write failing tests**

`point.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::int_vector::IntVector;
    use crate::vector::Vector;
    use crate::CRIT_INT;
    use num_bigint::BigInt;

    #[test]
    fn new_promotes_beyond_crit_int() {
        assert!(matches!(Point::new(5, 5), Point::Int(_)));
        assert!(matches!(Point::new(CRIT_INT + 1, 0), Point::Rational(_)));
        assert!(matches!(Point::new(CRIT_INT, 0), Point::Int(_)));
    }

    #[test]
    fn from_big_reduces_and_demotes() {
        let p = Point::from_big(BigInt::from(100), BigInt::from(200), BigInt::from(50));
        assert_eq!(p, Point::Int(IntPoint::new(2, 4)));
        let p = Point::from_big(BigInt::from(-6), BigInt::from(9), BigInt::from(-3));
        assert_eq!(p, Point::Int(IntPoint::new(2, -3)));
        let p = Point::from_big(BigInt::from(3), BigInt::from(3), BigInt::from(2));
        assert!(matches!(p, Point::Rational(_)));
    }

    #[test]
    fn rational_equality_is_by_value() {
        let a = Point::Rational(RationalPoint::new(BigInt::from(100), BigInt::from(200), BigInt::from(50)));
        let b = Point::Rational(RationalPoint::new(BigInt::from(2), BigInt::from(4), BigInt::from(1)));
        assert_eq!(a, b);
        let inf1 = RationalPoint::new(BigInt::from(10), BigInt::from(20), BigInt::from(0));
        let inf2 = RationalPoint::new(BigInt::from(30), BigInt::from(40), BigInt::from(0));
        assert_eq!(inf1, inf2);
        assert!(inf1.is_infinite());
    }

    #[test]
    fn translate_and_difference_mix_representations() {
        let half = Point::from_big(BigInt::from(1), BigInt::from(1), BigInt::from(2)); // (0.5, 0.5)
        let moved = half.translate_by(&Vector::Int(IntVector::new(1, 1)));
        assert_eq!(moved, Point::from_big(BigInt::from(3), BigInt::from(3), BigInt::from(2)));
        let d = moved.difference_by(&half);
        assert_eq!(d, Vector::Int(IntVector::new(1, 1))); // reduced back to Int
        let big = Point::new(CRIT_INT + 1, 0);
        assert_eq!(big.compare_x(&Point::new(CRIT_INT, 0)), std::cmp::Ordering::Greater);
    }
}
```
`direction.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_direction::IntDirection;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::vector::Vector;

    #[test]
    fn from_vector_normalises_and_between_is_none_for_equal_points() {
        assert_eq!(Direction::from_vector(&Vector::new(4, 0)), Direction::Int(IntDirection::RIGHT));
        assert_eq!(Direction::from_vector(&Vector::new(-6, 6)), Direction::Int(IntDirection::UP45));
        let p = Point::Int(IntPoint::new(1, 1));
        assert!(Direction::between(&p, &p).is_none());
        assert_eq!(Direction::between(&p, &Point::Int(IntPoint::new(1, 9))), Some(Direction::Int(IntDirection::UP)));
    }

    #[test]
    fn from_angle_approx() {
        assert_eq!(Direction::from_angle_approx(0.0), Direction::Int(IntDirection::RIGHT));
        assert_eq!(Direction::from_angle_approx(std::f64::consts::FRAC_PI_2), Direction::Int(IntDirection::UP));
        assert_eq!(Direction::from_angle_approx(std::f64::consts::FRAC_PI_4), Direction::Int(IntDirection::RIGHT45));
    }

    #[test]
    fn big_direction_orders_against_int() {
        let big = Direction::from_vector(&Vector::new(crate::CRIT_INT + 5, crate::CRIT_INT + 5)); // ≈ RIGHT45
        assert!(matches!(big, Direction::Big(_)));
        assert!(Direction::Int(IntDirection::RIGHT) < big);
        assert!(big < Direction::Int(IntDirection::UP));
    }
}
```
Careful: `Vector::new(CRIT_INT+5, CRIT_INT+5)` is a `RationalVector` (promoted), and its `to_normalized_direction` yields a `BigIntDirection` — check `RationalVector.toNormalizedDirection` in Java to confirm it does not reduce to an `IntDirection`; if it does reduce (gcd → (1,1)), change the test vector to `(CRIT_INT + 5, CRIT_INT + 7)` so it cannot.

- [ ] **Step 2: Run to verify failure** — `cargo test -p fr-geometry point direction`

- [ ] **Step 3: Implement the six files**

Guidance:
- Java's double dispatch (`add(Vector)` → `other.add(this)` → concrete overload) collapses into one `match (self, other)` with four arms; the Rational arms call the `_rational` helpers, the mixed arms convert the `Int` side with `RationalVector::from_int` (Java does exactly this in `RationalVector.add(IntVector)`).
- `Vector::from_big` / `Point::from_big`: copy the Java logic literally, including "if `x mod z == 0` then divide **both** x and y by z" — note Java only checks `x.mod(z)`; it then divides `y` too even if `y mod z != 0`. **Port the quirk** (it is what Java does; a comment must say so). Then the `Int` demotion checks `|x| <= CRIT_INT && |y| <= CRIT_INT`.
- `RationalPoint::new` in Java takes `(x, y, z)` without sign normalisation; `is_infinite` is `z == 0`; keep both.
- `RationalPoint` `compare_x` vs `IntPoint`: Java cross-multiplies (`this.x` vs `other.x * this.z`) — keep sign handling exactly (z is positive by construction via `from_big`, but a raw `new` may have negative z; follow Java line for line).
- `BigIntDirection::compare_to_int`: port `BigIntDirection.compareTo(IntDirection)` from Java (it mirrors the `IntDirection` nested cases with `BigInt` determinants). `Direction`'s `Ord` dispatches: `(Int,Int)` → Task 6's `Ord`; `(Int,Big)` → `-(big.compare_to_int(int))`; `(Big,Int)` → `big.compare_to_int(int)`; `(Big,Big)` → `compare_to_big`.
- `Direction::from_angle_approx`: `x = java_round(cos(angle) * 10000.0)`, `y = java_round(sin(angle) * 10000.0)`, `from_vector(&Vector::new(x as i32, y as i32))`.

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): rational point/vector/direction and Point/Vector/Direction enums"
```

---

### Task 9: `FloatPoint` and `FloatLine`

**Files:**
- Create: `crates/fr-geometry/src/float_point.rs`, `crates/fr-geometry/src/float_line.rs`
- Modify: `int_point.rs`, `int_vector.rs`, `point.rs`, `vector.rs`, `rational_point.rs` (add `to_float`), `lib.rs`
- Java: `FloatPoint.java`, `FloatLine.java`

**Interfaces:**
- Produces:
  - `FloatPoint { pub x: f64, pub y: f64 }` (`Copy, PartialEq, Debug`) — `new`, `ZERO`, `from_int(&IntPoint)`, `size() -> f64`, `distance(&FloatPoint)`, `distance_square`, `weighted_distance(&FloatPoint, hw, vw)`, `round() -> IntPoint` (uses `java_round`), `round_to_the_right(&Direction) -> IntPoint`, `round_to_the_left(&Direction) -> IntPoint`, `round_to_grid(h: i32, v: i32) -> IntPoint`, `add`, `subtract`, `scalar_product(&FloatPoint, &FloatPoint)`, `change_size(f64)`, `change_length(&FloatPoint, f64)`, `middle_point`, `side_of(&FloatPoint, &FloatPoint) -> Side`, `rotate(angle, pole)`, `turn_90_degree(i32)`, `turn_90_degree_pole(i32, &FloatPoint)`, `is_contained_in_box(&FloatPoint, &FloatPoint, tol)`, `tangential_points(&FloatPoint, dist) -> Option<[FloatPoint; 2]>` (Java returns `null` when `toPoint` is inside the circle), `left_tangential_point`, `right_tangential_point` (both `Option`), `circle_center(&FloatPoint, &FloatPoint) -> Option<FloatPoint>` (null when collinear), `inside_circle(p1,p2,p3) -> bool`, `impl Display` (Java `toString(Locale)` with 2 fraction digits — match `toString()` exactly: check the Java for the default locale/format and reproduce it, e.g. `(1.50, -2.00)` vs `(1.5, -2)`; write a test once you've read it). `bounding_box() -> IntBox` and `projection_approx(&Line)` are added in Tasks 11 and 10.
  - `FloatLine { pub a: FloatPoint, pub b: FloatPoint }` — `new`, `opposite`, `adjust_direction(&FloatLine)`, `intersection(&FloatLine) -> Option<FloatPoint>` (Java returns a point with `Integer.MAX_VALUE` coordinates when parallel — check and reproduce: if Java returns a sentinel, return the same sentinel `FloatPoint`, not `None`), `translate(f64)`, `signed_distance(&FloatPoint)`, `perpendicular_projection(&FloatPoint)`, `segment_distance(&FloatPoint)`, `segment_projection(&FloatLine) -> Option<FloatLine>`, `segment_projection_2(&FloatLine) -> Option<FloatLine>`, `shrink_segment(f64)`, `nearest_segment_point(&FloatPoint)`, `divide_segment_into_sections(count) -> Vec<FloatLine>`.
  - `IntVector::to_float`, `IntPoint::to_float`, `RationalPoint::to_float` (x/z, y/z as f64), `Point::to_float`, `Vector::to_float`, `Vector::change_length_approx(f64) -> Vector` (Java: `to_float().change_size(len).round().difference_by(ZERO)`).

- [ ] **Step 1: Write failing tests**

`float_point.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::int_direction::IntDirection;
    use crate::direction::Direction;

    #[test]
    fn round_uses_java_semantics() {
        assert_eq!(FloatPoint::new(1.5, -1.5).round(), IntPoint::new(2, -1));
        assert_eq!(FloatPoint::new(2.4, 2.6).round(), IntPoint::new(2, 3));
    }

    #[test]
    fn round_to_the_right_of_direction() {
        // Java: for dir UP (x==0, y>0): x is ceil'd, y is Math.round'd.
        let p = FloatPoint::new(1.2, 3.7);
        let r = p.round_to_the_right(&Direction::Int(IntDirection::UP));
        assert_eq!(r, IntPoint::new(2, 4));
        let l = p.round_to_the_left(&Direction::Int(IntDirection::UP));
        assert_eq!(l, IntPoint::new(1, 4));
    }

    #[test]
    fn round_to_grid() {
        assert_eq!(FloatPoint::new(17.0, 26.0).round_to_grid(10, 10), IntPoint::new(20, 30));
        assert_eq!(FloatPoint::new(17.0, 26.0).round_to_grid(0, 10), IntPoint::new(17, 30));
    }

    #[test]
    fn size_change_and_middle() {
        let p = FloatPoint::new(3.0, 4.0);
        assert_eq!(p.size(), 5.0);
        let q = p.change_size(10.0);
        assert!((q.x - 6.0).abs() < 1e-12 && (q.y - 8.0).abs() < 1e-12);
        assert_eq!(FloatPoint::new(0.0, 0.0).middle_point(&p), FloatPoint::new(1.5, 2.0));
    }

    #[test]
    fn circle_center_and_inside_circle() {
        let a = FloatPoint::new(0.0, 0.0);
        let b = FloatPoint::new(2.0, 0.0);
        let c = FloatPoint::new(0.0, 2.0);
        let center = a.circle_center(&b, &c).unwrap();
        assert!((center.x - 1.0).abs() < 1e-9 && (center.y - 1.0).abs() < 1e-9);
        assert!(a.circle_center(&b, &FloatPoint::new(4.0, 0.0)).is_none());
        assert!(FloatPoint::new(1.0, 1.0).inside_circle(&a, &b, &c));
        assert!(!FloatPoint::new(5.0, 5.0).inside_circle(&a, &b, &c));
    }

    #[test]
    fn tangential_points_none_when_inside() {
        let origin = FloatPoint::new(0.0, 0.0);
        assert!(origin.tangential_points(&FloatPoint::new(1.0, 0.0), 5.0).is_none());
        let t = origin.tangential_points(&FloatPoint::new(10.0, 0.0), 5.0).unwrap();
        for p in t {
            assert!((p.size() - 5.0).abs() < 1e-9);
        }
    }
}
```
`float_line.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intersection_and_projection() {
        let h = FloatLine::new(FloatPoint::new(0.0, 1.0), FloatPoint::new(10.0, 1.0));
        let v = FloatLine::new(FloatPoint::new(3.0, -5.0), FloatPoint::new(3.0, 5.0));
        let i = h.intersection(&v);
        assert_eq!(i.x, 3.0);
        assert_eq!(i.y, 1.0);
        let pr = h.perpendicular_projection(&FloatPoint::new(4.0, 9.0));
        assert_eq!(pr, FloatPoint::new(4.0, 1.0));
        assert!((h.signed_distance(&FloatPoint::new(4.0, 9.0)).abs() - 8.0).abs() < 1e-12);
    }
    #[test]
    fn segment_distance_clamps_to_endpoints() {
        let s = FloatLine::new(FloatPoint::new(0.0, 0.0), FloatPoint::new(10.0, 0.0));
        assert!((s.segment_distance(&FloatPoint::new(13.0, 4.0)) - 5.0).abs() < 1e-12);
        assert_eq!(s.nearest_segment_point(&FloatPoint::new(-3.0, 2.0)), FloatPoint::new(0.0, 0.0));
        let parts = s.divide_segment_into_sections(4);
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[1].a, FloatPoint::new(2.5, 0.0));
    }
}
```
The `intersection` test assumes the Java sentinel behaviour (returns a point, never null). Adjust `Option` vs value after reading `FloatLine.intersection` — the signature listed above says "reproduce Java".

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement**

`round_to_the_right/left(dir)`: Java branches on `dir.getVector()` components' signs — the `Direction` may be `Big`, so read `dir.get_vector()` and match on `Vector`; for `Rational` use the sign of `x`/`y` numerators (z>0). Port the six `ceil/floor/round` cases exactly.

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): FloatPoint and FloatLine, to_float conversions"
```

---

### Task 10: `Line`

**Files:**
- Create: `crates/fr-geometry/src/line.rs`
- Modify: `int_point.rs`, `point.rs`, `rational_point.rs`, `float_point.rs` (line-dependent methods), `lib.rs`
- Java: `Line.java`

**Interfaces:**
- Produces:
  - `Line { pub a: IntPoint, pub b: IntPoint }` (`Copy, Eq, Hash`) — Java stores `Point` but every arithmetic method casts to `IntPoint` and the constructor warns otherwise, so the port fixes `IntPoint`. `Line::new(a, b)`, `Line::from_coords(ax, ay, bx, by)`, `Line::from_direction(a: IntPoint, dir: &Direction) -> Line` (b = a + dir vector; **only** valid for `Direction::Int` — for `Big` directions Java would produce a non-int point; `debug_assert!` and fall back to the rational vector's float rounding? No: Java `Line(Point a, Direction dir)` calls `a.translateBy(dir.getVector())` which yields a `RationalPoint` and then *warns*. Since board code only builds lines from `IntDirection`, take `&IntDirection` in the Rust signature and add `from_direction_any(&Direction) -> Option<Line>` returning `None` for `Big`.)
  - `direction() -> IntDirection` (cached lazily in Java; just compute), `side_of(&Point) -> Side`, `side_of_float(&FloatPoint, tolerance: f64) -> Side`, `side_of_float_exact(&FloatPoint)`, `side_of_intersection(&Line, &Line) -> Side`, `is_on_the_left(&TileShape)`/`is_on_the_right` (**defer to Task 14**), `signed_distance(&FloatPoint) -> f64`, `overlaps(&Line) -> bool`, `opposite()`, `intersection(&Line) -> Point` (exact, ported verbatim including the axis-aligned/diagonal fast paths and the BigInt general case), `intersection_approx(&Line) -> FloatPoint` (Java sentinel `Integer.MAX_VALUE` when parallel — reproduce), `perpendicular_projection(&Point) -> Point`, `translate(f64) -> Line`, `translate_by(&Vector) -> Line` (`Int` vectors only; for `Rational` Java's `translateBy` — check and follow), `is_orthogonal`, `is_diagonal`, `is_multiple_of_45_degree`, `is_parallel`, `is_perpendicular`, `is_equal_or_opposite`, `cos_angle`, `compare_to(&Line) -> Ordering` (Java `compareTo`), `function_value_approx(x: f64) -> f64`, `function_in_y_value_approx(y: f64) -> f64`, `perpendicular_direction(&Point) -> Direction`, `turn_90_degree(i32, &IntPoint)`, `mirror_vertical`, `mirror_horizontal`, `length() -> f32` (Java returns `float`; keep `f32`).
  - Added to other files: `IntPoint::side_of_line(&Line) -> Side`, `IntPoint::perpendicular_projection(&Line) -> Point`, `RationalPoint::side_of_line`, `RationalPoint::perpendicular_projection`, `Point::side_of_line`, `Point::perpendicular_projection`, `Point::perpendicular_direction(&Line) -> Direction`, `FloatPoint::projection_approx(&Line) -> FloatPoint`.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::int_direction::IntDirection;
    use crate::point::Point;
    use crate::Side;
    use num_bigint::BigInt;

    fn l(ax: i32, ay: i32, bx: i32, by: i32) -> Line { Line::from_coords(ax, ay, bx, by) }

    #[test]
    fn axis_aligned_and_diagonal_fast_paths() {
        let vertical = l(5, 0, 5, 1);
        let horizontal = l(0, 3, 1, 3);
        assert_eq!(vertical.intersection(&horizontal), Point::Int(IntPoint::new(5, 3)));
        assert_eq!(horizontal.intersection(&vertical), Point::Int(IntPoint::new(5, 3)));
        let right_diag = l(0, 0, 1, 1);
        assert_eq!(vertical.intersection(&right_diag), Point::Int(IntPoint::new(5, 5)));
        let left_diag = l(0, 10, 1, 9);
        assert_eq!(horizontal.intersection(&left_diag), Point::Int(IntPoint::new(7, 3)));
    }

    #[test]
    fn general_intersection_exact() {
        assert_eq!(l(0, 0, 2, 2).intersection(&l(0, 2, 2, 0)), Point::Int(IntPoint::new(1, 1)));
        // (0,0)-(2,1) with (0,1)-(2,0) meet at (1, 0.5)
        let p = l(0, 0, 2, 1).intersection(&l(0, 1, 2, 0));
        assert_eq!(p, Point::from_big(BigInt::from(2), BigInt::from(1), BigInt::from(2)));
        // parallel lines: z == 0 → infinite point
        assert!(l(0, 0, 1, 1).intersection(&l(0, 1, 1, 2)).is_infinite());
    }

    #[test]
    fn side_of_and_direction() {
        let line = l(0, 0, 10, 0); // pointing RIGHT
        assert_eq!(line.direction(), IntDirection::RIGHT);
        let above = Point::Int(IntPoint::new(3, 4));
        let below = Point::Int(IntPoint::new(3, -4));
        assert_ne!(line.side_of(&above), line.side_of(&below));
        assert_eq!(line.side_of(&Point::Int(IntPoint::new(99, 0))), Side::Collinear);
        assert_eq!(line.side_of(&above), line.opposite().side_of(&below));
    }

    #[test]
    fn perpendicular_projection() {
        let diag = l(0, 0, 1, 1);
        assert_eq!(IntPoint::new(4, 0).perpendicular_projection(&diag), Point::Int(IntPoint::new(2, 2)));
        assert_eq!(
            IntPoint::new(3, 0).perpendicular_projection(&diag),
            Point::from_big(BigInt::from(3), BigInt::from(3), BigInt::from(2))
        );
    }

    #[test]
    fn predicates() {
        assert!(l(0, 0, 0, 5).is_orthogonal());
        assert!(l(0, 0, 3, -3).is_diagonal());
        assert!(l(0, 0, 2, 2).is_parallel(&l(5, 5, 9, 9)));
        assert!(l(0, 0, 2, 2).is_perpendicular(&l(0, 0, -1, 1)));
        assert!(l(0, 0, 2, 2).is_equal_or_opposite(&l(9, 9, 1, 1)));
        assert!(l(0, 0, 2, 2).overlaps(&l(9, 9, 1, 1)));
        assert!(!l(0, 0, 2, 2).overlaps(&l(0, 1, 2, 3)));
    }

    #[test]
    fn function_values() {
        let line = l(0, 0, 2, 4); // y = 2x
        assert_eq!(line.function_value_approx(3.0), 6.0);
        assert_eq!(line.function_in_y_value_approx(6.0), 3.0);
    }
}
```
Pin `side_of(&above)` to the exact `Side` after implementation (it follows the `IntPoint.sideOf(Line)` → `v1.sideOf(v2)` convention from Tasks 6–7) and replace `assert_ne!` with the two exact asserts.

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement** — port `Line.java` verbatim; `intersection` uses `i64` for `det1/det2/det` inputs (`IntPoint::determinant` is `i64`) and `BigInt` for the products, exactly as Java. `compare_to` follows Java's `compareTo` (direction first, then a point-side comparison — read it carefully; it defines the ordering used by `Simplex`).

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): Line with exact intersection and projections"
```

---

### Task 11: `IntBox`

**Files:**
- Create: `crates/fr-geometry/src/int_box.rs`
- Modify: `int_point.rs`, `rational_point.rs`, `point.rs`, `float_point.rs` (box-dependent methods), `lib.rs`
- Java: `IntBox.java`

**Interfaces:**
- Produces: `IntBox { pub ll: IntPoint, pub ur: IntPoint }` (`Copy, Eq, Hash`) — `new(ll, ur)`, `from_coords(llx, lly, urx, ury)`, `EMPTY` (Java `IntBox.EMPTY` — check its definition, likely `ll=(CRIT_INT,CRIT_INT), ur=(-CRIT_INT,-CRIT_INT)`), `is_empty`, `width`, `height`, `max_width`, `min_width`, `area`, `circumference`, `corner(i) -> IntPoint`, `dimension() -> i32`, `contains_inside(&IntPoint)`, `is_int_box()`, `is_int_octagon()`, `nearest_point(&FloatPoint) -> FloatPoint`, `nearest_border_projections(&IntPoint, max: usize) -> Vec<IntPoint>`, `distance(&FloatPoint)`, `weighted_distance(&IntBox, hw, vw)`, `bounding_box()`, `get_id() -> i32`, `is_bounded`, `corner_is_bounded(i)`, `union(&IntBox) -> IntBox`, `intersection(&IntBox) -> IntBox`, `intersects(&IntBox)`, `overlaps(&IntBox)`, `contains(&IntBox)`, `contains_in_interior(&IntBox)`, `is_contained_in(&IntBox)`, `translate_by(&Vector) -> IntBox` (Java: `Int` vectors translate; `Rational` → check), `turn_90_degree(i32, &IntPoint)`, `border_line(i) -> Line`, `border_line_index(&Line) -> Option<usize>` (Java `-1` → `None`), `offset(f64) -> IntBox`, `horizontal_offset(f64)`, `vertical_offset(f64)`, `shrink(i32)`, `compare(&IntBox, edge_index) -> Side`, `nearest_part(&IntBox) -> IntBox`, `divide_into_sections(f64) -> Vec<IntBox>`, `cutout_from(&IntBox) -> Vec<IntBox>`. Methods that return/consume `IntOctagon`, `Simplex`, `TileShape`, `RegularTileShape`, `Circle`, `ShapeBoundingDirections` are **added in Tasks 12–14** (they are `impl IntBox` blocks in those files).
- Added elsewhere: `IntPoint::surrounding_box`, `IntPoint::is_contained_in(&IntBox)`, `RationalPoint::surrounding_box`, `RationalPoint::is_contained_in`, `Point::surrounding_box`, `Point::is_contained_in`, `FloatPoint::bounding_box() -> IntBox`.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::float_point::FloatPoint;
    use crate::int_vector::IntVector;
    use crate::vector::Vector;

    fn b(a: i32, b_: i32, c: i32, d: i32) -> IntBox { IntBox::from_coords(a, b_, c, d) }

    #[test]
    fn metrics() {
        let x = b(0, 0, 10, 4);
        assert_eq!(x.width(), 10);
        assert_eq!(x.height(), 4);
        assert_eq!(x.area(), 40.0);
        assert_eq!(x.circumference(), 28.0);
        assert_eq!(x.max_width(), 10.0);
        assert_eq!(x.min_width(), 4.0);
        assert!(!x.is_empty());
        assert!(b(5, 5, 4, 9).is_empty());
        assert_eq!(x.dimension(), 2);
        assert_eq!(b(3, 3, 3, 3).dimension(), 0);
        assert_eq!(b(3, 3, 9, 3).dimension(), 1);
    }

    #[test]
    fn set_operations() {
        let x = b(0, 0, 10, 10);
        let y = b(5, 5, 20, 20);
        assert_eq!(x.intersection(&y), b(5, 5, 10, 10));
        assert_eq!(x.union(&y), b(0, 0, 20, 20));
        assert!(x.intersects(&y));
        assert!(x.intersects(&b(10, 10, 12, 12))); // touching counts as intersecting
        assert!(!x.overlaps(&b(10, 10, 12, 12))); // overlaps requires interior overlap
        assert!(x.contains(&b(1, 1, 2, 2)));
        assert!(!x.contains_in_interior(&b(0, 1, 2, 2)));
        assert!(b(1, 1, 2, 2).is_contained_in(&x));
        assert!(x.intersection(&b(50, 50, 60, 60)).is_empty());
    }

    #[test]
    fn corners_and_border_lines_are_counterclockwise() {
        let x = b(0, 0, 10, 4);
        let corners: Vec<IntPoint> = (0..4).map(|i| x.corner(i)).collect();
        assert!(corners.contains(&IntPoint::new(0, 0)));
        assert!(corners.contains(&IntPoint::new(10, 4)));
        // border_line(i) must run from corner(i) to corner(i+1) with the box on its left.
        for i in 0..4 {
            let line = x.border_line(i);
            assert_eq!(line.a, x.corner(i));
            assert_eq!(line.b, x.corner((i + 1) % 4));
            assert_eq!(x.border_line_index(&line), Some(i));
        }
        assert_eq!(x.border_line_index(&crate::line::Line::from_coords(0, 0, 1, 1)), None);
    }

    #[test]
    fn offsets_and_translation() {
        let x = b(0, 0, 10, 10);
        assert_eq!(x.offset(2.0), b(-2, -2, 12, 12));
        assert_eq!(x.offset(1.4), b(-1, -1, 11, 11)); // Java: Math.round
        assert_eq!(x.horizontal_offset(3.0), b(-3, 0, 13, 10));
        assert_eq!(x.shrink(2), b(2, 2, 8, 8));
        assert_eq!(x.shrink(50), b(5, 5, 5, 5)); // collapses to the centre
        assert_eq!(x.translate_by(&Vector::Int(IntVector::new(1, -1))), b(1, -1, 11, 9));
        assert_eq!(x.turn_90_degree(1, &IntPoint::new(0, 0)), b(-10, 0, 0, 10));
    }

    #[test]
    fn nearest_point_and_distance() {
        let x = b(0, 0, 10, 10);
        assert_eq!(x.nearest_point(&FloatPoint::new(-3.0, 4.0)), FloatPoint::new(0.0, 4.0));
        assert_eq!(x.nearest_point(&FloatPoint::new(13.0, 14.0)), FloatPoint::new(10.0, 10.0));
        assert_eq!(x.distance(&FloatPoint::new(13.0, 14.0)), 5.0);
        assert_eq!(x.distance(&FloatPoint::new(5.0, 5.0)), 0.0);
    }

    #[test]
    fn cutout_from_returns_surrounding_pieces() {
        let outer = b(0, 0, 10, 10);
        let inner = b(4, 4, 6, 6);
        let pieces = inner.cutout_from(&outer);
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert_eq!(total, outer.area() - inner.area());
        for p in &pieces {
            assert!(!p.overlaps(&inner));
            assert!(outer.contains(p));
        }
        assert!(b(20, 20, 30, 30).cutout_from(&outer).len() == 1);
    }

    #[test]
    fn divide_into_sections() {
        let x = b(0, 0, 100, 10);
        let parts = x.divide_into_sections(30.0);
        assert_eq!(parts.iter().map(|p| p.area()).sum::<f64>(), 1000.0);
        assert!(parts.iter().all(|p| p.width() <= 30 && p.height() <= 30));
    }

    #[test]
    fn point_helpers() {
        assert_eq!(IntPoint::new(3, 4).surrounding_box(), b(3, 4, 3, 4));
        assert!(IntPoint::new(3, 4).is_contained_in(&b(0, 0, 3, 4)));
        assert_eq!(FloatPoint::new(1.2, -1.2).bounding_box(), b(1, -2, 2, -1));
    }
}
```
The `cutout_from` piece-count expectation for the disjoint case, the `divide_into_sections` piece shape, and the exact `shrink(50)` centre (`(ll+ur)/2`) all follow the Java source — verify against it while porting; where the plan's guess is wrong, Java wins.

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement** — port `IntBox.java` verbatim (skip the `IntOctagon`/`Simplex`/`TileShape`-typed methods for now; list them at the bottom of the file in a `// added in Task 12/13/14:` comment so nothing is forgotten). `offset` uses `java_round`.

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): IntBox"
```

---

### Task 12: `IntOctagon`

**Files:**
- Create: `crates/fr-geometry/src/int_octagon.rs`
- Modify: `int_box.rs` (octagon-typed methods), `int_point.rs`, `rational_point.rs`, `point.rs` (`surrounding_octagon`), `lib.rs`
- Java: `IntOctagon.java` (1,722 lines — the largest single file; budget accordingly)

**Interfaces:**
- Produces: `IntOctagon { pub left_x, pub bottom_y, pub right_x, pub top_y, pub upper_left_diagonal_x, pub lower_right_diagonal_x, pub lower_left_diagonal_x, pub upper_right_diagonal_x: i32 }` (`Copy, Eq, Hash`), constructor argument order **exactly** Java's `(leftX, bottomY, rightX, topY, upperLeftDiagonalX, lowerRightDiagonalX, lowerLeftDiagonalX, upperRightDiagonalX)`; `EMPTY`; `is_empty`, `is_int_octagon` (true), `is_bounded`, `corner_is_bounded`, `bounding_box`, `bounding_octagon`, `dimension`, `corner(i) -> IntPoint`, `corner_x(i)`, `corner_y(i)`, `get_id`, `area`, `border_line_count() -> usize` (8), `border_line(i) -> Line`, `translate_by(&Vector)`, `max_width`, `min_width`, `offset(f64)`, `enlarge(f64)`, `contains_regular(&RegularTileShape)` (**Task 14**), `contains_float(&FloatPoint)`, `union(&IntOctagon)`, `union_box(&IntBox) -> IntOctagon`, `intersection(&IntOctagon)`, `intersection_box(&IntBox) -> IntOctagon`, `normalize() -> IntOctagon`, `is_normalized`, `side_of_border_line(x, y, line_no) -> Side`, `is_contained_in(&IntBox)`, `is_contained_in_octagon(&IntOctagon)`, `intersects_box(&IntBox)`, `intersects_octagon(&IntOctagon)`, `overlaps(&IntOctagon)`, `left_x_value(y)`, `right_x_value(y)`, `lower_y_value(x)`, `upper_y_value(x)`, `compare_octagon(&IntOctagon, edge) -> Side`, `compare_box(&IntBox, edge) -> Side`, `border_line_index(&Line) -> Option<usize>`, `border_point(&IntPoint, FortyfiveDegreeDirection) -> IntPoint` (**Task 14** defines the enum; put this method in `bounding_directions.rs`), `nearest_border_projections(&IntPoint, max) -> Vec<IntPoint>`, `border_line_side_of(&FloatPoint, line_index, tolerance) -> Side`, `is_int_box`, `cutout_from_box(&IntBox) -> Vec<IntOctagon>`, `cutout_from_octagon(&IntOctagon) -> Vec<IntOctagon>`, `impl Display`. `to_simplex`, `simplify`, `intersects(&Simplex)`, `intersects(&Circle)`, `bounding_shape` → Tasks 13–14.
- Added to `IntBox`: `is_int_octagon` (`true`), `to_int_octagon()`, `union_octagon(&IntOctagon) -> IntOctagon`, `intersection_octagon(&IntOctagon) -> IntOctagon`, `intersects_octagon(&IntOctagon)`, `is_contained_in_octagon(&IntOctagon)`, `enlarge(f64) -> IntOctagon`, `bounding_octagon()`, `compare_octagon(&IntOctagon, edge) -> Side`, `cutout_from_octagon(&IntOctagon) -> Vec<IntOctagon>`.
- Added elsewhere: `IntPoint::surrounding_octagon`, `RationalPoint::surrounding_octagon`, `Point::surrounding_octagon`.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::float_point::FloatPoint;

    /// Octagon from a box: diagonals are the extreme x-y / x+y values.
    fn from_box(b: IntBox) -> IntOctagon { b.to_int_octagon() }

    #[test]
    fn surrounding_octagon_of_point() {
        assert_eq!(IntPoint::new(3, 5).surrounding_octagon(), IntOctagon::new(3, 5, 3, 5, -2, -2, 8, 8));
    }

    #[test]
    fn box_conversion_and_normalize_roundtrip() {
        let b = IntBox::from_coords(0, 0, 10, 10);
        let o = from_box(b);
        assert_eq!(o.left_x, 0);
        assert_eq!(o.right_x, 10);
        assert_eq!(o.upper_left_diagonal_x, -10); // min of x - y
        assert_eq!(o.lower_right_diagonal_x, 10); // max of x - y
        assert_eq!(o.lower_left_diagonal_x, 0);   // min of x + y
        assert_eq!(o.upper_right_diagonal_x, 20); // max of x + y
        assert!(o.is_normalized());
        assert!(o.is_int_box());
        assert_eq!(o.bounding_box(), b);
        assert_eq!(o.area(), 100.0);
        assert_eq!(o.border_line_count(), 8);
    }

    #[test]
    fn normalize_tightens_loose_diagonals() {
        // A box-shaped octagon with slack diagonals must normalise to the tight ones.
        let loose = IntOctagon::new(0, 0, 10, 10, -50, 50, -50, 50);
        assert!(!loose.is_normalized());
        assert_eq!(loose.normalize(), from_box(IntBox::from_coords(0, 0, 10, 10)));
    }

    #[test]
    fn diamond_octagon() {
        // |x| + |y| <= 10 : left=-10,bottom=-10,right=10,top=10, x-y in [-10,10], x+y in [-10,10]
        let d = IntOctagon::new(-10, -10, 10, 10, -10, 10, -10, 10).normalize();
        assert!(d.contains_float(&FloatPoint::new(4.0, 4.0)));
        assert!(!d.contains_float(&FloatPoint::new(8.0, 8.0)));
        assert_eq!(d.area(), 200.0);
        assert!(!d.is_int_box());
        assert_eq!(d.left_x_value(0), -10);
        assert_eq!(d.left_x_value(5), -5);
        assert_eq!(d.upper_y_value(5), 5);
    }

    #[test]
    fn union_intersection_and_containment() {
        let a = from_box(IntBox::from_coords(0, 0, 10, 10));
        let b = from_box(IntBox::from_coords(5, 5, 20, 20));
        assert_eq!(a.intersection(&b), from_box(IntBox::from_coords(5, 5, 10, 10)));
        assert_eq!(a.union(&b).bounding_box(), IntBox::from_coords(0, 0, 20, 20));
        assert!(a.intersects_octagon(&b));
        assert!(a.intersects_box(&IntBox::from_coords(10, 10, 12, 12)));
        assert!(!a.overlaps(&from_box(IntBox::from_coords(10, 10, 12, 12))));
        assert!(from_box(IntBox::from_coords(1, 1, 2, 2)).is_contained_in_octagon(&a));
        assert!(a.intersection(&from_box(IntBox::from_coords(50, 50, 60, 60))).is_empty());
    }

    #[test]
    fn corners_and_border_lines_are_consistent() {
        let d = IntOctagon::new(-10, -10, 10, 10, -10, 10, -10, 10).normalize();
        for i in 0..8 {
            let line = d.border_line(i);
            assert_eq!(d.border_line_index(&line), Some(i));
            // every corner must be on or to the left of every border line
            for j in 0..8 {
                let c = crate::point::Point::Int(d.corner(j));
                assert_ne!(line.side_of(&c), crate::Side::OnTheRight, "corner {j} right of line {i}");
            }
        }
    }

    #[test]
    fn offset_and_translate() {
        let a = from_box(IntBox::from_coords(0, 0, 10, 10));
        let grown = a.offset(2.0);
        assert_eq!(grown.bounding_box(), IntBox::from_coords(-2, -2, 12, 12));
        assert!(a.is_contained_in_octagon(&grown));
        let moved = a.translate_by(&crate::vector::Vector::new(3, 4));
        assert_eq!(moved.bounding_box(), IntBox::from_coords(3, 4, 13, 14));
    }

    #[test]
    fn cutout_pieces_cover_difference() {
        let outer = from_box(IntBox::from_coords(0, 0, 20, 20));
        let inner = IntOctagon::new(5, 5, 15, 15, -5, 5, 15, 25).normalize(); // diamond-ish inside
        let pieces = inner.cutout_from_octagon(&outer);
        assert!(!pieces.is_empty());
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - (outer.area() - inner.intersection(&outer).area())).abs() < 1e-6);
        for p in &pieces {
            assert!(!p.overlaps(&inner));
            assert!(p.is_contained_in_octagon(&outer));
        }
    }
}
```
The side-of convention in `corners_and_border_lines_are_consistent` (shape on the *left* of its border lines) is the freerouting convention (`TileShape.contains` checks `side_of != ON_THE_RIGHT`); if the port shows the opposite, re-read `TileShape.contains(Point)` before changing anything.

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement** — port `IntOctagon.java` in source order. The two `cutoutFrom` methods (~600 lines of case analysis) must be transcribed literally; do not attempt to simplify them. `offset`/`enlarge` use `java_round` on `dist` and `dist * SQRT2` as Java does. `area()` in Java sums `double` products of `int` differences — keep `f64` there (it is an approximation API).

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): IntOctagon incl. normalize and cutout"
```

---

### Task 13: `Simplex`

**Files:**
- Create: `crates/fr-geometry/src/simplex.rs`
- Modify: `int_box.rs`, `int_octagon.rs` (simplex-typed methods), `lib.rs`
- Java: `Simplex.java`

**Interfaces:**
- Produces: `Simplex { lines: Vec<Line> }` (`Clone, PartialEq, Eq, Hash`; private field + `lines() -> &[Line]`) — `Simplex::new(lines: Vec<Line>)` (Java: `new Simplex(Line[])` — keeps lines as given; the *public* factories `Simplex.get_instance(Point[])` / `get_instance(Line[])` in Java normalise via `removeRedundantLines` — read `Simplex.java` and port whichever static factories exist as `Simplex::from_points(&[IntPoint])`, `Simplex::from_lines(Vec<Line>)`), `EMPTY`, `is_empty`, `simplify() -> TileShape` (**Task 14**), `get_id`, `corner_is_bounded(i)`, `is_bounded`, `border_line_count`, `corner(i) -> Point` (exact intersection of adjacent lines), `corner_approx(i) -> FloatPoint`, `corner_approx_arr() -> Vec<FloatPoint>`, `border_line(i) -> Line`, `dimension`, `max_width`, `min_width`, `is_int_box`, `is_int_octagon`, `to_int_octagon() -> IntOctagon`, `translate_by(&Vector)`, `bounding_box`, `bounding_octagon`, `bounding_tile() -> Simplex`, `offset(f64)`, `enlarge(f64)`, `index_of_right_most_corner(&Point)`, `intersection_box(&IntBox)`, `intersection_octagon(&IntOctagon)`, `intersection(&Simplex)`, `intersects(&Simplex)`, `intersects_box`, `intersects_octagon`, `border_line_index(&Line) -> Option<usize>`, `remove_border_line(i)`, `to_simplex() -> Simplex`, `cutout_from(&Simplex) -> Vec<Simplex>`, `cutout_from_octagon`, `cutout_from_box`, `remove_redundant_lines() -> Simplex`.
- Added to `IntBox`/`IntOctagon`: `to_simplex()`, `intersection_simplex(&Simplex) -> Simplex`, `intersects_simplex(&Simplex)`, `cutout_from_simplex(&Simplex) -> Vec<Simplex>`.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::line::Line;

    fn unit_box() -> IntBox { IntBox::from_coords(0, 0, 10, 10) }

    #[test]
    fn box_to_simplex_has_four_lines_and_same_corners() {
        let s = unit_box().to_simplex();
        assert_eq!(s.border_line_count(), 4);
        assert!(s.is_bounded());
        assert!(s.is_int_box());
        assert!(s.is_int_octagon());
        assert_eq!(s.bounding_box(), unit_box());
        let corners: Vec<Point> = (0..4).map(|i| s.corner(i)).collect();
        for c in [(0, 0), (10, 0), (10, 10), (0, 10)] {
            assert!(corners.contains(&Point::Int(IntPoint::new(c.0, c.1))));
        }
        assert_eq!(s.to_int_octagon(), unit_box().to_int_octagon());
    }

    #[test]
    fn triangle_from_points() {
        let t = Simplex::from_points(&[IntPoint::new(0, 0), IntPoint::new(10, 0), IntPoint::new(0, 10)]);
        assert_eq!(t.border_line_count(), 3);
        assert!(!t.is_int_box());
        assert_eq!(t.bounding_box(), unit_box());
        assert_eq!(t.dimension(), 2);
        assert!(t.corner_approx_arr().iter().any(|p| (p.x - 10.0).abs() < 1e-9 && p.y.abs() < 1e-9));
    }

    #[test]
    fn intersection_of_box_and_triangle() {
        let t = Simplex::from_points(&[IntPoint::new(0, 0), IntPoint::new(20, 0), IntPoint::new(0, 20)]);
        let i = t.intersection_box(&IntBox::from_coords(5, 5, 30, 30));
        assert!(!i.is_empty());
        // triangle (5,5),(15,5),(5,15)
        assert_eq!(i.bounding_box(), IntBox::from_coords(5, 5, 15, 15));
        assert_eq!(i.border_line_count(), 3);
        assert!(t.intersects_box(&IntBox::from_coords(5, 5, 30, 30)));
        assert!(!t.intersects_box(&IntBox::from_coords(15, 15, 30, 30)));
        assert!(t.intersection_box(&IntBox::from_coords(15, 15, 30, 30)).is_empty());
    }

    #[test]
    fn redundant_lines_are_removed() {
        let mut lines: Vec<Line> = unit_box().to_simplex().lines().to_vec();
        // an extra line far away that does not cut the box
        lines.push(Line::from_coords(0, 100, 10, 100));
        let s = Simplex::from_lines(lines);
        assert_eq!(s.border_line_count(), 4);
    }

    #[test]
    fn unbounded_simplex() {
        // half-plane to the left of the upward line x = 0
        let s = Simplex::from_lines(vec![Line::from_coords(0, 0, 0, 1)]);
        assert!(!s.is_bounded());
        assert_eq!(s.dimension(), 2);
        assert!(!s.corner_is_bounded(0));
    }

    #[test]
    fn cutout_from_box() {
        let outer = IntBox::from_coords(0, 0, 20, 20).to_simplex();
        let inner = Simplex::from_points(&[IntPoint::new(5, 5), IntPoint::new(15, 5), IntPoint::new(10, 15)]);
        let pieces = inner.cutout_from(&outer);
        assert!(pieces.len() >= 3);
        for p in &pieces {
            assert!(!p.intersection(&inner).dimension() == 2 || p.intersection(&inner).area() == 0.0);
        }
    }
}
```
The last assertion is clumsy: replace it with `assert!(p.intersection(&inner).dimension() < 2)` once `dimension` is ported. `area()` for `Simplex` comes from the shared `TileShape.area()` in Task 14 — if you need it here, temporarily compute via `corner_approx_arr` shoelace in the test.

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement** — port `Simplex.java`. Line ordering matters everywhere (corners are intersections of consecutive lines); `remove_redundant_lines` sorts by `Line::compare_to` (Task 10) — that is why `compare_to` had to be exact.

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): Simplex"
```

---

### Task 14: `TileShape` enum, `RegularTileShape`, bounding directions, and the shared `TileShape.java` algorithms

**Files:**
- Create: `crates/fr-geometry/src/{tile_shape.rs,regular_tile_shape.rs,bounding_directions.rs}`
- Modify: `int_box.rs`, `int_octagon.rs`, `simplex.rs` (`simplify`, `bounding_shape`, `intersection(&TileShape)`, `cutout(&TileShape)`), `line.rs` (`is_on_the_left/right(&TileShape)`), `lib.rs`
- Java: `TileShape.java`, `RegularTileShape.java`, `ShapeBoundingDirections.java`, `OrthogonalBoundingDirections.java`, `FortyfiveDegreeBoundingDirections.java`, `FortyfiveDegreeDirection.java`, and the `TileShape`-typed methods left over in `IntBox.java`/`IntOctagon.java`/`Simplex.java`

**Interfaces:**
- Produces:
  - `enum TileShape { Box(IntBox), Octagon(IntOctagon), Simplex(Simplex) }` with `From` impls from each variant, and every method of `TileShape.java` (both the abstract ones dispatched by `match` and the concrete shared algorithms ported once on the enum): `get_instance_from_points(&[IntPoint]) -> TileShape` (Java `TileShape.getInstance(Point[])` → `Simplex`), `get_instance_from_box`, `intersection_with_simplify(&TileShape)`, `intersection(&TileShape) -> TileShape`, `area`, `is_outside(&Point)`, `contains(&Point)`, `contains_float(&FloatPoint)`, `contains_float_tol(&FloatPoint, f64)`, `contains_tile(&TileShape)`, `contains_inside(&Point)`, `side_of_border(&FloatPoint, tol)`, `contains_on_border_line_no(&Point) -> Option<usize>`, `contains_on_border(&Point)`, `contains_approx(&TileShape)`, `distance(&FloatPoint)`, `border_distance(&FloatPoint)`, `smallest_radius`, `nearest_point(&Point) -> Point`, `nearest_point_approx(&FloatPoint)`, `nearest_border_point(&Point)`, `nearest_border_point_approx`, `nearest_border_points_approx(&FloatPoint, count) -> Vec<FloatPoint>`, `index_of_nearest_corner(&Point)`, `diagonal_corner_segment() -> FloatLine`, `nearest_relative_outside_locations(&TileShape, count)`, `shrink(f64) -> TileShape`, `length`, `touching_sides(&TileShape) -> Option<[usize; 2]>` (Java returns `null` or int[2]), `distance_to_the_left(&Line)`, `side_of_line(&Line) -> Side`, `turn_90_degree(i32, &IntPoint)`, `rotate_approx(f64, &FloatPoint)`, `mirror_vertical`, `mirror_horizontal`, `intersecting_border_line_no(&Point, &Direction) -> Option<usize>`, `cutout(&Polyline) -> Vec<Polyline>` (**Task 16** — Polyline needed; add there), `entrance_points(&Polyline)` (**Task 16**), `split_to_convex() -> Vec<TileShape>`, `divide_into_sections(f64) -> Vec<TileShape>`, `is_intersected_interior_by(&LineSegment)` (**Task 15**), `is_intersected_interior_by_points(&Point, &Point, &Line)`, `cutout_from(&TileShape) -> Vec<TileShape>` (dispatch to the three `cutout_from_*` per variant pair), plus the `PolylineShape`/`Shape` methods (`corner`, `corner_approx`, `border_line`, `border_line_count`, `corner_is_bounded`, `is_bounded`, `is_empty`, `dimension`, `bounding_box`, `bounding_octagon`, `bounding_tile`, `translate_by`, `offset`, `enlarge`, `max_width`, `min_width`, `simplify`, `is_int_box`, `is_int_octagon`, `to_simplex`, `intersects(&TileShape)`, `intersects_box`, `intersects_octagon`, `intersects_simplex`, `get_id`).
  - `enum RegularTileShape { Box(IntBox), Octagon(IntOctagon) }` — `compare(&RegularTileShape, edge) -> Side`, `union(&RegularTileShape) -> RegularTileShape`, `contains(&RegularTileShape)`, `is_contained_in_octagon`, `to_tile_shape() -> TileShape`, `bounding_box`, `area`, `get_id`. This is the R-tree key type in Plan 2.
  - `enum ShapeBoundingDirections { Orthogonal, FortyfiveDegree }` — `bounds_box(&IntBox) -> RegularTileShape`, `bounds_octagon`, `bounds_simplex`, `bounds_circle` (**Task 17**), `bounds_polygon` (**Task 17**), `bounds_tile(&TileShape)`, `bounds_shape(&Shape)` (**Task 17**). Orthogonal → `Box(bounding_box())`, FortyfiveDegree → `Octagon(bounding_octagon())`.
  - `enum FortyfiveDegreeDirection { Right, Right45, Up, Up45, Left, Left45, Down, Down45 }` with `to_int_direction() -> IntDirection`, `from_int_direction(&IntDirection) -> Option<Self>`, `from_line(&Line) -> Option<Self>` (mirror the Java `getInstance` methods that exist); `IntOctagon::border_point(&IntPoint, FortyfiveDegreeDirection) -> IntPoint` lives here.
  - `Line::is_on_the_left(&TileShape)`, `Line::is_on_the_right(&TileShape)`.

- [ ] **Step 1: Write failing tests**

`tile_shape.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::float_point::FloatPoint;
    use crate::simplex::Simplex;

    fn bx() -> TileShape { TileShape::Box(IntBox::from_coords(0, 0, 10, 10)) }
    fn tri() -> TileShape {
        TileShape::Simplex(Simplex::from_points(&[IntPoint::new(0, 0), IntPoint::new(10, 0), IntPoint::new(0, 10)]))
    }

    #[test]
    fn containment_family() {
        let b = bx();
        assert!(b.contains(&Point::Int(IntPoint::new(5, 5))));
        assert!(b.contains(&Point::Int(IntPoint::new(0, 5))));        // border counts as contained
        assert!(!b.contains_inside(&Point::Int(IntPoint::new(0, 5))));
        assert!(b.contains_on_border(&Point::Int(IntPoint::new(0, 5))));
        assert_eq!(b.contains_on_border_line_no(&Point::Int(IntPoint::new(5, 5))), None);
        assert!(b.is_outside(&Point::Int(IntPoint::new(11, 5))));
        assert!(b.contains_float(&FloatPoint::new(9.9, 0.1)));
        assert!(b.contains_tile(&TileShape::Box(IntBox::from_coords(2, 2, 3, 3))));
        assert!(!b.contains_tile(&TileShape::Box(IntBox::from_coords(2, 2, 30, 3))));
    }

    #[test]
    fn area_and_intersection_across_variants() {
        assert_eq!(bx().area(), 100.0);
        assert!((tri().area() - 50.0).abs() < 1e-9);
        let i = bx().intersection(&tri());
        assert!((i.area() - 50.0).abs() < 1e-9);
        let oct = TileShape::Octagon(IntBox::from_coords(5, 5, 20, 20).to_int_octagon());
        let j = bx().intersection(&oct);
        assert_eq!(j.area(), 25.0);
        assert!(bx().intersects(&oct));
        assert!(!tri().intersects(&TileShape::Box(IntBox::from_coords(8, 8, 9, 9))));
        // simplify folds a box-shaped simplex back into IntBox
        let s = TileShape::Simplex(IntBox::from_coords(0, 0, 10, 10).to_simplex());
        assert!(matches!(s.simplify(), TileShape::Box(_)));
        assert!(matches!(bx().intersection_with_simplify(&oct), TileShape::Box(_)));
    }

    #[test]
    fn distances_and_nearest() {
        let b = bx();
        assert_eq!(b.distance(&FloatPoint::new(13.0, 14.0)), 5.0);
        assert_eq!(b.distance(&FloatPoint::new(5.0, 5.0)), 0.0);
        assert_eq!(b.border_distance(&FloatPoint::new(5.0, 5.0)), 5.0);
        assert_eq!(b.smallest_radius(), 5.0);
        assert_eq!(b.nearest_point(&Point::Int(IntPoint::new(-4, 5))), Point::Int(IntPoint::new(0, 5)));
        assert_eq!(b.nearest_border_point(&Point::Int(IntPoint::new(1, 5))), Point::Int(IntPoint::new(0, 5)));
        assert_eq!(b.max_width(), 10.0);
    }

    #[test]
    fn touching_sides_and_side_of_line() {
        let a = bx();
        let b = TileShape::Box(IntBox::from_coords(10, 0, 20, 10));
        let ts = a.touching_sides(&b).expect("boxes share an edge");
        assert_eq!(a.border_line(ts[0]).a.x, 10);
        assert_eq!(b.border_line(ts[1]).a.x, 10);
        assert!(a.touching_sides(&TileShape::Box(IntBox::from_coords(30, 0, 40, 10))).is_none());
        let far = crate::line::Line::from_coords(50, 0, 50, 1);
        assert_ne!(a.side_of_line(&far), crate::Side::Collinear);
        assert!(far.is_on_the_right(&a) || far.is_on_the_left(&a));
    }

    #[test]
    fn transformations() {
        let t = tri().turn_90_degree(1, &IntPoint::new(0, 0));
        assert_eq!(t.bounding_box(), IntBox::from_coords(-10, 0, 0, 10));
        let m = tri().mirror_vertical(&IntPoint::new(0, 0));
        assert_eq!(m.bounding_box(), IntBox::from_coords(-10, 0, 0, 10));
        let s = bx().shrink(2.0);
        assert_eq!(s.bounding_box(), IntBox::from_coords(2, 2, 8, 8));
        let parts = bx().divide_into_sections(4.0);
        assert!(parts.len() >= 4);
        assert!((parts.iter().map(|p| p.area()).sum::<f64>() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn cutout_from_tile() {
        let outer = TileShape::Box(IntBox::from_coords(0, 0, 20, 20));
        let pieces = tri().cutout_from(&outer);
        let total: f64 = pieces.iter().map(|p| p.area()).sum();
        assert!((total - (400.0 - 50.0)).abs() < 1e-6);
    }
}
```
`regular_tile_shape.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_box::IntBox;
    use crate::bounding_directions::ShapeBoundingDirections;
    use crate::simplex::Simplex;
    use crate::int_point::IntPoint;

    #[test]
    fn union_and_contains_mixed() {
        let a = RegularTileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        let o = RegularTileShape::Octagon(IntBox::from_coords(5, 5, 20, 20).to_int_octagon());
        let u = a.union(&o);
        assert!(u.contains(&a));
        assert!(u.contains(&o));
        assert_eq!(u.bounding_box(), IntBox::from_coords(0, 0, 20, 20));
        assert!(matches!(a.union(&RegularTileShape::Box(IntBox::from_coords(1, 1, 2, 2))), RegularTileShape::Box(_)));
    }

    #[test]
    fn bounding_directions_pick_variant() {
        let tri = Simplex::from_points(&[IntPoint::new(0, 0), IntPoint::new(10, 0), IntPoint::new(0, 10)]);
        assert!(matches!(ShapeBoundingDirections::Orthogonal.bounds_simplex(&tri), RegularTileShape::Box(_)));
        assert!(matches!(ShapeBoundingDirections::FortyfiveDegree.bounds_simplex(&tri), RegularTileShape::Octagon(_)));
        let oct = ShapeBoundingDirections::FortyfiveDegree.bounds_simplex(&tri);
        assert!((oct.area() - 50.0).abs() < 1e-9); // the 45° bound of a right triangle is exact
    }
}
```

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement** — `TileShape.java` is mostly concrete algorithms on top of `corner(i)`/`border_line(i)`/`border_line_count()`. Implement those three as `match` on the enum and then port the rest of the file *once*, as methods on the enum. Move the previously deferred `IntBox`/`IntOctagon`/`Simplex` cross-type methods into `impl` blocks in this file.

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): TileShape/RegularTileShape enums and bounding directions"
```

---

### Task 15: `LineSegment`

**Files:**
- Create: `crates/fr-geometry/src/line_segment.rs`
- Modify: `tile_shape.rs` (`is_intersected_interior_by(&LineSegment)`), `lib.rs`
- Java: `LineSegment.java`

**Interfaces:**
- Produces: `LineSegment { start: Line, middle: Line, end: Line }` — `new(start, middle, end)`, `from_polyline(&Polyline, no)` (**Task 16**), `from_polyline_shape(&TileShape, no)` (Java takes `PolylineShape`; in Plan 2 `PolygonShape` also needs it — implement generically over `&dyn PolylineShapeOps` in Task 17), `start_point() -> Point`, `end_point() -> Point`, `start_point_approx`, `end_point_approx`, `get_line`, `get_start_closing_line`, `get_end_closing_line`, `opposite`, `to_polyline() -> Polyline` (**Task 16**), `to_simplex() -> Simplex`, `contains(&Point)`, `bounding_box`, `bounding_octagon`, `change_length_approx(f64)`, `intersection(&LineSegment) -> Vec<Line>` (Java returns `Line[]` of length 0/1/2), `intersects(&LineSegment)`, `overlaps(&LineSegment)`, `stair_approximation(width, to_the_right) -> Vec<IntPoint>`, `stair_approximation_45`, `border_intersections(&TileShape) -> Vec<usize>` (Java `int[]`, may log a warning on odd counts — keep the check as a `debug_assert!`-free branch that just truncates as Java does), `sort_endpoints_in_xy()`.
- `TileShape::is_intersected_interior_by(&LineSegment)`.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::Line;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::int_box::IntBox;
    use crate::tile_shape::TileShape;

    fn seg(ax: i32, ay: i32, bx: i32, by: i32) -> LineSegment {
        let middle = Line::from_coords(ax, ay, bx, by);
        let d = middle.direction();
        let perp = d.turn_45_degree(2);
        let start = Line::from_direction(IntPoint::new(ax, ay), &perp);
        let end = Line::from_direction(IntPoint::new(bx, by), &perp);
        LineSegment::new(start, middle, end)
    }

    #[test]
    fn endpoints_and_box() {
        let s = seg(0, 0, 10, 0);
        assert_eq!(s.start_point(), Point::Int(IntPoint::new(0, 0)));
        assert_eq!(s.end_point(), Point::Int(IntPoint::new(10, 0)));
        assert_eq!(s.bounding_box(), IntBox::from_coords(0, 0, 10, 0));
        assert_eq!(s.opposite().start_point(), Point::Int(IntPoint::new(10, 0)));
        assert!(s.contains(&Point::Int(IntPoint::new(4, 0))));
        assert!(!s.contains(&Point::Int(IntPoint::new(11, 0))));
    }

    #[test]
    fn intersections() {
        let h = seg(0, 0, 10, 0);
        let v = seg(5, -5, 5, 5);
        assert!(h.intersects(&v));
        assert_eq!(h.intersection(&v).len(), 1);
        assert!(!h.intersects(&seg(20, -5, 20, 5)));
        assert!(h.overlaps(&seg(5, 0, 15, 0)));
        assert!(!h.overlaps(&seg(11, 0, 15, 0)));
    }

    #[test]
    fn stair_approximation_stays_near_segment() {
        let s = seg(0, 0, 20, 7);
        let pts = s.stair_approximation(2.0, true);
        assert!(pts.len() >= 3);
        assert_eq!(pts.first().copied(), Some(IntPoint::new(0, 0)));
        assert_eq!(pts.last().copied(), Some(IntPoint::new(20, 7)));
        for w in pts.windows(2) {
            let d = w[1].difference_by(&w[0]);
            assert!(d.is_orthogonal(), "stairs must be orthogonal steps");
        }
    }

    #[test]
    fn border_intersections_with_tile() {
        let s = seg(-5, 5, 15, 5);
        let b = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        let hits = s.border_intersections(&b);
        assert_eq!(hits.len(), 2);
        assert!(b.is_intersected_interior_by(&s));
        assert!(!b.is_intersected_interior_by(&seg(-5, 0, 15, 0))); // runs along the border
    }

    #[test]
    fn to_simplex_is_the_segment_strip() {
        let s = seg(0, 0, 10, 0);
        let sx = s.to_simplex();
        assert_eq!(sx.dimension(), 1);
        assert_eq!(sx.bounding_box(), IntBox::from_coords(0, 0, 10, 0));
    }
}
```

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement** — port verbatim. `stair_approximation` uses `java_round` for the width computations and `function_value_approx`.

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): LineSegment"
```

---

### Task 16: `Polygon` and `Polyline`

**Files:**
- Create: `crates/fr-geometry/src/polygon.rs`, `crates/fr-geometry/src/polyline.rs`
- Modify: `line_segment.rs` (`from_polyline`, `to_polyline`), `tile_shape.rs` (`cutout(&Polyline)`, `entrance_points(&Polyline)`), `lib.rs`
- Java: `Polygon.java`, `Polyline.java`

**Interfaces:**
- Produces:
  - `Polygon { corners: Vec<Point> }` — `new(points: Vec<Point>)` (Java removes consecutive duplicates and collinear middle corners — port that normalisation), `corner_array() -> &[Point]`, `revert_corners()`, `winding_number_after_closing() -> i32`.
  - `Polyline { lines: Vec<Line> }` (`Clone, PartialEq, Eq, Hash`) — `from_polygon(&Polygon)`, `from_points(&[Point])`, `from_two_points(&Point, &Point)`, `from_lines(Vec<Line>)` (Java `Polyline(Line[])` — removes redundant/parallel consecutive lines; port exactly, it is used by trace normalisation), `lines() -> &[Line]`, `corner_count() -> usize`, `is_empty`, `is_point`, `is_orthogonal`, `is_multiple_of_45_degree`, `first_corner() -> Point`, `last_corner()`, `corners() -> Vec<Point>`, `corner_approx_arr() -> Vec<FloatPoint>`, `corner_approx(i)`, `corner(i) -> Point`, `reverse()`, `length_approx_between(from, to) -> f64`, `length_approx()`, `offset_shapes(half_width: i32) -> Vec<TileShape>`, `offset_shapes_between(half_width, from, to)`, `offset_shape(half_width, no) -> TileShape`, `offset_box(half_width, no) -> IntBox`, `translate_by(&Vector)`, `turn_90_degree(i32, &IntPoint)`, `rotate_approx(f64, &FloatPoint)`, `mirror_vertical`, `mirror_horizontal`, `bounding_box_between(from, to) -> IntBox`, `bounding_box()`, `bounding_octagon_between(from, to)`, `nearest_point_approx(&FloatPoint)`, `distance(&FloatPoint)`, `combine(&Polyline) -> Polyline` (Java returns a new polyline or *this* when they don't join — read the code; represent "could not combine" as `Option<Polyline>` only if Java signals it with `null`; otherwise return `Polyline`), `split(line_index, end_line: &Line) -> Option<[Polyline; 2]>` (Java returns `null` when the split is degenerate), `skip_lines(from, to)`, `contains(&Point)`, `projection_line(&Point) -> Option<LineSegment>`, `shorten(new_line_count, last_segment_length) -> Polyline`.
  - `LineSegment::from_polyline(&Polyline, no)`, `LineSegment::to_polyline()`.
  - `TileShape::cutout(&Polyline) -> Vec<Polyline>`, `TileShape::entrance_points(&Polyline) -> Vec<[usize; 2]>`.

- [ ] **Step 1: Write failing tests**

`polyline.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::int_box::IntBox;
    use crate::line::Line;
    use crate::float_point::FloatPoint;

    fn pts(v: &[(i32, i32)]) -> Vec<Point> { v.iter().map(|&(x, y)| Point::Int(IntPoint::new(x, y))).collect() }
    fn l_shape() -> Polyline { Polyline::from_points(&pts(&[(0, 0), (10, 0), (10, 10)])) }

    #[test]
    fn corners_roundtrip_and_length() {
        let p = l_shape();
        assert_eq!(p.corner_count(), 3);
        assert_eq!(p.lines().len(), 4); // n corners ⇒ n+1 lines (closing lines at both ends)
        assert_eq!(p.corners(), pts(&[(0, 0), (10, 0), (10, 10)]));
        assert_eq!(p.first_corner(), Point::Int(IntPoint::new(0, 0)));
        assert_eq!(p.last_corner(), Point::Int(IntPoint::new(10, 10)));
        assert_eq!(p.length_approx(), 20.0);
        assert!(p.is_orthogonal());
        assert!(p.is_multiple_of_45_degree());
        assert!(!p.is_point());
        assert_eq!(p.reverse().first_corner(), Point::Int(IntPoint::new(10, 10)));
        assert_eq!(p.bounding_box(), IntBox::from_coords(0, 0, 10, 10));
    }

    #[test]
    fn collinear_middle_corner_is_dropped() {
        let p = Polyline::from_points(&pts(&[(0, 0), (5, 0), (10, 0)]));
        assert_eq!(p.corner_count(), 2);
        let q = Polyline::from_points(&pts(&[(0, 0), (0, 0), (10, 0)]));
        assert_eq!(q.corner_count(), 2);
    }

    #[test]
    fn offset_shapes_cover_segments() {
        let p = l_shape();
        let shapes = p.offset_shapes(2);
        assert_eq!(shapes.len(), 2);
        assert!(shapes[0].contains(&Point::Int(IntPoint::new(5, 1))));
        assert!(shapes[0].contains(&Point::Int(IntPoint::new(0, 0))));
        assert!(!shapes[0].contains(&Point::Int(IntPoint::new(5, 4))));
        assert_eq!(p.offset_box(2, 0), IntBox::from_coords(-2, -2, 12, 2));
    }

    #[test]
    fn combine_and_split() {
        let a = Polyline::from_points(&pts(&[(0, 0), (10, 0)]));
        let b = Polyline::from_points(&pts(&[(10, 0), (10, 10)]));
        let c = a.combine(&b);
        assert_eq!(c.corners(), pts(&[(0, 0), (10, 0), (10, 10)]));
        // split the L at its horizontal segment (line index 1) by the vertical line x = 5
        let parts = l_shape().split(1, &Line::from_coords(5, 0, 5, 1)).expect("splits");
        assert_eq!(parts[0].corners(), pts(&[(0, 0), (5, 0)]));
        assert_eq!(parts[1].corners(), pts(&[(5, 0), (10, 0), (10, 10)]));
    }

    #[test]
    fn nearest_point_distance_contains() {
        let p = l_shape();
        assert_eq!(p.nearest_point_approx(&FloatPoint::new(5.0, 3.0)), FloatPoint::new(5.0, 0.0));
        assert_eq!(p.distance(&FloatPoint::new(5.0, 3.0)), 3.0);
        assert!(p.contains(&Point::Int(IntPoint::new(10, 4))));
        assert!(!p.contains(&Point::Int(IntPoint::new(4, 4))));
        let proj = p.projection_line(&Point::Int(IntPoint::new(12, 4))).unwrap();
        assert_eq!(proj.start_point(), Point::Int(IntPoint::new(10, 0)));
    }

    #[test]
    fn transformations() {
        let p = l_shape();
        assert_eq!(p.translate_by(&crate::vector::Vector::new(1, 1)).bounding_box(), IntBox::from_coords(1, 1, 11, 11));
        assert_eq!(p.turn_90_degree(1, &IntPoint::new(0, 0)).bounding_box(), IntBox::from_coords(-10, 0, 0, 10));
        assert_eq!(p.mirror_vertical(&IntPoint::new(0, 0)).bounding_box(), IntBox::from_coords(-10, 0, 0, 10));
        assert_eq!(p.skip_lines(0, 0).corner_count(), 3);
    }

    #[test]
    fn shorten_reduces_lines() {
        let p = Polyline::from_points(&pts(&[(0, 0), (10, 0), (10, 10), (20, 10)]));
        let s = p.shorten(3, 5.0);
        assert_eq!(s.lines().len(), 3);
        assert!(s.length_approx() < p.length_approx());
    }
}
```
`polygon.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    fn pts(v: &[(i32, i32)]) -> Vec<Point> { v.iter().map(|&(x, y)| Point::Int(IntPoint::new(x, y))).collect() }

    #[test]
    fn winding_number_sign_follows_orientation() {
        let ccw = Polygon::new(pts(&[(0, 0), (10, 0), (10, 10), (0, 10)]));
        let cw = ccw.revert_corners();
        assert_eq!(ccw.winding_number_after_closing().signum(), -cw.winding_number_after_closing().signum());
        assert_eq!(ccw.corner_array().len(), 4);
    }

    #[test]
    fn duplicates_and_collinear_removed() {
        let p = Polygon::new(pts(&[(0, 0), (0, 0), (5, 0), (10, 0), (10, 10)]));
        assert_eq!(p.corner_array().len(), 3);
    }
}
```
The `lines().len() == 4` for 3 corners assumption comes from `Polyline(Polygon)` in Java, which adds a closing line before the first and after the last corner. Verify, and if the count differs, Java wins.

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement** — port `Polygon.java`, then `Polyline.java`. `Polyline(Line[])` (the 100-line constructor) is where trace normalisation bugs live; port with its comments. `offset_shapes` is the core of trace-to-shape conversion used by the search tree; its 45°/90° special cases must be exact.

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): Polygon and Polyline with offset shapes"
```

---

### Task 17: `PolylineShapeOps`, `PolygonShape`, `PolylineArea`, `Circle`, `Ellipse`, `Shape`/`Area`

**Files:**
- Create: `crates/fr-geometry/src/{polyline_shape.rs,polygon_shape.rs,polyline_area.rs,circle.rs,ellipse.rs,shape.rs}`
- Modify: `bounding_directions.rs`, `line_segment.rs` (generic `from_polyline_shape`), `tile_shape.rs` (`intersects_circle`, `ShapeOps` impl), `lib.rs`
- Java: `PolylineShape.java`, `PolygonShape.java`, `PolylineArea.java`, `Circle.java`, `Ellipse.java`, `Shape.java`, `ConvexShape.java`, `Area.java`

**Interfaces:**
- Produces:
  - `trait PolylineShapeOps { fn corner(&self, i) -> Point; fn border_line(&self, i) -> Line; fn border_line_count(&self) -> usize; fn corner_is_bounded(&self, i) -> bool; fn is_bounded(&self) -> bool; fn dimension(&self) -> i32; … }` with the concrete `PolylineShape.java` methods as provided methods: `bounded_corners`, `corner_approx`, `corner_approx_arr`, `equals_corner(&Point) -> Option<usize>`, `circumference`, `centre_of_gravity`, `is_contained_in(&IntBox)`, `index_of_left_most_corner(&FloatPoint)`, `index_of_right_most_corner`, `polar_line_segment(&FloatPoint) -> FloatLine`, `prev_no`, `next_no`, `intersects_line(&Line)`, `left_most_corner(&Point)`, `right_most_corner`. Implemented for `TileShape` and `PolygonShape`.
  - `PolygonShape { corners: Vec<Point> }` (Java constructor sorts to counter-clockwise and de-duplicates — port) — `from_polygon`, `from_points`, `corner`, `border_line_count`, `corner_is_bounded`, `intersects(&Shape)`, `intersects_circle`, `intersects_simplex`, `intersects_octagon`, `intersects_box`, `cutout(&Polyline)`, `enlarge(f64) -> PolygonShape`, `border_distance`, `smallest_radius`, `contains_float`, `contains(&Point)`, `contains_inside`, `is_outside`, `contains_on_border` (Java: stub returning `false` with a commented warning — port as-is), `distance`, `translate_by`, `bounding_shape(dirs)`, `bounding_box`, `bounding_octagon`, `is_convex`, `convex_hull() -> PolygonShape`, `bounding_tile() -> TileShape`, `area`, `dimension`, `is_bounded`, `is_empty`, `border_line`, `nearest_point_approx`, `turn_90_degree`, `rotate_approx`, `mirror_vertical`, `mirror_horizontal`, `split_to_convex() -> Vec<TileShape>` (Java: may return `null` on failure → `Option<Vec<TileShape>>`; check).
  - `PolylineArea { border: PolygonShape /* or TileShape? Java: PolylineShape */, holes: Vec<…> }` — Java holds `PolylineShape` for border and holes; represent with `enum PolylineShapeRef { Tile(TileShape), Polygon(PolygonShape) }` or, simpler and sufficient for DSN input, `PolygonShape` for both. **Decision:** use `PolygonShape` (DSN areas always arrive as polygons; `Simplex` borders are never constructed as areas in board code — confirm with `grep -rn "new PolylineArea(" ../freerouting/src/main/java`; if a `TileShape` border exists, use the enum). Methods: `new(border, holes)`, `dimension`, `is_bounded`, `is_empty`, `is_contained_in(&IntBox)`, `get_border`, `get_holes`, `bounding_box`, `bounding_octagon`, `contains_float`, `contains(&Point)`, `nearest_point_approx`, `translate_by`, `corner_approx_arr`, `split_to_convex() -> Vec<TileShape>` (takes an optional stop-check closure `Option<&dyn Fn() -> bool>` in place of Java's `Stoppable`), `turn_90_degree`, `rotate_approx`, `mirror_vertical`, `mirror_horizontal`.
  - `Circle { center: IntPoint, radius: i32 }` — all `Circle.java` methods; `bounding_tile_max_seg(max_segment_length)`.
  - `Ellipse { center: FloatPoint, rotation: f64, radius_1: f64, radius_2: f64 }` — data only + `new`.
  - `trait ShapeOps` (from `Shape.java` + `Area.java` + `ConvexShape.java` where applicable) and `enum Shape { Tile(TileShape), Polygon(PolygonShape), Circle(Circle) }`, `enum Area { Shape(Shape), Polyline(PolylineArea) }`, both dispatching by `match`. `Shape::intersects(&Shape)` covers all 9 pairings via the per-type methods.
  - `ShapeBoundingDirections::bounds_circle`, `bounds_polygon`, `bounds_shape`.
  - `LineSegment::from_polyline_shape(&dyn PolylineShapeOps, no)`.

- [ ] **Step 1: Write failing tests**

`polygon_shape.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::int_box::IntBox;
    use crate::float_point::FloatPoint;
    use crate::polyline_shape::PolylineShapeOps;

    fn pts(v: &[(i32, i32)]) -> Vec<Point> { v.iter().map(|&(x, y)| Point::Int(IntPoint::new(x, y))).collect() }
    fn square() -> PolygonShape { PolygonShape::from_points(&pts(&[(0, 0), (10, 0), (10, 10), (0, 10)])) }
    fn l_shape() -> PolygonShape { PolygonShape::from_points(&pts(&[(0, 0), (20, 0), (20, 10), (10, 10), (10, 20), (0, 20)])) }

    #[test]
    fn convexity_area_and_split() {
        assert!(square().is_convex());
        assert_eq!(square().area(), 100.0);
        assert_eq!(square().split_to_convex().len(), 1);
        assert!(!l_shape().is_convex());
        assert_eq!(l_shape().area(), 300.0);
        let parts = l_shape().split_to_convex();
        assert!(parts.len() >= 2);
        assert!((parts.iter().map(|t| t.area()).sum::<f64>() - 300.0).abs() < 1e-9);
        assert_eq!(l_shape().convex_hull().area(), 400.0);
        assert_eq!(l_shape().bounding_box(), IntBox::from_coords(0, 0, 20, 20));
    }

    #[test]
    fn orientation_is_normalised_to_counterclockwise() {
        let cw = PolygonShape::from_points(&pts(&[(0, 0), (0, 10), (10, 10), (10, 0)]));
        // Java's constructor reverses clockwise input; both must then be equal corner sequences up to rotation.
        let a = square().corner_approx_arr();
        let b = cw.corner_approx_arr();
        assert_eq!(a.len(), b.len());
        assert!(a.iter().all(|p| b.contains(p)));
        assert!(cw.is_convex());
    }

    #[test]
    fn containment() {
        assert!(l_shape().contains(&Point::Int(IntPoint::new(5, 15))));
        assert!(!l_shape().contains(&Point::Int(IntPoint::new(15, 15))));
        assert!(l_shape().contains_float(&FloatPoint::new(15.0, 5.0)));
        assert!(l_shape().is_outside(&Point::Int(IntPoint::new(25, 5))));
        assert!(l_shape().intersects_box(&IntBox::from_coords(15, 15, 30, 30)) == false || true); // touching corner: pin from Java
        assert!(square().intersects_box(&IntBox::from_coords(5, 5, 30, 30)));
    }

    #[test]
    fn polyline_shape_ops() {
        let s = square();
        assert_eq!(s.border_line_count(), 4);
        assert_eq!(s.circumference(), 40.0);
        assert_eq!(s.centre_of_gravity(), FloatPoint::new(5.0, 5.0));
        assert_eq!(s.equals_corner(&Point::Int(IntPoint::new(10, 10))).is_some(), true);
        assert_eq!(s.next_no(3), 0);
        assert_eq!(s.prev_no(0), 3);
    }
}
```
Replace the `== false || true` line with the exact Java result after reading `PolygonShape.intersects(IntBox)`.

`circle.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::int_box::IntBox;
    use crate::float_point::FloatPoint;

    #[test]
    fn metrics_and_containment() {
        let c = Circle::new(IntPoint::new(0, 0), 10);
        assert!((c.area() - std::f64::consts::PI * 100.0).abs() < 1e-9);
        assert_eq!(c.bounding_box(), IntBox::from_coords(-10, -10, 10, 10));
        assert!(c.contains(&Point::Int(IntPoint::new(5, 5))));
        assert!(!c.contains(&Point::Int(IntPoint::new(8, 8))));
        assert!(c.contains_on_border(&Point::Int(IntPoint::new(10, 0))));
        assert_eq!(c.distance(&FloatPoint::new(13.0, 0.0)), 3.0);
        assert_eq!(c.smallest_radius(), 10.0);
        assert!(c.bounding_tile().contains(&Point::Int(IntPoint::new(7, 7))));
        assert!(c.bounding_octagon().is_normalized());
        assert!(c.intersects_box(&IntBox::from_coords(9, 9, 20, 20)) == c.intersects_box(&IntBox::from_coords(9, 9, 20, 20))); // pin after reading Java
        assert!(!c.intersects_box(&IntBox::from_coords(11, 11, 20, 20)));
        assert!(c.intersects_circle(&Circle::new(IntPoint::new(15, 0), 6)));
        assert!(!c.intersects_circle(&Circle::new(IntPoint::new(20, 0), 6)));
    }
}
```
Pin the tautological assertion once the exact semantics are read.

`polyline_area.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::polygon_shape::PolygonShape;
    use crate::int_box::IntBox;

    fn pts(v: &[(i32, i32)]) -> Vec<Point> { v.iter().map(|&(x, y)| Point::Int(IntPoint::new(x, y))).collect() }

    #[test]
    fn square_with_hole() {
        let border = PolygonShape::from_points(&pts(&[(0, 0), (30, 0), (30, 30), (0, 30)]));
        let hole = PolygonShape::from_points(&pts(&[(10, 10), (20, 10), (20, 20), (10, 20)]));
        let a = PolylineArea::new(border, vec![hole]);
        assert!(a.is_bounded());
        assert_eq!(a.bounding_box(), IntBox::from_coords(0, 0, 30, 30));
        assert!(a.contains(&Point::Int(IntPoint::new(5, 5))));
        assert!(!a.contains(&Point::Int(IntPoint::new(15, 15))));
        let parts = a.split_to_convex(None);
        assert!(parts.len() >= 4);
        assert!((parts.iter().map(|t| t.area()).sum::<f64>() - 800.0).abs() < 1e-9);
        for t in &parts {
            assert!(!t.contains_inside(&Point::Int(IntPoint::new(15, 15))));
        }
    }
}
```
`shape.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::circle::Circle;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::tile_shape::TileShape;
    use crate::bounding_directions::ShapeBoundingDirections;

    #[test]
    fn shape_dispatch() {
        let c = Shape::Circle(Circle::new(IntPoint::new(0, 0), 10));
        let b = Shape::Tile(TileShape::Box(IntBox::from_coords(5, 5, 20, 20)));
        assert!(c.intersects(&b));
        assert!(b.intersects(&c));
        assert_eq!(b.bounding_box(), IntBox::from_coords(5, 5, 20, 20));
        assert!(matches!(ShapeBoundingDirections::Orthogonal.bounds_shape(&c), crate::regular_tile_shape::RegularTileShape::Box(_)));
        assert_eq!(Area::Shape(b.clone()).bounding_box(), b.bounding_box());
    }
}
```

- [ ] **Step 2: Run to verify failure**

- [ ] **Step 3: Implement** — port in this order: `PolylineShape.java` (trait), `PolygonShape.java`, `Circle.java`, `Ellipse.java`, `Shape.java`/`Area.java`/`ConvexShape.java` (trait + enums), `PolylineArea.java` (`split_to_convex` uses `PolygonShape.split_to_convex` + `TileShape.cutout_from` for holes — port exactly; the algorithm is used for copper pours).

- [ ] **Step 4: Run tests, lint, commit**

```bash
git add -A
git commit -m "feat(geometry): PolygonShape, PolylineArea, Circle, Shape/Area enums"
```

---

### Task 18: `lib.rs` public surface, docs, and an unported-method audit

**Files:**
- Modify: `crates/fr-geometry/src/lib.rs`
- Create: `crates/fr-geometry/README.md`, `scripts/audit-geometry-port.sh`

**Interfaces:**
- Produces: `fr_geometry::prelude::*` re-exporting every public type (`IntPoint, IntVector, IntDirection, Point, Vector, Direction, RationalPoint, RationalVector, BigIntDirection, FloatPoint, FloatLine, Line, LineSegment, IntBox, IntOctagon, Simplex, TileShape, RegularTileShape, ShapeBoundingDirections, FortyfiveDegreeDirection, Polygon, Polyline, PolylineShapeOps, PolygonShape, PolylineArea, Circle, Ellipse, Shape, ShapeOps, Area, Side, Signum, CRIT_INT, java_round`).

- [ ] **Step 1: Write the audit script**

`scripts/audit-geometry-port.sh` — lists Java public methods per class and greps the Rust crate for the snake_case name, printing any that are missing and not marked `not ported`:
```bash
#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}/src/main/java/app/freerouting/geometry/planar"
RS="$ROOT/crates/fr-geometry/src"
missing=0
for f in "$JAVA"/*.java; do
  cls="$(basename "$f" .java)"
  grep -hoE '^\s*public [^=(]*\b([a-zA-Z0-9_]+)\s*\(' "$f" | sed -E 's/.*\b([a-zA-Z0-9_]+)\s*\($/\1/' | sort -u | while read -r m; do
    [[ "$m" == "$cls" ]] && continue   # constructors
    snake="$(echo "$m" | sed -E 's/([a-z0-9])([A-Z])/\1_\2/g; s/([A-Z])([A-Z][a-z])/\1_\2/g' | tr 'A-Z' 'a-z')"
    if ! grep -rqE "fn ${snake}(_[a-z0-9_]+)?\s*[<(]" "$RS" && ! grep -rqE "not ported: .*\b${m}\b" "$RS"; then
      echo "MISSING $cls.$m  (expected fn ${snake}*)"
      missing=1
    fi
  done
done
exit $missing
```
`chmod +x` it.

- [ ] **Step 2: Run the audit and fix every hit**

Run: `scripts/audit-geometry-port.sh`
Expected: prints `MISSING …` lines. For each: either port the method (with a one-line test) or add `// not ported: unused outside geometry — <ClassName>.<method>` after confirming with `grep -rn "\.method(" ../freerouting/src/main/java/app/freerouting --include=*.java | grep -v geometry/planar` that nothing outside geometry calls it. Iterate until the script exits 0.

- [ ] **Step 3: Write `prelude` and README**

`lib.rs` gets `pub mod prelude { pub use crate::{…}; }` with the list above. `README.md`: 15 lines — what the crate is, the type-mapping table from the "Geometry port — conventions" section, and the invariants (no f64 in exact paths, `java_round`).

- [ ] **Step 4: Run everything, commit**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo doc -p fr-geometry --no-deps`

```bash
git add -A
git commit -m "feat(geometry): prelude, README, and port-completeness audit"
```

---

### Task 19: Cross-representation consistency tests

**Files:**
- Create: `crates/fr-geometry/tests/consistency.rs`

**Interfaces:**
- Consumes: the whole `fr_geometry::prelude`.

These are cheap property-style checks (deterministic pseudo-random inputs via a tiny LCG — no `rand` dependency) that catch the class of bug a behavioral port is most prone to: two code paths that Java keeps in agreement diverging in the port.

- [ ] **Step 1: Write the tests**

```rust
use fr_geometry::prelude::*;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.0 >> 33 }
    fn coord(&mut self, range: i32) -> i32 { (self.next() % (2 * range as u64 + 1)) as i32 - range }
    fn point(&mut self, range: i32) -> IntPoint { IntPoint::new(self.coord(range), self.coord(range)) }
}

#[test]
fn line_intersection_exact_matches_approx_when_integral() {
    let mut rng = Lcg(1);
    for _ in 0..2000 {
        let l1 = Line::new(rng.point(1000), rng.point(1000));
        let l2 = Line::new(rng.point(1000), rng.point(1000));
        if l1.a == l1.b || l2.a == l2.b || l1.is_parallel(&l2) { continue; }
        let exact = l1.intersection(&l2);
        let approx = l1.intersection_approx(&l2);
        let f = exact.to_float();
        assert!((f.x - approx.x).abs() < 1e-6 && (f.y - approx.y).abs() < 1e-6, "{l1:?} {l2:?}");
        // the exact point lies on both lines
        assert_eq!(l1.side_of(&exact), Side::Collinear);
        assert_eq!(l2.side_of(&exact), Side::Collinear);
    }
}

#[test]
fn box_octagon_simplex_agree_on_containment_and_intersection() {
    let mut rng = Lcg(2);
    for _ in 0..500 {
        let (a, b) = (rng.point(500), rng.point(500));
        let bx = IntBox::new(IntPoint::new(a.x.min(b.x), a.y.min(b.y)), IntPoint::new(a.x.max(b.x), a.y.max(b.y)));
        let oct = bx.to_int_octagon();
        let sx = bx.to_simplex();
        for _ in 0..20 {
            let p = Point::Int(rng.point(600));
            let t_box = TileShape::Box(bx);
            let t_oct = TileShape::Octagon(oct);
            let t_sx = TileShape::Simplex(sx.clone());
            assert_eq!(t_box.contains(&p), t_oct.contains(&p), "{bx:?} {p:?}");
            assert_eq!(t_box.contains(&p), t_sx.contains(&p), "{bx:?} {p:?}");
        }
        let (c, d) = (rng.point(500), rng.point(500));
        let other = IntBox::new(IntPoint::new(c.x.min(d.x), c.y.min(d.y)), IntPoint::new(c.x.max(d.x), c.y.max(d.y)));
        let i1 = bx.intersection(&other);
        let i2 = oct.intersection_box(&other);
        let i3 = sx.intersection_box(&other);
        assert_eq!(i1.is_empty(), i2.is_empty());
        assert_eq!(i1.is_empty(), i3.is_empty());
        if !i1.is_empty() {
            assert_eq!(i2.bounding_box(), i1);
            assert_eq!(i3.bounding_box(), i1);
        }
    }
}

#[test]
fn polyline_offset_shapes_contain_their_segments() {
    let mut rng = Lcg(3);
    for _ in 0..200 {
        let n = 2 + (rng.next() % 5) as usize;
        let pts: Vec<Point> = (0..n).map(|_| Point::Int(rng.point(2000))).collect();
        let pl = Polyline::from_points(&pts);
        if pl.corner_count() < 2 { continue; }
        let hw = 1 + (rng.next() % 50) as i32;
        let shapes = pl.offset_shapes(hw);
        assert_eq!(shapes.len(), pl.corner_count() - 1);
        for (i, s) in shapes.iter().enumerate() {
            assert!(s.contains(&pl.corner(i)), "segment {i} start not inside its offset shape");
            assert!(s.contains(&pl.corner(i + 1)), "segment {i} end not inside its offset shape");
            assert!(s.contains_tile(&TileShape::Box(pl.offset_box(0, i))) || true, "loose check; box may exceed on diagonals");
        }
    }
}

#[test]
fn rational_and_int_points_compare_consistently() {
    let mut rng = Lcg(4);
    for _ in 0..2000 {
        let a = rng.point(CRIT_INT);
        let b = rng.point(CRIT_INT);
        let pa = Point::Int(a);
        let pb = Point::Int(b);
        let ra = Point::Rational(RationalPoint::from_int(&a));
        assert_eq!(pa.compare_xy(&pb), ra.compare_xy(&pb));
        assert_eq!(pa, ra);
        assert_eq!(pa.difference_by(&pb), ra.difference_by(&pb));
    }
}
```
Delete the `|| true` line in the third test (it is a placeholder for a check that is not universally true) before committing.

- [ ] **Step 2: Run** — `cargo test -p fr-geometry --test consistency`
Expected: pass. Any failure here is a real porting bug: fix it in the module, never by loosening the test.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "test(geometry): cross-representation consistency checks"
```

---

## Plan self-review (done at write time)

**Spec coverage (this plan's slice):** §4 workspace layout → Task 1; §12 legacy shim + subcommands → Task 2 (stubs; real behaviour in Plan 8); §13 stdio JSON-RPC skeleton → Task 3 (tools registered in Plan 8); §14 harness + reference generation → Task 4; §5 geometry (all classes) → Tasks 5–17, completeness enforced by Task 18's audit, invariants (`i64` signs, `java_round`, BigInt promotion) in Global Constraints and Task 19.

**Deliberately out of this plan:** `board/searchtree`, `datastructures/ShapeTree`, `PlanarDelaunayTriangulation` (Plan 2); settings, DSN, DRC, router (Plans 3–7).

**Type consistency:** `Point`/`Vector`/`Direction`/`TileShape`/`RegularTileShape`/`Shape`/`Area` names are fixed in the conventions table and used identically in every task; `Line { a: IntPoint, b: IntPoint }` (Task 10) is what `Simplex`, `LineSegment`, `Polyline` consume. Methods that need a later type are listed as "added in Task N" at both ends.

**Known judgement calls the executor must confirm against Java (each is flagged in its task):** `Vector.sideOf` sign convention (Task 6), `turn45Degree` negative modulo (Task 6), `from_big` divides `y` after checking only `x mod z` (Task 8), `FloatLine.intersection` sentinel (Task 9), `IntBox` corner numbering and `EMPTY` (Task 11), `PolylineArea` border type (Task 17), `-mp 0` producing an unrouted SES (Task 4).
