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

/// The flag that moves `setting` away from its default.
fn long_flag(setting: clm_itn::Setting) -> String {
    let prefix = if clm_itn::Normalizer::english().is_on(setting) {
        "--no-"
    } else {
        "--"
    };
    format!("{prefix}{}", setting.name())
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
fn no_flags_switch_settings_off() {
    let spoken = b"like eight years at fifty percent\n";
    assert_eq!(run(&[], spoken).stdout, b"like eight years at 50%\n");
    let off = run(&["--no-percent"], spoken);
    assert!(off.status.success());
    assert_eq!(off.stdout, b"like eight years at 50 percent\n");
    let digits = run(&["--single-digits", "--no-percent"], spoken);
    assert_eq!(digits.stdout, b"like 8 years at 50 percent\n");
    let later = run(&["--single-digits", "--no-single-digits"], spoken);
    assert_eq!(later.stdout, b"like eight years at 50%\n");
}

#[test]
fn text_arguments_are_normalized_one_per_line() {
    let out = run(&["at five thirty pm", "fifty percent off"], b"ignored\n");
    assert!(out.status.success(), "{:?}", out.status);
    assert_eq!(out.stdout, b"at 5:30 pm\n50% off\n");
}

#[test]
fn flags_mix_with_text_and_double_dash_ends_flags() {
    let out = run(&["--no-percent", "fifty percent off"], b"");
    assert_eq!(out.stdout, b"50 percent off\n");
    let out = run(&["--", "--no-percent five"], b"");
    assert_eq!(out.stdout, b"--no-percent five\n");
}

fn temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clm-itn-{}-{name}", std::process::id()));
    std::fs::write(&path, bytes).expect("write temp file");
    path
}

#[test]
fn file_arguments_normalize_whole_files_in_order() {
    let a = temp_file("a.txt", b"five thirty pm\r\nfifty percent");
    let b = temp_file("b.txt", b"like eight years\n");
    let (a, b) = (a.to_str().expect("utf8"), b.to_str().expect("utf8"));
    let out = run(&["-f", a, "--file", b, "last four"], b"ignored\n");
    assert!(out.status.success(), "{:?}", out.status);
    assert_eq!(out.stdout, b"5:30 pm\n50%\nlike eight years\nlast four\n");
    let joined = format!("--file={a}");
    let out = run(&[joined.as_str()], b"");
    assert_eq!(out.stdout, b"5:30 pm\n50%\n");
}

#[test]
fn dash_file_reads_stdin() {
    let out = run(&["-f", "-", "--no-percent"], b"fifty percent\n");
    assert_eq!(out.stdout, b"50 percent\n");
}

#[test]
fn missing_file_reports_its_path_and_fails() {
    let out = run(&["-f", "/nonexistent/clm-itn.txt"], b"");
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8(out.stderr).expect("utf8");
    assert!(err.contains("/nonexistent/clm-itn.txt"), "{err}");
}

