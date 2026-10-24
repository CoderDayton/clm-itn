import argparse
import json
import multiprocessing as mp
import os
import re
import sys
import time
import urllib.request
from collections import Counter
from pathlib import Path

USER_AGENT = "clm-itn-pairgen/0.1 (research corpus build)"

GUTENBERG = [
    (103, "Around the World in Eighty Days"),
    (1661, "The Adventures of Sherlock Holmes"),
    (2852, "The Hound of the Baskervilles"),
    (4300, "Ulysses"),
    (245, "Life on the Mississippi"),
    (1228, "On the Origin of Species"),
    (3300, "The Wealth of Nations"),
    (4200, "The Diary of Samuel Pepys, complete"),
    (731, "History of the Decline and Fall of the Roman Empire, Vol. 1"),
    (3207, "Leviathan"),
    (2701, "Moby Dick"),
    (1342, "Pride and Prejudice"),
    (36, "The War of the Worlds"),
    (164, "Twenty Thousand Leagues under the Sea"),
    (18857, "A Journey to the Centre of the Earth"),
    (2610, "Notre-Dame de Paris"),
    (1400, "Great Expectations"),
    (996, "Don Quixote"),
    (5200, "Metamorphosis"),
    (20203, "Autobiography of Benjamin Franklin"),
    (4367, "Personal Memoirs of U. S. Grant, Complete"),
    (16643, "Essays by Ralph Waldo Emerson"),
    (205, "Walden"),
    (1232, "The Prince"),
    (2009, "The Origin of Species, 6th ed."),
]

PG_START = re.compile(r"^\*\*\* ?START OF (THE|THIS) PROJECT GUTENBERG EBOOK.*$", re.M | re.I)
PG_END = re.compile(r"^\*\*\* ?END OF (THE|THIS) PROJECT GUTENBERG EBOOK.*$", re.M | re.I)

CLASS_NAMES = {
    "cardinal": "CARDINAL",
    "ordinal": "ORDINAL",
    "decimal": "DECIMAL",
    "fraction": "FRACTION",
    "money": "MONEY",
    "measure": "MEASURE",
    "date": "DATE",
    "time": "TIME",
    "telephone": "TELEPHONE",
    "digit": "DIGIT",
    "electronic": "ELECTRONIC",
    "address": "ADDRESS",
}

MAX_SPOKEN_TOKENS = 400
PUNCT_ONLY = re.compile(r"^[\W_]+$", re.U)


def get(url, accept=None):
    headers = {"User-Agent": USER_AGENT}
    if accept:
        headers["Accept"] = accept
    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def fnv1a64(data: bytes) -> int:
    h = 0xCBF29CE484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def split_of(spoken: str) -> str:
    bucket = fnv1a64(spoken.encode("utf-8")) % 100
    if bucket < 90:
        return "train"
    if bucket < 95:
        return "val"
    return "test"


def fetch_gutenberg(out: Path, manifest: list, delay: float):
    for book, title in GUTENBERG:
        dst = out / f"pg{book}.txt"
        url = f"https://www.gutenberg.org/cache/epub/{book}/pg{book}.txt"
        if not dst.exists():
            raw = get(url)
            dst.write_bytes(raw)
            time.sleep(delay)
        text = dst.read_text("utf-8", errors="replace")
        head = text[: text.find("*** START")] if "*** START" in text else text[:3000]
        if re.search(r"copyrighted project gutenberg", head, re.I):
            dst.unlink()
            print(f"skip pg{book}: copyrighted", file=sys.stderr)
            continue
        manifest.append(
            {
                "id": f"pg{book}",
                "title": title,
                "url": url,
                "kind": "gutenberg",
                "basis": "Project Gutenberg eBook marked public domain in the USA; "
                "PG header, footer and every reference to Project Gutenberg are stripped "
                "before use, per https://www.gutenberg.org/policy/license.html",
                "file": dst.name,
            }
        )


def fetch_federal_register(out: Path, manifest: list, docs: int, delay: float):
    api = (
        "https://www.federalregister.gov/api/v1/documents.json?per_page=100"
        "&order=newest&fields[]=document_number&fields[]=title&fields[]=raw_text_url"
        "&fields[]=html_url&fields[]=type&fields[]=agencies"
    )
    page, got = 1, 0
    while got < docs:
        listing = json.loads(get(f"{api}&page={page}"))
        results = listing.get("results", [])
        if not results:
            break
        for d in results:
            if got >= docs:
                break
            if not d.get("raw_text_url"):
                continue
            dst = out / f"fr-{d['document_number']}.txt"
            if not dst.exists():
                dst.write_bytes(get(d["raw_text_url"]))
                time.sleep(delay)
            manifest.append(
                {
                    "id": f"fr-{d['document_number']}",
                    "title": d.get("title", ""),
                    "url": d.get("html_url") or d["raw_text_url"],
                    "kind": "federal-register",
                    "basis": "Federal Register document issued by a US federal agency: a work of "
                    "the US Government, not subject to copyright (17 U.S.C. 105)",
                    "agency": ", ".join(a.get("name", "") for a in d.get("agencies") or []),
                    "file": dst.name,
                }
            )
            got += 1
        page += 1


