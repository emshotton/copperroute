//! The job model — `core/RoutingJob.java`'s **file / format / state** half, `io/FileFormat.java`,
//! `core/RoutingJobState.java`, `core/RoutingStage.java` and `core/Session.java`'s validation.
//!
//! # What is here and what is not
//!
//! Java's `RoutingJob` is three objects wearing one name: a *file* object (the input, the rules,
//! the derived output), a *queue* object (`priority`, `compareTo`, the scheduler's states) and an
//! *event* object (four listener lists and four `fire*` methods). Plan 8 ports the first, rosters
//! the second (Task 0, `lib.rs` §4 and §6 — there is no queue and no daemon in a single-shot CLI)
//! and replaces the third with spec §10's [`crate::progress::SyncProgressSink`].
//!
//! # `FileFormat` is `io/FileFormat.java`, and it has nine values (scan ruling R17)
//!
//! The plan draft called it "`RoutingJob.java`'s enum" with seven values. It is
//! `src/main/java/app/freerouting/io/FileFormat.java` and it has **nine**: the draft omitted
//! `SCR` and `FRB`, both of which `getFileFormat(Path):230-247` returns and both of which a
//! `-de`/`-do` argv can reach. `io/` is Plan 3's audit surface, so the enum's audit home stays
//! `scripts/audit-map/fr-dsn.map`; its *code* home is here, beside its only two producers.
//!
//! # The platform separator is pinned to POSIX
//!
//! `BoardFileDetails.setFilename` branches on `filename.contains(File.separator)` and then runs
//! two Windows-only rewrites unconditionally (quirk #246). The port pins the separator to `/`,
//! which is what the parity jar sees on this project's host, and reproduces the two rewrites
//! verbatim — including the regex bug — so a path that *does* contain a backslash is mangled
//! identically. See [`FILE_SEPARATOR`].

use crate::Error;
use crate::file_details::BoardFileDetails;
use crate::manifest::RouterJobResourceUsage;
use fr_settings::{DesignRulesCheckerSettings, RouterSettings};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// `RoutingJob.DSN_FILE_EXTENSION` (`RoutingJob.java:31`).
pub const DSN_FILE_EXTENSION: &str = "dsn";
/// `RoutingJob.BINARY_FILE_EXTENSION` (`:32`).
pub const BINARY_FILE_EXTENSION: &str = "frb";
/// `RoutingJob.RULES_FILE_EXTENSION` (`:33`, private).
pub const RULES_FILE_EXTENSION: &str = "rules";
/// `RoutingJob.SES_FILE_EXTENSION` (`:34`, private).
pub const SES_FILE_EXTENSION: &str = "ses";
/// `RoutingJob.EAGLE_SCRIPT_FILE_EXTENSION` (`:35`, private).
pub const EAGLE_SCRIPT_FILE_EXTENSION: &str = "scr";

/// `java.io.File.separator` as the parity jar sees it on this project's host.
///
/// Pinned rather than `std::path::MAIN_SEPARATOR` on purpose: `setFilename`'s behaviour
/// *branches* on it (`BoardFileDetails.java:158`), so a port that read the host separator would
/// disagree with every committed transcript when built on Windows. The transcript, not the
/// constant, is the contract. Quirk #246 records that the branch is platform-dependent in Java
/// too.
///
/// **The pin is wider than this constant, and wider than `setFilename`.** The whole [`java_path`]
/// module below hardcodes `/` as *the* separator and `starts_with('/')` as *the* definition of an
/// absolute path, so every caller of it is POSIX-only as well:
/// [`RoutingJob::change_file_extension`] (which every derived output name goes through),
/// [`RoutingJob::set_input`]'s absolutisation, [`crate::BoardFileDetails::get_absolute_path`],
/// [`crate::BoardFileDetails::get_file`] and [`crate::BoardFileDetails::from_file`].
/// **Windows support is a rewrite of `java_path`, not an unpinning of this constant** — and it
/// would have to regenerate `crates/fr-core/tests/data/p8t1-job-model.txt` against a jar running
/// on Windows, because half its rows would legitimately change. Said plainly here so that whoever
/// revisits it does not mistake the scope.
pub const FILE_SEPARATOR: char = '/';

// =================================================================================================
// Java `java.nio.file.Path` semantics, reproduced on `str`
// =================================================================================================

/// The handful of `java.nio.file.Path` behaviours the job model depends on, reproduced exactly.
///
/// Rust's [`std::path::Path`] agrees with Java on most of this and disagrees on three points that
/// are all load-bearing here, which is why these are hand-written rather than delegated:
///
/// | case | Java | Rust `std::path::Path` |
/// |---|---|---|
/// | `Path.of("out.ses").getParent()` | `null` | `Some("")` |
/// | `Path.of("a//b/").toString()` | `"a/b"` | `"a//b/"` (no normalisation) |
/// | `Path.of("").getFileName()` | the empty path | `None` |
///
/// **This module is POSIX-only.** `/` is hardcoded as the separator and `starts_with('/')` as the
/// definition of an absolute path, exactly as [`FILE_SEPARATOR`] is — and for the same reason: the
/// parity surface is the HEAD jar as it runs on this project's host, and the committed transcript
/// is the contract. Windows support is a rewrite of this module (and a regenerated transcript),
/// not a change to one constant.
pub(crate) mod java_path {
    /// `Path.of(s).toString()` — repeated separators collapsed, a trailing separator dropped,
    /// `.` and `..` components **kept** (Java's `Path.of` does not normalise those).
    pub(crate) fn of_to_string(s: &str) -> String {
        let absolute = s.starts_with(super::FILE_SEPARATOR);
        let segments: Vec<&str> = s
            .split(super::FILE_SEPARATOR)
            .filter(|seg| !seg.is_empty())
            .collect();
        if segments.is_empty() {
            return if absolute {
                "/".to_string()
            } else {
                String::new()
            };
        }
        let joined = segments.join("/");
        if absolute {
            format!("/{joined}")
        } else {
            joined
        }
    }

    /// `Path.of(s).getParent()` on an already-normalised string. `None` is Java's `null`.
    pub(crate) fn parent_of_normalized(normalized: &str) -> Option<String> {
        match normalized.rfind(super::FILE_SEPARATOR) {
            None => None,
            Some(0) if normalized.len() == 1 => None, // `Path.of("/").getParent()` is null
            Some(0) => Some("/".to_string()),
            Some(i) => Some(normalized[..i].to_string()),
        }
    }

    /// `Path.of(s).getFileName()` on an already-normalised string. `None` is Java's `null`,
    /// which happens only for the root.
    pub(crate) fn file_name_of_normalized(normalized: &str) -> Option<String> {
        if normalized == "/" {
            return None;
        }
        if normalized.is_empty() {
            // `Path.of("").getFileName()` is the *empty* path, not null.
            return Some(String::new());
        }
        Some(
            normalized
                .rsplit(super::FILE_SEPARATOR)
                .next()
                .unwrap_or("")
                .to_string(),
        )
    }

