use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_freerouting"))
}

#[test]
fn legacy_de_do_reaches_route_stub() {
    let out = bin()
        .args(["-de", "a.dsn", "-do", "b.ses", "-mp", "1"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(3),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("route: not implemented"));
}

#[test]
fn legacy_di_is_rejected() {
    let out = bin().args(["-di", "dir"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not supported"));
}

#[test]
fn subcommand_help_works() {
    let out = bin().args(["route", "--help"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("--max-passes"));
}
