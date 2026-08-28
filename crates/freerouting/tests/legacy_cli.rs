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
fn legacy_de_takes_several_files_and_warns_on_unknown_extensions() {
    let out = bin()
        .args(["-de", "a.dsn", "a.rules", "weird.txt", "-do", "b.ses"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "stderr: {stderr}");
    assert!(
        stderr.contains("warning: ignoring input file with unknown extension: weird.txt"),
        "stderr: {stderr}"
    );
    // The unknown file must not have displaced the DSN (tracing colours the field, so match
    // the value alone).
    assert!(
        stderr.contains("route: not implemented"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("a.dsn"), "stderr: {stderr}");
    // …and it appears exactly once, in the warning — never as the route input.
    assert_eq!(stderr.matches("weird.txt").count(), 1, "stderr: {stderr}");
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
