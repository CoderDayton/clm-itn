use super::language::{Classes, Language, Literals, NumberStyle, VocabSpec, WordKind};
use crate::hash::next_seed;
use crate::types::{
    CLASS_UNKNOWN, ClassId, DOMAIN_CENTS, DOMAIN_MEASURE, DOMAIN_MEASURE_POWER,
    DOMAIN_MEASURE_PREFIX, DOMAIN_MONEY, DOMAIN_PERCENT, DomainId, Role, Setting, kind, name,
    sense,
};

pub const DIGIT: ClassId = 2;
pub const OH: ClassId = 3;
pub const TEEN: ClassId = 4;
pub const TENS: ClassId = 5;
pub const HUNDRED: ClassId = 6;
pub const SCALE: ClassId = 7;
pub const POINT: ClassId = 8;
pub const AND: ClassId = 9;
pub const ORDINAL_UNIT: ClassId = 10;
pub const ORDINAL_TEEN: ClassId = 11;
pub const ORDINAL_TENS: ClassId = 12;
pub const AT: ClassId = 13;
pub const AM: ClassId = 14;
pub const PM: ClassId = 15;
pub const OCLOCK: ClassId = 16;
pub const NUMBER: ClassId = 17;
pub const ROOM: ClassId = 18;
pub const FLIGHT: ClassId = 19;
pub const CODE: ClassId = 20;
pub const PIN: ClassId = 21;
pub const EXTENSION: ClassId = 22;
pub const THE: ClassId = 23;
pub const MSCALE: ClassId = 24;
pub const MONTH: ClassId = 25;
pub const DOLLAR: ClassId = 26;
pub const CENT: ClassId = 27;
pub const PERCENT: ClassId = 28;
pub const LETTER_PM: ClassId = 30;
pub const UNIT: ClassId = 31;
pub const POWER: ClassId = 32;
pub const PREFIX: ClassId = 33;
pub const MAY: ClassId = 34;
pub const PER: ClassId = 35;
pub const DEGREE: ClassId = 36;
pub const TEMP: ClassId = 37;
/// A word that names a numbered item: "step five" -> "step 5".
pub const LABEL: ClassId = 38;
/// "dot" between numbers, as in an IP address: the numbers are identifiers.
pub const DOT: ClassId = 39;
/// "it", "is" and "was" exist only to fold "it is" and "it was" into
/// `SOFT_AT`; every other table read treats them as unknown words (see
/// `ALIASES`).
pub const IT: ClassId = 40;
pub const IS: ClassId = 41;
pub const WAS: ClassId = 42;
/// "it's", or "it is" and "it was" folded. Reads like `AT`, but an impossible
/// time after it is no time: "it was one ninety nine" stays words, where "at
/// nine sixty nine" is `9:69`.
pub const SOFT_AT: ClassId = 43;
/// "hours" after a clock reading: "fifteen thirty hours" is 24-hour time.
pub const HOURS: ClassId = 44;
pub const LEFT_FOLDS: [[ClassId; 3]; 2] = [[IT, IS, SOFT_AT], [IT, WAS, SOFT_AT]];
/// Classes whose context entries are copies of another class's.
pub const ALIASES: [(ClassId, ClassId); 4] = [
    (IT, CLASS_UNKNOWN),
    (IS, CLASS_UNKNOWN),
    (WAS, CLASS_UNKNOWN),
    (SOFT_AT, AT),
];
pub const PER_UNITS: [ClassId; 5] = [UNIT, POWER, PREFIX, DEGREE, TEMP];
pub const PAIR_ONLY: [ClassId; 2] = [DEGREE, TEMP];

pub const DOMAIN_MONTH_DATE: DomainId = 13;
pub const DOMAIN_MAY_DATE: DomainId = 14;
pub const START_CLASSES: [ClassId; 2] = [MONTH, MAY];

/// Word classes kept as spoken when their setting is off.
pub const SETTING_CLASSES: [(Setting, &[ClassId]); 5] = [
    (Setting::Percent, &[PERCENT]),
    (Setting::Currency, &[DOLLAR, CENT]),
    (Setting::Letters, &[LETTER_PM]),
    (Setting::Oclock, &[OCLOCK]),
    (Setting::Temperature, &[TEMP]),
];

/// Domains that use their rank-1 candidate when their setting is off.
pub const SETTING_DOMAINS: [(Setting, &[DomainId]); 1] =
    [(Setting::Dates, &[DOMAIN_MONTH_DATE, DOMAIN_MAY_DATE])];

pub use crate::types::{
    ROLE_AMPM, ROLE_AND, ROLE_CURRENCY, ROLE_DIGIT, ROLE_HUNDRED, ROLE_MER, ROLE_MINUS,
    ROLE_MSCALE, ROLE_OH, ROLE_ORDINAL, ROLE_POINT, ROLE_SCALE, ROLE_TEEN, ROLE_TENS, ROLE_UNIT,
};

pub const TIME_RIGHT: [ClassId; 4] = [AM, PM, OCLOCK, HOURS];
pub const TIME_LEFT: ClassId = AT;
pub const IDENTIFIER_LEFT: [ClassId; 7] = [NUMBER, ROOM, FLIGHT, CODE, PIN, EXTENSION, LABEL];

pub use super::language::{
    ClassDef, Example, LiteralClassDef, SRC_CARDINAL, SRC_DATE, SRC_IDENTIFIER, SRC_TIME, SRC_YEAR,
};

pub const SUFFIX_ROLES: u16 = (1 << ROLE_CURRENCY) | (1 << ROLE_UNIT);
pub const PREFIX_ROLES: u16 = 1 << ROLE_MINUS;
pub const BIND_ROLES: u16 = (1 << ROLE_HUNDRED) | (1 << ROLE_SCALE) | (1 << ROLE_MSCALE);
pub const LEAD_SKIP_ROLES: u16 = (1 << ROLE_AMPM) | (1 << ROLE_MER);
pub const LITV_ROLES: [Role; 3] = [ROLE_CURRENCY, ROLE_AMPM, ROLE_UNIT];

