//! Rust twin of `scripts/differential/java/P4T1.java` (Plan 4 Task 9).
//!
//! Runs `fr_settings::resolve_headless` — plan ruling 1's single linear pass — over the same
//! case table the Java driver walks through the *real* two-merge composition
//! (`Freerouting.java:125-146` + `HeadlessBoardManager.java:739-748` +
//! `RoutingJobScheduler.java:103-186`), and prints the resulting `RouterSettings` in the same
//! normalised `path=value` form. See `scripts/differential/README.md`.
//!
//! # Modes
//!
//! * `0` — the canonical dump. `<case-index|all>` picks one row or the whole table.
//! * `1` — Java-only (Gson). Plan 4 Task 10 owns serde parity; this driver refuses the mode
//!   rather than emitting something that would agree by accident.
//! * `2` — every case, whatever `<case-index>` says (`all 0` and `<anything> 2` are the same run).
//!
//! Every case is preceded by a `CASE <id>` line, so a diff names the row that moved.
//!
//! # Where the two sides build their inputs differently, and why that is exact
//!
//! Java parses the scheduler's `.rules` file **twice**, with two different layer structures:
//!
//! * at priority 40 through `RulesFileSettings` → `RulesReader.readRouterSettings`, whose layer
//!   structure is *discovered from the file itself* (`RulesReader.java:238-273`, a
//!   `LinkedHashSet` of the `(layer_rule …)` names), and
//! * again after the merge through `RulesReader.read(…, board, settings)`, whose layer structure
//!   is the **board's** (`RulesReader.java:112`).
//!
//! `fr_settings::SettingsInputs` has one `scheduler_rules` field for both, so this driver feeds
//! it the *board*-structured parse. That is exact whenever a board is present, which is every
//! case here: the priority-40 slot reaches the answer only through `fill_absent_from`, and by
//! that point `HeadlessBoardManager.java:741-745` has filled every `layers[i]` field and both
//! `scoring` cost arrays, leaving only `resultJsonPath`, `optimizer.timeoutString` and
//! `fanout.timeoutString` absent — three fields no `(autoroute_settings)` block can carry. The
//! two parses differ only when the file names fewer layers than the board has (a 4-layer board
//! and a two-`layer_rule` file map `B.Cu` to index 3 and index 1 respectively), and the driver's
//! `dsn4-*` rows are exactly where that would show. Recorded in the Task 9 report as the one
//! place `SettingsInputs` is lossier than Java.
//!
//! The `-dr` file at priority 40 of **merge #1** (`cli_rules`) is *not* re-parsed: Java reaches it
//! only through `RulesFileSettings`, so this driver does too.

use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use fr_board::prelude::*;
use fr_dsn::format::double::{java_double_to_string, java_float_to_string};
use fr_dsn::keyword::Keyword;
use fr_dsn::lexer::{DsnScanner, LexicalState, Token};
use fr_dsn::parser::autoroute_settings::read_autoroute_settings_scope;
use fr_dsn::parser::geometry::DsnLayerStructure;
use fr_dsn::parser::scope_parameter::{skip_scope, DsnReadOptions};
use fr_dsn::BoardReadResult;
use fr_geometry::{IntBox, PolylineShapeRef, TileShape};
use fr_settings::prelude::*;

const BOARD_WIDTH: i32 = 2_000_000;
const BOARD_HEIGHT: i32 = 1_000_000;
const LAYER_NAMES: [&str; 4] = ["F.Cu", "In1.Cu", "In2.Cu", "B.Cu"];

// -------------------------------------------------------------------------------------------
// the case table — `scripts/differential/matrix/p4t1-cases.tsv`, read by both drivers
// -------------------------------------------------------------------------------------------

struct Case {
    id: String,
    dsn: String,
    cli_rules: String,
    scheduler_rules: String,
    env: String,
    argv: String,
    board: String,
}

fn read_cases(path: &Path) -> Vec<Case> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            assert_eq!(f.len(), 7, "expected 7 columns in {line:?}");
            Case {
                id: f[0].to_string(),
                dsn: f[1].to_string(),
                cli_rules: f[2].to_string(),
                scheduler_rules: f[3].to_string(),
                env: f[4].to_string(),
                argv: f[5].to_string(),
                board: f[6].to_string(),
            }
        })
        .collect()
}

