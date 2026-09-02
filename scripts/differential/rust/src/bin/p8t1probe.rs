//! Plan 8 Task 1's Rust twin of `scripts/differential/java/probes/P8T1Probe.java`.
//!
//! Prints the same eight tables in the same order with the same normalisation, so
//! `scripts/differential/run.sh p8t1probe` is a byte diff of the whole job model against the HEAD
//! jar. See the Java probe's class javadoc for what each table covers and for how the two
//! totalised rows (`XDIFF`) are printed on both sides.
//!
//! The driver is called `p8t1probe`, not `p8t1`: the plan reserves `p8t1` for Task 6's end-to-end
//! SES-byte gate.

use fr_core::{BoardFileDetails, FileFormat, RoutingJob};
use std::path::{Path, PathBuf};

/// Transcribed from the Java probe, whose watchdog measures it: what the port answers on the rows
/// Java's shift loop spins for ever on.
const JAVA_HANG_MARKER: &str = "java=HANG(>5000ms)";

const SNIFF_INPUTS: &[&str] = &[
    "h:",
    "s:(",
    "s:(pcb",
    "s:(pcb ",
    "s:(pcb x",
    "s:(PCB x",
    "s:(Pcb x",
    "s:(ses x",
    "s:(SES x",
    "s:(Ses x",
    "s:(rules",
    "s:(RULES",
    "s:(RuLes",
    "s:(rUlEs",
    "s:(rulx  ",
    "h:aced000500 00",
    "h:aced0005",
    "h:aced000600 00",
    "h:aced00057b7b",
    "s:{",
    "s:   {",
    "h:09 0d0a 20 7b",
    "s:\n\n\n\n\n\n{",
    "s:\n\n\n\n\n\n   {",
    "s:[1,2]",
    "h:efbbbf7b",
    "h:efbbbf2870636220",
    "h:fffe2800700 0",
    "s:\n(pcb ",
    "s:\r\n(pcb",
    "s:\n\n(pcb  ",
    "s:\n\n\n(pcb ",
    "s:\n\n\n\n(pcb",
    "s:\n\n\n\n\n(pcb",
    "s:\n\n\n\n\n",
    "s:\n\n\n\n\n\n(pcb",
    "h:0d0a0d0a0d0a",
    "h:0d0d0d0d0d0d58",
    "s: (pcb ",
    "h:09 28706362 20",
    "s:(pcb\n\n",
    "s:(sesx ",
    "s:(net (",
    "h:000000000000",
    "s:(Rules",
    "s:(RuLeS",
    // `content == null` (`RoutingJob.java:152-154`), appended so rows 0-45 keep their indices.
    "n:",
];

const EXT_INPUTS: &[&str] = &[
    "board.dsn",
    "board.DSN",
    "board.frb",
    "board.ses",
    "board.rules",
    "board.scr",
    "board.json",
    "board.JSON",
    "board.txt",
    "board.drc",
    "board",
    "board.",
    ".dsn",
    "a.b.dsn",
    "board.ses.",
    "board..dsn",
    "dir.dsn/board",
    "dir.dsn/board.ses",
    "/tmp/x.DsN",
    "board.scr ",
    "",
    "..",
    "x.SCR",
    "x.Rules",
];

const CFE_INPUTS: &[(&str, &str)] = &[
    ("out.ses", "ses"),
    ("out.dsn", "ses"),
    ("out", "ses"),
    ("", "ses"),
    ("/", "ses"),
    ("dir/out.dsn", "ses"),
    ("dir/out.ses", "ses"),
    ("./out.dsn", "ses"),
    ("/tmp/out.dsn", "ses"),
    ("/tmp/out.SES", "ses"),
    ("/tmp/out.DSN", "ses"),
    ("/tmp/out.ses", "SES"),
    ("/tmp/out.dsn", "dsn"),
    ("/tmp/archive.tar.gz", "ses"),
    ("/tmp/out.", "ses"),
    ("/tmp/.hidden", "ses"),
    ("/a.b/out", "ses"),
    ("/tmp/dir.d/", "ses"),
    ("/board.dsn", "ses"),
];