pub const LITERAL_CLASSES: &[LiteralClassDef] = &[
    LiteralClassDef {
        class: MONTH,
        role: ROLE_MINUS,
        words: &[
            ("january", "January "),
            ("february", "February "),
            ("march", "March "),
            ("april", "April "),
            ("june", "June "),
            ("july", "July "),
            ("august", "August "),
            ("september", "September "),
            ("october", "October "),
            ("november", "November "),
            ("december", "December "),
        ],
    },
    LiteralClassDef {
        class: MAY,
        role: ROLE_MINUS,
        words: &[("may", "may ")],
    },
    LiteralClassDef {
        class: DOLLAR,
        role: ROLE_CURRENCY,
        words: &[("dollar", "$"), ("dollars", "$")],
    },
    LiteralClassDef {
        class: CENT,
        role: ROLE_CURRENCY,
        words: &[("cent", "$"), ("cents", "$")],
    },
    LiteralClassDef {
        class: PERCENT,
        role: ROLE_UNIT,
        words: &[("percent", "%")],
    },
    LiteralClassDef {
        class: AM,
        role: ROLE_AMPM,
        words: &[("am", "am")],
    },
    LiteralClassDef {
        class: PM,
        role: ROLE_AMPM,
        words: &[("pm", "pm")],
    },
    LiteralClassDef {
        class: UNIT,
        role: ROLE_UNIT,
        words: UNIT_WORDS,
    },
    LiteralClassDef {
        class: POWER,
        role: ROLE_UNIT,
        words: POWER_WORDS,
    },
    LiteralClassDef {
        class: PREFIX,
        role: ROLE_UNIT,
        words: PREFIX_WORDS,
    },
    LiteralClassDef {
        class: DEGREE,
        role: ROLE_UNIT,
        words: &[("degree", "degree"), ("degrees", "degrees")],
    },
    LiteralClassDef {
        class: TEMP,
        role: ROLE_UNIT,
        words: &[
            ("fahrenheit", "Fahrenheit"),
            ("celsius", "Celsius"),
            ("centigrade", "Celsius"),
        ],
    },
];

/// Literal entries whose written form is the spoken word.
macro_rules! as_spoken {
    ($($w:literal),* $(,)?) => {
        &[$(($w, $w)),*]
    };
}

const UNIT_WORDS: &[(&str, &str)] = as_spoken![
    "euro",
    "euros",
    "meter",
    "meters",
    "metre",
    "metres",
    "kilometer",
    "kilometers",
    "kilometre",
    "kilometres",
    "centimeter",
    "centimeters",
    "millimeter",
    "millimeters",
    "micrometer",
    "micrometers",
    "nanometer",
    "nanometers",
    "mile",
    "miles",
    "foot",
    "feet",
    "inch",
    "inches",
    "yard",
    "yards",
    "gram",
    "grams",
    "kilogram",
    "kilograms",
    "milligram",
    "milligrams",
    "tonne",
    "tonnes",
    "ounce",
    "pound",
    "pounds",
    "ounces",
    "liter",
    "liters",
    "litre",
    "litres",
    "milliliter",
    "milliliters",
    "hectare",
    "hectares",
    "acre",
    "acres",
    "hertz",
    "kilohertz",
    "megahertz",
    "gigahertz",
    "watt",
    "watts",
    "milliwatt",
    "milliwatts",
    "kilowatt",
    "kilowatts",
    "megawatt",
    "megawatts",
    "gigawatt",
    "gigawatts",
    "volt",
    "volts",
    "kilovolt",
    "kilovolts",
    "byte",
    "bytes",
    "kilobyte",
    "kilobytes",
    "megabyte",
    "megabytes",
    "gigabyte",
    "gigabytes",
    "terabyte",
    "terabytes",
    "horsepower",
    "lumen",
    "lumens",
    "calorie",
    "calories",
    "kilocalorie",
    "kilocalories",
];

const POWER_WORDS: &[(&str, &str)] = as_spoken!["square", "cubic"];

const PREFIX_WORDS: &[(&str, &str)] = as_spoken![
    "kilo", "mega", "giga", "tera", "peta", "hecto", "deci", "centi", "milli", "micro", "nano",
    "c",
];

const POWER_BASES: [&str; 9] = [
    "meters",
    "kilometers",
    "centimeters",
    "millimeters",
    "feet",
    "inches",
    "yards",
    "miles",
    "meter",
];

const PREFIX_BASES: [&str; 10] = [
    "watts", "watt", "volts", "meters", "grams", "liters", "bytes", "hertz", "volt", "byte",
];

pub fn affix_domain(word: &str) -> Option<DomainId> {
    let class = LITERAL_CLASSES
        .iter()
        .find(|c| c.words.iter().any(|w| w.0 == word))?
        .class;
    match class {
        DOLLAR => Some(DOMAIN_MONEY),
        CENT => Some(DOMAIN_CENTS),
        PERCENT => Some(DOMAIN_PERCENT),
        UNIT => Some(DOMAIN_MEASURE),
        POWER => Some(DOMAIN_MEASURE_POWER),
        PREFIX => Some(DOMAIN_MEASURE_PREFIX),
        _ => None,
    }
}

