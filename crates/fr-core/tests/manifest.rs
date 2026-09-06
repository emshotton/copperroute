use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use fr_board::Board;
use fr_core::{
    Ctx, PhaseDetail, PhaseMetrics, RoutingJob, RoutingJobState, RoutingPipeline,
    RoutingResultManifest, SessionId, SyncProgressSink, resolve_git_sha, sha256_hex,
};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::score::BoardStatistics;
use fr_settings::sources::{
    CliSettings, DefaultSettings, DsnFileSettings, EnvironmentVariablesSource,
};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

const FIXED_INSTANT: &str = "1970-01-01T00:00:00Z";
const FIXED_GIT_SHA: &str = "0000000";

struct ManifestRow {
    lines: Vec<String>,
    len: usize,
}

struct Transcript {
    manifests: BTreeMap<String, ManifestRow>,
    durations: Vec<(u64, u64, String)>,
    git_shas: Vec<GitShaRow>,
    sha256s: BTreeMap<String, String>,
    writes: BTreeMap<String, String>,
    norm: Vec<String>,
}

struct GitShaRow {
    label: String,
    env: Option<String>,
    prop: Option<String>,
    legacy_prop: Option<String>,
    answer: String,
}

/// The version string the transcript was cut with; the manifest now stamps the crate's own.
const TRANSCRIPT_VERSION: &str = "2.3.1-SNAPSHOT";

fn transcript_len(recorded: usize) -> usize {
    let delta = fr_core::SERVER_VERSION.len() as isize - TRANSCRIPT_VERSION.len() as isize;
    (recorded as isize + delta) as usize
}

fn transcript() -> Transcript {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/p8t2-manifest-shape.txt"),
    )
    .expect("the committed p8t2 transcript")
    .replace(TRANSCRIPT_VERSION, fr_core::SERVER_VERSION);

    let mut manifests: BTreeMap<String, ManifestRow> = BTreeMap::new();
    let mut durations = Vec::new();
    let mut git_shas = Vec::new();
    let mut sha256s = BTreeMap::new();
    let mut writes = BTreeMap::new();
    let mut norm = Vec::new();

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MAN ") {
            let (label, rest) = rest.split_once(' ').expect("a label");
            if label == "root_input" {
                continue;
            }
            let entry = manifests.entry(label.to_string()).or_insert(ManifestRow {
                lines: Vec::new(),
                len: 0,
            });
            match rest.split_once(' ') {
                Some(("len", value)) => entry.len = value.parse().expect("a byte count"),
                Some((_index, content)) => entry.lines.push(unescape(content)),
                None => entry.lines.push(String::new()),
            }
        } else if let Some(rest) = line.strip_prefix("DUR ") {
            let (pair, value) = rest.split_once(' ').expect("a value");
            if value.starts_with("XDIFF") {
                continue;
            }
            let (start, finish) = pair.split_once("->").expect("a pair");
            durations.push((
                start.parse().expect("a start"),
                finish.parse().expect("a finish"),
                value.to_string(),
            ));
        } else if let Some(rest) = line.strip_prefix("GITSHA ") {
            let (label, rest) = rest.split_once(' ').expect("a label");
            if rest.starts_with("XDIFF") {
                continue;
            }
            let (sources, answer) = rest.split_once(" out=").expect("an answer");
            let sources = sources.strip_prefix("in=").expect("the input column");
            git_shas.push(GitShaRow {
                label: label.to_string(),
                env: source_value(sources, "env:FREEROUTING_GIT_SHA"),
                prop: source_value(sources, "prop:freerouting.git.sha"),
                legacy_prop: source_value(sources, "prop:FREEROUTING_GIT_SHA"),
                answer: unescape(answer),
            });
        } else if let Some(rest) = line.strip_prefix("SHA256 ") {
            let (label, value) = rest.split_once(' ').expect("a value");
            sha256s.insert(label.to_string(), value.to_string());
        } else if let Some(rest) = line.strip_prefix("WRITE ") {
            let (label, value) = rest.split_once(' ').expect("a value");
            writes.insert(label.to_string(), value.to_string());
        } else if let Some(rest) = line.strip_prefix("NORM live ") {
            let (_index, content) = rest.split_once(' ').expect("a line");
            norm.push(unescape(content));
        }
    }

    for row in manifests.values_mut() {
        if row
            .lines
            .iter()
            .any(|line| line.contains(fr_core::SERVER_VERSION))
        {
            row.len = transcript_len(row.len);
        }
    }

    Transcript {
        manifests,
        durations,
        git_shas,
        sha256s,
        writes,
        norm,
    }
}

