//! `core.results.RoutingResultManifest` (`core/results/RoutingResultManifest.java`, 172 lines) —
//! the machine-readable summary a headless CLI run writes when `--router.result_json=<path>` is
//! given, plus `core.RouterJobResourceUsage` (`core/RouterJobResourceUsage.java`, 28 lines).
//!
//! Its one production caller is `Freerouting.writeCliResultManifestIfRequested`
//! (`Freerouting.java:203-241`), which builds it from the finished job and writes it beside the
//! SES. Nothing reads it back inside freerouting: it exists for benchmark and autopilot
//! harnesses, which is why every field is `@SerializedName`d and why the shape is a parity
//! surface in its own right.
//!
//! # The JSON is Gson's, so declaration order **is** key order
//!
//! `write` (`:138-144`) calls `GsonProvider.GSON.toJson(manifest)`, the same instance
//! [`crate::stats_json`] and `fr_settings::json` write through: pretty-printed with a two-space
//! indent, `": "` after every key, no trailing newline, `Number.toString()` numbers, `U+2028` and
//! `U+2029` escaped, **no** `serializeNulls()` and **no**
//! `serializeSpecialFloatingPointValues()`. Two consequences that the types below encode:
//!
//! 1. **Key order is Java's field declaration order** — `schema_version`, `generated_at`,
//!    `app_version`, `git_sha`, `fixture`, `settings_snapshot`, `phases`, `board_statistics`,
//!    `normalized_score`, `resource_usage`, `final_state`, `exit_code`, `output_written`
//!    (`:28-65`). serde's derived `Serialize` walks a struct in declaration order too, so the
//!    Rust field order below is load-bearing and must not be "tidied".
//! 2. **A `null` field is omitted, a primitive one is not.** `schemaVersion` (`int`), `exitCode`
//!    (`int`) and `outputWritten` (`boolean`) are Java primitives and are therefore always
//!    written; every other field is a reference type, is `null`-able, and disappears when null.
//!    `#[serde(skip_serializing_if = "Option::is_none")]` on the `Option` fields is that rule.
//!
//! # Why this module serialises through serde and `to_gson_string_pretty`, not by hand
//!
//! Task 2's controller note warns that `serde_json::Value` loses both Gson's key order (its `Map`
//! is a `BTreeMap` without `preserve_order`) and `Float.toString`'s shorter text (`f32` widens to
//! `f64`). Both losses are properties of the **`Value` tree**, not of serde: a derived
//! `Serialize` streams fields in declaration order straight into
//! [`fr_dsn::format::json::to_gson_string_pretty`], whose [`JavaNumberFormatter`] writes an `f32`
//! through `Float.toString`. So the manifest's own shell is a derived `Serialize`, exactly as the
//! brief's sketch asked, and the one subtree that serde could not carry by itself —
//! `board_statistics` — goes through [`crate::GsonBoardStatistics`] with `serialize_with`. The
//! `board_statistics` key order and float widths are therefore Task 2's, unchanged.
//!
//! [`JavaNumberFormatter`]: fr_dsn::format::json::JavaNumberFormatter
//!
//! # `sha256Hex` and the absent dependency
//!
//! `:163-171` is `MessageDigest.getInstance("SHA-256")` plus `HexFormat.of().formatHex`. The
//! Global Constraint on this port forbids adding a dependency, and `Cargo.lock` carries no
//! SHA-256 implementation (checked at port time: the lock's crates are `aho-corasick`,
//! `anstream`… `zmij`, with no `sha2`, `ring`, `digest` or `openssl`). The FIPS 180-4 core is
//! therefore written out below, in about sixty lines, and pinned against the NIST vectors.
//!
//! # What is deliberately not here
//!
// not ported: the manifest's **read** side (core/results/RoutingResultManifest.java:25-26) — the no-arg constructor exists so Gson can deserialize a manifest, and `grep -rn "RoutingResultManifest.class" src/main/java src/test/java` finds exactly one reader: `src/test/java/…/RoutingResultManifestTest.java:83`'s round-trip assertion. **Nothing in `src/main` ever reads a manifest back** — `Freerouting.writeCliResultManifestIfRequested:229-243` is the only production site and it only writes. The port therefore derives `Serialize` and not `Deserialize`, which is also the only choice available without changing `fr-router`: `board_statistics` is `fr_router::score::BoardStatistics`, whose Gson shape this crate reads through [`crate::GsonBoardStatistics`]' hand-written `Serialize` and which has no `Deserialize` at all. That test's other seven assertions ARE ported, as `crates/fr-core/tests/manifest.rs::the_java_unit_tests_assertions_hold` (ruling 12); the round-trip half is what this marker covers.
//!
//! [`RoutingResultManifest::default`] is `new RoutingResultManifest()` for the write side, which
//! is the only side that runs. Its Java field initialisers are reproduced there, not derived: `schemaVersion` starts at
//! `SCHEMA_VERSION` and `phases` at `new PhaseMetrics()` (whose own initialisers allocate three
//! `PhaseDetail`s), so a manifest that was never filled still serialises
//! `{"schema_version": 1, "phases": {"fanout": {}, "autorouter": {}, "optimizer": {}},
//! "exit_code": 0, "output_written": false}`.

use std::path::Path;

use serde::{Serialize, Serializer};

use fr_dsn::format::json::to_gson_string_pretty;
use fr_router::score::BoardStatistics;

use crate::PARITY_VERSION;
use crate::job::{RoutingJob, java_path};
use crate::stats_json::GsonBoardStatistics;

// =================================================================================================
// RouterJobResourceUsage — `core/RouterJobResourceUsage.java`
// =================================================================================================

