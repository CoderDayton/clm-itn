use std::collections::BTreeMap;

use crate::builder::english::{SRC_CARDINAL, SRC_IDENTIFIER, SRC_TIME};
use crate::builder::learned::{Cell, ClassRef, Learned, Rule, ShapeCount};
use crate::builder::{BuildError, Built, CLASS_IDS, build};
use crate::engine::span::{Affixes, Span, segment};
use crate::tables::Tables;
use crate::types::{
    CLASS_UNKNOWN, CandidateId, CellIndex, ClassId, DOMAIN_CARDINAL, DOMAIN_COUNT, DOMAIN_DATE,
    DOMAIN_IDENTIFIER, DOMAIN_ORDINAL, DOMAIN_PLAIN, DOMAIN_TIME, DomainId, MAX_TOKENS, ROLE_NONE,
    cell_domain, cell_index, cell_shape,
};

pub const MAX_RECORD_TOKENS: usize = 400;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gold<'a> {
    pub start: usize,
    pub len: usize,
    pub class: &'a str,
    pub written: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record<'a> {
    pub tokens: Vec<&'a str>,
    pub gold: Vec<Gold<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairError {
    pub line: usize,
}

pub struct Pairs<'a> {
    lines: std::iter::Enumerate<std::str::SplitInclusive<'a, char>>,
    failed: bool,
}

pub fn pairs(text: &str) -> Pairs<'_> {
    Pairs {
        lines: text.split_inclusive('\n').enumerate(),
        failed: false,
    }
}

impl<'a> Iterator for Pairs<'a> {
    type Item = Result<Record<'a>, PairError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let item = self.read().transpose();
        self.failed = matches!(item, Some(Err(_)));
        item
    }
}

impl<'a> Pairs<'a> {
    fn read(&mut self) -> Result<Option<Record<'a>>, PairError> {
        let at = |i: usize| PairError { line: i + 1 };
        let Some((i, first)) = self.lines.next() else {
            return Ok(None);
        };
        let s = first
            .strip_suffix('\n')
            .and_then(|l| l.strip_prefix("S "))
            .ok_or(at(i))?;
        let tokens: Vec<&str> = s.split(' ').collect();
        if tokens.len() > MAX_RECORD_TOKENS || tokens.iter().any(|t| t.is_empty()) {
            return Err(at(i));
        }
        let mut gold = Vec::new();
        let mut free = 0;
        let mut last = i;
        loop {
            let (j, line) = self.lines.next().ok_or(at(last + 1))?;
            last = j;
            let line = line.strip_suffix('\n').ok_or(at(j))?;
            if line.is_empty() {
                return Ok(Some(Record { tokens, gold }));
            }
            let g = parse_gold(line, tokens.len(), free).ok_or(at(j))?;
            free = g.start + g.len;
            gold.push(g);
        }
    }
}

fn parse_gold(line: &str, n: usize, free: usize) -> Option<Gold<'_>> {
    let mut f = line.strip_prefix("T ")?.splitn(4, ' ');
    let start: usize = f.next()?.parse().ok()?;
    let len: usize = f.next()?.parse().ok()?;
    let class = f.next()?;
    let written = f.next()?;
    let class_ok = !class.is_empty() && class.bytes().all(|b| b.is_ascii_uppercase());
    let fits = len >= 1 && start >= free && start.checked_add(len)? <= n;
    (class_ok && fits && !written.is_empty()).then_some(Gold {
        start,
        len,
        class,
        written,
    })
}

pub type Windows<'w> = &'w dyn Fn(usize, &dyn Fn(usize) -> bool) -> Vec<(usize, usize, bool)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    Domain(DomainId),
    Mismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observed<'a> {
    pub start: usize,
    pub len: usize,
    pub span: Span,
    pub label: Label,
    pub written: Option<&'a str>,
}

pub fn observe<'a>(t: &Tables, rec: &Record<'a>, windows: Windows) -> Vec<Observed<'a>> {
    let classes: Vec<ClassId> = rec
        .tokens
        .iter()
        .map(|w| t.lexicon.lookup(w.as_bytes()).class)
        .collect();
    let numeric = |i: usize| t.role_of[classes[i] as usize] != ROLE_NONE;
    let affixes = Affixes {
        suffix: t.suffix_roles,
        prefix: t.prefix_roles,
        starts: t.starts.0,
        per_stop: t.per_stop.0,
        per_units: t.per_units.0,
        pair_only: t.pair_only.0,
        connectors: t.connector_roles,
        binds: t.bind_roles,
    };
    let known = |k| t.shapes.lookup(k) != crate::types::SHAPE_UNKNOWN;
    let mut out = Vec::new();
    let mut spans = [Span::default(); MAX_TOKENS];
    for (ws, we, raw) in windows(classes.len(), &numeric) {
        if raw {
            continue;
        }
        let n = segment(&classes[ws..we], &t.role_of, affixes, known, &mut spans);
        for span in &spans[..n] {
            let start = ws + span.start as usize;
            let len = span.len as usize;
            let hit = rec
                .gold
                .iter()
                .find(|g| g.start < start + len && start < g.start + g.len);
            let (label, written) = match hit {
                None => (Label::Domain(DOMAIN_PLAIN), None),
                Some(g) if g.start == start && g.len == len => match gold_domain(g.class) {
                    Some(d) => (Label::Domain(d), Some(g.written)),
                    None => (Label::Mismatch, None),
                },
                Some(_) => (Label::Mismatch, None),
            };
            out.push(Observed {
                start,
                len,
                span: *span,
                label,
                written,
            });
        }
    }
    out
}

pub const SRC_SHARE: f64 = 0.05;

#[derive(Clone, Debug, Default)]
pub struct ShapeTally {
    per: BTreeMap<u32, [u32; 3]>,
}

impl ShapeTally {
    pub fn add(&mut self, o: &Observed) {
        let Label::Domain(d) = o.label else { return };
        if d == DOMAIN_PLAIN {
            return;
        }
        let slot = match d {
            DOMAIN_TIME => 1,
            DOMAIN_IDENTIFIER => 2,
            _ => 0,
        };
        self.per.entry(o.span.key).or_default()[slot] += 1;
    }

