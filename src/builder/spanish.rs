use super::language::{
    ClassDef, Classes, Example, Language, LiteralClassDef, Literals, NumberStyle, SRC_CARDINAL,
    SRC_IDENTIFIER, SRC_TIME, VocabSpec, WordKind,
};
use crate::hash::next_seed;
use crate::types::{
    CLASS_UNKNOWN, ClassId, DOMAIN_CENTS, DOMAIN_MEASURE, DOMAIN_MONEY, DOMAIN_PERCENT, DomainId,
    MAX_TOKENS, ROLE_AMPM, ROLE_AND, ROLE_CURRENCY, ROLE_DIGIT, ROLE_HUNDRED, ROLE_MINUS,
    ROLE_MSCALE, ROLE_OH, ROLE_POINT, ROLE_SCALE, ROLE_TEEN, ROLE_TENS, ROLE_UNIT, Setting, kind,
    name, sense,
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
pub const AM: ClassId = 28;
pub const PM: ClassId = 29;

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
    "metro",
    "metros",
    "kilómetro",
    "kilómetros",
    "centímetro",
    "centímetros",
    "milímetro",
    "milímetros",
    "gramo",
    "gramos",
    "kilogramo",
    "kilogramos",
    "kilo",
    "kilos",
    "miligramo",
    "miligramos",
    "tonelada",
    "toneladas",
    "litro",
    "litros",
    "mililitro",
    "mililitros",
    "hectárea",
    "hectáreas",
    "milla",
    "millas",
    "pie",
    "pies",
    "pulgada",
    "pulgadas",
    "libra",
    "libras",
    "onza",
    "onzas",
    "vatio",
    "vatios",
    "kilovatio",
    "kilovatios",
    "voltio",
    "voltios",
    "byte",
    "bytes",
    "megabyte",
    "megabytes",
    "gigabyte",
    "gigabytes",
    "caloría",
    "calorías",
    "grado",
    "grados",
    "hora",
    "horas",
    "minuto",
    "minutos",
    "segundos",
    "día",
    "días",
    "semana",
    "semanas",
    "mes",
    "meses",
    "año",
    "años",
];

