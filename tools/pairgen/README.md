# pairgen

Builds spoken/written ITN training pairs by running NVIDIA NeMo-text-processing English text normalization (written → spoken) over public-domain text. Output feeds `src/bin/clm-learn/`.

Nothing here is a crate dependency. The Python environment, downloads, grammar cache and generated pairs all live outside the repo. Do not commit any of them.

## Licenses

- NeMo-text-processing and pynini: Apache-2.0. Program output carries no license from them.
- Inputs are public domain only:
  - Project Gutenberg eBooks that are public domain in the USA. `extract` strips the Project Gutenberg header, footer and every line that mentions Project Gutenberg, as https://www.gutenberg.org/policy/license.html requires for unrestricted use. Books whose header says "Copyrighted Project Gutenberg eBook" are skipped.
  - Federal Register documents (federalregister.gov API): works of the US Government, 17 U.S.C. §105. GovInfo notice: https://www.govinfo.gov/about/policies#copyright. Government documents can quote copyrighted material; such passages are rare in rule text but not filtered.
  - National Weather Service Area Forecast Discussions (api.weather.gov): https://www.weather.gov/disclaimer states NWS information is public domain unless noted otherwise.
- `fetch` writes `manifest.json` with each document's URL and its public-domain basis.

## Setup

Requires `uv` and CPython 3.11 (pynini 2.1.6.post1 has wheels for 3.8–3.13 only).

```sh
W=/path/outside/the/repo
uv venv --python 3.11 $W/venv
uv pip install --python $W/venv/bin/python -r tools/pairgen/requirements.txt
```

`requirements.txt` pins every package; the two that matter are `nemo-text-processing==1.2.0` and `pynini==2.1.6.post1`.

## Run

```sh
PY=$W/venv/bin/python
$PY tools/pairgen/pairgen.py fetch    --out $W/src --fr-docs 150 --nws-products 300 --delay 0.5
$PY tools/pairgen/pairgen.py extract  --src $W/src --out $W/sentences.tsv --cache $W/nemo-cache
$PY tools/pairgen/pairgen.py generate --sentences $W/sentences.tsv --out $W/pairs --cache $W/nemo-cache --jobs 8
```

- `fetch` downloads with a delay between requests and skips files already present. Downloads are parsed as text only.
- `extract` writes one `<source id>\t<sentence>` line per sentence (3–120 words), split with NeMo's sentence splitter.
- `generate` writes `train.txt`, `val.txt`, `test.txt`, a matching `train.src`, `val.src`, `test.src`, and `report.json` (counts per split, class and source; drop reasons; run time). The first run compiles NeMo's grammars into `--cache`.
- `generate` uses `--jobs 8` worker processes by default; it is CPU-bound, so wall time scales with the job count. It prints `done/total`, elapsed time and an ETA every `--checkpoint-every` sentences (default 1000).
- Records are written as they are produced. At each checkpoint the files are flushed and `checkpoint.json` stores the sentences done, the counters and each file's size. After a crash or kill, rerun the same command with `--resume`: the files are cut back to the last checkpoint and generation continues from there, giving the same output as an uninterrupted run. Without `--resume`, the output files are overwritten.
- Each `.src` file has one line per record, in record order: `<kind> <document id>`, where kind is `gutenberg`, `federal-register` or `nws` and the id is a key in `manifest.json`.

## Output format

UTF-8, LF, no BOM, no header. One record per sentence, ended by exactly one empty line:

```
S <spoken tokens joined by single spaces>
T <start> <len> <CLASS> <written form>
```

- `S` comes first, once. Tokens are NeMo's full normalization output, lowercased and split on whitespace, so punctuation stays attached as in the CLI tokenizer (`str::split_whitespace`). At most 400 tokens.
- `T` lines follow in increasing `start` order and never overlap. `start` and `len` are 0-based indices into the `S` tokens, `len ≥ 1`. `<written form>` is everything after the fourth space: the original text of the span, unescaped, possibly with spaces.
- `CLASS` is NeMo's semiotic class in upper case: CARDINAL ORDINAL DECIMAL FRACTION MONEY MEASURE DATE TIME TELEPHONE ELECTRONIC ADDRESS (NeMo's `measure` with units `address`). Two more mark plain-word tokens that NeMo rewrote: SERIAL (written form has a digit, e.g. `12B`) and WHITELIST (e.g. `Dr.` → `doctor`).
- Sentences with no `T` line are kept: they are the plain-word negatives.
- Exact duplicate `S` lines are dropped before splitting.
- Split: FNV-1a 64 over the UTF-8 bytes of the spoken tokens (the `S` line without its `S ` prefix); `h % 100 < 90` → train, `< 95` → val, else test.

## Alignment

Each token's written text comes from the input labels on the tagger's best path, cut at the arc where the token's tagged output starts. Each token is then verbalized on its own; a span covers the `S` words whose characters come from that token. A word may share a token with an attached punctuation-only token (e.g. `nine,`).

A sentence is dropped, and its reason counted in `report.json`, when:

- the per-token written texts do not cover the input;
- the per-token spoken texts do not equal NeMo's full `normalize()` output, ignoring whitespace;
- a word mixes two non-punctuation tokens, or spans overlap;
- verbalization fails or NeMo raises.
