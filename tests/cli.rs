use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_clm-itn"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn clm-itn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write stdin");
    child.wait_with_output().expect("wait")
}

#[test]
fn invalid_utf8_line_does_not_stop_later_lines() {
    let out = run(&[], b"five \xff six\r\ntwenty five\n");
    assert!(out.status.success(), "{:?}", out.status);
    let text = String::from_utf8(out.stdout).expect("utf8 output");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines, ["five \u{FFFD} six", "25"]);
}

#[test]
fn single_digits_flag_writes_lone_numbers_as_digits() {
    let plain = run(&[], b"like eight years\n");
    assert_eq!(plain.stdout, b"like eight years\n");
    let digits = run(&["--single-digits"], b"like eight years\n");
    assert!(digits.status.success());
    assert_eq!(digits.stdout, b"like 8 years\n");
}

#[test]
fn version_prints_name_and_version() {
    let out = run(&["--version"], b"");
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        text.trim(),
        format!("clm-itn {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_prints_usage() {
    let out = run(&["--help"], b"");
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).expect("utf8");
    assert!(text.starts_with("Usage: clm-itn"), "{text}");
}

#[test]
fn unknown_argument_exits_with_usage_error() {
    let out = run(&["five"], b"");
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8(out.stderr).expect("utf8");
    assert!(err.contains("Usage: clm-itn"), "{err}");
}