pub const LITERAL_CLASSES: &[LiteralClassDef] = &[
    LiteralClassDef {
        class: DOLLAR,
        role: ROLE_CURRENCY,
        words: &[
            ("dólar", "$"),
            ("dólares", "$"),
            ("dolar", "$"),
            ("dolares", "$"),
            ("peso", "$"),
            ("pesos", "$"),
            ("euro", "€"),
            ("euros", "€"),
            ("usd", "US$"),
            ("cad", "CA$"),
            ("mxn", "Mex$"),
            ("yen", "¥"),
            ("yenes", "¥"),
            ("won", "₩"),
            ("wones", "₩"),
            ("peseta", "₧"),
            ("pesetas", "₧"),
            ("colón", "₡"),
            ("colones", "₡"),
            ("quetzal", "Q"),
            ("quetzales", "Q"),
            ("rupia", "₹"),
            ("rupias", "₹"),
        ],
    },
    LiteralClassDef {
        class: CENT,
        role: ROLE_CURRENCY,
        words: &[
            ("centavo", "$"),
            ("centavos", "$"),
            ("céntimo", "€"),
            ("céntimos", "€"),
        ],
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
        class: MINUS,
        role: ROLE_MINUS,
        words: &[("menos", "-")],
    },
    LiteralClassDef {
        class: PERCENT,
        role: ROLE_UNIT,
        words: &[("porciento", " %")],
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
            ("un", 1),
            ("uno", 1),
            ("una", 1),
            ("dos", 2),
            ("tres", 3),
            ("cuatro", 4),
            ("cinco", 5),
            ("seis", 6),
            ("siete", 7),
            ("ocho", 8),
            ("nueve", 9),
        ],
    },
    ClassDef {
        class: ZERO,
        role: ROLE_OH,
        words: &[("cero", 0)],
    },
    ClassDef {
        class: TEEN,
        role: ROLE_TEEN,
        words: &[
            ("diez", 10),
            ("once", 11),
            ("doce", 12),
            ("trece", 13),
            ("catorce", 14),
            ("quince", 15),
            ("dieciséis", 16),
            ("dieciseis", 16),
            ("diecisiete", 17),
            ("dieciocho", 18),
            ("diecinueve", 19),
            ("veintiuno", 21),
            ("veintiún", 21),
            ("veintiun", 21),
            ("veintiuna", 21),
            ("veintidós", 22),
            ("veintidos", 22),
            ("veintitrés", 23),
            ("veintitres", 23),
            ("veinticuatro", 24),
            ("veinticinco", 25),
            ("veintiséis", 26),
            ("veintiseis", 26),
            ("veintisiete", 27),
            ("veintiocho", 28),
            ("veintinueve", 29),
        ],
    },
    ClassDef {
        class: TENS,
        role: ROLE_TENS,
        words: &[
            ("veinte", 20),
            ("treinta", 30),
            ("cuarenta", 40),
            ("cincuenta", 50),
            ("sesenta", 60),
            ("setenta", 70),
            ("ochenta", 80),
            ("noventa", 90),
        ],
    },
    ClassDef {
        class: HUNDRED,
        role: ROLE_HUNDRED,
        words: &[
            ("cien", 100),
            ("ciento", 100),
            ("doscientos", 200),
            ("doscientas", 200),
            ("trescientos", 300),
            ("trescientas", 300),
            ("cuatrocientos", 400),
            ("cuatrocientas", 400),
            ("quinientos", 500),
            ("quinientas", 500),
            ("seiscientos", 600),
            ("seiscientas", 600),
            ("setecientos", 700),
            ("setecientas", 700),
            ("ochocientos", 800),
            ("ochocientas", 800),
            ("novecientos", 900),
            ("novecientas", 900),
        ],
    },
    ClassDef {
        class: SCALE,
        role: ROLE_SCALE,
        words: &[("mil", 1_000)],
    },
    ClassDef {
        class: MSCALE,
        role: ROLE_MSCALE,
        words: &[
            ("millón", 1_000_000),
            ("millon", 1_000_000),
            ("millones", 1_000_000),
            ("millardo", 1_000_000_000),
            ("millardos", 1_000_000_000),
        ],
    },
    ClassDef {
        class: POINT,
        role: ROLE_POINT,
        words: &[("coma", 0), ("punto", 0)],
    },
    ClassDef {
        class: AND,
        role: ROLE_AND,
        words: &[("y", 0), ("con", 0)],
    },
    ClassDef {
        class: AT,
        role: 0,
        words: &[("las", 0)],
    },
    ClassDef {
        class: TO,
        role: 0,
        words: &[("a", 0)],
    },
    ClassDef {
        class: THE,
        role: 0,
        words: &[("la", 0)],
    },
    ClassDef {
        class: IS,
        role: 0,
        words: &[("es", 0)],
    },
    ClassDef {
        class: NUMBER,
        role: 0,
        words: &[("número", 0), ("numero", 0)],
    },
];

const UNITS: [&str; 10] = [
    "cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve",
];
const TEENS: [&str; 10] = [
    "diez",
    "once",
    "doce",
    "trece",
    "catorce",
    "quince",
    "dieciséis",
    "diecisiete",
    "dieciocho",
    "diecinueve",
];
const TWENTIES: [&str; 10] = [
    "veinte",
    "veintiuno",
    "veintidós",
    "veintitrés",
    "veinticuatro",
    "veinticinco",
    "veintiséis",
    "veintisiete",
    "veintiocho",
    "veintinueve",
];
const TENS_WORDS: [&str; 10] = [
    "",
    "",
    "veinte",
    "treinta",
    "cuarenta",
    "cincuenta",
    "sesenta",
    "setenta",
    "ochenta",
    "noventa",
];
const HUNDREDS: [&str; 10] = [
    "",
    "ciento",
    "doscientos",
    "trescientos",
    "cuatrocientos",
    "quinientos",
    "seiscientos",
    "setecientos",
    "ochocientos",
    "novecientos",
];