    pub fn finish(&self) -> Vec<ShapeCount> {
        self.per
            .iter()
            .map(|(&key, &n)| {
                let count: u32 = n.iter().sum();
                let min = (f64::from(count) * SRC_SHARE).max(1.0);
                let src = [SRC_CARDINAL, SRC_TIME, SRC_IDENTIFIER]
                    .iter()
                    .zip(n)
                    .filter(|&(_, c)| f64::from(c) >= min)
                    .fold(0, |s, (bit, _)| s | bit);
                ShapeCount { key, count, src }
            })
            .collect()
    }
}

pub fn count_shapes(observed: &[Observed]) -> Vec<ShapeCount> {
    let mut t = ShapeTally::default();
    observed.iter().for_each(|o| t.add(o));
    t.finish()
}

pub const MIN_COUNT: u32 = 20;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub pairs: Vec<(ClassRef, ClassRef, Rule)>,
    pub lefts: Vec<(ClassRef, Rule)>,
    pub rights: Vec<(ClassRef, Rule)>,
    pub prior: Option<Rule>,
}

type Counts = [u32; DOMAIN_COUNT];

#[derive(Clone, Debug, Default)]
pub struct ContextTally {
    pairs: BTreeMap<(ClassId, ClassId), Counts>,
    lefts: BTreeMap<ClassId, Counts>,
    rights: BTreeMap<ClassId, Counts>,
    all: Counts,
}

impl ContextTally {
    pub fn add(&mut self, o: &Observed) {
        let Label::Domain(d) = o.label else { return };
        if o.span.core < o.span.len {
            return;
        }
        let (l, r, d) = (o.span.left, o.span.right, d as usize);
        self.pairs.entry((l, r)).or_insert([0; DOMAIN_COUNT])[d] += 1;
        self.lefts.entry(l).or_insert([0; DOMAIN_COUNT])[d] += 1;
        self.rights.entry(r).or_insert([0; DOMAIN_COUNT])[d] += 1;
        self.all[d] += 1;
    }

    pub fn finish(&self, built: &Built, plain: Plain) -> Context {
        let c = |class| built.class_ref(class);
        Context {
            pairs: ranked(&self.pairs, plain)
                .into_iter()
                .map(|((l, r), rule)| (c(l), c(r), rule))
                .collect(),
            lefts: ranked(&self.lefts, plain)
                .into_iter()
                .map(|(l, r)| (c(l), r))
                .collect(),
            rights: ranked(&self.rights, plain)
                .into_iter()
                .map(|(k, r)| (c(k), r))
                .collect(),
            prior: rule_of(&self.all, plain),
        }
    }
}

pub fn context_rules(built: &Built, observed: &[Observed]) -> Context {
    let mut t = ContextTally::default();
    observed.iter().for_each(|o| t.add(o));
    t.finish(built, Plain::All)
}

#[derive(Clone, Debug, Default)]
pub struct CellTally {
    cells: BTreeMap<CellIndex, BTreeMap<CandidateId, u32>>,
    pub canonicalized: u64,
}

impl CellTally {
    pub fn add(&mut self, built: &Built, rec: &Record, o: &Observed) {
        let (Label::Domain(gold), Some(written)) = (o.label, o.written) else {
            return;
        };
        let t = &built.tables;
        let shape = t.shapes.lookup(o.span.key);
        if shape == 0 || t.shape_len[shape as usize] != o.span.core {
            return;
        }
        let domain = if o.span.core < o.span.len {
            t.context.lookup(o.span.left, o.span.right).0
        } else {
            gold
        };
        let cell = cell_index(domain, shape);
        let Some(list) = built.lists.get(&cell) else {
            return;
        };
        let words: Vec<&[u8]> = rec.tokens[o.start..o.start + o.len]
            .iter()
            .map(|w| w.as_bytes())
            .collect();
        let values: Vec<u32> = words
            .iter()
            .map(|w| t.value_of[t.lexicon.lookup(w).canonical as usize])
            .collect();
        let written = house_style(written);
        if matches!(written, std::borrow::Cow::Owned(_)) {
            self.canonicalized += 1;
        }
        let mut out = [0u8; RENDER_BYTES];
        let d = cell_domain(cell);
        for &cand in list {
            if t.candidates.span_len(d, cand) as usize != o.len {
                continue;
            }
            let mut pos = 0;
            let ok = t
                .candidates
                .render(d, cand, &values, &words, &mut out, &mut pos);
            if ok.is_ok() && &out[..pos] == written.as_bytes() {
                *self.cells.entry(cell).or_default().entry(cand).or_default() += 1;
            }
        }
    }

    pub fn finish(&self, built: &Built) -> Vec<Cell> {
        let mut out = Vec::new();
        for (&cell, counts) in &self.cells {
            let mut ranked: Vec<(u32, CandidateId)> = counts
                .iter()
                .filter(|&(_, &n)| n >= MIN_COUNT)
                .map(|(&c, &n)| (n, c))
                .collect();
            if ranked.is_empty() {
                continue;
            }
            let list = &built.lists[&cell];
            let pos = |c: CandidateId| list.iter().position(|&x| x == c);
            ranked.sort_by(|a, b| b.0.cmp(&a.0).then(pos(a.1).cmp(&pos(b.1))));
            out.push(Cell {
                domain: cell_domain(cell),
                key: built.keys[cell_shape(cell) as usize],
                ranked: ranked
                    .into_iter()
                    .map(|(n, c)| (n, built.ops[&(cell_domain(cell), c)].clone()))
                    .collect(),
            });
        }
        out
    }
}

const RENDER_BYTES: usize = 256;

pub const MAX_CUE_WORDS: usize = 512;
pub const MAX_CUE_CLASSES: usize = 96;
pub const CUE_MIN_GAIN: f64 = 16.0;
const CUE_DIMS: usize = 2 * DOMAIN_COUNT;
const CUE_MAX_BYTES: usize = 15;

