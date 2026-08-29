//! The Plan 4 precedence matrix, as data.
//!
//! Task 8 runs every case through **both** forms of Java's headless settings resolution — the
//! literal two-merge shape (`Freerouting.java:125-146` + `RoutingJobScheduler.java:103-186`) and
//! the linear [`fr_settings::resolve_headless`] — and asserts they agree field for field.
//! Task 9's `p4t1` differential runs the *same* table against the JVM, which is why the axes live
//! here as `const` data rather than inline in `tests/precedence.rs`: a driver elsewhere in the
//! workspace can `#[path = "…/tests/matrix/mod.rs"] mod matrix;` and enumerate the identical
//! case list, and the Java side can be fed the same fixture names, argv and environment pairs.
//!
//! # The axes
//!
//! | axis | values |
//! |---|---|
//! | DSN (`DsnFileSettings`, priority 20) | none, 2-layer without an `(autoroute_settings)` block, 2-layer with one, 4-layer |
//! | rules (`RulesFileSettings`, priority 40) | none, `-dr` only, scheduler-only, `-dr` ≠ scheduler |
//! | environment (priority 55) | none, `max_passes` + a per-layer direction |
//! | CLI (priority 60) | none, `max_passes` + `optimizer.enabled` |
//!
//! # Which combinations are reachable — the pruning rule
//!
//! The task brief expected the 4 × 4 × 2 × 2 = 64 cross product to prune to 40 "reachable"
//! combinations. The table runs all 64 instead: 44 of them are reachable from the CLI-started
//! headless path this matrix models, and the remaining 20 are kept deliberately as extra
//! coverage of the merge algebra, labelled as such below rather than passed off as paths Java
//! runs. The pruning rule is therefore written down as the reachability argument it was meant to
//! be, rather than left implicit (Task 8 report, deviation D1 and fix round 1).
//!
//! The rules axis is already the *pruned* one: the two rules slots are the CLI's
//! `initialRulesFile`, which feeds **merge #1** (`Freerouting.java:129-136`), and the scheduler's
//! own choice, which feeds **merge #2** (`RoutingJobScheduler.java:118-152`,
//! `job.rules ?? -dr ?? adjacent <design>.rules`). Of their four shapes, two are reachable from
//! the CLI-started path this matrix models, one is reachable there only with a DSN input, and one
//! is not reachable there at all:
//!
//! - `none` — no `-dr`, no adjacent file. **CLI-reachable.**
//! - `cli` — `-dr R`: `Freerouting.java:130` also stores `R` as `job.rules`, so the scheduler's
//!   `else if` chain picks the same file and both merges see `R`. **CLI-reachable.**
//! - `scheduler` — an adjacent `<design>.rules` beside a DSN input (`:131-151`). The probe is
//!   `isDsn`-gated, so this shape is **CLI-reachable only on the three DSN rows**; the
//!   `dsn-none/rules-scheduler` cases are extra coverage (below).
//! - `cli ≠ scheduler` — merge #1 sees `R` and merge #2 sees a different file. Nothing in the
//!   CLI path can produce it: `Freerouting.java:129` reads `globalSettings.initialRulesFile`,
//!   `:130` gives `job.rules` that same file, and the scheduler prefers `job.rules` (`:118-121`).
//!   **Not CLI-reachable** — extra coverage.
//!
//! The shape "merge #1 has rules, merge #2 has none" is unreachable for a different reason (the
//! scheduler's chain cannot lose a `-dr` file merge #1 already read) and is absent from the axis
//! altogether.
//!
//! ## The two columns that are extra coverage, and why they are not "the API path"
//!
//! An earlier revision of this file justified `rules-scheduler` without a DSN, and `rules-split`,
//! by pointing at an API job carrying its own `job.rules` — the scheduler is shared by the CLI
//! and the API, after all. **That justification is wrong, and the reviewer caught it.** An API
//! job never runs merge #1: `job.routerSettings` is `new RouterSettings()`
//! (`core/RoutingJob.java:105`), or the request body deserialised straight into one
//! (`api/v1/JobInputResource.java:203-211`), or a bare `setLayerCount` (`:337-339`, `:540-542`).
//! So `new ApiSettings(job.routerSettings)` at priority 70 (`RoutingJobScheduler.java:163-166`)
//! is a **sparse** payload there, not a complete one, and merge #2's own `0..60` chain does the
//! real work — which is the opposite of the premise
//! [`fr_settings::resolve_headless`] linearises. The API path needs a different composition and
//! is Plan 8's problem (`obligation:` on `resolve_headless`).
//!
//! The two columns stay in the table regardless: they are the only inputs anywhere in the matrix
//! where the two merges see *different* rules, and they are where the linearisation's one
//! boundary showed up (`tests/precedence.rs::a_split_rules_pair_restarts_the_cost_array_race`) —
//! which no CLI-reachable case could have found. They are labelled as synthetic coverage rather
//! than as a path Java runs. What they do *not* do is separate `fill_absent_from` from the
//! post-merge re-apply: both steps consume the same source and the re-apply overwrites whatever
//! the fill wrote, so that one distinction is drawn by `src/resolve.rs`'s step tests instead
//! (fix round 1's mutation table).
//!
//! # Fixtures and the board
//!
//! Every case gets a synthetic all-signal board of 2 000 000 × 1 000 000 (the geometry Task 5's
//! `BProbe` goldens use) with as many layers as the DSN axis names — 2 for the "no DSN" row,
//! since a job that reaches the scheduler always has a board. Keeping the board synthetic keeps
//! the aspect-ratio penalties exact (`0.2` and `0.5` at one decimal) and lets Task 9's JVM driver
//! build the identical `RoutingBoard` with the six-line recipe `BProbe.java` already uses; the
//! *settings* still come from real DSN fixtures read by the real `DsnFileSettings`.
//!
//! ```sh
//! # the fixtures this table names
//! F=/Users/em/Development/freerouting/freerouting/fixtures
//! ls $F/Issue413-test.dsn $F/Issue187-processor.Z80.dsn $F/Issue066-Project_GP8B.dsn
//! ls crates/fr-settings/tests/data/Plan4Matrix-{primary,adjacent}.rules
//! ```

