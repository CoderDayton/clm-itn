use super::language::{
    ClassDef, Classes, Example, Language, LiteralClassDef, Literals, NumberStyle, SRC_CARDINAL,
    SRC_IDENTIFIER, VocabSpec, WordKind,
};
use crate::hash::next_seed;
use crate::types::{
    CLASS_UNKNOWN, ClassId, DOMAIN_CENTS, DOMAIN_MEASURE, DOMAIN_MONEY, DOMAIN_PERCENT, DomainId,
    MAX_TOKENS, ROLE_AND, ROLE_CURRENCY, ROLE_DIGIT, ROLE_HUNDRED, ROLE_MINUS, ROLE_MSCALE,
    ROLE_OH, ROLE_POINT, ROLE_SCALE, ROLE_TEEN, ROLE_TENS, ROLE_UNIT, Setting, kind, name, sense,
};

pub const DIGIT: ClassId = 2;
pub const ZERO: ClassId = 3;
pub const TEEN: ClassId = 4;
pub const TENS: ClassId = 5;
pub const HUNDRED: ClassId = 6;
pub const SCALE: ClassId = 7;
pub const MSCALE: ClassId = 8;
pub const POINT: ClassId = 9;
pub const AND: ClassId = 10;
pub const MINUS: ClassId = 11;
pub const DOLLAR: ClassId = 12;
pub const CENT: ClassId = 13;
pub const PERCENT: ClassId = 14;
pub const UNIT: ClassId = 15;
pub const NUMBER: ClassId = 16;
pub const POWER: ClassId = 17;
pub const PREFIX: ClassId = 18;
pub const DEGREE: ClassId = 19;
pub const MONTH: ClassId = 20;
pub const MAY: ClassId = 21;
pub const DOT: ClassId = 22;
pub const AT: ClassId = 23;
pub const PER: ClassId = 24;
pub const TO: ClassId = 25;
pub const THE: ClassId = 26;
pub const IS: ClassId = 27;

pub const DOMAIN_MONTH_DATE: DomainId = 13;
pub const DOMAIN_MAY_DATE: DomainId = 14;

const SUFFIX_ROLES: u16 = (1 << ROLE_CURRENCY) | (1 << ROLE_UNIT);
const PREFIX_ROLES: u16 = 1 << ROLE_MINUS;
const BIND_ROLES: u16 = (1 << ROLE_HUNDRED) | (1 << ROLE_SCALE) | (1 << ROLE_MSCALE);

macro_rules! as_spoken {
    ($($w:literal),* $(,)?) => {
        &[$(($w, $w)),*]
    };
}

const UNIT_WORDS: &[(&str, &str)] = as_spoken![
    "meter",
    "kilometer",
    "zentimeter",
    "millimeter",
    "gramm",
    "kilogramm",
    "kilo",
    "milligramm",
    "tonne",
    "tonnen",
    "liter",
    "milliliter",
    "hektar",
    "meile",
    "meilen",
    "zoll",
    "quadratmeter",
    "kubikmeter",
    "kubikzentimeter",
    "watt",
    "kilowatt",
    "megawatt",
    "kilowattstunde",
    "kilowattstunden",
    "volt",
    "millivolt",
    "byte",
    "bytes",
    "kilobyte",
    "megabyte",
    "megabytes",
    "gigabyte",
    "gigabytes",
    "terabyte",
    "kalorien",
    "grad",
    "stunde",
    "stunden",
    "minute",
    "minuten",
    "sekunden",
    "tage",
    "wochen",
    "monate",
    "jahre",
];

pub const LITERAL_CLASSES: &[LiteralClassDef] = &[
    LiteralClassDef {
        class: DOLLAR,
        role: ROLE_CURRENCY,
        words: &[
            ("euro", "€"),
            ("euros", "€"),
            ("dollar", "$"),
            ("us-dollar", "US$"),
            ("franken", "CHF"),
            ("yen", "¥"),
        ],
    },
    LiteralClassDef {
        class: CENT,
        role: ROLE_CURRENCY,
        words: &[("cent", "€"), ("cents", "€"), ("rappen", "CHF")],
    },
    LiteralClassDef {
        class: MINUS,
        role: ROLE_MINUS,
        words: &[("minus", "-")],
    },
    LiteralClassDef {
        class: PERCENT,
        role: ROLE_UNIT,
        words: &[("prozent", " %")],
    },
    LiteralClassDef {
        class: UNIT,
        role: ROLE_UNIT,
        words: UNIT_WORDS,
    },
];

