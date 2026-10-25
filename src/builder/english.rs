use crate::hash::next_seed;
use crate::types::{
    ClassId, DOMAIN_CENTS, DOMAIN_MEASURE, DOMAIN_MEASURE_POWER, DOMAIN_MEASURE_PREFIX,
    DOMAIN_MONEY, DOMAIN_NEGATIVE, DOMAIN_PERCENT, DomainId, Role,
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
pub const MINUS: ClassId = 29;
pub const LETTER_PM: ClassId = 30;
pub const UNIT: ClassId = 31;
pub const POWER: ClassId = 32;
pub const PREFIX: ClassId = 33;
pub const MAY: ClassId = 34;
pub const PER: ClassId = 35;
pub const DEGREE: ClassId = 36;
pub const TEMP: ClassId = 37;
pub const PER_UNITS: [ClassId; 5] = [UNIT, POWER, PREFIX, DEGREE, TEMP];
pub const PAIR_ONLY: [ClassId; 2] = [DEGREE, TEMP];

pub const DOMAIN_MONTH_DATE: DomainId = 13;
pub const DOMAIN_MAY_DATE: DomainId = 14;
pub const START_CLASSES: [ClassId; 2] = [MONTH, MAY];

pub use crate::types::{
    ROLE_AMPM, ROLE_AND, ROLE_CURRENCY, ROLE_DIGIT, ROLE_HUNDRED, ROLE_MER, ROLE_MINUS,
    ROLE_MSCALE, ROLE_OH, ROLE_ORDINAL, ROLE_POINT, ROLE_SCALE, ROLE_TEEN, ROLE_TENS, ROLE_UNIT,
};

pub const TIME_RIGHT: [ClassId; 3] = [AM, PM, OCLOCK];
pub const TIME_LEFT: ClassId = AT;
pub const IDENTIFIER_LEFT: [ClassId; 6] = [NUMBER, ROOM, FLIGHT, CODE, PIN, EXTENSION];

pub const SRC_CARDINAL: u8 = 1;
pub const SRC_TIME: u8 = 2;
pub const SRC_IDENTIFIER: u8 = 4;
pub const SRC_YEAR: u8 = 8;
pub const SRC_DATE: u8 = 16;

pub const SUFFIX_ROLES: u16 = (1 << ROLE_CURRENCY) | (1 << ROLE_UNIT);
pub const PREFIX_ROLES: u16 = 1 << ROLE_MINUS;
pub const BIND_ROLES: u16 = (1 << ROLE_HUNDRED) | (1 << ROLE_SCALE) | (1 << ROLE_MSCALE);
pub const LITV_ROLES: [Role; 3] = [ROLE_CURRENCY, ROLE_AMPM, ROLE_UNIT];

pub struct LiteralClassDef {
    pub class: ClassId,
    pub role: Role,
    pub words: &'static [(&'static str, &'static str)],
}

pub const LITERAL_CLASSES: &[LiteralClassDef] = &[
    LiteralClassDef {
        class: MINUS,
        role: ROLE_MINUS,
        words: &[("minus", "-")],
    },
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
        words: &[
            ("dollar", "$"),
            ("dollars", "$"),
            ("yen", "¥"),
            ("won", "₩"),
            ("rupee", "₹"),
            ("rupees", "₹"),
        ],
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
        words: &[("am", " a.m.")],
    },
    LiteralClassDef {
        class: PM,
        role: ROLE_AMPM,
        words: &[("pm", " p.m.")],
    },
    LiteralClassDef {
        class: UNIT,
        role: ROLE_UNIT,
        words: UNITS_SI,
    },
    LiteralClassDef {
        class: POWER,
        role: ROLE_UNIT,
        words: POWERS,
    },
    LiteralClassDef {
        class: PREFIX,
        role: ROLE_UNIT,
        words: SI_PREFIXES,
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
            ("fahrenheit", " Fahrenheit"),
            ("celsius", " Celsius"),
            ("centigrade", " Celsius"),
        ],
    },
];