/// Port of `core.RouterJobResourceUsage` (`core/RouterJobResourceUsage.java:6-28`).
///
/// **All five fields are Java `float` primitives, so Gson always writes all five** — including
/// the two nothing ever assigns. See [`RoutingJob::resource_usage`] for who fills it (in Java:
/// one thread this port does not have) and quirk #256 for the two dead fields.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct RouterJobResourceUsage {
    /// `cpuTimeUsed` (`:9-10`) — "total CPU time used in seconds".
    ///
    /// Java's `monitorCpuAndMemoryUsage` (`RoutingJobSchedulerActionThread.java:227-229`) assigns
    /// this once a second from `ThreadMXBean.getThreadCpuTime(job.thread.threadId())`, i.e. the
    /// **job thread only** — the router's worker threads are not counted, as that method's own
    /// comment says. The port has no monitor thread at all (rostered in `lib.rs`, quirk #237), so
    /// this stays `0.0`; `p8t2`'s `normalize_manifest` strips the whole object for that reason
    /// (plan ruling 8, quirk label J).
    #[serde(rename = "cpu_time")]
    pub cpu_time_used: f32,
    /// `maxMemoryUsed` (`:14-15`) — declared "the total amount of memory allocated in MB", and
    /// assigned `getThreadAllocatedBytes` of the job thread (`:238`), which is cumulative
    /// allocation rather than a maximum. Always `0.0` here.
    #[serde(rename = "max_memory")]
    pub max_memory_used: f32,
    /// `peakMemoryUsed` (`:18-19`) — the running maximum of `MemoryMXBean.getHeapMemoryUsage()`
    /// (`:245-251`). Always `0.0` here.
    #[serde(rename = "peak_memory")]
    pub peak_memory_used: f32,
    /// `ioRead` (`:22-23`). **Never assigned anywhere in the Java tree** — quirk #256.
    #[serde(rename = "io_read")]
    pub io_read: f32,
    /// `ioWrite` (`:26-27`). **Never assigned anywhere in the Java tree** — quirk #256.
    #[serde(rename = "io_written")]
    pub io_write: f32,
}

// =================================================================================================
// The manifest and its three nested DTOs
// =================================================================================================

/// `RoutingResultManifest.SCHEMA_VERSION` (`:23`).
pub const SCHEMA_VERSION: i32 = 1;

/// Port of `core.results.RoutingResultManifest` (`core/results/RoutingResultManifest.java:21-171`).
///
/// **The field order below is the JSON key order** (`:28-65`, module docs point 1). Every `Option`
/// carries `skip_serializing_if`, which is Gson's default `serializeNulls = false` (point 2); the
/// three non-`Option` fields are the three Java primitives, which Gson always writes.
///
/// Serialise it with [`fr_dsn::format::json::to_gson_string_pretty`] — [`write`] does — never with
/// `serde_json::to_string_pretty`, which would use Rust's float formatting and a four-space-free
/// but subtly different pretty layout.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoutingResultManifest {
    /// `schemaVersion` (`:28-29`), an `int` initialised to [`SCHEMA_VERSION`] — always written.
    #[serde(rename = "schema_version")]
    pub schema_version: i32,
    /// `generatedAt` (`:31-32`) — `Instant.now().toString()` at `:101`, i.e. ISO-8601 UTC with a
    /// trailing `Z`. Injected through [`RoutingResultManifest::from_job`]'s `now` parameter so
    /// the port has no hidden clock read.
    #[serde(rename = "generated_at", skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    /// `appVersion` (`:34-35`) — `Constants.FREEROUTING_VERSION` at `:102`, i.e.
    /// [`crate::PARITY_VERSION`] (ruling AT/5), never `CARGO_PKG_VERSION`.
    #[serde(rename = "app_version", skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    /// `gitSha` (`:37-38`) — [`resolve_git_sha`] at `:103`, so never `None` after `from_job`.
    #[serde(rename = "git_sha", skip_serializing_if = "Option::is_none")]
    pub git_sha: Option<String>,
    /// `fixture` (`:40-41`) — allocated unconditionally at `:104`, so `{}` when there is no input
    /// path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixture: Option<FixtureInfo>,
    /// `settingsSnapshot` (`:43-44`) — `job.routerSettings` at `:110`, serialised by
    /// `RouterSettingsTypeAdapterFactory` (`:36-46`), whose *write* half is a pass-through to the
    /// reflective delegate. `fr_settings`' own derived `Serialize` is that delegate (quirk #141
    /// governs the read half's strictness split, which this write path never reaches).
    #[serde(rename = "settings_snapshot", skip_serializing_if = "Option::is_none")]
    pub settings_snapshot: Option<fr_settings::RouterSettings>,
    /// `phases` (`:46-47`) — allocated by the field initialiser, so present on every manifest.
    /// See [`PhaseMetrics`] for what is and is not written into it (quirk #254).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phases: Option<PhaseMetrics>,
    /// `boardStatistics` (`:49-50`) — `new BoardStatistics(job.board)` at `:117`, i.e. the
    /// *computing* constructor, when and only when `job.board != null`.
    ///
    /// Serialised through [`crate::GsonBoardStatistics`], which is Task 2's Gson field order and
    /// `Float.toString` widths; `serde_json::to_value` would lose both.
    #[serde(
        rename = "board_statistics",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_board_statistics"
    )]
    pub board_statistics: Option<BoardStatistics>,
    /// `normalizedScore` (`:52-53`) — a boxed `Float`, written at `:119-120` only when the board
    /// **and** `routerSettings.scoring` are both non-null.
    #[serde(rename = "normalized_score", skip_serializing_if = "Option::is_none")]
    pub normalized_score: Option<f32>,
    /// `resourceUsage` (`:55-56`) — `job.resourceUsage` at `:114`, which
    /// `RoutingJob.java:111-113` initialises to `new RouterJobResourceUsage()`, so it is present
    /// on every manifest a CLI run writes.
    #[serde(rename = "resource_usage", skip_serializing_if = "Option::is_none")]
    pub resource_usage: Option<RouterJobResourceUsage>,
    /// `finalState` (`:58-59`) — `job.state.name()` at `:111`.
    #[serde(rename = "final_state", skip_serializing_if = "Option::is_none")]
    pub final_state: Option<String>,
    /// `exitCode` (`:61-62`), an `int` — always written, `0` on a default manifest.
    #[serde(rename = "exit_code")]
    pub exit_code: i32,
    /// `outputWritten` (`:64-65`), a `boolean` — always written, `false` on a default manifest.
    #[serde(rename = "output_written")]
    pub output_written: bool,
}

