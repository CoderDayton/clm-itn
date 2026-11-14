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
    "quilômetro",
    "quilômetros",
    "quilómetro",
    "quilómetros",
    "centímetro",
    "centímetros",
    "milímetro",
    "milímetros",
    "grama",
    "gramas",
    "quilograma",
    "quilogramas",
    "quilo",
    "quilos",
    "miligrama",
    "miligramas",
    "tonelada",
    "toneladas",
    "litro",
    "litros",
    "mililitro",
    "mililitros",
    "hectare",
    "hectares",
    "milha",
    "milhas",
    "pé",
    "pés",
    "polegada",
    "polegadas",
    "libra",
    "libras",
    "onça",
    "onças",
    "watt",
    "watts",
    "quilowatt",
    "quilowatts",
    "volt",
    "volts",
    "byte",
    "bytes",
    "megabyte",
    "megabytes",
    "gigabyte",
    "gigabytes",
    "caloria",
    "calorias",
    "grau",
    "graus",
    "hora",
    "horas",
    "minuto",
    "minutos",
    "segundos",
    "dia",
    "dias",
    "semana",
    "semanas",
    "mês",
    "meses",
    "ano",
    "anos",
];

pub const LITERAL_CLASSES: &[LiteralClassDef] = &[
    LiteralClassDef {
        class: DOLLAR,
        role: ROLE_CURRENCY,
        words: &[
            ("real", "R$ "),
            ("reais", "R$ "),
            ("dólar", "$ "),
            ("dólares", "$ "),
            ("dolar", "$ "),
            ("dolares", "$ "),
            ("euro", "€ "),
            ("euros", "€ "),
            ("usd", "US$ "),
            ("cad", "CA$ "),
            ("gbp", "£ "),
            ("iene", "¥ "),
            ("ienes", "¥ "),
            ("peso", "$ "),
            ("pesos", "$ "),
        ],
    },
    LiteralClassDef {
        class: CENT,
        role: ROLE_CURRENCY,
        words: &[
            ("centavo", "R$ "),
            ("centavos", "R$ "),
            ("cêntimo", "€ "),
            ("cêntimos", "€ "),
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
        words: &[("porcento", " %")],
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
            ("um", 1),
            ("uma", 1),
            ("dois", 2),
            ("duas", 2),
            ("três", 3),
            ("tres", 3),
            ("quatro", 4),
            ("cinco", 5),
            ("seis", 6),
            ("sete", 7),
            ("oito", 8),
            ("nove", 9),
        ],
    },
    ClassDef {
        class: ZERO,
        role: ROLE_OH,
        words: &[("zero", 0)],
    },
    ClassDef {
        class: TEEN,
        role: ROLE_TEEN,
        words: &[
            ("dez", 10),
            ("onze", 11),
            ("doze", 12),
            ("treze", 13),
            ("catorze", 14),
            ("quatorze", 14),
            ("quinze", 15),
            ("dezesseis", 16),
            ("dezasseis", 16),
            ("dezessete", 17),
            ("dezassete", 17),
            ("dezoito", 18),
            ("dezenove", 19),
            ("dezanove", 19),
        ],
    },
    ClassDef {
        class: TENS,
        role: ROLE_TENS,
        words: &[
            ("vinte", 20),
            ("trinta", 30),
            ("quarenta", 40),
            ("cinquenta", 50),
            ("cinqüenta", 50),
            ("sessenta", 60),
            ("setenta", 70),
            ("oitenta", 80),
            ("noventa", 90),
        ],
    },
    ClassDef {
        class: HUNDRED,
        role: ROLE_HUNDRED,
        words: &[
            ("cem", 100),
            ("cento", 100),
            ("duzentos", 200),
            ("duzentas", 200),
            ("trezentos", 300),
            ("trezentas", 300),
            ("quatrocentos", 400),
            ("quatrocentas", 400),
            ("quinhentos", 500),
            ("quinhentas", 500),
            ("seiscentos", 600),
            ("seiscentas", 600),
            ("setecentos", 700),
            ("setecentas", 700),
            ("oitocentos", 800),
            ("oitocentas", 800),
            ("novecentos", 900),
            ("novecentas", 900),
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
            ("milhão", 1_000_000),
            ("milhao", 1_000_000),
            ("milhões", 1_000_000),
            ("milhoes", 1_000_000),
            ("bilhão", 1_000_000_000),
            ("bilhao", 1_000_000_000),
            ("bilhões", 1_000_000_000),
            ("bilhoes", 1_000_000_000),
        ],
    },
    ClassDef {
        class: POINT,
        role: ROLE_POINT,
        words: &[("vírgula", 0), ("virgula", 0), ("ponto", 0)],
    },
    ClassDef {
        class: AND,
        role: ROLE_AND,
        words: &[("e", 0), ("com", 0)],
    },
    ClassDef {
        class: AT,
        role: 0,
        words: &[("às", 0), ("à", 0)],
    },
    ClassDef {
        class: TO,
        role: 0,
        words: &[("até", 0)],
    },
    ClassDef {
        class: THE,
        role: 0,
        words: &[("as", 0)],
    },
    ClassDef {
        class: IS,
        role: 0,
        words: &[("são", 0)],
    },
    ClassDef {
        class: NUMBER,
        role: 0,
        words: &[("número", 0), ("numero", 0)],
    },
];

