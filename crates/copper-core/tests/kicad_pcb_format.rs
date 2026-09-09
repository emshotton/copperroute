use copper_core::FileFormat;
use std::path::Path;

#[test]
fn it_recognises_the_extension() {
    assert_eq!(
        FileFormat::from_path(Path::new("board.kicad_pcb")),
        FileFormat::KicadPcb
    );
}

#[test]
fn it_sniffs_a_board_header() {
    assert_eq!(
        FileFormat::sniff_bytes(b"(kicad_pcb (version 20241229)"),
        FileFormat::KicadPcb
    );
}

#[test]
fn it_sniffs_a_board_header_after_newlines() {
    assert_eq!(
        FileFormat::sniff_bytes(b"\n\n(kicad_pcb (version 20241229)"),
        FileFormat::KicadPcb
    );
}

#[test]
fn it_still_sniffs_a_dsn() {
    assert_eq!(FileFormat::sniff_bytes(b"(pcb board.dsn"), FileFormat::Dsn);
}

#[test]
fn it_round_trips_its_name() {
    assert_eq!(
        FileFormat::from_name(FileFormat::KicadPcb.name()),
        Some(FileFormat::KicadPcb)
    );
}