def fetch_nws(out: Path, manifest: list, products: int, delay: float):
    listing = json.loads(get("https://api.weather.gov/products/types/AFD", "application/ld+json"))
    for p in listing.get("@graph", [])[:products]:
        pid = p["id"]
        dst = out / f"nws-{pid}.txt"
        url = f"https://api.weather.gov/products/{pid}"
        if not dst.exists():
            doc = json.loads(get(url, "application/ld+json"))
            dst.write_text(doc.get("productText", ""), "utf-8")
            time.sleep(delay)
        manifest.append(
            {
                "id": f"nws-{pid}",
                "title": f"NWS Area Forecast Discussion {p.get('issuingOffice', '')} {p.get('issuanceTime', '')}",
                "url": url,
                "kind": "nws",
                "basis": "National Weather Service product: a work of the US Government "
                "(17 U.S.C. 105); weather.gov states NWS information is public domain",
                "file": dst.name,
            }
        )


def cmd_fetch(a):
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    manifest = []
    fetch_gutenberg(out, manifest, a.delay)
    fetch_federal_register(out, manifest, a.fr_docs, a.delay)
    fetch_nws(out, manifest, a.nws_products, a.delay)
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1), "utf-8")
    print(f"fetched {len(manifest)} documents into {out}")


def strip_gutenberg(text: str) -> str:
    s, e = PG_START.search(text), PG_END.search(text)
    body = text[s.end() : e.start()] if s and e else text
    return "\n".join(l for l in body.splitlines() if "gutenberg" not in l.lower())


def clean_nws(text: str) -> str:
    lines = []
    for l in text.splitlines():
        t = l.strip()
        if not t or t in ("&&", "$$") or t.startswith((".", "000", "FXUS", "AFD")):
            lines.append("")
            continue
        lines.append(t)
    return "\n".join(lines)


FOOTNOTE_REF = re.compile(r"\\\d+\\")
HTML_TAG = re.compile(r"<[^>\n]{1,300}>")
ITALIC_MARK = re.compile(r"(?<![\w])_+|_+(?![\w])")
STAR_RUN = re.compile(r"(\s*\*\s*){2,}|^\s*\*\s*$", re.M)
CONTROL = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")
RULE_LINE = re.compile(r"^\s*[=\-_~]{5,}\s*$", re.M)


def clean_markup(text: str) -> str:
    text = CONTROL.sub("\n", text)
    text = RULE_LINE.sub("\n", text)
    text = FOOTNOTE_REF.sub("", text)
    text = HTML_TAG.sub(" ", text)
    text = text.replace("``", '"').replace("''", '"')
    text = ITALIC_MARK.sub("", text)
    return STAR_RUN.sub("\n", text)


def paragraphs(text: str):
    for p in re.split(r"\n\s*\n", text):
        p = " ".join(p.split())
        if p:
            yield p


def cmd_extract(a):
    src = Path(a.src)
    manifest = json.loads((src / "manifest.json").read_text("utf-8"))
    from nemo_text_processing.text_normalization.normalize import Normalizer

    splitter = Normalizer(input_case="cased", lang="en", cache_dir=a.cache)
    per_source = Counter()
    with open(a.out, "w", encoding="utf-8") as o:
        for m in manifest:
            text = (src / m["file"]).read_text("utf-8", errors="replace")
            if m["kind"] == "gutenberg":
                text = strip_gutenberg(text)
            elif m["kind"] == "nws":
                text = clean_nws(text)
            for p in paragraphs(clean_markup(text)):
                for s in splitter.split_text_into_sentences(p):
                    s = " ".join(s.split())
                    if 3 <= len(s.split()) <= 120:
                        o.write(f"{m['id']}\t{s}\n")
                        per_source[m["kind"]] += 1
    print(json.dumps(per_source))


NORMALIZER = None


def init_worker(cache):
    global NORMALIZER
    import logging

    logging.disable(logging.WARNING)
    from nemo_text_processing.text_normalization.normalize import Normalizer

    NORMALIZER = Normalizer(input_case="cased", lang="en", cache_dir=cache)


