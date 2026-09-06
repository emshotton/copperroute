# fr-settings

The router-configuration layer. It answers one question — *given a DSN file,
a `.rules` file, a settings JSON, the environment, the command line and a
board, what settings does the router actually run with?* Use
`fr_settings::prelude::*` to bring in every public type.

The crate sits on `fr-dsn` (which owns the `.dsn` and `.rules` readers) and
on `fr-board` (for the one pass that reads a board), and depends on `serde`,
`serde_json` and `thiserror` besides. No `tracing`: a source that cannot be
applied becomes a `MergeReport::errors` entry. No static mutable state.

## The settings types

`RouterSettings` is the root: `enabled`, `algorithm`, `max_passes`,
`max_items`, `max_threads`, `vias_allowed`, `automatic_neckdown`,
`strict_drc`, `trace_pull_tight_accuracy`, `copper_to_edge_clearance_um`,
`hole_clearance_um`, `neck_width_um`, `job_timeout_string`,
`result_json_path`, `ignore_net_classes`, `save_intermediate_stages`,
`opt_changed_area_ms`, `smd_via_relaxation`, `failure_give_up_threshold`, a
per-layer `layers: Vec<LayerSettings>`, and the nested `fanout:
FanoutSettings`, `optimizer: OptimizerSettings` and `scoring:
ScoringSettings`. `DesignRulesCheckerSettings` and `DebugSettings` live in
`drc_settings.rs`; `BoardUpdateStrategy` and `ItemSelectionStrategy` in
`optimizer_settings.rs`.

**Every field is `Option<T>`, and that is the merge protocol.** `None` means
"this source has no opinion", not "off". The null-coalescing accessors
(`get_max_passes`, `get_layer_active`, `get_bend_cost`, …) apply the defaults
only at the point of use, so a partial source can be merged over a complete
one without erasing anything.

`ExpansionCostFactor { horizontal, vertical }` is the per-layer trace cost
pair the router reads; `RouterSettings::get_trace_costs` builds one per
layer from the layer settings.

## Sources and the merge

Each source implements `SettingsSource` (`get_settings`, `get_source_name`,
`get_priority`, `kind`). `SettingsMerger` sorts its sources by priority,
clones the first non-empty one and applies the rest over it with
`apply_new_values_from`, then validates. The priorities are constants in
`merger::priority`:

| priority | source | where it comes from |
|---|---|---|
| 0 | `DefaultSettings` | built-in defaults, sized by `HostEnvironment` |
| 10 | `JsonFileSettings` | a `freerouting.json` named on the command line (`--settings <file>`) |
| 20 | `DsnFileSettings` | the design's own `(autoroute_settings …)` block |
| 30 | `SesFileSettings` | a session file (contributes nothing today) |
| 40 | `RulesFileSettings` | a `.rules` file |
| 55 | `EnvironmentVariablesSource` | `FREEROUTING__ROUTER__<A>__<B>=v` |
| 60 | `CliSettings` | `--router.<a>.<b>=v`, or `--set router.<a>.<b>=v` on the native command line |
| 65 | GUI | reserved, unused |
| 70 | `ApiSettings` | the sparse tier an MCP `route_board` call's `settings` argument fills |

`set_field_value(&mut RouterSettings, path, value)` (`field_path.rs`) is the
string-path writer behind the environment and command-line sources; it
knows every field's type (`FieldSpec`/`FieldKind`) and refuses unknown paths
and unparsable values with a `MergeError`.

`copy_fields.rs` is the field-by-field merge (`CopyFields`, implemented per
struct in declaration order) with two rules that decide routing behaviour:
primitive and `String` arrays copy only into an empty target (first writer
wins), and object arrays merge element-wise and are never shrunk. That is
why a `.rules` file's per-layer trace costs do not reach the router through
the merge but its per-layer preferred directions do. `MergeMode::FillAbsent`
is the inverse mode, used once (below).

## `resolve_headless`

The headless run does not perform one merge; it performs two, with a
board-tuning pass between them and a `.rules` re-apply after the second,
and the `.rules` file ends up outranking the environment and the command
line for every field an `(autoroute_settings)` block can carry.
`resolve_headless` (`resolve.rs`) is that sequence as one linear pass:

```text
s = DefaultSettings
s.apply(json_file)                                   // 10
s.apply(dsn)                                         // 20
s.apply(parse(cli_rules))                            // 40   (a rules file named on the command line)
s.apply(env)                                         // 55
s.apply(cli)                                         // 60
s.validate()
s.set_layer_count(board) if it disagrees
s.apply_board_specific_optimizations(board)
s.board_specific_trace_costs_applied = None
s.fill_absent_from(parse(scheduler_rules))           // the second merge's own contribution
s.validate()
s.apply(parse_against(scheduler_rules, board))       // the rules file, re-read against the board's layers
s.apply_board_specific_optimizations(board)
```

Its inputs are `SettingsInputs`:

```rust,ignore
pub struct SettingsInputs<'a> {
    pub json_file:       Option<&'a RouterSettings>,
    pub dsn:             Option<&'a RouterSettings>,
    pub cli_rules:       Option<&'a [u8]>,
    pub scheduler_rules: Option<&'a [u8]>,
    pub env:             Option<&'a RouterSettings>,
    pub cli:             Option<&'a RouterSettings>,
}
```