#![allow(dead_code)] // Task 9 reuses accessors this test binary does not call.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use fr_board::prelude::*;
use fr_geometry::{IntBox, PolylineShapeRef, TileShape};
use fr_settings::prelude::*;

// ---------------------------------------------------------------------------------------------
// the axes, as data
// ---------------------------------------------------------------------------------------------

/// One value of the DSN axis: the file `DsnFileSettings` reads, and the board that goes with it.
pub struct DsnCase {
    /// Stable id, used in the case id and in the JVM driver's output.
    pub id: &'static str,
    /// The fixture under `../freerouting/fixtures`, or `None` for "no DSN source at all" — a
    /// KiCad JSON input, where `RoutingJobScheduler.java:111-115` registers no `DsnFileSettings`.
    pub fixture: Option<&'static str>,
    /// The board's layer count; also the DSN's, where there is one.
    pub layer_count: usize,
}

/// One value of the rules axis — the two rules slots described in the module docs.
pub struct RulesCase {
    pub id: &'static str,
    /// `-dr` / `-de …rules`: merge #1's priority-40 source (`Freerouting.java:129-136`).
    pub cli_rules: Option<&'static str>,
    /// The scheduler's choice: merge #2's priority-40 source **and** the post-merge
    /// `RulesReader.read` payload (`RoutingJobScheduler.java:118-152`, `:172-181`).
    pub scheduler_rules: Option<&'static str>,
}

/// One value of the environment axis (`EnvironmentVariablesSource`, priority 55).
pub struct EnvCase {
    pub id: &'static str,
    pub vars: &'static [(&'static str, &'static str)],
}

/// One value of the CLI axis (`CliSettings`, priority 60).
pub struct CliCase {
    pub id: &'static str,
    pub argv: &'static [&'static str],
}

/// The board every case is tuned against — `2 000 000 × 1 000 000`, all signal layers.
pub const BOARD_WIDTH: i32 = 2_000_000;
/// See [`BOARD_WIDTH`].
pub const BOARD_HEIGHT: i32 = 1_000_000;

