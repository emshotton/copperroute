//! `core/BoardFileDetails.java` (219 lines) — the bytes of one file plus everything the job model
//! derives from them.
//!
//! The field names are `api/dto/BoardFilePayload`'s, which controller ruling AO keeps so that an
//! agent written against the Java HTTP server is not gratuitously broken by the port's MCP:
//! `size`, `crc32`, `format`, `statistics`, `filename`, `path`.
//!
//! Two of its methods carry quirks that reach the CLI's observable behaviour.
//! [`BoardFileDetails::set_data`] re-sniffed the bytes it was given, which is the mechanism of
//! quirk **#289** (`-do out.json` writes the board as it was before routing) — **fixed in Plan 9
//! Task 3**: the format is a parameter now, and the sniff sits at the three callers that want it.
//! [`BoardFileDetails::set_filename`] runs two Windows-only string rewrites unconditionally, one
//! of which is a regex bug (quirk #246), and that one is still reproduced.

use crate::job::{FileFormat, java_path};
use fr_router::score::BoardStatistics;
use std::path::{Path, PathBuf};

/// Port of `core.BoardFileDetails` (`BoardFileDetails.java:25-218`).
#[derive(Debug, Clone, Default)]
pub struct BoardFileDetails {
    /// `size` (`:31-32`) — the length of the data, not of the file on disk.
    pub size: u64,
    /// `crc32` (`:35-36`). Java's field is a `long` holding an unsigned 32-bit value; the port
    /// keeps it `u32` and every transcript prints the same decimal.
    pub crc32: u32,
    /// `format` (`:39-40`), initially `UNKNOWN`.
    pub format: FileFormat,
    /// `statistics` (`:42-43`).
    ///
    /// `// added in Task 2: BoardStatistics` — `setData:116` fills this with
    /// `new BoardStatistics(this.dataBytes, this.format)`, the **byte-scraping** constructor
    /// (`core/scoring/BoardStatistics.java:436-554`), which is Task 2's port. Until it lands this
    /// stays at its `new BoardStatistics()` default, which is exactly what Java's field
    /// initialiser at `:43` is.
    pub statistics: BoardStatistics,
    /// `filename` (`:46-47`) — the name only, never a path. `protected` in Java, which is why
    /// `P8T1Probe` declares `package app.freerouting.core`.
    pub filename: String,
    /// `directoryPath` (`:50-51`) — serialised as `"path"`.
    pub directory_path: String,
    /// `dataBytes` (`:53`), `transient`.
    data_bytes: Vec<u8>,
}

impl BoardFileDetails {
    /// `BoardFileDetails(File)` (`:58-67`).
    ///
    /// The order matters and is not the obvious one: `setFilename(file.getAbsolutePath())` runs
    /// **first**, so the extension decides the format, and only then is the file read — and if it
    /// cannot be read the `catch (IOException)` at `:64-66` swallows the failure and leaves the
    /// defaults in place. So a `BoardFileDetails` for a file that does not exist is a perfectly
    /// good object with a format, a name, a directory, `size == 0` and `crc32 == 0`. That is the
    /// state `RoutingJob::try_to_set_output_file` leaves `job.output` in for every real CLI run,
    /// because the output file has just been deleted (`Freerouting.java:117-121`).
    pub fn from_file(file: &Path) -> BoardFileDetails {
        let mut details = BoardFileDetails::default();
        let absolute = java_path::to_absolute_path(&file.to_string_lossy());
        details.set_filename(Some(&absolute)); // `:60`
        if let Ok(data) = std::fs::read(file) {
            // fixed: T3 (#289) — the sniff Java did inside `setData` (`:113`), moved here where it
            // is Java's own answer: for a file that exists, the *bytes* decide the format and the
            // extension `setFilename` derived above is only the fallback.
            let format = FileFormat::sniff_bytes(&data);
            details.set_data(data, format); // `:63`
        }
        details
    }

