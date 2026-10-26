pub mod codegen;
pub mod english;
pub mod learn;
pub mod learned;

use std::collections::HashMap;

use crate::engine::span::MAX_SUFFIX;
use crate::tables::candidate::{Candidate, CandidateError, CandidateTable, op};
use crate::tables::cell::{pack, unpack};
use crate::tables::context::{ContextLut, pack_entry};
use crate::tables::lexicon::{self, Lexicon};
use crate::tables::shape::{ShapeBuildError, ShapeTable, shape_key};
use crate::tables::{MAX_CANONICAL, Tables};
use crate::types::{
    CANDIDATE_NONE, CLASS_BOUNDARY, CLASS_UNKNOWN, CandidateId, CanonicalId, CellIndex, ClassId,
    DOMAIN_CARDINAL, DOMAIN_CENTS, DOMAIN_COUNT, DOMAIN_IDENTIFIER, DOMAIN_MEASURE,
    DOMAIN_MEASURE_POWER, DOMAIN_MEASURE_PREFIX, DOMAIN_MONEY, DOMAIN_PERCENT, DOMAIN_TIME,
    DomainId, MAX_TOKENS, ROLE_NONE, Role, SHAPE_COUNT, ShapeId, cell_domain, cell_index,
    cell_shape,
};
use learned::{ClassRef, Learned};

pub const CLASS_IDS: usize = 256;

#[derive(Debug, PartialEq, Eq)]
pub enum BuildError {
    Lexicon(lexicon::BuildError),
    Shapes(ShapeBuildError),
    Literal(CandidateError),
    TooManyCandidates,
    TooManyCanonicals,
    Candidate {
        id: CandidateId,
        err: CandidateError,
    },
    BadCellWord {
        cell: CellIndex,
    },
    NoneAtRank0 {
        cell: CellIndex,
    },
    NoneBeforeCandidate {
        cell: CellIndex,
    },
    SpanLenMismatch {
        cell: CellIndex,
        id: CandidateId,
    },
    BadDefault {
        shape: ShapeId,
    },
    BadRole {
        class: ClassId,
    },
    Learned(learned::ParseError),
    BadClassRef,
    CueClash,
    TooManyClasses,
    LitvValue,
    BadShapeKey,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuildStats {
    pub lexicon_entries: usize,
    pub shapes: usize,
    pub shapes_dropped: usize,
    pub candidates: usize,
    pub programs: usize,
    pub literals: usize,
    pub cells: usize,
    pub affix_cells: usize,
    pub affix_dropped: usize,
}

pub const DOMAINS: usize = 6;

const AFFIX_DOMAINS: [(DomainId, u8); 6] = [
    (DOMAIN_MONEY, 1),
    (DOMAIN_CENTS, 1),
    (DOMAIN_PERCENT, 1),
    (DOMAIN_MEASURE, 1),
    (DOMAIN_MEASURE_POWER, 2),
    (DOMAIN_MEASURE_PREFIX, 2),
];

pub fn affix_split(roles: &[Role]) -> (usize, usize) {
    let has = |mask: u16, r: Role| (mask >> (r & 0x0F)) & 1 == 1;
    let mut end = roles.len();
    for _ in 0..MAX_SUFFIX {
        if end >= 2 && has(english::SUFFIX_ROLES, roles[end - 1]) {
            end -= 1;
        }
    }
    let start = usize::from(end >= 2 && has(english::PREFIX_ROLES, roles[0]));
    (start, end)
}

fn core_roles(roles: &[Role]) -> &[Role] {
    let (start, end) = affix_split(roles);
    &roles[start..end]
}

fn affix_route(left: ClassId, right: ClassId) -> Option<u8> {
    use english::{CENT, DOLLAR, PERCENT, POWER, PREFIX, UNIT};
    let domain = match right {
        DOLLAR => DOMAIN_MONEY,
        CENT => DOMAIN_CENTS,
        PERCENT => DOMAIN_PERCENT,
        UNIT => DOMAIN_MEASURE,
        POWER => DOMAIN_MEASURE_POWER,
        PREFIX | english::DEGREE => DOMAIN_MEASURE_PREFIX,
        _ if left == english::MONTH => english::DOMAIN_MONTH_DATE,
        _ if left == english::MAY => english::DOMAIN_MAY_DATE,
        _ => return None,
    };
    Some(pack_entry(domain, 15))
}

const LEARNED: &str = include_str!("learned.txt");

pub fn build_english() -> Result<Box<Tables>, BuildError> {
    build_english_with_stats().map(|(t, _)| t)
}

pub fn build_english_with_stats() -> Result<(Box<Tables>, BuildStats), BuildError> {
    let learned = learned::parse(LEARNED).map_err(BuildError::Learned)?;
    build(&learned).map(|b| (b.tables, b.stats))
}

pub struct Built {
    pub tables: Box<Tables>,
    pub stats: BuildStats,
    pub first_cue: ClassId,
    pub cue_classes: usize,
    pub lists: HashMap<CellIndex, Vec<CandidateId>>,
    pub ops: HashMap<(DomainId, CandidateId), Vec<u32>>,
    pub keys: Vec<u32>,
    class_words: Vec<Option<&'static str>>,
}

impl Built {
    pub fn class_ref(&self, class: ClassId) -> ClassRef {
        match class {
            CLASS_BOUNDARY => ClassRef::Boundary,
            c if c >= self.first_cue => ClassRef::Cue(c - self.first_cue),
            c => match self.class_words[c as usize] {
                Some(w) => ClassRef::Word(w.to_string()),
                None => ClassRef::Unknown,
            },
        }
    }