pub const DSN_CASES: [DsnCase; 4] = [
    DsnCase {
        id: "dsn-none",
        fixture: None,
        layer_count: 2,
    },
    // No `(autoroute_settings)` block at all: `DsnFileSettings.java:46-48` seeds the layer count,
    // which is what freezes both `scoring` cost arrays for every later source (quirk #128).
    DsnCase {
        id: "dsn2-bare",
        fixture: Some("Issue413-test.dsn"),
        layer_count: 2,
    },
    // Has an `(autoroute_settings)` block with per-layer rules, so the seeding at `:46-48` does
    // **not** fire and the block's own costs are what reach priority 20.
    DsnCase {
        id: "dsn2-autoroute",
        fixture: Some("Issue187-processor.Z80.dsn"),
        layer_count: 2,
    },
    DsnCase {
        id: "dsn4-bare",
        fixture: Some("Issue066-Project_GP8B.dsn"),
        layer_count: 4,
    },
];

/// The `.rules` files under `crates/fr-settings/tests/data`, committed so the JVM driver and the
/// Rust test read the same bytes (`tests/data/README.md`).
pub const PRIMARY_RULES: &str = "Plan4Matrix-primary.rules";
/// See [`PRIMARY_RULES`]. Differs in every value it carries, so a case where merge #1 and
/// merge #2 see different rules cannot pass by coincidence.
pub const ADJACENT_RULES: &str = "Plan4Matrix-adjacent.rules";

/// The four rules shapes. `rules-none` and `rules-cli` are reachable from the CLI-started path on
/// every DSN row; `rules-scheduler` is reachable there only on the three DSN rows (the adjacent
/// probe is `isDsn`-gated); `rules-split` is **synthetic** — see the module docs.
pub const RULES_CASES: [RulesCase; 4] = [
    RulesCase {
        id: "rules-none",
        cli_rules: None,
        scheduler_rules: None,
    },
    RulesCase {
        id: "rules-cli",
        cli_rules: Some(PRIMARY_RULES),
        scheduler_rules: Some(PRIMARY_RULES),
    },
    RulesCase {
        id: "rules-scheduler",
        cli_rules: None,
        scheduler_rules: Some(ADJACENT_RULES),
    },
    RulesCase {
        id: "rules-split",
        cli_rules: Some(PRIMARY_RULES),
        scheduler_rules: Some(ADJACENT_RULES),
    },
];

pub const ENV_CASES: [EnvCase; 2] = [
    EnvCase {
        id: "env-none",
        vars: &[],
    },
    // The per-layer direction carries **two** tokens whatever the board's layer count is, so on
    // the 4-layer row it is discarded wholesale by `HeadlessBoardManager`'s `setLayerCount`
    // (quirk #119) and on the 2-layer rows it survives.
    EnvCase {
        id: "env-set",
        vars: &[
            ("FREEROUTING__ROUTER__MAX_PASSES", "77"),
            (
                "FREEROUTING__ROUTER__LAYERS__PREFERRED_DIRECTION_HORIZONTAL",
                "true,true",
            ),
        ],
    },
];

pub const CLI_CASES: [CliCase; 2] = [
    CliCase {
        id: "cli-none",
        argv: &[],
    },
    CliCase {
        id: "cli-set",
        argv: &["--router.max_passes=88", "--router.optimizer.enabled=false"],
    },
];

/// One row of the matrix.
pub struct Case {
    pub id: String,
    pub dsn: &'static DsnCase,
    pub rules: &'static RulesCase,
    pub env: &'static EnvCase,
    pub cli: &'static CliCase,
}

/// The full cross product, in a stable order — see the module docs for why nothing is pruned.
pub fn cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(64);
    for dsn in &DSN_CASES {
        for rules in &RULES_CASES {
            for env in &ENV_CASES {
                for cli in &CLI_CASES {
                    cases.push(Case {
                        id: format!("{}/{}/{}/{}", dsn.id, rules.id, env.id, cli.id),
                        dsn,
                        rules,
                        env,
                        cli,
                    });
                }
            }
        }
    }
    cases
}