def path_alignment(lattice):
    import pynini

    path = pynini.shortestpath(lattice, nshortest=1, unique=True)
    path.topsort()
    out_bytes, in_at, consumed = bytearray(), [], 0
    state = path.start()
    while True:
        arcs = list(path.arcs(state))
        if not arcs:
            break
        arc = arcs[0]
        if arc.olabel:
            out_bytes.append(arc.olabel)
            in_at.append(consumed)
        if arc.ilabel:
            consumed += 1
        state = arc.nextstate
    in_at.append(consumed)
    return bytes(out_bytes), in_at


TOKEN_START = re.compile(rb"(?:^|(?<=\}) +)(tokens \{)")


def token_starts(tagged: bytes):
    return [m.start(1) for m in TOKEN_START.finditer(tagged)]


def verbalize(n, tok):
    import pynini

    lattice = None
    for tagged in n.generate_permutations([tok]):
        lattice = n.find_verbalizer(pynini.escape(tagged))
        if lattice.num_states() != 0:
            break
    if lattice is None or lattice.num_states() == 0:
        return None
    return n.select_verbalizer(lattice)


def token_class(tok):
    inner = tok["tokens"]
    measure = inner.get("measure")
    if isinstance(measure, dict) and measure.get("units") == "address":
        return "ADDRESS"
    for k in inner:
        if k in CLASS_NAMES:
            return CLASS_NAMES[k]
    return None


def squash(s: str) -> str:
    return "".join(s.split())


def align(sentence: str):
    import pynini

    n = NORMALIZER
    raw = sentence.encode("utf-8")
    lattice = n.find_tags(pynini.escape(sentence))
    tagged_str = n.select_tag(lattice)
    tagged, in_at = path_alignment(lattice)
    if tagged.decode("utf-8", errors="replace") != tagged_str:
        return None, "path-mismatch"
    starts = token_starts(tagged)
    n.parser(tagged_str)
    toks = n.parser.parse()
    if len(starts) != len(toks):
        return None, "token-count"
    bounds = [0] + [in_at[s] for s in starts[1:]] + [len(raw)]
    written = [raw[bounds[i] : bounds[i + 1]].decode("utf-8", errors="replace").strip() for i in range(len(toks))]
    if squash(" ".join(written)) != squash(sentence):
        return None, "written-cover"
    spoken_parts = []
    for tok in toks:
        v = verbalize(n, tok)
        if v is None:
            return None, "verbalize"
        spoken_parts.append(" ".join(v.replace("\xa0", " ").split()))
    full = n.normalize(sentence)
    words = full.lower().split()
    if not words or len(words) > MAX_SPOKEN_TOKENS:
        return None, "length"
    flat = "".join(words)
    if flat != squash("".join(spoken_parts)).lower():
        return None, "spoken-cover"
    owner, pos = [], 0
    for i, part in enumerate(spoken_parts):
        owner.extend([i] * len(squash(part)))
    word_tokens, pos = [], 0
    for w in words:
        word_tokens.append(set(owner[pos : pos + len(w)]))
        pos += len(w)
    spans = []
    for i, tok in enumerate(toks):
        cls = token_class(tok)
        if cls is None:
            if "name" in tok["tokens"] and squash(spoken_parts[i]).lower() != squash(written[i]).lower():
                cls = "SERIAL" if re.search(r"\d", written[i]) else "WHITELIST"
            else:
                continue
        idx = [j for j, ts in enumerate(word_tokens) if i in ts]
        if not idx:
            return None, "empty-span"
        for j in idx:
            for other in word_tokens[j] - {i}:
                oname = toks[other]["tokens"].get("name")
                if oname is None or not PUNCT_ONLY.match(oname):
                    return None, "shared-word"
        spans.append((idx[0], idx[-1] - idx[0] + 1, cls, written[i]))
    for (s0, l0, _, _), (s1, _, _, _) in zip(spans, spans[1:]):
        if s0 + l0 > s1:
            return None, "overlap"
    return (" ".join(words), spans), None


def source_kind(doc_id: str) -> str:
    if doc_id.startswith("fr-"):
        return "federal-register"
    if doc_id.startswith("nws-"):
        return "nws"
    return "gutenberg"


def work(line):
    src, _, sentence = line.rstrip("\n").partition("\t")
    try:
        rec, why = align(sentence)
    except Exception as e:
        return src, None, f"error:{type(e).__name__}"
    return src, rec, why


def restore_outputs(out: Path, names, sizes, resume: bool) -> set:
    for n in names:
        p = out / n
        if resume and p.exists():
            with open(p, "r+b") as f:
                f.truncate(sizes[n])
        else:
            p.write_bytes(b"")
    seen = set()
    for n in names:
        if n.endswith(".txt"):
            with open(out / n, encoding="utf-8") as f:
                seen.update(l[2:].rstrip("\n") for l in f if l.startswith("S "))
    return seen


