use fr_core::{
    BoardFileDetails, Error, FileFormat, RoutingJob, RoutingJobState, SessionId,
    validate_session_host,
};
use std::path::{Path, PathBuf};

const TRANSCRIPT: &[&str] = &[
    "SNIFF-ROWS\t47",
    "EXT-ROWS\t24",
    "CFE-ROWS\t19",
    "SETFN-ROWS\t24",
    "CRC-ROWS\t13",
    "TSI-ROWS\t8",
    "TSOF-ROWS\t11",
    "SIF-ROWS\t8",
    "SNIFF\t0\t-\tUNKNOWN",
    "SNIFF\t1\t28\tUNKNOWN",
    "SNIFF\t2\t28706362\tUNKNOWN",
    "SNIFF\t3\t2870636220\tUNKNOWN",
    "SNIFF\t4\t287063622078\tDSN",
    "SNIFF\t5\t285043422078\tDSN",
    "SNIFF\t6\t285063622078\tUNKNOWN",
    "SNIFF\t7\t287365732078\tSES",
    "SNIFF\t8\t285345532078\tSES",
    "SNIFF\t9\t285365732078\tUNKNOWN",
    "SNIFF\t10\t2872756c6573\tRULES",
    "SNIFF\t11\t2852554c4553\tRULES",
    "SNIFF\t12\t2852754c6573\tRULES",
    "SNIFF\t13\t2872556c4573\tRULES",
    "SNIFF\t14\t2872756c782020\tRULES",
    "SNIFF\t15\taced00050000\tFRB",
    "SNIFF\t16\taced0005\tUNKNOWN",
    "SNIFF\t17\taced00060000\tUNKNOWN",
    "SNIFF\t18\taced00057b7b\tFRB",
    "SNIFF\t19\t7b\tKICAD_DESIGN_JSON",
    "SNIFF\t20\t2020207b\tKICAD_DESIGN_JSON",
    "SNIFF\t21\t090d0a207b\tKICAD_DESIGN_JSON",
    "SNIFF\t22\t0a0a0a0a0a0a7b\tKICAD_DESIGN_JSON",
    "SNIFF\t23\t0a0a0a0a0a0a2020207b\tKICAD_DESIGN_JSON",
    "SNIFF\t24\t5b312c325d\tUNKNOWN",
    "SNIFF\t25\tefbbbf7b\tUNKNOWN",
    "SNIFF\t26\tefbbbf2870636220\tUNKNOWN",
    "SNIFF\t27\tfffe28007000\tUNKNOWN",
    "SNIFF\t28\t0a2870636220\tDSN",
    "SNIFF\t29\t0d0a28706362\tDSN",
    "SNIFF\t30\t0a0a287063622020\tDSN",
    "SNIFF\t31\t0a0a0a2870636220\tUNKNOWN",
    "SNIFF\t32\t0a0a0a0a28706362\tUNKNOWN",
    "SNIFF\t33\t0a0a0a0a0a28706362\tUNKNOWN",
    "SNIFF\t34\t0a0a0a0a0a\tUNKNOWN",
    "SNIFF\t35\t0a0a0a0a0a0a28706362\tXDIFF\tjava=HANG(>5000ms)\trust=UNKNOWN",
    "SNIFF\t36\t0d0a0d0a0d0a\tXDIFF\tjava=HANG(>5000ms)\trust=UNKNOWN",
    "SNIFF\t37\t0d0d0d0d0d0d58\tXDIFF\tjava=HANG(>5000ms)\trust=UNKNOWN",
    "SNIFF\t38\t202870636220\tUNKNOWN",
    "SNIFF\t39\t092870636220\tUNKNOWN",
    "SNIFF\t40\t287063620a0a\tDSN",
    "SNIFF\t41\t287365737820\tSES",
    "SNIFF\t42\t286e65742028\tUNKNOWN",
    "SNIFF\t43\t000000000000\tUNKNOWN",
    "SNIFF\t44\t2852756c6573\tRULES",
    "SNIFF\t45\t2852754c6553\tRULES",
    "SNIFF\t46\t<null>\tUNKNOWN",
    "EXT\t0\t\"board.dsn\"\tDSN",
    "EXT\t1\t\"board.DSN\"\tDSN",
    "EXT\t2\t\"board.frb\"\tFRB",
    "EXT\t3\t\"board.ses\"\tSES",
    "EXT\t4\t\"board.rules\"\tRULES",
    "EXT\t5\t\"board.scr\"\tSCR",
    "EXT\t6\t\"board.json\"\tKICAD_DESIGN_JSON",
    "EXT\t7\t\"board.JSON\"\tKICAD_DESIGN_JSON",
    "EXT\t8\t\"board.txt\"\tUNKNOWN",
    "EXT\t9\t\"board.drc\"\tUNKNOWN",
    "EXT\t10\t\"board\"\tUNKNOWN",
    "EXT\t11\t\"board.\"\tUNKNOWN",
    "EXT\t12\t\".dsn\"\tDSN",
    "EXT\t13\t\"a.b.dsn\"\tDSN",
    "EXT\t14\t\"board.ses.\"\tSES",
    "EXT\t15\t\"board..dsn\"\tDSN",
    "EXT\t16\t\"dir.dsn/board\"\tUNKNOWN",
    "EXT\t17\t\"dir.dsn/board.ses\"\tSES",
    "EXT\t18\t\"/tmp/x.DsN\"\tDSN",
    "EXT\t19\t\"board.scr \"\tUNKNOWN",
    "EXT\t20\t\"\"\tUNKNOWN",
    "EXT\t21\t\"..\"\tUNKNOWN",
    "EXT\t22\t\"x.SCR\"\tSCR",
    "EXT\t23\t\"x.Rules\"\tRULES",
    "CFE\t0\t\"out.ses\"\t\"ses\"\tXDIFF\tjava=NullPointerException\trust=out.ses",
    "CFE\t1\t\"out.dsn\"\t\"ses\"\tXDIFF\tjava=NullPointerException\trust=out.ses",
    "CFE\t2\t\"out\"\t\"ses\"\tXDIFF\tjava=NullPointerException\trust=out.ses",
    "CFE\t3\t\"\"\t\"ses\"\tXDIFF\tjava=NullPointerException\trust=.ses",
    "CFE\t4\t\"/\"\t\"ses\"\tXDIFF\tjava=NullPointerException\trust=.ses",
    "CFE\t5\t\"dir/out.dsn\"\t\"ses\"\t<CWD>/dir/out.ses",
    "CFE\t6\t\"dir/out.ses\"\t\"ses\"\tdir/out.ses",
    "CFE\t7\t\"./out.dsn\"\t\"ses\"\t<CWD>/./out.ses",
    "CFE\t8\t\"/tmp/out.dsn\"\t\"ses\"\t/tmp/out.ses",
    "CFE\t9\t\"/tmp/out.SES\"\t\"ses\"\t/tmp/out.SES",
    "CFE\t10\t\"/tmp/out.DSN\"\t\"ses\"\t/tmp/out.ses",
    "CFE\t11\t\"/tmp/out.ses\"\t\"SES\"\t/tmp/out.SES",
    "CFE\t12\t\"/tmp/out.dsn\"\t\"dsn\"\t/tmp/out.dsn",
    "CFE\t13\t\"/tmp/archive.tar.gz\"\t\"ses\"\t/tmp/archive.tar.ses",
    "CFE\t14\t\"/tmp/out.\"\t\"ses\"\t/tmp/out..ses",
    "CFE\t15\t\"/tmp/.hidden\"\t\"ses\"\t/tmp/.ses",
    "CFE\t16\t\"/a.b/out\"\t\"ses\"\t/a.b/out.ses",
    "CFE\t17\t\"/tmp/dir.d/\"\t\"ses\"\t/tmp/dir.ses",
    "CFE\t18\t\"/board.dsn\"\t\"ses\"\t/board.ses",
    "SETFN\t0\t-\t\"<null>\"\tdir=\"\"\tname=\"\"\tformat=UNKNOWN\tabs=\"\"\tstem=\"\"",
    "SETFN\t1\t-\t\"board.dsn\"\tdir=\"\"\tname=\"board.dsn\"\tformat=DSN\tabs=\"board.dsn\"\tstem=\"board\"",
    "SETFN\t2\t-\t\"board\"\tdir=\"\"\tname=\"board\"\tformat=UNKNOWN\tabs=\"board\"\tstem=\"board\"",
    "SETFN\t3\t-\t\"/tmp/board.dsn\"\tdir=\"/tmp\"\tname=\"board.dsn\"\tformat=DSN\tabs=\"/tmp/board.dsn\"\tstem=\"board\"",
    "SETFN\t4\t-\t\"/tmp/board\"\tdir=\"/tmp\"\tname=\"board\"\tformat=UNKNOWN\tabs=\"/tmp/board\"\tstem=\"board\"",
    "SETFN\t5\t-\t\"/tmp/board.DSN\"\tdir=\"/tmp\"\tname=\"board.DSN\"\tformat=DSN\tabs=\"/tmp/board.DSN\"\tstem=\"board\"",
    "SETFN\t6\t-\t\"/tmp/\"\tdir=\"\"\tname=\"tmp\"\tformat=UNKNOWN\tabs=\"tmp\"\tstem=\"tmp\"",
    "SETFN\t7\t-\t\"/board.dsn\"\tdir=\"\"\tname=\"board.dsn\"\tformat=DSN\tabs=\"board.dsn\"\tstem=\"board\"",
    "SETFN\t8\t-\t\"/\"\tXDIFF\tjava=NullPointerException\trust=dir=\"\"\tname=\"\"\tformat=UNKNOWN\tabs=\"\"\tstem=\"\"",
    "SETFN\t9\t-\t\"dir/board.ses\"\tdir=\"<CWD>/dir\"\tname=\"board.ses\"\tformat=SES\tabs=\"<CWD>/dir/board.ses\"\tstem=\"board\"",
    "SETFN\t10\t-\t\"/tmp/dir\\\\x/board.dsn\"\tdir=\"/tmp/dir\"\tname=\"board.dsn\"\tformat=DSN\tabs=\"/tmp/dir/board.dsn\"\tstem=\"board\"",
    "SETFN\t11\t-\t\"/tmp/a\\\\.\\\\b/board.dsn\"\tdir=\"/tmp/a\"\tname=\"board.dsn\"\tformat=DSN\tabs=\"/tmp/a/board.dsn\"\tstem=\"board\"",
    "SETFN\t12\t-\t\"/tmp/dir\\\\/board.dsn\"\tdir=\"/tmp/dir\"\tname=\"board.dsn\"\tformat=DSN\tabs=\"/tmp/dir/board.dsn\"\tstem=\"board\"",
    "SETFN\t13\t-\t\"/tmp/b.dsn/x\"\tdir=\"/tmp/b.dsn\"\tname=\"x\"\tformat=UNKNOWN\tabs=\"/tmp/b.dsn/x\"\tstem=\"x\"",
    "SETFN\t14\t-\t\"\"\tdir=\"\"\tname=\"<CWD-BASENAME>\"\tformat=UNKNOWN\tabs=\"<CWD-BASENAME>\"\tstem=\"<CWD-BASENAME>\"",
    "SETFN\t15\tSES\t\"/tmp/board\"\tdir=\"/tmp\"\tname=\"board.ses\"\tformat=SES\tabs=\"/tmp/board.ses\"\tstem=\"board\"",
    "SETFN\t16\tDSN\t\"/tmp/board\"\tdir=\"/tmp\"\tname=\"board.dsn\"\tformat=DSN\tabs=\"/tmp/board.dsn\"\tstem=\"board\"",
    "SETFN\t17\tFRB\t\"/tmp/board\"\tdir=\"/tmp\"\tname=\"board.frb\"\tformat=FRB\tabs=\"/tmp/board.frb\"\tstem=\"board\"",
    "SETFN\t18\tRULES\t\"/tmp/board\"\tdir=\"/tmp\"\tname=\"board.rules\"\tformat=RULES\tabs=\"/tmp/board.rules\"\tstem=\"board\"",
    "SETFN\t19\tSCR\t\"/tmp/board\"\tdir=\"/tmp\"\tname=\"board.scr\"\tformat=SCR\tabs=\"/tmp/board.scr\"\tstem=\"board\"",
    "SETFN\t20\tDRC_JSON\t\"/tmp/board\"\tdir=\"/tmp\"\tname=\"board\"\tformat=DRC_JSON\tabs=\"/tmp/board\"\tstem=\"board\"",
    "SETFN\t21\tKICAD_SESSION_JSON\t\"/tmp/board\"\tdir=\"/tmp\"\tname=\"board\"\tformat=KICAD_SESSION_JSON\tabs=\"/tmp/board\"\tstem=\"board\"",
    "SETFN\t22\tDSN\t\"/tmp/board.txt\"\tdir=\"/tmp\"\tname=\"board.txt\"\tformat=DSN\tabs=\"/tmp/board.txt\"\tstem=\"board\"",
    "SETFN\t23\tSES\t\"board\"\tdir=\"\"\tname=\"board.ses\"\tformat=SES\tabs=\"board.ses\"\tstem=\"board\"",
    "CRC\t0\tbytes\t-\tlen=0\t0",
    "CRC\t1\tbytes\t61\tlen=1\t3904355907",
    "CRC\t2\tbytes\t313233343536373839\tlen=9\t3421780262",
    "CRC\t3\tbytes\t54686520717569636b2062726f776e20666f78206a756d7073206f76657220746865206c617a7920646f67\tlen=43\t1095738169",
    "CRC\t4\tbytes\t00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000+8127\tlen=8191\t4141366926",
    "CRC\t5\tbytes\t00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000+8128\tlen=8192\t3639908756",
    "CRC\t6\tbytes\t00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000+8129\tlen=8193\t632835145",
    "CRC\t7\tfile\tempty_board.dsn\tlen=442\t3474467974",
    "CRC\t8\tfile\tIssue026-J2_reference.dsn\tlen=7925\t2979694418",
    "CRC\t9\tfile\tIssue026-J2_reference.ses\tlen=14421\t3753402078",
    "CRC\t10\tfile\tIssue029-hw48na.rules\tlen=22807\t3477915108",
    "CRC\t11\tfile\tIssue034-Green14SegLED.dsn\tlen=33706\t3720379131",
    "CRC\t12\tfile\tIssue107-freq_teiler_200kHz_kicad.dsn\tlen=69307\t221053834",
    "TSI\t0\t<null>\tok=false\tformat=UNKNOWN\tsize=0\tcrc32=0",
    "TSI\t1\t-\tok=false\tformat=UNKNOWN\tsize=0\tcrc32=0",
    "TSI\t2\t28706362205829\tok=true\tformat=DSN\tsize=7\tcrc32=758947457",
    "TSI\t3\t28736573205829\tok=true\tformat=SES\tsize=7\tcrc32=3427608949",
    "TSI\t4\t2872756c6573205829\tok=true\tformat=RULES\tsize=9\tcrc32=3505810634",
    "TSI\t5\taced00050005\tok=true\tformat=FRB\tsize=6\tcrc32=2046379324",
    "TSI\t6\t7b2261223a317d\tok=true\tformat=KICAD_DESIGN_JSON\tsize=7\tcrc32=1444654255",
    "TSI\t7\t68656c6c6f21\tok=false\tformat=UNKNOWN\tsize=0\tcrc32=0",
    "TSOF\t0\t\"!<null>\"\tok=false\toutput=<null>",
    "TSOF\t1\t\"out.ses\"\tok=true\tformat=SES\tdir=\"<SCRATCH>\"\tname=\"out.ses\"\tsize=0\tcrc32=0",
    "TSOF\t2\t\"out.dsn\"\tok=true\tformat=DSN\tdir=\"<SCRATCH>\"\tname=\"out.dsn\"\tsize=0\tcrc32=0",
    "TSOF\t3\t\"out.frb\"\tok=true\tformat=FRB\tdir=\"<SCRATCH>\"\tname=\"out.frb\"\tsize=0\tcrc32=0",
    "TSOF\t4\t\"out.scr\"\tok=true\tformat=SCR\tdir=\"<SCRATCH>\"\tname=\"out.scr\"\tsize=0\tcrc32=0",
    "TSOF\t5\t\"out.json\"\tok=true\tformat=KICAD_SESSION_JSON\tdir=\"<SCRATCH>\"\tname=\"out.json\"\tsize=0\tcrc32=0",
    "TSOF\t6\t\"out.rules\"\tok=false\toutput=<null>",
    "TSOF\t7\t\"out.txt\"\tok=false\toutput=<null>",
    "TSOF\t8\t\"out\"\tok=false\toutput=<null>",
    "TSOF\t9\t\"a.dsn\"\tok=true\tformat=DSN\tdir=\"<SCRATCH>\"\tname=\"a.dsn\"\tsize=8\tcrc32=3149009220",
    "TSOF\t10\t\"!out.ses\"\tok=true\tformat=SES\tdir=\"<CWD>\"\tname=\"out.ses\"\tsize=0\tcrc32=0",
    "SIF\t0\ta.dsn\tin.format=DSN\tin.dir=\"<SCRATCH>\"\tin.name=\"a.dsn\"\tin.size=8\tin.crc32=3149009220\tout.format=SES\tout.dir=\"<SCRATCH>\"\tout.name=\"a.ses\"\tjob.name=\"a\"",
    "SIF\t1\tb.frb\tin.format=FRB\tin.dir=\"<SCRATCH>\"\tin.name=\"b.frb\"\tin.size=6\tin.crc32=2046379324\tout.format=FRB\tout.dir=\"<SCRATCH>\"\tout.name=\"b.frb\"\tjob.name=\"b\"",
    "SIF\t2\tc.json\tin.format=KICAD_DESIGN_JSON\tin.dir=\"<SCRATCH>\"\tin.name=\"c.json\"\tin.size=8\tin.crc32=1961403206\tout.format=KICAD_DESIGN_JSON\tout.dir=\"<SCRATCH>\"\tout.name=\"c.json\"\tjob.name=\"c\"",
    "SIF\t3\td.txt\tin.format=DSN\tin.dir=\"<SCRATCH>\"\tin.name=\"d.txt\"\tin.size=8\tin.crc32=3178877871\tout.format=SES\tout.dir=\"<SCRATCH>\"\tout.name=\"d.ses\"\tjob.name=\"d\"",
    "SIF\t4\te.dsn\tin.format=DSN\tin.dir=\"<SCRATCH>\"\tin.name=\"e.dsn\"\tin.size=0\tin.crc32=0\tout.format=SES\tout.dir=\"<SCRATCH>\"\tout.name=\"e.ses\"\tjob.name=\"e\"",
    "SIF\t5\tf.txt\tin.format=UNKNOWN\tin.dir=\"<SCRATCH>\"\tin.name=\"f.txt\"\tin.size=0\tin.crc32=0\tout=<null>\tjob.name=\"<ID-DERIVED>\"",
    "SIF\t6\tg.ses\tin.format=SES\tin.dir=\"<SCRATCH>\"\tin.name=\"g.ses\"\tin.size=8\tin.crc32=84814404\tout=<null>\tjob.name=\"g\"",
    "SIF\t7\th.missing\tthrows=FileNotFoundException\tinput=<null>",
];