    fn resolve(&self, r: &ClassRef) -> Result<ClassId, BuildError> {
        match r {
            ClassRef::Boundary => Ok(CLASS_BOUNDARY),
            ClassRef::Unknown => Ok(CLASS_UNKNOWN),
            ClassRef::Cue(k) if (*k as usize) < self.cue_classes => Ok(self.first_cue + k),
            ClassRef::Word(w) => {
                let class = self.tables.lexicon.lookup(w.as_bytes()).class;
                let base = class != CLASS_UNKNOWN && class < self.first_cue;
                if base && self.tables.role_of[class as usize] == ROLE_NONE {
                    Ok(class)
                } else {
                    Err(BuildError::BadClassRef)
                }
            }
            ClassRef::Cue(_) => Err(BuildError::BadClassRef),
        }
    }
}

struct LexiconInfo {
    entries: usize,
    word_role: HashMap<&'static str, Role>,
    class_words: Vec<Option<&'static str>>,
    first_cue: ClassId,
    cue_classes: usize,
}

fn push_entry<'a>(
    entries: &mut Vec<(&'a [u8], CanonicalId, ClassId)>,
    t: &mut Tables,
    word: &'a str,
    class: ClassId,
    value: u32,
) -> Result<(), BuildError> {
    let canonical = entries.len() + 1;
    if canonical >= MAX_CANONICAL {
        return Err(BuildError::TooManyCanonicals);
    }
    entries.push((word.as_bytes(), canonical as CanonicalId, class));
    t.value_of[canonical] = value;
    Ok(())
}

fn build_lexicon(
    t: &mut Tables,
    learned: &Learned,
    pool: &Pool,
) -> Result<LexiconInfo, BuildError> {
    let mut word_role = HashMap::new();
    let mut class_words = vec![None; CLASS_IDS];
    let mut entries = Vec::new();
    for def in english::CLASSES {
        t.role_of[def.class as usize] = def.role;
        class_words[def.class as usize].get_or_insert(def.words[0].0);
        for &(word, value) in def.words {
            push_entry(&mut entries, t, word, def.class, value)?;
            word_role.insert(word, def.role);
        }
    }
    for def in english::LITERAL_CLASSES {
        t.role_of[def.class as usize] = def.role;
        class_words[def.class as usize].get_or_insert(def.words[0].0);
        for &(word, _) in def.words {
            let id = pool.word_literal[word];
            if id as usize >= pool.table.literal_count() {
                return Err(BuildError::LitvValue);
            }
            push_entry(&mut entries, t, word, def.class, id)?;
            word_role.insert(word, def.role);
        }
    }
    let classes = english::CLASSES.iter().map(|d| d.class);
    let literal_classes = english::LITERAL_CLASSES.iter().map(|d| d.class);
    let first_cue = classes.chain(literal_classes).max().unwrap_or(1) + 1;
    let cue_classes = learned
        .cues
        .iter()
        .map(|c| c.1 as usize + 1)
        .max()
        .unwrap_or(0);
    if first_cue as usize + cue_classes > CLASS_IDS {
        return Err(BuildError::TooManyClasses);
    }
    for (word, k) in &learned.cues {
        if word_role.contains_key(word.as_str()) {
            return Err(BuildError::CueClash);
        }
        push_entry(&mut entries, t, word, first_cue + k, 0)?;
    }
    t.lexicon = Lexicon::build(&entries).map_err(BuildError::Lexicon)?;
    Ok(LexiconInfo {
        entries: entries.len(),
        word_role,
        class_words,
        first_cue,
        cue_classes,
    })
}

const AFFIX_SPREAD: f64 = 0.01;
/// Clock fields out of these ranges are no time ("19:99"): the span reads as
/// a plain number instead.
const MAX_HOUR: u8 = 23;
const MAX_MINUTE: u8 = 59;
const CLOCK_ROLES: [Role; 2] = [english::ROLE_MER, english::ROLE_AMPM];
const NOT_AMOUNT: [Role; 5] = [
    english::ROLE_MER,
    english::ROLE_ORDINAL,
    english::ROLE_CURRENCY,
    english::ROLE_AMPM,
    english::ROLE_UNIT,
];

type CellLists = (Vec<u32>, HashMap<CellIndex, Vec<CandidateId>>, usize, usize);

fn fill_cells(
    t: &mut Tables,
    pool: &mut Pool,
    ranked: &[RankedShape],
    learned: &Learned,
) -> Result<CellLists, BuildError> {
    let mut keys = vec![0u32; SHAPE_COUNT];
    let mut all_lists = HashMap::new();
    let orders: HashMap<(DomainId, u32), &learned::Cell> = learned
        .cells
        .iter()
        .map(|c| ((c.domain, c.key), c))
        .collect();
    for (i, (key, info)) in ranked.iter().enumerate() {
        t.shape_len[i + 1] = info.roles.len() as u8;
        keys[i + 1] = *key;
    }
    let mut place = |t: &mut Tables,
                     pool: &mut Pool,
                     domain: DomainId,
                     shape: ShapeId,
                     list: Vec<ProgramId>|
     -> Result<(), BuildError> {
        let cell = cell_index(domain, shape);
        let order = match orders.get(&(domain, keys[shape as usize])) {
            Some(c) => pool.reorder(t.shape_len[shape as usize] as usize, &list, c),
            None => list.clone(),
        };
        let ranks = to_ranks(&pool.place(domain, &order)?);
        if domain == DOMAIN_CARDINAL {
            t.cardinal_default[shape as usize] = ranks[0];
        }
        t.initial_cells[cell as usize] = pack(ranks, [0; 4]);
        all_lists.insert(cell, pool.place(domain, &list)?);
        Ok(())
    };
    let mut affix_wanted = Vec::new();
    for (i, (_, info)) in ranked.iter().enumerate() {
        let shape = (i + 1) as ShapeId;
        let lists = pool.domain_lists(info.src, &info.roles)?;
        for (domain, list) in lists.into_iter().enumerate() {
            place(t, pool, domain as DomainId, shape, list)?;
        }
        let amount = !info.roles.iter().any(|r| NOT_AMOUNT.contains(r));
        for (slot, &mass) in info.affix.iter().enumerate() {
            let mass = mass + AFFIX_SPREAD * info.mass * f64::from(u8::from(amount));
            if mass > 0.0 {
                affix_wanted.push((mass, shape, slot));
            }
        }
    }
    let first = |slot: usize| AFFIX_DOMAINS[slot].0 != DOMAIN_CENTS;
    affix_wanted.sort_by(|a, b| {
        first(a.2)
            .cmp(&first(b.2))
            .then(b.0.total_cmp(&a.0))
            .then((a.1, a.2).cmp(&(b.1, b.2)))
    });
    let mut dropped = 0;
    let mut month_cells = 0;
    for &(_, shape, slot) in &affix_wanted {
        let roles = &ranked[shape as usize - 1].1.roles;
        let list = pool.affix_list(slot, roles)?;
        match place(t, pool, AFFIX_DOMAINS[slot].0, shape, list) {
            Ok(()) => {}
            Err(BuildError::TooManyCandidates) => dropped += 1,
            Err(e) => return Err(e),
        }
    }
    for (i, (_, info)) in ranked.iter().enumerate() {
        let shape = (i + 1) as ShapeId;
        for (domain, list) in pool.month_lists(&info.roles)? {
            match place(t, pool, domain, shape, list) {
                Ok(()) => month_cells += 1,
                Err(BuildError::TooManyCandidates) => dropped += 1,
                Err(e) => return Err(e),
            }
        }
    }
    let placed = affix_wanted.len() + month_cells - dropped;
    Ok((keys, all_lists, placed, dropped))
}

pub fn build(learned: &Learned) -> Result<Built, BuildError> {
    let mut t = Tables::boxed_empty();
    let mut pool = Pool::new()?;
    let lex = build_lexicon(&mut t, learned, &pool)?;
    let (ranked, shapes_dropped) = rank_shapes(&lex.word_role, learned)?;
    let shape_entries: Vec<(u32, ShapeId)> = ranked
        .iter()
        .enumerate()
        .map(|(i, (key, _))| (*key, (i + 1) as ShapeId))
        .collect();
    t.shapes = ShapeTable::build(&shape_entries).map_err(BuildError::Shapes)?;
    t.suffix_roles = english::SUFFIX_ROLES;
    t.prefix_roles = english::PREFIX_ROLES;
    t.connector_roles = 1 << english::ROLE_AND;
    t.bind_roles = english::BIND_ROLES;
    t.words_first_roles = 1 << english::ROLE_ORDINAL;
    t.lead_skip_roles = english::LEAD_SKIP_ROLES;
    t.left_folds = english::LEFT_FOLDS;
    for w in english::PLURAL {
        let c = t.lexicon.lookup(w.as_bytes()).canonical as usize;
        t.plural[c / 64] |= 1 << (c % 64);
    }
    for w in english::WORDS_FIRST {
        let c = t.lexicon.lookup(w.as_bytes()).canonical as usize;
        t.words_first[c / 64] |= 1 << (c % 64);
    }
    for (domain, len) in AFFIX_DOMAINS {
        t.affix_len[domain as usize] = len;
    }
    for domain in [english::DOMAIN_MONTH_DATE, english::DOMAIN_MAY_DATE] {
        t.affix_len[domain as usize] = 1;
    }
    let mark = |bits: &mut [u64; 4], classes: &[ClassId]| {
        for &c in classes {
            bits[c as usize / 64] |= 1 << (c % 64);
        }
    };
    mark(&mut t.starts, &english::START_CLASSES);
    mark(&mut t.per_stop, &[english::PER]);
    mark(&mut t.per_units, &english::PER_UNITS);
    mark(&mut t.pair_only, &english::PAIR_ONLY);
    mark_settings(&mut t);
    let (keys, lists, affix_cells, affix_dropped) =
        fill_cells(&mut t, &mut pool, &ranked, learned)?;

    let mut b = Built {
        tables: t,
        stats: BuildStats::default(),
        first_cue: lex.first_cue,
        cue_classes: lex.cue_classes,
        lists,
        ops: pool.local_ops(),
        keys,
        class_words: lex.class_words,
    };
    let entries = lex.entries;
    match learned.prior {
        Some(prior) => b.learned_context(learned, prior)?,
        None => {
            for left in 0..=255u8 {
                for right in 0..=255u8 {
                    let e = affix_route(left, right).unwrap_or_else(|| route(left, right));
                    b.tables.context.set(left, right, e);
                }
            }
        }
    }
    alias(&mut b.tables.context, &english::ALIASES);

    b.stats = BuildStats {
        lexicon_entries: entries,
        shapes: ranked.len(),
        shapes_dropped,
        candidates: pool.max_domain_candidates(),
        programs: pool.programs.len(),
        literals: pool.table.literal_count(),
        cells: b.tables.initial_cells.iter().filter(|&&w| w != 0).count(),
        affix_cells,
        affix_dropped,
    };
    b.tables.candidates = *pool.table;
    validate(&b.tables)?;
    Ok(b)
}

/// Gives each `(class, like)` the context entries of `like`: fold words read
/// as unknown words, and a fold's class reads as its cue.
fn alias(context: &mut ContextLut, aliases: &[(ClassId, ClassId)]) {
    let raw = *context.as_raw();
    let at = |l: ClassId, r: ClassId| raw[(l as usize) << 8 | r as usize];
    let read = |c: ClassId| aliases.iter().find(|a| a.0 == c).map_or(c, |a| a.1);
    for l in 0..=255u8 {
        for r in 0..=255u8 {
            context.set(l, r, at(read(l), read(r)));
        }
    }
}

fn mark_settings(t: &mut Tables) {
    for (setting, classes) in english::SETTING_CLASSES {
        for &c in classes {
            t.class_settings[c as usize] |= setting.bit();
        }
    }
    for (setting, domains) in english::SETTING_DOMAINS {
        for &d in domains {
            t.domain_settings[d as usize] |= setting.bit();
        }
    }
}

impl Built {
    fn learned_context(&mut self, l: &Learned, prior: learned::Rule) -> Result<(), BuildError> {
        let entry = |r: &learned::Rule| pack_entry(r.domain, r.confidence);
        let mut pairs = HashMap::new();
        let (mut lefts, mut rights) = ([None; 256], [None; 256]);
        for (a, b, r) in &l.pairs {
            pairs.insert((self.resolve(a)?, self.resolve(b)?), entry(r));
        }
        for (a, r) in &l.lefts {
            lefts[self.resolve(a)? as usize] = Some(entry(r));
        }
        for (a, r) in &l.rights {
            rights[self.resolve(a)? as usize] = Some(entry(r));
        }
        for left in 0..=255u8 {
            for right in 0..=255u8 {
                let pair = pairs.get(&(left, right)).copied();
                let (l, r) = (lefts[left as usize], rights[right as usize]);
                let e = match hand_rule(left, right) {
                    Some((h, side)) => {
                        let same = if side == Side::Left { l } else { r };
                        pair.or(same).filter(|x| x >> 4 >= h >> 4).unwrap_or(h)
                    }
                    None => pair.or(l).or(r).unwrap_or(entry(&prior)),
                };
                let e = affix_route(left, right).unwrap_or(e);
                self.tables.context.set(left, right, e);
            }
        }
        Ok(())
    }
}

const MAX_SHAPES: usize = SHAPE_COUNT - 1;
const LONE_WORDS: &[Role] = &[english::ROLE_OH];

struct ShapeInfo {
    mass: f64,
    src: u8,
    roles: Vec<Role>,
    affix: [f64; AFFIX_DOMAINS.len()],
}

type RankedShape = (u32, ShapeInfo);

fn affix_slot(words: &[&str], full: &[Role]) -> Option<usize> {
    let word = match affix_split(full) {
        (0, end) if end < full.len() => words[end],
        (1, end) if end == full.len() => words[0],
        _ => return None,
    };
    let (start, end) = affix_split(full);
    let affixes = start + full.len() - end;
    let domain = english::affix_domain(word)?;
    AFFIX_DOMAINS
        .iter()
        .position(|a| a.0 == domain && a.1 as usize == affixes)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct KindReport {
    pub kind: &'static str,
    pub shapes_kept: usize,
    pub shapes_dropped: usize,
    pub mass_covered: f64,
}

pub fn shape_report() -> Vec<KindReport> {
    let word_role: HashMap<&str, Role> = english::CLASSES
        .iter()
        .flat_map(|d| d.words.iter().map(move |w| (w.0, d.role)))
        .chain(english::literal_words())
        .collect();
    let Ok((ranked, _)) = rank_shapes(&word_role, &Learned::default()) else {
        return Vec::new();
    };
    let kept: std::collections::HashSet<u32> = ranked.iter().map(|r| r.0).collect();
    let mut per_shape: HashMap<u32, HashMap<&'static str, f64>> = HashMap::new();
    let mut mass: HashMap<&'static str, (f64, f64)> = HashMap::new();
    for e in english::examples() {
        if e.words.len() > MAX_TOKENS {
            continue;
        }
        let m = mass.entry(e.kind).or_default();
        m.0 += e.weight;
        let roles: Vec<Role> = e.words.iter().map(|w| word_role[w]).collect();
        let key = shape_key(core_roles(&roles));
        if kept.contains(&key) {
            m.1 += e.weight;
        }
        *per_shape.entry(key).or_default().entry(e.kind).or_default() += e.weight;
    }
    let mut out: Vec<KindReport> = Vec::new();
    for (key, kinds) in &per_shape {
        let kind = kinds
            .iter()
            .max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(a.0)))
            .map(|k| *k.0)
            .unwrap_or("");
        let idx = match out.iter().position(|r| r.kind == kind) {
            Some(i) => i,
            None => {
                out.push(KindReport {
                    kind,
                    ..KindReport::default()
                });
                out.len() - 1
            }
        };
        if kept.contains(key) {
            out[idx].shapes_kept += 1;
        } else {
            out[idx].shapes_dropped += 1;
        }
    }
    for r in &mut out {
        let (total, covered) = mass[r.kind];
        r.mass_covered = covered / total;
    }
    out.sort_by(|a, b| a.kind.cmp(b.kind));
    out
}