The two rules slots are **bytes**, not parsed objects, because the same
file is parsed twice against two different layer structures: once with the
layers discovered from the file's own `(layer_rule …)` names, and once
against the board's. A two-layer rules file on a four-layer board puts
`B.Cu` at a different index in each, and handing one parsed object to both
steps produces the wrong settings. Feed it `std::fs::read(path)`.

`resolve_scheduler_rules_path(job_rules, cli_rules, dsn_path)` says which
`.rules` file a run picks up (an explicit one, else the one beside the
design); `_with` injects the file-exists probe for tests.

Three things to keep in view while reading `resolve.rs`:

- **`board_specific_trace_costs_applied` is cleared by hand** at the point
  where the two merges would have handed over to a fresh object, so the
  second `apply_board_specific_optimizations` re-derives the costs.
- **`validate()` is not idempotent.** `max_passes == 0` becomes `i32::MAX` on
  the first call and `9999` on the second, because the second call clamps a
  value above `9999`. The linear pass calls it exactly twice, like the two
  merges it replaces.
- **`resolve_headless` models the CLI-started run.** An MCP `route_board`
  call has a sparse priority-70 payload instead, and `crates/freerouting`
  composes that path itself.

## Defaults, clamps and the host

`DefaultSettings::new(&host)` fills every field; the headline constants are
public (`DEFAULT_VIA_COSTS = 50`, `DEFAULT_PLANE_VIA_COSTS = 5`,
`DEFAULT_START_RIPUP_COSTS = 100`, `DEFAULT_BEND_PENALTY = 10.0`,
`DEFAULT_UNROUTED_NET_PENALTY = 5e6`,
`DEFAULT_CLEARANCE_VIOLATION_PENALTY = 1e6`,
`DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM = 500.0`,
`DEFAULT_HOLE_CLEARANCE_UM = 0.0`). `layers` and the two cost arrays are left
absent — their size depends on the board.

Clamps live on the setters: `set_bend_cost` clamps to `[0.0, 9.9]`;
`set_via_costs`, `set_plane_via_costs` and `set_start_ripup_costs` floor at
1; the two trace-cost setters floor at 0.1 and set the applied flag;
`validate` clamps `max_passes` to `0..=9999` (with `0` meaning unlimited),
`max_threads` to the core count, and `trace_pull_tight_accuracy` to at
least 1.

Every machine-dependent number goes through `HostEnvironment`.
`HostEnvironment::detect()` calls `std::thread::available_parallelism()`
once — the only such call in the crate — and `HostEnvironment::with_processors(n)`
is what every test uses, so the processor count never leaks into a test
expectation.

## JSON

`RouterSettings::from_json_str` / `to_json_string_pretty` (`json.rs`) read
and write the settings document. The reader is strict JSON (`NaN` and
`Infinity` are refused) and drops the three transient fields (`max_items`,
`save_intermediate_stages`, `ignore_net_classes`). The writer emits fields
in declaration order through `fr_dsn::format::json::to_gson_string_pretty`,
the layout shared with the DRC report and the result manifest.

## The legacy short flags

`sources/cli.rs` also carries `apply_command_line_arguments`, the table for
the legacy short flags (`-mp`, `-mt`, `-oit`, `-us`, `-is`, `-hr`, `-inc`,
`-drc`). It writes a `LegacyBridge` that nothing on the routing path reads:
those flags are parsed for their effect on argument positions and
diagnostics, and the values that actually reach the router come through the
priority-60 `CliSettings` spelling. The `freerouting` binary no longer accepts
that form; nothing in the workspace calls this table any more.

## Tests

`cargo test -p fr-settings` runs the unit tests plus eleven integration
suites: `copy_fields`, `field_path`, `router_settings`, `board_optimizations`,
`sources`, `env_source`, `cli_source`, `precedence`, `json`, `struct_shape`
and `corpus`.

- **`precedence.rs`** is the load-bearing one. It runs a 64-case matrix
  (`tests/matrix/mod.rs`: 4 DSN shapes × 4 rules shapes × 2 environments × 2
  command lines) through **both** forms of the composition — the literal
  two-merge shape built out of `SettingsMerger`, and the linear
  `resolve_headless` — and asserts they agree field for field.
- **`struct_shape.rs`** pins every struct's field order, because
  `copy_fields` iterates it and `json.rs` emits it.
- **`json.rs`** replays 64 committed settings documents from
  `tests/golden/p4t1-mode1/all.txt` through the reader and writer.
- **`corpus.rs`** runs `DsnFileSettings::new` over every `.dsn` in the
  fixture corpus (~2.7 s in debug, not ignored) and asserts the source's
  layer count matches the header the reader reported. Its `.rules` sibling is
  `#[cfg_attr(debug_assertions, ignore)]`; run it with
  `cargo test -p fr-settings --release --test corpus`.

`tests/data/README.md` describes the committed inputs — the two `.rules`
matrices the precedence suite feeds in and the trimmed
`Issue029-hw48na_reduced.rules`.

### What needs the fixture corpus

`../freerouting` (or `FREEROUTING_JAVA_DIR`) supplies the DSN and `.rules`
fixtures; it is not vendored. `corpus.rs`, `precedence.rs`, `sources.rs` and
`json.rs` skip with a printed message when it is absent;
`board_optimizations.rs`'s three `*_golden_*` tests read a fixture board and
panic without it, while its synthetic-board tests — the majority — need
nothing. `copy_fields`, `field_path`, `router_settings`, `env_source`,
`cli_source` and `struct_shape` are pure.

## Conventions this crate shares with the workspace

**`#![forbid(unsafe_code)]`** sits in the crate root, as it does in every
workspace crate.
