//! Plan 8 Task 4 differential twin of `scripts/differential/java/P8T2.java`:
//! `fr_core::manifest` against `core.results.RoutingResultManifest`.
//!
//! Usage: `p8t2 [shape] <fixturesDir> <scratchDir>`, plus the internal `p8t2 gitsha-child` mode.
//! `scripts/differential/run.sh p8t2` runs this and the Java half and diffs their stdout, which
//! must be empty. The Java half's class comment carries the whole design — what "fixed clock"
//! means, why the git-sha rows re-exec a child process, and which half of the plan's `p8t2` gate
//! lands in Task 4 (this one) versus Task 6 (the end-to-end `e2e` mode).
//!
//! The one asymmetry worth repeating here: **the port has no system properties**, so
//! `resolveGitSha`'s two `System.getProperty` arms become environment lookups of the same names.
//! A row whose input reads `prop:freerouting.git.sha="…"` sets a `-D` on the Java side and an
//! environment variable literally called `freerouting.git.sha` on this one. The single row where
//! that rename is observable is printed as an `XDIFF` on both sides.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use fr_board::Board;
use fr_core::{
    FixtureInfo, PhaseDetail, PhaseMetrics, RoutingJob, RoutingJobState, RoutingResultManifest,
    SessionId, resolve_git_sha, sha256_hex,
};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::score::BoardStatistics;

/// `P8T2.FIXED_INSTANT`.
const FIXED_INSTANT: &str = "1970-01-01T00:00:00Z";
/// `P8T2.FIXED_GIT_SHA`.
const FIXED_GIT_SHA: &str = "0000000";
/// `P8T2.NORMALIZED`.
const NORMALIZED: &str = "\"<normalized>\"";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("shape");
    if mode == "gitsha-child" {
        println!("{}", resolve_git_sha());
        return;
    }
    assert_eq!(
        mode, "shape",
        "usage: p8t2 [shape] <fixturesDir> <scratchDir>  (the e2e mode is Task 6's)"
    );

    let fixtures = PathBuf::from(&args[1]);
    let scratch = PathBuf::from(&args[2]);
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).expect("create the scratch directory");

    println!("# p8t2 shape — RoutingResultManifest (core/results/RoutingResultManifest.java)");
    manifest_table(&fixtures, &scratch);
    duration_table();
    git_sha_table();
    sha256_table(&fixtures, &scratch);
    write_table(&scratch);
    norm_table(&fixtures, &scratch);
}

// =================================================================================================
// [man]
// =================================================================================================