pub const CLASSES: &[ClassDef] = &[
    ClassDef {
        class: DIGIT,
        role: ROLE_DIGIT,
        words: &[
            ("one", 1),
            ("two", 2),
            ("three", 3),
            ("four", 4),
            ("five", 5),
            ("six", 6),
            ("seven", 7),
            ("eight", 8),
            ("nine", 9),
        ],
    },
    ClassDef {
        class: OH,
        role: ROLE_OH,
        words: &[("zero", 0), ("oh", 0), ("o", 0)],
    },
    ClassDef {
        class: TEEN,
        role: ROLE_TEEN,
        words: &[
            ("ten", 10),
            ("eleven", 11),
            ("twelve", 12),
            ("thirteen", 13),
            ("fourteen", 14),
            ("fifteen", 15),
            ("sixteen", 16),
            ("seventeen", 17),
            ("eighteen", 18),
            ("nineteen", 19),
        ],
    },
    ClassDef {
        class: TENS,
        role: ROLE_TENS,
        words: &[
            ("twenty", 20),
            ("thirty", 30),
            ("forty", 40),
            ("fifty", 50),
            ("sixty", 60),
            ("seventy", 70),
            ("eighty", 80),
            ("ninety", 90),
            ("twenties", 20),
            ("thirties", 30),
            ("forties", 40),
            ("fifties", 50),
            ("sixties", 60),
            ("seventies", 70),
            ("eighties", 80),
            ("nineties", 90),
        ],
    },
    ClassDef {
        class: HUNDRED,
        role: ROLE_HUNDRED,
        words: &[("hundred", 100)],
    },
    ClassDef {
        class: SCALE,
        role: ROLE_SCALE,
        words: &[("thousand", 1_000)],
    },
    ClassDef {
        class: MSCALE,
        role: ROLE_MSCALE,
        words: &[("million", 1_000_000), ("billion", 1_000_000_000)],
    },
    ClassDef {
        class: POINT,
        role: ROLE_POINT,
        words: &[("point", 0)],
    },
    ClassDef {
        class: AND,
        role: ROLE_AND,
        words: &[("and", 0)],
    },
    ClassDef {
        class: ORDINAL_UNIT,
        role: ROLE_ORDINAL,
        words: &[
            ("zeroth", 0),
            ("hundredth", 100),
            ("thousandth", 1_000),
            ("first", 1),
            ("second", 2),
            ("third", 3),
            ("fourth", 4),
            ("fifth", 5),
            ("sixth", 6),
            ("seventh", 7),
            ("eighth", 8),
            ("ninth", 9),
        ],
    },
    ClassDef {
        class: ORDINAL_TEEN,
        role: ROLE_ORDINAL,
        words: &[
            ("tenth", 10),
            ("eleventh", 11),
            ("twelfth", 12),
            ("thirteenth", 13),
            ("fourteenth", 14),
            ("fifteenth", 15),
            ("sixteenth", 16),
            ("seventeenth", 17),
            ("eighteenth", 18),
            ("nineteenth", 19),
        ],
    },
    ClassDef {
        class: ORDINAL_TENS,
        role: ROLE_ORDINAL,
        words: &[
            ("twentieth", 20),
            ("thirtieth", 30),
            ("fortieth", 40),
            ("fiftieth", 50),
            ("sixtieth", 60),
            ("seventieth", 70),
            ("eightieth", 80),
            ("ninetieth", 90),
        ],
    },
    ClassDef {
        class: AT,
        role: 0,
        words: &[("at", 0)],
    },
    ClassDef {
        class: SOFT_AT,
        role: 0,
        // "it's five thirty" states the time now, like "at five thirty".
        words: &[("it's", 0), ("it’s", 0)],
    },
    ClassDef {
        class: OCLOCK,
        role: ROLE_MER,
        words: &[("o'clock", 0), ("oclock", 0)],
    },
    ClassDef {
        class: HOURS,
        role: 0,
        words: &[("hours", 0)],
    },
    ClassDef {
        class: LETTER_PM,
        role: ROLE_MER,
        words: &[("p", 0), ("m", 0)],
    },
    ClassDef {
        class: NUMBER,
        role: 0,
        words: &[("number", 0)],
    },
    ClassDef {
        class: ROOM,
        role: 0,
        words: &[("room", 0)],
    },
    ClassDef {
        class: FLIGHT,
        role: 0,
        words: &[("flight", 0)],
    },
    ClassDef {
        class: CODE,
        role: 0,
        words: &[("code", 0)],
    },
    ClassDef {
        class: PIN,
        role: 0,
        words: &[("pin", 0)],
    },
    ClassDef {
        class: EXTENSION,
        role: 0,
        words: &[("extension", 0)],
    },
    ClassDef {
        class: LABEL,
        role: 0,
        words: &[
            ("chapter", 0),
            ("episode", 0),
            ("level", 0),
            ("option", 0),
            ("page", 0),
            ("part", 0),
            ("phase", 0),
            ("season", 0),
            ("step", 0),
            ("track", 0),
            ("version", 0),
        ],
    },
    ClassDef {
        class: THE,
        role: 0,
        words: &[("the", 0)],
    },
    ClassDef {
        class: PER,
        role: 0,
        words: &[("per", 0)],
    },
    ClassDef {
        class: DOT,
        role: 0,
        words: &[("dot", 0)],
    },
    ClassDef {
        class: IT,
        role: 0,
        words: &[("it", 0)],
    },
    ClassDef {
        class: IS,
        role: 0,
        words: &[("is", 0)],
    },
    ClassDef {
        class: WAS,
        role: 0,
        words: &[("was", 0)],
    },
];