type CueCounts = [u32; CUE_DIMS];

#[derive(Clone, Debug, Default)]
pub struct CueTally<'a> {
    words: BTreeMap<&'a str, CueCounts>,
    unknown: CueCounts,
}

impl<'a> CueTally<'a> {
    pub fn add(&mut self, rec: &Record<'a>, o: &Observed) {
        let Label::Domain(d) = o.label else { return };
        if o.span.core < o.span.len {
            return;
        }
        let d = d as usize & 0x0F;
        if o.span.left == CLASS_UNKNOWN && o.start > 0 {
            self.bump(rec.tokens[o.start - 1], d);
        }
        let end = o.start + o.len;
        if o.span.right == CLASS_UNKNOWN && end < rec.tokens.len() {
            self.bump(rec.tokens[end], DOMAIN_COUNT + d);
        }
    }

    fn bump(&mut self, word: &'a str, dim: usize) {
        self.unknown[dim] += 1;
        let ok = word.len() <= CUE_MAX_BYTES && !word.bytes().any(|b| b.is_ascii_uppercase());
        if ok {
            self.words.entry(word).or_insert([0; CUE_DIMS])[dim] += 1;
        }
    }

    pub fn finish(&self, max_words: usize, max_classes: usize) -> Vec<(String, u8)> {
        let pool = &self.unknown;
        let mut chosen: Vec<(f64, &str, &CueCounts)> = self
            .words
            .iter()
            .filter(|(_, c)| c.iter().sum::<u32>() >= MIN_COUNT)
            .map(|(w, c)| (split_gain(c, pool), *w, c))
            .filter(|g| g.0 >= CUE_MIN_GAIN)
            .collect();
        chosen.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
        chosen.truncate(max_words);
        let mut clusters: Vec<(Vec<&str>, CueCounts)> =
            chosen.iter().map(|&(_, w, c)| (vec![w], *c)).collect();
        merge_clusters(&mut clusters, max_classes.max(1));
        for c in &mut clusters {
            c.0.sort_unstable();
        }
        clusters.sort_by(|a, b| total(&b.1).cmp(&total(&a.1)).then(a.0[0].cmp(b.0[0])));
        let mut out: Vec<(String, u8)> = clusters
            .iter()
            .enumerate()
            .flat_map(|(k, (ws, _))| ws.iter().map(move |w| (w.to_string(), k as u8)))
            .collect();
        out.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        out
    }
}

fn total(c: &CueCounts) -> u64 {
    c.iter().map(|&x| u64::from(x)).sum()
}

fn split_gain(c: &CueCounts, pool: &CueCounts) -> f64 {
    let (n, m) = (total(c) as f64, total(pool) as f64);
    c.iter()
        .zip(pool)
        .filter(|&(&k, _)| k > 0)
        .map(|(&k, &p)| f64::from(k) * ((f64::from(k) / n) / (f64::from(p) / m)).ln())
        .sum()
}

fn merge_cost(a: &CueCounts, b: &CueCounts) -> f64 {
    let mut ab = [0u32; CUE_DIMS];
    for (i, x) in ab.iter_mut().enumerate() {
        *x = a[i] + b[i];
    }
    split_gain(a, &ab) + split_gain(b, &ab)
}

