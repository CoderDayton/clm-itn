use crate::lines::*;

fn case(s: &str, w: Option<&str>, t: &str) -> Case {
    Case {
        spoken: s.into(),
        written: w.map(Into::into),
        tag: t.into(),
    }
}

fn spoken(cases: &[Case]) -> Vec<&str> {
    cases.iter().map(|c| c.spoken.as_str()).collect()
}

#[test]
fn tsv_reads_spoken_written_and_tag_skipping_blanks_and_comments() {
    let text = "# header comment\n\
                set an alarm for seven a m\tset an alarm for 7 a.m.\ttime\r\n\
                \n\
                i have one dog\ti have one dog\n\
                it was ok\tit was ok\tkeep!\n\
                no tab line\n";
    let (cases, bad) = tsv_cases(text);
    assert_eq!(bad, 0);
    assert_eq!(
        cases,
        vec![
            case(
                "set an alarm for seven a m",
                Some("set an alarm for 7 a.m."),
                "time"
            ),
            case("i have one dog", Some("i have one dog"), UNTAGGED),
            case("it was ok", Some("it was ok"), "keep!"),
            case("no tab line", None, UNTAGGED),
        ]
    );
    assert!(cases[2].debatable());
    assert!(!cases[0].debatable());
}

#[test]
fn tsv_counts_empty_sides_as_malformed() {
    let (cases, bad) = tsv_cases("\tonly written\nonly spoken\t\nok\tok\na\tb\t \n");
    assert_eq!(
        cases,
        vec![
            case("ok", Some("ok"), UNTAGGED),
            case("a", Some("b"), UNTAGGED)
        ]
    );
    assert_eq!(bad, 2);
}

#[test]
fn jsonl_reads_the_first_known_text_field() {
    let text = concat!(
        "{\"id\": 1, \"spoken\": \"five pm\", \"raw\": \"ignored\"}\n",
        "\n",
        "{\"meta\": {\"text\": \"nested\"}, \"input\": \"two \\\"tabs\\\"\\tok\"}\n",
        "{\"tags\": [\"a\", {\"b\": 1}], \"raw\": \"caf\\u00e9 \\ud83d\\ude00\\n\", \"n\": -1.5e3}\r\n",
        "{\"text\": \"plain\", \"ok\": true, \"none\": null}\n",
        "{\"sentence\": \"three dogs\", \"gold\": \"3 dogs\", \"category\": \"cardinal\"}\n",
    );
    let cases = jsonl_cases(text).unwrap();
    assert_eq!(
        spoken(&cases),
        [
            "five pm",
            "two \"tabs\"\tok",
            "café 😀\n",
            "plain",
            "three dogs"
        ]
    );
    assert_eq!(cases[0], case("five pm", None, UNTAGGED));
    assert_eq!(cases[4], case("three dogs", Some("3 dogs"), "cardinal"));
}

#[test]
fn jsonl_reads_written_and_tag_keys() {
    let cases =
        jsonl_cases("{\"spoken\": \"ten am\", \"written\": \"10 am\", \"tag\": \"time!\"}\n")
            .unwrap();
    assert_eq!(cases, vec![case("ten am", Some("10 am"), "time!")]);
    assert!(jsonl_cases("{\"spoken\": \"a\", \"gold\": 5}\n").is_err());
}

#[test]
fn jsonl_reports_the_bad_line_number() {
    let err = jsonl_cases("{\"spoken\": \"a\"}\n{\"id\": 2}\n").unwrap_err();
    assert!(err.contains("line 2"), "{err}");
    let err = jsonl_cases("{\"spoken\": \"a\"\n").unwrap_err();
    assert!(err.contains("line 1"), "{err}");
    assert!(jsonl_cases("[\"spoken\"]\n").is_err());
    assert!(jsonl_cases("{\"spoken\": 5}\n").is_err());
}

#[test]
fn load_reads_one_file_or_every_corpus_file_in_a_folder() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("clm-bench-corpora");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("b.jsonl"), "{\"spoken\": \"ten am\"}\n").unwrap();
    std::fs::write(dir.join("a.tsv"), "five pm\t5 pm\n# note\nsix\t6\n\tbad\n").unwrap();
    std::fs::write(dir.join("c.txt"), "seven\n").unwrap();
    std::fs::write(dir.join("skip.md"), "not a corpus\n").unwrap();

    let all = load(dir.to_str().unwrap()).unwrap();
    let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
    assert!(names[0].ends_with("a.tsv") && names[1].ends_with("b.jsonl"));
    assert!(names[2].ends_with("c.txt") && names.len() == 3);
    assert_eq!(spoken(&all[0].cases), ["five pm", "six"]);
    assert_eq!(all[0].malformed, 1);
    assert_eq!(spoken(&all[1].cases), ["ten am"]);

    let one = load(dir.join("b.jsonl").to_str().unwrap()).unwrap();
    assert_eq!(one.len(), 1);
    assert!(load(dir.join("missing.tsv").to_str().unwrap()).is_err());
    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(load(empty.to_str().unwrap()).is_err());
}

#[test]
fn per_line_percentile_reads_each_line_across_passes() {
    let samples = [10, 1, 20, 2, 30, 3, 40, 4];
    assert_eq!(per_line(&samples, 2, 1.0), [40, 4]);
    assert_eq!(per_line(&samples, 2, 0.5), [20, 2]);
    assert_eq!(per_line(&samples, 2, 0.0), [10, 1]);
}

#[test]
fn worst_lists_indexes_slowest_first_and_stable_on_ties() {
    assert_eq!(worst(&[5, 9, 1, 9, 7], 3), [1, 3, 4]);
    assert_eq!(worst(&[5, 9], 5), [1, 0]);
    assert!(worst(&[], 5).is_empty());
}

#[test]
fn alloc_summary_counts_lines_that_allocate() {
    let s = alloc_summary(&[0, 2, 0, 5, 1]);
    assert_eq!(
        s,
        AllocSummary {
            calls: 5,
            total: 8,
            max: 5,
            lines_allocating: 3
        }
    );
    assert_eq!(alloc_summary(&[]), AllocSummary::default());
}