fn under_1000(n: u32, before_scale: bool, out: &mut Vec<&'static str>) {
    let (h, r) = ((n / 100) as usize, (n % 100) as usize);
    match (h, r) {
        (1, 0) => out.push("cien"),
        (0, _) => {}
        _ => out.push(HUNDREDS[h]),
    }
    let one = |r: usize| match (r, before_scale) {
        (1, true) => "un",
        (21, true) => "veintiún",
        (21, false) => TWENTIES[1],
        (1, false) => UNITS[1],
        (20..=29, _) => TWENTIES[r - 20],
        (10..=19, _) => TEENS[r - 10],
        _ => UNITS[r],
    };
    match r {
        30.. => {
            out.push(TENS_WORDS[r / 10]);
            if r % 10 > 0 {
                out.extend(["y", one(r % 10)]);
            }
        }
        1..=29 => out.push(one(r)),
        _ => {}
    }
}

pub fn cardinal(n: u64) -> Vec<&'static str> {
    if n == 0 {
        return vec!["cero"];
    }
    let mut out = Vec::new();
    let millions = ((n / 1_000_000) % 1_000_000) as u32;
    match millions {
        0 => {}
        1 => out.extend(["un", "millón"]),
        m => {
            if m >= 1000 {
                if m / 1000 > 1 {
                    under_1000(m / 1000, true, &mut out);
                }
                out.push("mil");
            }
            under_1000(m % 1000, true, &mut out);
            out.push("millones");
        }
    }
    let thousands = ((n / 1000) % 1000) as u32;
    match thousands {
        0 => {}
        1 => out.push("mil"),
        k => {
            under_1000(k, true, &mut out);
            out.push("mil");
        }
    }
    under_1000((n % 1000) as u32, false, &mut out);
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

const EXAMPLE_SEED: u64 = 0x4553_505F_4954_4E31;
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
    ("time", 0.5),
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
        words.push(if self.rand().is_multiple_of(4) {
            "punto"
        } else {
            "coma"
        });
        let r = self.rand();
        if r % 10 < 4 {
            let len = pick(r >> 8, &[(1, 45), (2, 35), (3, 15), (4, 5), (5, 3), (6, 2)]);
            for _ in 0..len {
                let d = (self.rand() % 10) as usize;
                words.push(UNITS[d]);
            }
        } else {
            let n = self.number(3);
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
            words = [&["menos"][..], &words].concat();
        }
        s.push(words, SRC_CARDINAL, 0.2 / 999.0, "cardinal");
    }
    let mut groups = vec![0u64, 2];
    groups.extend(patterns(1000));
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
}

fn patterns(below: u64) -> Vec<u64> {
    let mut seen = std::collections::HashSet::new();
    (1..below)
        .filter(|&g| seen.insert(cardinal(g).iter().map(|w| role_of(w)).collect::<Vec<u8>>()))
        .collect()
}

fn decimals(s: &mut Sink) {
    let w = 1.0 / DECIMAL_SAMPLES as f64;
    for _ in 0..DECIMAL_SAMPLES {
        let r = s.rand();
        let mut words = if r % 10 < 2 {
            vec!["cero"]
        } else {
            cardinal(s.number(4))
        };
        s.fraction(&mut words);
        s.push(words, SRC_CARDINAL, w, "decimal");
    }
    zero_patterns(s, "coma");
    let w = 1.0 / SCALED_SAMPLES as f64;
    for _ in 0..SCALED_SAMPLES {
        let r = s.rand();
        let mut words = match r % 10 {
            0 => vec!["cero"],
            1..=2 => cardinal(1000 + s.number(3)),
            _ => cardinal(s.number(3)),
        };
        if words == ["cero"] || s.rand().is_multiple_of(3) {
            s.fraction(&mut words);
        }
        let one = words == ["un"] || words == ["uno"];
        words.push(match (one, (r >> 20).is_multiple_of(4)) {
            (true, false) => "millón",
            (false, false) => "millones",
            (true, true) => "millardo",
            (false, true) => "millardos",
        });
        if words.first() == Some(&"uno") {
            words[0] = "un";
        }
        s.push(words, SRC_CARDINAL, w, "scaled");
    }
}