    /// `Path.of(s).toAbsolutePath().toString()` — `cwd.resolve(this)` with **no** normalisation
    /// of `.`/`..`, which is why `changeFileExtension("./out.dsn", "ses")` answers
    /// `<cwd>/./out.ses`.
    pub(crate) fn to_absolute_path(s: &str) -> String {
        let normalized = of_to_string(s);
        if normalized.starts_with(super::FILE_SEPARATOR) {
            return normalized;
        }
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if normalized.is_empty() {
            return cwd;
        }
        join2(&cwd, &normalized)
    }

    /// `Path.of(first, second).toString()` — an empty element contributes nothing.
    pub(crate) fn join2(first: &str, second: &str) -> String {
        if first.is_empty() {
            return of_to_string(second);
        }
        if second.is_empty() {
            return of_to_string(first);
        }
        if first.ends_with(super::FILE_SEPARATOR) {
            of_to_string(&format!("{first}{second}"))
        } else {
            of_to_string(&format!("{first}/{second}"))
        }
    }

    /// `String.split("\\.")` with `limit == 0`.
    ///
    /// Two behaviours a naive `str::split('.')` gets wrong, and both are reachable from a
    /// `-de`/`-do` argv: when the pattern never matches Java returns a one-element array holding
    /// the whole input (so `"".split("\\.")` has length **1**, not 0), and when it does match
    /// every *trailing* empty part is dropped (so `"board.ses.".split("\\.")` has length **2** and
    /// its last element is `"ses"`).
    pub(crate) fn split_on_dot(s: &str) -> Vec<&str> {
        if !s.contains('.') {
            return vec![s];
        }
        let mut parts: Vec<&str> = s.split('.').collect();
        while parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
        parts
    }
}

// =================================================================================================
// FileFormat — `io/FileFormat.java` (scan ruling R17: nine values, not seven)
// =================================================================================================

/// Port of `app.freerouting.io.FileFormat` (`io/FileFormat.java:4-14`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FileFormat {
    /// `UNKNOWN` (`:5`).
    #[default]
    Unknown,
    /// `DSN` (`:6`).
    Dsn,
    /// `FRB` (`:7`) — the Java-serialisation board dump.
    Frb,
    /// `SES` (`:8`).
    Ses,
    /// `RULES` (`:9`).
    Rules,
    /// `SCR` (`:10`) — the EAGLE script.
    Scr,
    /// `DRC_JSON` (`:11`).
    DrcJson,
    /// `KICAD_DESIGN_JSON` (`:12`).
    KicadDesignJson,
    /// `KICAD_SESSION_JSON` (`:13`).
    KicadSessionJson,
}

/// How many times `RoutingJob.getFileFormat`'s shift loop (`:181-187`) may run before the port
/// stops it.
///
/// **Five is exact, not a guess.** After *n* shifts `buffer[i] == original[min(i + n, 5)]`,
/// because the loop never refills `buffer[5]`. So the loop's condition can only see a *new* byte
/// for `n <= 5`; from `n == 5` on, `buffer[0]` is `original[5]` for ever. Java therefore
/// terminates iff some `original[n]` with `n <= 5` is not CR/LF, and spins for ever otherwise —
/// and the port reaches the same decision after at most five iterations.
const SHIFT_LOOP_BOUND: usize = 5;