/// `board_statistics` through Task 2's Gson adapter rather than `BoardStatistics`' own (absent)
/// `Serialize` — see [`crate::GsonBoardStatistics`].
///
/// `skip_serializing_if` means the `None` arm is unreachable from [`RoutingResultManifest`], but
/// `serialize_with` is applied to the whole `Option`, so the arm has to exist; it spells `null`,
/// which is what Gson would write if `serializeNulls()` were on.
fn serialize_board_statistics<S: Serializer>(
    value: &Option<BoardStatistics>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(stats) => GsonBoardStatistics(stats).serialize(serializer),
        None => serializer.serialize_none(),
    }
}

impl Default for RoutingResultManifest {
    /// The Java field initialisers of `:28-65`, which is what `new RoutingResultManifest()`
    /// (`:26`) leaves behind: `schemaVersion = SCHEMA_VERSION`, `phases = new PhaseMetrics()`,
    /// two zeroed primitives, everything else `null`.
    fn default() -> RoutingResultManifest {
        RoutingResultManifest {
            schema_version: SCHEMA_VERSION,
            generated_at: None,
            app_version: None,
            git_sha: None,
            fixture: None,
            settings_snapshot: None,
            phases: Some(PhaseMetrics::default()),
            board_statistics: None,
            normalized_score: None,
            resource_usage: None,
            final_state: None,
            exit_code: 0,
            output_written: false,
        }
    }
}

/// Port of `RoutingResultManifest.FixtureInfo` (`:67-74`) — "input fixture identity for the run".
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct FixtureInfo {
    /// `filename` (`:69-70`) — `Path.of(inputFilePath).getFileName().toString()` (`:107`), the
    /// bare name, never the directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// `sha256` (`:72-73`) — [`sha256_hex`] of the whole input file (`:108`), and `None`
    /// (i.e. the key is omitted) whenever that file cannot be read. Quirk #255.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Port of `RoutingResultManifest.PhaseMetrics` (`:76-86`) — "per-stage duration and pass counts".
///
/// **Two of the three stages are allocated and never written** (quirk #254): only
/// `autorouter` is ever assigned, at `:125` and `:131`, and what `:131` puts in its
/// `duration_seconds` is the **whole job's** wall-clock duration rather than the routing stage's.
/// All three fields are non-null in Java because the field initialisers construct them, so
/// `"fanout": {}` and `"optimizer": {}` are written on every manifest.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhaseMetrics {
    /// `fanout` (`:78-79`). Always `{}` — see the type docs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fanout: Option<PhaseDetail>,
    /// `autorouter` (`:81-82`). The only one `fromJob` ever fills.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autorouter: Option<PhaseDetail>,
    /// `optimizer` (`:84-85`). Always `{}` — see the type docs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimizer: Option<PhaseDetail>,
}

impl Default for PhaseMetrics {
    /// `:78`, `:81`, `:84` — three `new PhaseDetail()` field initialisers.
    fn default() -> PhaseMetrics {
        PhaseMetrics {
            fanout: Some(PhaseDetail::default()),
            autorouter: Some(PhaseDetail::default()),
            optimizer: Some(PhaseDetail::default()),
        }
    }
}

/// Port of `RoutingResultManifest.PhaseDetail` (`:88-95`) — "duration and pass count for one
/// routing stage". Both fields are boxed, so an untouched `PhaseDetail` serialises as `{}`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct PhaseDetail {
    /// `durationSeconds` (`:90-91`), a `Float`.
    #[serde(rename = "duration_seconds", skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f32>,
    /// `passesCompleted` (`:93-94`), an `Integer`.
    #[serde(rename = "passes_completed", skip_serializing_if = "Option::is_none")]
    pub passes_completed: Option<i32>,
}

// =================================================================================================
// fromJob
// =================================================================================================