def cmd_generate(a):
    lines = Path(a.sentences).read_text("utf-8").splitlines()
    if a.limit:
        lines = lines[: a.limit]
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    splits = ("train", "val", "test")
    names = [f"{s}.{ext}" for s in splits for ext in ("txt", "src")]
    ckpt_path = out / "checkpoint.json"
    state = {
        "done": 0,
        "seconds": 0.0,
        "drops": {},
        "sentences": {},
        "by_source": {s: {} for s in splits},
        "spans": {s: {} for s in splits},
        "sizes": {n: 0 for n in names},
    }
    if a.resume and ckpt_path.exists():
        state = json.loads(ckpt_path.read_text("utf-8"))
    seen = restore_outputs(out, names, state["sizes"], a.resume)
    drops = Counter(state["drops"])
    sentences = Counter(state["sentences"])
    by_source = {s: Counter(state["by_source"][s]) for s in splits}
    stats = {s: Counter(state["spans"][s]) for s in splits}
    files = {n: open(out / n, "a", encoding="utf-8", newline="\n") for n in names}
    start = state["done"]
    run_start = time.time()
    t0 = run_start - state["seconds"]

    def checkpoint(done):
        for f in files.values():
            f.flush()
            os.fsync(f.fileno())
        state.update(
            done=done,
            seconds=round(time.time() - t0, 1),
            drops=dict(drops),
            sentences=dict(sentences),
            by_source={s: dict(by_source[s]) for s in splits},
            spans={s: dict(stats[s]) for s in splits},
            sizes={n: (out / n).stat().st_size for n in names},
        )
        tmp = out / "checkpoint.json.tmp"
        tmp.write_text(json.dumps(state), "utf-8")
        tmp.replace(ckpt_path)

    if start:
        print(f"resuming at {start}/{len(lines)}", file=sys.stderr, flush=True)
    with mp.Pool(a.jobs, initializer=init_worker, initargs=(a.cache,)) as pool:
        results = pool.imap(work, lines[start:], chunksize=16)
        for k, (src, rec, why) in enumerate(results, start=start + 1):
            if rec is None:
                drops[why] += 1
            elif rec[0] in seen:
                drops["duplicate"] += 1
            else:
                spoken, spans = rec
                seen.add(spoken)
                split = split_of(spoken)
                files[f"{split}.src"].write(f"{source_kind(src)} {src}\n")
                o = files[f"{split}.txt"]
                o.write(f"S {spoken}\n")
                for s, l, cls, w in spans:
                    o.write(f"T {s} {l} {cls} {w}\n")
                    stats[split][cls] += 1
                o.write("\n")
                sentences[split] += 1
                by_source[split][source_kind(src)] += 1
            if k % a.checkpoint_every == 0:
                checkpoint(k)
                rate = (k - start) / max(1e-9, time.time() - run_start)
                left = (len(lines) - k) / max(rate, 1e-9)
                print(f"{k}/{len(lines)} {time.time() - t0:.0f}s eta {left / 60:.1f} min", file=sys.stderr, flush=True)
    checkpoint(len(lines))
    for f in files.values():
        f.close()
    report = {
        "input_sentences": len(lines),
        "kept": sum(sentences.values()),
        "drops": dict(drops),
        "drop_rate_excluding_duplicates": round(
            (sum(drops.values()) - drops["duplicate"]) / max(1, len(lines)), 4
        ),
        "sentences": dict(sentences),
        "sentences_by_source": {k: dict(v) for k, v in by_source.items()},
        "spans": {k: dict(v.most_common()) for k, v in stats.items()},
        "seconds": state["seconds"],
        "jobs": a.jobs,
    }
    (out / "report.json").write_text(json.dumps(report, indent=1), "utf-8")
    print(json.dumps(report, indent=1))


def main():
    p = argparse.ArgumentParser(description="Generate spoken/written ITN pairs with NeMo TN.")
    sub = p.add_subparsers(dest="cmd", required=True)
    f = sub.add_parser("fetch")
    f.add_argument("--out", required=True)
    f.add_argument("--fr-docs", type=int, default=150)
    f.add_argument("--nws-products", type=int, default=300)
    f.add_argument("--delay", type=float, default=1.0)
    e = sub.add_parser("extract")
    e.add_argument("--src", required=True)
    e.add_argument("--out", required=True)
    e.add_argument("--cache", required=True)
    g = sub.add_parser("generate")
    g.add_argument("--sentences", required=True)
    g.add_argument("--out", required=True)
    g.add_argument("--cache", required=True)
    g.add_argument("--jobs", type=int, default=8)
    g.add_argument("--limit", type=int, default=0)
    g.add_argument("--checkpoint-every", type=int, default=1000)
    g.add_argument("--resume", action="store_true")
    a = p.parse_args()
    {"fetch": cmd_fetch, "extract": cmd_extract, "generate": cmd_generate}[a.cmd](a)


if __name__ == "__main__":
    main()