impl FileFormat {
    /// The `name()` of the Java enum constant — the spelling every probe transcript uses.
    pub fn java_name(self) -> &'static str {
        match self {
            FileFormat::Unknown => "UNKNOWN",
            FileFormat::Dsn => "DSN",
            FileFormat::Frb => "FRB",
            FileFormat::Ses => "SES",
            FileFormat::Rules => "RULES",
            FileFormat::Scr => "SCR",
            FileFormat::DrcJson => "DRC_JSON",
            FileFormat::KicadDesignJson => "KICAD_DESIGN_JSON",
            FileFormat::KicadSessionJson => "KICAD_SESSION_JSON",
        }
    }

    /// Parses a Java enum constant name. `None` for anything else.
    pub fn from_java_name(name: &str) -> Option<FileFormat> {
        Some(match name {
            "UNKNOWN" => FileFormat::Unknown,
            "DSN" => FileFormat::Dsn,
            "FRB" => FileFormat::Frb,
            "SES" => FileFormat::Ses,
            "RULES" => FileFormat::Rules,
            "SCR" => FileFormat::Scr,
            "DRC_JSON" => FileFormat::DrcJson,
            "KICAD_DESIGN_JSON" => FileFormat::KicadDesignJson,
            "KICAD_SESSION_JSON" => FileFormat::KicadSessionJson,
            _ => return None,
        })
    }

    /// `// renamed: RoutingJob.getFileFormat` (`RoutingJob.java:151-227`) — format detection from
    /// the file's **bytes**.
    ///
    /// Seventy-seven lines of byte comparison, transcribed branch for branch. What the branches
    /// actually decide, in the order they run:
    ///
    /// 1. **The JSON pre-check** (`:156-164`) scans forward over `' '`, `'\t'`, `'\r'`, `'\n'`
    ///    and answers [`FileFormat::KicadDesignJson`] if the first other byte is `'{'`. It runs
    ///    on the whole buffer and needs **one** byte, so `"{"` alone is KiCad JSON, and so is a
    ///    file with two hundred newlines before its brace. Note what it does *not* skip: a UTF-8
    ///    BOM. `EF BB BF 7B` is `UNKNOWN`.
    /// 2. **The six-byte guard** (`:170-171`). `bytesRead == 6` or nothing else runs, so
    ///    `"(pcb "` — five bytes — is `UNKNOWN` even though `"(pcb x"` is `DSN`.
    /// 3. **The Java-serialisation magic** `AC ED 00 05` (`:173-178`), tested **before** the
    ///    shift loop and before anything else, so an `.frb` whose fifth byte is `{` is still
    ///    `FRB`.
    /// 4. **The shift loop** (`:181-187`) — see [`FileFormat::java_shift_loop_hangs`] and quirk
    ///    #241. Its comment says "0x0A or 0x13"; the code tests `0x0A`/`0x0D`, and the **code** is
    ///    what is transcribed. It strips CR and LF only: a leading space or tab survives it and
    ///    defeats every check below.
    /// 5. `(pcb` / `(PCB` (`:190-199`), `(ses` / `(SES` (`:202-211`) — whole-case branches, so
    ///    `(Pcb` matches neither; then `(r|R)(u|U)(l|L)` (`:214-219`) — **per-character** case
    ///    folding, so `(RuLes` is `RULES` and so, because only four bytes are ever compared, is
    ///    `(rulx`.
    pub fn sniff_bytes(content: &[u8]) -> FileFormat {
        FileFormat::sniff_bytes_inner(content).0
    }

    /// `RoutingJob.getFileFormat(byte[])`'s **first** branch (`:152-154`):
    /// `if (content == null) { return FileFormat.UNKNOWN; }`.
    ///
    /// Rust has no null slice, so the branch is modelled where a null can actually arrive — at
    /// [`RoutingJob::set_input_bytes`], whose parameter is an `Option<&[u8]>`. This wrapper is
    /// that branch as a function, so `scripts/differential/run.sh p8t1probe` can drive it against
    /// the jar's own `getFileFormat(null)` like every other branch of `:151-227`
    /// (transcript row `SNIFF 46`). Note that the jar's *other* null guard,
    /// `tryToSetInput:336-338`, returns before `getFileFormat` is reached — which is why
    /// `TSI 0` does not cover this line and a `SNIFF` row is needed.
    pub fn sniff_bytes_opt(content: Option<&[u8]>) -> FileFormat {
        match content {
            None => FileFormat::Unknown, // `:152-154`
            Some(content) => FileFormat::sniff_bytes(content),
        }
    }

    /// Whether Java's `getFileFormat(byte[])` **spins for ever** on these bytes.
    ///
    /// `// totalized: RoutingJob.getFileFormat` — quirk #241. Java's shift loop (`:181-187`)
    /// assigns `buffer[0] = buffer[1] … buffer[4] = buffer[5]` and never refills `buffer[5]`, so
    /// a buffer whose first six bytes are all CR/LF converges to six copies of a CR/LF and the
    /// loop's guard is satisfied for ever. The port bounds the loop at [`SHIFT_LOOP_BOUND`] and
    /// answers [`FileFormat::Unknown`] (which is what the converged buffer sniffs as); this
    /// predicate is what says so out loud, and `scripts/differential/run.sh p8t1probe` pins it in
    /// **both** directions — the Java half runs every row on a five-second watchdog, so a row
    /// where the two disagree about hanging is a diff.
    pub fn java_shift_loop_hangs(content: &[u8]) -> bool {
        FileFormat::sniff_bytes_inner(content).1
    }

    /// `(format, java_would_hang)`.
    fn sniff_bytes_inner(content: &[u8]) -> (FileFormat, bool) {
        // `:152-154` — `content == null` answers UNKNOWN. Rust has no null slice; that branch is
        // `FileFormat::sniff_bytes_opt`, and the caller that can produce a null is
        // `RoutingJob::set_input_bytes`, which takes an `Option`.

        // `:156-164` — the first non-whitespace byte decides, and only `'{'` decides anything.
        for &b in content {
            if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
                continue;
            }
            if b == b'{' {
                return (FileFormat::KicadDesignJson, false);
            }
            break;
        }

        // `:168-171` — `read(buffer, 0, 6)`, and every check below is inside `bytesRead == 6`.
        if content.len() < 6 {
            return (FileFormat::Unknown, false);
        }
        let mut buffer = [
            content[0], content[1], content[2], content[3], content[4], content[5],
        ];

        // `:173-178` — the `java.io.ObjectOutputStream` stream header.
        if buffer[0] == 0xAC && buffer[1] == 0xED && buffer[2] == 0x00 && buffer[3] == 0x05 {
            return (FileFormat::Frb, false);
        }

        // `:181-187` — `// totalized:` the unbounded shift loop; see `java_shift_loop_hangs`.
        let mut shifts = 0usize;
        while (buffer[0] == 0x0A || buffer[0] == 0x0D) && shifts < SHIFT_LOOP_BOUND {
            buffer[0] = buffer[1];
            buffer[1] = buffer[2];
            buffer[2] = buffer[3];
            buffer[3] = buffer[4];
            buffer[4] = buffer[5];
            shifts += 1;
        }
        let hangs = shifts == SHIFT_LOOP_BOUND && (buffer[0] == 0x0A || buffer[0] == 0x0D);

        // `:190-199` — "(pcb" or "(PCB", each as a whole.
        if (buffer[0] == 0x28 && buffer[1] == 0x70 && buffer[2] == 0x63 && buffer[3] == 0x62)
            || (buffer[0] == 0x28 && buffer[1] == 0x50 && buffer[2] == 0x43 && buffer[3] == 0x42)
        {
            return (FileFormat::Dsn, hangs);
        }

        // `:202-211` — "(ses" or "(SES", each as a whole.
        if (buffer[0] == 0x28 && buffer[1] == 0x73 && buffer[2] == 0x65 && buffer[3] == 0x73)
            || (buffer[0] == 0x28 && buffer[1] == 0x53 && buffer[2] == 0x45 && buffer[3] == 0x53)
        {
            return (FileFormat::Ses, hangs);
        }

        // `:214-219` — "(rul" with each letter independently case-folded. Only four bytes.
        if buffer[0] == 0x28
            && (buffer[1] == 0x72 || buffer[1] == 0x52)
            && (buffer[2] == 0x75 || buffer[2] == 0x55)
            && (buffer[3] == 0x6C || buffer[3] == 0x4C)
        {
            return (FileFormat::Rules, hangs);
        }

        // `:226` — and the `catch (IOException)` at `:221-224` cannot fire on a byte array.
        (FileFormat::Unknown, hangs)
    }

    /// `// renamed: RoutingJob.getFileFormat` (`RoutingJob.java:230-247`) — format detection from
    /// the path **extension**.
    ///
    /// The whole path is lower-cased and split on `.`, so a dot in a *directory* name takes part:
    /// `"dir.dsn/board"` splits into `["dir", "dsn/board"]` and answers `UNKNOWN`, while
    /// `"dir.dsn/board.ses"` answers `SES`. `parts.length > 1` is the only guard, and
    /// [`java_path::split_on_dot`] is why `"board."` is `UNKNOWN` but `"board.ses."` is `SES`.
    ///
    /// Note the asymmetry with [`FileFormat::sniff_bytes`]: there is no `.drc` case, so a
    /// [`FileFormat::DrcJson`] file is never recognised by its name, and
    /// [`FileFormat::KicadSessionJson`] is produced only by `tryToSetOutputFile:391`.
    pub fn from_path(path: &Path) -> FileFormat {
        let filename = java_path::of_to_string(&path.to_string_lossy()).to_lowercase();
        let parts = java_path::split_on_dot(&filename);
        if parts.len() > 1 {
            let extension = parts[parts.len() - 1].to_lowercase();
            return match extension.as_str() {
                DSN_FILE_EXTENSION => FileFormat::Dsn,
                BINARY_FILE_EXTENSION => FileFormat::Frb,
                "ses" => FileFormat::Ses,
                RULES_FILE_EXTENSION => FileFormat::Rules,
                "scr" => FileFormat::Scr,
                "json" => FileFormat::KicadDesignJson,
                _ => FileFormat::Unknown,
            };
        }
        FileFormat::Unknown
    }

    /// `BoardFileDetails.setFilename`'s default-extension switch (`BoardFileDetails.java:181-189`).
    ///
    /// The five formats with a name and the four without — `DRC_JSON` and both KiCad formats fall
    /// through to the `default -> ""` arm, so a `BoardFileDetails` whose format is
    /// `KICAD_SESSION_JSON` and whose filename has no dot keeps its dotless filename.
    pub fn default_extension(self) -> &'static str {
        match self {
            FileFormat::Ses => "ses",
            FileFormat::Dsn => "dsn",
            FileFormat::Frb => "frb",
            FileFormat::Rules => "rules",
            FileFormat::Scr => "scr",
            _ => "",
        }
    }
}