const CRC_LITERAL_SPECS: &[&str] = &[
    "h:",
    "s:a",
    "s:123456789",
    "s:The quick brown fox jumps over the lazy dog",
    "z:8191",
    "z:8192",
    "z:8193",
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


#[test]
fn the_committed_transcript_still_says_what_this_table_says() {
    let file = include_str!("data/p8t1-job-model.txt");
    let file_lines: Vec<&str> = file.lines().collect();
    assert_eq!(
        file_lines.len(),
        TRANSCRIPT.len(),
        "tests/data/p8t1-job-model.txt has {} lines, the table has {}",
        file_lines.len(),
        TRANSCRIPT.len()
    );
    for (i, (a, b)) in file_lines.iter().zip(TRANSCRIPT.iter()).enumerate() {
        assert_eq!(a, b, "line {} of the committed transcript", i + 1);
    }
}

#[test]
fn the_port_reproduces_every_transcript_row() {
    let ctx = Ctx::new();
    ctx.write_scratch_files();
    let mut mismatches = Vec::new();
    for (i, line) in TRANSCRIPT.iter().enumerate() {
        if let Some(rebuilt) = ctx.rebuild(line)
            && rebuilt != *line
        {
            mismatches.push(format!("row {}\n  java: {line}\n  rust: {rebuilt}", i + 1));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} row(s) disagree with the jar:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn every_table_is_as_long_as_its_header_says() {
    let counts: Vec<(&str, usize)> = TRANSCRIPT
        .iter()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(tag, _)| tag.ends_with("-ROWS"))
        .map(|(tag, n)| (tag.trim_end_matches("-ROWS"), n.parse().unwrap()))
        .collect();
    assert_eq!(counts.len(), 8, "eight tables");
    for (tag, want) in counts {
        let got = TRANSCRIPT
            .iter()
            .filter(|l| l.starts_with(&format!("{tag}\t")))
            .count();
        assert_eq!(got, want, "table {tag}");
    }
}


#[test]
fn six_leading_crlf_bytes_hang_java_and_the_port_bounds_the_loop() {
    for content in [
        b"\n\n\n\n\n\n(pcb".to_vec(),
        b"\r\n\r\n\r\n".to_vec(),
        b"\r\r\r\r\r\rX".to_vec(),
    ] {
        assert!(
            FileFormat::java_shift_loop_hangs(&content),
            "java should hang on {content:?}"
        );
        assert_eq!(FileFormat::sniff_bytes(&content), FileFormat::Unknown);
    }
}

#[test]
fn a_null_content_is_unknown_before_anything_else_runs() {
    assert_eq!(FileFormat::sniff_bytes_opt(None), FileFormat::Unknown);
    assert_eq!(
        FileFormat::sniff_bytes_opt(Some(b"(pcb x")),
        FileFormat::Dsn
    );
    assert_eq!(FileFormat::sniff_bytes_opt(Some(b"")), FileFormat::Unknown);
}

#[test]
fn the_json_precheck_saves_six_newlines_followed_by_a_brace() {
    let content = b"\n\n\n\n\n\n   {";
    assert!(!FileFormat::java_shift_loop_hangs(content));
    assert_eq!(
        FileFormat::sniff_bytes(content),
        FileFormat::KicadDesignJson
    );
}

#[test]
fn a_leading_newline_run_floods_the_buffer_and_loses_the_content() {
    assert_eq!(FileFormat::sniff_bytes(b"\n\n(pcb  "), FileFormat::Dsn);
    assert_eq!(FileFormat::sniff_bytes(b"\n\n\n(pcb "), FileFormat::Unknown);
    assert_eq!(
        FileFormat::sniff_bytes(b"\n\n\n\n(pcb"),
        FileFormat::Unknown
    );
    assert_eq!(
        FileFormat::sniff_bytes(b"\n\n\n\n\n(pcb"),
        FileFormat::Unknown
    );
}

#[test]
fn a_leading_space_or_tab_is_not_stripped_by_the_shift_loop() {
    assert_eq!(FileFormat::sniff_bytes(b" (pcb "), FileFormat::Unknown);
    assert_eq!(FileFormat::sniff_bytes(b"\t(pcb "), FileFormat::Unknown);
    assert_eq!(FileFormat::sniff_bytes(b"\n(pcb "), FileFormat::Dsn);
}

#[test]
fn a_utf8_bom_defeats_both_the_json_precheck_and_the_sniff() {
    assert_eq!(
        FileFormat::sniff_bytes(b"\xef\xbb\xbf{"),
        FileFormat::Unknown
    );
    assert_eq!(
        FileFormat::sniff_bytes(b"\xef\xbb\xbf(pcb "),
        FileFormat::Unknown
    );
}

#[test]
fn five_bytes_are_never_enough_but_one_brace_is() {
    assert_eq!(FileFormat::sniff_bytes(b"(pcb "), FileFormat::Unknown);
    assert_eq!(FileFormat::sniff_bytes(b"(pcb x"), FileFormat::Dsn);
    assert_eq!(FileFormat::sniff_bytes(b"{"), FileFormat::KicadDesignJson);
}

#[test]
fn the_rules_branch_is_case_folded_per_character_and_only_four_bytes_long() {
    for input in [
        &b"(rules"[..],
        &b"(RULES"[..],
        &b"(RuLes"[..],
        &b"(rUlEs"[..],
        &b"(rulx  "[..],
    ] {
        assert_eq!(
            FileFormat::sniff_bytes(input),
            FileFormat::Rules,
            "{input:?}"
        );
    }
    assert_eq!(FileFormat::sniff_bytes(b"(Pcb x"), FileFormat::Unknown);
    assert_eq!(FileFormat::sniff_bytes(b"(Ses x"), FileFormat::Unknown);
}

#[test]
fn the_frb_magic_is_tested_before_the_shift_loop_and_before_the_brace() {
    assert_eq!(
        FileFormat::sniff_bytes(&[0xAC, 0xED, 0x00, 0x05, 0x7B, 0x7B]),
        FileFormat::Frb
    );
    assert_eq!(
        FileFormat::sniff_bytes(&[0xAC, 0xED, 0x00, 0x06, 0x00, 0x00]),
        FileFormat::Unknown
    );
}

#[test]
fn board_dot_is_unknown_but_board_ses_dot_is_ses() {
    assert_eq!(
        FileFormat::from_path(Path::new("board.")),
        FileFormat::Unknown
    );
    assert_eq!(
        FileFormat::from_path(Path::new("board.ses.")),
        FileFormat::Ses
    );
    assert_eq!(FileFormat::from_path(Path::new(".dsn")), FileFormat::Dsn);
    assert_eq!(FileFormat::from_path(Path::new("")), FileFormat::Unknown);
    assert_eq!(FileFormat::from_path(Path::new("..")), FileFormat::Unknown);
}

#[test]
fn a_dot_in_a_directory_takes_part_in_the_extension_switch() {
    assert_eq!(
        FileFormat::from_path(Path::new("dir.dsn/board")),
        FileFormat::Unknown
    );
    assert_eq!(
        FileFormat::from_path(Path::new("dir.dsn/board.ses")),
        FileFormat::Ses
    );
}

#[test]
fn change_file_extension_npes_on_a_bare_filename_and_the_port_answers_the_bare_name() {
    assert_eq!(
        RoutingJob::change_file_extension("out.ses", "ses"),
        "out.ses"
    );
    assert_eq!(
        RoutingJob::change_file_extension("out.dsn", "ses"),
        "out.ses"
    );
    assert_eq!(RoutingJob::change_file_extension("out", "ses"), "out.ses");
    assert_eq!(RoutingJob::change_file_extension("", "ses"), ".ses");
    assert_eq!(RoutingJob::change_file_extension("/", "ses"), ".ses");
}

#[test]
fn change_file_extension_returns_its_relative_argument_when_the_extension_already_matches() {
    assert_eq!(
        RoutingJob::change_file_extension("dir/out.ses", "ses"),
        "dir/out.ses"
    );
    let derived = RoutingJob::change_file_extension("dir/out.dsn", "ses");
    assert!(derived.starts_with('/'), "{derived} should be absolute");
    assert!(derived.ends_with("/dir/out.ses"), "{derived}");
}

#[test]
fn change_file_extension_cuts_the_original_extensions_length_and_never_folds_the_new_one() {
    assert_eq!(
        RoutingJob::change_file_extension("/tmp/out.DSN", "ses"),
        "/tmp/out.ses"
    );
    assert_eq!(
        RoutingJob::change_file_extension("/tmp/out.SES", "ses"),
        "/tmp/out.SES"
    );
    assert_eq!(
        RoutingJob::change_file_extension("/tmp/out.ses", "SES"),
        "/tmp/out.SES"
    );
    assert_eq!(
        RoutingJob::change_file_extension("/tmp/out.", "ses"),
        "/tmp/out..ses"
    );
    assert_eq!(
        RoutingJob::change_file_extension("/tmp/.hidden", "ses"),
        "/tmp/.ses"
    );
    assert_eq!(
        RoutingJob::change_file_extension("/tmp/archive.tar.gz", "ses"),
        "/tmp/archive.tar.ses"
    );
}

#[test]
fn set_filename_strips_a_backslash_and_whatever_follows_it() {
    let mut d = BoardFileDetails::default();
    d.set_filename(Some("/tmp/dir\\x/board.dsn"));
    assert_eq!(d.get_directory_path(), "/tmp/dir");
    assert_eq!(d.get_filename(), "board.dsn");
    assert_eq!(d.get_absolute_path(), "/tmp/dir/board.dsn");

    let mut d = BoardFileDetails::default();
    d.set_filename(Some("/tmp/a\\.\\b/board.dsn"));
    assert_eq!(d.get_directory_path(), "/tmp/a");

    let mut d = BoardFileDetails::default();
    d.set_filename(Some("/tmp/dir\\/board.dsn"));
    assert_eq!(d.get_directory_path(), "/tmp/dir");
}

#[test]
fn set_filename_at_the_root_loses_the_directory_entirely() {
    let mut d = BoardFileDetails::default();
    d.set_filename(Some("/board.dsn"));
    assert_eq!(d.get_directory_path(), "");
    assert_eq!(d.get_absolute_path(), "board.dsn");
    assert_eq!(d.format, FileFormat::Dsn);

    let mut d = BoardFileDetails::default();
    d.set_filename(Some("/tmp/"));
    assert_eq!(d.get_directory_path(), "");
    assert_eq!(d.get_filename(), "tmp");
}

#[test]
fn set_filename_drops_the_directory_of_a_name_without_a_separator() {
    let mut d = BoardFileDetails::default();
    d.set_filename(Some("board.dsn"));
    assert_eq!(d.get_directory_path(), "");
    assert_eq!(d.get_filename(), "board.dsn");
    assert_eq!(d.get_absolute_path(), "board.dsn");
}

#[test]
fn set_filename_appends_the_default_extension_only_for_the_five_named_formats() {
    for (format, want) in [
        (FileFormat::Ses, "board.ses"),
        (FileFormat::Dsn, "board.dsn"),
        (FileFormat::Frb, "board.frb"),
        (FileFormat::Rules, "board.rules"),
        (FileFormat::Scr, "board.scr"),
        (FileFormat::DrcJson, "board"),
        (FileFormat::KicadDesignJson, "board"),
        (FileFormat::KicadSessionJson, "board"),
    ] {
        let mut d = BoardFileDetails::default();
        d.format = format;
        d.set_filename(Some("/tmp/board"));
        assert_eq!(d.get_filename(), want, "{format:?}");
        assert_eq!(d.format, format);
    }
}

#[test]
fn get_filename_without_extension_cuts_at_the_last_dot() {
    let mut d = BoardFileDetails::default();
    d.set_filename(Some("/tmp/archive.tar.gz"));
    assert_eq!(d.get_filename_without_extension(), "archive.tar");
    let mut d = BoardFileDetails::default();
    d.set_filename(Some("/tmp/board"));
    assert_eq!(d.get_filename_without_extension(), "board");
}

#[test]
fn calculate_crc32_matches_the_standard_check_vector() {
    assert_eq!(BoardFileDetails::calculate_crc32(b""), 0);
    assert_eq!(BoardFileDetails::calculate_crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(BoardFileDetails::calculate_crc32(b"a"), 0xE8B7_BE43);
}

#[test]
fn set_data_keeps_the_format_it_was_given() {
    let mut d = BoardFileDetails::default();
    d.set_data(b"{\"a\":1}".to_vec(), FileFormat::KicadSessionJson);
    assert_eq!(
        d.format,
        FileFormat::KicadSessionJson,
        "the re-sniff at BoardFileDetails.java:113 is gone; `{{` no longer means KICAD_DESIGN_JSON \
         when the caller said otherwise"
    );
    assert_eq!(d.size, 7);
    assert_eq!(d.crc32, BoardFileDetails::calculate_crc32(b"{\"a\":1}"));
    assert_eq!(d.get_data(), b"{\"a\":1}");

    let mut d = BoardFileDetails::default();
    d.set_data(b"(ses X)".to_vec(), FileFormat::Ses);
    assert_eq!(d.format, FileFormat::Ses);

    let mut job = RoutingJob::default();
    assert!(job.set_rules_bytes(b"(rules (clearance 200))"));
    assert_eq!(job.rules.as_ref().expect("rules").format, FileFormat::Rules);
    let mut job = RoutingJob::default();
    assert!(job.set_rules_bytes(b"not a rules file"));
    assert_eq!(
        job.rules.as_ref().expect("rules").format,
        FileFormat::Unknown,
        "setRules' own `RULES`-then-lose-it is Java's and is kept — it just lives at the caller"
    );
}

#[test]
fn try_to_set_input_leaves_a_non_null_but_empty_input_when_it_fails() {
    let mut job = RoutingJob::default();
    assert!(!job.set_input_bytes(Some(b"hello!")));
    let input = job
        .input
        .as_ref()
        .expect("assigned before the content is looked at");
    assert_eq!(input.format, FileFormat::Unknown);
    assert_eq!(input.size, 0);
    assert_eq!(input.crc32, 0);

    assert!(!RoutingJob::default().set_input_bytes(None));
}

#[test]
fn try_to_set_output_file_rejects_rules_and_rewrites_kicad_design_json() {
    let mut job = RoutingJob::default();
    assert!(!job.try_to_set_output_file(Some(Path::new("/tmp/out.rules"))));
    assert!(job.output.is_none());
    assert!(!job.try_to_set_output_file(Some(Path::new("/tmp/out.txt"))));
    assert!(!job.try_to_set_output_file(None));

    for (path, want) in [
        ("/tmp/out.ses", FileFormat::Ses),
        ("/tmp/out.dsn", FileFormat::Dsn),
        ("/tmp/out.frb", FileFormat::Frb),
        ("/tmp/out.scr", FileFormat::Scr),
        ("/tmp/out.json", FileFormat::KicadSessionJson),
    ] {
        let mut job = RoutingJob::default();
        assert!(job.try_to_set_output_file(Some(Path::new(path))), "{path}");
        assert_eq!(job.output.as_ref().unwrap().format, want, "{path}");
    }
}

#[test]
fn is_cli_terminal_totalises_javas_invalid_omission() {
    for state in [
        RoutingJobState::Completed,
        RoutingJobState::Terminated,
        RoutingJobState::TimedOut,
        RoutingJobState::Cancelled,
        RoutingJobState::Invalid,
    ] {
        assert!(state.is_cli_terminal(), "{state:?}");
    }
    for state in [
        RoutingJobState::Queued,
        RoutingJobState::ReadyToStart,
        RoutingJobState::Running,
        RoutingJobState::Paused,
        RoutingJobState::Stopping,
    ] {
        assert!(!state.is_cli_terminal(), "{state:?}");
    }
    assert_eq!(RoutingJobState::default(), RoutingJobState::Invalid);
    assert_eq!(RoutingJobState::TimedOut.java_name(), "TIMED_OUT");
}

#[test]
fn the_session_host_check_normalises_null_and_blank_and_then_demands_two_parts() {
    assert_eq!(validate_session_host(None).unwrap(), "Unknown/0.0");
    assert_eq!(validate_session_host(Some("   ")).unwrap(), "Unknown/0.0");
    assert_eq!(validate_session_host(Some("")).unwrap(), "Unknown/0.0");
    assert_eq!(
        validate_session_host(Some("Freerouting/2.3.1-SNAPSHOT")).unwrap(),
        "Freerouting/2.3.1-SNAPSHOT"
    );
    assert_eq!(validate_session_host(Some("/a")).unwrap(), "/a");
    for bad in ["a", "a/", "a/b/c", "a//b"] {
        let err = validate_session_host(Some(bad)).unwrap_err();
        assert!(matches!(err, Error::Session(_)), "{bad}");
        assert_eq!(
            err.to_string(),
            format!(
                "Invalid host value: '{bad}'. It must contain the host name and version separated by '/'."
            )
        );
    }
}

#[test]
fn set_input_from_a_file_derives_the_output_name_from_the_content_not_the_extension() {
    let dir = scratch("derive");
    std::fs::write(dir.join("d.txt"), b"(pcb D)\n").unwrap();
    let mut job = RoutingJob::default();
    job.set_input(&dir.join("d.txt")).unwrap();
    assert_eq!(job.input.as_ref().unwrap().format, FileFormat::Dsn);
    assert_eq!(job.output.as_ref().unwrap().get_filename(), "d.ses");
    assert_eq!(job.name, "d");
}

#[test]
fn an_frb_or_kicad_input_derives_its_own_path_as_its_output() {
    let dir = scratch("selfoutput");
    std::fs::write(dir.join("b.frb"), [0xAC, 0xED, 0x00, 0x05, 0x00, 0x05]).unwrap();
    std::fs::write(dir.join("c.json"), b"{\"a\":1}\n").unwrap();
    for (name, format) in [
        ("b.frb", FileFormat::Frb),
        ("c.json", FileFormat::KicadDesignJson),
    ] {
        let mut job = RoutingJob::default();
        job.set_input(&dir.join(name)).unwrap();
        assert_eq!(job.input.as_ref().unwrap().format, format);
        let output = job.output.as_ref().expect("derived");
        assert_eq!(output.get_filename(), name);
        assert_eq!(
            output.get_absolute_path(),
            job.input.as_ref().unwrap().get_absolute_path()
        );
    }
}

#[test]
fn an_unrecognised_dsn_falls_back_to_its_extension_with_size_and_crc_still_zero() {
    let dir = scratch("fallback");
    std::fs::write(dir.join("e.dsn"), b"hello!\n").unwrap();
    let mut job = RoutingJob::default();
    job.set_input(&dir.join("e.dsn")).unwrap();
    let input = job.input.as_ref().unwrap();
    assert_eq!(input.format, FileFormat::Dsn);
    assert_eq!(input.size, 0);
    assert_eq!(input.crc32, 0);
    assert_eq!(input.get_data(), b"");
    assert_eq!(job.output.as_ref().unwrap().get_filename(), "e.ses");
}

#[test]
fn a_ses_input_derives_no_output_but_still_names_the_job() {
    let dir = scratch("ses");
    std::fs::write(dir.join("g.ses"), b"(ses G)\n").unwrap();
    let mut job = RoutingJob::default();
    job.set_input(&dir.join("g.ses")).unwrap();
    assert_eq!(job.input.as_ref().unwrap().format, FileFormat::Ses);
    assert!(job.output.is_none());
    assert_eq!(job.name, "g");
}

#[test]
fn a_missing_input_file_is_the_only_way_job_input_stays_none() {
    let dir = scratch("missing");
    let mut job = RoutingJob::default();
    let err = job.set_input(&dir.join("nope.dsn")).unwrap_err();
    assert!(matches!(err, Error::Io(_)));
    assert!(job.input.is_none());
}

#[test]
fn a_missing_rules_file_is_silently_ignored() {
    let dir = scratch("rules");
    let mut job = RoutingJob::default();
    job.set_rules(&dir.join("nope.rules")).unwrap();
    assert!(job.rules.is_none());

    std::fs::write(dir.join("x.rules"), b"nonsense\n").unwrap();
    job.set_rules(&dir.join("x.rules")).unwrap();
    let rules = job.rules.as_ref().unwrap();
    assert_eq!(rules.get_filename(), "x.rules");
    assert_eq!(rules.format, FileFormat::Unknown);

    std::fs::write(dir.join("y.rules"), b"(rules X)\n").unwrap();
    job.set_rules(&dir.join("y.rules")).unwrap();
    assert_eq!(job.rules.as_ref().unwrap().format, FileFormat::Rules);
}

#[test]
fn a_defaulted_job_carries_javas_new_router_settings_not_an_empty_one() {
    let job = RoutingJob::default();
    assert!(
        job.router_settings.fanout.is_some(),
        "RouterSettings.java:122"
    );
    assert!(
        job.router_settings.optimizer.is_some(),
        "RouterSettings.java:120"
    );
    assert!(
        job.router_settings.scoring.is_some(),
        "RouterSettings.java:121"
    );
    assert_eq!(job.router_settings, fr_settings::RouterSettings::new());
    assert_ne!(
        job.router_settings,
        fr_settings::RouterSettings::default(),
        "`new RouterSettings()` and an all-null one must not be the same object"
    );
    assert_eq!(
        RoutingJob::new(SessionId::NIL).router_settings,
        fr_settings::RouterSettings::new()
    );
    assert_eq!(
        job.resource_usage,
        fr_core::RouterJobResourceUsage::default()
    );
}

#[test]
fn set_dummy_input_file_matches_a_bare_dsn_suffix_not_a_dot_dsn_one() {
    let mut job = RoutingJob::default();
    job.set_dummy_input_file(Some("boarddsn"));
    assert_eq!(job.input.as_ref().unwrap().format, FileFormat::Dsn);

    let mut job = RoutingJob::default();
    job.set_dummy_input_file(Some("board.dsn.bak"));
    assert_eq!(job.input.as_ref().unwrap().format, FileFormat::Unknown);
    assert!(job.output.is_some());
}

#[test]
fn the_rules_and_eagle_script_files_are_derived_from_the_output_not_the_input() {
    let mut job = RoutingJob::default();
    assert!(job.get_rules_file().is_none());
    assert!(job.get_eagle_script_file().is_none());
    assert!(job.try_to_set_output_file(Some(Path::new("/tmp/out.ses"))));
    assert_eq!(
        job.get_rules_file().unwrap(),
        PathBuf::from("/tmp/out.rules")
    );
    assert_eq!(
        job.get_eagle_script_file().unwrap(),
        PathBuf::from("/tmp/out.scr")
    );
}

#[test]
fn the_job_constructors_build_the_java_names() {
    let job = RoutingJob::default();
    assert_eq!(job.name, "J-000000");
    assert_eq!(job.short_name, "000000");
    assert_eq!(job.state, RoutingJobState::Invalid);
    assert!(job.session_id.is_none());

    let session = fr_core::SessionId::from_bytes([
        0x0a, 0xb1, 0xc2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ]);
    let id =
        fr_core::JobId::from_bytes([0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let job = RoutingJob::with_id(session, id);
    assert_eq!(job.short_name, "0AB1C2\\DEADBE");
    assert_eq!(job.name, "J-DEADBE");
    assert_eq!(job.log_prefix(), "[0AB1C2\\DEADBE] ");
    assert_eq!(job.session_id, Some(session));
    assert_eq!(
        session.to_java_string(),
        "0ab1c203-0405-0607-0809-0a0b0c0d0e0f"
    );
}

#[test]
fn get_duration_is_none_before_the_job_starts() {
    let mut job = RoutingJob::default();
    assert!(job.get_duration().is_none());
    let t0 = std::time::Instant::now();
    job.started_at = Some(t0);
    assert!(job.get_duration().is_some());
    job.finished_at = Some(t0 + std::time::Duration::from_secs(7));
    assert_eq!(job.get_duration(), Some(std::time::Duration::from_secs(7)));
}

#[test]
fn the_current_pass_accessors_round_trip() {
    let mut job = RoutingJob::default();
    assert_eq!(job.get_current_pass(), 0);
    job.set_current_pass(4);
    assert_eq!(job.get_current_pass(), 4);
    assert!(!job.is_cancelled_by_user());
    job.set_cancelled_by_user(true);
    assert!(job.is_cancelled_by_user());
}


fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p8t1-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Ctx {
    cwd: String,
    cwd_basename: String,
    scratch_dir: String,
}

impl Ctx {
    fn new() -> Ctx {
        let cwd = std::env::current_dir().unwrap();
        let scratch = scratch("replay");
        Ctx {
            cwd: cwd.to_string_lossy().into_owned(),
            cwd_basename: cwd.file_name().unwrap().to_string_lossy().into_owned(),
            scratch_dir: scratch.to_string_lossy().into_owned(),
        }
    }

    fn write_scratch_files(&self) {
        for (name, spec) in SIF_FILES {
            let path = PathBuf::from(&self.scratch_dir).join(name);
            if *spec == "!" {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            std::fs::write(&path, decode(spec)).unwrap();
        }
    }

    fn norm(&self, s: &str) -> String {
        let s = s.replace(&self.scratch_dir, "<SCRATCH>");
        let s = s.replace(&self.cwd, "<CWD>");
        if s == self.cwd_basename {
            return "<CWD-BASENAME>".to_string();
        }
        s
    }

            fn rebuild(&self, line: &str) -> Option<String> {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "SNIFF" => {
                let content = if f[2] == "<null>" {
                    None
                } else {
                    Some(from_hex(f[2]))
                };
                let hangs = content
                    .as_deref()
                    .is_some_and(FileFormat::java_shift_loop_hangs);
                let format = FileFormat::sniff_bytes_opt(content.as_deref()).java_name();
                let answer = if hangs {
                    format!("XDIFF\tjava=HANG(>5000ms)\trust={format}")
                } else {
                    format.to_string()
                };
                Some(format!("SNIFF\t{}\t{}\t{answer}", f[1], f[2]))
            }
            "EXT" => {
                let raw = unquote(f[2]);
                Some(format!(
                    "EXT\t{}\t{}\t{}",
                    f[1],
                    f[2],
                    FileFormat::from_path(Path::new(&raw)).java_name()
                ))
            }
            "CFE" => {
                let path = unquote(f[2]);
                let ext = unquote(f[3]);
                let value = RoutingJob::change_file_extension(&path, &ext);
                let answer = if java_parent_is_null(&path) {
                    format!(
                        "XDIFF\tjava=NullPointerException\trust={}",
                        self.norm(&value)
                    )
                } else {
                    self.norm(&value)
                };
                Some(format!("CFE\t{}\t{}\t{}\t{answer}", f[1], f[2], f[3]))
            }
            "SETFN" => {
                let preset = f[2];
                let raw = unquote(f[3]);
                let arg = if raw == "<null>" {
                    None
                } else {
                    Some(raw.as_str())
                };
                let mut d = BoardFileDetails::default();
                if preset != "-" {
                    d.format = FileFormat::from_java_name(preset).unwrap();
                }
                let java_npes = arg == Some("/");
                d.set_filename(arg);
                let body = format!(
                    "dir={}\tname={}\tformat={}\tabs={}\tstem={}",
                    quote(&self.norm(d.get_directory_path())),
                    quote(&self.norm(d.get_filename())),
                    d.format.java_name(),
                    quote(&self.norm(&d.get_absolute_path())),
                    quote(&self.norm(&d.get_filename_without_extension())),
                );
                let answer = if java_npes {
                    format!("XDIFF\tjava=NullPointerException\trust={body}")
                } else {
                    body
                };
                Some(format!("SETFN\t{}\t{preset}\t{}\t{answer}", f[1], f[3]))
            }
            "CRC" => {
                let idx: usize = f[1].parse().unwrap();
                let data = if f[2] == "bytes" {
                    decode(CRC_LITERAL_SPECS[idx])
                } else {
                    std::fs::read(parity::fixture(f[3])).unwrap()
                };
                Some(format!(
                    "CRC\t{}\t{}\t{}\tlen={}\t{}",
                    f[1],
                    f[2],
                    f[3],
                    data.len(),
                    BoardFileDetails::calculate_crc32(&data)
                ))
            }
            "TSI" => {
                let content = if f[2] == "<null>" {
                    None
                } else {
                    Some(from_hex(f[2]))
                };
                let mut job = RoutingJob::default();
                let ok = job.set_input_bytes(content.as_deref());
                let input = job.input.as_ref().unwrap();
                Some(format!(
                    "TSI\t{}\t{}\tok={ok}\tformat={}\tsize={}\tcrc32={}",
                    f[1],
                    f[2],
                    input.format.java_name(),
                    input.size,
                    input.crc32
                ))
            }
            "TSOF" => {
                let spec = unquote(f[2]);
                let file: Option<PathBuf> = if spec == "!<null>" {
                    None
                } else if let Some(bare) = spec.strip_prefix('!') {
                    Some(PathBuf::from(bare))
                } else {
                    Some(PathBuf::from(&self.scratch_dir).join(&spec))
                };
                let mut job = RoutingJob::default();
                let ok = job.try_to_set_output_file(file.as_deref());
                let out = match job.output.as_ref() {
                    None => "output=<null>".to_string(),
                    Some(o) => format!(
                        "format={}\tdir={}\tname={}\tsize={}\tcrc32={}",
                        o.format.java_name(),
                        quote(&self.norm(o.get_directory_path())),
                        quote(&self.norm(o.get_filename())),
                        o.size,
                        o.crc32
                    ),
                };
                Some(format!("TSOF\t{}\t{}\tok={ok}\t{out}", f[1], f[2]))
            }
            "SIF" => {
                let name = f[2];
                let path = PathBuf::from(&self.scratch_dir).join(name);
                let mut job = RoutingJob::default();
                let id_derived = format!("J-{}", job.id.short_upper6());
                let answer = match job.set_input(&path) {
                    Err(_) => format!(
                        "throws=FileNotFoundException\tinput={}",
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
                                quote(&self.norm(o.get_directory_path())),
                                quote(&self.norm(o.get_filename()))
                            ),
                        };
                        format!(
                            "in.format={}\tin.dir={}\tin.name={}\tin.size={}\tin.crc32={}\t{out}\tjob.name={}",
                            input.format.java_name(),
                            quote(&self.norm(input.get_directory_path())),
                            quote(&self.norm(input.get_filename())),
                            input.size,
                            input.crc32,
                            quote(&if job.name == id_derived {
                                "<ID-DERIVED>".to_string()
                            } else {
                                self.norm(&job.name)
                            })
                        )
                    }
                };
                Some(format!("SIF\t{}\t{name}\t{answer}", f[1]))
            }
            _ => None,
        }
    }
}

fn java_parent_is_null(filename: &str) -> bool {
    let absolute = filename.starts_with('/');
    let segments: Vec<&str> = filename.split('/').filter(|x| !x.is_empty()).collect();
    let normalized = if segments.is_empty() {
        if absolute {
            "/".to_string()
        } else {
            String::new()
        }
    } else if absolute {
        format!("/{}", segments.join("/"))
    } else {
        segments.join("/")
    };
    match normalized.rfind('/') {
        None => true,
        Some(0) => normalized.len() == 1,
        Some(_) => false,
    }
}

fn from_hex(h: &str) -> Vec<u8> {
    if h == "-" {
        return Vec::new();
    }
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

fn decode(spec: &str) -> Vec<u8> {
    let body = &spec[2..];
    match spec.as_bytes()[0] {
        b's' => body.bytes().collect(),
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

fn unquote(s: &str) -> String {
    let inner = &s[1..s.len() - 1];
    let chars: Vec<char> = inner.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            i += 1;
            if chars[i] == 'u' {
                let code = u32::from_str_radix(&chars[i + 1..i + 5].iter().collect::<String>(), 16)
                    .unwrap();
                out.push(char::from_u32(code).unwrap());
                i += 5;
                continue;
            }
            out.push(chars[i]);
        } else {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}