fn source_value(sources: &str, key: &str) -> Option<String> {
    for part in sources.split('|') {
        if let Some(rest) = part.strip_prefix(key)
            && let Some(value) = rest.strip_prefix("=\"").and_then(|v| v.strip_suffix('"'))
        {
            return Some(unescape(value));
        }
    }
    None
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' || chars.peek() != Some(&'u') {
            out.push(c);
            continue;
        }
        chars.next();
        let hex: String = (0..4)
            .map(|_| chars.next().expect("four hex digits"))
            .collect();
        let unit = u16::from_str_radix(&hex, 16).expect("a hex code unit");
        out.push(char::from_u32(u32::from(unit)).expect("a BMP code point"));
    }
    out
}

fn fixtures() -> PathBuf {
    parity::java_dir().join("fixtures")
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join("fr-core-plan8-task4");
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn fresh_job() -> RoutingJob {
    RoutingJob::new(SessionId::NIL)
}

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

fn from_job(
    job: &RoutingJob,
    input: Option<&Path>,
    output_written: bool,
    exit_code: i32,
    stats: Option<&BoardStatistics>,
) -> RoutingResultManifest {
    let mut manifest = RoutingResultManifest::from_job(
        job,
        input,
        output_written,
        exit_code,
        &|| FIXED_INSTANT.to_string(),
        stats,
    );
    manifest.git_sha = Some(FIXED_GIT_SHA.to_string());
    manifest
}

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

fn cases() -> BTreeMap<String, RoutingResultManifest> {
    let fixtures = fixtures();
    let scratch = scratch();
    let dsn = fixtures.join("empty_board.dsn");
    let epoch = Instant::now();
    let mut out: BTreeMap<String, RoutingResultManifest> = BTreeMap::new();

    out.insert("default".into(), RoutingResultManifest::default());
    out.insert(
        "no_input".into(),
        from_job(&fresh_job(), None, false, 1, None),
    );
    out.insert(
        "input_present".into(),
        from_job(&fresh_job(), Some(&dsn), true, 0, None),
    );
    out.insert(
        "input_missing".into(),
        from_job(
            &fresh_job(),
            Some(&scratch.join("no-such-file.dsn")),
            false,
            1,
            None,
        ),
    );
    out.insert(
        "input_directory".into(),
        from_job(&fresh_job(), Some(&fixtures), false, 1, None),
    );

    let mut passes = fresh_job();
    passes.set_current_pass(7);
    out.insert(
        "passes_7".into(),
        from_job(&passes, Some(&dsn), true, 0, None),
    );

    let mut zero = fresh_job();
    zero.set_current_pass(0);
    out.insert(
        "passes_0".into(),
        from_job(&zero, Some(&dsn), true, 0, None),
    );

    let mut timed = fresh_job();
    timed.started_at = Some(epoch);
    timed.finished_at = Some(epoch + Duration::from_millis(1234));
    timed.set_current_pass(3);
    timed.state = RoutingJobState::Completed;
    out.insert(
        "completed_run".into(),
        from_job(&timed, Some(&dsn), true, 0, None),
    );

    let mut half = fresh_job();
    half.started_at = Some(epoch);
    out.insert(
        "started_only".into(),
        from_job(&half, Some(&dsn), false, 1, None),
    );

    let mut timed_out = fresh_job();
    timed_out.state = RoutingJobState::TimedOut;
    out.insert(
        "timed_out".into(),
        from_job(&timed_out, Some(&dsn), true, 0, None),
    );

    let mut with_path = fresh_job();
    with_path.router_settings.result_json_path =
        Some(scratch.join("result.json").to_string_lossy().into_owned());
    let mut result_json = from_job(&with_path, Some(&dsn), true, 0, None);
    if let Some(settings) = result_json.settings_snapshot.as_mut()
        && let Some(path) = settings.result_json_path.as_mut()
    {
        *path = path.replace(&*scratch.to_string_lossy(), "<SCRATCH>");
    }
    out.insert("result_json_path".into(), result_json);

    let mut board = load_board(&dsn);
    let stats = BoardStatistics::new(&mut board);
    let mut boarded = with_weights(fresh_job());
    boarded.state = RoutingJobState::Completed;
    boarded.started_at = Some(epoch);
    boarded.finished_at = Some(epoch + Duration::from_millis(2500));
    boarded.set_current_pass(1);
    out.insert(
        "empty_board".into(),
        from_job(&boarded, Some(&dsn), true, 0, Some(&stats)),
    );

    let mut no_scoring = fresh_job();
    no_scoring.router_settings.scoring = None;
    out.insert(
        "board_without_scoring".into(),
        from_job(&no_scoring, Some(&dsn), true, 0, Some(&stats)),
    );

    let routed = fixtures.join("Issue103-Board-Routed.dsn");
    let mut routed_board = load_board(&routed);
    let routed_stats = BoardStatistics::new(&mut routed_board);
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
    out.insert(
        "connected_board".into(),
        from_job(&connected, Some(&routed), true, 0, Some(&routed_stats)),
    );

    out
}

fn json_of(manifest: &RoutingResultManifest) -> String {
    manifest
        .to_gson_string()
        .expect("no non-finite float in a corpus manifest")
}

#[test]
fn the_manifest_transcript_replays_row_for_row() {
    let transcript = transcript();
    let cases = cases();
    assert_eq!(
        cases.keys().collect::<Vec<_>>(),
        transcript.manifests.keys().collect::<Vec<_>>(),
        "every transcript case is rebuilt, and no extra ones"
    );
    for (label, manifest) in &cases {
        let expected = &transcript.manifests[label];
        let json = json_of(manifest);
        let actual: Vec<String> = json.split('\n').map(str::to_string).collect();
        assert_eq!(actual, expected.lines, "MAN {label}");
        assert_eq!(json.len(), expected.len, "MAN {label} len");
    }
}

#[test]
fn the_duration_narrowing_matches_the_transcript() {
    let epoch = Instant::now();
    for (start, finish, expected) in transcript().durations {
        let started = epoch + Duration::from_millis(start);
        let finished = epoch + Duration::from_millis(finish);
        let mut job = fresh_job();
        job.started_at = Some(started);
        job.finished_at = Some(finished);
        let manifest = from_job(&job, None, false, 1, None);
        let seconds = manifest
            .phases
            .expect("phases")
            .autorouter
            .expect("autorouter")
            .duration_seconds
            .expect("both instants are set");
        assert_eq!(
            fr_dsn::format_float(seconds),
            expected,
            "DUR {start}->{finish}"
        );
    }
}

#[test]
fn sha256_hex_matches_the_nist_vectors_and_the_corpus() {
    let transcript = transcript();
    let fixtures = fixtures();
    let scratch = scratch();

    let expect = |label: &str, path: &Path| {
        let expected = &transcript.sha256s[label];
        let actual = sha256_hex(path).unwrap_or_else(|| "(null->key omitted)".to_string());
        assert_eq!(&actual, expected, "SHA256 {label}");
    };

    for (label, bytes) in [
        ("empty_file", Vec::new()),
        ("nist_abc", b"abc".to_vec()),
        (
            "nist_two_blocks",
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".to_vec(),
        ),
        ("nist_million_a", vec![b'a'; 1_000_000]),
        ("block_boundary_55", vec![b'a'; 55]),
        ("block_boundary_56", vec![b'a'; 56]),
        ("block_boundary_63", vec![b'a'; 63]),
        ("block_boundary_64", vec![b'a'; 64]),
        ("block_boundary_65", vec![b'a'; 65]),
    ] {
        let path = scratch.join(format!("{label}.bin"));
        std::fs::write(&path, &bytes).expect("write");
        expect(label, &path);
    }

    expect("fixture_empty_board", &fixtures.join("empty_board.dsn"));
    expect(
        "fixture_issue026_ses",
        &fixtures.join("Issue026-J2_reference.ses"),
    );
    expect("missing", &scratch.join("definitely-not-here.bin"));
    expect("directory", &fixtures);
}

#[test]
fn the_key_order_is_declaration_order() {
    let manifest = cases()
        .remove("connected_board")
        .expect("the fullest manifest");
    let json = json_of(&manifest);
    let keys: Vec<&str> = json
        .lines()
        .filter(|line| line.starts_with("  \""))
        .map(|line| {
            line.trim_start()
                .trim_start_matches('"')
                .split('"')
                .next()
                .expect("a key")
        })
        .collect();
    assert_eq!(
        keys,
        [
            "schema_version",
            "generated_at",
            "app_version",
            "git_sha",
            "fixture",
            "settings_snapshot",
            "phases",
            "board_statistics",
            "normalized_score",
            "resource_usage",
            "final_state",
            "exit_code",
            "output_written",
        ],
        "`RoutingResultManifest.java:28-65`, top to bottom — all thirteen keys, because this is \
         the one case where nothing is null"
    );
}

#[test]
fn nulls_are_omitted() {
    let json = json_of(&RoutingResultManifest::default());
    assert_eq!(
        json,
        "{\n  \"schema_version\": 1,\n  \"phases\": {\n    \"fanout\": {},\n    \
         \"autorouter\": {},\n    \"optimizer\": {}\n  },\n  \"exit_code\": 0,\n  \
         \"output_written\": false\n}",
        "ten null fields gone; the three primitives stay"
    );

    let cases = cases();
    let missing = json_of(&cases["input_missing"]);
    assert!(
        missing.contains("\"fixture\": {\n    \"filename\": \"no-such-file.dsn\"\n  },"),
        "{missing}"
    );
}

#[test]
fn fanout_and_optimizer_phases_are_empty_objects() {
    for (label, manifest) in cases() {
        let phases = manifest.phases.expect("phases is always allocated");
        assert_eq!(
            phases.fanout,
            Some(PhaseDetail::default()),
            "phases.fanout on {label}"
        );
        assert_eq!(
            phases.optimizer,
            Some(PhaseDetail::default()),
            "phases.optimizer on {label}"
        );
        let json = json_of(&RoutingResultManifest {
            phases: Some(PhaseMetrics {
                fanout: phases.fanout,
                autorouter: None,
                optimizer: phases.optimizer,
            }),
            ..RoutingResultManifest::default()
        });
        assert!(
            json.contains("\"fanout\": {}") && json.contains("\"optimizer\": {}"),
            "both serialise as `{{}}` — {json}"
        );
    }
}

#[test]
fn the_two_stages_report_their_own_pass_counts() {
    let mut job = fresh_job();
    job.set_current_pass(3);
    job.set_optimizer_pass(1);

    assert_eq!(job.get_current_pass(), 3, "the routing stage's own count");
    assert_eq!(
        job.get_optimizer_pass(),
        1,
        "the optimizer stage's own count"
    );

    let phases = from_job(&job, None, false, 0, None)
        .phases
        .expect("phases is always allocated");
    assert_eq!(
        phases
            .autorouter
            .expect("autorouter is always allocated")
            .passes_completed,
        Some(3),
        "fixed: T9 (#267) — the key that names the autorouter reports the autorouter's passes, \
         not whichever stage announced a pass last"
    );
    assert_eq!(
        phases.optimizer.expect("optimizer is always allocated"),
        PhaseDetail::default(),
        "phases.optimizer stays an empty object until Task 20 writes it (quirk #254); the \
         number it will be written from is on the job already"
    );

    let mut shared = fresh_job();
    shared.set_current_pass(3);
    shared.set_optimizer_pass(1);
    assert_ne!(
        shared.get_current_pass(),
        shared.get_optimizer_pass(),
        "the two writes must not land in the same field"
    );
}

#[test]
fn autorouter_duration_is_the_whole_job() {
    let epoch = Instant::now();
    let mut job = fresh_job();
    job.started_at = Some(epoch);
    job.finished_at = Some(epoch + Duration::from_millis(1234));
    let manifest = from_job(&job, None, false, 1, None);
    let phases = manifest.phases.expect("phases");
    assert_eq!(
        phases
            .autorouter
            .expect("autorouter")
            .duration_seconds
            .expect("both instants set"),
        1.234,
        "the job's whole 1 234 ms, in the autorouter's slot"
    );
    assert_eq!(phases.fanout, Some(PhaseDetail::default()));
    assert_eq!(phases.optimizer, Some(PhaseDetail::default()));

    let mut half = fresh_job();
    half.started_at = Some(epoch);
    let manifest = from_job(&half, None, false, 1, None);
    assert_eq!(
        manifest
            .phases
            .expect("phases")
            .autorouter
            .expect("autorouter"),
        PhaseDetail::default(),
        "`startedAt` alone leaves the whole PhaseDetail empty"
    );
}

#[test]
fn a_connectionless_board_scores_zero_not_nan() {
    let dsn = fixtures().join("empty_board.dsn");
    let mut board = load_board(&dsn);
    let stats = BoardStatistics::new(&mut board);
    assert_eq!(
        stats.connections.maximum_count,
        Some(0),
        "empty_board.dsn has no connections at all"
    );

    let job = with_weights(fresh_job());
    let scoring = job.router_settings.scoring.clone().expect("scoring");
    assert_eq!(stats.maximum_score(&scoring), 0.0);
    let score = stats.normalized_score(&scoring);
    assert!(score.is_finite(), "not NaN: {score}");
    assert_eq!(score, 0.0);

    let manifest = from_job(&job, Some(&dsn), true, 0, Some(&stats));
    assert_eq!(manifest.normalized_score, Some(0.0));
    assert!(json_of(&manifest).contains("\"normalized_score\": 0"));

    let mut no_scoring = fresh_job();
    no_scoring.router_settings.scoring = None;
    let manifest = from_job(&no_scoring, Some(&dsn), true, 0, Some(&stats));
    assert_eq!(manifest.normalized_score, None);
    assert!(manifest.board_statistics.is_some());
    assert!(!json_of(&manifest).contains("normalized_score"));
}

#[test]
fn an_unreadable_input_omits_the_sha256_key() {
    let scratch = scratch();
    let missing = scratch.join("no-such-file.dsn");
    assert_eq!(sha256_hex(&missing), None);
    assert_eq!(sha256_hex(&fixtures()), None);

    let manifest = from_job(&fresh_job(), Some(&missing), false, 1, None);
    let fixture = manifest
        .fixture
        .clone()
        .expect("fixture is allocated at :104");
    assert_eq!(fixture.filename.as_deref(), Some("no-such-file.dsn"));
    assert_eq!(fixture.sha256, None);
    let json = json_of(&manifest);
    assert!(
        !json.contains("sha256"),
        "the key is gone entirely — {json}"
    );

    let none = from_job(&fresh_job(), None, false, 1, None);
    assert_eq!(none.fixture, Some(fr_core::FixtureInfo::default()));
    assert!(json_of(&none).contains("\"fixture\": {},"));
}

/// `#![forbid(unsafe_code)]`, so each row spawns **this test binary** in the child mode below —
#[test]
fn resolve_git_sha_walks_all_three_sources_then_unknown() {
    let scratch = scratch();
    let rows = transcript().git_shas;
    assert!(rows.len() >= 18, "the transcript carries the whole ladder");
    for row in rows {
        let out = scratch.join(format!("gitsha-{}.txt", row.label));
        let _ = std::fs::remove_file(&out);
        let mut command = Command::new(std::env::current_exe().expect("this test binary"));
        command.args(["--exact", "resolve_git_sha_child"]);
        command.stdout(std::process::Stdio::null());
        command.stderr(std::process::Stdio::null());
        command.env("FR_GITSHA_OUT", &out);
        command.env_remove("FREEROUTING_GIT_SHA");
        command.env_remove("freerouting.git.sha");
        if let Some(value) = &row.env {
            command.env("FREEROUTING_GIT_SHA", value);
        }
        if let Some(value) = &row.prop {
            command.env("freerouting.git.sha", value);
        }
        if let Some(value) = &row.legacy_prop {
            command.env("FREEROUTING_GIT_SHA", value);
        }
        let status = command.status().expect("the child runs");
        assert!(status.success(), "GITSHA {} child failed", row.label);
        let answer = std::fs::read_to_string(&out).expect("the child wrote its answer");
        assert_eq!(answer, row.answer, "GITSHA {}", row.label);
    }
}

#[test]
fn resolve_git_sha_child() {
    if let Ok(path) = std::env::var("FR_GITSHA_OUT") {
        std::fs::write(path, resolve_git_sha()).expect("write the answer back");
    }
}

#[test]
fn no_trailing_newline() {
    let transcript = transcript();
    let manifest = from_job(&fresh_job(), None, false, 1, None);
    let path = scratch().join("no-trailing-newline.json");
    RoutingResultManifest::write(&path, &manifest).expect("write");
    let bytes = std::fs::read(&path).expect("read back");
    assert_eq!(
        bytes.last(),
        Some(&b'}'),
        "the last byte is the closing brace"
    );
    assert_eq!(bytes, json_of(&manifest).into_bytes());
    assert!(
        transcript.writes["existing_parent"].contains("last_byte=}"),
        "and the jar agrees"
    );
    assert!(
        transcript.writes["existing_parent"].contains("bytes=519"),
        "the jar's own byte count"
    );
    assert_eq!(
        bytes.len(),
        transcript_len(519),
        "and the port writes exactly as many, less the version string's length difference"
    );
}

#[test]
fn the_parent_directory_is_created() {
    let transcript = transcript();
    let root = scratch().join("created-parents");
    let _ = std::fs::remove_dir_all(&root);
    let nested = root.join("a/b/c/manifest.json");
    assert!(!nested.parent().expect("a parent").exists());

    let manifest = from_job(&fresh_job(), None, false, 1, None);
    RoutingResultManifest::write(&nested, &manifest).expect("write");
    assert!(nested.exists());
    assert_eq!(
        std::fs::read(&nested).expect("read"),
        json_of(&manifest).into_bytes()
    );

    assert_eq!(transcript.writes["bare_filename"], "parent=null");
    assert_eq!(transcript.writes["root_parent"], "parent=null");
    assert_eq!(transcript.writes["nested_parent"], "parent=<SCRATCH>/a/b/c");
}

#[test]
fn the_pipelines_cached_statistics_equal_a_fresh_recompute() {
    let dsn = fixtures().join("Issue143-rpi_splitter.dsn");
    let bytes = std::fs::read(&dsn).expect("the stem is readable");
    let file_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let mut board = load_board(&dsn);

    let argv = [
        "-de".to_string(),
        dsn.display().to_string(),
        "-mp".to_string(),
        "2".to_string(),
        "--router.fanout.enabled=true".to_string(),
        "--router.optimizer.enabled=true".to_string(),
    ];
    let dsn_source = DsnFileSettings::new(&bytes[..], &file_name);
    let env_map: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let env_source = EnvironmentVariablesSource::new(&env_map);
    let cli_source = CliSettings::new(&argv);
    let inputs = SettingsInputs {
        json_file: None,
        dsn: dsn_source.get_settings(),
        cli_rules: None,
        scheduler_rules: None,
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };
    let settings = resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());
    fr_core::prepare_board(&mut board, &settings);

    let sink = SyncProgressSink::noop();
    let ctx = Ctx::with_disabled_budget(&settings, &sink);
    let result = RoutingPipeline::run(&mut board, &ctx).expect("the stem has a routable layer");
    assert!(
        result.pipeline.router_passes_completed > 0,
        "the router must actually have run, or the comparison is about an unrouted board"
    );

    let cached = result.pipeline.final_statistics.clone();
    let fresh = BoardStatistics::new(&mut board);
    assert_eq!(
        cached, fresh,
        "PipelineResult::final_statistics is not what `new BoardStatistics(job.board)` would          answer for the same board — `from_job`'s cached `stats` would change the manifest"
    );

    let job = with_weights(fresh_job());
    assert_eq!(
        json_of(&from_job(&job, Some(&dsn), true, 0, Some(&cached))),
        json_of(&from_job(&job, Some(&dsn), true, 0, Some(&fresh))),
        "the manifest built from the cached statistics differs from the recomputed one"
    );
    let json = json_of(&from_job(&job, Some(&dsn), true, 0, Some(&cached)));
    assert!(json.contains("\"board_statistics\""), "{json}");
    assert!(json.contains("\"normalized_score\""), "{json}");
}