// =================================================================================================
// RoutingJobState / RoutingStage
// =================================================================================================

/// Port of `core.RoutingJobState` (`core/RoutingJobState.java:4-15`), in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RoutingJobState {
    /// `INVALID` (`:5`) — and the initial value of `RoutingJob.state` (`RoutingJob.java:73`).
    #[default]
    Invalid,
    /// `QUEUED` (`:6`).
    Queued,
    /// `READY_TO_START` (`:7`).
    ReadyToStart,
    /// `RUNNING` (`:8`).
    Running,
    /// `PAUSED` (`:9`).
    Paused,
    /// `COMPLETED` (`:10`).
    Completed,
    /// `TIMED_OUT` (`:11`).
    TimedOut,
    /// `STOPPING` (`:12`).
    Stopping,
    /// `CANCELLED` (`:13`).
    Cancelled,
    /// `TERMINATED` (`:14`).
    Terminated,
}

impl RoutingJobState {
    /// The `name()` of the Java enum constant — what `RoutingResultManifest.finalState` carries
    /// (`core/results/RoutingResultManifest.java:110`).
    pub fn java_name(self) -> &'static str {
        match self {
            RoutingJobState::Invalid => "INVALID",
            RoutingJobState::Queued => "QUEUED",
            RoutingJobState::ReadyToStart => "READY_TO_START",
            RoutingJobState::Running => "RUNNING",
            RoutingJobState::Paused => "PAUSED",
            RoutingJobState::Completed => "COMPLETED",
            RoutingJobState::TimedOut => "TIMED_OUT",
            RoutingJobState::Stopping => "STOPPING",
            RoutingJobState::Cancelled => "CANCELLED",
            RoutingJobState::Terminated => "TERMINATED",
        }
    }

    /// `// totalized: Freerouting.isCliTerminalState` (`Freerouting.java:189-194`) **plus
    /// `INVALID`** — quirk #244.
    ///
    /// Java's set is `COMPLETED | TERMINATED | TIMED_OUT | CANCELLED`. It omits `INVALID`, which
    /// is both the field's initial value (`RoutingJob.java:73`) and what
    /// `RoutingJobScheduler.java:83` assigns when the input is null or is neither DSN nor KiCad
    /// JSON — so `-de x.ses -do y.ses` leaves the CLI in `Freerouting.java:151-158`'s
    /// `while (!isCliTerminalState(...)) Thread.sleep(500)` for ever, at 0 % CPU, with no output
    /// and no message. Plan ruling 7 totalises it: `INVALID` is terminal in the port and the CLI
    /// exits 1 (Task 6 owns the exit code).
    ///
    /// `QUEUED`, `READY_TO_START`, `RUNNING`, `PAUSED` and `STOPPING` stay non-terminal, exactly
    /// as in Java.
    pub fn is_cli_terminal(self) -> bool {
        matches!(
            self,
            RoutingJobState::Completed
                | RoutingJobState::Terminated
                | RoutingJobState::TimedOut
                | RoutingJobState::Cancelled
                // Java's omission, totalised (quirk #244).
                | RoutingJobState::Invalid
        )
    }
}

/// Port of `core.RoutingStage` (`core/RoutingStage.java:4-8`).
///
/// Task 0's roster (`lib.rs` §6) records that this enum has **no control-flow reader** in
/// `src/main/java`: `autoroute/pipeline/RoutingPipeline.java:84`, `:94` and `:121` write it and
/// nothing compares it. The field is carried here anyway, because it is
/// `@SerializedName("stage")` at `RoutingJob.java:75-77` and is therefore read *reflectively* by
/// Gson — so the manifest and MCP surfaces that Tasks 4 and 12 build have a value to report.
/// Nothing in the port branches on it, and `RoutingPipeline::run` does not write it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RoutingStage {
    /// `IDLE` (`:5`) — and the initial value of `RoutingJob.stage` (`RoutingJob.java:77`).
    #[default]
    Idle,
    /// `ROUTING` (`:6`).
    Routing,
    /// `OPTIMIZATION` (`:7`).
    Optimization,
}

impl RoutingStage {
    /// The `name()` of the Java enum constant.
    pub fn java_name(self) -> &'static str {
        match self {
            RoutingStage::Idle => "IDLE",
            RoutingStage::Routing => "ROUTING",
            RoutingStage::Optimization => "OPTIMIZATION",
        }
    }
}

// =================================================================================================
// The two identifiers
// =================================================================================================

/// A `java.util.UUID`-shaped 128-bit identifier, rendered exactly as `UUID.toString()` does.
///
/// # There is no random id in the port
///
/// Java mints `UUID.randomUUID()` for both `RoutingJob.id` (`RoutingJob.java:39`) and
/// `Session.id` (`Session.java:12`). Plan 6 ruling 5 forbids a `rand` dependency and the Global
/// Constraints forbid static mutable state, so the port has no RNG and [`Uuid128::NIL`] is what
/// [`RoutingJob::new`] uses. Nothing in the port reads an id for anything but a display string:
/// `RoutingResultManifest.fromJob` (`core/results/RoutingResultManifest.java:98-135`) never
/// touches `job.id`, and the four `FRLogger` calls that do are rostered. A caller that has a real
/// id — the MCP's `job_id` (ruling AO) — supplies it through [`RoutingJob::with_id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Uuid128([u8; 16]);

/// `RoutingJob.id`'s type (`RoutingJob.java:37-39`).
pub type JobId = Uuid128;

/// `core/Session`'s collapse — plan ruling: the class exists so that
/// `RoutingJobScheduler.enqueueJob:315-318` can validate a `userId` it never uses (quirk #238),
/// and its constructor mutates before it validates (quirk #245). The **validation** is ported
/// ([`validate_session_host`]); the object is not.
pub type SessionId = Uuid128;

impl Uuid128 {
    /// The all-zero UUID — `00000000-0000-0000-0000-000000000000`.
    pub const NIL: Uuid128 = Uuid128([0u8; 16]);

    /// Builds an id from its 16 bytes, big-endian, in `UUID.toString()` order.
    pub fn from_bytes(bytes: [u8; 16]) -> Uuid128 {
        Uuid128(bytes)
    }