/// `D:<name>` is `crates/fr-settings/tests/data`; `F:<name>` (and a bare name) is the corpus.
fn resolve(spec: &str, fixtures: &Path, data: &Path) -> PathBuf {
    if let Some(rest) = spec.strip_prefix("D:") {
        data.join(rest)
    } else if let Some(rest) = spec.strip_prefix("F:") {
        fixtures.join(rest)
    } else {
        fixtures.join(spec)
    }
}

fn base_name(spec: &str) -> String {
    let s = spec
        .strip_prefix("D:")
        .or_else(|| spec.strip_prefix("F:"))
        .unwrap_or(spec);
    s.rsplit('/').next().unwrap_or(s).to_string()
}

fn read_optional(spec: &str, fixtures: &Path, data: &Path) -> Option<Vec<u8>> {
    if spec == "-" {
        return None;
    }
    let path = resolve(spec, fixtures, data);
    Some(std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display())))
}

fn parse_env(spec: &str) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    if spec == "-" {
        return env;
    }
    for pair in spec.split(';') {
        let (key, value) = pair.split_once('=').expect("env pair is K=V");
        env.insert(key.to_string(), value.to_string());
    }
    env
}

fn parse_argv(spec: &str) -> Vec<String> {
    if spec == "-" {
        Vec::new()
    } else {
        spec.split(' ').map(str::to_string).collect()
    }
}

// -------------------------------------------------------------------------------------------
// main
// -------------------------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: p4t1 <cases.tsv> <case-index|all> <mode 0-2>");
        std::process::exit(2);
    }
    let tsv = std::fs::canonicalize(&args[0]).expect("cases.tsv exists");
    let select = args[1].clone();
    let mode: u8 = args[2].parse().expect("mode must be 0-2");

    if mode == 1 {
        eprintln!(
            "p4t1: mode 1 is the Gson dump, which Plan 4 Task 10 (serde parity) owns. The Rust \
             side has no Gson-shaped serialiser yet, so this driver refuses the mode instead of \
             printing something that could agree by accident."
        );
        std::process::exit(3);
    }

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let fixtures = PathBuf::from(require_env("P4T1_FIXTURES"));
    let data = PathBuf::from(require_env("P4T1_DATA"));
    let processors: usize = require_env("P4T1_PROCESSORS").parse().expect("a number");

    // The header: plan ruling 7's check that the driver runs against the jar the port is a port
    // of, and ruling 6's check that the machine-dependent defaults are pinned to one number.
    // Java derives the path from `RouterSettings.class`'s code source; this side takes it from
    // the environment `run.sh` sets, so a mismatch is a real finding rather than a shared
    // assumption.
    let jar = std::fs::canonicalize(require_env("FREEROUTING_JAR")).expect("jar exists");
    let meta = std::fs::metadata(&jar).expect("jar metadata");
    let mtime = meta
        .modified()
        .expect("mtime")
        .duration_since(UNIX_EPOCH)
        .expect("after epoch")
        .as_millis();
    writeln!(
        out,
        "HEADER jar={} bytes={} mtime={mtime} processors={processors} cases={} select={select} mode={mode}",
        jar.display(),
        meta.len(),
        tsv.file_name().expect("file name").to_string_lossy(),
    )
    .expect("write");

    // Spec §2: there is no persistent config file, so the port has no priority-10 tier at all.
    // The Java driver *checks* that on an empty user-data directory and aborts with
    // `JSON_SOURCE_NOT_EMPTY` if the tier ever carries a value; this line is the agreement.
    writeln!(out, "JSON_SOURCE_EMPTY dir=<temp>").expect("write");

    let host = HostEnvironment::with_processors(processors);
    let cases = read_cases(&tsv);

    let selected: Vec<&Case> = if mode == 2 || select == "all" {
        cases.iter().collect()
    } else {
        let index: usize = select.parse().expect("case index, or `all`");
        vec![&cases[index]]
    };
    for case in selected {
        writeln!(out, "CASE {}", case.id).expect("write");
        emit(&mut out, &resolve_case(case, &fixtures, &data, &host));
    }
}

fn require_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("environment variable {name} is not set"))
}

// -------------------------------------------------------------------------------------------
// one case
// -------------------------------------------------------------------------------------------