fn manifest_table(fixtures: &Path, scratch: &Path) {
    println!("[man]");

    emit("default", &RoutingResultManifest::default());

    emit("no_input", &pinned(from_job(&fresh_job(), None, false, 1, None)));

    let dsn = fixtures.join("empty_board.dsn");
    emit(
        "input_present",
        &pinned(from_job(&fresh_job(), Some(&dsn), true, 0, None)),
    );

    emit(
        "input_missing",
        &pinned(from_job(
            &fresh_job(),
            Some(&scratch.join("no-such-file.dsn")),
            false,
            1,
            None,
        )),
    );

    emit(
        "input_directory",
        &pinned(from_job(&fresh_job(), Some(fixtures), false, 1, None)),
    );

    // `fromJob:107` dereferences `Path.of("/").getFileName()`, which is null for the root; the
    // port totalises it to the empty string. Both sides print this literal row.
    println!("MAN root_input 000 XDIFF java=NullPointerException rust=fixture.filename=\"\"");

    let mut passes = fresh_job();
    passes.set_current_pass(7);
    emit(
        "passes_7",
        &pinned(from_job(&passes, Some(&dsn), true, 0, None)),
    );

    let mut zero_pass = fresh_job();
    zero_pass.set_current_pass(0);
    emit(
        "passes_0",
        &pinned(from_job(&zero_pass, Some(&dsn), true, 0, None)),
    );

    let mut timed = fresh_job();
    let epoch = Instant::now();
    timed.started_at = Some(epoch);
    timed.finished_at = Some(epoch + Duration::from_millis(1234));
    timed.set_current_pass(3);
    timed.state = RoutingJobState::Completed;
    emit(
        "completed_run",
        &pinned(from_job(&timed, Some(&dsn), true, 0, None)),
    );

    let mut half_clock = fresh_job();
    half_clock.started_at = Some(epoch);
    emit(
        "started_only",
        &pinned(from_job(&half_clock, Some(&dsn), false, 1, None)),
    );

    let mut timed_out = fresh_job();
    timed_out.state = RoutingJobState::TimedOut;
    emit(
        "timed_out",
        &pinned(from_job(&timed_out, Some(&dsn), true, 0, None)),
    );

    let mut with_path = fresh_job();
    with_path.router_settings.result_json_path =
        Some(scratch.join("result.json").to_string_lossy().into_owned());
    emit(
        "result_json_path",
        &strip_scratch(
            pinned(from_job(&with_path, Some(&dsn), true, 0, None)),
            scratch,
        ),
    );

    // ── the three rows with a real board ────────────────────────────────────────────────────
    //
    // The weights are fixed literals rather than `DefaultSettings`': `ScoringSettings::default()`
    // leaves every weight `None`, exactly as `new ScoringSettings()` leaves every one `null`, and
    // `maximum_score` then panics where Java throws (quirk #258).
    let mut board = load_board(&dsn);
    let stats = BoardStatistics::new(&mut board);
    let mut boarded = with_weights(fresh_job());
    boarded.state = RoutingJobState::Completed;
    boarded.started_at = Some(epoch);
    boarded.finished_at = Some(epoch + Duration::from_millis(2500));
    boarded.set_current_pass(1);
    emit(
        "empty_board",
        &pinned(from_job(&boarded, Some(&dsn), true, 0, Some(&stats))),
    );

    let mut no_scoring = fresh_job();
    no_scoring.router_settings.scoring = None;
    emit(
        "board_without_scoring",
        &pinned(from_job(&no_scoring, Some(&dsn), true, 0, Some(&stats))),
    );

    let routed = fixtures.join("Issue103-Board-Routed.dsn");
    let mut routed_board = load_board(&routed);
    let routed_stats = BoardStatistics::new(&mut routed_board);
    // A second weight set — see `P8T2.manifestTable`'s comment: `with_weights`' trace cost makes
    // a routed board's score negative, and `java_max_f32` then clamps it to the same 0 the guard
    // produces. These weights leave the division as the only thing deciding the value.
    let mut connected = fresh_job();
    {
        let scoring = connected
            .router_settings
            .scoring
            .as_mut()
            .expect("RouterSettings::new allocates scoring");
        scoring.unrouted_net_penalty = Some(10.0);
        scoring.clearance_violation_penalty = Some(5.0);
        scoring.bend_penalty = Some(0.5);
        scoring.default_preferred_direction_trace_cost = Some(0.0);
        scoring.via_costs = Some(1);
    }
    connected.state = RoutingJobState::Completed;
    connected.set_current_pass(2);
    emit(
        "connected_board",
        &pinned(from_job(
            &connected,
            Some(&routed),
            true,
            0,
            Some(&routed_stats),
        )),
    );
}

/// `P8T2.withWeights`.
fn with_weights(mut job: RoutingJob) -> RoutingJob {
    let scoring = job
        .router_settings
        .scoring
        .as_mut()
        .expect("RouterSettings::new allocates scoring");
    scoring.unrouted_net_penalty = Some(10.0);
    scoring.clearance_violation_penalty = Some(5.0);
    scoring.bend_penalty = Some(0.5);
    scoring.default_preferred_direction_trace_cost = Some(1.0);
    scoring.via_costs = Some(42);
    job
}

/// `P8T2.fromJob`, with the injected clock this port needs and the statistics scan ruling R20
/// moved out of the job.
fn from_job(
    job: &RoutingJob,
    input: Option<&Path>,
    output_written: bool,
    exit_code: i32,
    stats: Option<&BoardStatistics>,
) -> RoutingResultManifest {
    RoutingResultManifest::from_job(
        job,
        input,
        output_written,
        exit_code,
        &|| FIXED_INSTANT.to_string(),
        stats,
    )
}

/// `P8T2.pinned`. The Rust side injects `generated_at` through `from_job`'s clock, so only
/// `git_sha` is overwritten here; the effect is the same two literals.
fn pinned(mut manifest: RoutingResultManifest) -> RoutingResultManifest {
    manifest.generated_at = Some(FIXED_INSTANT.to_string());
    manifest.git_sha = Some(FIXED_GIT_SHA.to_string());
    manifest
}

/// `P8T2.stripScratch`.
fn strip_scratch(mut manifest: RoutingResultManifest, scratch: &Path) -> RoutingResultManifest {
    if let Some(settings) = manifest.settings_snapshot.as_mut() {
        if let Some(path) = settings.result_json_path.as_mut() {
            *path = path.replace(&*scratch.to_string_lossy(), "<SCRATCH>");
        }
    }
    manifest
}