const UNITS: [&str; 10] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];
const TEENS: [&str; 10] = [
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
const TENS_WORDS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const ORD_UNITS: [&str; 10] = [
    "", "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
];
const ORD_TEENS: [&str; 10] = [
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
];
const ORD_TENS: [&str; 10] = [
    "",
    "",
    "twentieth",
    "thirtieth",
    "fortieth",
    "fiftieth",
    "sixtieth",
    "seventieth",
    "eightieth",
    "ninetieth",
];

const IDENTIFIER_SAMPLES: usize = 100;
const EXAMPLE_SEED: u64 = 0x454E_475F_4954_4E31;
const CARDINAL_SAMPLES: usize = 6000;
const DECIMAL_SAMPLES: usize = 4000;
const SCALED_SAMPLES: usize = 1500;
const ORDINAL_SAMPLES: usize = 3000;
const AND_SHARE: f64 = 0.3;
const YEAR_SAMPLES: usize = 3000;
const DATE_SAMPLES: usize = 2000;
const MONEY_SAMPLES: usize = 3000;
const PERCENT_SAMPLES: usize = 1000;
const MEASURE_SAMPLES: usize = 1500;

pub const KIND_MASS: &[(&str, f64)] = &[
    ("cardinal", 1.0),
    ("hundreds", 0.15),
    ("measure", 0.3),
    ("decimal", 0.5),
    ("scaled", 0.25),
    ("ordinal", 0.5),
    ("year", 0.6),
    ("date", 0.4),
    ("time", 0.5),
    ("identifier", 0.3),
    ("money", 0.5),
    ("percent", 0.2),
];

pub const PLURAL: &[&str] = &[
    "twenties",
    "thirties",
    "forties",
    "fifties",
    "sixties",
    "seventies",
    "eighties",
    "nineties",
];

pub const WORDS_FIRST: &[&str] = &[
    "one", "first", "second", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
];

/// Words of each `types::sense` bit. A word not in the lexicon gets an entry
/// of class `CLASS_UNKNOWN`, which only carries these bits.
pub const SENSES: &[(u32, &[&str])] = &[
    (
        sense::COUNT,
        &[
            "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
        ],
    ),
    (sense::ONE, &["one"]),
    (
        sense::ONE_AFTER,
        &[
            "no",
            "which",
            "whichever",
            "this",
            "that",
            "the",
            "each",
            "every",
            "any",
            "other",
            "old",
            "new",
            "last",
            "next",
            "same",
            "right",
            "wrong",
            "bottom",
            "top",
            "best",
            "first",
            "big",
            "small",
            "little",
            "main",
            "basic",
            "good",
        ],
    ),
    (
        sense::ONE_CLAUSE,
        &["is", "was", "that", "that's", "which", "who"],
    ),
    (
        sense::ONE_BEFORE,
        &[
            "of", "more", "another", "at", "way", "thing", "sec", "every", "piece",
        ],
    ),
    (sense::GROUP, &["us", "them", "you"]),
    (
        sense::FRACTION,
        &[
            "half",
            "quarter",
            "third",
            "thirds",
            "quarters",
            "halves",
            "fourths",
            "fifths",
            "sixths",
            "sevenths",
            "eighths",
            "ninths",
            "tenths",
            "sixteenths",
        ],
    ),
    (sense::RELATIVE, &["to", "till", "past", "after"]),
    (
        sense::HOUR,
        &[
            "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
            "eleven", "twelve",
        ],
    ),
    (sense::MINUTE, &["five", "ten", "twenty", "quarter", "half"]),
    (
        sense::TIME_CUE,
        &[
            "at", "is", "it's", "was", "by", "until", "till", "around", "about",
        ],
    ),
    (sense::A, &["a"]),
    (sense::AN, &["an"]),
    (sense::AND, &["and"]),
    (sense::MERIDIEM, &["am", "pm", "m"]),
    (
        sense::SUBJECT,
        &[
            "i", "you", "we", "they", "he", "she", "it", "who", "to", "will", "can", "could",
            "might", "would", "should", "must",
        ],
    ),
    (sense::BY, &["by"]),
    (sense::DAY, &["day"]),
    (sense::IN, &["in"]),
    (sense::PIECE, &["piece"]),
    (sense::MILLION, &["million"]),
    (sense::OF, &["of"]),
    (sense::TWENTY, &["twenty"]),
    (sense::FIVE, &["five"]),
    (sense::QUARTER, &["quarter"]),
];

/// Phrases whose number words stay words. A match tried from a word follows
/// this order, then the word's place in the phrase.
pub const IDIOMS: &[&[&str]] = &[
    &["six", "of", "one"],
    &["two", "heads", "are", "better", "than", "one"],
    &["one", "off"],
    &["cloud", "nine"],
    &["twenty", "twenty", "vision"],
    &["eleventh", "hour"],
];

pub fn kind_mass(kind: &str) -> f64 {
    KIND_MASS.iter().find(|k| k.0 == kind).map_or(0.0, |k| k.1)
}

fn under_1000(n: u32, out: &mut Vec<&'static str>) {
    let (h, r) = ((n / 100) as usize, (n % 100) as usize);
    if h > 0 {
        out.extend([UNITS[h], "hundred"]);
    }
    match r {
        20.. => {
            out.push(TENS_WORDS[r / 10]);
            if r % 10 > 0 {
                out.push(UNITS[r % 10]);
            }
        }
        10..=19 => out.push(TEENS[r - 10]),
        1..=9 => out.push(UNITS[r]),
        _ => {}
    }
}

pub fn cardinal(n: u64, and: bool) -> Vec<&'static str> {
    if n == 0 {
        return vec!["zero"];
    }
    let mut out = Vec::new();
    for (scale, word) in [
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
        (1_000, "thousand"),
    ] {
        let group = ((n / scale) % 1000) as u32;
        if group > 0 {
            under_1000(group, &mut out);
            out.push(word);
        }
    }
    let r = (n % 1000) as u32;
    if and && !r.is_multiple_of(100) && n >= 100 {
        under_1000(r - r % 100, &mut out);
        out.push("and");
        under_1000(r % 100, &mut out);
    } else {
        under_1000(r, &mut out);
    }
    out
}

pub fn hundreds_style(n: u32, and: bool) -> Vec<&'static str> {
    let mut out = cardinal(u64::from(n / 100), false);
    out.push("hundred");
    if !n.is_multiple_of(100) {
        if and {
            out.push("and");
        }
        under_1000(n % 100, &mut out);
    }
    out
}

pub fn ordinal(n: u64) -> Option<Vec<&'static str>> {
    let mut words = cardinal(n, false);
    ordinalize(&mut words, (n % 100) as usize)?;
    Some(words)
}