const SETFN_INPUTS: &[(&str, &str)] = &[
    ("-", "<null>"),
    ("-", "board.dsn"),
    ("-", "board"),
    ("-", "/tmp/board.dsn"),
    ("-", "/tmp/board"),
    ("-", "/tmp/board.DSN"),
    ("-", "/tmp/"),
    ("-", "/board.dsn"),
    ("-", "/"),
    ("-", "dir/board.ses"),
    ("-", "/tmp/dir\\x/board.dsn"),
    ("-", "/tmp/a\\.\\b/board.dsn"),
    ("-", "/tmp/dir\\/board.dsn"),
    ("-", "/tmp/b.dsn/x"),
    ("-", ""),
    ("SES", "/tmp/board"),
    ("DSN", "/tmp/board"),
    ("FRB", "/tmp/board"),
    ("RULES", "/tmp/board"),
    ("SCR", "/tmp/board"),
    ("DRC_JSON", "/tmp/board"),
    ("KICAD_SESSION_JSON", "/tmp/board"),
    ("DSN", "/tmp/board.txt"),
    ("SES", "board"),
];

const CRC_LITERALS: &[&str] = &[
    "h:",
    "s:a",
    "s:123456789",
    "s:The quick brown fox jumps over the lazy dog",
    "z:8191",
    "z:8192",
    "z:8193",
];

const CRC_FILES: &[&str] = &[
    "empty_board.dsn",
    "Issue026-J2_reference.dsn",
    "Issue026-J2_reference.ses",
    "Issue029-hw48na.rules",
    "Issue034-Green14SegLED.dsn",
    "Issue107-freq_teiler_200kHz_kicad.dsn",
];

const TSI_INPUTS: &[&str] = &[
    "n:",
    "h:",
    "s:(pcb X)",
    "s:(ses X)",
    "s:(rules X)",
    "h:aced000500 05",
    "s:{\"a\":1}",
    "s:hello!",
];

const TSOF_INPUTS: &[&str] = &[
    "!<null>",
    "out.ses",
    "out.dsn",
    "out.frb",
    "out.scr",
    "out.json",
    "out.rules",
    "out.txt",
    "out",
    "a.dsn",
    "!out.ses",
];

const SIF_FILES: &[(&str, &str)] = &[
    ("a.dsn", "s:(pcb A)\n"),
    ("b.frb", "h:aced000500 05"),
    ("c.json", "s:{\"a\":1}\n"),
    ("d.txt", "s:(pcb D)\n"),
    ("e.dsn", "s:hello!\n"),
    ("f.txt", "s:hello!\n"),
    ("g.ses", "s:(ses G)\n"),
    ("h.missing", "!"),
];

struct Ctx {
    cwd: String,
    cwd_basename: String,
    fixtures_dir: String,
    scratch_dir: String,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir().unwrap();
    let ctx = Ctx {
        cwd: cwd.to_string_lossy().into_owned(),
        cwd_basename: cwd.file_name().unwrap().to_string_lossy().into_owned(),
        fixtures_dir: absolute(&args[0]),
        scratch_dir: absolute(&args[1]),
    };
    std::fs::create_dir_all(&ctx.scratch_dir).unwrap();

    println!("SNIFF-ROWS\t{}", SNIFF_INPUTS.len());
    println!("EXT-ROWS\t{}", EXT_INPUTS.len());
    println!("CFE-ROWS\t{}", CFE_INPUTS.len());
    println!("SETFN-ROWS\t{}", SETFN_INPUTS.len());
    println!("CRC-ROWS\t{}", CRC_LITERALS.len() + CRC_FILES.len());
    println!("TSI-ROWS\t{}", TSI_INPUTS.len());
    println!("TSOF-ROWS\t{}", TSOF_INPUTS.len());
    println!("SIF-ROWS\t{}", SIF_FILES.len());

    sniff_table();
    ext_table();
    cfe_table(&ctx);
    setfn_table(&ctx);
    crc_table(&ctx);
    tsi_table();
    tsof_table(&ctx);
    sif_table(&ctx);
}

fn sniff_table() {
    for (i, spec) in SNIFF_INPUTS.iter().enumerate() {
        // `n:` is Java's `content == null`; the port models it as `Option::None` at
        // `FileFormat::sniff_bytes_opt`, which is `:152-154` as a function.
        let content = if spec.starts_with("n:") {
            None
        } else {
            Some(decode(spec))
        };
        let hangs = content
            .as_deref()
            .is_some_and(FileFormat::java_shift_loop_hangs);
        let format = FileFormat::sniff_bytes_opt(content.as_deref()).java_name();
        let answer = if hangs {
            format!("XDIFF\t{JAVA_HANG_MARKER}\trust={format}")
        } else {
            format.to_string()
        };
        println!(
            "SNIFF\t{i}\t{}\t{answer}",
            content.as_ref().map_or("<null>".to_string(), |c| hex(c))
        );
    }
}

fn ext_table() {
    for (i, raw) in EXT_INPUTS.iter().enumerate() {
        let answer = FileFormat::from_path(Path::new(raw)).java_name();
        println!("EXT\t{i}\t{}\t{answer}", quote(raw));
    }
}

