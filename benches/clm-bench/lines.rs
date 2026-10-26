use std::path::{Path, PathBuf};

pub const DEFAULT_PASSES: usize = 1000;
pub const WORST: usize = 5;
/// JSONL keys for the spoken text, the written form and the tag; first match wins.
pub const TEXT_KEYS: [&str; 5] = ["spoken", "input", "raw", "text", "sentence"];
pub const GOLD_KEYS: [&str; 2] = ["written", "gold"];
pub const TAG_KEYS: [&str; 2] = ["tag", "category"];
pub const UNTAGGED: &str = "(untagged)";
const CORPUS_EXTS: [&str; 3] = ["jsonl", "tsv", "txt"];
const MAX_DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    pub spoken: String,
    pub written: Option<String>,
    pub tag: String,
}

impl Case {
    /// Tags ending in `!` mark lines whose gold is debatable.
    pub fn debatable(&self) -> bool {
        self.tag.ends_with('!')
    }
}

pub struct CorpusFile {
    pub name: String,
    pub cases: Vec<Case>,
    pub malformed: usize,
}

/// One corpus file, or every `.jsonl`, `.tsv` and `.txt` file in a folder,
/// sorted by name. `.jsonl` lines are objects; other files are
/// `spoken [TAB written [TAB tag]]` lines with `#` comments.
pub fn load(path: &str) -> Result<Vec<CorpusFile>, String> {
    let p = Path::new(path);
    if !p.is_dir() {
        return Ok(vec![read_corpus(p)?]);
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(p)
        .map_err(|e| format!("cannot read {path}: {e}"))?
        .flatten()
        .map(|e| e.path())
        .filter(|f| f.is_file() && ext(f).is_some_and(|x| CORPUS_EXTS.contains(&x)))
        .collect();
    if files.is_empty() {
        return Err(format!("no .jsonl, .tsv or .txt files in {path}"));
    }
    files.sort();
    files.iter().map(|f| read_corpus(f)).collect()
}

fn ext(f: &Path) -> Option<&str> {
    f.extension().and_then(|x| x.to_str())
}

fn read_corpus(f: &Path) -> Result<CorpusFile, String> {
    let name = f.display().to_string();
    let text = std::fs::read_to_string(f).map_err(|e| format!("cannot read {name}: {e}"))?;
    let (cases, malformed) = match ext(f) {
        Some("jsonl") => (jsonl_cases(&text).map_err(|e| format!("{name}: {e}"))?, 0),
        _ => tsv_cases(&text),
    };
    if cases.is_empty() {
        return Err(format!("no lines in {name}"));
    }
    Ok(CorpusFile {
        name,
        cases,
        malformed,
    })
}

/// `spoken [TAB written [TAB tag]]` lines. A line with a tab but an empty
/// spoken or written side is malformed.
pub fn tsv_cases(text: &str) -> (Vec<Case>, usize) {
    let mut cases = Vec::new();
    let mut bad = 0;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.splitn(3, '\t');
        let spoken = cols.next().unwrap_or("").trim();
        let written = cols.next().map(str::trim);
        let tag = cols.next().map(str::trim).filter(|t| !t.is_empty());
        if spoken.is_empty() || written == Some("") {
            bad += 1;
            continue;
        }
        cases.push(Case {
            spoken: spoken.to_string(),
            written: written.map(str::to_string),
            tag: tag.unwrap_or(UNTAGGED).to_string(),
        });
    }
    (cases, bad)
}

type Fields = Vec<(String, Option<String>)>;

/// The first string field named in `keys`; an error if that field is not a string.
fn field<'a>(fields: &'a Fields, keys: &[&str]) -> Result<Option<&'a str>, String> {
    match keys
        .iter()
        .find_map(|k| fields.iter().find(|(f, _)| f == k))
    {
        Some((_, Some(s))) => Ok(Some(s)),
        Some((k, None)) => Err(format!("\"{k}\" is not a string")),
        None => Ok(None),
    }
}

/// One case per non-blank JSONL line, from [`TEXT_KEYS`], [`GOLD_KEYS`] and [`TAG_KEYS`].
pub fn jsonl_cases(text: &str) -> Result<Vec<Case>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let at = |e: String| format!("line {}: {e}", i + 1);
        let fields = Json {
            b: line.as_bytes(),
            i: 0,
        }
        .object()
        .map_err(at)?;
        let spoken = field(&fields, &TEXT_KEYS)
            .map_err(at)?
            .ok_or_else(|| at(format!("no {} field", TEXT_KEYS.join("/"))))?;
        out.push(Case {
            spoken: spoken.to_string(),
            written: field(&fields, &GOLD_KEYS).map_err(at)?.map(str::to_string),
            tag: field(&fields, &TAG_KEYS)
                .map_err(at)?
                .unwrap_or(UNTAGGED)
                .to_string(),
        });
    }
    Ok(out)
}