/// `P8T2.emit`.
fn emit(label: &str, manifest: &RoutingResultManifest) {
    let json = manifest
        .to_gson_string()
        .expect("no non-finite float in a manifest built from a corpus board");
    for (index, line) in json.split('\n').enumerate() {
        println!("MAN {label} {index:03} {}", escape(line));
    }
    println!("MAN {label} len {}", json.len());
}

/// `P8T2.freshJob`.
fn fresh_job() -> RoutingJob {
    RoutingJob::new(SessionId::NIL)
}

/// `P8T2.loadBoard`.
fn load_board(dsn: &Path) -> Board {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    let design_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    match fr_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

// =================================================================================================
// [dur]
// =================================================================================================

fn duration_table() {
    println!("[dur]");
    let pairs: [(u64, u64); 12] = [
        (0, 0),
        (0, 1),
        (0, 3),
        (0, 33),
        (0, 100),
        (0, 999),
        (0, 1000),
        (0, 1234),
        (0, 3661500),
        (0, 86400000),
        (0, 86399999),
        (5, 1239),
    ];
    let epoch = Instant::now();
    for (start, finish) in pairs {
        let started = epoch + Duration::from_millis(start);
        let finished = epoch + Duration::from_millis(finish);
        let millis = finished.saturating_duration_since(started).as_millis();
        let seconds = (millis as f64 / 1000.0) as f32;
        println!(
            "DUR {start}->{finish} {}",
            fr_dsn::java_float_to_string(seconds)
        );
    }
    println!("DUR 1234->0 XDIFF java=-1.234 rust=unreachable(monotonic clock)");
}

// =================================================================================================
// [gitsha]
// =================================================================================================

fn git_sha_table() {
    println!("[gitsha]");
    git_sha("none", None, None, None);
    git_sha("env", Some("deadbeef"), None, None);
    git_sha("env_padded", Some("  deadbeef  "), None, None);
    git_sha("env_tab_newline", Some("\tdeadbeef\n"), None, None);
    git_sha("env_empty", Some(""), None, None);
    git_sha("env_spaces", Some("   "), None, None);
    git_sha("prop", None, Some("cafebabe"), None);
    git_sha("prop_padded", None, Some(" cafebabe "), None);
    git_sha("env_beats_prop", Some("deadbeef"), Some("cafebabe"), None);
    git_sha("blank_env_falls_to_prop", Some("   "), Some("cafebabe"), None);
    git_sha("legacy_prop_only", None, None, Some("0badc0de"));
    git_sha("file_separators_only", Some("\u{1c}\u{1d}\u{1e}\u{1f}"), None, None);
    git_sha("file_separators_around", Some("\u{1c}deadbeef\u{1f}"), None, None);
    git_sha("nbsp_only", Some("\u{a0}"), None, None);
    git_sha("figure_space_only", Some("\u{2007}"), None, None);
    git_sha("narrow_nbsp_only", Some("\u{202f}"), None, None);
    git_sha("ideographic_space_only", Some("\u{3000}"), None, None);
    git_sha("nbsp_around", Some("\u{a0}deadbeef\u{a0}"), None, None);
    println!(
        "GITSHA legacy_prop_beats_blank_env XDIFF java=0badc0de rust=unknown \
         (resolveGitSha:156-159 renames onto arm 1's variable)"
    );
    // The rename's second consequence: Java's legacy property is the LAST arm and loses to
    // `freerouting.git.sha`; here it has become the FIRST arm's environment variable and wins.
    println!(
        "GITSHA prop_beats_legacy_prop XDIFF java=cafebabe rust=0badc0de \
         (arm 3 renames onto arm 1, which outranks arm 2)"
    );
}

/// `P8T2.gitSha`. The Java half spawns a JVM with the row's `-D`s; this spawns **this same
/// binary** in `gitsha-child` mode with the row's environment. Neither side mutates its own
/// environment, and the two `System.getProperty` arms become environment variables of the same
/// literal names — the rename `resolve_git_sha`'s `// renamed:` marker records.
fn git_sha(label: &str, env: Option<&str>, prop: Option<&str>, legacy_prop: Option<&str>) {
    let exe = std::env::current_exe().expect("this binary's own path");
    let mut command = Command::new(exe);
    command.arg("gitsha-child");
    command.env_remove("FREEROUTING_GIT_SHA");
    command.env_remove("freerouting.git.sha");
    if let Some(value) = env {
        command.env("FREEROUTING_GIT_SHA", value);
    }
    if let Some(value) = prop {
        command.env("freerouting.git.sha", value);
    }
    if let Some(value) = legacy_prop {
        // Java's third arm is `System.getProperty("FREEROUTING_GIT_SHA")`, which renames onto the
        // *first* arm's environment variable — see this module's docs and the `// not reachable:`
        // marker on `resolve_git_sha`.
        command.env("FREEROUTING_GIT_SHA", value);
    }
    let output = command.output().expect("the child runs");
    let mut answer = String::from_utf8(output.stdout).expect("the child writes UTF-8");
    if answer.ends_with('\n') {
        answer.pop();
    }
    println!(
        "GITSHA {label} in={} out={}",
        describe_sources(env, prop, legacy_prop),
        escape(&answer)
    );
}

/// `P8T2.describeSources`.
fn describe_sources(env: Option<&str>, prop: Option<&str>, legacy_prop: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(value) = env {
        parts.push(format!("env:FREEROUTING_GIT_SHA=\"{}\"", escape(value)));
    }
    if let Some(value) = prop {
        parts.push(format!("prop:freerouting.git.sha=\"{}\"", escape(value)));
    }
    if let Some(value) = legacy_prop {
        parts.push(format!("prop:FREEROUTING_GIT_SHA=\"{}\"", escape(value)));
    }
    if parts.is_empty() {
        return "(nothing)".to_string();
    }
    parts.join("|")
}

// =================================================================================================
// [sha256]
// =================================================================================================

fn sha256_table(fixtures: &Path, scratch: &Path) {
    println!("[sha256]");

    let empty = scratch.join("sha-empty.bin");
    std::fs::write(&empty, b"").expect("write");
    sha256("empty_file", &empty);

    let abc = scratch.join("sha-abc.bin");
    std::fs::write(&abc, b"abc").expect("write");
    sha256("nist_abc", &abc);

    let two_blocks = scratch.join("sha-two-blocks.bin");
    std::fs::write(
        &two_blocks,
        b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    )
    .expect("write");
    sha256("nist_two_blocks", &two_blocks);

    let million = scratch.join("sha-million-a.bin");
    std::fs::write(&million, vec![b'a'; 1_000_000]).expect("write");
    sha256("nist_million_a", &million);

    for length in [55usize, 56, 63, 64, 65] {
        let path = scratch.join(format!("sha-{length}a.bin"));
        std::fs::write(&path, vec![b'a'; length]).expect("write");
        sha256(&format!("block_boundary_{length}"), &path);
    }

    sha256("fixture_empty_board", &fixtures.join("empty_board.dsn"));
    sha256(
        "fixture_issue026_ses",
        &fixtures.join("Issue026-J2_reference.ses"),
    );
    sha256("missing", &scratch.join("no-such-file.bin"));
    sha256("directory", fixtures);
}

fn sha256(label: &str, path: &Path) {
    let answer = sha256_hex(path);
    println!(
        "SHA256 {label} {}",
        answer.unwrap_or_else(|| "(null->key omitted)".to_string())
    );
}

// =================================================================================================
// [write]
// =================================================================================================

fn write_table(scratch: &Path) {
    println!("[write]");
    let manifest = pinned(from_job(&fresh_job(), None, false, 1, None));
    let expected = manifest.to_gson_string().expect("a finite manifest");

    let flat = scratch.join("manifest.json");
    RoutingResultManifest::write(&flat, &manifest).expect("write");
    report("existing_parent", &flat, &expected);

    let nested = scratch.join("a/b/c/manifest.json");
    RoutingResultManifest::write(&nested, &manifest).expect("write");
    report("created_parents", &nested, &expected);

    RoutingResultManifest::write(&flat, &manifest).expect("write");
    report("overwritten", &flat, &expected);

    println!("WRITE bare_filename parent={}", java_parent("manifest.json"));
    println!(
        "WRITE nested_parent parent={}",
        java_parent(&nested.to_string_lossy()).replace(&*scratch.to_string_lossy(), "<SCRATCH>")
    );
    println!("WRITE root_parent parent={}", java_parent("/"));
}

/// `Path.of(s).getParent()`, spelt as Java's `String.valueOf(Object)` would print it.
///
/// `fr_core`'s own `java_path` module is `pub(crate)`, so the two rules this driver needs are
/// re-derived here: a bare filename has no parent, and neither does the root.
fn java_parent(path: &str) -> String {
    let normalized = {
        let absolute = path.starts_with('/');
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            if absolute {
                "/".to_string()
            } else {
                String::new()
            }
        } else if absolute {
            format!("/{}", segments.join("/"))
        } else {
            segments.join("/")
        }
    };
    match normalized.rfind('/') {
        None => "null".to_string(),
        Some(0) if normalized.len() == 1 => "null".to_string(),
        Some(0) => "/".to_string(),
        Some(index) => normalized[..index].to_string(),
    }
}

