use clm_itn::engine::Engine;
use clm_itn::learn::cell::unpack;
use clm_itn::learn::ring::channel;
use clm_itn::learn::worker::drain;
use clm_itn::types::{
    CANDIDATE_NONE, CELL_ABSTAINED, DEFAULT_H, DOMAIN_CARDINAL, DOMAIN_IDENTIFIER, DOMAIN_TIME,
    DomainId, MAX_TOKENS, SpanReceipt, cell_domain,
};

const ABSTAINED: usize = 4;
const SLICES: usize = 5;
const NAMES: [&str; SLICES] = ["cardinal", "time", "identifier", "ordinal", "abstained"];
const RING: usize = 1024;

const HOURS: [&str; 12] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
    "twelve",
];
const MINUTES: [&str; 4] = ["oh five", "fifteen", "thirty", "forty five"];
const ID_WORDS: [&str; 6] = ["room", "flight", "code", "pin", "extension", "number"];
const IDS: [&str; 6] = [
    "four oh two",
    "oh four two",
    "one two three",
    "seven one two three",
    "five",
    "nine eight seven six",
];
const CARDINALS: [&str; 12] = [
    "one hundred twenty three",
    "two thousand five",
    "nine thousand nine hundred ninety nine",
    "forty two",
    "eleven",
    "five",
    "five thirty",
    "one two three four",
    "three point one four",
    "zero point five",
    "the twenty first",
    "the tenth",
];

struct Item {
    text: String,
    slice: usize,
    receipt: SpanReceipt,
}

fn tokens(text: &str) -> Vec<&[u8]> {
    text.split(' ').map(str::as_bytes).collect()
}

fn format(engine: &Engine, text: &str) -> (String, Vec<SpanReceipt>) {
    let mut out = [0u8; 256];
    let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
    let done = engine
        .format(&tokens(text), &mut out, &mut receipts)
        .expect("format");
    let s = String::from_utf8(out[..done.len].to_vec()).expect("utf8");
    (s, receipts[..done.spans].to_vec())
}

fn slice_of(receipt: SpanReceipt) -> usize {
    if receipt.cell == CELL_ABSTAINED {
        ABSTAINED
    } else {
        cell_domain(receipt.cell) as usize
    }
}

fn utterances() -> Vec<String> {
    let mut out = Vec::new();
    for h in HOURS {
        for m in MINUTES {
            for suffix in ["am", "pm"] {
                out.push(format!("at {h} {m} {suffix}"));
            }
        }
        out.push(format!("at {h}"));
        out.push(format!("{h} pm"));
    }
    for w in ID_WORDS {
        for id in IDS {
            out.push(format!("{w} {id}"));
        }
    }
    out.extend(CARDINALS.iter().map(|s| s.to_string()));
    out
}

fn validation_set(engine: &Engine) -> Vec<Item> {
    utterances()
        .into_iter()
        .map(|text| {
            let (_, receipts) = format(engine, &text);
            assert_eq!(receipts.len(), 1, "{text:?} must have exactly one span");
            Item {
                slice: slice_of(receipts[0]),
                receipt: receipts[0],
                text,
            }
        })
        .collect()
}

fn outputs(engine: &Engine, items: &[Item]) -> Vec<String> {
    items.iter().map(|i| format(engine, &i.text).0).collect()
}

fn rank1_text(engine: &Engine, item: &Item) -> Option<Vec<u8>> {
    let t = engine.tables();
    let all = tokens(&item.text);
    let (start, len) = (item.receipt.start as usize, item.receipt.len as usize);
    let span = &all[start..start + len];
    let (ranks, _) = unpack(engine.cells().load(item.receipt.cell));
    if ranks[1] == CANDIDATE_NONE {
        return None;
    }
    let values: Vec<u32> = span
        .iter()
        .map(|w| t.value_of[t.lexicon.lookup(w).canonical as usize])
        .collect();
    let mut buf = [0u8; 256];
    let mut pos = 0;
    t.candidates
        .render(
            clm_itn::types::cell_domain(item.receipt.cell),
            ranks[1],
            &values,
            span,
            &mut buf,
            &mut pos,
        )
        .expect("render rank1");
    Some(buf[..pos].to_vec())
}

fn burst(engine: &Engine, items: &[Item], domain: DomainId) {
    let targets: Vec<(&Item, Vec<u8>)> = items
        .iter()
        .filter(|i| i.slice == domain as usize)
        .filter_map(|i| rank1_text(engine, i).map(|t| (i, t)))
        .collect();
    assert!(!targets.is_empty());
    assert!(targets.len() <= RING);
    let (mut producer, mut consumer) = channel(RING);
    for _ in 0..DEFAULT_H as usize * 3 {
        for (item, target) in &targets {
            engine
                .correct(&mut producer, item.receipt, &tokens(&item.text), target)
                .expect("in-set correction");
        }
        drain(&mut consumer, engine.cells(), DEFAULT_H);
    }
    assert_eq!(producer.dropped(), 0);
}

fn run_drift(target: DomainId) -> [(usize, usize); SLICES] {
    let engine = Engine::new(clm_itn::english());
    let items = validation_set(&engine);
    let before = outputs(&engine, &items);
    burst(&engine, &items, target);
    let after = outputs(&engine, &items);
    let mut stats = [(0usize, 0usize); SLICES];
    for ((item, b), a) in items.iter().zip(&before).zip(&after) {
        stats[item.slice].0 += 1;
        stats[item.slice].1 += usize::from(b != a);
    }
    for (name, (size, changed)) in NAMES.iter().zip(stats) {
        eprintln!(
            "burst {}: {name} slice size {size}, changed {changed}",
            NAMES[target as usize]
        );
    }
    stats
}

fn assert_covers_routed_domains(stats: &[(usize, usize); SLICES]) {
    for d in [DOMAIN_CARDINAL, DOMAIN_TIME, DOMAIN_IDENTIFIER] {
        assert!(
            stats[d as usize].0 > 0,
            "{} slice is empty",
            NAMES[d as usize]
        );
    }
}

#[test]
fn time_burst_changes_only_time_outputs() {
    let s = run_drift(DOMAIN_TIME);
    assert_covers_routed_domains(&s);
    assert_eq!(s[DOMAIN_CARDINAL as usize].1, 0);
    assert_eq!(s[DOMAIN_IDENTIFIER as usize].1, 0);
    assert_eq!(s[ABSTAINED].1, 0);
    assert!(s[DOMAIN_TIME as usize].1 > 0, "burst had no effect");
}

#[test]
fn identifier_burst_changes_only_identifier_outputs() {
    let s = run_drift(DOMAIN_IDENTIFIER);
    assert_covers_routed_domains(&s);
    assert_eq!(s[DOMAIN_CARDINAL as usize].1, 0);
    assert_eq!(s[DOMAIN_TIME as usize].1, 0);
    assert_eq!(s[ABSTAINED].1, 0);
    assert!(s[DOMAIN_IDENTIFIER as usize].1 > 0, "burst had no effect");
}