/// Top-level fields of one JSON object; only string values are kept.
struct Json<'a> {
    b: &'a [u8],
    i: usize,
}

impl Json<'_> {
    fn object(mut self) -> Result<Fields, String> {
        let mut fields = Vec::new();
        self.eat(b'{')?;
        if !self.close(b'}') {
            loop {
                let key = self.string()?;
                self.eat(b':')?;
                self.ws();
                let value = match self.peek() {
                    Some(b'"') => Some(self.string()?),
                    _ => self.skip(0).map(|_| None)?,
                };
                fields.push((key, value));
                if self.close(b'}') {
                    break;
                }
                self.eat(b',')?;
            }
        }
        self.ws();
        match self.peek() {
            None => Ok(fields),
            Some(_) => Err(format!("trailing data at byte {}", self.i)),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.peek() != Some(c) {
            return Err(format!("expected '{}' at byte {}", c as char, self.i));
        }
        self.i += 1;
        Ok(())
    }

    fn close(&mut self, c: u8) -> bool {
        self.ws();
        let hit = self.peek() == Some(c);
        self.i += usize::from(hit);
        hit
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut s = String::new();
        loop {
            let start = self.i;
            while !matches!(self.peek(), None | Some(b'"' | b'\\')) {
                self.i += 1;
            }
            // Cut only at ASCII quote or backslash, so the run stays valid UTF-8.
            s.push_str(std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?);
            match self.peek() {
                None => return Err("unterminated string".into()),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(s);
                }
                _ => s.push(self.escape()?),
            }
        }
    }

    fn escape(&mut self) -> Result<char, String> {
        self.i += 1;
        let e = self.peek().ok_or("unterminated escape")?;
        self.i += 1;
        Ok(match e {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => self.unicode()?,
            _ => return Err(format!("bad escape at byte {}", self.i - 1)),
        })
    }

    fn unicode(&mut self) -> Result<char, String> {
        let hi = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&hi) {
            if self.b.get(self.i..self.i + 2) != Some(b"\\u") {
                return Err("lone surrogate".into());
            }
            self.i += 2;
            let lo = self.hex4()?;
            if !(0xDC00..0xE000).contains(&lo) {
                return Err("bad low surrogate".into());
            }
            0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
        } else {
            hi
        };
        char::from_u32(code).ok_or_else(|| format!("bad \\u escape {code:#x}"))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err(format!("bad \\u escape at byte {}", self.i));
        }
        self.i += 4;
        let s = std::str::from_utf8(digits).map_err(|e| e.to_string())?;
        u32::from_str_radix(s, 16).map_err(|e| e.to_string())
    }

    fn skip(&mut self, depth: usize) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err(format!("nested deeper than {MAX_DEPTH}"));
        }
        self.ws();
        match self.peek() {
            Some(b'"') => self.string().map(drop),
            Some(open @ (b'{' | b'[')) => {
                let close = if open == b'{' { b'}' } else { b']' };
                self.i += 1;
                if self.close(close) {
                    return Ok(());
                }
                loop {
                    if open == b'{' {
                        self.string()?;
                        self.eat(b':')?;
                    }
                    self.skip(depth + 1)?;
                    if self.close(close) {
                        return Ok(());
                    }
                    self.eat(b',')?;
                }
            }
            Some(_) => {
                let start = self.i;
                while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || b"+-.".contains(&c))
                {
                    self.i += 1;
                }
                let tok = std::str::from_utf8(&self.b[start..self.i]).unwrap_or("");
                match tok {
                    "true" | "false" | "null" => Ok(()),
                    _ if !tok.is_empty() && tok.parse::<f64>().is_ok() => Ok(()),
                    _ => Err(format!("bad value at byte {start}")),
                }
            }
            None => Err("unexpected end of line".into()),
        }
    }
}

pub fn per_line(samples: &[u32], lines: usize, p: f64) -> Vec<u32> {
    (0..lines)
        .map(|i| {
            let mut v: Vec<u32> = samples.iter().skip(i).step_by(lines).copied().collect();
            v.sort_unstable();
            crate::replay::percentile(&v, p)
        })
        .collect()
}

pub fn worst(per_line: &[u32], k: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..per_line.len()).collect();
    idx.sort_by(|&a, &b| per_line[b].cmp(&per_line[a]));
    idx.truncate(k);
    idx
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllocSummary {
    pub calls: usize,
    pub total: u64,
    pub max: u64,
    pub lines_allocating: usize,
}

pub fn alloc_summary(per_line: &[u64]) -> AllocSummary {
    AllocSummary {
        calls: per_line.len(),
        total: per_line.iter().sum(),
        max: per_line.iter().copied().max().unwrap_or(0),
        lines_allocating: per_line.iter().filter(|&&n| n > 0).count(),
    }
}
