# Multi-code-point character references

93 named character references stand for **two** code points, and Svelte's decoder
emits only the first. Its table (`1-parse/utils/entities.js`) is generated with one
code point per name, so the second — a combining mark, a variation selector, or a
second character outright — is dropped from the text. That is a slip in the
generated data rather than a choice, so tsv decodes both. See
[conformance_svelte.md §Entity Decoding Corrections](../../../../../../docs/conformance_svelte.md#entity-decoding-corrections)
for the catalog entry.

The [named character reference state](https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state)
resolves a reference through the
[named character references table](https://html.spec.whatwg.org/entities.json), whose
every entry gives a `characters` string; for these 93 names that string is two code
points long. Emitting one of them is content loss with no rule behind it: `&NotEqualTilde;`
negates U+2242 with a combining solidus, and without it the text asserts the relation it
was written to deny — the plain U+2242 of `&esim;`.

## The cases

Every kind of second code point the table carries, one row each:

- **A combining mark** — `&NotEqualTilde;` (U+2242 U+0338), `&nLt;` (U+226A U+20D2),
  `&acE;` (U+223E U+0333). The mark overlays or underlines the base character, which is
  what makes the reference mean what it names.
- **A variation selector** — `&caps;` (U+2229 U+FE00), selecting the "with serifs" form of
  the base operator.
- **Two characters** — `&fjlig;` (the `fj` ligature, spelled as the two letters) and
  `&ThickSpace;` (U+205F U+200A, a medium space plus a hair space). Neither second code
  point is a mark, so the loss is a whole missing character.

The attribute value pins the same question on the other decoding path: `&bne;` is
U+003D U+20E5, an `=` struck through by a combining reverse solidus.

## Contrast cases

The second half is the base character of each pair, named by its own single-code-point
reference — `&esim;`, `&ll;`, `&ac;`, `&cap;`, and `&equals;` in an attribute. Both
decoders agree there, so those rows are the same in both expectations, and the diff
between them is exactly the dropped second code point.

Formatting is unaffected throughout — the printer emits the source text of a reference
verbatim, so this is a parse-side divergence only. The two layout questions that *do* read
the decoded text (`is_separator_like_text` / `is_one_line_separator`, which ask what the
characters are rather than how they are spelled) test `is_collapsible_ws`, i.e. `[ \t\n\r]`,
and no second code point in the table is in that class — not even `&ThickSpace;`, whose
U+200A is a space the layout never collapses.
