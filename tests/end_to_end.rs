use clm_itn::{Engine, format_line};

fn engine() -> Engine<'static> {
    Engine::english()
}

fn run(engine: &Engine, text: &str) -> String {
    let tokens: Vec<&[u8]> = text.split(' ').map(str::as_bytes).collect();
    let mut out = [0u8; 256];
    let done = engine.format(&tokens, &mut out).expect("format");
    String::from_utf8(out[..done.len].to_vec()).expect("utf8")
}

#[test]
fn spoken_to_written() {
    let engine = engine();
    let cases = [
        ("at five thirty pm", "at 5:30 pm"),
        ("five oh five am", "5:05 am"),
        ("five pm", "5 pm"),
        ("at three o'clock", "at 3 o'clock"),
        ("room four oh two", "room 402"),
        ("flight one two three", "flight 123"),
        ("one hundred twenty three", "123"),
        ("two thousand five", "2005"),
        ("three point one four", "3.14"),
        ("the twenty first", "the 21st"),
        ("call me later", "call me later"),
        ("Call ME", "Call ME"),
        ("five", "5"),
    ];
    let mut failures = Vec::new();
    for (input, want) in cases {
        let got = run(&engine, input);
        if got != want {
            failures.push(format!("{input:?} -> {got:?}, want {want:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn format_line_never_cuts_a_number() {
    let engine = engine();
    let cases = [
        (
            "the twenty first of june costs two thousand five dollars",
            "the 21st of june costs $2005",
        ),
        (
            "meet me at five thirty pm in room four oh two",
            "meet me at 5:30 pm in room 402",
        ),
        (
            "one hundred forty seven thousand four hundred fifty one",
            "147451",
        ),
        (
            "it cost one hundred forty seven thousand four hundred fifty one.",
            "it cost 147451.",
        ),
        (
            "one two three four five six seven eight nine",
            "one two three four five six seven eight nine",
        ),
        ("", ""),
        ("  spaced   out  ", "spaced out"),
    ];
    for (input, want) in cases {
        let mut got = String::new();
        format_line(&engine, input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}

#[test]
fn format_line_keeps_punctuation_outside_numbers() {
    let engine = engine();
    let cases = [
        ("it was twenty five, then thirty.", "it was 25, then 30."),
        ("meet at five thirty pm.", "meet at 5:30 pm."),
        ("room (four oh two) is free", "room (402) is free"),
        ("one, two, three", "one, two, three"),
        ("First, we go home.", "First, we go home."),
        ("second, the price.", "second, the price."),
        ("we saw two, then left", "we saw two, then left"),
        ("twenty, thirty, forty", "20, 30, 40"),
        ("at five, then home", "at 5, then home"),
        ("\"twenty five\"", "\"25\""),
        ("at five p.m. sharp", "at 5 p.m. sharp"),
        ("hello, world!", "hello, world!"),
        ("at seven p m.", "at 7 pm."),
        ("at eleven forty a m", "at 11:40 am"),
        ("at eleven forty a m, then", "at 11:40 am, then"),
        ("the meeting is at three p m", "the meeting is at 3 pm"),
        ("the deadline is twelve noon", "the deadline is noon"),
        ("it ends at twelve midnight.", "it ends at midnight."),
        ("twelve nooners", "12 nooners"),
        ("five a day", "five a day"),
        ("plan a m", "plan a m"),
        ("at seven a m e s t", "at 7 am EST"),
        ("at eleven forty a m g m t.", "at 11:40 am GMT."),
        ("call a b c now", "call a b c now"),
        ("the e s t office", "the e s t office"),
        ("...", "..."),
        (
            "in sixteen sixty five\u{201D} he said",
            "in 1665\u{201D} he said",
        ),
        ("\u{201C}twenty five\u{2019}", "\u{201C}25\u{2019}"),
        ("about twenty five\u{2026}", "about 25\u{2026}"),
    ];
    for (input, want) in cases {
        let mut got = String::new();
        format_line(&engine, input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}

#[test]
fn format_line_keeps_ambiguous_runs_as_words() {
    let engine = engine();
    let cases = [
        ("uh twenty uh five", "uh twenty uh five"),
        ("twenty, uh, five", "twenty, uh, five"),
        ("uh twenty five", "uh 25"),
        ("twenty five um then", "25 um then"),
        ("twenty fives", "twenty fives"),
        ("twenty five's", "twenty five's"),
        ("eighteen hundreds", "eighteen hundreds"),
        ("hundreds of people", "hundreds of people"),
        ("the timer is thirty seconds", "the timer is 30 seconds"),
        ("twelve and a half", "twelve and a half"),
        ("two acres and a half", "two acres and a half"),
        ("five and a quarter miles", "five and a quarter miles"),
        ("in eighteen ninety two and then", "in 1892 and then"),
        ("zero point two seconds faster", "0.2 seconds faster"),
        ("it's 5 thirty", "it's 5 thirty"),
        ("twenty 5", "twenty 5"),
        ("room 5 is open", "room 5 is open"),
        ("at five thirty pm", "at 5:30 pm"),
        ("she finished third", "she finished third"),
        ("she's in fifth grade", "she's in fifth grade"),
        ("we live on the ninth floor", "we live on the ninth floor"),
        (
            "we celebrate the fourth of july",
            "we celebrate the fourth of july",
        ),
        ("see you on the third", "see you on the third"),
        ("he's in tenth grade", "he's in 10th grade"),
        ("the twenty third", "the 23rd"),
        (
            "born on july fourth nineteen ninety",
            "born on July 4, 1990",
        ),
        ("on march third", "on March 3"),
        ("fifty percent off", "50% off"),
        ("a fee of point five percent", "a fee of 0.5%"),
        ("it is point five", "it is 0.5"),
        ("three point five percent", "3.5%"),
        ("five meters", "5 meters"),
        ("i ran five miles", "i ran 5 miles"),
        ("it weighs twenty five pounds", "it weighs 25 pounds"),
        (
            "the room is four hundred square feet",
            "the room is 400 square feet",
        ),
        ("eight gigabytes of memory", "8 gigabytes of memory"),
        ("a two point four gigahertz band", "a 2.4 gigahertz band"),
        ("it costs fifty cents", "it costs 50 cents"),
        ("one cent", "1 cent"),
        ("it costs five euros", "it costs 5 euros"),
        ("one euro", "1 euro"),
        ("five dollars", "$5"),
        ("ten yen", "¥10"),
        ("the budget is ten thousand dollars", "the budget is $10000"),
        (
            "we raised two hundred fifty thousand dollars",
            "we raised $250000",
        ),
        (
            "the video has three hundred thousand views",
            "the video has 300000 views",
        ),
        (
            "i walked one thousand two hundred thirty four steps",
            "i walked 1234 steps",
        ),
        ("it's four thousand miles away", "it's 4000 miles away"),
        ("born in nineteen ninety", "born in 1990"),
        ("in two thousand five", "in 2005"),
        ("room four oh two one", "room 4021"),
        ("twelve million dollars", "$12 million"),
    ];
    for (input, want) in cases {
        let mut got = String::new();
        format_line(&engine, input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}

#[test]
fn single_digits_style_writes_lone_numbers_as_digits_and_keeps_ordinals() {
    let ap = engine();
    let digits = Engine::english().with_style(clm_itn::Style::SingleDigits);
    let cases = [
        ("like eight years", "like eight years", "like 8 years"),
        (
            "track two and track three",
            "track two and track three",
            "track 2 and track 3",
        ),
        (
            "from three to five pm",
            "from three to 5 pm",
            "from 3 to 5 pm",
        ),
        ("so three.", "so three.", "so 3."),
        ("do one and two", "do one and two", "do 1 and 2"),
        (
            "fix eleven, twelve, and thirteen",
            "fix 11, 12, and 13",
            "fix 11, 12, and 13",
        ),
        (
            "she finished third",
            "she finished third",
            "she finished third",
        ),
        ("twenty five people", "25 people", "25 people"),
    ];
    for (input, want_ap, want_digits) in cases {
        for (e, want) in [(&ap, want_ap), (&digits, want_digits)] {
            let mut got = String::new();
            format_line(e, input, &mut got);
            assert_eq!(got, want, "{input:?}");
        }
    }
}

#[test]
fn an_ordinal_after_code_stays_a_word() {
    let engine = engine();
    for (input, want) in [
        ("review of the code first", "review of the code first"),
        ("code four oh two", "code 402"),
    ] {
        let mut got = String::new();
        format_line(&engine, input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}