fn cfe_table(ctx: &Ctx) {
    for (i, (path, ext)) in CFE_INPUTS.iter().enumerate() {
        let value = RoutingJob::change_file_extension(path, ext);
        // Java NPEs whenever `Path.of(filename).getParent()` is null (`RoutingJob.java:356`), i.e.
        // for a bare name, the empty path and the root. The port totalises; the row carries both.
        let answer = if java_change_file_extension_npes(path) {
            format!("XDIFF\tjava=NullPointerException\trust={}", norm(ctx, &value))
        } else {
            norm(ctx, &value)
        };
        println!("CFE\t{i}\t{}\t{}\t{answer}", quote(path), quote(ext));
    }
}

/// `Path.of(filename).getParent() == null` — Java's NPE condition at `RoutingJob.java:356`.
fn java_change_file_extension_npes(filename: &str) -> bool {
    let normalized = normalize_java_path(filename);
    match normalized.rfind('/') {
        None => true,
        Some(0) => normalized.len() == 1,
        Some(_) => false,
    }
}

/// `Path.of(s).toString()` — the same normalisation `fr_core::job::java_path::of_to_string` does,
/// repeated here because that module is crate-private.
fn normalize_java_path(s: &str) -> String {
    let absolute = s.starts_with('/');
    let segments: Vec<&str> = s.split('/').filter(|x| !x.is_empty()).collect();
    if segments.is_empty() {
        return if absolute { "/".into() } else { String::new() };
    }
    let joined = segments.join("/");
    if absolute {
        format!("/{joined}")
    } else {
        joined
    }
}

fn setfn_table(ctx: &Ctx) {
    for (i, (preset, raw)) in SETFN_INPUTS.iter().enumerate() {
        let arg = if *raw == "<null>" { None } else { Some(*raw) };
        let mut d = BoardFileDetails::default();
        if *preset != "-" {
            d.format = FileFormat::from_java_name(preset).unwrap();
        }
        // `setFilename("/")` is Java's NPE (neither a parent nor a file name); the port totalises
        // both nulls to "" and every derived value comes out empty.
        let java_npes = arg == Some("/");
        d.set_filename(arg);
        let body = format!(
            "dir={}\tname={}\tformat={}\tabs={}\tstem={}",
            quote(&norm(ctx, d.get_directory_path())),
            quote(&norm(ctx, d.get_filename())),
            d.format.java_name(),
            quote(&norm(ctx, &d.get_absolute_path())),
            quote(&norm(ctx, &d.get_filename_without_extension())),
        );
        let answer = if java_npes {
            format!("XDIFF\tjava=NullPointerException\trust={body}")
        } else {
            body
        };
        println!("SETFN\t{i}\t{preset}\t{}\t{answer}", quote(raw));
    }
}

fn crc_table(ctx: &Ctx) {
    let mut idx = 0;
    for spec in CRC_LITERALS {
        let data = decode(spec);
        println!(
            "CRC\t{idx}\tbytes\t{}\tlen={}\t{}",
            hex(&data),
            data.len(),
            BoardFileDetails::calculate_crc32(&data)
        );
        idx += 1;
    }
    for name in CRC_FILES {
        let data = std::fs::read(PathBuf::from(&ctx.fixtures_dir).join(name)).unwrap();
        println!(
            "CRC\t{idx}\tfile\t{name}\tlen={}\t{}",
            data.len(),
            BoardFileDetails::calculate_crc32(&data)
        );
        idx += 1;
    }
}

fn tsi_table() {
    for (i, spec) in TSI_INPUTS.iter().enumerate() {
        let content = if spec.starts_with("n:") {
            None
        } else {
            Some(decode(spec))
        };
        let mut job = RoutingJob::default();
        let ok = job.set_input_bytes(content.as_deref());
        let input = job.input.as_ref().unwrap();
        println!(
            "TSI\t{i}\t{}\tok={ok}\tformat={}\tsize={}\tcrc32={}",
            content.as_ref().map_or("<null>".to_string(), |c| hex(c)),
            input.format.java_name(),
            input.size,
            input.crc32
        );
    }
}

fn tsof_table(ctx: &Ctx) {
    write_scratch_files(ctx);
    for (i, spec) in TSOF_INPUTS.iter().enumerate() {
        let file: Option<PathBuf> = if *spec == "!<null>" {
            None
        } else if let Some(bare) = spec.strip_prefix('!') {
            Some(PathBuf::from(bare))
        } else {
            Some(PathBuf::from(&ctx.scratch_dir).join(spec))
        };
        let mut job = RoutingJob::default();
        let ok = job.try_to_set_output_file(file.as_deref());
        let out = match job.output.as_ref() {
            None => "output=<null>".to_string(),
            Some(o) => format!(
                "format={}\tdir={}\tname={}\tsize={}\tcrc32={}",
                o.format.java_name(),
                quote(&norm(ctx, o.get_directory_path())),
                quote(&norm(ctx, o.get_filename())),
                o.size,
                o.crc32
            ),
        };
        println!("TSOF\t{i}\t{}\tok={ok}\t{out}", quote(spec));
    }
}