fn zero_patterns(s: &mut Sink, point: &'static str) {
    let w = 0.1 / 500.0;
    for len in 1..=6u32 {
        let wholes: &[u64] = match len {
            1..=4 => &[0, 3, 12, 60, 100, 245],
            _ => &[1, 45, 300],
        };
        for zeros in 0..1u32 << len {
            for &whole in wholes {
                let mut words = cardinal(whole);
                words.push(point);
                words.extend((0..len).map(|d| match zeros >> d & 1 {
                    1 => UNITS[0],
                    _ => UNITS[1 + (whole as usize + 7 * d as usize + zeros as usize) % 9],
                }));
                s.push(
                    [&["menos"][..], &words].concat(),
                    SRC_CARDINAL,
                    w,
                    "decimal",
                );
                s.push(words, SRC_CARDINAL, w, "decimal");
            }
        }
    }
}

fn money(s: &mut Sink) {
    let w = 1.0 / MONEY_SAMPLES as f64;
    for _ in 0..MONEY_SAMPLES {
        let r = s.rand();
        let (unit, units, cent, cents) = pick(
            r >> 40,
            &[
                (("dólar", "dólares", "centavo", "centavos"), 5),
                (("peso", "pesos", "centavo", "centavos"), 3),
                (("euro", "euros", "céntimo", "céntimos"), 4),
            ],
        );
        let c = 1 + (r >> 8) % 99;
        let one = |n: u64| if n == 1 { vec!["un"] } else { cardinal(n) };
        let mut words = match r % 20 {
            0..=1 => {
                let mut words = one(c);
                words.push(if c == 1 { cent } else { cents });
                s.push(words, SRC_CARDINAL, w, "money");
                continue;
            }
            2..=7 => one(s.number(3)),
            _ => one(s.number(7)),
        };
        words.push(if words == ["un"] { unit } else { units });
        if (2..=7).contains(&(r % 20)) {
            match r % 20 {
                2..=3 => words.push("y"),
                4..=5 => words.push("con"),
                _ => {}
            }
            words.extend(one(c));
            if !matches!(r % 20, 3 | 5 | 7) {
                words.push(if c == 1 { cent } else { cents });
            }
        }
        s.push(words, SRC_CARDINAL, w, "money");
    }
    let (amounts, cents) = (patterns(1000), patterns(100));
    let w = 0.1 / (amounts.len() * cents.len() * 6) as f64;
    for &a in &amounts {
        for &c in &cents {
            for (joiner, named) in [
                (None, true),
                (Some("y"), true),
                (Some("y"), false),
                (Some("con"), true),
                (Some("con"), false),
            ] {
                let mut words = cardinal(a);
                words.push("dólares");
                words.extend(joiner);
                words.extend(cardinal(c));
                if named {
                    words.push("centavos");
                }
                s.push(words, SRC_CARDINAL, w, "money");
            }
        }
    }
}