const UNITS_SI: &[(&str, &str)] = &[
    ("euro", "euro"),
    ("euros", "euros"),
    ("meter", "m"),
    ("meters", "m"),
    ("metre", "m"),
    ("metres", "m"),
    ("kilometer", "km"),
    ("kilometers", "km"),
    ("kilometre", "km"),
    ("kilometres", "km"),
    ("centimeter", "cm"),
    ("centimeters", "cm"),
    ("millimeter", "mm"),
    ("millimeters", "mm"),
    ("micrometer", "μm"),
    ("micrometers", "μm"),
    ("nanometer", "nm"),
    ("nanometers", "nm"),
    ("mile", "mi"),
    ("miles", "mi"),
    ("foot", "ft"),
    ("feet", "ft"),
    ("inch", "in"),
    ("inches", "in"),
    ("yard", "yd"),
    ("yards", "yd"),
    ("gram", "g"),
    ("grams", "g"),
    ("kilogram", "kg"),
    ("kilograms", "kg"),
    ("milligram", "mg"),
    ("milligrams", "mg"),
    ("tonne", "t"),
    ("tonnes", "t"),
    ("ounce", "oz"),
    ("pound", "lb"),
    ("pounds", "lb"),
    ("ounces", "oz"),
    ("liter", "l"),
    ("liters", "l"),
    ("litre", "l"),
    ("litres", "l"),
    ("milliliter", "ml"),
    ("milliliters", "ml"),
    ("hectare", "ha"),
    ("hectares", "ha"),
    ("acre", "ac"),
    ("acres", "ac"),
    ("hertz", "Hz"),
    ("kilohertz", "kHz"),
    ("megahertz", "MHz"),
    ("gigahertz", "GHz"),
    ("watt", "W"),
    ("watts", "W"),
    ("milliwatt", "mW"),
    ("milliwatts", "mW"),
    ("kilowatt", "kW"),
    ("kilowatts", "kW"),
    ("megawatt", "MW"),
    ("megawatts", "MW"),
    ("gigawatt", "GW"),
    ("gigawatts", "GW"),
    ("volt", "V"),
    ("volts", "V"),
    ("kilovolt", "kV"),
    ("kilovolts", "kV"),
    ("byte", "B"),
    ("bytes", "B"),
    ("kilobyte", "kB"),
    ("kilobytes", "kB"),
    ("megabyte", "MB"),
    ("megabytes", "MB"),
    ("gigabyte", "GB"),
    ("gigabytes", "GB"),
    ("terabyte", "TB"),
    ("terabytes", "TB"),
    ("horsepower", "hp"),
    ("lumen", "lm"),
    ("lumens", "lm"),
    ("calorie", "cal"),
    ("calories", "cal"),
    ("kilocalorie", "kcal"),
    ("kilocalories", "kcal"),
];

const POWERS: &[(&str, &str)] = &[("square", "²"), ("cubic", "³")];

const SI_PREFIXES: &[(&str, &str)] = &[
    ("kilo", "k"),
    ("mega", "M"),
    ("giga", "G"),
    ("tera", "T"),
    ("peta", "P"),
    ("hecto", "h"),
    ("deci", "d"),
    ("centi", "c"),
    ("milli", "m"),
    ("micro", "µ"),
    ("nano", "n"),
    ("c", "c"),
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
    if word == "minus" {
        return Some(DOMAIN_NEGATIVE);
    }
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

pub fn literal_words() -> impl Iterator<Item = (&'static str, Role)> {
    LITERAL_CLASSES
        .iter()
        .flat_map(|c| c.words.iter().map(move |w| (w.0, c.role)))
}

pub struct ClassDef {
    pub class: ClassId,
    pub role: Role,
    pub words: &'static [(&'static str, u32)],
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
        class: OCLOCK,
        role: ROLE_MER,
        words: &[("o'clock", 0), ("oclock", 0)],
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
        class: THE,
        role: 0,
        words: &[("the", 0)],
    },
    ClassDef {
        class: PER,
        role: 0,
        words: &[("per", 0)],
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
const NEGATIVE_SAMPLES: usize = 1000;
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
    ("negative", 0.1),
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

pub fn kind_mass(kind: &str) -> f64 {
    KIND_MASS.iter().find(|k| k.0 == kind).map_or(0.0, |k| k.1)
}

pub struct Example {
    pub words: Vec<&'static str>,
    pub src: u8,
    pub weight: f64,
    pub kind: &'static str,
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
            s.push(words, SRC_CARDINAL, w, "ordinal");
        }
    }
    let hundreds = hundreds_values();
    let w = 0.2 / hundreds.len() as f64;
    for &n in &hundreds {
        let mut words = hundreds_style(n, false);
        if ordinalize(&mut words, (n % 100) as usize).is_some() {
            s.push(words, SRC_CARDINAL, w, "ordinal");
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

pub fn year(y: u32, style: u64) -> Vec<&'static str> {
    let (hi, lo) = (y / 100, (y % 100) as usize);
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

fn negatives(s: &mut Sink) {
    let w = 1.0 / NEGATIVE_SAMPLES as f64;
    for _ in 0..NEGATIVE_SAMPLES {
        let mut words = vec!["minus"];
        words.extend(amount(s, AFFIX_MAX_DIGITS, 30));
        s.push(words, SRC_CARDINAL, w, "negative");
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
                words.push(pick_of(POWERS, r >> 8));
                words.push(POWER_BASES[((r >> 16) % POWER_BASES.len() as u64) as usize]);
            }
            3..=5 => {
                let prefix = pick_of(SI_PREFIXES, r >> 8);
                words.push(prefix);
                words.push(if prefix == "c" {
                    "c"
                } else {
                    PREFIX_BASES[((r >> 16) % PREFIX_BASES.len() as u64) as usize]
                });
            }
            _ => words.push(pick_of(UNITS_SI, r)),
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

fn coverage(s: &mut Sink) {
    for (kind, list) in [
        ("decimal", coverage_decimals()),
        ("cardinal", coverage_cardinals()),
        ("scaled", coverage_scaled()),
    ] {
        let w = COVERAGE_WEIGHT / list.len() as f64;
        for words in list {
            s.push(words, SRC_CARDINAL, w, kind);
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
    negatives(&mut s);
    measures(&mut s);
    times(&mut s);
    identifiers(&mut s);
    coverage(&mut s);
    s.out
}

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
            .chain(literal_words().map(|w| w.0))
            .collect();
        for e in examples() {
            for w in e.words {
                assert!(words.contains(w), "{w}");
            }
        }
    }
}