    /// The 16 bytes.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// `UUID.toString()` — 8-4-4-4-12 lower-case hex.
    pub fn to_java_string(self) -> String {
        let h: Vec<String> = self.0.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "{}{}{}{}-{}{}-{}{}-{}{}-{}{}{}{}{}{}",
            h[0],
            h[1],
            h[2],
            h[3],
            h[4],
            h[5],
            h[6],
            h[7],
            h[8],
            h[9],
            h[10],
            h[11],
            h[12],
            h[13],
            h[14],
            h[15]
        )
    }

    /// `id.toString().substring(0, 6).toUpperCase()` — the six characters `RoutingJob`'s two
    /// constructors build `name` and `shortName` out of (`RoutingJob.java:136-137`, `:144-147`).
    pub fn short_upper6(self) -> String {
        self.to_java_string()
            .chars()
            .take(6)
            .collect::<String>()
            .to_uppercase()
    }
}

/// `Session`'s constructor host handling (`core/Session.java:27-43`), **validating before it
/// assigns**.
///
/// Returns the normalised host on success. Quirk #245: Java assigns `this.host` at `:34` and only
/// then checks `host.split("/").length != 2` at `:37`, so the `IllegalArgumentException` is thrown
/// out of a partly-constructed object — and `SessionManager.setPrimarySession` does the same at
/// `:157` / `:162`. The port validates first. **The difference is unobservable**: the exception
/// escapes the constructor, so no caller anywhere in `src/main/java` ever holds the half-built
/// `Session`, and the port's value-returning shape cannot expose one either. Recorded rather than
/// hidden.
///
/// The normalisation itself (`:31-33`) is what makes the check passable: `null` and a blank host
/// both become `"Unknown/0.0"`, which has exactly two parts.
///
/// Note what `split("/")` means here: Java drops trailing empty parts, so `"a/"` has length **1**
/// and is rejected, while `"/a"` has length 2 and is **accepted**.
pub fn validate_session_host(host: Option<&str>) -> Result<String, Error> {
    // `:31-33` — normalise null or blank to the safe default.
    let host = match host {
        Some(h) if !h.trim().is_empty() => h,
        _ => "Unknown/0.0",
    };
    // `:37` — `host.split("/").length != 2`, with Java's trailing-empty removal.
    let parts = if host.contains('/') {
        let mut v: Vec<&str> = host.split('/').collect();
        while v.last().is_some_and(|p| p.is_empty()) {
            v.pop();
        }
        v
    } else {
        vec![host]
    };
    if parts.len() != 2 {
        // `:38-42`, verbatim.
        return Err(Error::Session(format!(
            "Invalid host value: '{host}'. It must contain the host name and version separated by '/'."
        )));
    }
    Ok(host.to_string())
}

// =================================================================================================
// RoutingJob
// =================================================================================================

/// Port of `core.RoutingJob`'s **file / format / state** half (`core/RoutingJob.java`, 548 lines).
///
/// # Fields that are deliberately absent
///
/// * `priority` (`:79-81`) and `compareTo` (`:409-418`) — rostered in `lib.rs` §6:
///   `RoutingJobPriority.getValue()` has no caller anywhere and the ordering it exists for belongs
///   to a queue the port does not have.
/// * the four listener lists (`:46-53`) and the four `fire*` methods — spec §10 replaces them.
/// * `thread` and `board` (`:115-116`) — the port's routing thread is
///   [`crate::RoutingPipeline::run`]'s stack frame and the board is its `&mut Board` parameter.
/// * `timeoutAt` (`:117`) — [`crate::Deadline`] (Task 0) carries the monitor thread's two
///   observable instants instead.
///
/// (`resourceUsage` `:111-113` was on this list until Task 4; it is now the
/// [`RoutingJob::resource_usage`] field below, because `RoutingResultManifest.fromJob:114` reads
/// it off the job.)
#[derive(Debug, Clone)]
pub struct RoutingJob {
    /// `id` (`:37-39`). See [`Uuid128`] for why it is not random.
    pub id: JobId,
    /// `createdAt` (`:41-43`). Java's is a wall-clock `java.time.Instant`; the port uses the
    /// monotonic clock, because the only reader of any of the three instants is
    /// [`RoutingJob::get_duration`] and a monotonic clock cannot run backwards.
    pub created_at: Instant,
    /// `shortName` (`:55-57`) — `"N/A"` until a constructor overwrites it.
    pub short_name: String,
    /// `name` (`:59-61`), overwritten by `setInputFromFile:457` once the input format is known.
    pub name: String,
    /// `startedAt` (`:63-65`).
    pub started_at: Option<Instant>,
    /// `finishedAt` (`:67-69`).
    pub finished_at: Option<Instant>,
    /// `state` (`:71-73`), initially `INVALID`.
    pub state: RoutingJobState,
    /// `stage` (`:75-77`), initially `IDLE`. See [`RoutingStage`] — written by nothing in the
    /// port, carried for the serialisation surface.
    pub stage: RoutingStage,
    /// `sessionId` (`:83-85`).
    pub session_id: Option<SessionId>,
    /// `input` (`:87-89`).
    pub input: Option<BoardFileDetails>,
    /// `output` (`:91-93`).
    pub output: Option<BoardFileDetails>,
    /// `rules` (`:95-97`).
    pub rules: Option<BoardFileDetails>,
    /// `drc` (`:99-101`).
    pub drc: Option<BoardFileDetails>,
    /// `routerSettings` (`:103-105`) — `new RouterSettings()` until merge #1 replaces it
    /// (`Freerouting.java:146`).
    pub router_settings: RouterSettings,
    /// `drcSettings` (`:107-109`).
    pub drc_settings: DesignRulesCheckerSettings,
    /// `resourceUsage` (`:111-113`) — `new RouterJobResourceUsage()`, and never anything else on
    /// this port: what fills it in Java is the monitor thread of
    /// `RoutingJobSchedulerActionThread:55-90`, which is rostered rather than ported (quirk #237).
    /// [`RoutingResultManifest::from_job`](crate::RoutingResultManifest::from_job) copies it into
    /// the manifest's `resource_usage`, which `p8t2` normalises out (plan ruling 8).
    pub resource_usage: RouterJobResourceUsage,
    /// `currentPass` (`:130-132`, private). Quirk #230 already records that this under-reports by
    /// one on a `maxPasses`-capped exit; `PipelineResult::passes_run` is the number the port's
    /// own surfaces use.
    current_pass: i32,
    /// `isCancelledByUser` (`:118`, private).
    is_cancelled_by_user: bool,
}

impl Default for RoutingJob {
    /// `RoutingJob()` (`:134-138`) — "we need a parameterless constructor for the serialization".
    fn default() -> RoutingJob {
        let id = JobId::NIL;
        let short6 = id.short_upper6();
        RoutingJob {
            id,
            created_at: Instant::now(),
            short_name: short6.clone(),
            name: format!("J-{short6}"),
            started_at: None,
            finished_at: None,
            state: RoutingJobState::Invalid,
            stage: RoutingStage::Idle,
            session_id: None,
            input: None,
            output: None,
            rules: None,
            drc: None,
            // `:105` is `= new RouterSettings()`, **not** an all-null one: the no-arg
            // constructor allocates `optimizer`, `scoring` and `fanout`
            // (`RouterSettings.java:119-124`), and `RouterSettings::default()` does not. The
            // difference is observable — it is `settings_snapshot` in the result manifest, and it
            // is whether `fromJob:118`'s `routerSettings.scoring != null` guard passes at all.
            router_settings: RouterSettings::new(),
            drc_settings: DesignRulesCheckerSettings::default(),
            resource_usage: RouterJobResourceUsage::default(),
            current_pass: 0,
            is_cancelled_by_user: false,
        }
    }
}