fn report(label: &str, path: &Path, expected: &str) {
    let actual = std::fs::read(path).expect("read back");
    let last = actual
        .last()
        .map(|byte| escape(&(*byte as char).to_string()))
        .unwrap_or_else(|| "(none)".to_string());
    println!(
        "WRITE {label} exists={} bytes={} matches_toJson={} last_byte={}",
        path.exists(),
        actual.len(),
        actual == expected.as_bytes(),
        last
    );
}

// =================================================================================================
// [norm]
// =================================================================================================

fn norm_table(fixtures: &Path, scratch: &Path) {
    println!("[norm]");
    let mut job = fresh_job();
    let started = Instant::now();
    job.started_at = Some(started);
    job.finished_at = Some(started + Duration::from_millis(777));
    job.set_current_pass(4);
    job.state = RoutingJobState::Completed;
    job.router_settings.result_json_path =
        Some(scratch.join("result.json").to_string_lossy().into_owned());
    job.resource_usage.cpu_time_used = 1.5;
    job.resource_usage.peak_memory_used = 321.25;

    // Deliberately not `pinned`, and deliberately the real `resolve_git_sha()` `from_job` calls:
    // the normaliser is what makes this reproducible.
    let manifest = RoutingResultManifest::from_job(
        &job,
        Some(&fixtures.join("empty_board.dsn")),
        true,
        0,
        &|| chrono_like_now(),
        None,
    );
    let json = manifest.to_gson_string().expect("a finite manifest");
    let normalized = normalize_manifest(&json, &scratch.to_string_lossy());
    for (index, line) in normalized.split('\n').enumerate() {
        println!("NORM live {index:03} {}", escape(line));
    }
}