fn affix_domain(word: &str) -> Option<DomainId> {
    let class = LITERAL_CLASSES
        .iter()
        .find(|c| c.words.iter().any(|w| w.0 == word))?
        .class;
    match class {
        DOLLAR => Some(DOMAIN_MONEY),
        CENT => Some(DOMAIN_CENTS),
        PERCENT => Some(DOMAIN_PERCENT),
        UNIT => Some(DOMAIN_MEASURE),
        _ => None,
    }
}

pub const CLASSES: &[ClassDef] = &[
    ClassDef {
        class: DIGIT,
        role: ROLE_DIGIT,
        words: &[
            ("eins", 1),
            ("ein", 1),
            ("eine", 1),
            ("zwei", 2),
            ("zwo", 2),
            ("drei", 3),
            ("vier", 4),
            ("fünf", 5),
            ("sechs", 6),
            ("sieben", 7),
            ("acht", 8),
            ("neun", 9),
        ],
    },
    ClassDef {
        class: ZERO,
        role: ROLE_OH,
        words: &[("null", 0)],
    },
    ClassDef {
        class: TEEN,
        role: ROLE_TEEN,
        words: &[
            ("zehn", 10),
            ("elf", 11),
            ("zwölf", 12),
            ("dreizehn", 13),
            ("vierzehn", 14),
            ("fünfzehn", 15),
            ("sechzehn", 16),
            ("siebzehn", 17),
            ("achtzehn", 18),
            ("neunzehn", 19),
            ("einundzwanzig", 21),
            ("zweiundzwanzig", 22),
            ("dreiundzwanzig", 23),
            ("vierundzwanzig", 24),
            ("fünfundzwanzig", 25),
            ("sechsundzwanzig", 26),
            ("siebenzwanzig", 27),
            ("achtundzwanzig", 28),
            ("neunundzwanzig", 29),
            ("einunddreißig", 31),
            ("zweiunddreißig", 32),
            ("dreiunddreißig", 33),
            ("vierunddreißig", 34),
            ("fünfdreißig", 35),
            ("sechsdreißig", 36),
            ("siebendreißig", 37),
            ("achtunddreißig", 38),
            ("neununddreißig", 39),
            ("einundvierzig", 41),
            ("zweiundvierzig", 42),
            ("dreiundvierzig", 43),
            ("vierundvierzig", 44),
            ("fünfundvierzig", 45),
            ("sechsundvierzig", 46),
            ("siebenvierzig", 47),
            ("achtundvierzig", 48),
            ("neunundvierzig", 49),
            ("einundfünfzig", 51),
            ("zweiundfünfzig", 52),
            ("dreiundfünfzig", 53),
            ("vierundfünfzig", 54),
            ("fünffünfzig", 55),
            ("sechsfünfzig", 56),
            ("siebenfünfzig", 57),
            ("achtundfünfzig", 58),
            ("neunundfünfzig", 59),
            ("einundsechzig", 61),
            ("zweiundsechzig", 62),
            ("dreiundsechzig", 63),
            ("vierundsechzig", 64),
            ("fünfundsechzig", 65),
            ("sechsundsechzig", 66),
            ("siebensechzig", 67),
            ("achtundsechzig", 68),
            ("neunundsechzig", 69),
            ("einundsiebzig", 71),
            ("zweiundsiebzig", 72),
            ("dreiundsiebzig", 73),
            ("vierundsiebzig", 74),
            ("fünfundsiebzig", 75),
            ("sechsundsiebzig", 76),
            ("siebensiebzig", 77),
            ("achtundsiebzig", 78),
            ("neunundsiebzig", 79),
            ("einundachtzig", 81),
            ("zweiundachtzig", 82),
            ("dreiundachtzig", 83),
            ("vierundachtzig", 84),
            ("fünfundachtzig", 85),
            ("sechsundachtzig", 86),
            ("siebenachtzig", 87),
            ("achtundachtzig", 88),
            ("neunundachtzig", 89),
            ("einundneunzig", 91),
            ("zweiundneunzig", 92),
            ("dreiundneunzig", 93),
            ("vierundneunzig", 94),
            ("fünfundneunzig", 95),
            ("sechsundneunzig", 96),
            ("siebenneunzig", 97),
            ("achtundneunzig", 98),
            ("neunundneunzig", 99),
        ],
    },
    ClassDef {
        class: TENS,
        role: ROLE_TENS,
        words: &[
            ("zwanzig", 20),
            ("dreißig", 30),
            ("dreissig", 30),
            ("vierzig", 40),
            ("fünfzig", 50),
            ("sechzig", 60),
            ("siebzig", 70),
            ("achtzig", 80),
            ("neunzig", 90),
        ],
    },
    ClassDef {
        class: HUNDRED,
        role: ROLE_HUNDRED,
        words: &[
            ("hundert", 100),
            ("einhundert", 100),
            ("zweihundert", 200),
            ("zwohundert", 200),
            ("dreihundert", 300),
            ("vierhundert", 400),
            ("fünfhundert", 500),
            ("sechshundert", 600),
            ("siebenhundert", 700),
            ("achthundert", 800),
            ("neunhundert", 900),
        ],
    },
    ClassDef {
        class: SCALE,
        role: ROLE_SCALE,
        words: &[("tausend", 1_000)],
    },
    ClassDef {
        class: MSCALE,
        role: ROLE_MSCALE,
        words: &[
            ("million", 1_000_000),
            ("millionen", 1_000_000),
            ("milliarde", 1_000_000_000),
            ("milliarden", 1_000_000_000),
        ],
    },
    ClassDef {
        class: POINT,
        role: ROLE_POINT,
        words: &[("komma", 0)],
    },
    ClassDef {
        class: AND,
        role: ROLE_AND,
        words: &[("und", 0)],
    },
    ClassDef {
        class: AT,
        role: 0,
        words: &[("um", 0)],
    },
    ClassDef {
        class: TO,
        role: 0,
        words: &[("bis", 0)],
    },
    ClassDef {
        class: THE,
        role: 0,
        words: &[("die", 0)],
    },
    ClassDef {
        class: IS,
        role: 0,
        words: &[("ist", 0)],
    },
    ClassDef {
        class: NUMBER,
        role: 0,
        words: &[("nummer", 0)],
    },
];