impl RoutingJob {
    /// `RoutingJob(UUID sessionId)` (`:140-148`).
    ///
    /// `shortName` becomes `<session6>\<job6>` — with a literal **backslash**, on every platform
    /// (`:146`). It is the prefix of every `logInfo`/`logWarning`/`logError`/`logDebug` message.
    pub fn new(session_id: SessionId) -> RoutingJob {
        RoutingJob::with_id(session_id, JobId::NIL)
    }

    /// [`RoutingJob::new`] with the job id supplied rather than defaulted — the port's answer to
    /// `UUID.randomUUID()`, for the one caller (the MCP, ruling AO's `job_id`) that has a real id.
    pub fn with_id(session_id: SessionId, id: JobId) -> RoutingJob {
        let mut job = RoutingJob {
            id,
            ..RoutingJob::default()
        };
        let short6 = id.short_upper6();
        job.name = format!("J-{short6}");
        job.session_id = Some(session_id);
        job.short_name = format!("{}\\{}", session_id.short_upper6(), short6);
        job
    }

    // ── the accessors ────────────────────────────────────────────────────────────────────────

    /// `getCurrentPass` (`:249-252`).
    pub fn get_current_pass(&self) -> i32 {
        self.current_pass
    }

    /// `setCurrentPass` (`:254-257`).
    pub fn set_current_pass(&mut self, current_pass: i32) {
        self.current_pass = current_pass;
    }

    /// `isCancelledByUser` (`:120-123`).
    pub fn is_cancelled_by_user(&self) -> bool {
        self.is_cancelled_by_user
    }

    /// `setCancelledByUser` (`:125-128`).
    pub fn set_cancelled_by_user(&mut self, cancelled: bool) {
        self.is_cancelled_by_user = cancelled;
    }

    /// `getInput` (`:420-423`).
    pub fn get_input(&self) -> Option<&BoardFileDetails> {
        self.input.as_ref()
    }

    /// `getDuration` (`:259-268`) — `null` before the job starts, the finished span once it has
    /// finished, and the running span in between.
    pub fn get_duration(&self) -> Option<Duration> {
        let started = self.started_at?;
        match self.finished_at {
            Some(finished) => Some(finished.saturating_duration_since(started)),
            None => Some(Instant::now().saturating_duration_since(started)),
        }
    }

    /// The `"[" + shortName + "] "` prefix that `logInfo`/`logWarning`/`logError`/`logDebug`
    /// (`:525-547`) put on every message. The four helpers themselves are rostered below; this is
    /// the one part of them a `tracing` caller in Task 6 needs.
    pub fn log_prefix(&self) -> String {
        format!("[{}] ", self.short_name)
    }

    // ── the input ────────────────────────────────────────────────────────────────────────────

    /// `setInput(byte[])` (`:270-275`) — allocates a fresh [`BoardFileDetails`] and then runs
    /// `tryToSetInput` (`:335-349`) on it.
    ///
    /// Two things the return value hides, both of which Task 6 depends on:
    ///
    /// * `this.input` is assigned **before** the content is examined, so it is never `null`
    ///   afterwards — which is why `Freerouting.java:108`'s `if (routingJob.input == null)` guard
    ///   can only fire when `setInput` *threw* (an unreadable file), never when it merely failed
    ///   to recognise the bytes.
    /// * on `false` the details keep `format == UNKNOWN`, `size == 0` and `crc32 == 0`, because
    ///   `:343`'s `setData` is inside the `if`.
    pub fn set_input_bytes(&mut self, content: Option<&[u8]>) -> bool {
        self.input = Some(BoardFileDetails::default());
        // `:273` registers `fireInputUpdatedEvent` as the listener; spec §10 has no events.
        self.try_to_set_input(content)
    }

    /// `tryToSetInput` (`:335-349`, private).
    fn try_to_set_input(&mut self, content: Option<&[u8]>) -> bool {
        let Some(content) = content else {
            return false; // `:336-338`
        };
        let input = self
            .input
            .as_mut()
            .expect("set_input_bytes assigns `input` before calling this");
        input.format = FileFormat::sniff_bytes(content);
        if input.format != FileFormat::Unknown {
            input.set_data(content.to_vec());
            return true;
        }
        false
    }

    /// `setInput(String)` / `setInput(File)` / `setInputFromFile` (`:277-285`, `:425-461`).
    ///
    /// The 37 lines of `setInputFromFile` are where the **default output name** comes from, and
    /// getting them right is what makes quirk #243's `-do out.txt` behaviour reproducible in
    /// Task 6. In order:
    ///
    /// 1. read the whole file (an I/O failure propagates — this is the only way `job.input` stays
    ///    `None`), then `setInput(content)` — so the format comes from the **bytes** first;
    /// 2. `input.setFilename(absolutePath)` (`:432`);
    /// 3. **only if the bytes said nothing**, re-derive the format from the extension (`:433-436`)
    ///    — which is how `e.dsn` containing `"hello"` still becomes a DSN job, with `size == 0`
    ///    and `crc32 == 0` because `setData` never ran;
    /// 4. FRB → `<input>.frb`, DSN → `<input>.ses`, KiCad design JSON → `<input>.json`
    ///    (`:438-454`). Two of those three derive the **input's own path**, because
    ///    [`RoutingJob::change_file_extension`] returns its argument unchanged when the extension
    ///    already matches: an `.frb` input's default output *is* the input file, and so is a
    ///    `.json` one. A `SES` or `RULES` input derives **no** output at all;
    /// 5. `name = input.getFilenameWithoutExtension()` if the format is known (`:456-458`) —
    ///    otherwise the job keeps its id-derived `J-XXXXXX`.
    pub fn set_input(&mut self, input_file: &Path) -> Result<(), Error> {
        // `:428-429` — `new FileInputStream(inputFile).readAllBytes()`.
        let content = std::fs::read(input_file)?;

        self.set_input_bytes(Some(&content)); // `:431`
        let absolute = java_path::to_absolute_path(&input_file.to_string_lossy());
        {
            let input = self.input.as_mut().expect("set_input_bytes assigns it");
            input.set_filename(Some(&absolute)); // `:432`
            if input.format == FileFormat::Unknown {
                // `:433-436` — the extension is the fallback, and only the fallback.
                input.format = FileFormat::from_path(Path::new(&input.get_absolute_path()));
            }
        }

        let input_format = self.input.as_ref().map(|i| i.format);
        let input_absolute = self
            .input
            .as_ref()
            .map(|i| i.get_absolute_path())
            .unwrap_or_default();

        // `:438-454` — three formats derive a default output, and the other six derive none.
        let derived = match input_format {
            Some(FileFormat::Frb) => Some(BINARY_FILE_EXTENSION),
            Some(FileFormat::Dsn) => Some(SES_FILE_EXTENSION),
            Some(FileFormat::KicadDesignJson) => Some("json"),
            _ => None,
        };
        if let Some(extension) = derived {
            let mut output = BoardFileDetails::default();
            output.set_filename(Some(&RoutingJob::change_file_extension(
                &input_absolute,
                extension,
            )));
            self.output = Some(output);
        }

        // `:456-458`.
        if input_format != Some(FileFormat::Unknown)
            && let Some(input) = self.input.as_ref()
        {
            self.name = input.get_filename_without_extension();
        }
        Ok(())
    }

