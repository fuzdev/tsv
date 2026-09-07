# escaped_whitespace_prettier_divergence

A CSS value containing an **escaped whitespace** — `\` followed by a space.

Per CSS Syntax 3 [§4.3.4 "Check if two code points are a valid escape"](https://www.w3.org/TR/css-syntax-3/#starts-with-a-valid-escape)
(only a newline after `\` invalidates it) and [§4.3.7 "Consume an escaped code
point"](https://www.w3.org/TR/css-syntax-3/#consume-escaped-code-point) (whose
final branch returns the code point itself), `\ ` is a valid escape whose escaped
code point **is that space**. So the trailing space in `width: 50px\ ;` is value
*content*, not padding — the value is the ident `50px ` and the `;` terminates the
declaration.

tsv preserves it, so `input.svelte` formats to itself.

**Prettier drops the escape's payload**, which strands the backslash onto whatever
follows:

| source | prettier | consequence |
| --- | --- | --- |
| `width: 50px\ ;` | `width: 50px\;` | `\;` escapes the terminator — the declaration never ends |
| `height: var(--a, 50px\ );` | `height: var(--a, 50px\);` | `\)` escapes the closer — the function never closes |
| `margin: calc(1px\ );` | `margin: calc(1px\);` | same |
| `top: calc\ (1px);` | `top: calc\(1px);` | `\(` escapes the **opener** — the value is no longer a function call but the single ident `calc(1px` followed by a `)` that closes nothing (it still parses, as text) |
| `color: a\ b;` | `color: a\b;` | not a delimiter, but the value silently changes from `a b` to `ab` |
| `padding: var(--b, x\,y);` | `padding: var(--b, x\, y);` | the **escaped comma** is read as an argument separator, so one ident is split and rejoined with `", "` — a space is inserted *inside* the value `x,y` |
| `gap: a, b\,;` | `gap: a, b\;` | the same escaped comma at the value's *end*: its payload is dropped and `\;` escapes the terminator |
| `inset: a\+b;` | `inset: a\+ b;` | the **escaped `+`** is read as an operator and gets operator spacing — one ident `a+b` becomes two values |
| `left: (1px\ ) /* c */;` | `left: (1px\) /* c */;` | `\)` escapes the group's closer, as in the `calc` row |
| `right: f(1px\ , 2px) /* c */;` | `right: f(1px\, 2px) /* c */;` | `\,` is no longer a separator — two arguments become one |
| `bottom: (a b c\ )(d);` | `bottom: (a b c\) (d);` | same closer as the `left` row, in a shell the value parser leaves opaque |

The `gap` row is also the boundary of a separate tsv rule: a comma **closing** a
value is authored content tsv keeps ([comma_closing](../lists/comma_closing_prettier_divergence/)),
and an escaped comma closes nothing — so nothing is appended after it.

The first three make prettier's own output **fail to re-parse**: tsv's CSS parser
rejects `output_prettier.svelte` with `Expected '}'`, because with `;` and `)`
escaped the declaration and the block never close. (Svelte's error-recovering
`parseCss` still consumes it, so the AST oracle is unaffected — this is a
*formatter* divergence, not a parser one. tsv's parse AST matches `parseCss` on
`input.svelte` exactly, escaped space and all.)

The `inset` and `padding` rows are the same root cause seen from the other side: an
escape is **opaque**, so nothing inside it — a space, a comma, a paren — is structure.
tsv steps over escapes whole wherever it scans a value.

The last three rows say where the payload has to survive on tsv's side, and it is not
the same question as stepping over the escape while scanning. These values reach the
printer's whitespace normalizer as *text* rather than as a value tree — a value holding a
comment is re-emitted from source, and a paren shell whose first `(` does not close at the
value's end stays one opaque token — and that normalizer strips the spaces before a `)` or
a `,` to place the delimiter. The escape's payload is such a space, so the strips are
bounded by where the last one was emitted. Only the payload is bounded: a separator the
author wrote *after* it still strips (`( 1px\  )` → `(1px\ )`), and a **hex** escape's
trailing whitespace is §4.3.7's optional terminator rather than a payload, so it is dropped
here too, matching prettier (`f(a\41 )` → `f(a\41)`). The comment-free spellings of the
same three values already print correctly from the value tree, so this is one rule reaching
its second emitter, not a second rule.

A *hex* escape is opaque for one byte more than it looks: it takes up to six hex
digits and then, optionally, **one whitespace terminator**, which belongs to the
escape (§4.3.7). So `\41 2px` is the single ident `A2px`, not `\41` followed by
`2px` — tsv keeps it intact (pinned by
[css/values/escaped_hex_terminator](../escaped_hex_terminator/), where prettier
agrees). The long-value wrap case is
[escaped_whitespace_long](../escaped_whitespace_long_prettier_divergence/); the
selector axis is [css/selectors/escaped_names](../../selectors/escaped_names/).

tsv declines to reproduce it: **its format→re-parse invariant outranks matching
prettier.** Emitting output that does not parse is never the defensible side,
whatever the reference formatter does — and the repo's CSS stance is
spec-over-prettier where the two disagree.

`output_prettier.svelte` pins prettier's corrupt output; `input.svelte` is tsv's
stable form.

See [conformance_prettier_css.md §CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values).
