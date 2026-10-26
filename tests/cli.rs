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
    assert_eq!(lines, ["5 \u{FFFD} 6", "25"]);
}

#[test]
fn no_flags_switch_settings_off() {
    let all = run(&[], b"like eight years at fifty percent\n");
    assert_eq!(all.stdout, b"like 8 years at 50%\n");
    let off = run(
        &["--no-single-digits", "--no-percent"],
        b"like eight years at fifty percent\n",
    );
    assert!(off.status.success());
    assert_eq!(off.stdout, b"like eight years at 50 percent\n");
}

#[test]
fn help_lists_every_setting() {
    let out = run(&["--help"], b"");
    let text = String::from_utf8(out.stdout).expect("utf8");
    for s in clm_itn::Setting::ALL {
        assert!(text.contains(&format!("--no-{}", s.name())), "{text}");
    }
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
fn help_and_version_work_next_to_other_flags() {
    let help = run(&["--no-percent", "--help"], b"");
    assert!(help.status.success(), "{:?}", help.status);
    let text = String::from_utf8(help.stdout).expect("utf8");
    assert!(text.starts_with("Usage: clm-itn"), "{text}");

    let version = run(&["-V", "--no-dates"], b"");
    assert!(version.status.success(), "{:?}", version.status);
    let text = String::from_utf8(version.stdout).expect("utf8");
    assert!(text.starts_with("clm-itn "), "{text}");

    let bad = run(&["--no-percent", "--bogus"], b"");
    assert_eq!(bad.status.code(), Some(2));
}

#[test]
fn unknown_argument_exits_with_usage_error() {
    let out = run(&["five"], b"");
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8(out.stderr).expect("utf8");
    assert!(err.contains("Usage: clm-itn"), "{err}");
}
