use std::process::Command;

#[test]
fn clm_eval_scores_the_dictation_set_without_crashing() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/dictation.tsv");
    let out = Command::new(env!("CARGO_BIN_EXE_clm-eval"))
        .args(["--lines", path])
        .output()
        .expect("spawn clm-eval");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{:?}\n{stderr}", out.status);
    let text = String::from_utf8(out.stdout).expect("utf8 output");
    assert!(text.contains("all lines"), "{text}");
    assert!(text.contains("firm (no \"!\" tags)"), "{text}");
    assert!(text.contains("malformed lines skipped: 0"), "{text}");
}