fn resolve_case(
    case: &Case,
    fixtures: &Path,
    data: &Path,
    host: &HostEnvironment,
) -> RouterSettings {
    let dsn_bytes = read_optional(&case.dsn, fixtures, data);
    let cli_rules_bytes = read_optional(&case.cli_rules, fixtures, data);
    let scheduler_rules_bytes = read_optional(&case.scheduler_rules, fixtures, data);

    let board = build_board(case, dsn_bytes.as_deref());

    let dsn = dsn_bytes
        .as_ref()
        .map(|bytes| DsnFileSettings::new(&bytes[..], &base_name(&case.dsn)));
    let cli_rules = cli_rules_bytes
        .as_ref()
        .map(|bytes| RulesFileSettings::new(&bytes[..], &base_name(&case.cli_rules)));
    // The board-structured parse — see the module docs.
    let scheduler_rules = scheduler_rules_bytes
        .as_ref()
        .and_then(|bytes| read_rules_against_board(bytes, &board));
    let env = EnvironmentVariablesSource::new(&parse_env(&case.env));
    let cli = CliSettings::new(&parse_argv(&case.argv));

    let inputs = SettingsInputs {
        dsn: dsn.as_ref().and_then(SettingsSource::get_settings),
        cli_rules: cli_rules.as_ref().and_then(SettingsSource::get_settings),
        scheduler_rules: scheduler_rules.as_ref(),
        env: env.get_settings(),
        cli: cli.get_settings(),
    };
    resolve_headless(&inputs, Some(&board), host)
}

/// `RulesReader.read`'s `(autoroute_settings …)` arm alone (`RulesReader.java:80-99`, `:153-157`):
/// the file parsed against the **board's** layer structure (`:112`).
///
/// It is `read_router_settings` (`fr_dsn::rules_reader`) with `discover_layer_structure` replaced
/// by [`DsnLayerStructure::from_board`], which is the single difference between Java's two
/// parses of the same bytes.
fn read_rules_against_board(bytes: &[u8], board: &Board) -> Option<RouterSettings> {
    let text = String::from_utf8_lossy(bytes).into_owned();
    let layer_structure = DsnLayerStructure::from_board(board.layer_structure());
    let mut scanner = DsnScanner::new(&text).ok()?;

    // The `(rules PCB <name>` header (`RulesReader.java:80-110`); the name is consumed and, on a
    // mismatch, only logged.
    if scanner.next_token().ok()?? != Token::Open {
        return None;
    }
    if scanner.next_token().ok()?? != Token::Kw(Keyword::Rules) {
        return None;
    }
    if scanner.next_token().ok()?? != Token::Kw(Keyword::PcbScope) {
        return None;
    }
    scanner.yybegin(LexicalState::Name);
    scanner.next_token().ok()?;

    // `:116-162`, keeping only the `AUTOROUTE_SETTINGS` arm: every other scope writes the board's
    // rules, not the settings.
    let mut prev_was_open = false;
    loop {
        let next_token = scanner.next_token().ok()??;
        if next_token == Token::Close {
            return None;
        }
        let is_open = next_token == Token::Open;
        if prev_was_open {
            if next_token == Token::Kw(Keyword::AutorouteSettings) {
                let parsed = read_autoroute_settings_scope(&mut scanner, &layer_structure).ok()?;
                return parsed.map(|parsed| RouterSettings::from(&parsed));
            }
            skip_scope(&mut scanner).ok()?;
        }
        prev_was_open = is_open;
    }
}