fn rank_shapes(
    word_role: &HashMap<&str, Role>,
    learned: &Learned,
) -> Result<(Vec<RankedShape>, usize), BuildError> {
    let mut freq: HashMap<u32, ShapeInfo> = HashMap::new();
    for english::Example {
        words,
        src,
        weight,
        kind,
    } in english::examples()
    {
        let weight = weight * english::kind_mass(kind);
        if words.len() > MAX_TOKENS {
            continue;
        }
        let full: Vec<Role> = words.iter().map(|w| word_role[w]).collect();
        let roles = core_roles(&full).to_vec();
        let e = freq.entry(shape_key(&roles)).or_insert_with(|| ShapeInfo {
            mass: 0.0,
            src: 0,
            roles,
            affix: [0.0; AFFIX_DOMAINS.len()],
        });
        e.mass += weight;
        e.src |= src;
        if let Some(slot) = affix_slot(&words, &full) {
            e.affix[slot] += weight;
        }
    }
    let prior_total: f64 = freq.values().map(|e| e.mass).sum();
    let mut observed: HashMap<u32, f64> = HashMap::new();
    for s in &learned.shapes {
        let roles = key_roles(s.key).ok_or(BuildError::BadShapeKey)?;
        let e = freq.entry(s.key).or_insert_with(|| ShapeInfo {
            mass: 0.0,
            src: 0,
            roles,
            affix: [0.0; AFFIX_DOMAINS.len()],
        });
        e.src |= s.src;
        *observed.entry(s.key).or_default() += f64::from(s.count);
    }
    let observed_total: f64 = observed.values().sum();
    let floor = if observed_total > 0.0 {
        f64::from(learned.floor.unwrap_or(1000)) / 1000.0
    } else {
        1.0
    };
    let score = |key: u32, info: &ShapeInfo| {
        let seen = observed.get(&key).map_or(0.0, |n| n / observed_total);
        (1.0 - floor) * seen + floor * info.mass / prior_total
    };
    let mut ranked: Vec<(f64, RankedShape)> = freq
        .into_iter()
        .map(|(key, info)| (score(key, &info), (key, info)))
        .filter(|(s, (_, info))| *s > 0.0 || info.mass > 0.0)
        .collect();
    ranked.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(b.1.1.mass.total_cmp(&a.1.1.mass))
            .then(a.1.0.cmp(&b.1.0))
    });
    let dropped = ranked.len().saturating_sub(MAX_SHAPES);
    ranked.truncate(MAX_SHAPES);
    Ok((ranked.into_iter().map(|(_, r)| r).collect(), dropped))
}

fn key_roles(key: u32) -> Option<Vec<Role>> {
    let mut roles = Vec::new();
    let mut k = key;
    while k & 0x0F != 0 {
        roles.push((k & 0x0F) as Role);
        k >>= 4;
    }
    (k == 0 && !roles.is_empty()).then_some(roles)
}