/// `Instant.now().toString()` — an ISO-8601 UTC instant. The port has no date library and the
/// value is stripped by the normaliser two lines later, so this is the shape rather than the
/// clock: a fixed prefix and the process's own wall-clock nanoseconds, which is enough to prove
/// the normaliser is doing the stripping rather than the row being constant by construction.
fn chrono_like_now() -> String {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970");
    format!(
        "1970-01-01T00:00:{:02}.{:09}Z",
        since_epoch.as_secs() % 60,
        since_epoch.subsec_nanos()
    )
}

/// `P8T2.normalizeManifest` — the whole contract is documented there; this is the same five rules
/// in the same order, so that Task 6's end-to-end gate compares the two manifests through
/// identical normalisation.
pub fn normalize_manifest(json: &str, absolute_prefix: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    let mut skip_depth = 0i32;
    let mut skipping = false;
    for line in json.split('\n') {
        if skipping {
            skip_depth += count(line, '{') - count(line, '}');
            if skip_depth <= 0 {
                skipping = false;
            }
            continue;
        }
        if line.trim_start().starts_with("\"resource_usage\":") {
            let depth = count(line, '{') - count(line, '}');
            if depth > 0 {
                skipping = true;
                skip_depth = depth;
            }
            continue;
        }
        kept.push(normalize_line(line, absolute_prefix));
    }
    kept.join("\n")
}

fn normalize_line(line: &str, absolute_prefix: &str) -> String {
    for key in ["generated_at", "git_sha", "duration_seconds"] {
        let needle = format!("\"{key}\": ");
        if let Some(at) = line.find(&needle) {
            let indent = &line[..at];
            let comma = if line.ends_with(',') { "," } else { "" };
            return format!("{indent}{needle}{NORMALIZED}{comma}");
        }
    }
    if absolute_prefix.is_empty() {
        line.to_string()
    } else {
        line.replace(absolute_prefix, "<DIR>")
    }
}

fn count(text: &str, wanted: char) -> i32 {
    text.chars().filter(|c| *c == wanted).count() as i32
}

// =================================================================================================
// shared helpers
// =================================================================================================

/// `P8T2.escape`.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if (' '..'\u{7f}').contains(&c) {
            out.push(c);
        } else {
            // Java escapes UTF-16 code units, so a supplementary code point becomes two
            // `\uXXXX` escapes on that side; nothing in this driver's data is outside the BMP.
            for unit in [c].iter().flat_map(|c| {
                let mut buffer = [0u16; 2];
                c.encode_utf16(&mut buffer).to_vec()
            }) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

/// The types the module docs name but the code reaches only through `fr_core`'s re-exports.
#[allow(dead_code)]
fn referenced(_: FixtureInfo, _: PhaseMetrics, _: PhaseDetail) {}