    // ── the rules ────────────────────────────────────────────────────────────────────────────

    /// `setRules(byte[])` (`:287-293`).
    ///
    /// Unlike [`RoutingJob::set_input_bytes`], the format is **forced** to `RULES` before
    /// `setData` runs — and `setData` then re-sniffs the bytes and overwrites it (`:291` calls
    /// `BoardFileDetails.setData`, whose `:113` is `this.format = getFileFormat(this.dataBytes)`).
    /// So `job.rules.format` is `RULES` only when the file really starts with `(rul`; a `.rules`
    /// file that does not is left `UNKNOWN`. The method returns `true` either way.
    pub fn set_rules_bytes(&mut self, content: &[u8]) -> bool {
        let mut rules = BoardFileDetails::default();
        rules.format = FileFormat::Rules; // `:290`
        rules.set_data(content.to_vec()); // `:291`
        self.rules = Some(rules);
        true
    }

    /// `setRules(String)` / `setRules(File)` (`:295-310`).
    ///
    /// **A missing file is silently ignored** (`:302`'s `rulesFile.exists()` guard), leaving
    /// `job.rules` as it was — which for the CLI means `None`, and
    /// `Freerouting.java:132`'s `if (routingJob.rules != null …)` then skips the whole
    /// `RulesFileSettings` tier without a warning. Only a file that exists and cannot be read
    /// reaches the `catch` at `Freerouting.java:137`.
    ///
    /// Note the order, which differs from `setInputFromFile`: `setFilename` runs **before**
    /// `setData` here (`:306` then `:307`), so the extension-derived format is what `setData`'s
    /// re-sniff overwrites, not the other way round. With `format` already `RULES` at `:305`,
    /// `setFilename:174`'s re-sniff is skipped and `:180-194` may append a `.rules` extension to a
    /// dotless name.
    pub fn set_rules(&mut self, rules_file: &Path) -> Result<(), Error> {
        if !rules_file.exists() {
            return Ok(()); // `:302`
        }
        let content = std::fs::read(rules_file)?;
        let mut rules = BoardFileDetails::default();
        rules.format = FileFormat::Rules; // `:305`
        // `:306` — `rulesFile.getName()`, the bare name, NOT the absolute path.
        let name = rules_file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        rules.set_filename(Some(&name));
        rules.set_data(content); // `:307`
        self.rules = Some(rules);
        Ok(())
    }

    // ── the output ───────────────────────────────────────────────────────────────────────────

    /// `tryToSetOutputFile` (`:377-397`) — **the return value is a real decision and every caller
    /// must say in writing whether it ignores it.**
    ///
    /// `Freerouting.java:123` ignores it (quirk label L, Task 6's row): `-do out.txt` answers
    /// `false`, `job.output` keeps whatever `setInputFromFile` derived — `<input>.ses` — and the
    /// SES bytes are then written to `out.txt` anyway by `Files.write(initialOutputFile)`. Task 6
    /// owns that consequence; Task 1 owns the predicate.
    ///
    /// The accepted set is `DSN | FRB | SES | SCR | KICAD_DESIGN_JSON` (`:384-388`). **`RULES` is
    /// not in it**, so `-do out.rules` is rejected even though `getFileFormat` recognises the
    /// extension. `KICAD_DESIGN_JSON` is rewritten to `KICAD_SESSION_JSON` at `:391` — the one
    /// place in the tree that produces that constant, and the first half of quirk label T
    /// (Task 10's), because `BoardFileDetails::set_data` will re-sniff the written bytes and turn
    /// it back into `KICAD_DESIGN_JSON`.
    ///
    /// `// not reachable: RoutingJob.fireInputUpdatedEvent` at `:390` — the *output* details are
    /// given an updated-listener that fires the **input** event. It is a copy-paste bug
    /// (`setInputFromFile:440`, `:446` and `:452` all use `fireOutputUpdatedEvent` correctly), and
    /// it is quirk #243. The port has no events at all (spec §10), so there is nothing to
    /// reproduce — the row exists so the defect is recorded rather than silently normalised away.
    pub fn try_to_set_output_file(&mut self, output_file: Option<&Path>) -> bool {
        let Some(output_file) = output_file else {
            return false; // `:378-380`
        };
        let ff = FileFormat::from_path(output_file); // `:382`
        if !matches!(
            ff,
            FileFormat::Dsn
                | FileFormat::Frb
                | FileFormat::Ses
                | FileFormat::Scr
                | FileFormat::KicadDesignJson
        ) {
            return false; // `:394-396`
        }
        let mut output = BoardFileDetails::from_file(output_file); // `:389`
        output.format = if ff == FileFormat::KicadDesignJson {
            FileFormat::KicadSessionJson // `:391`
        } else {
            ff
        };
        self.output = Some(output);
        true
    }

    /// `getRulesFile` (`:312-315`) — the `.rules` file **beside the output**, not beside the
    /// input. `None` where Java would throw an NPE for want of an output.
    pub fn get_rules_file(&self) -> Option<PathBuf> {
        let output = self.output.as_ref()?;
        Some(PathBuf::from(RoutingJob::change_file_extension(
            &output.get_absolute_path(),
            RULES_FILE_EXTENSION,
        )))
    }

    /// `getEagleScriptFile` (`:317-321`).
    pub fn get_eagle_script_file(&self) -> Option<PathBuf> {
        let output = self.output.as_ref()?;
        Some(PathBuf::from(RoutingJob::change_file_extension(
            &output.get_absolute_path(),
            EAGLE_SCRIPT_FILE_EXTENSION,
        )))
    }

    /// `setDummyInputFile` (`:323-333`).
    ///
    /// The guard is `filename.toLowerCase().endsWith(DSN_FILE_EXTENSION)` — `endsWith("dsn")`,
    /// **not** `endsWith(".dsn")` — so `"boarddsn"` is accepted as a DSN and `"board.dsn.bak"` is
    /// not. Transcribed as written. Note also that `output` is replaced with an empty
    /// `BoardFileDetails` unconditionally, even when the name is rejected.
    pub fn set_dummy_input_file(&mut self, filename: Option<&str>) {
        self.input = Some(BoardFileDetails::default());
        self.output = Some(BoardFileDetails::default());
        if let Some(filename) = filename
            && filename.to_lowercase().ends_with(DSN_FILE_EXTENSION)
        {
            let input = self.input.as_mut().expect("just assigned");
            input.format = FileFormat::Dsn;
            input.set_filename(Some(filename));
        }
    }