    /// `BoardFileDetails(BasicBoard)` (`:69-72`) — the statistics-only constructor. It sets
    /// **nothing else**: no bytes, no size, no CRC, no filename. Its callers are
    /// `RoutingJobSchedulerActionThread.setJobOutput:262` and `api/v1/**`.
    pub fn from_board(board: &mut fr_board::Board) -> BoardFileDetails {
        BoardFileDetails {
            statistics: BoardStatistics::new(board), // `:71`
            ..BoardFileDetails::default()
        }
    }

    /// `calculateCrc32(InputStream)` (`:74-87`) — `java.util.zip.CRC32` over the whole stream.
    ///
    /// That is CRC-32/ISO-HDLC: reflected, polynomial `0xEDB88320`, initial and final value
    /// `0xFFFFFFFF`. Hand-written because no workspace dependency provides one and the Global
    /// Constraints forbid adding a `crc32` crate; `"123456789"` → `0xCBF43926` is the standard
    /// check vector and `crates/fr-core/tests/job.rs` asserts it.
    ///
    /// Java's 8 192-byte read buffer is not modelled: it changes nothing about the answer, and the
    /// `p8t1probe` transcript covers 8 191, 8 192 and 8 193 bytes to prove it. The
    /// `catch (IOException)` at `:83-85` logs and returns the CRC **of what was read so far**;
    /// the port's callers hand it a slice, which cannot fail.
    ///
    // renamed: BoardFileDetails.calculateCrc32 — `audit-port.sh`'s mechanical snake-case is
    // `calculate_crc_32` (it splits letter-then-digit); the port spells it `calculate_crc32`,
    // which is what every other `crc32` identifier in the workspace uses.
    pub fn calculate_crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    /// `getAbsolutePath` (`:96-98`) — `Path.of(directoryPath, filename).toString()`.
    ///
    /// It is not necessarily absolute. `setFilename("/board.dsn")` leaves `directoryPath` empty
    /// (the trailing-separator strip at `:164` eats the root), so this answers `"board.dsn"`.
    pub fn get_absolute_path(&self) -> String {
        java_path::join2(&self.directory_path, &self.filename)
    }

    /// `getData` (`:100-102`) — Java wraps the bytes in a fresh `ByteArrayInputStream` each call.
    pub fn get_data(&self) -> &[u8] {
        &self.data_bytes
    }

    /// `setData(byte[])` (`:104-119`) — replaces the bytes, the size and the CRC.
    ///
    /// fixed: T3 (#289) — **the re-sniff at `:113` is gone, and the format is a parameter.**
    /// Java overwrites whatever the caller had put in `format` with
    /// `RoutingJob.getFileFormat(data)`, and that costs it two things:
    ///
    /// * `setRules(byte[]):290-291` sets `RULES` and then loses it again unless the bytes really
    ///   start with `(rul`;
    /// * `setJobOutput` (`RoutingJobSchedulerActionThread.java:266`) sets `KICAD_SESSION_JSON` and
    ///   then loses it again, because JSON starts with `{` and `getFileFormat(byte[]):160-161`
    ///   answers `KICAD_DESIGN_JSON` — after which neither `:275`'s `== KICAD_SESSION_JSON` nor
    ///   `:282`'s `== SES` ever matches again and **every later `setJobOutput` call is a no-op**.
    ///   That is the mechanism of quirk #289: `-do out.json` keeps what the *first* call wrote,
    ///   which is the board as loaded. The SES path escapes it only because `(ses` re-detects as
    ///   `SES`.
    ///
    /// The register's own suggested fix is *"give `BoardFileDetails` a `setData(byte[],
    /// FileFormat)` that does not re-sniff"*, and this is that. The sniff itself has not been
    /// deleted — it has moved to the callers that actually want it, one line each, where it is
    /// visible:
    ///
    /// | caller | format it passes | why |
    /// |---|---|---|
    /// | [`BoardFileDetails::from_file`] (`:63`) | [`FileFormat::sniff_bytes`] | Java's, kept: an input file's bytes outrank its extension |
    /// | `RoutingJob::try_to_set_input` (`:343`) | the format it just sniffed | Java sniffed the same bytes twice; now once |
    /// | `RoutingJob::set_rules_bytes` / `set_rules` (`:291`, `:307`) | [`FileFormat::sniff_bytes`] | Java's `RULES`-then-lose-it, transcribed at the site that has it |
    /// | `commands::route::set_job_output` (`:279`, `:288`) | the format it serialised **for** | the fix: `KICAD_SESSION_JSON` stays `KICAD_SESSION_JSON` |
    ///
    /// So the only behaviour that moves is the last row's, which is the one the register asked to
    /// move. `crates/fr-core/tests/job.rs::set_data_keeps_the_format_it_was_given` is the gate.
    pub fn set_data(&mut self, data: Vec<u8>, format: FileFormat) {
        self.size = data.len() as u64; // `:107`
        self.crc32 = BoardFileDetails::calculate_crc32(&data); // `:108-110`
        // `:112-113` — `this.format = RoutingJob.getFileFormat(data);`, which is what the
        // parameter replaces. See the table above for what each caller passes and why.
        self.format = format;
        // `:115-116` — `new BoardStatistics(this.dataBytes, this.format)`.
        // added in Task 2: BoardStatistics — the byte-scraping constructor (`BoardStatistics.java:436-554`).
        self.data_bytes = data; // `:106`
        // `:118` — `fireUpdatedEvent()`; spec §10 has no events.
    }