fn merge_clusters(clusters: &mut Vec<(Vec<&str>, CueCounts)>, max: usize) {
    let n = clusters.len();
    let mut cost = vec![vec![f64::INFINITY; n]; n];
    for i in 0..n {
        for j in i + 1..n {
            cost[i][j] = merge_cost(&clusters[i].1, &clusters[j].1);
        }
    }
    let mut alive = vec![true; n];
    let mut count = n;
    while count > max {
        let mut best = (f64::INFINITY, 0, 0);
        for i in 0..n {
            if !alive[i] {
                continue;
            }
            for j in i + 1..n {
                if alive[j] && cost[i][j] < best.0 {
                    best = (cost[i][j], i, j);
                }
            }
        }
        let (_, i, j) = best;
        let moved = std::mem::take(&mut clusters[j].0);
        clusters[i].0.extend(moved);
        let add = clusters[j].1;
        for (x, y) in clusters[i].1.iter_mut().zip(add) {
            *x += y;
        }
        alive[j] = false;
        count -= 1;
        for k in 0..n {
            if alive[k] && k != i {
                let c = merge_cost(&clusters[i].1, &clusters[k].1);
                let (lo, hi) = if k < i { (k, i) } else { (i, k) };
                cost[lo][hi] = c;
            }
        }
    }
    let mut keep = alive.iter();
    clusters.retain(|_| *keep.next().unwrap_or(&false));
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DomainEval {
    pub spans: u32,
    pub correct: u32,
    pub abstained: u32,
}

#[derive(Clone, Debug)]
pub struct RouteTally {
    n: Box<[[[u32; 16]; DOMAIN_COUNT]; DOMAIN_COUNT]>,
}

impl Default for RouteTally {
    fn default() -> Self {
        Self {
            n: Box::new([[[0; 16]; DOMAIN_COUNT]; DOMAIN_COUNT]),
        }
    }
}

impl RouteTally {
    pub fn add(&mut self, built: &Built, o: &Observed) {
        let Label::Domain(gold) = o.label else { return };
        if o.span.core < o.span.len {
            return;
        }
        let (predicted, confidence) = built.tables.context.lookup(o.span.left, o.span.right);
        self.count(gold, predicted, confidence);
    }

    pub fn count(&mut self, gold: DomainId, predicted: DomainId, confidence: u8) {
        self.n[gold as usize & 0x0F][predicted as usize & 0x0F][confidence as usize & 0x0F] += 1;
    }

    pub fn wrong_rate(&self, tau: u8) -> f64 {
        let (mut wrong, mut total) = (0u64, 0u64);
        for (g, per) in self.n.iter().enumerate() {
            for (p, confs) in per.iter().enumerate() {
                for (c, &k) in confs.iter().enumerate() {
                    total += u64::from(k);
                    if p != g && c >= tau as usize {
                        wrong += u64::from(k);
                    }
                }
            }
        }
        if total == 0 {
            0.0
        } else {
            wrong as f64 / total as f64
        }
    }

    pub fn domain(&self, gold: DomainId, tau: u8) -> DomainEval {
        let mut e = DomainEval::default();
        for (p, confs) in self.n[gold as usize & 0x0F].iter().enumerate() {
            for (c, &k) in confs.iter().enumerate() {
                e.spans += k;
                if c < tau as usize {
                    e.abstained += k;
                } else if p == gold as usize {
                    e.correct += k;
                }
            }
        }
        e
    }
}

pub fn pick_tau(r: &RouteTally, max_wrong: f64) -> u8 {
    (0..=15)
        .find(|&t| r.wrong_rate(t) <= max_wrong)
        .unwrap_or(15)
}

pub const FLOORS: [u16; 7] = [0, 50, 100, 200, 300, 500, 1000];
pub const MAX_WRONG: f64 = 0.005;
pub const DEFAULT_FLOOR: u16 = 1000;

pub type Format<'f> = &'f dyn Fn(&Tables, &[String]) -> Vec<String>;

pub fn exact_sentences(format: Format, built: &Built, text: &str) -> (u64, u64) {
    let recs: Vec<Record> = pairs(text).map_while(Result::ok).collect();
    let lines: Vec<String> = recs.iter().map(|r| r.tokens.join(" ")).collect();
    let got = format(&built.tables, &lines);
    let exact = recs
        .iter()
        .zip(&got)
        .filter(|(r, g)| **g == written_sentence(r))
        .count();
    (exact as u64, recs.len() as u64)
}

pub fn pick_floor(sweep: &[(u16, f64)]) -> u16 {
    let best = sweep.iter().map(|f| f.1).fold(f64::MIN, f64::max);
    sweep
        .iter()
        .filter(|f| f.1 >= best)
        .map(|f| f.0)
        .max()
        .unwrap_or(DEFAULT_FLOOR)
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub learned: Learned,
    pub tau: u8,
    pub before: RouteTally,
    pub after: RouteTally,
    pub floors: Vec<(u16, f64)>,
    pub canonicalized: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LearnError {
    Train(PairError),
    Val(PairError),
    Build(BuildError),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Plain {
    All,
    #[default]
    Strong,
    Off,
}

pub const STRONG_PLAIN: f64 = 0.9;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub plain: Plain,
}

pub fn learn(
    train: &str,
    val: &str,
    windows: Windows,
    format: Format,
) -> Result<Outcome, LearnError> {
    learn_with(train, val, windows, format, &Options::default())
}

pub fn learn_with(
    train: &str,
    val: &str,
    windows: Windows,
    format: Format,
    opts: &Options,
) -> Result<Outcome, LearnError> {
    let is_plain = |o: &Observed| o.label == Label::Domain(DOMAIN_PLAIN);
    let keep = |o: &Observed| opts.plain != Plain::Off || !is_plain(o);
    let scored = |o: &Observed| opts.plain == Plain::All || !is_plain(o);
    let built = |l: &Learned| build(l).map_err(LearnError::Build);
    let base = built(&Learned::default())?;
    let mut shapes = ShapeTally::default();
    let mut cue_tally = CueTally::default();
    for_each(train, LearnError::Train, &base, windows, |rec, o| {
        shapes.add(o);
        if keep(o) {
            cue_tally.add(rec, o);
        }
    })?;
    let room = CLASS_IDS.saturating_sub(base.first_cue as usize);
    let cues = cue_tally.finish(MAX_CUE_WORDS, MAX_CUE_CLASSES.min(room));
    let mut before = RouteTally::default();
    for_each(val, LearnError::Val, &base, windows, |_, o| {
        before.add(&base, o);
    })?;
    let counted = shapes.finish();
    let floors = sweep_floor(&counted, val, format)?;
    let mut learned = Learned {
        cues,
        shapes: counted,
        floor: Some(pick_floor(&floors)),
        ..Learned::default()
    };
    let staged = built(&learned)?;
    let mut context = ContextTally::default();
    let mut cells = CellTally::default();
    for_each(train, LearnError::Train, &staged, windows, |rec, o| {
        if keep(o) {
            context.add(o);
        }
        cells.add(&staged, rec, o);
    })?;
    let ctx = context.finish(&staged, opts.plain);
    learned.pairs = ctx.pairs;
    learned.lefts = ctx.lefts;
    learned.rights = ctx.rights;
    learned.prior = ctx.prior;
    learned.cells = cells.finish(&staged);
    let canonicalized = cells.canonicalized;
    let done = built(&learned)?;
    let mut after = RouteTally::default();
    let mut kept = RouteTally::default();
    for_each(val, LearnError::Val, &done, windows, |_, o| {
        after.add(&done, o);
        if scored(o) {
            kept.add(&done, o);
        }
    })?;
    let tau = pick_tau(&kept, MAX_WRONG);
    Ok(Outcome {
        learned,
        tau,
        before,
        after,
        floors,
        canonicalized,
    })
}

type Sweep = Vec<(u16, f64)>;

fn sweep_floor(counted: &[ShapeCount], val: &str, format: Format) -> Result<Sweep, LearnError> {
    let mut floors = Vec::new();
    for floor in FLOORS {
        let l = Learned {
            shapes: counted.to_vec(),
            floor: Some(floor),
            ..Learned::default()
        };
        let b = build(&l).map_err(LearnError::Build)?;
        let (exact, total) = exact_sentences(format, &b, val);
        floors.push((floor, exact as f64 / total.max(1) as f64));
    }
    Ok(floors)
}

pub fn house_style(gold: &str) -> std::borrow::Cow<'_, str> {
    let b = gold.as_bytes();
    let grouped = |i: usize| {
        b[i] == b','
            && i > 0
            && b[i - 1].is_ascii_digit()
            && b.get(i + 1).is_some_and(u8::is_ascii_digit)
    };
    if !(0..b.len()).any(grouped) {
        return std::borrow::Cow::Borrowed(gold);
    }
    let kept: Vec<u8> = (0..b.len())
        .filter(|&i| !grouped(i))
        .map(|i| b[i])
        .collect();
    std::borrow::Cow::Owned(String::from_utf8_lossy(&kept).into_owned())
}

pub fn written_sentence(rec: &Record) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(rec.tokens.len());
    let mut i = 0;
    for g in &rec.gold {
        parts.extend(&rec.tokens[i..g.start]);
        parts.push(g.written);
        i = g.start + g.len;
    }
    parts.extend(&rec.tokens[i..]);
    parts.join(" ")
}

