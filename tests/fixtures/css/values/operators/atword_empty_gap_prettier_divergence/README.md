# atword_empty_gap_prettier_divergence

A lone `@` with an authored gap after it — `@ a`, `@ (2.5)`. Prettier welds it onto the
member that follows (`@a`, `@(2.5)`); tsv keeps the gap.

`◆prettier_bug`. See
[conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values)
— entry "Empty `@`-word gap".

## Why prettier welds

postcss-values-parser gives `@` its own `atword` token, ended by whitespace, a quote, a
comma, a paren, a `{`, a `\`, a `;` or a `/` — so a lone `@` is an atword whose value is
empty. Prettier's value printer then hits its "Ignore `@` in Less (i.e. `@@var;`)" arm,
which glues an empty atword to whatever follows it, and the arm runs for the `css`
parser too.

## Why tsv keeps the gap

The weld changes the token stream. css-syntax-3 tokenizes `@ a` as a `<delim-token>` `@`,
whitespace and an `<ident-token>`, and `@a` as one `<at-keyword-token>`; in a custom
property's value the token sequence *is* the value. A formatter that turns the first into
the second has rewritten the author's tokens, not their spacing — the arm exists for a
Less spelling that cannot occur here.

## Cells

| authoring | tsv | prettier |
| --- | --- | --- |
| `@ a` | `@ a` | `@a` |
| `@ (2.5)` | `@ (2.5)` | `@(2.5)` |

The last two declarations are the controls, where the two agree: an authored glue is kept
on both sides (`@(2.5)`, `a @(2.5)`). The rest of the `@`-word rule — the ordinary member
gap on either side of a non-empty `@`-word, and the word's own extent — is in
[atword](../atword/).