fn spoken(n: u32, roles: &[u8]) -> &'static str {
    CLASSES
        .iter()
        .filter(|c| roles.contains(&c.role))
        .flat_map(|c| c.words.iter())
        .find(|w| w.1 == n)
        .map_or("", |w| w.0)
}

fn below_100(n: u32) -> &'static str {
    spoken(n, &[ROLE_OH, ROLE_DIGIT, ROLE_TEEN, ROLE_TENS])
}

fn under_1000(n: u32, out: &mut Vec<&'static str>) {
    let (h, r) = (n / 100, n % 100);
    if h > 0 {
        out.push(spoken(h * 100, &[ROLE_HUNDRED]));
    }
    if r > 0 {
        out.push(below_100(r));
    }
}

pub fn cardinal(n: u64) -> Vec<&'static str> {
    if n == 0 {
        return vec!["null"];
    }
    const GROUPS: [(u64, &str, &str); 2] = [
        (1_000_000_000, "milliarde", "milliarden"),
        (1_000_000, "million", "millionen"),
    ];
    let mut out = Vec::new();
    for (size, one, many) in GROUPS {
        match ((n / size) % 1000) as u32 {
            0 => {}
            1 => out.extend(["eine", one]),
            g => {
                under_1000(g, &mut out);
                out.push(many);
            }
        }
    }
    match ((n / 1000) % 1000) as u32 {
        0 => {}
        1 => out.push("tausend"),
        k => {
            under_1000(k, &mut out);
            out.push("tausend");
        }
    }
    under_1000((n % 1000) as u32, &mut out);
    out
}

