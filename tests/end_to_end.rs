use clm_itn::{Normalizer, Setting};

/// AP style: lone single numbers stay words.
fn engine() -> Normalizer<'static> {
    Normalizer::english().without(Setting::SingleDigits)
}

fn line(engine: &Normalizer, text: &str) -> String {
    engine.normalize(text)
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
        ("five", "five"),
    ];
    let mut failures = Vec::new();
    for (input, want) in cases {
        let got = line(&engine, input);
        if got != want {
            failures.push(format!("{input:?} -> {got:?}, want {want:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn normalize_never_cuts_a_number() {
    let engine = engine();
    let cases = [
        (
            "the twenty first of june costs two thousand five dollars",
            "the 21st of June costs $2,005",
        ),
        (
            "meet me at five thirty pm in room four oh two",
            "meet me at 5:30 pm in room 402",
        ),
        (
            "one hundred forty seven thousand four hundred fifty one",
            "147,451",
        ),
        (
            "it cost one hundred forty seven thousand four hundred fifty one.",
            "it cost 147,451.",
        ),
        ("one two three four five six seven eight nine", "123456789"),
        ("", ""),
        ("  spaced   out  ", "  spaced   out  "),
    ];
    for (input, want) in cases {
        let mut got = String::new();
        engine.normalize_into(input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}

#[test]
fn normalize_keeps_punctuation_outside_numbers() {
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
        engine.normalize_into(input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}

#[test]
fn normalize_keeps_ambiguous_runs_as_words() {
    let engine = engine();
    let cases = [
        ("uh twenty uh five", "uh twenty uh five"),
        ("twenty, uh, five", "twenty, uh, five"),
        ("uh twenty five", "uh 25"),
        ("twenty five um then", "25 um then"),
        ("twenty fives", "twenty fives"),
        ("twenty five's", "twenty five's"),
        ("twenty billions", "twenty billions"),
        ("twenty trillions", "twenty trillions"),
        ("twenty quadrillion's", "twenty quadrillion's"),
        ("eighteen hundreds", "1800s"),
        ("hundreds of people", "hundreds of people"),
        ("the timer is thirty seconds", "the timer is 30 seconds"),
        ("twelve and a half", "twelve and a half"),
        ("two acres and a half", "two acres and a half"),
        ("five and a quarter miles", "five and a quarter miles"),
        ("in eighteen ninety two and then", "in 1892 and then"),
        ("zero point two seconds faster", "0.2 seconds faster"),
        ("twenty 5", "twenty 5"),
        ("room 5 is open", "room 5 is open"),
        ("at five thirty pm", "at 5:30 pm"),
        ("she finished third", "she finished third"),
        ("she's in fifth grade", "she's in fifth grade"),
        ("we live on the ninth floor", "we live on the ninth floor"),
        (
            "we celebrate the fourth of july",
            "we celebrate the 4th of July",
        ),
        ("see you on the third", "see you on the third"),
        ("he's in tenth grade", "he's in 10th grade"),
        ("the twenty third", "the 23rd"),
        (
            "born on july fourth nineteen ninety",
            "born on July 4th, 1990",
        ),
        ("on march third", "on March 3rd"),
        ("january eighth", "January 8th"),
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
        ("ten yen", "10 yen"),
        ("five hundred won", "500 won"),
        ("five hundred rupees", "500 rupees"),
        ("minus five degrees", "minus 5 degrees"),
        ("two kilo liters", "2 kilo liters"),
        ("five square meters", "5 square meters"),
        (
            "the budget is ten thousand dollars",
            "the budget is $10,000",
        ),
        (
            "we raised two hundred fifty thousand dollars",
            "we raised $250,000",
        ),
        (
            "the video has three hundred thousand views",
            "the video has 300,000 views",
        ),
        (
            "i walked one thousand two hundred thirty four steps",
            "i walked 1,234 steps",
        ),
        ("it's four thousand miles away", "it's 4,000 miles away"),
        ("born in nineteen ninety", "born in 1990"),
        ("in two thousand five", "in 2005"),
        ("room four oh two one", "room 4021"),
        ("twelve million dollars", "$12 million"),
    ];
    for (input, want) in cases {
        let mut got = String::new();
        engine.normalize_into(input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}

#[test]
fn single_digits_writes_lone_numbers_as_digits_and_keeps_ordinals() {
    let ap = engine();
    let digits = Normalizer::english();
    let cases = [
        ("like eight years", "like eight years", "like 8 years"),
        (
            "track two and track three",
            "track 2 and track 3",
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
            e.normalize_into(input, &mut got);
            assert_eq!(got, want, "{input:?}");
        }
    }
}

#[test]
fn single_digits_keeps_one_to_nine_as_words_when_not_a_count() {
    let digits = Normalizer::english();
    let words = [
        "no one knows",
        "which one do you want",
        "this one is better",
        "that's the one i want",
        "on the one hand it's cheap",
        "one of them is broken",
        "one day we will go",
        "Oh, one more thing.",
        "check them one by one",
        "it's one thing to say it",
        "you get to look at one every time",
        "if it's one piece, keep it",
        "one at a time please",
        "one way or another",
        "it's a one way street",
        "give me one sec",
        "they help one another",
        "she's one in a million",
        "it arrived in one piece",
        "two heads are better than one",
        "the two of them left",
        "two of us can go",
        "the three of us went",
        "i'm on cloud nine",
        "six of one half a dozen of the other",
        "two thirds of voters agree",
        "it's three quarters full",
        "override the other one.",
        "delete the old one, then",
        "whichever one.",
        "the bottom one is how",
        "pick one that works",
        "One is fine",
        "every one costed money",
        "Here's one.",
        "is it the Orca one?",
        "pick up the next one and make it",
        "if there's one that's smaller",
    ];
    for input in words {
        assert_eq!(line(&digits, input), input, "{input:?}");
    }
    let counts = [
        ("all into one file", "all into 1 file"),
        ("the next five years", "the next 5 years"),
        ("one terabyte", "1 terabyte"),
        ("two or three seconds", "2 or 3 seconds"),
        ("go with option one, and", "go with option 1, and"),
        ("on step one.", "on step 1."),
        ("number one is a", "number 1 is a"),
        ("one of five", "1 of 5"),
        ("one in five people", "1 in 5 people"),
        ("in one day", "in 1 day"),
        ("one in the morning", "1 in the morning"),
        ("the one hundred people", "the 100 people"),
        ("i have one question", "i have 1 question"),
        ("twenty one of them", "21 of them"),
        ("do one and two", "do 1 and 2"),
    ];
    for (input, want) in counts {
        assert_eq!(line(&digits, input), want, "{input:?}");
    }
}

#[test]
fn a_label_word_writes_the_next_number_as_digits() {
    let engine = engine();
    for (input, want) in [
        ("continue to step five", "continue to step 5"),
        ("train it on part two", "train it on part 2"),
        ("read chapter four", "read chapter 4"),
        ("turn to page twelve", "turn to page 12"),
        ("season two episode three", "season 2 episode 3"),
        ("phase one is done", "phase 1 is done"),
        ("at level nine", "at level 9"),
        ("install version two point one", "install version 2.1"),
        ("for the most part", "for the most part"),
        ("step by step", "step by step"),
        ("the first page", "the first page"),
    ] {
        let mut got = String::new();
        engine.normalize_into(input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}

#[test]
fn a_window_cut_keeps_the_word_before_a_number() {
    let engine = engine();
    for (input, want) in [
        ("a b c d e f at five and see", "a b c d e f at 5 and see"),
        (
            "a b c d e f room five and see",
            "a b c d e f room 5 and see",
        ),
        (
            "I need you to look at track two and track three and see",
            "I need you to look at track 2 and track 3 and see",
        ),
        ("a b c d e five pm and see", "a b c d e 5 pm and see"),
    ] {
        let mut got = String::new();
        engine.normalize_into(input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}

#[test]
fn a_relative_time_stays_as_spoken() {
    let ap = engine();
    let digits = Normalizer::english();
    let same = [
        "it is ten to four",
        "meet me at twenty five to six.",
        "it's ten past four",
        "ten after four",
        "it's half past four",
        "at a quarter to five, we left",
        "it is five to four",
    ];
    let ranges = [
        ("pages ten to twenty", "pages 10 to 20"),
        ("from ten to twelve people", "from 10 to 12 people"),
    ];
    for e in [&ap, &digits] {
        for input in same {
            let mut got = String::new();
            e.normalize_into(input, &mut got);
            assert_eq!(got, input, "{e:?}");
        }
        for (input, want) in ranges {
            let mut got = String::new();
            e.normalize_into(input, &mut got);
            assert_eq!(got, want, "{e:?}");
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
        engine.normalize_into(input, &mut got);
        assert_eq!(got, want, "{input:?}");
    }
}

fn check_lines(engine: &Normalizer, cases: &[(&str, &str)]) {
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|&(input, want)| {
            let got = line(engine, input);
            (got != want).then(|| format!("{input:?}\n   got  {got:?}\n   want {want:?}"))
        })
        .collect();
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn digit_sequences_keep_every_digit() {
    check_lines(
        &Normalizer::english(),
        &[
            ("my pin is zero zero one two", "my pin is 0012"),
            ("flight four seventeen is delayed", "flight 417 is delayed"),
            ("the port is eight zero eight zero", "the port is 8080"),
            ("they live in nine oh two one oh", "they live in 90210"),
            (
                "my number is eight oh oh five five five oh one nine nine",
                "my number is 8005550199",
            ),
            (
                "call one eight hundred two five five four six eight eight",
                "call 18002554688",
            ),
            ("agent double oh seven reporting", "agent 007 reporting"),
            (
                "the order number is triple five two one",
                "the order number is 55521",
            ),
            (
                "the ip address is one ninety two dot one sixty eight dot one dot one",
                "the ip address is 192.168.1.1",
            ),
            (
                "in an emergency dial nine one one",
                "in an emergency dial 911",
            ),
            ("a double espresso", "a double espresso"),
            ("click the dot", "click the dot"),
        ],
    );
}

#[test]
fn numbers_the_tables_used_to_miss() {
    check_lines(
        &Normalizer::english(),
        &[
            (
                "the tank holds a hundred and twenty liters",
                "the tank holds 120 liters",
            ),
            ("a hundred people came", "a hundred people came"),
            (
                "pi is roughly three point one four one five nine",
                "pi is roughly 3.14159",
            ),
            (
                "we migrated to python three point twelve",
                "we migrated to python 3.12",
            ),
            (
                "the crate requires rust one point eighty two",
                "the crate requires rust 1.82",
            ),
            (
                "the recession of twenty oh eight hurt everyone",
                "the recession of 2008 hurt everyone",
            ),
            (
                "the system launched on january first two thousand",
                "the system launched on January 1st, 2000",
            ),
            (
                "she was born on february twenty ninth twenty twenty",
                "she was born on February 29th, 2020",
            ),
            ("i am one hundred percent sure", "i am 100% sure"),
            (
                "he is six feet two inches tall",
                "he is 6 feet 2 inches tall",
            ),
            ("the vat is twelve per cent", "the vat is 12%"),
            (
                "we watch fireworks on the fourth of july",
                "we watch fireworks on the 4th of July",
            ),
            (
                "the wall fell on the ninth of november nineteen eighty nine",
                "the wall fell on the 9th of November 1989",
            ),
            (
                "the office is at fourteen fifty five market street",
                "the office is at 1455 market street",
            ),
        ],
    );
}

#[test]
fn may_and_march_as_verbs_stay_lowercase_words() {
    check_lines(
        &Normalizer::english(),
        &[
            ("we march five miles a day", "we march 5 miles a day"),
            ("they march ten miles", "they march 10 miles"),
            ("it may first be noticed", "it may first be noticed"),
            ("you may one day understand", "you may one day understand"),
            ("It may first seem odd.", "It may first seem odd."),
            ("the race is may fifth", "the race is May 5th"),
            ("on march third", "on March 3rd"),
        ],
    );
}

#[test]
fn hyphenated_number_words_read_as_numbers() {
    check_lines(
        &Normalizer::english(),
        &[
            ("Is it twenty-five or thirty?", "Is it 25 or 30?"),
            ("I paid twenty-five dollars.", "I paid $25."),
            (
                "The flight is at six forty-five a.m.",
                "The flight is at 6:45 a.m.",
            ),
        ],
    );
}

#[test]
fn spoken_money_forms() {
    check_lines(
        &Normalizer::english(),
        &[
            ("give me a hundred dollars", "give me $100"),
            ("it costs a thousand dollars", "it costs $1,000"),
            ("coffee is a dollar fifty", "coffee is $1.50"),
            (
                "the total is ten dollars and five cents",
                "the total is $10.05",
            ),
        ],
    );
}

#[test]
fn decades_digits_and_fillers() {
    check_lines(
        &Normalizer::english(),
        &[
            ("meet at 5 thirty pm", "meet at 5:30 pm"),
            ("houses from the eighteen hundreds", "houses from the 1800s"),
            ("in the eighteen hundreds", "in the 1800s"),
            (
                "it's like twenty uh twenty five people",
                "it's like twenty uh 25 people",
            ),
            ("One, two, three, go.", "One, two, three, go."),
        ],
    );
}

#[test]
fn its_before_a_clock_reading_is_a_time() {
    check_lines(
        &Normalizer::english(),
        &[
            ("it's five thirty", "it's 5:30"),
            ("it's 5 thirty", "it's 5:30"),
            ("it's eleven forty five", "it's 11:45"),
            ("it's twenty five dollars", "it's $25"),
            ("it's twenty five", "it's 25"),
            ("it's ten to four", "it's ten to four"),
        ],
    );
}

#[test]
fn a_digit_before_punctuation_and_and_keeps_its_value() {
    check_lines(
        &Normalizer::english(),
        &[
            ("4, and", "4, and"),
            ("4; and", "4; and"),
            ("4. And", "4. And"),
            ("picks 1 and 2, and then", "picks 1 and 2, and then"),
            ("sonnet 4, and probably", "sonnet 4, and probably"),
        ],
    );
}

#[test]
fn an_impossible_time_falls_back_to_the_plain_reading() {
    check_lines(
        &Normalizer::english(),
        &[
            ("it was nineteen ninety nine", "it was 1999"),
            ("it is nineteen ninety nine", "it is 1999"),
            ("at nineteen ninety nine", "at 1999"),
            ("it was twelve sixty", "it was 1260"),
            ("it was one ninety nine", "it was one ninety nine"),
            ("it is nine sixty nine", "it is nine sixty nine"),
            ("it's one ninety nine", "it's one ninety nine"),
            ("it's nineteen ninety nine", "it's 1999"),
            ("it was nine sixty nine pm", "it was 9:69 pm"),
            ("at twelve sixty pm", "at 12:60 pm"),
            ("at nine sixty nine pm", "at 9:69 pm"),
            ("call at eight seventy five", "call at 8:75"),
            ("at nine fifty nine", "at 9:59"),
            ("at eleven forty five pm", "at 11:45 pm"),
        ],
    );
}

#[test]
fn it_was_before_a_clock_reading_is_a_time_but_was_alone_is_not() {
    check_lines(
        &Normalizer::english(),
        &[
            ("it was five thirty", "it was 5:30"),
            ("it was 5 thirty", "it was 5:30"),
            ("It was eleven forty five.", "It was 11:45."),
            ("it was twenty five dollars", "it was $25"),
            ("it was five", "it was 5"),
            ("it was ten to four", "it was ten to four"),
            ("the score was five thirty", "the score was five thirty"),
        ],
    );
}

#[test]
fn it_is_before_a_clock_reading_is_a_time_but_is_alone_is_not() {
    check_lines(
        &Normalizer::english(),
        &[
            ("it is five thirty", "it is 5:30"),
            ("it is 5 thirty", "it is 5:30"),
            ("It is eleven forty five.", "It is 11:45."),
            (
                "we all waited outside the store until it is eleven forty five",
                "we all waited outside the store until it is 11:45",
            ),
            ("it is twenty five", "it is 25"),
            ("it is ten to four", "it is ten to four"),
            ("the score is five thirty", "the score is five thirty"),
            ("is five thirty ok", "is five thirty ok"),
            ("this is it five thirty", "this is it five thirty"),
            ("what is twenty five", "what is 25"),
        ],
    );
}

#[test]
fn a_window_edge_before_a_clock_reading_keeps_its_cue() {
    check_lines(
        &Normalizer::english(),
        &[
            ("a b c d e f it is five thirty", "a b c d e f it is 5:30"),
            ("a b c d e f g at five thirty", "a b c d e f g at 5:30"),
        ],
    );
}

#[test]
fn hundred_and_years_take_no_comma() {
    check_lines(
        &Normalizer::english(),
        &[
            ("in nineteen hundred and five", "in 1905"),
            (
                "built in eighteen hundred and twelve, then",
                "built in 1812, then",
            ),
            ("fifteen hundred and fifty people", "1,550 people"),
        ],
    );
}

#[test]
fn a_rewrite_kept_as_words_prints_as_spoken() {
    check_lines(
        &Normalizer::english(),
        &[
            ("a hundred and so on", "a hundred and so on"),
            (
                "there were a hundred and then more",
                "there were a hundred and then more",
            ),
            ("say double oh", "say double oh"),
        ],
    );
}

#[test]
fn idioms_and_fractions_stay_words() {
    let engine = Normalizer::english();
    for input in [
        "they finished at the eleventh hour",
        "it was a one off event",
        "she has twenty twenty vision",
        "drill a five eighths inch hole",
        "use a seven sixteenths wrench",
        "we hiked two and three quarters miles",
    ] {
        assert_eq!(line(&engine, input), input, "{input:?}");
    }
}

#[test]
fn counts_of_a_thousand_or_more_get_commas() {
    check_lines(
        &Normalizer::english(),
        &[
            (
                "the file has one thousand lines",
                "the file has 1,000 lines",
            ),
            ("fifteen hundred people showed up", "1,500 people showed up"),
            ("it dates to nineteen hundred", "it dates to 1900"),
            (
                "built in nineteen hundred, then sold",
                "built in 1900, then sold",
            ),
            ("report at seventeen hundred hours", "report at 1700 hours"),
            ("it was two thousand five hundred", "it was 2,500"),
            ("two thousand five hundred people", "2,500 people"),
            (
                "the total was nine million eight hundred seventy six thousand five hundred forty three",
                "the total was 9,876,543",
            ),
            (
                "the house sold for four hundred fifty thousand dollars",
                "the house sold for $450,000",
            ),
            ("the rent is twelve hundred dollars", "the rent is $1,200"),
            ("lunch was twenty thousand yen", "lunch was 20,000 yen"),
            ("it costs two thousand five dollars", "it costs $2,005"),
            ("in two thousand five", "in 2005"),
            ("born in nineteen ninety", "born in 1990"),
            ("room four oh two one", "room 4021"),
            ("at eleven forty five pm", "at 11:45 pm"),
            ("three point one four", "3.14"),
            ("twelve million dollars", "$12 million"),
        ],
    );
}

#[test]
fn a_letter_next_to_a_number_joins_it() {
    check_lines(
        &Normalizer::english(),
        &[
            ("i bought a four k monitor", "i bought a 4k monitor"),
            ("she lives in apartment four b", "she lives in apartment 4b"),
            ("take i sixty four east", "take i64 east"),
            ("from gate b twelve", "from gate b12"),
            ("five a day", "5 a day"),
            ("call a b c now", "call a b c now"),
        ],
    );
}

#[test]
fn each_setting_switches_off_only_its_own_rewrite() {
    use Setting::*;
    let all = Normalizer::english();
    let cases = [
        (
            SingleDigits,
            "like eight years",
            "like 8 years",
            "like eight years",
        ),
        (Percent, "fifty percent off", "50% off", "50 percent off"),
        (
            Currency,
            "it costs five dollars",
            "it costs $5",
            "it costs 5 dollars",
        ),
        (
            Currency,
            "four dollars and fifty cents",
            "$4.50",
            "4 dollars and 50 cents",
        ),
        (
            Dates,
            "born on july fourth nineteen ninety",
            "born on July 4th, 1990",
            "born on july 4th 1990",
        ),
        (
            Dates,
            "in july two thousand twelve",
            "in July 2012",
            "in july 2012",
        ),
        (
            Letters,
            "at seven a m e s t",
            "at 7 am EST",
            "at 7 a m e s t",
        ),
        (
            Letters,
            "the meeting is at three p m",
            "the meeting is at 3 pm",
            "the meeting is at 3 p m",
        ),
        (
            Noon,
            "the deadline is twelve noon",
            "the deadline is noon",
            "the deadline is 12 noon",
        ),
        (Oclock, "at five oclock", "at 5 o'clock", "at 5 oclock"),
        (
            Temperature,
            "seventy two degrees fahrenheit",
            "72 degrees Fahrenheit",
            "72 degrees fahrenheit",
        ),
    ];
    for (setting, input, on, off) in cases {
        assert_eq!(line(&all, input), on, "{setting:?} on");
        assert_eq!(line(&all.without(setting), input), off, "{setting:?} off");
    }
}

#[test]
fn whitespace_prints_as_written() {
    check_lines(
        &engine(),
        &[
            ("", ""),
            ("   ", "   "),
            ("  hello   world \u{a0}", "  hello   world \u{a0}"),
            ("  twenty five  ", "  25  "),
            (
                "it costs twenty dollars\nsee you at ten thirty",
                "it costs $20\nsee you at 10:30",
            ),
            ("twenty one\r\ntwenty two\r\n", "21\r\n22\r\n"),
            ("it  costs  twenty five  dollars", "it  costs  $25"),
            ("at  ten thirty  pm", "at  10:30  pm"),
            ("twenty\u{a0}five", "25"),
            ("\n\nforty two\n\n", "\n\n42\n\n"),
        ],
    );
}

#[test]
fn line_breaks_and_tabs_end_a_number() {
    check_lines(
        &engine(),
        &[
            ("twenty\nfive", "20\nfive"),
            ("twenty\tfive", "20\tfive"),
            ("room\nfour oh two", "room\n402"),
            ("twenty\u{85}five", "20\u{85}five"),
            ("twenty\u{2028}five", "20\u{2028}five"),
            ("twenty\u{2029}\u{2029}five", "20\u{2029}\u{2029}five"),
            ("twenty five\u{2028}", "25\u{2028}"),
            ("it\u{2019}s twenty \u{a0}five", "it\u{2019}s 25"),
        ],
    );
}

#[test]
fn numbers_longer_than_a_window_read_whole() {
    check_lines(
        &engine(),
        &[
            (
                "nine hundred ninety nine trillion nine hundred ninety nine billion nine hundred ninety nine million nine hundred ninety nine thousand nine hundred ninety nine",
                "999,999,999,999,999",
            ),
            (
                "twenty five thousand three hundred and forty two dollars and sixteen cents",
                "$25,342 and 16 cents",
            ),
            (
                "one hundred twenty three thousand four hundred fifty six point seven eight",
                "123,456.78",
            ),
            (
                "one hundred twenty three million four hundred fifty six thousand dollars",
                "$123,456,000",
            ),
            (
                "two hundred twenty five thousand three hundred and forty second",
                "225,342nd",
            ),
            (
                "it weighs two hundred thirty four thousand five hundred sixty seven kilograms",
                "it weighs 234,567 kilograms",
            ),
            (
                "one hundred and twenty one thousand three hundred and forty five people",
                "121,345 people",
            ),
            ("five trillion dollars", "$5 trillion"),
            ("two trillion five hundred billion", "2,500,000,000,000"),
            ("one quadrillion", "1 quadrillion"),
        ],
    );
}

const ONES: [&str; 20] = [
    "",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const SCALES: [&str; 6] = [
    "",
    "thousand",
    "million",
    "billion",
    "trillion",
    "quadrillion",
];

/// `n` in words; `and` says "hundred and five" and "thousand and five".
fn spell(n: u64, and: bool) -> Vec<&'static str> {
    let mut groups = Vec::new();
    let mut rest = n;
    while rest > 0 {
        groups.push(rest % 1000);
        rest /= 1000;
    }
    let mut words = Vec::new();
    for (k, &g) in groups.iter().enumerate().rev() {
        if g == 0 {
            continue;
        }
        if and && k == 0 && g < 100 && !words.is_empty() {
            words.push("and");
        }
        if g >= 100 {
            words.extend([ONES[(g / 100) as usize], "hundred"]);
            if and && g % 100 != 0 {
                words.push("and");
            }
        }
        match g % 100 {
            0 => {}
            r if r < 20 => words.push(ONES[r as usize]),
            r => {
                words.push(TENS[(r / 10) as usize]);
                if r % 10 != 0 {
                    words.push(ONES[(r % 10) as usize]);
                }
            }
        }
        if k > 0 {
            words.push(SCALES[k]);
        }
    }
    words
}

fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[test]
fn spelled_numbers_of_any_length_read_back_exactly() {
    let engine = engine();
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut failures = Vec::new();
    for case in 0..20_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let digits = 5 + (state % 14) as u32;
        let n = 10u64.pow(digits - 1) + (state >> 8) % (9 * 10u64.pow(digits - 1));
        let words = spell(n, case % 2 == 1).join(" ");
        let (input, want) = match case % 4 {
            0 => (words.clone(), grouped(n)),
            1 => (format!("{words} dollars"), format!("${}", grouped(n))),
            2 => (
                format!("{words} people came"),
                format!("{} people came", grouped(n)),
            ),
            _ => (format!("{words} point five"), format!("{}.5", grouped(n))),
        };
        let got = engine.normalize(&input);
        if got != want && failures.len() < 10 {
            failures.push(format!("{input:?}\n   got  {got:?}\n   want {want:?}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn letter_and_dot_joins_ignore_wide_gaps() {
    check_lines(
        &Normalizer::english(),
        &[
            ("four  k", "4k"),
            ("four\u{a0}k", "4k"),
            ("gate  b twelve", "gate  b12"),
            (
                "one nine two  dot one six eight dot one  dot one",
                "192.168.1.1",
            ),
            ("  four k  ", "  4k  "),
        ],
    );
}