const UNITS: [&str; 10] = [
    "zero", "um", "dois", "três", "quatro", "cinco", "seis", "sete", "oito", "nove",
];
const TEENS: [&str; 10] = [
    "dez",
    "onze",
    "doze",
    "treze",
    "catorze",
    "quinze",
    "dezesseis",
    "dezessete",
    "dezoito",
    "dezenove",
];
const TENS_WORDS: [&str; 10] = [
    "",
    "",
    "vinte",
    "trinta",
    "quarenta",
    "cinquenta",
    "sessenta",
    "setenta",
    "oitenta",
    "noventa",
];
const HUNDREDS: [&str; 10] = [
    "",
    "cento",
    "duzentos",
    "trezentos",
    "quatrocentos",
    "quinhentos",
    "seiscentos",
    "setecentos",
    "oitocentos",
    "novecentos",
];

fn under_1000(n: u32, out: &mut Vec<&'static str>) {
    let (h, r) = ((n / 100) as usize, (n % 100) as usize);
    match (h, r) {
        (0, _) => {}
        (1, 0) => out.push("cem"),
        _ => out.push(HUNDREDS[h]),
    }
    if h > 0 && r > 0 {
        out.push("e");
    }
    match r {
        20.. => {
            out.push(TENS_WORDS[r / 10]);
            if r % 10 > 0 {
                out.extend(["e", UNITS[r % 10]]);
            }
        }
        10..=19 => out.push(TEENS[r - 10]),
        1..=9 => out.push(UNITS[r]),
        _ => {}
    }
}

pub fn cardinal(n: u64) -> Vec<&'static str> {
    if n == 0 {
        return vec!["zero"];
    }
    const GROUPS: [(u64, &str, &str); 3] = [
        (1_000_000_000, "bilhão", "bilhões"),
        (1_000_000, "milhão", "milhões"),
        (1_000, "mil", "mil"),
    ];
    let mut parts: Vec<(u32, Vec<&'static str>)> = Vec::new();
    for (size, one, many) in GROUPS {
        let g = ((n / size) % 1000) as u32;
        let mut words = Vec::new();
        match g {
            0 => continue,
            1 if size == 1_000 => {}
            1 => words.push("um"),
            _ => under_1000(g, &mut words),
        }
        words.push(if g == 1 { one } else { many });
        parts.push((g, words));
    }
    let u = (n % 1000) as u32;
    if u > 0 {
        let mut words = Vec::new();
        under_1000(u, &mut words);
        parts.push((u, words));
    }
    let mut out = Vec::new();
    let last = parts.len() - 1;
    for (k, (g, words)) in parts.into_iter().enumerate() {
        if k == last && k > 0 && (g < 100 || g % 100 == 0) {
            out.push("e");
        }
        out.extend(words);
    }
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
    let mut run = words.to_vec();
    let mut k = 1;
    while run.len() > MAX_TOKENS && k + 1 < run.len() {
        let bind = match role_of(run[k - 1]) {
            ROLE_HUNDRED | ROLE_SCALE | ROLE_MSCALE => true,
            ROLE_CURRENCY => run.len() - 1 <= MAX_TOKENS,
            _ => false,
        };
        if run[k] == "e" && bind {
            run.remove(k);
        } else {
            k += 1;
        }
    }
    let starts = |w: &str| {
        matches!(
            role_of(w),
            ROLE_DIGIT | ROLE_TEEN | ROLE_TENS | ROLE_OH | ROLE_HUNDRED
        )
    };
    let scale = |w: &str| matches!(role_of(w), ROLE_SCALE | ROLE_MSCALE);
    let mut out = Vec::new();
    let mut pos = 0;
    while run.len() - pos > MAX_TOKENS {
        let cut = (pos..run.len() - 1).find(|&j| scale(run[j]) && starts(run[j + 1]));
        match cut {
            Some(j) if j - pos < MAX_TOKENS => {
                out.push(run[pos..=j].to_vec());
                pos = j + 1;
            }
            _ => return Vec::new(),
        }
    }
    out.push(run[pos..].to_vec());
    out
}