fn percents(s: &mut Sink) {
    let w = 1.0 / PERCENT_SAMPLES as f64;
    for _ in 0..PERCENT_SAMPLES {
        let mut words = s.amount(4, 30);
        words.push("porciento");
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

fn hour(h: u32) -> Vec<&'static str> {
    if h == 1 {
        vec!["una"]
    } else {
        cardinal(u64::from(h))
    }
}

fn times(s: &mut Sink) {
    let w = 1.0 / (23.0 * 59.0);
    for h in 1..=23u32 {
        if h <= 12 {
            let meridiem = if h % 2 == 0 { "pm" } else { "am" };
            s.push(
                [&hour(h)[..], &[meridiem]].concat(),
                SRC_TIME,
                w * 5.0,
                "time",
            );
        }
        for m in 1..60u32 {
            let minutes = cardinal(u64::from(m));
            let joined = [&hour(h)[..], &["y"], &minutes].concat();
            if h <= 12 {
                let meridiem = if m % 2 == 0 { "pm" } else { "am" };
                let with = [&joined[..], &[meridiem]].concat();
                s.push(with, SRC_TIME, w * 0.2, "time");
            }
            s.push(joined, SRC_TIME, w * 0.7, "time");
            if h >= 13 && m >= 10 {
                s.push([&hour(h)[..], &minutes].concat(), SRC_TIME, w * 0.3, "time");
            }
        }
    }
}

fn identifiers(s: &mut Sink) {
    let w = 1.0 / (6 * IDENTIFIER_SAMPLES) as f64;
    for len in 3..=8 {
        for _ in 0..IDENTIFIER_SAMPLES {
            let words = (0..len).map(|_| UNITS[(s.rand() % 10) as usize]).collect();
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
    times(&mut s);
    identifiers(&mut s);
    s.out
}

const DIGIT_WORDS: &[&str] = &[
    "un", "uno", "una", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve",
];

const SENSES: &[(u32, &[&str])] = &[
    (sense::COUNT, DIGIT_WORDS),
    (sense::ONE, &["uno", "un", "una"]),
    (sense::AND, &["y"]),
    (sense::MILLION, &["millón", "millones"]),
    (sense::OF, &["de"]),
    (sense::TWENTY, &["veinte"]),
    (sense::FIVE, &["cinco"]),
];

const IDIOMS: &[&[&str]] = &[
    &["cada", "dos", "por", "tres"],
    &["de", "una", "vez"],
    &["mil", "y", "una"],
];

const KINDS: &[WordKind] = &[
    WordKind {
        kind: kind::OF,
        aux: 0,
        words: &["de"],
    },
    WordKind {
        kind: kind::PER,
        aux: 0,
        words: &["por"],
    },
    WordKind {
        kind: kind::FILLER,
        aux: 0,
        words: &["eh", "em", "ehm", "mmm"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 3,
        words: &["mil"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 6,
        words: &["millón", "millon", "millones"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 9,
        words: &["millardo", "millardos"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 12,
        words: &["billón", "billon", "billones"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 18,
        words: &["trillón", "trillon", "trillones"],
    },
];

const NAMES: [&str; name::COUNT] = [
    "uno",
    "dos",
    "cero",
    "mil",
    "millón",
    "porciento",
    "por ciento",
    "veinte",
    "doce",
    "horas",
    "y",
    "ciento",
    "primero",
    "am",
    "punto",
];

pub static LANGUAGE: Language = Language {
    name: "spanish",
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
        time_left: &[AT],
        time_right: &[AM, PM],
        identifier_left: &[NUMBER],
    },
    kinds: KINDS,
    vocab: VocabSpec {
        names: NAMES,
        months: [
            "enero",
            "febrero",
            "marzo",
            "abril",
            "mayo",
            "junio",
            "julio",
            "agosto",
            "septiembre",
            "octubre",
            "noviembre",
            "diciembre",
        ],
        hours: [
            "una", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve", "diez",
            "once", "doce",
        ],
        centuries: [
            "1100s", "1200s", "1300s", "1400s", "1500s", "1600s", "1700s", "1800s", "1900s",
        ],
        zones: &["GMT", "UTC", "CET"],
        not_after_number: "aeouy",
        not_before_number: "aeouy",
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
        currency_after: false,
    },
    senses: SENSES,
    idioms: IDIOMS,
    plural: &[],
    words_first: DIGIT_WORDS,
    suffix_roles: SUFFIX_ROLES,
    prefix_roles: PREFIX_ROLES,
    bind_roles: BIND_ROLES,
    lead_skip_roles: (1 << ROLE_AMPM) | (1 << ROLE_UNIT),
    left_folds: [[TO, THE, AT], [IS, THE, AT]],
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
        assert_eq!(cardinal(0), ["cero"]);
        assert_eq!(cardinal(21), ["veintiuno"]);
        assert_eq!(cardinal(35), ["treinta", "y", "cinco"]);
        assert_eq!(cardinal(100), ["cien"]);
        assert_eq!(cardinal(101), ["ciento", "uno"]);
        assert_eq!(cardinal(1001), ["mil", "uno"]);
        assert_eq!(cardinal(21_000), ["veintiún", "mil"]);
        assert_eq!(cardinal(1_000_000), ["un", "millón"]);
        assert_eq!(
            cardinal(2_500_000),
            ["dos", "millones", "quinientos", "mil"]
        );
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
