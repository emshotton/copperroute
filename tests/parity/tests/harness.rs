use parity::*;

#[test]
fn normalize_collapses_crlf_trailing_spaces_and_blank_runs() {
    let s = "a  \r\n\r\n\r\nb\n\n\n";
    assert_eq!(normalize_whitespace(s), "a\n\nb\n");
}

#[test]
fn normalize_adds_single_trailing_newline() {
    assert_eq!(normalize_whitespace("x"), "x\n");
    assert_eq!(normalize_whitespace("x\n\n"), "x\n");
}

#[test]
fn workspace_root_contains_cargo_toml() {
    assert!(workspace_root().join("Cargo.toml").exists());
}

#[test]
fn reference_path_layout() {
    let p = reference("tutorial_board", "roundtrip.dsn");
    assert!(p.ends_with("tests/reference/tutorial_board/roundtrip.dsn"));
}

#[test]
#[should_panic(expected = "parity mismatch")]
fn assert_text_parity_panics_on_difference() {
    let dir = std::env::temp_dir().join("fr-parity-test");
    std::fs::create_dir_all(&dir).unwrap();
    let r = dir.join("ref.txt");
    std::fs::write(&r, "one\ntwo\n").unwrap();
    assert_text_parity("one\nthree\n", &r);
}

#[test]
fn assert_text_parity_passes_modulo_whitespace() {
    let dir = std::env::temp_dir().join("fr-parity-test2");
    std::fs::create_dir_all(&dir).unwrap();
    let r = dir.join("ref.txt");
    std::fs::write(&r, "one\r\ntwo  \r\n").unwrap();
    assert_text_parity("one\ntwo\n\n", &r);
}