fn for_each<'t>(
    text: &'t str,
    wrap: fn(PairError) -> LearnError,
    built: &Built,
    windows: Windows,
    mut f: impl FnMut(&Record<'t>, &Observed<'t>),
) -> Result<(), LearnError> {
    for rec in pairs(text) {
        let rec = rec.map_err(wrap)?;
        for o in observe(&built.tables, &rec, windows) {
            f(&rec, &o);
        }
    }
    Ok(())
}

fn ranked<K: Ord + Copy>(counts: &BTreeMap<K, Counts>, plain: Plain) -> Vec<(K, Rule)> {
    let mut out: Vec<(K, Rule)> = counts
        .iter()
        .filter_map(|(k, c)| rule_of(c, plain).map(|r| (*k, r)))
        .collect();
    out.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(&b.0)));
    out
}

fn rule_of(c: &Counts, plain: Plain) -> Option<Rule> {
    let p = DOMAIN_PLAIN as usize;
    if plain == Plain::All {
        return argmax_rule(c);
    }
    let n: u32 = c.iter().sum();
    if plain == Plain::Strong && n >= MIN_COUNT && wilson_lower(c[p], n) >= STRONG_PLAIN {
        return argmax_rule(c);
    }
    let mut formatted = *c;
    formatted[p] = 0;
    argmax_rule(&formatted)
}

fn argmax_rule(c: &Counts) -> Option<Rule> {
    let n: u32 = c.iter().sum();
    if n < MIN_COUNT {
        return None;
    }
    let (d, &k) = c
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0)))?;
    Some(Rule {
        domain: d as DomainId,
        confidence: confidence(k, n),
        count: n,
    })
}

pub fn confidence(k: u32, n: u32) -> u8 {
    (15.0 * wilson_lower(k, n)).floor().clamp(0.0, 15.0) as u8
}

fn wilson_lower(k: u32, n: u32) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let z2 = WILSON_Z * WILSON_Z;
    let n = f64::from(n);
    let p = f64::from(k) / n;
    let spread = WILSON_Z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    (p + z2 / (2.0 * n) - spread) / (1.0 + z2 / n)
}

const WILSON_Z: f64 = 1.96;

