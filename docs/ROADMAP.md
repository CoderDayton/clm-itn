# Roadmap

Planned work that is not built yet.

## morph-fst

German splits compound numbers ("zweihundertfünfundvierzig") with a morph-fst grammar, `src/engine/window/rules/german.morph`, behind the `german` feature. Open points:

- The path dependency on morph-fst blocks `cargo publish` until morph-fst is published and the dependency names a version.
- Each compound costs 1–2 µs in morph-fst's `lex`: lattice building, allocation and the subword pass dominate. An allocation-free segmentation call in morph-fst would cut it.
- Spanish and Portuguese still list their inflected forms in the builder; moving them onto morph-fst grammars is next.