    /// `getFile` (`:126-132`) — `null` when no filename has been set.
    pub fn get_file(&self) -> Option<PathBuf> {
        if self.filename.is_empty() {
            return None;
        }
        Some(PathBuf::from(java_path::join2(
            &self.directory_path,
            &self.filename,
        )))
    }

    /// `getDirectoryPath` (`:134-137`).
    pub fn get_directory_path(&self) -> &str {
        &self.directory_path
    }

    /// `getFilename` (`:139-142`).
    pub fn get_filename(&self) -> &str {
        &self.filename
    }

    /// `setFilename(String)` (`:144-197`, 49 lines) — quirk #246.
    ///
    /// Four things happen and three of them are surprising.
    ///
    /// 1. **The split is decided by the argument, the values by its absolutised form** (`:156-169`).
    ///    `filename.contains(File.separator)` tests the *caller's* string; `path.getParent()` and
    ///    `path.getFileName()` read `Path.of(filename).toAbsolutePath()`. So `"board.dsn"` gets an
    ///    **empty** `directoryPath` even though the absolute path was computed, while
    ///    `"dir/board.dsn"` gets `<cwd>/dir`.
    /// 2. **The Windows-only string surgery runs unconditionally** (`:161-166`), on POSIX paths
    ///    too. `replace("\\.\\", "\\")` is a literal `\.\` → `\` rewrite; `replaceAll("[/\\\\]+$",
    ///    "")` strips trailing separators of either kind — which is what turns `"/tmp/"` into
    ///    `directoryPath = ""` and `name = "tmp"`, and what deletes the directory entirely for a
    ///    file at the root.
    /// 3. **`replaceAll("\\\\.$", "")` is a regex bug.** The pattern is `\\.$`: `\\` is a literal
    ///    backslash and `.` is **any character**, so it strips the last *two* characters of any
    ///    path whose second-to-last character is a backslash. The comment above it says it removes
    ///    `"\."` from the end. A POSIX directory legitimately named `dir\x` becomes `dir`, and the
    ///    file is then reported as living somewhere it does not. **Reproduced verbatim.**
    /// 4. **The extension is re-derived only from the bare name, and only when unknown** (`:174-194`).
    ///    `getFileFormat(Path.of(this.filename))` sees the name without its directory, so a dot in
    ///    a directory cannot influence it; and once the format is known, a **dotless** name gets
    ///    the default extension appended — which is how `setRules`'s `RULES` turns `"board"` into
    ///    `"board.rules"`, and why `DRC_JSON` and the two KiCad formats (whose switch arm is the
    ///    empty `default`) never do.
    ///
    /// `// totalized: BoardFileDetails.setFilename` — `:160` dereferences `path.getParent()` and
    /// `:172` dereferences `path.getFileName()`, both of which are `null` for `Path.of("/")`, so
    /// `setFilename("/")` throws an NPE. The port totalises each null to the empty string, which
    /// leaves every derived value empty; `p8t1probe` prints both answers side by side.
    pub fn set_filename(&mut self, filename: Option<&str>) {
        // `:150-154`.
        let Some(filename) = filename else {
            self.directory_path = String::new();
            self.filename = String::new();
            return;
        };

        let path = java_path::to_absolute_path(filename); // `:156`

        if filename.contains(crate::job::FILE_SEPARATOR) {
            // `:158-166`.
            self.directory_path = java_path::parent_of_normalized(&path).unwrap_or_default();
            // `:161-162` — replace the redundant `\.\` with a simple `\`.
            self.directory_path = self.directory_path.replace("\\.\\", "\\");
            // `:163-164` — remove `/` and `\` from the end.
            self.directory_path = self
                .directory_path
                .trim_end_matches(['/', '\\'])
                .to_string();
            // `:165-166` — "remove the `\.` from the end", except `.` is ANY character.
            self.directory_path = strip_backslash_and_any_char(&self.directory_path);
        } else {
            self.directory_path = String::new(); // `:167-169`
        }

        // `:171-172`.
        self.filename = java_path::file_name_of_normalized(&path).unwrap_or_default();

        // `:174-177`.
        if self.format == FileFormat::Unknown {
            self.format = FileFormat::from_path(Path::new(&self.filename));
        }

        // `:179-194`.
        if self.format != FileFormat::Unknown && !self.filename.contains('.') {
            let extension = self.format.default_extension();
            if !extension.is_empty() {
                self.filename = format!("{}.{extension}", self.filename);
            }
        }

        // `:196` — `fireUpdatedEvent()`; spec §10 has no events.
    }