fn sif_table(ctx: &Ctx) {
    write_scratch_files(ctx);
    for (i, (name, _)) in SIF_FILES.iter().enumerate() {
        let path = PathBuf::from(&ctx.scratch_dir).join(name);
        let mut job = RoutingJob::default();
        let id_derived_name = format!("J-{}", job.id.short_upper6());
        let answer = match job.set_input(&path) {
            Err(e) => format!(
                "throws={}\tinput={}",
                java_exception_name(&e),
                if job.input.is_none() {
                    "<null>"
                } else {
                    "<set>"
                }
            ),
            Ok(()) => {
                let input = job.input.as_ref().unwrap();
                let out = match job.output.as_ref() {
                    None => "out=<null>".to_string(),
                    Some(o) => format!(
                        "out.format={}\tout.dir={}\tout.name={}",
                        o.format.java_name(),
                        quote(&norm(ctx, o.get_directory_path())),
                        quote(&norm(ctx, o.get_filename()))
                    ),
                };
                format!(
                    "in.format={}\tin.dir={}\tin.name={}\tin.size={}\tin.crc32={}\t{out}\tjob.name={}",
                    input.format.java_name(),
                    quote(&norm(ctx, input.get_directory_path())),
                    quote(&norm(ctx, input.get_filename())),
                    input.size,
                    input.crc32,
                    quote(&if job.name == id_derived_name {
                        "<ID-DERIVED>".to_string()
                    } else {
                        norm(ctx, &job.name)
                    })
                )
            }
        };
        println!("SIF\t{i}\t{name}\t{answer}");
    }
}

/// The `getClass().getSimpleName()` Java would print for the same failure. `set_input`'s only
/// error arm is the `new FileInputStream(inputFile)` at `RoutingJob.java:428`.
fn java_exception_name(e: &fr_core::Error) -> &'static str {
    match e {
        fr_core::Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            "FileNotFoundException"
        }
        _ => "IOException",
    }
}

fn write_scratch_files(ctx: &Ctx) {
    for (name, spec) in SIF_FILES {
        let path = PathBuf::from(&ctx.scratch_dir).join(name);
        if *spec == "!" {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        std::fs::write(&path, decode(spec)).unwrap();
    }
}

// ── helpers, mirroring the Java probe's ────────────────────────────────────────────────────────

fn absolute(p: &str) -> String {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        normalize_java_path(&path.to_string_lossy())
    } else {
        normalize_java_path(
            &std::env::current_dir()
                .unwrap()
                .join(path)
                .to_string_lossy(),
        )
    }
}

fn decode(spec: &str) -> Vec<u8> {
    let body = &spec[2..];
    match spec.as_bytes()[0] {
        b's' => {
            let mut out = Vec::new();
            let chars: Vec<char> = body.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                    out.push(match chars[i] {
                        'n' => b'\n',
                        'r' => b'\r',
                        't' => b'\t',
                        c => c as u8,
                    });
                } else {
                    out.push(chars[i] as u8);
                }
                i += 1;
            }
            out
        }
        b'h' => {
            let clean: String = body.chars().filter(|c| *c != ' ').collect();
            (0..clean.len() / 2)
                .map(|i| u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16).unwrap())
                .collect()
        }
        b'z' => vec![0u8; body.parse().unwrap()],
        _ => panic!("bad spec {spec}"),
    }
}

fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".to_string();
    }
    let n = b.len().min(64);
    let mut s: String = b[..n].iter().map(|x| format!("{x:02x}")).collect();
    if b.len() > n {
        s.push_str(&format!("+{}", b.len() - n));
    }
    s
}

fn norm(ctx: &Ctx, s: &str) -> String {
    let s = s.replace(&ctx.scratch_dir, "<SCRATCH>");
    let s = s.replace(&ctx.fixtures_dir, "<FIXTURES>");
    let s = s.replace(&ctx.cwd, "<CWD>");
    if s == ctx.cwd_basename {
        return "<CWD-BASENAME>".to_string();
    }
    s
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
            out.push(c);
        } else if (c as u32) < 0x20 || (c as u32) > 0x7e {
            out.push_str(&format!("\\u{:04x}", c as u32));
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}