fn route(left: ClassId, right: ClassId) -> u8 {
    hand_rule(left, right).map_or(pack_entry(DOMAIN_CARDINAL, 4), |h| h.0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

fn hand_rule(left: ClassId, right: ClassId) -> Option<(u8, Side)> {
    use english::{IDENTIFIER_LEFT, TIME_LEFT, TIME_RIGHT};
    if TIME_RIGHT.contains(&right) {
        Some((pack_entry(DOMAIN_TIME, 15), Side::Right))
    } else if left == TIME_LEFT {
        Some((pack_entry(DOMAIN_TIME, 8), Side::Left))
    } else if IDENTIFIER_LEFT.contains(&left) || left == english::DOT {
        Some((pack_entry(DOMAIN_IDENTIFIER, 12), Side::Left))
    } else if right == english::DOT {
        Some((pack_entry(DOMAIN_IDENTIFIER, 12), Side::Right))
    } else {
        None
    }
}

fn to_ranks(list: &[CandidateId]) -> [CandidateId; 4] {
    let mut ranks = [CANDIDATE_NONE; 4];
    let mut n = 0;
    for &id in list {
        if n < 4 && !ranks[..n].contains(&id) {
            ranks[n] = id;
            n += 1;
        }
    }
    ranks
}

type ProgramId = u16;

struct Pool {
    table: Box<CandidateTable>,
    ids: HashMap<(u8, Vec<u32>), ProgramId>,
    programs: Vec<(u8, Vec<u32>)>,
    locals: Vec<HashMap<ProgramId, CandidateId>>,
    colon: u8,
    dot: u8,
    space: u8,
    oclock: u8,
    on_the_hour: u8,
    zero_dot: u8,
    comma: u8,
    may: u8,
    word_literal: HashMap<&'static str, u32>,
}

impl Pool {
    fn new() -> Result<Self, BuildError> {
        let mut table = Box::new(CandidateTable::new());
        let mut lit = |text: &[u8]| table.add_literal(text).map_err(BuildError::Literal);
        let (colon, dot, space) = (lit(b":")?, lit(b".")?, lit(b" ")?);
        let oclock = lit(b" o'clock")?;
        let on_the_hour = lit(b":00")?;
        let zero_dot = lit(b"0.")?;
        let (comma, may) = (lit(b", ")?, lit(b"May ")?);
        let mut by_text: HashMap<&str, u8> = HashMap::new();
        let mut word_literal = HashMap::new();
        for class in english::LITERAL_CLASSES {
            for &(word, text) in class.words {
                let id = match by_text.get(text) {
                    Some(&id) => id,
                    None => {
                        let id = lit(text.as_bytes())?;
                        by_text.insert(text, id);
                        id
                    }
                };
                word_literal.insert(word, u32::from(id));
            }
        }
        Ok(Self {
            table,
            ids: HashMap::new(),
            programs: Vec::new(),
            locals: vec![HashMap::new(); DOMAIN_COUNT],
            colon,
            dot,
            space,
            oclock,
            on_the_hour,
            zero_dot,
            comma,
            may,
            word_literal,
        })
    }

    fn intern(&mut self, span_len: usize, ops: &[u32]) -> Result<ProgramId, BuildError> {
        let key = (span_len as u8, ops.to_vec());
        if let Some(&id) = self.ids.get(&key) {
            return Ok(id);
        }
        let id =
            ProgramId::try_from(self.programs.len()).map_err(|_| BuildError::TooManyCandidates)?;
        self.programs.push(key.clone());
        self.ids.insert(key, id);
        Ok(id)
    }

    fn place(
        &mut self,
        domain: DomainId,
        list: &[ProgramId],
    ) -> Result<Vec<CandidateId>, BuildError> {
        let locals = &mut self.locals[domain as usize];
        let new = list.iter().filter(|p| !locals.contains_key(p)).count();
        if locals.len() + new > MAX_CANDIDATE_ID {
            return Err(BuildError::TooManyCandidates);
        }
        let mut out = Vec::with_capacity(list.len());
        for &p in list {
            let next = (locals.len() + 1) as CandidateId;
            let id = *locals.entry(p).or_insert(next);
            if id == next {
                let (len, ops) = &self.programs[p as usize];
                self.table.set(domain, id, Candidate::new(*len, ops));
            }
            out.push(id);
        }
        Ok(out)
    }

    fn local_ops(&self) -> HashMap<(DomainId, CandidateId), Vec<u32>> {
        let mut out = HashMap::new();
        for (d, locals) in self.locals.iter().enumerate() {
            for (&p, &id) in locals {
                out.insert((d as DomainId, id), self.programs[p as usize].1.clone());
            }
        }
        out
    }

    fn max_domain_candidates(&self) -> usize {
        self.locals.iter().map(HashMap::len).max().unwrap_or(0)
    }

    fn cardinal(&mut self, roles: &[Role]) -> Result<Vec<ProgramId>, BuildError> {
        let ops = self.reading(roles);
        let mut list = vec![self.intern(roles.len(), &ops)?];
        if roles == [english::ROLE_DIGIT] {
            list.push(self.raw(1)?);
        }
        if let Some(ops) = self.clock(roles, 2) {
            list.push(self.intern(roles.len(), &ops)?);
        }
        Ok(list)
    }

    fn clock(&self, roles: &[Role], hour_width: u8) -> Option<Vec<u32>> {
        use english::{ROLE_AMPM, ROLE_MER};
        let n = roles.len();
        if n >= 2 && roles[n - 1] == ROLE_AMPM {
            let mut ops = vec![op::fold(0, 1), op::fit(0, 0, MAX_HOUR)];
            if n > 2 {
                ops.extend([op::fold(1, (n - 2) as u8), op::fit(1, 0, MAX_MINUTE)]);
            }
            ops.push(op::emit(0, hour_width));
            if n > 2 {
                ops.extend([op::lit(self.colon), op::emit(1, 2)]);
            }
            ops.extend([op::lit(self.space), op::raw(1)]);
            return Some(ops);
        }
        if n < 2 || roles[n - 1] != ROLE_MER {
            return None;
        }
        let mer = if roles[n - 2] == ROLE_MER { 2 } else { 1 };
        let body = n - mer;
        let mut ops = vec![op::fold(0, 1), op::fit(0, 0, MAX_HOUR)];
        if body > 1 {
            ops.extend([op::fold(1, (body - 1) as u8), op::fit(1, 0, MAX_MINUTE)]);
        }
        ops.push(op::emit(0, hour_width));
        if body > 1 {
            ops.extend([op::lit(self.colon), op::emit(1, 2)]);
        } else if hour_width == 2 {
            ops.push(op::lit(self.on_the_hour));
        }
        if mer == 2 {
            ops.extend([op::lit(self.space), op::raw(1), op::raw(1)]);
        } else if hour_width == 2 {
            ops.push(op::skip(1));
        } else {
            ops.extend([op::lit(self.oclock), op::skip(1)]);
        }
        Some(ops)
    }

    fn reading(&self, roles: &[Role]) -> Vec<u32> {
        if let Some(ops) = self.clock(roles, 0) {
            return ops;
        }
        self.dollars_and_cents(roles)
            .unwrap_or_else(|| self.amount(roles, op::GROUP_NOT_YEAR))
    }

    fn dollars_and_cents(&self, roles: &[Role]) -> Option<Vec<u32>> {
        use english::{ROLE_AND, ROLE_CURRENCY};
        let d = roles
            .iter()
            .position(|&r| r == ROLE_CURRENCY)
            .filter(|&d| d > 0)?;
        let mut ops = vec![op::litv(d as u8)];
        ops.extend(self.amount(&roles[..d], op::GROUP));
        ops.push(op::skip(1));
        let mut rest = &roles[d + 1..];
        if rest.first() == Some(&ROLE_AND) {
            ops.push(op::skip(1));
            rest = &rest[1..];
        }
        if !rest.is_empty() {
            ops.extend([
                op::fold(3, rest.len() as u8),
                op::lit(self.dot),
                op::emit(3, 2),
            ]);
        }
        Some(ops)
    }

    /// The written number for `roles`; `group` is how integer parts take
    /// thousands commas (an `op::GROUP*` value).
    fn amount(&self, roles: &[Role], group: u8) -> Vec<u32> {
        use english::{ROLE_DIGIT, ROLE_MSCALE, ROLE_OH, ROLE_ORDINAL, ROLE_POINT, ROLE_SCALE};
        let n = roles.len();
        let n8 = n as u8;
        let scaled = n >= 2 && matches!(roles[n - 1], ROLE_SCALE | ROLE_MSCALE);
        if let Some(p) = roles.iter().position(|&r| r == ROLE_POINT) {
            let frac = (n - p - 1 - usize::from(scaled)) as u8;
            // "one point eighty two" is 1.82, not 1.802.
            let pair = p > 0 && two_digit_group(&roles[p + 1..p + 1 + frac as usize]);
            let mut ops = Vec::with_capacity(8);
            if p > 0 {
                ops.push(op::fold(0, p as u8));
            }
            ops.push(op::skip(1));
            ops.push(if pair {
                op::fold(1, frac)
            } else {
                op::digits(1, frac)
            });
            if p > 0 {
                ops.push(op::emit_grouped(0, group));
                ops.push(op::lit(self.dot));
            } else {
                ops.push(op::lit(self.zero_dot));
            }
            ops.push(op::emit(1, if pair { 2 } else { frac }));
            if scaled {
                ops.extend([op::lit(self.space), op::raw(1)]);
            }
            return ops;
        }
        let higher = roles[..n.saturating_sub(1)]
            .iter()
            .any(|r| matches!(*r, ROLE_SCALE | ROLE_MSCALE));
        if n >= 2 && roles[n - 1] == ROLE_MSCALE && !higher {
            return vec![
                op::fold(0, n8 - 1),
                op::emit_grouped(0, group),
                op::lit(self.space),
                op::raw(1),
            ];
        }
        if roles.last() == Some(&ROLE_ORDINAL) {
            return vec![op::fold(0, n8), op::emit_grouped(0, group), op::ord(0)];
        }
        if n >= 2 && roles.iter().all(|&r| r == ROLE_DIGIT || r == ROLE_OH) {
            return vec![op::digits(0, n8), op::emit(0, n8)];
        }
        if roles.first() == Some(&ROLE_DIGIT) && two_digit_group(&roles[1..]) {
            return vec![
                op::fold(0, 1),
                op::fold(1, n8 - 1),
                op::emit(0, 0),
                op::emit(1, 2),
            ];
        }
        vec![op::fold(0, n8), op::emit_grouped(0, group)]
    }

    fn domain_lists(
        &mut self,
        src: u8,
        roles: &[Role],
    ) -> Result<[Vec<ProgramId>; DOMAINS], BuildError> {
        use english::{SRC_IDENTIFIER, SRC_TIME};
        let reading = self.cardinal(roles)?;
        let card = if let Some(y) = self.year(roles)? {
            let mut c = vec![y];
            if src & SRC_TIME != 0 {
                c.push(self.raw(roles.len())?);
            }
            c.extend(&reading);
            c
        } else if src == SRC_TIME && !roles.iter().any(|r| CLOCK_ROLES.contains(r)) {
            let mut c = vec![self.raw(roles.len())?];
            c.extend(&reading);
            c
        } else {
            reading.clone()
        };
        let time = match (src & SRC_TIME != 0, self.time(roles)?) {
            (true, Some(mut t)) => {
                t.push(reading[0]);
                t
            }
            _ => card.clone(),
        };
        let raw = self.raw(roles.len())?;
        let ident = if src & SRC_IDENTIFIER != 0 {
            vec![self.identifier(roles)?, card[0]]
        } else {
            // After "flight" or "room", a time-shaped run is a number: 417.
            let mut c: Vec<ProgramId> = card.iter().copied().filter(|&p| p != raw).collect();
            c.push(raw);
            c
        };
        let date = self.date(roles)?;
        let mut plain = vec![raw];
        plain.extend(&reading);
        let card = if matches!(roles, [r] if LONE_WORDS.contains(r)) {
            plain.clone()
        } else {
            card
        };
        let ordinal = card.clone();
        Ok([card, time, ident, ordinal, date, plain])
    }

    fn affix_list(&mut self, slot: usize, roles: &[Role]) -> Result<Vec<ProgramId>, BuildError> {
        let n = roles.len();
        let affix = n as u8;
        let ops = match AFFIX_DOMAINS[slot].0 {
            DOMAIN_MONEY => [
                &[op::litv(affix)][..],
                &self.amount(roles, op::GROUP),
                &[op::skip(1)],
            ]
            .concat(),
            DOMAIN_CENTS => match self.dollars_and_cents(roles) {
                Some(ops) => [&ops[..], &[op::skip(1)]].concat(),
                None => [
                    &self.amount(roles, op::GROUP)[..],
                    &[op::lit(self.space), op::raw(1)],
                ]
                .concat(),
            },
            DOMAIN_MEASURE_POWER => return self.spelled(n, 2, roles),
            DOMAIN_MEASURE_PREFIX => [
                &self.amount(roles, op::GROUP)[..],
                &[
                    op::lit(self.space),
                    op::litv(affix),
                    op::lit(self.space),
                    op::litv(affix + 1),
                    op::skip(2),
                ],
            ]
            .concat(),
            DOMAIN_PERCENT => [
                &self.amount(roles, op::GROUP)[..],
                &[op::litv(affix), op::skip(1)],
            ]
            .concat(),
            DOMAIN_MEASURE => return self.spelled(n, 1, roles),
            _ => [
                &self.amount(roles, op::GROUP)[..],
                &[op::lit(self.space), op::litv(affix), op::skip(1)],
            ]
            .concat(),
        };
        let len = n + AFFIX_DOMAINS[slot].1 as usize;
        Ok(vec![self.intern(len, &ops)?, self.raw(len)?])
    }

    fn spelled(
        &mut self,
        n: usize,
        unit_len: usize,
        roles: &[Role],
    ) -> Result<Vec<ProgramId>, BuildError> {
        let len = n + unit_len;
        let spelled = [
            &self.amount(roles, op::GROUP)[..],
            &[op::lit(self.space), op::raw(unit_len as u8)],
        ]
        .concat();
        Ok(vec![self.intern(len, &spelled)?, self.raw(len)?])
    }

    fn reorder(&self, span_len: usize, list: &[ProgramId], cell: &learned::Cell) -> Vec<ProgramId> {
        let mut out: Vec<ProgramId> = cell
            .ranked
            .iter()
            .filter_map(|(_, ops)| self.ids.get(&(span_len as u8, ops.clone())).copied())
            .filter(|id| list.contains(id))
            .collect();
        let rest: Vec<ProgramId> = list
            .iter()
            .copied()
            .filter(|id| !out.contains(id))
            .collect();
        out.extend(rest);
        out
    }

    fn year(&mut self, roles: &[Role]) -> Result<Option<ProgramId>, BuildError> {
        let Some(a) = pair_split(roles) else {
            return Ok(None);
        };
        let b = roles.len() - a;
        let ops = [
            op::fold(0, a as u8),
            op::fold(1, b as u8),
            op::check(0, YEAR_LEAD.0, YEAR_LEAD.1),
            op::emit(0, 0),
            op::emit(1, 2),
        ];
        Ok(Some(self.intern(roles.len(), &ops)?))
    }

    fn date(&mut self, roles: &[Role]) -> Result<Vec<ProgramId>, BuildError> {
        let n = roles.len();
        let mut list = Vec::with_capacity(3);
        if let Some(k) = roles.iter().rposition(|&r| r == english::ROLE_ORDINAL) {
            let rest = &roles[k + 1..];
            let mut ops = vec![op::fold(0, (k + 1) as u8), op::emit(0, 0), op::ord(0)];
            if !rest.is_empty() {
                ops.push(op::lit(self.space));
                match pair_split(rest) {
                    Some(a) => ops.extend([
                        op::fold(1, a as u8),
                        op::fold(2, (rest.len() - a) as u8),
                        op::check(1, YEAR_LEAD.0, YEAR_LEAD.1),
                        op::emit(1, 0),
                        op::emit(2, 2),
                    ]),
                    None => ops.extend([op::fold(1, rest.len() as u8), op::emit(1, 0)]),
                }
            }
            list.push(self.intern(n, &ops)?);
        } else if let Some(y) = self.year(roles)? {
            list.push(y);
        }
        list.extend(self.cardinal(roles)?);
        Ok(list)
    }

    fn month_body(&self, roles: &[Role], comma: bool) -> Option<Vec<u32>> {
        let k = roles.iter().position(|&r| r == english::ROLE_ORDINAL);
        if let Some(k) = k.filter(|&k| k <= 1) {
            let rest = &roles[k + 1..];
            let mut ops = vec![op::fold(0, (k + 1) as u8), op::emit(0, 0), op::ord(0)];
            if !rest.is_empty() {
                let split = pair_split(rest);
                let year = split.is_some() || rest.len() >= 2;
                let sep = if comma && year {
                    self.comma
                } else {
                    self.space
                };
                ops.push(op::lit(sep));
                match split {
                    Some(a) => ops.extend([
                        op::fold(1, a as u8),
                        op::fold(2, (rest.len() - a) as u8),
                        op::check(1, YEAR_LEAD.0, YEAR_LEAD.1),
                        op::emit(1, 0),
                        op::emit(2, 2),
                    ]),
                    None => ops.extend([op::fold(1, rest.len() as u8), op::emit(1, 0)]),
                }
            }
            return Some(ops);
        }
        let a = pair_split(roles)?;
        Some(vec![
            op::fold(0, a as u8),
            op::fold(1, (roles.len() - a) as u8),
            op::check(0, YEAR_LEAD.0, YEAR_LEAD.1),
            op::emit(0, 0),
            op::emit(1, 2),
        ])
    }

    fn month_lists(
        &mut self,
        roles: &[Role],
    ) -> Result<Vec<(DomainId, Vec<ProgramId>)>, BuildError> {
        let (Some(comma), Some(plain)) =
            (self.month_body(roles, true), self.month_body(roles, false))
        else {
            return Ok(Vec::new());
        };
        let n = roles.len() + 1;
        let spoken = [&[op::raw(1), op::lit(self.space)][..], &plain].concat();
        let amount = self.amount(roles, op::GROUP_NONE);
        let mut out = Vec::with_capacity(2);
        for (domain, name) in [
            (english::DOMAIN_MONTH_DATE, op::litv(0)),
            (english::DOMAIN_MAY_DATE, op::lit(self.may)),
        ] {
            let lead = [name, op::skip(1)];
            let list = vec![
                self.intern(n, &[&lead[..], &comma].concat())?,
                self.intern(n, &spoken)?,
                self.intern(n, &[&lead[..], &plain].concat())?,
                self.intern(n, &[&lead[..], &amount].concat())?,
            ];
            out.push((domain, list));
        }
        Ok(out)
    }

    fn raw(&mut self, n: usize) -> Result<ProgramId, BuildError> {
        self.intern(n, &[op::raw(n as u8)])
    }

    fn time(&mut self, roles: &[Role]) -> Result<Option<Vec<ProgramId>>, BuildError> {
        let n = roles.len();
        if n < 2 || roles.iter().any(|r| CLOCK_ROLES.contains(r)) {
            return Ok(None);
        }
        let mut list = Vec::with_capacity(3);
        for (sep, hour_width) in [(self.colon, 0), (self.colon, 2), (self.space, 0)] {
            let ops = [
                op::fold(0, 1),
                op::fold(1, (n - 1) as u8),
                op::fit(0, 0, MAX_HOUR),
                op::fit(1, 0, MAX_MINUTE),
                op::emit(0, hour_width),
                op::lit(sep),
                op::emit(1, 2),
            ];
            list.push(self.intern(n, &ops)?);
        }
        Ok(Some(list))
    }

    fn identifier(&mut self, roles: &[Role]) -> Result<ProgramId, BuildError> {
        let n = roles.len() as u8;
        self.intern(roles.len(), &[op::digits(0, n), op::emit(0, n)])
    }
}

const MAX_CANDIDATE_ID: usize = u8::MAX as usize;

fn two_digit_group(roles: &[Role]) -> bool {
    use english::{ROLE_DIGIT, ROLE_TEEN, ROLE_TENS};
    matches!(roles, [ROLE_TEEN] | [ROLE_TENS] | [ROLE_TENS, ROLE_DIGIT])
}

const YEAR_LEAD: (u8, u8) = (10, 29);

fn pair_split(roles: &[Role]) -> Option<usize> {
    use english::{ROLE_DIGIT, ROLE_OH, ROLE_TEEN, ROLE_TENS};
    let lead = |p: &[Role]| matches!(p, [ROLE_TEEN] | [ROLE_TENS] | [ROLE_TENS, ROLE_DIGIT]);
    let two_digit = |p: &[Role]| lead(p) || matches!(p, [ROLE_OH, ROLE_DIGIT]);
    (1..=2).find(|&a| a < roles.len() && lead(&roles[..a]) && two_digit(&roles[a..]))
}

pub fn validate(t: &Tables) -> Result<(), BuildError> {
    for (class, &role) in t.role_of.iter().enumerate() {
        let reserved = class < 2 && role != ROLE_NONE;
        if role > 0x0F || reserved {
            return Err(BuildError::BadRole {
                class: class as ClassId,
            });
        }
    }
    let mut used = [false; SHAPE_COUNT];
    for (index, &word) in t.initial_cells.iter().enumerate() {
        let cell = index as CellIndex;
        if word == 0 {
            continue;
        }
        let (ranks, streaks) = unpack(word);
        if word >> 48 != 0 || streaks != [0; 4] {
            return Err(BuildError::BadCellWord { cell });
        }
        if ranks[0] == CANDIDATE_NONE {
            return Err(BuildError::NoneAtRank0 { cell });
        }
        let shape = cell_shape(cell);
        let domain = cell_domain(cell);
        let extra = t.affix_len[domain as usize];
        used[shape as usize] = true;
        let mut ended = false;
        for &id in &ranks {
            if id == CANDIDATE_NONE {
                ended = true;
            } else if ended {
                return Err(BuildError::NoneBeforeCandidate { cell });
            } else {
                check_candidate(t, domain, id, shape, extra)
                    .map_err(|e| e.unwrap_or(BuildError::SpanLenMismatch { cell, id }))?;
            }
        }
    }
    for (shape, _) in used.iter().enumerate().filter(|(_, u)| **u) {
        let shape = shape as ShapeId;
        let id = t.cardinal_default[shape as usize];
        if id == CANDIDATE_NONE || check_candidate(t, DOMAIN_CARDINAL, id, shape, 0).is_err() {
            return Err(BuildError::BadDefault { shape });
        }
    }
    Ok(())
}

fn check_candidate(
    t: &Tables,
    domain: DomainId,
    id: CandidateId,
    shape: ShapeId,
    extra: u8,
) -> Result<(), Option<BuildError>> {
    t.candidates
        .validate(domain, id)
        .map_err(|err| Some(BuildError::Candidate { id, err }))?;
    if t.candidates.span_len(domain, id) != t.shape_len[shape as usize] + extra {
        return Err(None);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DOMAIN_DATE, DOMAIN_ORDINAL, DOMAIN_PLAIN, SHAPE_UNKNOWN, Setting};

    fn shape_id(t: &Tables, phrase: &str) -> ShapeId {
        let words: Vec<&[u8]> = phrase.split(' ').map(str::as_bytes).collect();
        let roles: Vec<u8> = words
            .iter()
            .map(|w| t.role_of[t.lexicon.lookup(w).class as usize])
            .collect();
        assert!(roles.iter().all(|&r| r != 0), "{phrase} is not one span");
        t.shapes.lookup(shape_key(&roles))
    }

    fn ranks_of(t: &Tables, cell: CellIndex) -> [CandidateId; 4] {
        let word = t.initial_cells[cell as usize];
        assert_ne!(word, 0, "cell {cell:#x} exists");
        unpack(word).0
    }

    fn first_cell(t: &Tables) -> CellIndex {
        t.initial_cells
            .iter()
            .position(|&w| w != 0)
            .expect("a cell") as CellIndex
    }

    fn render_rank0(t: &Tables, phrase: &str, domain: DomainId) -> String {
        let words: Vec<&[u8]> = phrase.split(' ').map(str::as_bytes).collect();
        let ranks = ranks_of(t, cell_index(domain, shape_id(t, phrase)));
        let values: Vec<u32> = words
            .iter()
            .map(|w| t.value_of[t.lexicon.lookup(w).canonical as usize])
            .collect();
        let mut out = [0u8; 64];
        let mut pos = 0;
        t.candidates
            .render(domain, ranks[0], &values, &words, &mut out, &mut pos)
            .expect("render");
        String::from_utf8(out[..pos].to_vec()).expect("utf8")
    }

    #[test]
    fn build_english_succeeds_and_validates() {
        let (t, stats) = build_english_with_stats().expect("build");
        assert_eq!(validate(&t), Ok(()));
        assert!(stats.shapes >= 1 && stats.shapes <= MAX_SHAPES);
        assert!(stats.candidates <= 255);
        assert_eq!(stats.cells, stats.shapes * DOMAINS + stats.affix_cells);
        eprintln!("{stats:?}");
        for r in shape_report() {
            eprintln!(
                "{:<12} kept {:>3} dropped {:>4} mass {:.3}",
                r.kind, r.shapes_kept, r.shapes_dropped, r.mass_covered
            );
        }
    }

    #[test]
    fn sample_shapes_exist() {
        let t = build_english().expect("build");
        for p in [
            "five thirty",
            "one two three four",
            "five point two",
            "twenty first",
        ] {
            assert_ne!(shape_id(&t, p), SHAPE_UNKNOWN, "{p}");
        }
    }

    #[test]
    fn samples_render_through_rank0() {
        let t = build_english().expect("build");
        assert_eq!(render_rank0(&t, "five thirty", DOMAIN_TIME), "5:30");
        assert_eq!(render_rank0(&t, "five oh five", DOMAIN_TIME), "5:05");
        assert_eq!(
            render_rank0(&t, "one two three four", DOMAIN_IDENTIFIER),
            "1234"
        );
        assert_eq!(render_rank0(&t, "oh one two", DOMAIN_IDENTIFIER), "012");
        assert_eq!(render_rank0(&t, "five point two", DOMAIN_CARDINAL), "5.2");
        assert_eq!(render_rank0(&t, "twenty first", DOMAIN_CARDINAL), "21st");
        assert_eq!(
            render_rank0(&t, "one hundred twenty three", DOMAIN_CARDINAL),
            "123"
        );
        assert_eq!(
            render_rank0(&t, "two thousand five", DOMAIN_CARDINAL),
            "2005"
        );
    }

    fn render_reading(t: &Tables, phrase: &str, domain: DomainId) -> String {
        render_reading_at(t, phrase, domain, 0)
    }

    fn render_reading_at(t: &Tables, phrase: &str, domain: DomainId, rank: usize) -> String {
        let words: Vec<&[u8]> = phrase.split(' ').map(str::as_bytes).collect();
        let infos: Vec<_> = words.iter().map(|w| t.lexicon.lookup(w)).collect();
        let roles: Vec<Role> = infos.iter().map(|i| t.role_of[i.class as usize]).collect();
        assert!(roles.iter().all(|&r| r != 0), "{phrase} is not one span");
        let values: Vec<u32> = infos
            .iter()
            .map(|i| t.value_of[i.canonical as usize])
            .collect();
        let mut pool = Pool::new().expect("pool");
        let lists = pool
            .domain_lists(english::SRC_CARDINAL | english::SRC_TIME, &roles)
            .expect("lists");
        let local = pool.place(domain, &lists[domain as usize]).expect("place");
        let id = to_ranks(&local)[rank];
        assert_eq!(pool.table.validate(domain, id), Ok(()), "{phrase}");
        let mut out = [0u8; 64];
        let mut pos = 0;
        pool.table
            .render(domain, id, &values, &words, &mut out, &mut pos)
            .expect("render");
        String::from_utf8(out[..pos].to_vec()).expect("utf8")
    }

    fn assert_cardinal(t: &Tables, cases: &[(&str, &str)]) {
        for &(phrase, want) in cases {
            assert_eq!(render_reading(t, phrase, DOMAIN_CARDINAL), want, "{phrase}");
        }
    }

    #[test]
    fn cardinals_up_to_eight_tokens() {
        let t = build_english().expect("build");
        assert_cardinal(
            &t,
            &[
                ("thirty two thousand five hundred seventy one", "32,571"),
                ("sixteen million two hundred thousand", "16,200,000"),
                ("thirteen hundred", "1,300"),
                ("forty two hundred and six", "4,206"),
                ("twelve hundred eighty four", "1,284"),
            ],
        );
    }

    #[test]
    fn large_scale_word_ends_a_number_and_stays_a_word() {
        let t = build_english().expect("build");
        assert_cardinal(
            &t,
            &[
                ("seven hundred million", "700 million"),
                ("twenty three billion", "23 billion"),
                ("nine million", "9 million"),
                ("three point seven million", "3.7 million"),
                ("six point two five billion", "6.25 billion"),
                ("two point four thousand", "2.4 thousand"),
            ],
        );
    }

    #[test]
    fn decimals_keep_zero_digits_and_allow_no_integer_part() {
        let t = build_english().expect("build");
        assert_cardinal(
            &t,
            &[
                ("point zero seven", "0.07"),
                ("point four", "0.4"),
                ("seven point o five", "7.05"),
                ("six point zero", "6.0"),
                ("nine hundred twelve point five", "912.5"),
            ],
        );
    }

    #[test]
    fn spoken_meridiem_and_oclock_join_the_time_span() {
        let t = build_english().expect("build");
        assert_cardinal(
            &t,
            &[
                ("four p m", "4 pm"),
                ("ten twenty five p m", "10:25 pm"),
                ("nine oh seven p m", "9:07 pm"),
                ("six o'clock", "6 o'clock"),
                ("twelve oclock", "12 o'clock"),
            ],
        );
        assert_eq!(render_rank0(&t, "eleven forty", DOMAIN_TIME), "11:40");
    }

    #[test]
    fn single_token_meridiem_is_written_as_is_then_zero_padded() {
        let t = build_english().expect("build");
        for (phrase, as_is, padded) in [
            ("five thirty pm", "5:30 pm", "05:30 pm"),
            ("five pm", "5 pm", "05 pm"),
            ("eleven fifty five am", "11:55 am", "11:55 am"),
        ] {
            assert_eq!(render_reading_at(&t, phrase, DOMAIN_CARDINAL, 0), as_is);
            assert_eq!(render_reading_at(&t, phrase, DOMAIN_CARDINAL, 1), padded);
        }
    }

    #[test]
    fn zero_padded_time_is_the_next_rank() {
        let t = build_english().expect("build");
        for (phrase, domain, want) in [
            ("four p m", DOMAIN_CARDINAL, "04:00 pm"),
            ("six o'clock", DOMAIN_CARDINAL, "06:00"),
            ("five thirty", DOMAIN_TIME, "05:30"),
            ("nine oh seven", DOMAIN_TIME, "09:07"),
        ] {
            assert_eq!(render_reading_at(&t, phrase, domain, 1), want, "{phrase}");
        }
    }

    #[test]
    fn shape_ids_keep_every_generated_shape() {
        let b = build(&Learned::default()).expect("build");
        assert_eq!(b.stats.shapes_dropped, 0);
        assert!(b.stats.shapes > 744 && b.stats.shapes <= MAX_SHAPES);
        let t = &b.tables;
        for phrase in [
            "point zero two",
            "twenty fifth twenty twelve",
            "eight hundred eight point eight",
            "eleven dollars and fifty one cents",
            "eighteen point o o o",
            "point one two o five",
            "eight hundred eighteen point three o three",
            "sixty point two four zero zero",
            "eighteen thousand eight hundred one",
            "eighteen thousand nine hundred three",
            "eighteen million nine hundred twenty five thousand",
            "two million three",
            "one billion five hundred ninety three million",
            "eighteen billion sixty million",
            "zero feet",
            "zero point two million dollars",
        ] {
            let words: Vec<&[u8]> = phrase.split(' ').map(str::as_bytes).collect();
            let roles: Vec<Role> = words
                .iter()
                .map(|w| t.role_of[t.lexicon.lookup(w).class as usize])
                .collect();
            let key = shape_key(core_roles(&roles));
            assert_ne!(t.shapes.lookup(key), SHAPE_UNKNOWN, "{phrase}");
        }
    }

    #[test]
    fn per_domain_candidate_tables_place_every_affix_cell() {
        let b = build(&Learned::default()).expect("build");
        assert_eq!(b.stats.affix_dropped, 0);
        for (&cell, list) in &b.lists {
            let d = cell_domain(cell);
            for &id in list {
                assert_ne!(id, CANDIDATE_NONE);
                let ops = &b.ops[&(d, id)];
                let c = &b.tables.candidates.records(d)[id as usize];
                assert_eq!(&c.ops[..c.op_count as usize], &ops[..], "cell {cell:#x}");
            }
        }
        let engine = crate::engine::Normalizer::new(&b.tables);
        let mut line = String::new();
        crate::engine::window::format_line(&engine, "point two meters", &mut line);
        assert_eq!(line, "0.2 meters");
    }

    #[test]
    fn lone_zero_word_stays_words_unless_an_affix_follows() {
        let b = build(&Learned::default()).expect("build");
        let engine = crate::engine::Normalizer::new(&b.tables).without(Setting::SingleDigits);
        for (text, want) in [
            ("o lord", "o lord"),
            ("oh dear", "oh dear"),
            ("zero", "zero"),
            ("zero feet", "0 feet"),
            ("eighteen point o o o", "18.000"),
            ("five", "five"),
            ("five dollars", "$5"),
            ("twenty one", "21"),
            ("fifteen", "15"),
        ] {
            let mut line = String::new();
            crate::engine::window::format_line(&engine, text, &mut line);
            assert_eq!(line, want, "{text}");
        }
    }

    #[test]
    fn words_first_bit_keeps_a_lone_word_in_cardinal_and_ordinal_context() {
        let mut b = build(&Learned::default()).expect("build");
        for w in ["one", "first"] {
            let c = b.tables.lexicon.lookup(w.as_bytes()).canonical as usize;
            b.tables.words_first[c / 64] |= 1 << (c % 64);
        }
        let engine = crate::engine::Normalizer::new(&b.tables).without(Setting::SingleDigits);
        for (text, want) in [
            ("this one only", "this one only"),
            ("first we saved", "first we saved"),
            ("five", "five"),
            ("twenty one", "21"),
            ("one dollar", "$1"),
            ("at one pm", "at 1 pm"),
            ("one", "one"),
            ("at first sight", "at first sight"),
            ("at second glance", "at second glance"),
            ("at five", "at 5"),
        ] {
            let mut line = String::new();
            crate::engine::window::format_line(&engine, text, &mut line);
            assert_eq!(line, want, "{text}");
        }
        let tokens: Vec<&[u8]> = vec![b"this", b"one", b"only"];
        let mut out = [0u8; 32];
        let mut receipts = [crate::types::SpanReceipt::default(); MAX_TOKENS];
        let f = engine
            .format_receipts(&tokens, &mut out, &mut receipts)
            .expect("format");
        assert_eq!(f.spans, 1);
        assert_eq!(cell_domain(receipts[0].cell), DOMAIN_PLAIN);
    }

    #[test]
    fn month_dates_capitalize_the_month_and_comma_only_before_a_year() {
        let t = build_english().expect("build");
        let engine = crate::engine::Normalizer::new(&t).without(Setting::SingleDigits);
        for (text, want) in [
            ("january eighth eighteen fifteen", "January 8th, 1815"),
            ("july two thousand twelve", "July 2012"),
            ("june fifth twenty people", "june fifth twenty people"),
            ("on june fifth", "on June 5th"),
            ("he died may eleventh", "he died May 11th"),
            ("may nineteen ninety", "May 1990"),
            ("you may one day", "you may one day"),
            ("you may two days", "you may two days"),
            ("may five hundred", "may 500"),
            ("you may go", "you may go"),
            ("in march", "in march"),
            ("sixty seven january seventeenth", "67 January 17th"),
            (
                "the fifth april seventeen seventy one",
                "the 5th April 1771",
            ),
            ("five minus three", "five minus three"),
            ("minus fifty", "minus 50"),
        ] {
            let mut line = String::new();
            crate::engine::window::format_line(&engine, text, &mut line);
            assert_eq!(line, want, "{text}");
        }
    }

    #[test]
    fn and_leaves_a_span_it_ends_and_splits_a_run_with_no_shape() {
        let t = build_english().expect("build");
        let engine = crate::engine::Normalizer::new(&t);
        for (text, want) in [
            ("in eighteen ninety two and then", "in 1892 and then"),
            (
                "once in eighteen ninety two and once in eighteen ninety three",
                "once in 1892 and once in 1893",
            ),
            (
                "by the thirteenth and fourteenth of june",
                "by the 13th and 14th of June",
            ),
            ("ages twelve and thirteen", "ages 12 and 13"),
            ("four hundred and fifty three", "453"),
            ("one hundred and five", "105"),
            ("one hundred and five and twelve", "105 and 12"),
            (
                "seven hundred and one point one one",
                "seven hundred and one point one one",
            ),
            ("a hundred and fifty dollars", "$150"),
            ("eighteen hundred and more", "eighteen hundred and more"),
            ("four dollars and so on", "$4 and so on"),
            (
                "four million dollars and five million dollars",
                "$4 million and $5 million",
            ),
        ] {
            let mut line = String::new();
            crate::engine::window::format_line(&engine, text, &mut line);
            assert_eq!(line, want, "{text}");
        }
    }

    #[test]
    fn parts_that_break_place_value_stay_words_and_years_need_a_ten_to_twenty_nine_lead() {
        let t = build_english().expect("build");
        let engine = crate::engine::Normalizer::new(&t);
        for (text, want) in [
            ("it was fifty fifty", "it was fifty fifty"),
            ("one fifth of it", "one fifth of it"),
            ("it was twenty fifteenth", "it was twenty fifteenth"),
            ("we sold fifty fifteen", "we sold fifty fifteen"),
            ("thirty thirty", "thirty thirty"),
            ("seventy seventy", "seventy seventy"),
            ("in twenty fifteen", "in 2015"),
            ("nineteen oh five", "1905"),
            ("twenty twenty six", "2026"),
            ("nineteen eighty four", "1984"),
            ("twelve fifteen", "1215"),
            ("twenty fifth", "25th"),
            ("twenty five", "25"),
            ("one hundred five", "105"),
            ("sixty one hundred", "6,100"),
            ("two thousand five", "2005"),
            ("five thousand twenty", "5,020"),
            ("at five thirty", "at 5:30"),
            ("january eighth eighteen fifteen", "January 8th, 1815"),
        ] {
            let mut line = String::new();
            crate::engine::window::format_line(&engine, text, &mut line);
            assert_eq!(line, want, "{text}");
        }
    }

    #[test]
    fn units_before_per_keep_their_words_and_temperatures_take_two_tokens() {
        let t = build_english().expect("build");
        let engine = crate::engine::Normalizer::new(&t);
        for (text, want) in [
            ("sixty miles per hour", "60 miles per hour"),
            ("two miles per hour", "2 miles per hour"),
            ("we ran two miles per hour", "we ran 2 miles per hour"),
            ("two hundred kilometers per hour", "200 kilometers per hour"),
            ("five dollars per pound", "$5 per pound"),
            ("five miles", "5 miles"),
            ("seventy two degrees fahrenheit", "72 degrees Fahrenheit"),
            ("five degrees celsius", "5 degrees Celsius"),
            ("thirty degrees", "30 degrees"),
            ("seventy fahrenheit", "70 fahrenheit"),
            ("ten megahertz", "10 megahertz"),
            ("five milliwatts", "5 milliwatts"),
            ("eight hundred kilowatts", "800 kilowatts"),
        ] {
            let mut line = String::new();
            crate::engine::window::format_line(&engine, text, &mut line);
            assert_eq!(line, want, "{text}");
        }
    }

    #[test]
    fn month_date_cells_rank_the_spoken_form_second() {
        let t = build_english().expect("build");
        let engine = crate::engine::Normalizer::new(&t);
        let tokens: Vec<&[u8]> = vec![b"january", b"eighth", b"eighteen", b"fifteen"];
        let mut out = [0u8; 64];
        let mut receipts = [crate::types::SpanReceipt::default(); MAX_TOKENS];
        engine
            .format_receipts(&tokens, &mut out, &mut receipts)
            .expect("format");
        let cell = receipts[0].cell;
        assert_eq!(cell_domain(cell), english::DOMAIN_MONTH_DATE);
        let (ranks, _) = unpack(t.initial_cells[cell as usize]);
        let render = |cand| {
            let mut buf = [0u8; 64];
            let mut pos = 0;
            let values: Vec<u32> = tokens
                .iter()
                .map(|w| t.value_of[t.lexicon.lookup(w).canonical as usize])
                .collect();
            t.candidates
                .render(
                    english::DOMAIN_MONTH_DATE,
                    cand,
                    &values,
                    &tokens,
                    &mut buf,
                    &mut pos,
                )
                .expect("render");
            String::from_utf8(buf[..pos].to_vec()).expect("utf8")
        };
        assert_eq!(render(ranks[0]), "January 8th, 1815");
        assert_eq!(render(ranks[1]), "january 8th 1815");
        assert_eq!(render(ranks[2]), "January 8th 1815");
    }

    #[test]
    fn decade_words_render_their_value_with_an_s() {
        let t = build_english().expect("build");
        let engine = crate::engine::Normalizer::new(&t);
        for (text, want) in [
            ("the nineteen eighties", "the 1980s"),
            ("the twenty twenties", "the 2020s"),
            ("the twenties", "the 20s"),
            ("upper sixties", "upper 60s"),
            ("a bounty of thirties.", "a bounty of 30s."),
            ("the nineteen eighty", "the 1980"),
            ("sixty", "60"),
            ("hundreds of people", "hundreds of people"),
        ] {
            let mut line = String::new();
            crate::engine::window::format_line(&engine, text, &mut line);
            assert_eq!(line, want, "{text}");
        }
        let marked = |w: &str| {
            let c = t.lexicon.lookup(w.as_bytes()).canonical as usize;
            t.plural[c / 64] >> (c % 64) & 1 == 1
        };
        assert!(marked("nineties"));
        assert!(!marked("ninety"));
    }

    #[test]
    fn english_marks_only_the_words_first_list() {
        let t = build_english().expect("build");
        let marked = |w: &str| {
            let c = t.lexicon.lookup(w.as_bytes()).canonical as usize;
            t.words_first[c / 64] >> (c % 64) & 1 == 1
        };
        for w in english::WORDS_FIRST {
            assert!(marked(w), "{w}");
        }
        for w in ["twenty", "fifteen", "hundred", "thousand", "point"] {
            assert!(!marked(w), "{w}");
        }
    }

    #[test]
    fn trailing_scale_keeps_its_word_only_without_a_higher_scale_before_it() {
        let t = build_english().expect("build");
        for (phrase, want) in [
            ("four hundred million", "400 million"),
            ("five point two million", "5.2 million"),
            (
                "one billion five hundred ninety three million",
                "1,593,000,000",
            ),
            ("five billion two million", "5,002,000,000"),
        ] {
            assert_eq!(render_rank0(&t, phrase, DOMAIN_CARDINAL), want, "{phrase}");
        }
    }

    #[test]
    fn composite_money_cores_have_cents_cells() {
        let t = build_english().expect("build");
        for core in [
            "four dollars and fifty",
            "forty eight dollars and ninety six",
            "seven dollars fifty",
            "three dollars fifty three",
        ] {
            let shape = shape_id(&t, core);
            assert_ne!(shape, 0, "{core} has a shape");
            ranks_of(&t, cell_index(DOMAIN_CENTS, shape));
        }
    }

    #[test]
    fn digit_then_two_digit_group_reads_as_hundreds() {
        let t = build_english().expect("build");
        assert_cardinal(
            &t,
            &[
                ("one fifty five", "155"),
                ("nine ninety nine", "999"),
                ("two twenty", "220"),
                ("four fifteen", "415"),
            ],
        );
        assert_eq!(render_affix(&t, "one fifty five dollars"), "$155");
    }

    #[test]
    fn money_and_percent() {
        let t = build_english().expect("build");
        assert_cardinal(&t, &[("three dollars ninety nine", "$3.99")]);
        for (phrase, want) in [
            ("seven dollars", "$7"),
            ("one dollar", "$1"),
            ("three hundred dollars", "$300"),
            ("forty two dollars and fifteen cents", "$42.15"),
            ("sixty cents", "60 cents"),
            ("one cent", "1 cent"),
            ("twelve million dollars", "$12 million"),
            ("four point five billion dollars", "$4.5 billion"),
            ("twenty percent", "20%"),
            ("three point two percent", "3.2%"),
            ("two hundred meters", "200 meters"),
            ("five euros", "5 euros"),
        ] {
            assert_eq!(render_affix(&t, phrase), want, "{phrase}");
        }
    }

    fn render_affix(t: &Tables, phrase: &str) -> String {
        let words: Vec<&str> = phrase.split(' ').collect();
        let bytes: Vec<&[u8]> = words.iter().map(|w| w.as_bytes()).collect();
        let infos: Vec<_> = bytes.iter().map(|w| t.lexicon.lookup(w)).collect();
        let roles: Vec<Role> = infos.iter().map(|i| t.role_of[i.class as usize]).collect();
        let values: Vec<u32> = infos
            .iter()
            .map(|i| t.value_of[i.canonical as usize])
            .collect();
        let slot = affix_slot(&words, &roles).expect("affix");
        let mut pool = Pool::new().expect("pool");
        let d = AFFIX_DOMAINS[slot].0;
        let list = pool.affix_list(slot, core_roles(&roles)).expect("list");
        let id = pool.place(d, &list).expect("place")[0];
        assert_eq!(pool.table.validate(d, id), Ok(()), "{phrase}");
        let mut out = [0u8; 64];
        let mut pos = 0;
        pool.table
            .render(d, id, &values, &bytes, &mut out, &mut pos)
            .expect("render");
        String::from_utf8(out[..pos].to_vec()).expect("utf8")
    }

    #[test]
    fn shape_budget_covers_most_of_every_kind() {
        for r in shape_report() {
            assert!(r.mass_covered >= 0.7, "{r:?}");
        }
    }

    #[test]
    fn years_read_as_years_without_context() {
        let t = build_english().expect("build");
        assert_cardinal(
            &t,
            &[
                ("nineteen eighty four", "1984"),
                ("twenty nineteen", "2019"),
                ("eighteen oh two", "1802"),
                ("two thousand and eight", "2008"),
                ("two thousand four", "2004"),
                ("nineteen hundred", "1,900"),
            ],
        );
    }

    #[test]
    fn month_context_reads_days_and_years() {
        use english::{MONTH, THE};
        let t = build_english().expect("build");
        assert_eq!(t.context.lookup(MONTH, THE).0, english::DOMAIN_MONTH_DATE);
        for (phrase, want) in [
            ("twenty third", "23rd"),
            ("fourth", "4th"),
            ("thirty", "30"),
            ("nineteen seventy", "1970"),
            ("fourth nineteen ninety nine", "4th 1999"),
            ("eleventh two thousand three", "11th 2003"),
            ("twenty first twenty ten", "21st 2010"),
        ] {
            assert_eq!(render_reading(&t, phrase, DOMAIN_DATE), want, "{phrase}");
        }
    }

    #[test]
    fn ordinal_scale_words_and_zeroth() {
        let t = build_english().expect("build");
        assert_cardinal(
            &t,
            &[
                ("zeroth", "0th"),
                ("one hundredth", "100th"),
                ("three thousandth", "3,000th"),
                ("fourteen hundred thirty second", "1,432nd"),
            ],
        );
    }

    #[test]
    fn time_only_shape_stays_words_in_cardinal() {
        use english::THE;
        let t = build_english().expect("build");
        assert_eq!(t.context.lookup(THE, THE).0, DOMAIN_CARDINAL);
        assert_eq!(
            render_rank0(&t, "five thirty", DOMAIN_CARDINAL),
            "five thirty"
        );
        let shape = shape_id(&t, "five thirty");
        let ranks = ranks_of(&t, cell_index(DOMAIN_CARDINAL, shape));
        assert_eq!(t.cardinal_default[shape as usize], ranks[0]);
        let mut out = [0u8; 16];
        let mut pos = 0;
        t.candidates
            .render(
                DOMAIN_CARDINAL,
                ranks[1],
                &[5, 30],
                &[b"five", b"thirty"],
                &mut out,
                &mut pos,
            )
            .expect("render");
        assert_eq!(&out[..pos], b"530");
        assert_eq!(render_rank0(&t, "five thirty", DOMAIN_TIME), "5:30");
    }

    #[test]
    fn every_shape_has_time_and_identifier_cells() {
        let t = build_english().expect("build");
        for (cell, _) in t.initial_cells.iter().enumerate().filter(|(_, w)| **w != 0) {
            let shape = cell_shape(cell as CellIndex);
            for domain in 0..DOMAINS as DomainId {
                assert_ne!(t.initial_cells[cell_index(domain, shape) as usize], 0);
            }
        }
    }

    fn rule(domain: DomainId, confidence: u8) -> learned::Rule {
        learned::Rule {
            domain,
            confidence,
            count: 50,
        }
    }

    #[test]
    fn plain_and_ordinal_domains_have_cells() {
        let t = build_english().expect("build");
        assert_eq!(render_rank0(&t, "five thirty", DOMAIN_PLAIN), "five thirty");
        assert_eq!(render_rank0(&t, "twenty first", DOMAIN_ORDINAL), "21st");
    }

    #[test]
    fn learned_cues_join_the_lexicon_as_context_classes() {
        let l = Learned {
            cues: vec![("around".into(), 0), ("of".into(), 1)],
            ..Learned::default()
        };
        let b = build(&l).expect("build");
        let first = b.first_cue;
        assert_eq!(b.tables.lexicon.lookup(b"around").class, first);
        assert_eq!(b.tables.lexicon.lookup(b"of").class, first + 1);
        assert_eq!(b.tables.role_of[first as usize], 0);
        assert_eq!(b.class_ref(first + 1), ClassRef::Cue(1));
        assert_eq!(b.class_ref(english::THE), ClassRef::Word("the".into()));
        assert_eq!(b.class_ref(0), ClassRef::Boundary);
        assert_eq!(
            b.stats.lexicon_entries,
            build(&Learned::default()).expect("b").stats.lexicon_entries + 2
        );
    }

    #[test]
    fn learned_context_backs_off_pair_left_right_prior() {
        use learned::ClassRef::*;
        let l = Learned {
            cues: vec![("around".into(), 0)],
            pairs: vec![(Cue(0), Word("room".into()), rule(DOMAIN_TIME, 14))],
            lefts: vec![(Cue(0), rule(DOMAIN_DATE, 9))],
            rights: vec![(Word("the".into()), rule(DOMAIN_PLAIN, 7))],
            prior: Some(rule(DOMAIN_CARDINAL, 5)),
            ..Learned::default()
        };
        let b = build(&l).expect("build");
        let (cue, ctx) = (b.first_cue, &b.tables.context);
        assert_eq!(ctx.lookup(cue, english::ROOM), (DOMAIN_TIME, 14));
        assert_eq!(ctx.lookup(cue, english::THE), (DOMAIN_DATE, 9));
        assert_eq!(ctx.lookup(0, english::THE), (DOMAIN_PLAIN, 7));
        assert_eq!(ctx.lookup(1, 0), (DOMAIN_CARDINAL, 5));
        assert_eq!(
            ctx.lookup(english::MONTH, 0),
            (english::DOMAIN_MONTH_DATE, 15)
        );
        assert_eq!(
            ctx.lookup(english::MAY, english::THE),
            (english::DOMAIN_MAY_DATE, 15)
        );
        assert_eq!(ctx.lookup(english::ROOM, 0), (DOMAIN_IDENTIFIER, 12));
        assert_eq!(ctx.lookup(1, english::PM), (DOMAIN_TIME, 15));
        let strong = Learned {
            rights: vec![(Word("the".into()), rule(DOMAIN_PLAIN, 14))],
            lefts: vec![(Word("room".into()), rule(DOMAIN_PLAIN, 14))],
            prior: Some(rule(DOMAIN_PLAIN, 14)),
            ..Learned::default()
        };
        let b = build(&strong).expect("build");
        let ctx = &b.tables.context;
        assert_eq!(ctx.lookup(english::AT, english::THE), (DOMAIN_TIME, 8));
        assert_eq!(ctx.lookup(english::ROOM, 0), (DOMAIN_PLAIN, 14));
        assert_eq!(ctx.lookup(1, english::THE), (DOMAIN_PLAIN, 14));
    }

    #[test]
    fn learned_cell_order_promotes_and_keeps_rule_order_after() {
        let pool = Pool::new().expect("pool");
        let padded = vec![
            op::fold(0, 1),
            op::fold(1, 1),
            op::fit(0, 0, MAX_HOUR),
            op::fit(1, 0, MAX_MINUTE),
            op::emit(0, 2),
            op::lit(pool.colon),
            op::emit(1, 2),
        ];
        let key = shape_key(&[english::ROLE_DIGIT, english::ROLE_TENS]);
        let l = Learned {
            cells: vec![learned::Cell {
                domain: DOMAIN_TIME,
                key,
                ranked: vec![(3, padded), (1, vec![op::raw(9)])],
            }],
            ..Learned::default()
        };
        let t = build(&l).expect("build").tables;
        assert_eq!(render_rank0(&t, "five thirty", DOMAIN_TIME), "05:30");
        let rules = build_english().expect("rules");
        let cell = cell_index(DOMAIN_TIME, shape_id(&t, "five thirty"));
        let (r, l) = (ranks_of(&rules, cell), ranks_of(&t, cell));
        assert_eq!(l, [r[1], r[0], r[2], r[3]]);
    }

    #[test]
    fn learned_refs_must_resolve() {
        use learned::ClassRef::*;
        for bad in [Word("five".into()), Word("zebra".into()), Cue(3)] {
            let l = Learned {
                lefts: vec![(bad.clone(), rule(0, 1))],
                prior: Some(rule(0, 1)),
                ..Learned::default()
            };
            assert_eq!(build(&l).err(), Some(BuildError::BadClassRef), "{bad:?}");
        }
        let l = Learned {
            cues: vec![("five".into(), 0)],
            ..Learned::default()
        };
        assert_eq!(build(&l).err(), Some(BuildError::CueClash));
    }

    #[test]
    fn learned_shape_counts_rank_the_shape_budget() {
        use english::{ROLE_DIGIT, ROLE_TENS};
        use learned::ShapeCount;
        let odd = shape_key(&[ROLE_TENS; 5]);
        let base = build_english().expect("build");
        assert_eq!(base.shapes.lookup(odd), 0);
        let counted = |floor| Learned {
            shapes: vec![ShapeCount {
                key: odd,
                count: 1000,
                src: english::SRC_IDENTIFIER,
            }],
            floor: Some(floor),
            ..Learned::default()
        };
        let b = build(&counted(200)).expect("build");
        let shape = b.tables.shapes.lookup(odd);
        assert_ne!(shape, 0);
        assert_eq!(b.tables.shape_len[shape as usize], 5);
        assert_eq!(
            render_rank0(
                &b.tables,
                "twenty thirty forty fifty sixty",
                DOMAIN_IDENTIFIER
            ),
            "2030405060"
        );
        let prior_only = build(&counted(1000)).expect("build");
        assert_eq!(prior_only.tables.shapes.lookup(odd), 0);
        assert_eq!(
            prior_only.keys,
            build(&Learned::default()).expect("build").keys
        );
        let digit = shape_key(&[ROLE_DIGIT]);
        assert_ne!(
            build(&counted(0))
                .expect("build")
                .tables
                .shapes
                .lookup(digit),
            0
        );
        for key in [0x0101, 0x1001] {
            let bad = Learned {
                shapes: vec![ShapeCount {
                    key,
                    count: 1,
                    src: 1,
                }],
                ..Learned::default()
            };
            assert_eq!(build(&bad).err(), Some(BuildError::BadShapeKey), "{key:#x}");
        }
    }

    #[test]
    fn affix_rows_route_to_affix_domains_even_with_learned_data() {
        use english::*;
        let learned = Learned {
            prior: Some(rule(DOMAIN_PLAIN, 15)),
            ..Learned::default()
        };
        for t in [
            build_english().expect("b"),
            build(&learned).expect("b").tables,
        ] {
            assert_eq!(t.context.lookup(THE, DOLLAR), (DOMAIN_MONEY, 15));
            assert_eq!(t.context.lookup(0, CENT), (DOMAIN_CENTS, 15));
            assert_eq!(t.context.lookup(1, PERCENT), (DOMAIN_PERCENT, 15));
            assert_eq!(t.affix_len[DOMAIN_MONEY as usize], 1);
            assert_eq!(t.affix_len[DOMAIN_CARDINAL as usize], 0);
        }
    }

    #[test]
    fn affix_cells_render_the_full_span_from_the_core_shape() {
        let t = build_english().expect("build");
        let core = shape_id(&t, "two thousand five");
        let words: Vec<&[u8]> = "two thousand five dollars"
            .split(' ')
            .map(str::as_bytes)
            .collect();
        let values: Vec<u32> = words
            .iter()
            .map(|w| t.value_of[t.lexicon.lookup(w).canonical as usize])
            .collect();
        let rank0 = ranks_of(&t, cell_index(DOMAIN_MONEY, core))[0];
        let mut out = [0u8; 64];
        let mut pos = 0;
        t.candidates
            .render(DOMAIN_MONEY, rank0, &values, &words, &mut out, &mut pos)
            .expect("render");
        assert_eq!(&out[..pos], b"$2,005");
    }

    #[test]
    fn validate_rejects_candidate_that_skips_the_affix() {
        let mut t = build_english().expect("build");
        let core = shape_id(&t, "two thousand five");
        let card = t.initial_cells[cell_index(DOMAIN_CARDINAL, core) as usize];
        t.initial_cells[cell_index(DOMAIN_MONEY, core) as usize] = card;
        assert!(matches!(
            validate(&t),
            Err(BuildError::SpanLenMismatch { .. })
        ));
    }

    #[test]
    fn context_lut_routes_by_neighbors() {
        use english::*;
        let t = build_english().expect("build");
        assert_eq!(t.context.lookup(0, PM), (DOMAIN_TIME, 15));
        assert_eq!(t.context.lookup(AT, OCLOCK), (DOMAIN_TIME, 15));
        assert_eq!(t.context.lookup(AT, 0), (DOMAIN_TIME, 8));
        assert_eq!(t.context.lookup(ROOM, THE), (DOMAIN_IDENTIFIER, 12));
        assert_eq!(t.context.lookup(THE, 1), (DOMAIN_CARDINAL, 4));
    }

    #[test]
    fn validate_rejects_wrong_span_len() {
        let mut t = build_english().expect("build");
        let shape = cell_shape(first_cell(&t));
        t.shape_len[shape as usize] += 1;
        assert!(matches!(
            validate(&t),
            Err(BuildError::SpanLenMismatch { .. })
        ));
    }

    #[test]
    fn validate_rejects_none_at_rank0() {
        let mut t = build_english().expect("build");
        let cell = first_cell(&t);
        let r0 = ranks_of(&t, cell)[0];
        t.initial_cells[cell as usize] = pack([0, r0, 0, 0], [0; 4]);
        assert_eq!(validate(&t), Err(BuildError::NoneAtRank0 { cell }));
    }

    #[test]
    fn validate_rejects_none_before_candidate() {
        let mut t = build_english().expect("build");
        let cell = first_cell(&t);
        let r0 = ranks_of(&t, cell)[0];
        t.initial_cells[cell as usize] = pack([r0, 0, r0, 0], [0; 4]);
        assert_eq!(validate(&t), Err(BuildError::NoneBeforeCandidate { cell }));
    }

    #[test]
    fn validate_rejects_streaks_and_reserved_bits_in_initial_words() {
        let mut t = build_english().expect("build");
        let cell = first_cell(&t);
        let word = t.initial_cells[cell as usize];
        t.initial_cells[cell as usize] = word | (1 << 36);
        assert_eq!(validate(&t), Err(BuildError::BadCellWord { cell }));
        t.initial_cells[cell as usize] = word | (1 << 48);
        assert_eq!(validate(&t), Err(BuildError::BadCellWord { cell }));
    }

    #[test]
    fn validate_rejects_bad_cardinal_default() {
        let mut t = build_english().expect("build");
        let shape = cell_shape(first_cell(&t));
        t.cardinal_default[shape as usize] = 0;
        assert_eq!(validate(&t), Err(BuildError::BadDefault { shape }));
    }
}