pub fn gold_domain(class: &str) -> Option<DomainId> {
    match class {
        "CARDINAL" | "DECIMAL" | "MONEY" | "MEASURE" => Some(DOMAIN_CARDINAL),
        "ORDINAL" => Some(DOMAIN_ORDINAL),
        "DATE" => Some(DOMAIN_DATE),
        "TIME" => Some(DOMAIN_TIME),
        "TELEPHONE" | "SERIAL" => Some(DOMAIN_IDENTIFIER),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gold<'a>(start: usize, len: usize, class: &'a str, written: &'a str) -> Gold<'a> {
        Gold {
            start,
            len,
            class,
            written,
        }
    }

    #[test]
    fn pairs_read_records_in_order() {
        let text = "S at five thirty pm today\nT 1 3 TIME 5:30 p.m.\n\nS hello there\n\n\
                    S pay four dollars , then two\nT 1 2 MONEY $4\nT 5 1 CARDINAL 2\n\n";
        let got: Vec<_> = pairs(text).collect::<Result<_, _>>().expect("parse");
        assert_eq!(
            got,
            vec![
                Record {
                    tokens: vec!["at", "five", "thirty", "pm", "today"],
                    gold: vec![gold(1, 3, "TIME", "5:30 p.m.")],
                },
                Record {
                    tokens: vec!["hello", "there"],
                    gold: vec![],
                },
                Record {
                    tokens: vec!["pay", "four", "dollars", ",", "then", "two"],
                    gold: vec![gold(1, 2, "MONEY", "$4"), gold(5, 1, "CARDINAL", "2")],
                },
            ]
        );
    }

    #[test]
    fn bad_records_report_their_line() {
        for (text, line) in [
            ("T 0 1 CARDINAL 1\n\n", 1),
            ("S\n\n", 1),
            ("S a  b\n\n", 1),
            ("S one\nT 0 2 CARDINAL 1\n\n", 2),
            ("S one\nT 0 0 CARDINAL 1\n\n", 2),
            ("S one two\nT 1 1 CARDINAL 2\nT 0 1 CARDINAL 1\n\n", 3),
            ("S one two\nT 0 2 CARDINAL 12\nT 1 1 CARDINAL 2\n\n", 3),
            ("S one\nT 0 1 CARDINAL\n\n", 2),
            ("S one\nT x 1 CARDINAL 1\n\n", 2),
            ("S one\nS two\n\n", 2),
            ("S one\nT 0 1 CARDINAL 1\n", 3),
            ("S one\n", 2),
        ] {
            let got: Result<Vec<_>, _> = pairs(text).collect();
            assert_eq!(got, Err(PairError { line }), "{text:?}");
        }
    }

    fn runtime_windows(n: usize, numeric: &dyn Fn(usize) -> bool) -> Vec<(usize, usize, bool)> {
        crate::engine::window::windows(n, numeric)
            .into_iter()
            .map(|w| (w.start, w.end, w.raw))
            .collect()
    }

    fn runtime_format(t: &Tables, lines: &[String]) -> Vec<String> {
        let engine = crate::engine::Normalizer::new(t);
        lines
            .iter()
            .map(|l| {
                let mut out = String::new();
                crate::engine::window::format_line(&engine, l, &mut out);
                out
            })
            .collect()
    }

    fn labels(text: &str) -> Vec<(usize, usize, Label)> {
        let t = crate::builder::build_english().expect("build");
        let mut out = Vec::new();
        for rec in pairs(text) {
            let rec = rec.expect("parse");
            for o in observe(&t, &rec, &runtime_windows) {
                out.push((o.start, o.len, o.label));
            }
        }
        out
    }

    #[test]
    fn observe_labels_runtime_spans_from_gold() {
        use crate::types::{
            DOMAIN_CARDINAL, DOMAIN_IDENTIFIER, DOMAIN_ORDINAL, DOMAIN_PLAIN, DOMAIN_TIME,
        };
        let text = "S at five thirty pm today\nT 1 3 TIME 5:30 p.m.\n\n\
                    S one of them came second\nT 4 1 ORDINAL 2nd\n\n\
                    S pay four dollars , then two\nT 1 2 MONEY $4\nT 5 1 CARDINAL 2\n\n\
                    S room four oh two\nT 1 2 CARDINAL 40\n\n\
                    S flight one two three\nT 1 3 TELEPHONE 123\n\n";
        assert_eq!(
            labels(text),
            vec![
                (1, 3, Label::Domain(DOMAIN_TIME)),
                (0, 1, Label::Domain(DOMAIN_PLAIN)),
                (4, 1, Label::Domain(DOMAIN_ORDINAL)),
                (1, 2, Label::Domain(DOMAIN_CARDINAL)),
                (5, 1, Label::Domain(DOMAIN_CARDINAL)),
                (1, 3, Label::Mismatch),
                (1, 3, Label::Domain(DOMAIN_IDENTIFIER)),
            ]
        );
    }

    #[test]
    fn observe_keeps_runtime_window_context() {
        let t = crate::builder::build_english().expect("build");
        let text = "S at five thirty pm today\nT 1 3 TIME 5:30 p.m.\n\n";
        let rec = pairs(text).next().expect("one").expect("parse");
        let o = observe(&t, &rec, &runtime_windows);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].span.left, t.lexicon.lookup(b"at").class);
        assert_eq!(o[0].span.right, t.lexicon.lookup(b"today").class);
        assert_eq!(o[0].written, Some("5:30 p.m."));
    }

    fn seen(key: u32, domain: DomainId) -> Observed<'static> {
        Observed {
            start: 0,
            len: 1,
            span: Span {
                key,
                ..Span::default()
            },
            label: Label::Domain(domain),
            written: None,
        }
    }

    #[test]
    fn count_shapes_counts_formatted_cores_with_src_bits() {
        use crate::types::{DOMAIN_CARDINAL, DOMAIN_IDENTIFIER, DOMAIN_PLAIN, DOMAIN_TIME};
        let mut obs = vec![seen(0x31, DOMAIN_TIME); 30];
        obs.extend(vec![seen(0x31, DOMAIN_CARDINAL); 69]);
        obs.push(seen(0x31, DOMAIN_IDENTIFIER));
        obs.extend(vec![seen(0x1, DOMAIN_PLAIN); 500]);
        obs.extend(vec![seen(0x1, DOMAIN_CARDINAL); 4]);
        obs.push(Observed {
            label: Label::Mismatch,
            ..seen(0x111, DOMAIN_CARDINAL)
        });
        assert_eq!(
            count_shapes(&obs),
            vec![
                ShapeCount {
                    key: 0x1,
                    count: 4,
                    src: SRC_CARDINAL,
                },
                ShapeCount {
                    key: 0x31,
                    count: 100,
                    src: SRC_CARDINAL | SRC_TIME,
                },
            ]
        );
    }

    fn at_context(left: ClassId, right: ClassId, domain: DomainId) -> Observed<'static> {
        Observed {
            span: Span {
                key: 0x1,
                len: 1,
                core: 1,
                left,
                right,
                ..Span::default()
            },
            ..seen(0x1, domain)
        }
    }

    #[test]
    fn context_rules_take_argmax_with_wilson_confidence_and_min_count() {
        use crate::builder::english::{AT, ROOM, THE};
        use crate::types::{
            CLASS_BOUNDARY, DOMAIN_CARDINAL, DOMAIN_IDENTIFIER, DOMAIN_PLAIN, DOMAIN_TIME,
        };
        let b = crate::builder::build(&crate::builder::learned::Learned::default()).expect("b");
        let mut obs = vec![at_context(THE, CLASS_BOUNDARY, DOMAIN_PLAIN); 30];
        obs.extend(vec![at_context(AT, CLASS_BOUNDARY, DOMAIN_TIME); 20]);
        obs.extend(vec![at_context(AT, CLASS_BOUNDARY, DOMAIN_CARDINAL); 5]);
        obs.extend(vec![at_context(ROOM, CLASS_BOUNDARY, DOMAIN_IDENTIFIER); 5]);
        let mut affix = at_context(ROOM, CLASS_BOUNDARY, DOMAIN_CARDINAL);
        affix.span.len = 2;
        obs.extend(vec![affix; 50]);
        obs.extend(vec![
            Observed {
                label: Label::Mismatch,
                ..at_context(ROOM, CLASS_BOUNDARY, DOMAIN_CARDINAL)
            };
            50
        ]);
        let r = |domain, confidence, count| Rule {
            domain,
            confidence,
            count,
        };
        let w = |s: &str| ClassRef::Word(s.into());
        assert_eq!(
            context_rules(&b, &obs),
            Context {
                pairs: vec![
                    (w("the"), ClassRef::Boundary, r(DOMAIN_PLAIN, 13, 30)),
                    (w("at"), ClassRef::Boundary, r(DOMAIN_TIME, 9, 25)),
                ],
                lefts: vec![
                    (w("the"), r(DOMAIN_PLAIN, 13, 30)),
                    (w("at"), r(DOMAIN_TIME, 9, 25)),
                ],
                rights: vec![(ClassRef::Boundary, r(DOMAIN_PLAIN, 5, 60))],
                prior: Some(r(DOMAIN_PLAIN, 5, 60)),
            }
        );
    }

    #[test]
    fn cell_tally_ranks_candidates_by_gold_matches() {
        use crate::types::{DOMAIN_CARDINAL, cell_index};
        let b = crate::builder::build(&crate::builder::learned::Learned::default()).expect("b");
        let one = |written: &str| format!("S it was five thirty now\nT 2 2 CARDINAL {written}\n\n");
        let mut text = one("530").repeat(MIN_COUNT as usize);
        text.push_str(&one("5:30").repeat(3));
        text.push_str(&"S it was five thirty now\n\n".repeat(40));
        let mut tally = CellTally::default();
        for rec in pairs(&text) {
            let rec = rec.expect("parse");
            for o in observe(&b.tables, &rec, &runtime_windows) {
                tally.add(&b, &rec, &o);
            }
        }
        let cells = tally.finish(&b);
        let key = 0x31;
        let shape = b.tables.shapes.lookup(key);
        let list = &b.lists[&cell_index(DOMAIN_CARDINAL, shape)];
        let reading = list
            .iter()
            .copied()
            .find(|&c| render(&b, c, &["five", "thirty"]) == "530")
            .expect("530 candidate");
        assert_eq!(
            cells,
            vec![Cell {
                domain: DOMAIN_CARDINAL,
                key,
                ranked: vec![(MIN_COUNT, b.ops[&(DOMAIN_CARDINAL, reading)].clone())],
            }]
        );
    }

    fn render(b: &Built, id: crate::types::CandidateId, words: &[&str]) -> String {
        let t = &b.tables;
        let bytes: Vec<&[u8]> = words.iter().map(|w| w.as_bytes()).collect();
        let values: Vec<u32> = bytes
            .iter()
            .map(|w| t.value_of[t.lexicon.lookup(w).canonical as usize])
            .collect();
        let mut out = [0u8; 64];
        let mut pos = 0;
        t.candidates
            .render(
                crate::types::DOMAIN_CARDINAL,
                id,
                &values,
                &bytes,
                &mut out,
                &mut pos,
            )
            .expect("render");
        String::from_utf8(out[..pos].to_vec()).expect("utf8")
    }

    fn fixture() -> String {
        let mut s = String::new();
        for _ in 0..30 {
            s.push_str("S see you at five thirty today\nT 3 2 TIME 5:30\n\n");
            s.push_str("S it costs five dollars\nT 2 2 MONEY $5\n\n");
            s.push_str("S one of them\n\n");
            s.push_str("S go to room four oh two\nT 3 3 TELEPHONE 402\n\n");
            s.push_str("S we have twenty one\nT 2 2 CARDINAL 21\n\n");
        }
        s
    }

    #[test]
    fn strong_plain_mode_keeps_plain_only_on_overwhelming_counts() {
        use crate::types::{DOMAIN_CARDINAL, DOMAIN_PLAIN};
        let mut c = [0u32; DOMAIN_COUNT];
        c[DOMAIN_PLAIN as usize] = 990;
        c[DOMAIN_CARDINAL as usize] = 20;
        let domain = |c: &Counts, p| rule_of(c, p).map(|r| r.domain);
        assert_eq!(domain(&c, Plain::Strong), Some(DOMAIN_PLAIN));
        assert_eq!(domain(&c, Plain::Off), Some(DOMAIN_CARDINAL));
        c[DOMAIN_PLAIN as usize] = 30;
        c[DOMAIN_CARDINAL as usize] = 25;
        assert_eq!(domain(&c, Plain::Strong), Some(DOMAIN_CARDINAL));
        assert_eq!(domain(&c, Plain::All), Some(DOMAIN_PLAIN));
        c[DOMAIN_CARDINAL as usize] = 0;
        assert_eq!(rule_of(&c, Plain::Strong), None);
    }

    #[test]
    fn learn_without_plain_labels_learns_formatted_domains_only() {
        use crate::types::{DOMAIN_CARDINAL, DOMAIN_PLAIN};
        let mut text = fixture();
        text.push_str(&"S two of them\n\n".repeat(500));
        let opts = Options { plain: Plain::Off };
        let out =
            learn_with(&text, &text, &runtime_windows, &runtime_format, &opts).expect("learn");
        let l = &out.learned;
        let domains: Vec<u8> = l
            .pairs
            .iter()
            .map(|p| p.2.domain)
            .chain(l.lefts.iter().map(|x| x.1.domain))
            .chain(l.rights.iter().map(|x| x.1.domain))
            .collect();
        assert!(!domains.contains(&DOMAIN_PLAIN), "{domains:?}");
        assert_eq!(l.prior.map(|r| r.domain), Some(DOMAIN_CARDINAL));
        let all = Options { plain: Plain::All };
        let with =
            learn_with(&text, &text, &runtime_windows, &runtime_format, &all).expect("learn");
        assert_eq!(with.learned.prior.map(|r| r.domain), Some(DOMAIN_PLAIN));
    }

    #[test]
    fn floor_pick_takes_best_exact_rate_and_prefers_the_highest_floor_on_ties() {
        assert_eq!(pick_floor(&[(0, 0.70), (500, 0.72), (1000, 0.72)]), 1000);
        assert_eq!(pick_floor(&[(0, 0.80), (500, 0.72), (1000, 0.70)]), 0);
        assert_eq!(pick_floor(&[]), DEFAULT_FLOOR);
        assert_eq!(DEFAULT_FLOOR, 1000);
    }

    #[test]
    fn floor_sweep_scores_val_exact_sentences() {
        let text = fixture();
        let opts = Options::default();
        let out =
            learn_with(&text, &text, &runtime_windows, &runtime_format, &opts).expect("learn");
        assert_eq!(out.floors.len(), FLOORS.len());
        let base = build(&Learned::default()).expect("build");
        let (exact, total) = exact_sentences(&runtime_format, &base, &text);
        let at_1000 = out.floors.iter().find(|f| f.0 == 1000).map(|f| f.1);
        assert_eq!(at_1000, Some(exact as f64 / total as f64));
        assert_eq!(out.learned.floor, Some(pick_floor(&out.floors)));
    }

    #[test]
    fn route_tally_picks_the_smallest_tau_under_the_wrong_rate() {
        use crate::types::{DOMAIN_CARDINAL, DOMAIN_TIME};
        let mut r = RouteTally::default();
        for _ in 0..90 {
            r.count(DOMAIN_TIME, DOMAIN_TIME, 12);
        }
        for _ in 0..10 {
            r.count(DOMAIN_TIME, DOMAIN_CARDINAL, 2);
        }
        assert_eq!(r.wrong_rate(0), 0.1);
        assert_eq!(r.wrong_rate(3), 0.0);
        assert_eq!(pick_tau(&r, 0.005), 3);
        let at3 = r.domain(DOMAIN_TIME, 3);
        assert_eq!((at3.spans, at3.correct, at3.abstained), (100, 90, 10));
    }

    #[test]
    fn learn_turns_pairs_into_buildable_learned_data() {
        use crate::builder::english::{AT, ROOM};
        use crate::types::{DOMAIN_IDENTIFIER, DOMAIN_TIME};
        let text = fixture();
        let all = Options { plain: Plain::All };
        let out = learn_with(&text, &text, &runtime_windows, &runtime_format, &all).expect("learn");
        let l = &out.learned;
        assert!(l.prior.is_some());
        let w = |s: &str| ClassRef::Word(s.into());
        let find = |c: ClassRef| l.lefts.iter().find(|x| x.0 == c).map(|x| x.1.domain);
        assert_eq!(find(w("at")), Some(DOMAIN_TIME));
        assert_eq!(find(w("room")), Some(DOMAIN_IDENTIFIER));
        assert!(!l.shapes.is_empty());
        assert!(!l.cues.is_empty());
        assert!(l.floor.is_some());
        let text_out = crate::builder::learned::write(l);
        let parsed = crate::builder::learned::parse(&text_out).expect("parse");
        let b = crate::builder::build(&parsed).expect("build");
        assert_eq!(
            b.tables.context.lookup(AT, crate::builder::english::THE).0,
            DOMAIN_TIME
        );
        assert_eq!(b.tables.context.lookup(ROOM, 0).0, DOMAIN_IDENTIFIER);
        assert!(out.tau <= 15);
        assert!(out.after.wrong_rate(out.tau) <= 0.005);
    }

    #[test]
    fn cue_tally_picks_informative_unknown_neighbours_and_merges_alike() {
        let b = crate::builder::build(&crate::builder::learned::Learned::default()).expect("b");
        let mut text = String::new();
        for _ in 0..40 {
            for w in ["zork", "zorp"] {
                text.push_str(&format!("S {w} five thirty\nT 1 2 TIME 5:30\n\n"));
            }
            for w in ["blat", "blip"] {
                text.push_str(&format!("S {w} four oh two\nT 1 3 TELEPHONE 402\n\n"));
            }
        }
        for _ in 0..400 {
            text.push_str("S the twenty one\nT 1 2 CARDINAL 21\n\n");
        }
        for _ in 0..5 {
            text.push_str("S rare five thirty\nT 1 2 TIME 5:30\n\n");
        }
        let mut cues = CueTally::default();
        for rec in pairs(&text) {
            let rec = rec.expect("parse");
            for o in observe(&b.tables, &rec, &runtime_windows) {
                cues.add(&rec, &o);
            }
        }
        assert_eq!(
            cues.finish(512, 2),
            vec![
                ("blat".to_string(), 0),
                ("blip".to_string(), 0),
                ("zork".to_string(), 1),
                ("zorp".to_string(), 1),
            ]
        );
        assert_eq!(cues.finish(512, 4).len(), 4);
        assert_eq!(cues.finish(1, 4).len(), 1);
    }

    #[test]
    fn house_style_drops_commas_between_digit_groups_only() {
        for (gold, want) in [
            ("1,000", "1000"),
            ("$1,250.50", "$1250.50"),
            ("12,345,678", "12345678"),
            ("5, 6", "5, 6"),
            ("a,1", "a,1"),
            ("1,", "1,"),
            (",5", ",5"),
            ("5:30", "5:30"),
        ] {
            assert_eq!(house_style(gold), want, "{gold}");
        }
    }

    #[test]
    fn cell_tally_matches_canonical_gold_and_counts_changes() {
        let b = crate::builder::build(&crate::builder::learned::Learned::default()).expect("b");
        let text =
            "S we have two thousand five\nT 2 3 CARDINAL 2,005\n\n".repeat(MIN_COUNT as usize);
        let mut tally = CellTally::default();
        for rec in pairs(&text) {
            let rec = rec.expect("parse");
            for o in observe(&b.tables, &rec, &runtime_windows) {
                tally.add(&b, &rec, &o);
            }
        }
        assert_eq!(tally.canonicalized, u64::from(MIN_COUNT));
        assert_eq!(tally.finish(&b).len(), 1);
    }

    #[test]
    fn written_sentence_replaces_gold_spans() {
        let text = "S pay four dollars , then two now\nT 1 2 MONEY $4\nT 5 1 CARDINAL 2\n\n";
        let rec = pairs(text).next().expect("one").expect("parse");
        assert_eq!(written_sentence(&rec), "pay $4 , then 2 now");
    }

    #[test]
    fn records_past_the_token_cap_are_errors() {
        let long = vec!["one"; MAX_RECORD_TOKENS + 1].join(" ");
        let text = format!("S {long}\n\n");
        let got: Result<Vec<_>, _> = pairs(&text).collect();
        assert_eq!(got, Err(PairError { line: 1 }));
    }
}