fn ordinalize(words: &mut [&'static str], r: usize) -> Option<()> {
    let last = words.last_mut()?;
    *last = match (*last, r) {
        ("zero", _) => "zeroth",
        ("hundred", _) => "hundredth",
        ("thousand", _) => "thousandth",
        (_, 0) => return None,
        (_, 10..=19) => ORD_TEENS[r - 10],
        _ if r.is_multiple_of(10) => ORD_TENS[r / 10],
        _ => ORD_UNITS[r % 10],
    };
    Some(())
}

fn time(hour: u32, minute: u32) -> Vec<&'static str> {
    let mut out = cardinal(u64::from(hour), false);
    if minute < 10 {
        out.extend(["oh", UNITS[minute as usize]]);
    } else {
        under_1000(minute, &mut out);
    }
    out
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

fn zero_word(r: u64) -> &'static str {
    match r % 10 {
        0..=5 => "zero",
        6..=8 => "o",
        _ => "oh",
    }
}

fn fraction(state: &mut u64, len: usize, out: &mut Vec<&'static str>) {
    out.push("point");
    for _ in 0..len {
        let r = next_seed(state);
        out.push(match (r % 10) as usize {
            0 => zero_word(r >> 8),
            d => UNITS[d],
        });
    }
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
        self.out.push(Example {
            words,
            src,
            weight,
            kind,
        });
    }

    fn cardinal(&mut self, n: u64, weight: f64, kind: &'static str) {
        let plain = cardinal(n, false);
        let and = cardinal(n, true);
        if and == plain {
            self.push(plain, SRC_CARDINAL, weight, kind);
        } else {
            self.push(plain, SRC_CARDINAL, weight * (1.0 - AND_SHARE), kind);
            self.push(and, SRC_CARDINAL, weight * AND_SHARE, kind);
        }
    }

    fn rand(&mut self) -> u64 {
        next_seed(&mut self.state)
    }

    fn number(&mut self, max_digits: u32) -> u64 {
        sample_number(&mut self.state, max_digits)
    }
}

fn hundreds_values() -> Vec<u32> {
    (1100u32..10_000)
        .filter(|n| !(n / 100).is_multiple_of(10))
        .collect()
}

fn cardinals(s: &mut Sink) {
    let w = 1.0 / CARDINAL_SAMPLES as f64;
    for _ in 0..CARDINAL_SAMPLES {
        let n = s.number(12);
        s.cardinal(n, w, "cardinal");
    }
    let hundreds = hundreds_values();
    let w = 1.0 / hundreds.len() as f64;
    for &n in &hundreds {
        s.push(
            hundreds_style(n, false),
            SRC_CARDINAL,
            w * (1.0 - AND_SHARE),
            "hundreds",
        );
        s.push(
            hundreds_style(n, true),
            SRC_CARDINAL,
            w * AND_SHARE,
            "hundreds",
        );
    }
}

fn decimals(s: &mut Sink) {
    let w = 1.0 / DECIMAL_SAMPLES as f64;
    for _ in 0..DECIMAL_SAMPLES {
        let r = s.rand();
        let mut words = match r % 20 {
            0..=1 => Vec::new(),
            2..=4 => vec!["zero"],
            _ => cardinal(s.number(4), false),
        };
        let len = pick(r >> 8, &[(1, 45), (2, 35), (3, 15), (4, 5)]);
        fraction(&mut s.state, len, &mut words);
        s.push(words, SRC_CARDINAL, w, "decimal");
    }
    let w = 1.0 / SCALED_SAMPLES as f64;
    for _ in 0..SCALED_SAMPLES {
        let r = s.rand();
        let mut words = cardinal(s.number(3), false);
        let len = pick(r >> 8, &[(0, 2), (1, 5), (2, 3)]);
        if len > 0 {
            fraction(&mut s.state, len, &mut words);
        }
        words.push(pick(
            r >> 16,
            &[("thousand", 2), ("million", 5), ("billion", 3)],
        ));
        s.push(words, SRC_CARDINAL, w, "scaled");
    }
}

fn ordinals(s: &mut Sink) {
    let w = 1.0 / ORDINAL_SAMPLES as f64;
    for _ in 0..ORDINAL_SAMPLES {
        let r = s.rand();
        let n = if r.is_multiple_of(50) { 0 } else { s.number(5) };
        if let Some(words) = ordinal(n) {
            let mut and = cardinal(n, true);
            let spoken = ordinalize(&mut and, (n % 100) as usize).is_some() && and != words;
            let share = if spoken { AND_SHARE } else { 0.0 };
            s.push(words, SRC_CARDINAL, w * (1.0 - share), "ordinal");
            if spoken {
                s.push(and, SRC_CARDINAL, w * share, "ordinal");
            }
        }
    }
    let hundreds = hundreds_values();
    let w = 0.2 / hundreds.len() as f64;
    for &n in &hundreds {
        for (and, share) in [(false, 1.0 - AND_SHARE), (true, AND_SHARE)] {
            let mut words = hundreds_style(n, and);
            if ordinalize(&mut words, (n % 100) as usize).is_some() {
                s.push(words, SRC_CARDINAL, w * share, "ordinal");
            }
        }
    }
}

const ON_THE_HOUR: f64 = 6.0;

fn times(s: &mut Sink) {
    let w = 1.0 / (12.0 * 60.0);
    for hour in 1..=12 {
        let mut hour_only = cardinal(u64::from(hour), false);
        hour_only.push("o'clock");
        s.push(hour_only.clone(), SRC_TIME, w * ON_THE_HOUR, "time");
        hour_only.pop();
        let meridiem = if hour % 2 == 0 { "pm" } else { "am" };
        let with_meridiem = [&hour_only[..], &[meridiem]].concat();
        s.push(with_meridiem, SRC_TIME, w * ON_THE_HOUR, "time");
        hour_only.extend(["p", "m"]);
        s.push(hour_only, SRC_TIME, w * ON_THE_HOUR, "time");
        for minute in 1..60 {
            let mut words = time(hour, minute);
            s.push(words.clone(), SRC_TIME, w * 0.6, "time");
            let meridiem = if minute % 2 == 0 { "pm" } else { "am" };
            s.push(
                [&words[..], &[meridiem]].concat(),
                SRC_TIME,
                w * 0.3,
                "time",
            );
            words.extend(["p", "m"]);
            s.push(words, SRC_TIME, w * 0.4, "time");
        }
    }
}