impl RoutingResultManifest {
    /// Port of `RoutingResultManifest.fromJob` (`:97-135`).
    ///
    /// # The four parameters Java does not have, and why each one exists
    ///
    /// | parameter | Java | why |
    /// |---|---|---|
    /// | `now` | `Instant.now()` (`:101`) | the port reads no hidden clock; `p8t2`'s shape mode injects a fixed instant, and `normalize_manifest` strips the field anyway |
    /// | `stats` | `new BoardStatistics(job.board)` (`:117`) | **scan ruling R20** — the port's [`RoutingJob`] has no `board` field (it is `transient` in Java and the port's board is `RoutingPipeline::run`'s `&mut Board`), so the caller passes the statistics. `None` is Java's `job.board == null` |
    /// | `input_file_path` | `globalSettings.initialInputFile` (`Freerouting.java:236`) | Java reads it off a `GlobalSettings` this crate does not own; `None` is Java's `null` |
    /// | `exit_code`, `output_written` | already parameters at `:99` | — |
    ///
    /// **The `scoring` parameter scan ruling R20 also drafted is deliberately absent.** Java reads
    /// `job.routerSettings.scoring` (`:118-120`) and the port's [`RoutingJob::router_settings`] is
    /// the same object, so the weights are already reachable; taking them separately would let a
    /// caller score with weights that `settings_snapshot` does not report. See the Task 4 report.
    ///
    /// # `stats` is the cached statistics, and that is a deliberate choice
    ///
    /// Java recomputes at `:117` — a full `BoardStatistics(BasicBoard)`, which runs **two**
    /// `DesignRulesChecker`s (`BoardStatistics.java:265-268`, `:338-341`). The port's caller
    /// (Task 6) has `PipelineResult::final_statistics` for the same board and passes that;
    /// `crates/fr-core/tests/manifest.rs::the_cached_statistics_equal_a_fresh_recompute` asserts
    /// the two are equal on a real fixture, so the saved recompute is a measured equivalence
    /// rather than an assumption.
    ///
    // not reachable: `job.state != null ? … : RoutingJobState.INVALID.name()` (core/results/RoutingResultManifest.java:111) — the port's [`RoutingJob::state`] is a `RoutingJobState`, not an `Option`, and `INVALID` is already its `Default`; the ternary's else-arm exists for a Gson-deserialized manifest, which nothing deserializes.
    // The body assigns field by field, in Java's own statement order (`:100-133`), which is what
    // `clippy::field_reassign_with_default` objects to: a struct literal would put `:114`'s
    // `resourceUsage` above `:110`'s `settingsSnapshot` and lose the line-for-line correspondence
    // this port is audited on.
    #[allow(clippy::field_reassign_with_default)]
    pub fn from_job(
        job: &RoutingJob,
        input_file_path: Option<&Path>,
        output_written: bool,
        exit_code: i32,
        now: &dyn Fn() -> String,
        stats: Option<&BoardStatistics>,
    ) -> RoutingResultManifest {
        // :100.
        let mut manifest = RoutingResultManifest::default();
        // :101-103.
        manifest.generated_at = Some(now());
        manifest.app_version = Some(PARITY_VERSION.to_string());
        manifest.git_sha = Some(resolve_git_sha());
        // :104-109. `fixture` is allocated unconditionally; only its two fields are conditional.
        let mut fixture = FixtureInfo::default();
        if let Some(path) = input_file_path {
            let normalized = java_path::of_to_string(&path.to_string_lossy());
            // Java bug: RoutingResultManifest.fromJob (core/results/RoutingResultManifest.java:107) dereferences `Path.of(inputFilePath).getFileName()`, which is `null` for the filesystem root, so `-de /` throws a NullPointerException out of `writeCliResultManifestIfRequested`'s try block — which catches `IOException` only. Quirk #257.
            // totalized: RoutingResultManifest.fromJob:107 — the port answers the empty string where Java throws, so a manifest is still written; the driver row `MAN root_input` carries both answers.
            manifest.fixture = {
                fixture.filename =
                    Some(java_path::file_name_of_normalized(&normalized).unwrap_or_default());
                // :108. `null` on any failure, and Gson then omits the key — quirk #255.
                fixture.sha256 = sha256_hex(Path::new(&normalized));
                Some(fixture)
            };
        } else {
            manifest.fixture = Some(fixture);
        }
        // :110. Java's field is nullable and the port's is not; `Some` is what a CLI run always
        // has, because `RoutingJob.java:105` initialises it to `new RouterSettings()`.
        manifest.settings_snapshot = Some(job.router_settings.clone());
        // :111.
        manifest.final_state = Some(job.state.java_name().to_string());
        // :112-114.
        manifest.exit_code = exit_code;
        manifest.output_written = output_written;
        manifest.resource_usage = Some(job.resource_usage);

        // :116-122.
        if let Some(stats) = stats {
            manifest.board_statistics = Some(stats.clone());
            if let Some(scoring) = job.router_settings.scoring.as_ref() {
                manifest.normalized_score = Some(stats.normalized_score(scoring));
            }
        }

        // :124-126. `phases` is `Some` on a defaulted manifest, so the `expect` cannot fire.
        let phases = manifest
            .phases
            .as_mut()
            .expect("RoutingResultManifest::default allocates phases");
        let autorouter = phases
            .autorouter
            .as_mut()
            .expect("PhaseMetrics::default allocates autorouter");
        if job.get_current_pass() > 0 {
            autorouter.passes_completed = Some(job.get_current_pass());
        }

        // :128-132. Quirk #254: this is the **whole job's** duration, put in the autorouter's
        // slot; the fanout and optimizer slots stay `{}`.
        if let (Some(started), Some(finished)) = (job.started_at, job.finished_at) {
            // Java's `Duration.between(startedAt, finishedAt).toMillis()` is a `long` over two
            // wall-clock `Instant`s and can be **negative** (an NTP step between the two reads).
            // The port's instants are `std::time::Instant`, which is monotonic, so the saturation
            // below is unreachable rather than a clamp of a real Java value — see the Task 1
            // report's concern 4.
            let millis = finished.saturating_duration_since(started).as_millis();
            autorouter.duration_seconds = Some((millis as f64 / 1000.0) as f32);
        }

        // :134.
        manifest
    }

    /// Port of `RoutingResultManifest.write` (`:137-144`).
    ///
    /// `Files.createDirectories(parent)` when `Path.of(target).getParent()` is non-null — which is
    /// `null` for a bare filename, the same `getParent()` that quirk #242 turns into an NPE one
    /// file over — then `Files.writeString(target, json, UTF_8)`. **No trailing newline**:
    /// `Gson.toJson` does not append one and `writeString` does not either.
    ///
    /// # Errors
    ///
    /// Java throws `IOException`, which `Freerouting.java:238` catches and logs. Two failures the
    /// port must not hide behind that: a directory that cannot be created, and a target that
    /// cannot be written. A non-finite float anywhere in the manifest is where `Gson.toJson`
    /// throws `IllegalArgumentException` instead (`fr_dsn::format::json`, point 3); it surfaces
    /// here as [`std::io::ErrorKind::InvalidData`], because the whole call is fallible anyway and
    /// a panic in a CLI's last step would lose the routed board.
    pub fn write(path: &Path, manifest: &RoutingResultManifest) -> std::io::Result<()> {
        // :139-141.
        let normalized = java_path::of_to_string(&path.to_string_lossy());
        if let Some(parent) = java_path::parent_of_normalized(&normalized) {
            std::fs::create_dir_all(&parent)?;
        }
        // :142.
        let json = to_gson_string_pretty(manifest)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // :143.
        std::fs::write(&normalized, json.as_bytes())
    }