const EXAMPLE_SEED: u64 = 0x5054_425F_4954_4E31;
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
            "ponto"
        } else {
            "vírgula"
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
    let w = 0.05 / 72.0;
    for t in 2..10 {
        for d in 1..10 {
            s.push(vec![TENS_WORDS[t], UNITS[d]], SRC_CARDINAL, w, "cardinal");
            s.push(
                vec![UNITS[t % 4], "vírgula", TENS_WORDS[t], UNITS[d]],
                SRC_CARDINAL,
                w,
                "decimal",
            );
            s.push(
                vec![HUNDREDS[d], "e", TENS_WORDS[t], UNITS[d]],
                SRC_CARDINAL,
                w,
                "cardinal",
            );
        }
    }
}

fn decimals(s: &mut Sink) {
    let w = 1.0 / DECIMAL_SAMPLES as f64;
    for _ in 0..DECIMAL_SAMPLES {
        let r = s.rand();
        let mut words = if r % 10 < 2 {
            vec!["zero"]
        } else {
            cardinal(s.number(4))
        };
        s.fraction(&mut words);
        s.push(words, SRC_CARDINAL, w, "decimal");
    }
    zero_patterns(s, "vírgula");
    let w = 1.0 / SCALED_SAMPLES as f64;
    for _ in 0..SCALED_SAMPLES {
        let r = s.rand();
        let mut words = match r % 10 {
            0 => vec!["zero"],
            1..=2 => cardinal(1000 + s.number(3)),
            _ => cardinal(s.number(3)),
        };
        if words == ["zero"] || s.rand().is_multiple_of(3) {
            s.fraction(&mut words);
        }
        let one = words == ["um"];
        words.push(match (one, (r >> 20).is_multiple_of(4)) {
            (true, false) => "milhão",
            (false, false) => "milhões",
            (true, true) => "bilhão",
            (false, true) => "bilhões",
        });
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
                (("real", "reais", "centavo", "centavos"), 5),
                (("dólar", "dólares", "centavo", "centavos"), 4),
                (("euro", "euros", "cêntimo", "cêntimos"), 3),
            ],
        );
        let c = 1 + (r >> 8) % 99;
        let mut words = match r % 20 {
            0..=1 => {
                let mut words = cardinal(c);
                words.push(if c == 1 { cent } else { cents });
                s.push(words, SRC_CARDINAL, w, "money");
                continue;
            }
            2..=7 => cardinal(s.number(3)),
            _ => cardinal(s.number(7)),
        };
        words.push(if words == ["um"] { unit } else { units });
        if (2..=7).contains(&(r % 20)) {
            match r % 20 {
                2..=3 => words.push("e"),
                4..=5 => words.push("com"),
                _ => {}
            }
            words.extend(cardinal(c));
            if !matches!(r % 20, 3 | 5 | 7) {
                words.push(if c == 1 { cent } else { cents });
            }
        }
        s.push(words, SRC_CARDINAL, w, "money");
    }
}

