# Comma siblings whose printed form would spell a type-argument list — Svelte Divergence

**A known over-rejection, not a sanctioned reading.** Every line of `input.svelte` is two
comparisons in comma siblings to **tsc and to acorn-typescript alike** —
`expected_svelte.json` is the canonical tree — and tsv rejects it.

A `<` in one sibling and a `>` ahead of a `(` in a later one are a type-argument list
only where the tokens between them spell one. The lookahead grades the list's first
argument and matches delimiters past its first `,`, so the parser tries the list from a
checkpoint and, where the type grammar stops past that separator, rewinds and reads the
`<` as the operator
([relational_sibling_value_tail](../relational_sibling_value_tail/)). That fallback is
**refused where the printer's own rewrite of the abandoned tokens could complete the
list**, which is read off where the type grammar stopped:

- **at a `=>`** — an arrow function's bare parameter, which prints in a paren pair, and
  `(c) => c` is a function type: `fn(a < b, c => c > (d - e) * 2)` would print as
  `fn(a < b, (c) => c > (d - e) * 2)`, the generic call `a<b, (c) => c>(d - e)` times
  two. The arrow may be a sibling between the two comparisons or an element of one;
- **at a `.` or a `<` right behind a `)`** — a paren shell the printer strips, where the
  name left bare takes the member or the list behind it: `(c).d` prints as `c.d`, a
  qualified name, and `(c) < d >> (e, f)` as `c < d >> (e, f)`, a nested list closed with
  the outer one. A pair ahead of any other token is no refusal
  (`fn(a < b, (c & d) !== 0 && e > (f, g))` is two comparisons);
- **at a `[` or a `<` past a line break** — the break ends a type, and the printer folds
  it: `c⏎[d]` prints as `c[d]`, an indexed access type, and `c⏎<d>` as `c<d>`, a
  reference with its own argument list.

Each rewritten line is one every parser reads as another program, so the rejection is
loud where the alternative is silent. It is also wider than the hazard: it asks where
the list stopped, and never whether the rewritten tokens would in fact reach the `>` or
whether the printer strips the pair at all, so `fn(a < b, c => c + 1 > (d, e))` and
`fn(a < b, (c | d).e > (f, g))` are rejected with the rest.

**Authoring around it.** Nothing between the two tokens has to change: write the first
comparison from the other side (`fn(b > a, c => c > (d - e) * 2)`), or hoist either
comparison into a variable (`const lt = a < b; fn(lt, c => c > (d - e) * 2)`). An arrow
function may also be a `function` expression, which no type spells.

`tsv_rejects.txt` pins tsv's own error on the first line, and every line is pinned on its
own by `a_tail_the_printer_would_complete_stays_rejected` in
[tests/type_arguments_tail_fallback.rs](../../../../../type_arguments_tail_fallback.rs).

Because the canonical parser accepts the input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject), and with no accepted
parse there is nothing for a formatter to claim — hence no `expected.json` and no
format-claim siblings.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections (a type-argument list tried past its first separator).