fn role_of(word: &str) -> u8 {
    let number = CLASSES
        .iter()
        .find(|c| c.words.iter().any(|x| x.0 == word))
        .map(|c| c.role);
    let literal = || {
        LITERAL_CLASSES
            .iter()
            .find(|c| c.words.iter().any(|x| x.0 == word))
            .map(|c| c.role)
    };
    number.or_else(literal).unwrap_or(0)
}

fn windows(words: &[&'static str]) -> Vec<Vec<&'static str>> {
    let starts = |w: &str| {
        matches!(
            role_of(w),
            ROLE_DIGIT | ROLE_TEEN | ROLE_TENS | ROLE_OH | ROLE_HUNDRED
        )
    };
    let scale = |w: &str| matches!(role_of(w), ROLE_SCALE | ROLE_MSCALE);
    let mut out = Vec::new();
    let mut pos = 0;
    while words.len() - pos > MAX_TOKENS {
        let cut = (pos..words.len() - 1).find(|&j| scale(words[j]) && starts(words[j + 1]));
        match cut {
            Some(j) if j - pos < MAX_TOKENS => {
                out.push(words[pos..=j].to_vec());
                pos = j + 1;
            }
            _ => return Vec::new(),
        }
    }
    out.push(words[pos..].to_vec());
    out
}

const EXAMPLE_SEED: u64 = 0x4445_425F_4954_4E31;
const CARDINAL_SAMPLES: usize = 6000;
const DECIMAL_SAMPLES: usize = 3000;
const SCALED_SAMPLES: usize = 1000;
const MONEY_SAMPLES: usize = 3000;
const PERCENT_SAMPLES: usize = 1000;
const MEASURE_SAMPLES: usize = 1500;
const IDENTIFIER_SAMPLES: usize = 100;

const KIND_MASS: &[(&str, f64)] = &[
    ("cardinal", 1.0),
    ("decimal", 0.5),
    ("scaled", 0.25),
    ("money", 0.5),
    ("percent", 0.2),
    ("measure", 0.3),
    ("identifier", 0.3),
];

fn kind_mass(kind: &str) -> f64 {
    KIND_MASS.iter().find(|k| k.0 == kind).map_or(0.0, |k| k.1)
}

fn sample_number(state: &mut u64, max_digits: u32) -> u64 {
    let r = next_seed(state);
    let digits = 1 + (r % u64::from(max_digits)) as u32;
    let sig = match (r >> 16) % 10 {
        0..=2 => 1,
        3..=4 => 2,
        5..=6 => 3,
        _ => digits,
    }
    .min(digits);
    let low = 10u64.pow(sig - 1);
    let lead = low + next_seed(state) % (9 * low);
    lead * 10u64.pow(digits - sig)
}

fn pick<T: Copy>(r: u64, weighted: &[(T, u64)]) -> T {
    let total: u64 = weighted.iter().map(|w| w.1).sum();
    let mut x = r % total;
    for &(v, w) in weighted {
        if x < w {
            return v;
        }
        x -= w;
    }
    weighted[0].0
}

struct Sink {
    out: Vec<Example>,
    state: u64,
}

impl Sink {
    fn push(&mut self, words: Vec<&'static str>, src: u8, weight: f64, kind: &'static str) {
        for words in windows(&words) {
            self.out.push(Example {
                words,
                src,
                weight,
                kind,
            });
        }
    }

    fn rand(&mut self) -> u64 {
        next_seed(&mut self.state)
    }

    fn number(&mut self, max_digits: u32) -> u64 {
        sample_number(&mut self.state, max_digits)
    }

    fn fraction(&mut self, words: &mut Vec<&'static str>) {
        words.push("komma");
        let r = self.rand();
        if r % 10 < 6 {
            let len = pick(r >> 8, &[(1, 45), (2, 35), (3, 15), (4, 5), (5, 3), (6, 2)]);
            for _ in 0..len {
                words.push(below_100((self.rand() % 10) as u32));
            }
        } else {
            let n = self.number(2);
            words.extend(cardinal(n));
        }
    }

    fn amount(&mut self, max_digits: u32, decimal_share: u64) -> Vec<&'static str> {
        let r = self.rand();
        let mut words = cardinal(self.number(max_digits));
        if r % 100 < decimal_share {
            self.fraction(&mut words);
        }
        words
    }
}

fn cardinals(s: &mut Sink) {
    let w = 1.0 / CARDINAL_SAMPLES as f64;
    for _ in 0..CARDINAL_SAMPLES {
        let n = s.number(9);
        s.push(cardinal(n), SRC_CARDINAL, w, "cardinal");
    }
    let r = s.rand();
    for n in 1..1000u64 {
        let mut words = cardinal(n);
        if r.is_multiple_of(2) {
            words = [&["minus"][..], &words].concat();
        }
        s.push(words, SRC_CARDINAL, 0.2 / 999.0, "cardinal");
    }
    let mut seen = std::collections::HashSet::new();
    let mut groups = vec![0u64, 2];
    for g in 1..1000u64 {
        let roles: Vec<u8> = cardinal(g).iter().map(|w| role_of(w)).collect();
        if seen.insert(roles) {
            groups.push(g);
        }
    }
    let w = 0.3 / groups.len().pow(3) as f64;
    for &m in &groups {
        for &k in &groups {
            for &u in &groups {
                let n = m * 1_000_000 + k * 1000 + u;
                if n > 0 {
                    s.push(cardinal(n), SRC_CARDINAL, w, "cardinal");
                }
            }
        }
    }
    let w = 0.1 / 99.0;
    for u in 1..100u64 {
        for scale in [100, 100 * (2 + u % 8), 1000] {
            let mut words = cardinal(scale);
            words.extend(["und", below_100(u as u32)]);
            s.push(words, SRC_CARDINAL, w, "cardinal");
        }
    }
    let w = 0.1 / 900.0;
    for y in 1100..2000u32 {
        let mut words = vec![below_100(y / 100), "hundert"];
        if y % 100 > 0 {
            words.push(below_100(y % 100));
        }
        s.push(words, SRC_CARDINAL, w, "cardinal");
    }
}

fn decimals(s: &mut Sink) {
    let w = 1.0 / DECIMAL_SAMPLES as f64;
    for _ in 0..DECIMAL_SAMPLES {
        let r = s.rand();
        let mut words = if r % 10 < 2 {
            vec!["null"]
        } else {
            cardinal(s.number(4))
        };
        s.fraction(&mut words);
        s.push(words, SRC_CARDINAL, w, "decimal");
    }
    let w = 0.1 / 500.0;
    for len in 1..=6u32 {
        let wholes: &[u64] = match len {
            1..=4 => &[0, 3, 12, 60, 100, 245],
            _ => &[1, 45, 300],
        };
        for zeros in 0..1u32 << len {
            for &whole in wholes {
                let mut words = cardinal(whole);
                words.push("komma");
                words.extend((0..len).map(|d| match zeros >> d & 1 {
                    1 => "null",
                    _ => below_100(1 + (s.rand() % 9) as u32),
                }));
                s.push(
                    [&["minus"][..], &words].concat(),
                    SRC_CARDINAL,
                    w,
                    "decimal",
                );
                s.push(words, SRC_CARDINAL, w, "decimal");
            }
        }
    }
    let w = 1.0 / SCALED_SAMPLES as f64;
    for _ in 0..SCALED_SAMPLES {
        let r = s.rand();
        let mut words = match r % 10 {
            0 => vec!["null"],
            1..=2 => cardinal(1000 + s.number(3)),
            _ => cardinal(s.number(3)),
        };
        if words == ["null"] || s.rand().is_multiple_of(3) {
            s.fraction(&mut words);
        }
        let one = words == ["eins"];
        if one {
            words = vec!["eine"];
        }
        words.push(match (one, (r >> 20).is_multiple_of(4)) {
            (true, false) => "million",
            (false, false) => "millionen",
            (true, true) => "milliarde",
            (false, true) => "milliarden",
        });
        s.push(words, SRC_CARDINAL, w, "scaled");
    }
}

fn money(s: &mut Sink) {
    let w = 1.0 / MONEY_SAMPLES as f64;
    for _ in 0..MONEY_SAMPLES {
        let r = s.rand();
        let (unit, cent) = pick(
            r >> 40,
            &[
                (("euro", "cent"), 6),
                (("dollar", "cent"), 3),
                (("franken", "rappen"), 1),
            ],
        );
        let c = 1 + (r >> 8) % 99;
        let mut words = match r % 20 {
            0..=1 => {
                let mut words = cardinal(c);
                words.push(cent);
                s.push(words, SRC_CARDINAL, w, "money");
                continue;
            }
            2..=9 => cardinal(s.number(3)),
            _ => cardinal(s.number(7)),
        };
        words.push(unit);
        if (2..=9).contains(&(r % 20)) {
            if r % 20 <= 4 {
                words.push("und");
            }
            words.extend(cardinal(c));
            if r % 20 <= 6 {
                words.push(cent);
            }
        }
        s.push(words, SRC_CARDINAL, w, "money");
    }
}

fn percents(s: &mut Sink) {
    let w = 1.0 / PERCENT_SAMPLES as f64;
    for _ in 0..PERCENT_SAMPLES {
        let mut words = s.amount(4, 30);
        words.push("prozent");
        s.push(words, SRC_CARDINAL, w, "percent");
    }
}

fn measures(s: &mut Sink) {
    let w = 1.0 / MEASURE_SAMPLES as f64;
    for _ in 0..MEASURE_SAMPLES {
        let r = s.rand();
        let mut words = s.amount(6, 20);
        words.push(UNIT_WORDS[(r % UNIT_WORDS.len() as u64) as usize].0);
        s.push(words, SRC_CARDINAL, w, "measure");
    }
}

fn identifiers(s: &mut Sink) {
    let w = 1.0 / (6 * IDENTIFIER_SAMPLES) as f64;
    for len in 3..=8 {
        for _ in 0..IDENTIFIER_SAMPLES {
            let words = (0..len)
                .map(|_| below_100((s.rand() % 10) as u32))
                .collect();
            s.push(words, SRC_IDENTIFIER, w, "identifier");
        }
    }
}

fn examples() -> Vec<Example> {
    let mut s = Sink {
        out: Vec::new(),
        state: EXAMPLE_SEED,
    };
    cardinals(&mut s);
    decimals(&mut s);
    money(&mut s);
    percents(&mut s);
    measures(&mut s);
    identifiers(&mut s);
    s.out
}

const DIGIT_WORDS: &[&str] = &[
    "eins", "ein", "eine", "zwei", "zwo", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun",
];

const SENSES: &[(u32, &[&str])] = &[
    (sense::COUNT, DIGIT_WORDS),
    (sense::ONE, &["eins", "ein", "eine"]),
    (sense::AND, &["und"]),
    (sense::MILLION, &["million", "millionen"]),
    (sense::TWENTY, &["zwanzig"]),
    (sense::FIVE, &["fünf"]),
];

const IDIOMS: &[&[&str]] = &[&["auf", "einmal"], &["noch", "einmal"]];

const KINDS: &[WordKind] = &[
    WordKind {
        kind: kind::PER,
        aux: 0,
        words: &["pro"],
    },
    WordKind {
        kind: kind::FILLER,
        aux: 0,
        words: &["äh", "ähm", "öh", "hm", "hmm"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 3,
        words: &["tausend"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 6,
        words: &["million", "millionen"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 9,
        words: &["milliarde", "milliarden"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 12,
        words: &["billion", "billionen"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 15,
        words: &["billiarde", "billiarden"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 18,
        words: &["trillion", "trillionen"],
    },
];

const NAMES: [&str; name::COUNT] = [
    "eins",
    "zwei",
    "null",
    "tausend",
    "milliarde",
    "prozent",
    "pro zent",
    "zwanzig",
    "zwölf",
    "uhr",
    "und",
    "zent",
    "erste",
    "am",
    "punkt",
];

pub static LANGUAGE: Language = Language {
    name: "german",
    classes: CLASSES,
    literal_classes: LITERAL_CLASSES,
    class_ids: Classes {
        dollar: DOLLAR,
        cent: CENT,
        percent: PERCENT,
        unit: UNIT,
        power: POWER,
        prefix: PREFIX,
        degree: DEGREE,
        month: MONTH,
        may: MAY,
        dot: DOT,
        time_left: AT,
        time_right: &[],
        identifier_left: &[NUMBER],
    },
    kinds: KINDS,
    vocab: VocabSpec {
        names: NAMES,
        months: [
            "januar",
            "februar",
            "märz",
            "april",
            "mai",
            "juni",
            "juli",
            "august",
            "september",
            "oktober",
            "november",
            "dezember",
        ],
        hours: [
            "eins", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun", "zehn",
            "elf", "zwölf",
        ],
        centuries: [
            "1100s", "1200s", "1300s", "1400s", "1500s", "1600s", "1700s", "1800s", "1900s",
        ],
        zones: &["MEZ", "UTC", "GMT"],
        not_after_number: "aeiou",
        not_before_number: "aeiou",
    },
    literals: Literals {
        oclock: "",
        may: "",
    },
    numbers: NumberStyle {
        decimal: ",",
        grouped: false,
        pairs: false,
        fraction_number: true,
        currency_after: true,
    },
    senses: SENSES,
    idioms: IDIOMS,
    plural: &[],
    words_first: DIGIT_WORDS,
    suffix_roles: SUFFIX_ROLES,
    prefix_roles: PREFIX_ROLES,
    bind_roles: BIND_ROLES,
    lead_skip_roles: 1 << ROLE_UNIT,
    left_folds: [[IS, THE, AT], [TO, THE, AT]],
    aliases: &[
        (TO, CLASS_UNKNOWN),
        (THE, CLASS_UNKNOWN),
        (IS, CLASS_UNKNOWN),
    ],
    start_classes: &[],
    per: PER,
    per_units: &[UNIT],
    pair_only: &[],
    lone_after: &[],
    setting_classes: &[
        (Setting::Percent, &[PERCENT]),
        (Setting::Currency, &[DOLLAR, CENT]),
    ],
    setting_domains: &[],
    month_domain: DOMAIN_MONTH_DATE,
    may_domain: DOMAIN_MAY_DATE,
    affix_domain,
    examples,
    kind_mass,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinal_words() {
        assert_eq!(cardinal(0), ["null"]);
        assert_eq!(cardinal(1), ["eins"]);
        assert_eq!(cardinal(45), ["fünfundvierzig"]);
        assert_eq!(cardinal(100), ["hundert"]);
        assert_eq!(cardinal(245), ["zweihundert", "fünfundvierzig"]);
        assert_eq!(cardinal(1001), ["tausend", "eins"]);
        assert_eq!(
            cardinal(245_312),
            [
                "zweihundert",
                "fünfundvierzig",
                "tausend",
                "dreihundert",
                "zwölf"
            ]
        );
        assert_eq!(cardinal(1_000_000), ["eine", "million"]);
        assert_eq!(cardinal(2_000_003), ["zwei", "millionen", "drei"]);
    }

    #[test]
    fn every_example_word_is_in_the_lexicon() {
        let words: std::collections::HashSet<&str> = CLASSES
            .iter()
            .flat_map(|c| c.words.iter().map(|w| w.0))
            .chain(
                LITERAL_CLASSES
                    .iter()
                    .flat_map(|c| c.words.iter().map(|w| w.0)),
            )
            .collect();
        for e in examples() {
            for w in e.words {
                assert!(words.contains(w), "{w}");
            }
        }
    }
}