fn percents(s: &mut Sink) {
    let w = 1.0 / PERCENT_SAMPLES as f64;
    for _ in 0..PERCENT_SAMPLES {
        let mut words = s.amount(4, 30);
        words.push("porcento");
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
    match h {
        1 => vec!["uma"],
        2 => vec!["duas"],
        _ => cardinal(u64::from(h)),
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
            let joined = [&hour(h)[..], &["e"], &minutes].concat();
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
    "um", "uma", "dois", "duas", "três", "tres", "quatro", "cinco", "seis", "sete", "oito", "nove",
];

const SENSES: &[(u32, &[&str])] = &[
    (sense::COUNT, DIGIT_WORDS),
    (sense::ONE, &["um", "uma"]),
    (sense::AND, &["e"]),
    (sense::MILLION, &["milhão", "milhões"]),
    (sense::OF, &["de"]),
    (sense::TWENTY, &["vinte"]),
    (sense::FIVE, &["cinco"]),
];

const IDIOMS: &[&[&str]] = &[&["de", "uma", "vez"], &["cada", "um"], &["cada", "uma"]];

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
        words: &["hã", "hum", "ahn", "hmm"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 3,
        words: &["mil"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 6,
        words: &["milhão", "milhao", "milhões", "milhoes"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 9,
        words: &["bilhão", "bilhao", "bilhões", "bilhoes"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 12,
        words: &["trilhão", "trilhao", "trilhões", "trilhoes"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 15,
        words: &["quatrilhão", "quatrilhao", "quatrilhões", "quatrilhoes"],
    },
    WordKind {
        kind: kind::SCALE,
        aux: 18,
        words: &["quintilhão", "quintilhao", "quintilhões", "quintilhoes"],
    },
];

const NAMES: [&str; name::COUNT] = [
    "um",
    "dois",
    "zero",
    "mil",
    "bilhão",
    "porcento",
    "por cento",
    "vinte",
    "doze",
    "horas",
    "e",
    "cento",
    "primeiro",
    "am",
    "vírgula",
];

pub static LANGUAGE: Language = Language {
    name: "portuguese",
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
        time_right: &[AM, PM],
        identifier_left: &[NUMBER],
    },
    kinds: KINDS,
    vocab: VocabSpec {
        names: NAMES,
        months: [
            "janeiro",
            "fevereiro",
            "março",
            "abril",
            "maio",
            "junho",
            "julho",
            "agosto",
            "setembro",
            "outubro",
            "novembro",
            "dezembro",
        ],
        hours: [
            "uma", "duas", "três", "quatro", "cinco", "seis", "sete", "oito", "nove", "dez",
            "onze", "doze",
        ],
        centuries: [
            "1100s", "1200s", "1300s", "1400s", "1500s", "1600s", "1700s", "1800s", "1900s",
        ],
        zones: &["GMT", "UTC", "BRT"],
        not_after_number: "aeou",
        not_before_number: "aeou",
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
        assert_eq!(cardinal(0), ["zero"]);
        assert_eq!(cardinal(23), ["vinte", "e", "três"]);
        assert_eq!(cardinal(100), ["cem"]);
        assert_eq!(cardinal(101), ["cento", "e", "um"]);
        assert_eq!(cardinal(251), ["duzentos", "e", "cinquenta", "e", "um"]);
        assert_eq!(cardinal(1001), ["mil", "e", "um"]);
        assert_eq!(cardinal(1100), ["mil", "e", "cem"]);
        assert_eq!(cardinal(1126), ["mil", "cento", "e", "vinte", "e", "seis"]);
        assert_eq!(cardinal(2022), ["dois", "mil", "e", "vinte", "e", "dois"]);
        assert_eq!(cardinal(1_000_000), ["um", "milhão"]);
        assert_eq!(cardinal(2_000_003), ["dois", "milhões", "e", "três"]);
        assert_eq!(
            cardinal(2_500_000),
            ["dois", "milhões", "e", "quinhentos", "mil"]
        );
    }

    #[test]
    fn long_runs_elide_then_split_at_scales_like_the_engine() {
        assert_eq!(
            windows(&cardinal(147_451)),
            [
                vec!["cento", "quarenta", "e", "sete", "mil"],
                vec!["quatrocentos", "cinquenta", "e", "um"],
            ]
        );
        assert_eq!(windows(&cardinal(251)), [cardinal(251)]);
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
