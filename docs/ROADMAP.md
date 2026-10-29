# Roadmap

Planned work that is not built yet.

## Lexicon interface

Replace the compiled word list with a `Lexicon` trait so a lookup stage can be swapped at run time. An optional adapter would back it with morph-fst for languages where listing every word form is impractical. Open points:

- Lookup cost on the hot path against the current cuckoo hash.
- A path dependency on morph-fst blocks `cargo publish`; the alternative is a build-time word-list generator for a `Language`.

## Second language

Add a non-English `Language`. The engine already reads only table-carried words (proved by the respelled-word test), so the remaining work is in the builder: number-grammar programs for forms English lacks, such as Spanish "cuatrocientos", plus a language's examples, idioms and vocab.
