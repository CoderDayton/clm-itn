use crate::spell::*;

#[test]
fn numbers_are_spelled_the_way_people_say_them() {
    for (n, want) in [
        (0, "zero"),
        (7, "seven"),
        (15, "fifteen"),
        (21, "twenty one"),
        (100, "one hundred"),
        (554, "five hundred fifty four"),
        (2500, "two thousand five hundred"),
        (3000, "three thousand"),
        (300_000, "three hundred thousand"),
        (
            1_593_000_000,
            "one billion five hundred ninety three million",
        ),
    ] {
        assert_eq!(spell_number(n), want);
    }
}

#[test]
fn lines_keep_their_punctuation_and_words() {
    for (written, want) in [
        ("I have 15 apples.", Some("I have fifteen apples.")),
        (
            "It took 2.5 seconds, then 3.",
            Some("It took two point five seconds, then three."),
        ),
        ("50% off!", Some("fifty percent off!")),
        ("only 3,000 steps", Some("only three thousand steps")),
        ("(image 17)", Some("(image seventeen)")),
        ("nothing to spell here", Some("nothing to spell here")),
        (
            "It costs $250,000.",
            Some("It costs two hundred fifty thousand dollars."),
        ),
        ("just $1 more", Some("just one dollar more")),
        ("in 1990", None),
        ("the V7 model", None),
        ("the 1st one", None),
        (
            "limit is 2000 characters",
            Some("limit is two thousand characters"),
        ),
    ] {
        assert_eq!(spell_line(written).as_deref(), want, "{written}");
    }
}