    // ── changeFileExtension ──────────────────────────────────────────────────────────────────

    /// `changeFileExtension` (`:352-374`, private) — quirk #242.
    ///
    /// Three returns, and they are not the same kind of thing:
    ///
    /// * **extension already matches** (`:361-363`): returns `filePath.toString()`, i.e. the
    ///   caller's own string, **relative if it was relative** — while the other two returns are
    ///   built from `getParent().toAbsolutePath()` and are always absolute. So
    ///   `changeFileExtension("dir/out.ses", "ses")` is `"dir/out.ses"` and
    ///   `changeFileExtension("dir/out.dsn", "ses")` is `"<cwd>/dir/out.ses"`. **Reproduced.**
    /// * **extension differs** (`:364-369`): the *lower-cased* extension is compared but the
    ///   *original* one is what gets cut, so `"/tmp/out.DSN"` → `"/tmp/out.ses"` is right by
    ///   luck — the two have the same length. The new extension is never case-folded, so
    ///   `changeFileExtension("/tmp/out.ses", "SES")` is `"/tmp/out.SES"`.
    /// * **no extension** (`:372-373`): appends. `"/tmp/out."` takes this arm (Java's `split`
    ///   drops the trailing empty part) and becomes `"/tmp/out..ses"`.
    ///
    /// `// totalized: RoutingJob.changeFileExtension` — `:356` calls
    /// `filePath.getParent().toAbsolutePath()` and `:357` calls `filePath.getFileName()`, both
    /// **before** any branch, so **every bare filename NPEs**: `changeFileExtension("out.ses",
    /// "ses")` throws rather than answering `"out.ses"`. The port totalises each null to the empty
    /// string and carries on, which makes `Path.of("", name)` just `name` — so a bare name answers
    /// itself with the new extension, and `"/"` (which has neither a parent nor a file name)
    /// answers `".ses"`. `scripts/differential/run.sh p8t1probe` pins both halves: the Java side
    /// prints the exception class, the Rust side the value.
    pub fn change_file_extension(filename: &str, new_file_extension: &str) -> String {
        let normalized = java_path::of_to_string(filename); // `:353`

        // `:356` — `filePath.getParent().toAbsolutePath().toString()`, totalised.
        let original_full_path_without_filename = match java_path::parent_of_normalized(&normalized)
        {
            Some(parent) => java_path::to_absolute_path(&parent),
            None => String::new(),
        };
        // `:357` — `filePath.getFileName().toString()`, totalised.
        let original_filename = java_path::file_name_of_normalized(&normalized).unwrap_or_default();

        let name_parts = java_path::split_on_dot(&original_filename); // `:358`
        if name_parts.len() > 1 {
            let extension = name_parts[name_parts.len() - 1].to_lowercase(); // `:360`
            if extension == new_file_extension {
                return normalized; // `:362` — the caller's own, possibly relative, string
            }
            // `:364-367` — the ORIGINAL extension's length is what is cut.
            let keep = original_filename.len() - extension.len() - 1;
            let new_filename = format!("{}.{new_file_extension}", &original_filename[..keep]);
            return java_path::join2(&original_full_path_without_filename, &new_filename); // `:369`
        }

        // `:372-373`.
        java_path::join2(
            &original_full_path_without_filename,
            &format!("{original_filename}.{new_file_extension}"),
        )
    }
}

// =================================================================================================
// The roster for this file — one line per public Java method `audit-port.sh` must account for
// =================================================================================================
//
// (Plan 2 ruling 13: the marker's Java method name sits on the same line as the marker.)
//
// ── core/RoutingJob — the queue half and the event half ─────────────────────────────────────────
// not ported: RoutingJob.compareTo — orders by `priority.ordinal()` (`:411-413`), and both the
//   `RoutingJobPriority` field and the `LinkedList` it orders are rostered in `lib.rs` §4/§6. A
//   single-shot CLI enqueues nothing.
// not ported: RoutingJob.setSettings — `routerSettings.applyNewValuesFrom(settings) > 0` plus a
//   settings-updated event (`:463-471`). Plan 4 owns the merge (`fr_settings::SettingsMerger`), and
//   Task 6 assigns `job.router_settings` from `resolve_headless`'s result directly, which is what
//   `Freerouting.java:146` does too.
// not ported: RoutingJob.addSettingsUpdatedEventListener — spec §10; `core/events/**` is rostered.
// not ported: RoutingJob.addInputUpdatedEventListener — spec §10.
// not ported: RoutingJob.addOutputUpdatedEventListener — spec §10.
// not ported: RoutingJob.addLogEntryAddedEventListener — spec §10.
// not ported: RoutingJob.fireSettingsUpdatedEvent — spec §10.
// not ported: RoutingJob.fireInputUpdatedEvent — spec §10, and see the `// not reachable:` note on
//   `try_to_set_output_file` for the one site where firing it is itself a bug.
// not ported: RoutingJob.fireOutputUpdatedEvent — spec §10.
// not ported: RoutingJob.fireLogEntryAddedEvent — spec §10.
// not ported: RoutingJob.logInfo — `FRLogger.info("[" + shortName + "] " + m, id)` plus the event
//   (`:525-529`). The port has no `FRLogger` (Global Constraints); Task 6 emits the CLI's message
//   set through `tracing` on stderr, prefixing it with `RoutingJob::log_prefix`.
// not ported: RoutingJob.logWarning — as `logInfo` (`:531-535`).
// not ported: RoutingJob.logError — as `logInfo`, plus a `Throwable` (`:537-541`).
// not ported: RoutingJob.logDebug — as `logInfo` (`:543-547`).
// not ported: RoutingJob.getInputFileDetails — `new BoardFileDetails(input.getFile()).toString()`
//   (`:399-402`), i.e. a Gson dump of a **freshly re-read** copy of the file. Its only callers are
//   `api/v1/JobInputResource` and `gui/**`, both rostered; the port's JSON surfaces are the
//   manifest (Task 4) and the MCP payload (Task 12), which use ruling AO's field names.
// not ported: RoutingJob.getOutputFileDetails — as `getInputFileDetails` (`:404-407`).
//
// ── core/Session — collapsed to `SessionId` + `validate_session_host` ───────────────────────────
// not ported: Session.addJob — `RoutingJobScheduler.getInstance().enqueueJob(job)` (`Session.java:50-52`).
//   The scheduler, its queue and its daemon are rostered in `lib.rs` §4 (quirks #238/#239); the
//   port calls `RoutingPipeline::run` directly.
// not ported: Session.getId — the field is the value here (`Session.java:59-61`); `SessionId` is a
//   `Copy` newtype and has no accessor to port.
