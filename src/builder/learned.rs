use std::fmt::{self, Write};

use crate::types::DomainId;

pub const HEADER: &str = "clm-learned 1";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClassRef {
    Boundary,
    Unknown,
    Cue(u8),
    Word(String),
}

impl ClassRef {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "^" => Some(Self::Boundary),
            "?" => Some(Self::Unknown),
            _ => {
                if let Some(w) = s.strip_prefix('=') {
                    (!w.is_empty()).then(|| Self::Word(w.to_string()))
                } else {
                    s.strip_prefix('c')?.parse().ok().map(Self::Cue)
                }
            }
        }
    }
}

impl fmt::Display for ClassRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boundary => f.write_str("^"),
            Self::Unknown => f.write_str("?"),
            Self::Cue(k) => write!(f, "c{k}"),
            Self::Word(w) => write!(f, "={w}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub domain: DomainId,
    pub confidence: u8,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub domain: DomainId,
    pub key: u32,
    pub ranked: Vec<(u32, Vec<u32>)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Learned {
    pub comments: Vec<String>,
    pub cues: Vec<(String, u8)>,
    pub pairs: Vec<(ClassRef, ClassRef, Rule)>,
    pub lefts: Vec<(ClassRef, Rule)>,
    pub rights: Vec<(ClassRef, Rule)>,
    pub prior: Option<Rule>,
    pub cells: Vec<Cell>,
    pub shapes: Vec<ShapeCount>,
    pub floor: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeCount {
    pub key: u32,
    pub count: u32,
    pub src: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
}

pub fn parse(text: &str) -> Result<Learned, ParseError> {
    let mut l = Learned::default();
    let mut lines = text.lines().enumerate();
    match lines.next() {
        None => return Ok(l),
        Some((_, h)) if h == HEADER => {}
        Some(_) => return Err(ParseError { line: 1 }),
    }
    for (i, line) in lines {
        let err = ParseError { line: i + 1 };
        if line.is_empty() {
            continue;
        }
        if let Some(c) = line.strip_prefix("# ") {
            l.comments.push(c.to_string());
            continue;
        }
        let f: Vec<&str> = line.split(' ').collect();
        let class = |s: &str| ClassRef::parse(s).ok_or(err);
        match f.as_slice() {
            ["cue", w, k] if !w.is_empty() => {
                l.cues.push((w.to_string(), k.parse().map_err(|_| err)?));
            }
            ["pair", a, b, rule @ ..] => {
                l.pairs.push((class(a)?, class(b)?, parse_rule(rule, err)?))
            }
            ["left", a, rule @ ..] => l.lefts.push((class(a)?, parse_rule(rule, err)?)),
            ["right", a, rule @ ..] => l.rights.push((class(a)?, parse_rule(rule, err)?)),
            ["prior", rule @ ..] if l.prior.is_none() => l.prior = Some(parse_rule(rule, err)?),
            ["cell", d, key, ranked @ ..] if !ranked.is_empty() => {
                let domain = parse_nibble(d, err)?;
                let key = u32::from_str_radix(key, 16).map_err(|_| err)?;
                let ranked = ranked
                    .iter()
                    .map(|c| parse_candidate(c).ok_or(err))
                    .collect::<Result<_, _>>()?;
                l.cells.push(Cell {
                    domain,
                    key,
                    ranked,
                });
            }
            ["shape", key, n, src] => {
                let key = u32::from_str_radix(key, 16).map_err(|_| err)?;
                let src = src.parse::<u8>().ok().filter(|&s| s != 0).ok_or(err)?;
                if key == 0 {
                    return Err(err);
                }
                l.shapes.push(ShapeCount {
                    key,
                    count: n.parse().map_err(|_| err)?,
                    src,
                });
            }
            ["floor", p] if l.floor.is_none() => {
                l.floor = Some(p.parse::<u16>().ok().filter(|&p| p <= 1000).ok_or(err)?);
            }
            _ => return Err(err),
        }
    }
    Ok(l)
}

fn parse_nibble(s: &str, err: ParseError) -> Result<u8, ParseError> {
    s.parse::<u8>().ok().filter(|&v| v <= 15).ok_or(err)
}

fn parse_rule(f: &[&str], err: ParseError) -> Result<Rule, ParseError> {
    let [d, c, n] = f else {
        return Err(err);
    };
    Ok(Rule {
        domain: parse_nibble(d, err)?,
        confidence: parse_nibble(c, err)?,
        count: n.parse().map_err(|_| err)?,
    })
}

fn parse_candidate(s: &str) -> Option<(u32, Vec<u32>)> {
    let (n, ops) = s.split_once(':')?;
    let ops: Option<Vec<u32>> = ops
        .split(',')
        .map(|o| u32::from_str_radix(o, 16).ok())
        .collect();
    Some((n.parse().ok()?, ops.filter(|o| !o.is_empty())?))
}

pub fn write(l: &Learned) -> String {
    let mut s = String::new();
    let rule = |r: &Rule| format!("{} {} {}", r.domain, r.confidence, r.count);
    let _ = writeln!(s, "{HEADER}");
    for c in &l.comments {
        let _ = writeln!(s, "# {c}");
    }
    for (w, k) in &l.cues {
        let _ = writeln!(s, "cue {w} {k}");
    }
    for (a, b, r) in &l.pairs {
        let _ = writeln!(s, "pair {a} {b} {}", rule(r));
    }
    for (a, r) in &l.lefts {
        let _ = writeln!(s, "left {a} {}", rule(r));
    }
    for (a, r) in &l.rights {
        let _ = writeln!(s, "right {a} {}", rule(r));
    }
    if let Some(r) = &l.prior {
        let _ = writeln!(s, "prior {}", rule(r));
    }
    for c in &l.cells {
        let _ = write!(s, "cell {} {:x}", c.domain, c.key);
        for (n, ops) in &c.ranked {
            let ops: Vec<String> = ops.iter().map(|o| format!("{o:08x}")).collect();
            let _ = write!(s, " {n}:{}", ops.join(","));
        }
        s.push('\n');
    }
    for c in &l.shapes {
        let _ = writeln!(s, "shape {:x} {} {}", c.key, c.count, c.src);
    }
    if let Some(p) = l.floor {
        let _ = writeln!(s, "floor {p}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Learned {
        let r = |domain, confidence, count| Rule {
            domain,
            confidence,
            count,
        };
        Learned {
            comments: vec!["tau 3".into()],
            cues: vec![("around".into(), 0), ("of".into(), 1)],
            pairs: vec![(ClassRef::Cue(0), ClassRef::Word("pm".into()), r(1, 14, 90))],
            lefts: vec![(ClassRef::Boundary, r(0, 9, 400))],
            rights: vec![(ClassRef::Cue(1), r(5, 12, 70))],
            prior: Some(r(0, 6, 1000)),
            cells: vec![Cell {
                domain: 1,
                key: 0x31,
                ranked: vec![(7, vec![0x0101_0001, 0x0002_0003]), (2, vec![0x5])],
            }],
            shapes: vec![
                ShapeCount {
                    key: 0x31,
                    count: 500,
                    src: 3,
                },
                ShapeCount {
                    key: 0x1,
                    count: 90,
                    src: 1,
                },
            ],
            floor: Some(200),
        }
    }

    #[test]
    fn write_then_parse_round_trips() {
        let l = sample();
        let text = write(&l);
        assert!(text.starts_with("clm-learned 1\n# tau 3\ncue around 0\n"));
        assert!(text.contains("pair c0 =pm 1 14 90\n"));
        assert!(text.contains("cell 1 31 7:01010001,00020003 2:00000005\n"));
        assert!(text.contains("shape 31 500 3\nshape 1 90 1\nfloor 200\n"));
        assert_eq!(parse(&text), Ok(l));
    }

    #[test]
    fn empty_text_is_no_learned_data() {
        assert_eq!(parse(""), Ok(Learned::default()));
        assert_eq!(parse("clm-learned 1\n"), Ok(Learned::default()));
    }

    #[test]
    fn bad_lines_report_their_line_number() {
        for (text, line) in [
            ("clm-learned 2\n", 1),
            ("clm-learned 1\nprior 16 3 4\n", 2),
            ("clm-learned 1\nprior 0 3\n", 2),
            ("clm-learned 1\n\npair ^ x 0 1 1\n", 3),
            ("clm-learned 1\ncell 0 zz 1:5\n", 2),
            ("clm-learned 1\ncell 0 31 1:\n", 2),
            ("clm-learned 1\nprior 0 1 1\nprior 0 1 1\n", 3),
            ("clm-learned 1\nwhat\n", 2),
            ("clm-learned 1\nshape 31 5\n", 2),
            ("clm-learned 1\nshape 0 5 1\n", 2),
            ("clm-learned 1\nfloor 1001\n", 2),
            ("clm-learned 1\nfloor 1\nfloor 1\n", 3),
        ] {
            assert_eq!(parse(text), Err(ParseError { line }), "{text:?}");
        }
    }
}