    /// `GsonProvider.GSON.toJson(this)` on its own — [`write`](RoutingResultManifest::write)
    /// without the file, for the driver, the tests and any host that wants the bytes.
    ///
    // pub seam: `core/results/RoutingResultManifest.java` has no such method — `write:142` inlines the `toJson` call. Split out so the JSON can be produced without touching the filesystem, which is what `p8t2`'s shape mode and every test in `crates/fr-core/tests/manifest.rs` need.
    ///
    /// # Errors
    ///
    /// A non-finite `f32`/`f64` anywhere in the manifest — where `Gson.toJson` throws
    /// `IllegalArgumentException`.
    pub fn to_gson_string(&self) -> Result<String, serde_json::Error> {
        to_gson_string_pretty(self)
    }
}

// =================================================================================================
// resolveGitSha
// =================================================================================================

/// Port of `RoutingResultManifest.resolveGitSha` (`:146-161`).
///
/// Java walks three sources and falls back to `"unknown"`, taking the first that is non-null and
/// not `isBlank()`, and `trim()`ing what it takes:
///
/// | arm | Java | this port |
/// |---|---|---|
/// | `:148-151` | env `FREEROUTING_GIT_SHA` | env `FREEROUTING_GIT_SHA` |
/// | `:152-155` | system property `freerouting.git.sha` | env `freerouting.git.sha` |
/// | `:156-159` | system property `FREEROUTING_GIT_SHA` | — see below |
/// | `:160` | `"unknown"` | `"unknown"` |
///
// renamed: RoutingResultManifest.resolveGitSha's two `System.getProperty` lookups (core/results/RoutingResultManifest.java:152, :156) — the port has no JVM system properties, so both become `std::env::var` lookups of the *same names*. `freerouting.git.sha` is a legal POSIX environment-variable name (any byte but `=` and NUL), so the second arm keeps its spelling exactly.
///
// not reachable: RoutingResultManifest.resolveGitSha's third arm (core/results/RoutingResultManifest.java:156-159) — `System.getProperty("FREEROUTING_GIT_SHA")` renames to an env lookup of `FREEROUTING_GIT_SHA`, which is the *first* arm's variable. A value that satisfies arm three would have satisfied arm one, and a value that failed arm one (absent, or blank) fails arm three for the same reason, so the arm can never decide anything here. Java can tell them apart — `-DFREEROUTING_GIT_SHA=abc` with the env variable unset or blank — and that is the one input where the two disagree; `p8t2`'s `GITSHA legacy_prop_only` row carries both answers as an `XDIFF`.
///
/// **`isBlank()` and `trim()` do not use the same character set, in either language, and the two
/// languages do not agree on either.** `String.isBlank()` is "empty or every code point
/// `Character.isWhitespace`"; `String.trim()` strips code units `<= U+0020` and nothing else.
/// Rust's `str::trim` and `char::is_whitespace` are both the Unicode `White_Space` property.
///
/// The three-way difference is **eight characters, and that is the complete list** — it was swept
/// on the JVM rather than reasoned about (JDK 25, `Character.isWhitespace(cp)` and
/// `!s.trim().equals(s)` over every code point):
///
/// ```text
/// Character.isWhitespace : 0009-000D 001C-0020 1680 2000-2006 2008-200A 2028 2029 205F 3000
/// String.trim strips     : 0000-0020
/// char::is_whitespace    : 0009-000D 0020 0085 00A0 1680 2000-200A 2028 2029 202F 205F 3000
/// ```
///
/// | character | `Character.isWhitespace` | `String.trim` | Rust `is_whitespace`/`trim` |
/// |---|---|---|---|
/// | `U+001C`-`U+001F` (the four separators) | yes | yes | **no** |
/// | `U+0085` (NEL) | **no** | no | **yes** |
/// | `U+00A0`, `U+2007`, `U+202F` (the non-breaking spaces) | **no** | no | **yes** |
///
/// So a git sha of one non-breaking space — or one `U+0085` — is *not blank* to Java and is
/// returned **unchanged**, where `value.trim()` in Rust would answer the empty string.
/// [`java_is_blank`] and [`java_trim`] are written out for that reason; `whitespace_sets_are_the_
/// measured_ones` asserts the whole symmetric difference above rather than the four rows anybody
/// happened to think of, and the `GITSHA nbsp_only`/`nel_only`/`file_separators_only` rows of the
/// `p8t2` transcript carry the jar's own answers for three of them.
pub fn resolve_git_sha() -> String {
    // :148-151.
    if let Ok(value) = std::env::var("FREEROUTING_GIT_SHA")
        && !java_is_blank(&value)
    {
        return java_trim(&value);
    }
    // :152-155.
    if let Ok(value) = std::env::var("freerouting.git.sha")
        && !java_is_blank(&value)
    {
        return java_trim(&value);
    }
    // :160.
    "unknown".to_string()
}

/// `String.isBlank()` (`java.lang.String`): empty, or every code point `Character.isWhitespace`.
fn java_is_blank(value: &str) -> bool {
    value.chars().all(java_is_whitespace)
}