    /// `getFilenameWithoutExtension` (`:199-205`) — everything before the **last** dot, or the
    /// whole name when there is none. `".hidden"` therefore answers the empty string.
    pub fn get_filename_without_extension(&self) -> String {
        match self.filename.rfind('.') {
            Some(i) => self.filename[..i].to_string(),
            None => self.filename.clone(),
        }
    }
}

/// `String.replaceAll("\\\\.$", "")` — the regex is `\\.$`, i.e. a literal backslash followed by
/// **any one character** at the end of the string. Anchored, so at most one replacement.
///
/// Operates on `char`s rather than bytes so a multi-byte final character is removed whole; Java
/// works in UTF-16 code units, which differs only for a character outside the BMP at the very end
/// of a directory path.
fn strip_backslash_and_any_char(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 && chars[chars.len() - 2] == '\\' {
        return chars[..chars.len() - 2].iter().collect();
    }
    s.to_string()
}

// =================================================================================================
// The roster for this file
// =================================================================================================
//
// not ported: BoardFileDetails.toString — `GsonProvider.GSON.toJson(this)` (`:121-124`). The port
//   has no Gson dump of this class: its readers are `RoutingJob.getInputFileDetails` /
//   `getOutputFileDetails` (rostered in `job.rs`) and `api/v1/**` (ruling AU). The JSON the port
//   does emit is the manifest (Task 4) and the MCP payload (Task 12), both of which build ruling
//   AO's field set explicitly.
// not ported: BoardFileDetails.saveAs — writes `toString()` to a UTF-8 file (`:89-94`); its only
//   caller is `gui/board/BoardFrameFileActions`, which is out of scope (spec §2).
// not ported: BoardFileDetails.addUpdatedEventListener — spec §10; `core/events/**` is rostered by
//   Task 0. Its two registrations are `RoutingJob.java:273` and `:390`, and the second of those is
//   itself a bug (quirk #243).
// not ported: BoardFileDetails.fireUpdatedEvent — spec §10 (`:212-218`).