// ---------------------------------------------------------------------------------------------
// the sources each axis value builds
// ---------------------------------------------------------------------------------------------

/// `crates/fr-settings/tests/data/<name>`, resolved from the **workspace root** rather than from
/// `CARGO_MANIFEST_DIR`.
///
/// `env!("CARGO_MANIFEST_DIR")` expands in whichever crate compiles this module, so it would name
/// the wrong directory for a `#[path]` include from outside `fr-settings` — and Task 9's driver
/// (`scripts/differential/rust`) is exactly that. [`parity::workspace_root`] is derived from the
/// `parity` crate's own manifest directory, so it answers the same path for every consumer, the
/// way [`parity::fixture`] already does for the DSN fixtures below.
pub fn data_path(name: &str) -> PathBuf {
    parity::workspace_root()
        .join("crates")
        .join("fr-settings")
        .join("tests")
        .join("data")
        .join(name)
}

/// The DSN source, parsed once per fixture and cloned — `DsnFileSettings::new` re-reads the whole
/// file, and the matrix asks for the same three fixtures 48 times between them.
pub fn dsn_source(case: &DsnCase) -> Option<DsnFileSettings> {
    static CACHE: OnceLock<Vec<Option<DsnFileSettings>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        DSN_CASES
            .iter()
            .map(|dsn| {
                let name = dsn.fixture?;
                let bytes = std::fs::read(parity::fixture(name))
                    .unwrap_or_else(|e| panic!("cannot read fixture {name}: {e}"));
                Some(DsnFileSettings::new(&bytes[..], name))
            })
            .collect()
    });
    let index = DSN_CASES
        .iter()
        .position(|candidate| candidate.id == case.id)
        .expect("case comes from DSN_CASES");
    cache[index].clone()
}

/// A `.rules` source read from `tests/data`, parsed once per file and cloned.
pub fn rules_source(name: Option<&str>) -> Option<RulesFileSettings> {
    let name = name?;
    static CACHE: OnceLock<BTreeMap<String, RulesFileSettings>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        [PRIMARY_RULES, ADJACENT_RULES]
            .into_iter()
            .map(|name| {
                (
                    name.to_string(),
                    RulesFileSettings::from_path(&data_path(name)),
                )
            })
            .collect()
    });
    Some(cache[name].clone())
}

/// `EnvironmentVariablesSource` over this case's pairs.
pub fn env_source(case: &EnvCase) -> EnvironmentVariablesSource {
    let vars: BTreeMap<String, String> = case
        .vars
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect();
    EnvironmentVariablesSource::new(&vars)
}

/// `CliSettings` over this case's argv.
pub fn cli_source(case: &CliCase) -> CliSettings {
    let argv: Vec<String> = case.argv.iter().map(|arg| (*arg).to_string()).collect();
    CliSettings::new(&argv)
}

/// The synthetic board — `BProbe.java`'s recipe, with the layer names the two `.rules` fixtures
/// use so that a JVM driver's `RulesReader.read` can map them.
pub fn board(case: &DsnCase) -> Board {
    let names: [&str; 4] = ["F.Cu", "In1.Cu", "In2.Cu", "B.Cu"];
    let layer_names: Vec<&str> = match case.layer_count {
        2 => vec![names[0], names[3]],
        4 => names.to_vec(),
        other => panic!("no layer naming for {other} layers"),
    };
    let layers = LayerStructure::new(
        layer_names
            .into_iter()
            .map(|name| Layer::new(name.to_string(), true))
            .collect(),
    );
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers, 10);
    let mut rules = BoardRules::new(layers.clone(), clearance_matrix);
    rules.create_default_net_class();
    let box_ = IntBox::from_coords(0, 0, BOARD_WIDTH, BOARD_HEIGHT);
    Board::new(
        vec![PolylineShapeRef::Tile(TileShape::Box(box_))],
        0,
        box_,
        rules,
        BoardLibrary::new(Padstacks::new(layers), Packages::new()),
        Components::new(),
        Communication::default(),
    )
}