/// `Character.isWhitespace(int)`.
///
/// A Unicode space/line/paragraph separator that is **not** a non-breaking space, or one of the
/// five control characters `U+0009`-`U+000D`, or one of `U+001C`-`U+001F`.
///
/// The four `=> false` characters are the whole of what Rust calls whitespace and Java does not
/// (see [`resolve_git_sha`]'s swept table): the three non-breaking spaces, which
/// `Character.isWhitespace`'s javadoc excludes by name, and `U+0085` NEL, which it excludes by
/// silence — NEL is category `Cc`, not a separator, and it is not in the explicit
/// `U+0009`-`U+000D` / `U+001C`-`U+001F` list either. Measured, not inferred: the JVM answers
/// `Character.isWhitespace(0x85) == false`, `"\u0085".isBlank() == false` and
/// `"\u0085".trim().length() == 1`.
fn java_is_whitespace(c: char) -> bool {
    match c {
        '\u{9}'..='\u{D}' | '\u{1C}'..='\u{1F}' => true,
        // The three non-breaking spaces Java excludes by name, and NEL, which it excludes by
        // being a control character rather than a separator.
        '\u{85}' | '\u{A0}' | '\u{2007}' | '\u{202F}' => false,
        _ => c.is_whitespace(),
    }
}

/// `String.trim()` — strip leading and trailing code units `<= U+0020`, and nothing else.
///
/// Java compares UTF-16 code units, but every code unit `<= U+0020` is also a whole code point,
/// so scanning `char`s gives the same answer on every input.
fn java_trim(value: &str) -> String {
    value.trim_matches(|c: char| c <= '\u{20}').to_string()
}

// =================================================================================================
// sha256Hex — and the FIPS 180-4 core it needs
// =================================================================================================

/// Port of `RoutingResultManifest.sha256Hex` (`:163-171`).
///
/// **`None` is Java's `null`, and Gson then omits the `sha256` key** (quirk #255): a fixture that
/// cannot be read is indistinguishable in the manifest from one whose hash was never asked for.
/// Java's catch is `IOException | NoSuchAlgorithmException`; `NoSuchAlgorithmException` cannot
/// fire on any JRE that ships SHA-256, so every real `None` is an I/O failure.
///
// pub seam: `sha256Hex` is `private static` (core/results/RoutingResultManifest.java:163) and is reached from `P8T2.java` by reflection. The port exposes it, because the CLI's fixture identity is worth being able to recompute and because the NIST vector test needs a public entry point.
pub fn sha256_hex(path: &Path) -> Option<String> {
    // :165. Java's `Files.readAllBytes` also throws on a directory (`IsADirectoryException`, an
    // `IOException`), which is why a `-de <dir>` run omits the key rather than hashing nothing.
    let bytes = std::fs::read(path).ok()?;
    // :167 — `HexFormat.of()` is lower case.
    Some(hex_lower(&sha256(&bytes)))
}

/// `HexFormat.of().formatHex(byte[])` — lower case, two characters per byte, no separator.
fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).expect("a nibble is < 16"));
        out.push(char::from_digit(u32::from(byte & 0x0F), 16).expect("a nibble is < 16"));
    }
    out
}

/// FIPS 180-4 §6.2 SHA-256, §5.3.3's initial hash value.
const SHA256_H0: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

/// FIPS 180-4 §4.2.2 — the first thirty-two bits of the fractional parts of the cube roots of the
/// first sixty-four primes.
#[rustfmt::skip]
const SHA256_K: [u32; 64] = [
    0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1, 0x923f_82a4, 0xab1c_5ed5,
    0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3, 0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174,
    0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
    0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967,
    0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13, 0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85,
    0xa2bf_e8a1, 0xa81a_664b, 0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
    0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3,
    0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208, 0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
];