/// 24-hour times: "fifteen thirty", "oh seven thirty", "oh eight hundred".
fn military(s: &mut Sink) {
    let w = 1.0 / (24.0 * 60.0);
    for hour in 13..=23 {
        for minute in 1..60 {
            // "twenty thirty" reads as a year, and "forty thirty" would share its shape.
            if hour == 20 && minute >= 10 {
                continue;
            }
            s.push(time(hour, minute), SRC_TIME, w * 0.5, "time");
        }
    }
    for hour in 1..=9 {
        for minute in 1..60 {
            let mut words = vec!["oh", UNITS[hour as usize]];
            if minute < 10 {
                words.extend(["oh", UNITS[minute as usize]]);
            } else {
                under_1000(minute, &mut words);
            }
            s.push(words, SRC_TIME, w * 0.5, "time");
        }
        for zero in ["oh", "zero"] {
            s.push(
                vec![zero, UNITS[hour as usize], "hundred"],
                SRC_CARDINAL,
                w * 20.0,
                "time",
            );
        }
    }
}

pub fn year(y: u32, style: u64) -> Vec<&'static str> {
    let (hi, lo) = (y / 100, (y % 100) as usize);
    if (2001..2010).contains(&y) && (style >> 12) % 10 >= 7 {
        return vec!["twenty", "oh", UNITS[lo]];
    }
    if (2000..2010).contains(&y) || (y >= 2010 && style % 10 < 4) {
        return cardinal(u64::from(y), (style >> 4) % 10 < 3);
    }
    let mut out = cardinal(u64::from(hi), false);
    match lo {
        0 => out.push("hundred"),
        1..=9 => out.extend([if (style >> 8) % 10 < 7 { "oh" } else { "o" }, UNITS[lo]]),
        _ => under_1000(lo as u32, &mut out),
    }
    out
}

fn years(s: &mut Sink) {
    let w = 1.0 / YEAR_SAMPLES as f64;
    for _ in 0..YEAR_SAMPLES {
        let r = s.rand();
        let y = if r.is_multiple_of(5) {
            1000 + ((r >> 8) % 900) as u32
        } else {
            1900 + ((r >> 8) % 131) as u32
        };
        s.push(year(y, r >> 20), SRC_YEAR, w, "year");
    }
}

fn dates(s: &mut Sink) {
    let w = 1.0 / DATE_SAMPLES as f64;
    for _ in 0..DATE_SAMPLES {
        let r = s.rand();
        let day = 1 + (r % 31);
        let form = (r >> 8) % 20;
        let mut words = if form < 3 {
            cardinal(day, false)
        } else {
            ordinal(day).unwrap_or_default()
        };
        if form >= 13 {
            let y = 1900 + ((r >> 16) % 131) as u32;
            words.extend(year(y, r >> 28));
        }
        s.push(words, SRC_DATE, w, "date");
    }
}

fn amount(s: &mut Sink, max_digits: u32, decimal_share: u64) -> Vec<&'static str> {
    let r = s.rand();
    let mut words = cardinal(s.number(max_digits), false);
    if r % 100 < decimal_share {
        let len = pick(r >> 8, &[(1, 5), (2, 4), (3, 1)]);
        fraction(&mut s.state, len, &mut words);
    }
    words
}

const AFFIX_MAX_DIGITS: u32 = 9;
const COMPOSITE_MAX_DIGITS: u32 = 2;
const COMMON_CENTS: [u64; 8] = [50, 99, 95, 25, 75, 10, 20, 49];

fn money(s: &mut Sink) {
    let w = 1.0 / MONEY_SAMPLES as f64;
    for _ in 0..MONEY_SAMPLES {
        let r = s.rand();
        let form = r % 20;
        let cents = if (r >> 24).is_multiple_of(2) {
            COMMON_CENTS[((r >> 8) % COMMON_CENTS.len() as u64) as usize]
        } else {
            1 + (r >> 8) % 99
        };
        let mut words = match form {
            0..=1 => cardinal(cents, false),
            2..=4 => {
                let mut a = amount(s, 3, 40);
                a.push(pick(r >> 16, &[("million", 6), ("billion", 4)]));
                a
            }
            5..=8 => cardinal(s.number(COMPOSITE_MAX_DIGITS), false),
            _ => amount(s, AFFIX_MAX_DIGITS, 10),
        };
        if form <= 1 {
            words.push(if cents == 1 { "cent" } else { "cents" });
            s.push(words, SRC_CARDINAL, w, "money");
            continue;
        }
        words.push(if words == ["one"] {
            "dollar"
        } else {
            "dollars"
        });
        if (5..=8).contains(&form) && !words.contains(&"point") {
            if form <= 6 {
                words.push("and");
            }
            words.extend(cardinal(cents, false));
            if form != 8 {
                words.push(if cents == 1 { "cent" } else { "cents" });
            }
        }
        s.push(words, SRC_CARDINAL, w, "money");
    }
}

fn percents(s: &mut Sink) {
    let w = 1.0 / PERCENT_SAMPLES as f64;
    for _ in 0..PERCENT_SAMPLES {
        let mut words = amount(s, AFFIX_MAX_DIGITS, 40);
        words.push("percent");
        s.push(words, SRC_CARDINAL, w, "percent");
    }
}

fn measures(s: &mut Sink) {
    let w = 1.0 / MEASURE_SAMPLES as f64;
    for _ in 0..MEASURE_SAMPLES {
        let r = s.rand();
        let mut words = amount(s, AFFIX_MAX_DIGITS, 30);
        let pick_of = |list: &[(&'static str, &'static str)], k: u64| {
            list[(k % list.len() as u64) as usize].0
        };
        match (r >> 32) % 20 {
            0..=2 => {
                words.push(pick_of(POWER_WORDS, r >> 8));
                words.push(POWER_BASES[((r >> 16) % POWER_BASES.len() as u64) as usize]);
            }
            3..=5 => {
                let prefix = pick_of(PREFIX_WORDS, r >> 8);
                words.push(prefix);
                words.push(if prefix == "c" {
                    "c"
                } else {
                    PREFIX_BASES[((r >> 16) % PREFIX_BASES.len() as u64) as usize]
                });
            }
            _ => words.push(pick_of(UNIT_WORDS, r)),
        }
        s.push(words, SRC_CARDINAL, w, "measure");
    }
}