#[test]
fn the_normaliser_strips_exactly_the_five_irreproducible_things() {
    let norm = transcript().norm;
    let text = norm.join("\n");
    assert!(
        text.contains("\"generated_at\": \"<normalized>\""),
        "{text}"
    );
    assert!(text.contains("\"git_sha\": \"<normalized>\""), "{text}");
    assert!(
        text.contains("\"duration_seconds\": \"<normalized>\""),
        "{text}"
    );
    assert!(
        text.contains("\"result_json\": \"<DIR>/result.json\""),
        "{text}"
    );
    assert!(
        !text.contains("resource_usage") && !text.contains("io_read"),
        "the whole object is removed, not blanked — {text}"
    );
    assert!(text.contains("\"passes_completed\": 4"), "{text}");
    assert!(text.contains("\"final_state\": \"COMPLETED\""), "{text}");
    assert!(
        text.contains(&format!("\"app_version\": \"{}\"", fr_core::SERVER_VERSION)),
        "{text}"
    );
    serde_json::from_str::<serde_json::Value>(&text).expect("still valid JSON");
}

#[test]
fn app_version_is_the_crate_version() {
    let manifest = from_job(&fresh_job(), None, false, 1, None);
    assert_eq!(
        manifest.app_version.as_deref(),
        Some(fr_core::SERVER_VERSION)
    );
    assert_eq!(
        manifest.app_version.as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_filesystem_root_as_an_input_is_totalized() {
    let manifest = from_job(&fresh_job(), Some(Path::new("/")), false, 1, None);
    let fixture = manifest.fixture.clone().expect("fixture");
    assert_eq!(fixture.filename.as_deref(), Some(""));
    assert_eq!(fixture.sha256, None, "the root is a directory");
    let transcript = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/p8t2-manifest-shape.txt"),
    )
    .expect("the transcript");
    assert!(
        transcript.contains(
            "MAN root_input 000 XDIFF java=NullPointerException rust=fixture.filename=\"\""
        ),
        "and the jar throws where the port answers"
    );
}

#[test]
fn the_java_unit_tests_assertions_hold() {
    let dsn = fixtures().join("Issue143-rpi_splitter.dsn");
    let mut board = load_board(&dsn);
    let stats = BoardStatistics::new(&mut board);
    let layer_count = board.get_layer_count() as i32;

    let mut job = fresh_job();
    job.router_settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    job.state = RoutingJobState::Completed;
    job.resource_usage.cpu_time_used = 1.5;
    job.resource_usage.peak_memory_used = 128.0;

    let input = scratch().join("input.dsn");
    std::fs::copy(&dsn, &input).expect("copy the fixture");
    let manifest = RoutingResultManifest::from_job(
        &job,
        Some(&input),
        true,
        0,
        &|| FIXED_INSTANT.to_string(),
        Some(&stats),
    );
    let out = scratch().join("java-test-result.json");
    RoutingResultManifest::write(&out, &manifest).expect("write");

    let json = std::fs::read_to_string(&out).expect("read back");
    let root: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(root["schema_version"].as_i64(), Some(1));
    assert!(!root["app_version"].is_null());
    assert!(!root["git_sha"].is_null());
    assert!(!root["fixture"].is_null());
    assert!(!root["board_statistics"].is_null());
    assert!(!root["resource_usage"].is_null());
    assert_eq!(root["final_state"].as_str(), Some("COMPLETED"));
    assert_eq!(root["exit_code"].as_i64(), Some(0));
    assert_eq!(root["output_written"].as_bool(), Some(true));

    assert_eq!(
        root["board_statistics"]["layers"]["total_count"].as_i64(),
        Some(i64::from(layer_count))
    );
    assert_eq!(root["resource_usage"]["cpu_time"].as_f64(), Some(1.5));
    assert_eq!(root["resource_usage"]["peak_memory"].as_f64(), Some(128.0));
}

#[test]
fn resource_usage_writes_all_five_fields_including_the_two_dead_ones() {
    let json = json_of(&from_job(&fresh_job(), None, false, 1, None));
    assert!(
        json.contains(
            "\"resource_usage\": {\n    \"cpu_time\": 0,\n    \"max_memory\": 0,\n    \
             \"peak_memory\": 0,\n    \"io_read\": 0,\n    \"io_written\": 0\n  },"
        ),
        "{json}"
    );
    assert!(
        transcript().manifests["completed_run"]
            .lines
            .iter()
            .any(|line| line.contains("\"io_written\": 0.0")),
        "and that is what the jar wrote"
    );
}