#[test]
fn file_flag_without_a_path_is_a_usage_error() {
    let out = run(&["-f"], b"");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn help_has_usage_options_and_settings_sections() {
    let text = String::from_utf8(run(&["--help"], b"").stdout).expect("utf8");
    for part in ["Usage:", "Options:", "Settings:", "--file", "--version"] {
        assert!(text.contains(part), "{part}\n{text}");
    }
}

#[test]
fn short_flags_flip_settings_from_their_default_alone_or_together() {
    let spoken = b"like eight years at fifty percent\n";
    assert_eq!(
        run(&["-p"], spoken).stdout,
        b"like eight years at 50 percent\n"
    );
    assert_eq!(run(&["-d"], spoken).stdout, b"like 8 years at 50%\n");
    assert_eq!(
        run(&["-d", "-p"], spoken).stdout,
        b"like 8 years at 50 percent\n"
    );
    assert_eq!(
        run(&["-dp"], spoken).stdout,
        b"like 8 years at 50 percent\n"
    );
    assert_eq!(run(&["-dx"], spoken).status.code(), Some(2));
}

#[test]
fn every_short_flag_matches_its_long_flag_and_is_in_the_help() {
    let help = String::from_utf8(run(&["--help"], b"").stdout).expect("utf8");
    let mut seen = Vec::new();
    for s in clm_itn::Setting::ALL {
        let line = help
            .lines()
            .find(|l| l.contains(&long_flag(*s)))
            .expect("setting line");
        let short = line.trim_start().strip_prefix('-').expect("short flag");
        let short = &short[..1];
        assert!(!seen.contains(&short), "{short} repeats");
        seen.push(short);
        let long = run(&[&long_flag(*s)], b"one hundred dollars at five\n");
        let short = run(&[&format!("-{short}")], b"one hundred dollars at five\n");
        assert_eq!(long.stdout, short.stdout, "{}", s.name());
    }
}

#[test]
fn json_writes_one_object_per_line_with_escapes() {
    let out = run(
        &["--json", "at five thirty pm", "fifty \"quoted\" \\ \t"],
        b"",
    );
    assert!(out.status.success(), "{:?}", out.status);
    let want = concat!(
        "{\"input\":\"at five thirty pm\",\"output\":\"at 5:30 pm\"}\n",
        "{\"input\":\"fifty \\\"quoted\\\" \\\\ \\t\",\"output\":\"50 \\\"quoted\\\" \\\\ \\t\"}\n",
    );
    assert_eq!(String::from_utf8(out.stdout).expect("utf8"), want);
}

#[test]
fn json_reads_stdin_and_honours_settings() {
    let out = run(&["--json", "--no-percent"], b"fifty percent\n");
    assert_eq!(
        out.stdout,
        b"{\"input\":\"fifty percent\",\"output\":\"50 percent\"}\n"
    );
}

#[test]
fn json_errors_are_objects_on_stderr() {
    let out = run(&["--json", "-f", "/nonexistent/clm-itn.txt"], b"");
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8(out.stderr).expect("utf8");
    assert!(err.starts_with("{\"error\":\""), "{err}");
    assert!(err.contains("/nonexistent/clm-itn.txt"), "{err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn unknown_flag_is_a_usage_error() {
    let out = run(&["--bogus", "five"], b"");
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}

#[test]
fn help_lists_every_setting() {
    let out = run(&["--help"], b"");
    let text = String::from_utf8(out.stdout).expect("utf8");
    for s in clm_itn::Setting::ALL {
        let flag = long_flag(*s);
        assert!(text.contains(&flag), "{flag}\n{text}");
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
    assert!(text.starts_with("clm-itn "), "{text}");
    assert!(text.contains("Usage:"), "{text}");
}

#[test]
fn help_and_version_work_next_to_other_flags() {
    let help = run(&["--no-percent", "--help"], b"");
    assert!(help.status.success(), "{:?}", help.status);
    let text = String::from_utf8(help.stdout).expect("utf8");
    assert!(text.contains("Usage:"), "{text}");

    let version = run(&["-V", "--no-dates"], b"");
    assert!(version.status.success(), "{:?}", version.status);
    let text = String::from_utf8(version.stdout).expect("utf8");
    assert!(text.starts_with("clm-itn "), "{text}");

    let bad = run(&["--no-percent", "--bogus"], b"");
    assert_eq!(bad.status.code(), Some(2));
}

#[test]
fn unknown_argument_exits_with_usage_error() {
    let out = run(&["--no-such-setting"], b"");
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8(out.stderr).expect("utf8");
    assert!(err.contains("Usage:"), "{err}");
    assert!(err.contains("--help"), "{err}");
}

#[cfg(unix)]
#[test]
fn non_utf8_argument_becomes_a_replacement_character() {
    use std::os::unix::ffi::OsStrExt;
    let arg = std::ffi::OsStr::from_bytes(b"\xff five");
    let out = Command::new(env!("CARGO_BIN_EXE_clm-itn"))
        .arg(arg)
        .output()
        .expect("run clm-itn");
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, "\u{FFFD} five\n".as_bytes());
}

#[test]
fn closed_stdout_while_reading_a_file_exits_zero() {
    let path = std::env::temp_dir().join(format!("clm-itn-pipe-{}.txt", std::process::id()));
    std::fs::write(&path, "twenty five\n".repeat(200_000)).expect("write temp file");
    let mut child = Command::new(env!("CARGO_BIN_EXE_clm-itn"))
        .arg("-f")
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn clm-itn");
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("wait");
    std::fs::remove_file(&path).expect("remove temp file");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