const COVERAGE_WEIGHT: f64 = 0.05;
const COVERAGE_INTS: [Option<u64>; 10] = [
    None,
    Some(0),
    Some(7),
    Some(18),
    Some(60),
    Some(64),
    Some(800),
    Some(808),
    Some(818),
    Some(864),
];
const COVERAGE_GROUPS: [u64; 9] = [7, 18, 60, 64, 800, 801, 818, 860, 864];
const BILLION_GROUPS: [u64; 3] = [7, 18, 64];

fn fraction_word(zeros: u32, i: usize) -> &'static str {
    if zeros >> i & 1 == 1 {
        "o"
    } else {
        UNITS[2 + i]
    }
}

const LONG_FRACTION_INTS: [Option<u64>; 4] = [None, Some(7), Some(18), Some(64)];
const PAIR_FRACTIONS: [&[&str]; 3] = [&["twelve"], &["twenty"], &["eighty", "two"]];

fn coverage_decimals() -> Vec<Vec<&'static str>> {
    let mut out = Vec::new();
    for len in 1..=4usize {
        for mask in 0..1u32 << len {
            for int in COVERAGE_INTS {
                let mut words = int.map_or(Vec::new(), |n| cardinal(n, false));
                words.push("point");
                words.extend((0..len).map(|i| fraction_word(mask, i)));
                out.push(words);
            }
        }
    }
    for len in 5..=6usize {
        for int in LONG_FRACTION_INTS {
            let mut words = int.map_or(Vec::new(), |n| cardinal(n, false));
            words.push("point");
            words.extend((0..len).map(|i| UNITS[2 + i]));
            out.push(words);
        }
    }
    for int in COVERAGE_INTS.into_iter().flatten() {
        for frac in PAIR_FRACTIONS {
            let mut words = cardinal(int, false);
            words.push("point");
            words.extend(frac);
            out.push(words);
        }
    }
    out
}

const IDENTIFIER_COVERAGE_LENS: std::ops::RangeInclusive<usize> = 3..=8;

/// Every digit and "oh" pattern, so no spoken digit string is missing a shape.
fn coverage_identifiers() -> Vec<Vec<&'static str>> {
    let mut out = Vec::new();
    for len in IDENTIFIER_COVERAGE_LENS {
        for mask in 0..1u32 << len {
            out.push(
                (0..len)
                    .map(|i| if mask >> i & 1 == 1 { "oh" } else { "five" })
                    .collect(),
            );
        }
    }
    out
}

fn coverage_dates() -> Vec<Vec<&'static str>> {
    let years: [Vec<&'static str>; 14] = [
        vec!["nineteen", "oh", "five"],
        vec!["nineteen", "fifteen"],
        vec!["nineteen", "twenty"],
        vec!["nineteen", "forty", "five"],
        vec!["nineteen", "hundred"],
        vec!["twenty", "oh", "eight"],
        vec!["twenty", "fifteen"],
        vec!["twenty", "twenty"],
        vec!["twenty", "twenty", "four"],
        cardinal(2000, false),
        cardinal(2005, false),
        cardinal(2005, true),
        cardinal(2015, false),
        cardinal(2024, false),
    ];
    let mut out = Vec::new();
    for day in [1, 29] {
        for y in &years {
            let mut words = ordinal(day).unwrap_or_default();
            words.extend(y);
            out.push(words);
        }
    }
    out
}

fn coverage_cardinals() -> Vec<Vec<&'static str>> {
    let mut out = vec![vec!["zero"]];
    for hi in COVERAGE_GROUPS {
        for lo in std::iter::once(0).chain(COVERAGE_GROUPS) {
            out.push(cardinal(hi * 1000 + lo, false));
            out.push(cardinal(hi * 1_000_000 + lo * 1000, false));
            if BILLION_GROUPS.contains(&hi) {
                out.push(cardinal(hi * 1_000_000_000 + lo * 1_000_000, false));
            }
        }
        out.push(cardinal(hi * 1_000_000 + 3, false));
    }
    out
}

fn coverage_scaled() -> Vec<Vec<&'static str>> {
    let mut out = Vec::new();
    for scale in ["thousand", "million", "billion"] {
        for frac in [&["two"][..], &["two", "five"]] {
            let mut words = vec!["zero", "point"];
            words.extend(frac);
            words.push(scale);
            out.push(words);
        }
    }
    out
}

/// "ten dollars and five cents": every dollar-word shape with one- and
/// two-word cents, which random sampling can miss.
fn coverage_money() -> Vec<Vec<&'static str>> {
    let mut out = Vec::new();
    for dollars in [1, 5, 10, 15, 20, 25, 100] {
        for cents in [1, 5, 15, 50, 55] {
            for and in [true, false] {
                let mut words = cardinal(dollars, false);
                words.push(if dollars == 1 { "dollar" } else { "dollars" });
                if and {
                    words.push("and");
                }
                words.extend(cardinal(cents, false));
                words.push(if cents == 1 { "cent" } else { "cents" });
                out.push(words);
            }
        }
    }
    out
}

fn coverage(s: &mut Sink) {
    for (kind, src, list) in [
        ("money", SRC_CARDINAL, coverage_money()),
        ("decimal", SRC_CARDINAL, coverage_decimals()),
        ("cardinal", SRC_CARDINAL, coverage_cardinals()),
        ("scaled", SRC_CARDINAL, coverage_scaled()),
        ("identifier", SRC_IDENTIFIER, coverage_identifiers()),
        ("date", SRC_DATE, coverage_dates()),
    ] {
        let w = COVERAGE_WEIGHT / list.len() as f64;
        for words in list {
            s.push(words, src, w, kind);
        }
    }
}

