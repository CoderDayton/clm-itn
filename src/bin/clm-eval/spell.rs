use std::process::ExitCode;

const ONES: [&str; 20] = [
    "zero",
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
const SCALES: [(u64, &str); 3] = [
    (1_000_000_000, "billion"),
    (1_000_000, "million"),
    (1_000, "thousand"),
];

pub fn spell_number(n: u64) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut rest = n;
    for (size, name) in SCALES {
        if rest >= size {
            parts.push(format!("{} {name}", spell_number(rest / size)));
            rest %= size;
        }
    }
    if rest >= 100 {
        parts.push(format!("{} hundred", ONES[(rest / 100) as usize]));
        rest %= 100;
    }
    if rest >= 20 {
        let tens = TENS[(rest / 10) as usize];
        parts.push(match rest % 10 {
            0 => tens.to_string(),
            d => format!("{tens} {}", ONES[d as usize]),
        });
    } else if rest > 0 || parts.is_empty() {
        parts.push(ONES[rest as usize].to_string());
    }
    parts.join(" ")
}

fn grouped_digits(core: &str) -> Option<String> {
    let mut groups = core.split(',');
    let first = groups.next()?;
    if first.is_empty() || first.len() > 3 {
        return None;
    }
    let mut out = first.to_string();
    for g in groups {
        if g.len() != 3 {
            return None;
        }
        out.push_str(g);
    }
    Some(out)
}

fn spell_core(core: &str) -> Option<String> {
    let (body, percent) = match core.strip_suffix('%') {
        Some(b) => (b, true),
        None => (core, false),
    };
    let (int_part, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    let int_digits = if int_part.contains(',') {
        grouped_digits(int_part)?
    } else {
        int_part.to_string()
    };
    if int_digits.is_empty()
        || int_digits.len() > 9
        || !int_digits.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let value: u64 = int_digits.parse().ok()?;
    let year_like =
        !int_part.contains(',') && int_digits.len() == 4 && (1100..=2099).contains(&value);
    if year_like && value != 2000 && frac.is_none() {
        return None;
    }
    let mut out = spell_number(value);
    if let Some(f) = frac {
        if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        out.push_str(" point");
        for b in f.bytes() {
            out.push(' ');
            out.push_str(ONES[(b - b'0') as usize]);
        }
    }
    if percent {
        out.push_str(" percent");
    }
    Some(out)
}

pub fn spell_line(text: &str) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    for token in text.split(' ') {
        let start = token
            .find(|c: char| c.is_alphanumeric())
            .unwrap_or(token.len());
        let end = token
            .rfind(|c: char| c.is_alphanumeric() || c == '%')
            .map_or(start, |i| {
                i + token[i..].chars().next().map_or(1, char::len_utf8)
            });
        if start >= end || !token[start..end].bytes().any(|b| b.is_ascii_digit()) {
            out.push(token.to_string());
            continue;
        }
        let core = &token[start..end];
        let (lead, dollar) = match token[..start].strip_suffix('$') {
            Some(l) => (l, true),
            None => (&token[..start], false),
        };
        if dollar && core.contains(['.', '%']) {
            return None;
        }
        let mut spoken = spell_core(core)?;
        if dollar {
            spoken.push_str(if spoken == "one" {
                " dollar"
            } else {
                " dollars"
            });
        }
        out.push(format!("{lead}{spoken}{}", &token[end..]));
    }
    Some(out.join(" "))
}

pub fn run(path: &str) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (mut kept, mut skipped, mut plain) = (0usize, 0usize, 0usize);
    for line in text.lines() {
        let Some(written) = line.split('\t').nth(2) else {
            continue;
        };
        match spell_line(written) {
            Some(spoken) if spoken != written => {
                println!("{spoken}\t{written}\twhisper");
                kept += 1;
            }
            Some(_) => plain += 1,
            None => skipped += 1,
        }
    }
    eprintln!("spelled {kept}, no digits {plain}, skipped (years, ids, ordinals) {skipped}");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