/// SHA-256 of `data` — FIPS 180-4 §6.2.2, written out because the Global Constraint forbids
/// adding a dependency for it and `Cargo.lock` carries none (module docs).
///
/// The message schedule is the 64-word form of §6.2.2 step 1; the compression is step 3 with the
/// six logical functions of §4.1.2 inlined.
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = SHA256_H0;

    // §5.1.1 padding: `0x80`, then zeros to 56 mod 64, then the 64-bit big-endian bit length.
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for block in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (index, word) in block.chunks_exact(4).enumerate() {
            w[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            // §4.1.2 sigma_0 and sigma_1.
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for index in 0..64 {
            // §4.1.2 Sigma_1 and Ch.
            let big_s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(big_s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[index])
                .wrapping_add(w[index]);
            // §4.1.2 Sigma_0 and Maj.
            let big_s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = big_s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }

        for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(value);
        }
    }

    let mut digest = [0u8; 32];
    for (chunk, word) in digest.chunks_exact_mut(4).zip(h) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The digest of `bytes`, as `sha256Hex` would spell it.
    fn hex_of(bytes: &[u8]) -> String {
        hex_lower(&sha256(bytes))
    }

    /// The three FIPS 180-4 / NIST CAVP example vectors plus the empty input.
    ///
    /// `sha256_hex` reads a file and this reads bytes, so the file half is
    /// `crates/fr-core/tests/manifest.rs`' job; this is the core.
    #[test]
    fn sha256_matches_the_nist_vectors() {
        // FIPS 180-4 Appendix B.1 — one block.
        assert_eq!(
            hex_of(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Appendix B.2 — two blocks.
        assert_eq!(
            hex_of(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // Appendix B.3 — one million `a`s.
        assert_eq!(
            hex_of(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        // The empty message — the padding-only case.
        assert_eq!(
            hex_of(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// The two boundaries of the padding rule: 55 bytes fits in one block, 56 forces a second.
    #[test]
    fn sha256_pads_across_the_block_boundary() {
        assert_eq!(
            hex_of(&[b'a'; 55]),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
        );
        assert_eq!(
            hex_of(&[b'a'; 56]),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
        );
        assert_eq!(
            hex_of(&[b'a'; 64]),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
        );
    }

    /// `HexFormat.of()` is the lower-case one, and it pads every byte to two characters.
    #[test]
    fn hex_is_lower_case_and_zero_padded() {
        assert_eq!(hex_lower(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    }

    /// [`java_is_whitespace`] against the JVM's own answer, every code point, plus the complete
    /// symmetric difference with Rust's `char::is_whitespace`.
    ///
    /// This is the test that would have caught `U+0085` (task review SF1): the first version of
    /// the predicate special-cased the three non-breaking spaces by hand and let NEL fall through
    /// to `char::is_whitespace`, which says `true` where Java says `false`. Asserting the *whole*
    /// set against a swept JVM answer is what makes [`resolve_git_sha`]'s table a measurement
    /// rather than a list of the cases somebody thought of.
    #[test]
    fn whitespace_sets_are_the_measured_ones() {
        // Swept on the JVM at port time (JDK 25, `/opt/homebrew/opt/openjdk@25/bin/java`) with
        // `for (cp in 0..=0x10FFFF) if (Character.isWhitespace(cp))`. The ranges below are that
        // run's output, verbatim:
        //   0009 000A 000B 000C 000D 001C 001D 001E 001F 0020 1680 2000 2001 2002 2003 2004 2005
        //   2006 2008 2009 200A 2028 2029 205F 3000
        fn jvm_is_whitespace(c: char) -> bool {
            matches!(
                c as u32,
                0x09..=0x0D
                    | 0x1C..=0x20
                    | 0x1680
                    | 0x2000..=0x2006
                    | 0x2008..=0x200A
                    | 0x2028
                    | 0x2029
                    | 0x205F
                    | 0x3000
            )
        }

        let mut java_only: Vec<u32> = Vec::new();
        let mut rust_only: Vec<u32> = Vec::new();
        for cp in 0..=0x10FFFF_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            assert_eq!(
                java_is_whitespace(c),
                jvm_is_whitespace(c),
                "java_is_whitespace disagrees with the JVM at U+{cp:04X}"
            );
            match (jvm_is_whitespace(c), c.is_whitespace()) {
                (true, false) => java_only.push(cp),
                (false, true) => rust_only.push(cp),
                _ => {}
            }
        }
        assert_eq!(
            java_only,
            [0x1C, 0x1D, 0x1E, 0x1F],
            "the four separators Java calls whitespace and Rust does not"
        );
        assert_eq!(
            rust_only,
            [0x85, 0xA0, 0x2007, 0x202F],
            "NEL and the three non-breaking spaces — Rust calls them whitespace, Java does not"
        );

        // `String.trim()` strips code units <= U+0020 and nothing else — the same sweep's second
        // half printed exactly `0000`..`0020`.
        for cp in 0..=0xFFFF_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let text = c.to_string();
            let stripped = java_trim(&text).is_empty();
            assert_eq!(stripped, cp <= 0x20, "java_trim at U+{cp:04X}");
        }
    }

    /// `String.isBlank()`'s and `String.trim()`'s character sets, where they differ from Rust's.
    #[test]
    fn java_blankness_and_trimming_are_not_rusts() {
        assert!(java_is_blank(""));
        assert!(java_is_blank(" \t\r\n"));
        // `U+001C`-`U+001F`: `Character.isWhitespace` says yes, `char::is_whitespace` says no.
        assert!(java_is_blank("\u{1c}\u{1d}\u{1e}\u{1f}"));
        assert_eq!("\u{1c}".trim(), "\u{1c}", "Rust does not trim U+001C");
        assert_eq!(
            java_trim("\u{1c}abc\u{1f}"),
            "abc",
            "Java does — it is < 0x20"
        );
        // The non-breaking spaces: `char::is_whitespace` says yes, `Character.isWhitespace` says
        // no — so a git sha of one NBSP is *not* blank to Java and is returned unchanged, where
        // `value.trim()` in Rust would have answered the empty string.
        assert!(!java_is_blank("\u{a0}"));
        assert!(!java_is_blank("\u{2007}"));
        assert!(!java_is_blank("\u{202f}"));
        // `U+0085` NEL, the fourth member of that half of the difference — measured on the JVM as
        // `isWhitespace=false isBlank=false trimLen=1` (task review SF1).
        assert!(!java_is_blank("\u{85}"));
        assert_eq!(
            java_trim("\u{85}"),
            "\u{85}",
            "Java's trim keeps anything > U+0020"
        );
        assert_eq!("\u{85}".trim(), "", "Rust's does not");
        assert_eq!("\u{a0}".trim(), "", "Rust trims the non-breaking space");
        assert_eq!(java_trim("\u{a0}"), "\u{a0}", "Java does not");
        assert!(!java_is_blank("abc"));
    }
}

// =================================================================================================
// `Instant.now().toString()` — the `generated_at` producer (Plan 8 Task 6)
// =================================================================================================

/// `java.time.Instant.now().toString()` (`RoutingResultManifest.java:101`) — ISO-8601 UTC with a
/// trailing `Z`, hand-rolled from [`SystemTime`] because this workspace has no date library and
/// may not add one.
///
/// # Why it is hand-rolled, and what was measured
///
/// The Task 4 controller note left the choice open: format it here, or thread a string in from
/// the CLI edge. Task 6 formats it here, because `commands/route.rs`, `commands/drc.rs` and the
/// MCP tool all need the same rendering and a string threaded from three call sites is three
/// chances to spell it differently. `p8t2`'s `normalize_manifest` strips the field, so the
/// differential cannot catch a wrong format; [`crate::manifest`]'s unit tests pin it against
/// **jar-measured** examples instead (JDK 25, `Instant.ofEpochSecond(1_756_800_000, nanos)`):
///
/// ```text
/// nanos          Instant.toString()
/// 0              2025-09-02T08:00:00Z
/// 1              2025-09-02T08:00:00.000000001Z
/// 1_000          2025-09-02T08:00:00.000001Z
/// 1_000_000      2025-09-02T08:00:00.001Z
/// 10_000_000     2025-09-02T08:00:00.010Z
/// 100_000_000    2025-09-02T08:00:00.100Z
/// 120_000_000    2025-09-02T08:00:00.120Z
/// 123_000_000    2025-09-02T08:00:00.123Z
/// 123_456_789    2025-09-02T08:00:00.123456789Z
/// 500_000_000    2025-09-02T08:00:00.500Z
/// 999_999_999    2025-09-02T08:00:00.999999999Z
/// ```
///
/// The rule those rows encode is `DateTimeFormatter.ISO_INSTANT`'s: the fractional part is
/// **omitted** when the nanosecond field is zero and is otherwise printed at the smallest of
/// **3, 6 or 9** digits that represents it exactly. Seconds are always printed — `Instant
/// .ofEpochSecond(0).toString()` is `1970-01-01T00:00:00Z`, measured, not assumed.
///
// renamed: Instant.now().toString() -> now_utc_iso8601 — Java's is a library call on a type this
// port does not have; the name says what the string is rather than which class produced it.
#[must_use]
pub fn now_utc_iso8601() -> String {
    format_utc_iso8601(std::time::SystemTime::now())
}

/// [`now_utc_iso8601`] with the instant supplied, so a test can pin the rendering.
///
/// A `SystemTime` before the Unix epoch renders with a negative year exactly as
/// `Instant.toString()` would only for years `0000`-`9999`; outside that range Java prefixes a
/// `+`/`-` and this port does not, which is unreachable from a real clock and is recorded here
/// rather than branched on.
#[must_use]
pub fn format_utc_iso8601(time: std::time::SystemTime) -> String {
    let (secs, nanos) = match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(delta) => (
            i64::try_from(delta.as_secs()).unwrap_or(i64::MAX),
            delta.subsec_nanos(),
        ),
        // Before the epoch: `Duration` is unsigned, so the error carries the magnitude. A
        // non-zero sub-second part borrows a second, exactly as `Instant`'s own normalisation
        // does.
        Err(error) => {
            let delta = error.duration();
            let secs = i64::try_from(delta.as_secs()).unwrap_or(i64::MAX);
            match delta.subsec_nanos() {
                0 => (-secs, 0),
                sub => (-secs - 1, 1_000_000_000 - sub),
            }
        }
    };
    let days = secs.div_euclid(86_400);
    let seconds_of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60,
        seconds_of_day % 60,
    );
    let mut out = format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}");
    // `DateTimeFormatter.ISO_INSTANT`'s variable fraction: nothing, milli, micro or nano.
    if nanos != 0 {
        if nanos % 1_000_000 == 0 {
            out.push_str(&format!(".{:03}", nanos / 1_000_000));
        } else if nanos % 1_000 == 0 {
            out.push_str(&format!(".{:06}", nanos / 1_000));
        } else {
            out.push_str(&format!(".{nanos:09}"));
        }
    }
    out.push('Z');
    out
}

/// Howard Hinnant's `civil_from_days`: a day number relative to 1970-01-01 to `(year, month,
/// day)` in the proleptic Gregorian calendar, which is what `java.time` uses.
///
/// Not a port of any Java line — `java.time.LocalDate.ofEpochDay` is the JDK's own arithmetic and
/// this is the same algorithm, written out because the workspace has no date library.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (year, m as u32, d as u32)
}

#[cfg(test)]
mod instant_tests {
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    /// The eleven rows measured on JDK 25 — see [`now_utc_iso8601`]'s table.
    #[test]
    fn the_rendering_is_the_jvms_instant_to_string() {
        let base = 1_756_800_000u64;
        for (nanos, expected) in [
            (0u32, "2025-09-02T08:00:00Z"),
            (1, "2025-09-02T08:00:00.000000001Z"),
            (1_000, "2025-09-02T08:00:00.000001Z"),
            (1_000_000, "2025-09-02T08:00:00.001Z"),
            (10_000_000, "2025-09-02T08:00:00.010Z"),
            (100_000_000, "2025-09-02T08:00:00.100Z"),
            (120_000_000, "2025-09-02T08:00:00.120Z"),
            (123_000_000, "2025-09-02T08:00:00.123Z"),
            (123_456_789, "2025-09-02T08:00:00.123456789Z"),
            (500_000_000, "2025-09-02T08:00:00.500Z"),
            (999_999_999, "2025-09-02T08:00:00.999999999Z"),
        ] {
            let time = UNIX_EPOCH + Duration::new(base, nanos);
            assert_eq!(format_utc_iso8601(time), expected, "nanos {nanos}");
        }
        // `Instant.ofEpochSecond(0).toString()` — seconds are printed even when zero.
        assert_eq!(format_utc_iso8601(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    }

    /// A handful of calendar edges the day arithmetic has to get right: the leap day of a
    /// century-divisible leap year, the day after it, and a year boundary.
    #[test]
    fn the_calendar_is_proleptic_gregorian() {
        for (secs, expected) in [
            (951_782_400u64, "2000-02-29T00:00:00Z"),
            (951_868_800, "2000-03-01T00:00:00Z"),
            (1_072_915_199, "2003-12-31T23:59:59Z"),
            (1_072_915_200, "2004-01-01T00:00:00Z"),
            (4_102_444_800, "2100-01-01T00:00:00Z"),
        ] {
            assert_eq!(
                format_utc_iso8601(UNIX_EPOCH + Duration::from_secs(secs)),
                expected,
                "secs {secs}"
            );
        }
    }

    /// The clock read `RoutingResultManifest::from_job` gets: a well-formed instant in this
    /// decade, ending in `Z`.
    #[test]
    fn now_is_well_formed() {
        let now = now_utc_iso8601();
        assert!(now.ends_with('Z'), "{now}");
        assert_eq!(now.as_bytes()[4], b'-', "{now}");
        assert_eq!(now.as_bytes()[10], b'T', "{now}");
        assert!(now.starts_with("20"), "{now}");
        // Two reads are ordered, which is what a wall clock has to be for the field to mean
        // anything.
        assert!(now_utc_iso8601() >= now);
        let _ = SystemTime::now();
    }
}
