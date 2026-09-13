# signed_number_after_word_prettier_divergence

A word followed by a **signed number** the author wrote as its own whitespace-separated
element — `aa +1.5`, `aa +0.5`, inside a function and a group alike.

tsv: `aa +1.5` — the element lexes as one `<number-token>` and tsv does not split a number,
so the author's single gap is the one separator there is
Prettier: `aa + 1.5` — reads the `+` as an operator node and, because its left neighbour is
a word, requires a space on **both** sides of it

`◆prettier_bug` `◆spec_precedence`. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Signed number after a word".

## Reason

css-syntax-3 §4.3.1 "Consume a token": a `+` is a `<delim-token>` only when the three code
points starting with it would *not* start a number. `+1.5` starts a number, so
`aa +1.5` is `<ident aa>` `<whitespace>` `<number +1.5>` — two component values with one
separator between them, and no operator for a formatter to space. Prettier's value layer is
postcss-values-parser, whose tokenizer reads the `+` after a word as an `operator` node
instead; `requireSpaceBeforeOperator` / `requireSpaceAfterOperator` then fire on the word
neighbour and pad it. Writing that pad in **splits one token into two**, and in a custom
property's value — a verbatim token sequence `var()` substitutes (css-variables-1
§"Custom Property Value Syntax") — `aa +1.5` and `aa + 1.5` are not the same value.

⚠️ **The claim is about the element, not about the `+`.** One authoring over, where the
author glued the sign to the word (`aa+1.5`), tsv splits the run exactly as prettier does and
the two agree on `aa + 1.5` — the cell `k` here. That is not an inconsistency: a *run* of
glued tokens is split at postcss's word boundaries by design (a `+` ends a word wherever it
occurs — see the `tsv_css` crate doc's value-run bullet), because a run is the construct
prettier's value parser reads and tsv transcribes its reading. What tsv declines to do is
take a whitespace-separated element that already lexes as one `<number-token>` and cut it in
half.

Prettier's own rule is positional in the same way, which is what cell `j` pins: in
`aa +1.5 +2.5` only the **first** `+` has a word on its left, so prettier pads that one and
leaves the second signed number alone (`aa + 1.5 +2.5`). Cell `l` is the pair of those facts
in one value: `aa+1.5+2.5` splits at the word boundary on both sides and keeps the second
pair glued on both (`aa + 1.5+2.5`).

The bounding cells where the two agree are all in the fixture: a `-` in the same position
(`aa -1.5`, which starts no number after a word and is ident content), a **number** on the
left (`1.5 +2.5`, where postcss reads the sign as the number's own), a `+` followed by a
non-digit (`aa + bb`), where the operator is real, and the glued authorings above.

## Related

- [operator_head_weld](../operator_head_weld_prettier_divergence/) — the mirror image: there
  prettier **merges** two tokens by gluing a head operator onto the member after it