fn identifiers(s: &mut Sink) {
    let w = 1.0 / (6 * IDENTIFIER_SAMPLES) as f64;
    for len in 3..=8 {
        for _ in 0..IDENTIFIER_SAMPLES {
            let words = (0..len)
                .map(|_| {
                    let r = s.rand();
                    match (r % 10) as usize {
                        0 => zero_word(r >> 8),
                        d => UNITS[d],
                    }
                })
                .collect();
            s.push(words, SRC_IDENTIFIER, w, "identifier");
        }
    }
}

pub fn examples() -> Vec<Example> {
    let mut s = Sink {
        out: Vec::new(),
        state: EXAMPLE_SEED,
    };
    cardinals(&mut s);
    decimals(&mut s);
    ordinals(&mut s);
    years(&mut s);
    dates(&mut s);
    money(&mut s);
    percents(&mut s);
    measures(&mut s);
    times(&mut s);
    military(&mut s);
    identifiers(&mut s);
    coverage(&mut s);
    s.out
}

/// Words the line passes look for, by kind.
pub const KINDS: &[WordKind] = &[
    WordKind {
        kind: kind::A,
        aux: 0,
        words: &["a"],
    },
    WordKind {
        kind: kind::OF,
        aux: 0,
        words: &["of"],
    },
    WordKind {
        kind: kind::PER,
        aux: 0,
        words: &["per"],
    },
    WordKind {
        kind: kind::DOT,
        aux: 0,
        words: &["dot"],
    },
    WordKind {
        kind: kind::DOUBLE,
        aux: 1,
        words: &["double"],
    },
    WordKind {
        kind: kind::DOUBLE,
        aux: 2,
        words: &["triple"],
    },
    WordKind {
        kind: kind::HUNDRED,
        aux: 0,
        words: &["hundred"],
    },
    WordKind {
        kind: kind::HUNDREDS,
        aux: 0,
        words: &["hundreds"],
    },
    WordKind {
        kind: kind::STREET,
        aux: 0,
        words: &[
            "road",
            "lane",
            "drive",
            "court",
            "place",
            "street",
            "avenue",
            "parkway",
            "highway",
            "boulevard",
        ],
    },
    WordKind {
        kind: kind::MONTH_VERB,
        aux: 1,
        words: &["may"],
    },
    WordKind {
        kind: kind::MONTH_VERB,
        aux: 0,
        words: &["march"],
    },
    WordKind {
        kind: kind::FILLER,
        aux: 0,
        words: &["uh", "um", "er", "ah", "uhm", "hmm"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 3,
        words: &["thousand"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 6,
        words: &["million"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 9,
        words: &["billion"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 12,
        words: &["trillion"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 15,
        words: &["quadrillion"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 18,
        words: &["quintillion"],
    },
    WordKind {
        kind: kind::NOON,
        aux: 0,
        words: &["noon", "midnight"],
    },
];

const NAMES: [&str; name::COUNT] = [
    "one", "two", "oh", "thousand", "billion", "percent", "per cent", "twenty", "twelve", "hours",
    "and", "cent", "first", "am", "dot",
];

pub static LANGUAGE: Language = Language {
    name: "english",
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
        time_left: TIME_LEFT,
        time_right: &TIME_RIGHT,
        identifier_left: &IDENTIFIER_LEFT,
    },
    kinds: KINDS,
    vocab: VocabSpec {
        names: NAMES,
        months: [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ],
        hours: [
            "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
            "eleven", "twelve",
        ],
        centuries: [
            "1100s", "1200s", "1300s", "1400s", "1500s", "1600s", "1700s", "1800s", "1900s",
        ],
        zones: &[
            "EST", "EDT", "CST", "CDT", "MST", "MDT", "PST", "PDT", "GMT", "UTC", "BST",
        ],
        not_after_number: "ai",
        not_before_number: "a",
    },
    literals: Literals {
        oclock: " o'clock",
        may: "May ",
    },
    numbers: NumberStyle {
        decimal: ".",
        grouped: true,
        pairs: true,
        fraction_number: false,
    },
    senses: SENSES,
    idioms: IDIOMS,
    plural: PLURAL,
    words_first: WORDS_FIRST,
    suffix_roles: SUFFIX_ROLES,
    prefix_roles: PREFIX_ROLES,
    bind_roles: BIND_ROLES,
    lead_skip_roles: LEAD_SKIP_ROLES,
    left_folds: LEFT_FOLDS,
    aliases: &ALIASES,
    start_classes: &START_CLASSES,
    per: PER,
    per_units: &PER_UNITS,
    pair_only: &PAIR_ONLY,
    lone_after: &[HOURS],
    setting_classes: &SETTING_CLASSES,
    setting_domains: &SETTING_DOMAINS,
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
        assert_eq!(cardinal(0, false), ["zero"]);
        assert_eq!(cardinal(123, false), ["one", "hundred", "twenty", "three"]);
        assert_eq!(cardinal(105, true), ["one", "hundred", "and", "five"]);
        assert_eq!(cardinal(2005, false), ["two", "thousand", "five"]);
        assert_eq!(
            cardinal(5_300_000, false),
            ["five", "million", "three", "hundred", "thousand"]
        );
    }

    #[test]
    fn ordinal_words() {
        assert_eq!(ordinal(21), Some(vec!["twenty", "first"]));
        assert_eq!(ordinal(12), Some(vec!["twelfth"]));
        assert_eq!(ordinal(40), Some(vec!["fortieth"]));
        assert_eq!(ordinal(101), Some(vec!["one", "hundred", "first"]));
        assert_eq!(ordinal(300), Some(vec!["three", "hundredth"]));
        assert_eq!(ordinal(0), Some(vec!["zeroth"]));
        assert_eq!(ordinal(2_000_000), None);
    }

    #[test]
    fn time_words() {
        assert_eq!(time(5, 30), ["five", "thirty"]);
        assert_eq!(time(5, 5), ["five", "oh", "five"]);
        assert_eq!(time(12, 45), ["twelve", "forty", "five"]);
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