/// The case's board: either `BProbe.java`'s synthetic recipe with the named layer count, or —
/// for the rows whose `.rules` file carries `(padstack …)`/`(class …)` scopes that need a real
/// library — the fixture read back through the real reader, as `RoutingJobScheduler.java:95`
/// reaches it through `loadFromSpecctraDsn`.
fn build_board(case: &Case, dsn_bytes: Option<&[u8]>) -> Board {
    if case.board == "dsn" {
        let bytes = dsn_bytes.expect("a `dsn` board needs a DSN fixture");
        let design_name = base_name(&case.dsn)
            .strip_suffix(".dsn")
            .unwrap_or_default()
            .to_string();
        let options = DsnReadOptions::default();
        let result = fr_dsn::read_board(bytes, None, Some(&design_name), &options);
        let board = match result {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => board,
            other => panic!("cannot read {}: {other:?}", case.dsn),
        };
        return *board.expect("the reader built a board");
    }
    let layer_count: usize = case.board.parse().expect("board is `dsn` or a layer count");
    let layer_names: Vec<&str> = match layer_count {
        2 => vec![LAYER_NAMES[0], LAYER_NAMES[3]],
        4 => LAYER_NAMES.to_vec(),
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

// -------------------------------------------------------------------------------------------
// the normalised dump — the Java field names, sorted by path
// -------------------------------------------------------------------------------------------

fn emit(out: &mut impl Write, settings: &RouterSettings) {
    let mut lines: Vec<String> = Vec::new();
    dump_router(&mut lines, settings);
    lines.sort();
    for line in lines {
        writeln!(out, "{line}").expect("write");
    }
}

fn push<T: JavaValue>(lines: &mut Vec<String>, path: &str, value: Option<&T>) {
    lines.push(format!(
        "{path}={}",
        value.map_or_else(|| "null".to_string(), JavaValue::java_string)
    ));
}

/// `String.valueOf` for the wrapper types the dump can meet, plus `Double.toString` /
/// `Float.toString` (Plan 3's `java_double_to_string` / `java_float_to_string`) and `Enum.name`.
trait JavaValue {
    fn java_string(&self) -> String;
}
impl JavaValue for bool {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl JavaValue for i32 {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl JavaValue for i64 {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl JavaValue for f64 {
    fn java_string(&self) -> String {
        java_double_to_string(*self)
    }
}
impl JavaValue for f32 {
    fn java_string(&self) -> String {
        java_float_to_string(*self)
    }
}
impl JavaValue for String {
    fn java_string(&self) -> String {
        self.clone()
    }
}
impl JavaValue for BoardUpdateStrategy {
    fn java_string(&self) -> String {
        self.java_name().to_string()
    }
}
impl JavaValue for ItemSelectionStrategy {
    fn java_string(&self) -> String {
        self.java_name().to_string()
    }
}

/// Java's array arm: a `<path>.length` line plus one line per element, so `null` and "empty"
/// stay distinguishable.
fn push_slice<T: JavaValue>(lines: &mut Vec<String>, path: &str, value: Option<&Vec<T>>) {
    match value {
        None => lines.push(format!("{path}=null")),
        Some(values) => {
            lines.push(format!("{path}.length={}", values.len()));
            for (i, value) in values.iter().enumerate() {
                push(lines, &format!("{path}[{i}]"), Some(value));
            }
        }
    }
}

fn dump_router(lines: &mut Vec<String>, s: &RouterSettings) {
    push(lines, "enabled", s.enabled.as_ref());
    push(lines, "algorithm", s.algorithm.as_ref());
    match s.fanout.as_ref() {
        None => lines.push("fanout=null".to_string()),
        Some(fanout) => dump_fanout(lines, fanout),
    }
    push(
        lines,
        "copperToEdgeClearanceUm",
        s.copper_to_edge_clearance_um.as_ref(),
    );
    push(lines, "holeClearanceUm", s.hole_clearance_um.as_ref());
    push(lines, "neckWidthUm", s.neck_width_um.as_ref());
    push(lines, "strictDrc", s.strict_drc.as_ref());
    push(lines, "jobTimeoutString", s.job_timeout_string.as_ref());
    push(lines, "maxPasses", s.max_passes.as_ref());
    push(lines, "maxItems", s.max_items.as_ref());
    match s.layers.as_ref() {
        None => lines.push("layers=null".to_string()),
        Some(layers) => {
            lines.push(format!("layers.length={}", layers.len()));
            for (i, layer) in layers.iter().enumerate() {
                let path = format!("layers[{i}]");
                push(lines, &format!("{path}.routable"), layer.routable.as_ref());
                push(
                    lines,
                    &format!("{path}.preferredDirectionHorizontal"),
                    layer.preferred_direction_horizontal.as_ref(),
                );
                push(lines, &format!("{path}.bendCost"), layer.bend_cost.as_ref());
            }
        }
    }
    push(
        lines,
        "saveIntermediateStages",
        s.save_intermediate_stages.as_ref(),
    );
    push_slice(lines, "ignoreNetClasses", s.ignore_net_classes.as_ref());
    push(
        lines,
        "tracePullTightAccuracy",
        s.trace_pull_tight_accuracy.as_ref(),
    );
    push(lines, "viasAllowed", s.vias_allowed.as_ref());
    push(lines, "automaticNeckdown", s.automatic_neckdown.as_ref());
    match s.optimizer.as_ref() {
        None => lines.push("optimizer=null".to_string()),
        Some(optimizer) => dump_optimizer(lines, optimizer),
    }
    match s.scoring.as_ref() {
        None => lines.push("scoring=null".to_string()),
        Some(scoring) => dump_scoring(lines, scoring),
    }
    push(lines, "maxThreads", s.max_threads.as_ref());
    push(lines, "resultJsonPath", s.result_json_path.as_ref());
    // `private transient` in Java, so `ReflectionUtil.copyFields` never writes it
    // (`ReflectionUtil.java:226-228`) and the Java driver dumps it through its accessor too.
    push(
        lines,
        "boardSpecificTraceCostsApplied",
        Some(&s.are_board_specific_trace_costs_applied()),
    );
}

fn dump_fanout(lines: &mut Vec<String>, f: &FanoutSettings) {
    push(lines, "fanout.enabled", f.enabled.as_ref());
    push(lines, "fanout.maxPasses", f.max_passes.as_ref());
    push(lines, "fanout.maxItems", f.max_items.as_ref());
    push(
        lines,
        "fanout.maxMillisecondsPerPin",
        f.max_milliseconds_per_pin.as_ref(),
    );
    push(lines, "fanout.ripupAllowed", f.ripup_allowed.as_ref());
    push(
        lines,
        "fanout.minEscapeLengthMm",
        f.min_escape_length_mm.as_ref(),
    );
    push(
        lines,
        "fanout.maxEscapeLengthMm",
        f.max_escape_length_mm.as_ref(),
    );
    push(
        lines,
        "fanout.startViaDiameterMm",
        f.start_via_diameter_mm.as_ref(),
    );
    push(
        lines,
        "fanout.endViaDiameterMm",
        f.end_via_diameter_mm.as_ref(),
    );
    push(
        lines,
        "fanout.pinSortingOrder",
        f.pin_sorting_order.as_ref(),
    );
    push(
        lines,
        "fanout.fallbackToBoardVias",
        f.fallback_to_board_vias.as_ref(),
    );
    push(lines, "fanout.timeoutString", f.timeout_string.as_ref());
}

fn dump_optimizer(lines: &mut Vec<String>, o: &OptimizerSettings) {
    push(lines, "optimizer.enabled", o.enabled.as_ref());
    push(lines, "optimizer.algorithm", o.algorithm.as_ref());
    push(lines, "optimizer.maxPasses", o.max_passes.as_ref());
    push(lines, "optimizer.maxItems", o.max_items.as_ref());
    push(lines, "optimizer.maxThreads", o.max_threads.as_ref());
    push(
        lines,
        "optimizer.optimizationImprovementThreshold",
        o.optimization_improvement_threshold.as_ref(),
    );
    push(
        lines,
        "optimizer.maxConsecutiveFailures",
        o.max_consecutive_failures.as_ref(),
    );
    push(
        lines,
        "optimizer.additionalRipupCostFactorAtStart",
        o.additional_ripup_cost_factor_at_start.as_ref(),
    );
    push(
        lines,
        "optimizer.traceRipupCostFactor",
        o.trace_ripup_cost_factor.as_ref(),
    );
    push(
        lines,
        "optimizer.maxAutoroutePasses",
        o.max_autoroute_passes.as_ref(),
    );
    push(
        lines,
        "optimizer.boardUpdateStrategy",
        o.board_update_strategy.as_ref(),
    );
    push(lines, "optimizer.hybridRatio", o.hybrid_ratio.as_ref());
    push(
        lines,
        "optimizer.itemSelectionStrategy",
        o.item_selection_strategy.as_ref(),
    );
    push(lines, "optimizer.timeoutString", o.timeout_string.as_ref());
}

fn dump_scoring(lines: &mut Vec<String>, s: &ScoringSettings) {
    push_slice(
        lines,
        "scoring.preferredDirectionTraceCost",
        s.preferred_direction_trace_cost.as_ref(),
    );
    push_slice(
        lines,
        "scoring.undesiredDirectionTraceCost",
        s.undesired_direction_trace_cost.as_ref(),
    );
    push(
        lines,
        "scoring.defaultPreferredDirectionTraceCost",
        s.default_preferred_direction_trace_cost.as_ref(),
    );
    push(
        lines,
        "scoring.defaultUndesiredDirectionTraceCost",
        s.default_undesired_direction_trace_cost.as_ref(),
    );
    push(lines, "scoring.viaCosts", s.via_costs.as_ref());
    push(lines, "scoring.planeViaCosts", s.plane_via_costs.as_ref());
    push(
        lines,
        "scoring.startRipupCosts",
        s.start_ripup_costs.as_ref(),
    );
    push(
        lines,
        "scoring.unroutedNetPenalty",
        s.unrouted_net_penalty.as_ref(),
    );
    push(
        lines,
        "scoring.clearanceViolationPenalty",
        s.clearance_violation_penalty.as_ref(),
    );
    push(lines, "scoring.bendPenalty", s.bend_penalty.as_ref());
    push(
        lines,
        "scoring.defaultBendCost",
        s.default_bend_cost.as_ref(),
    );
}
