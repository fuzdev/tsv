# inline_wide_element_content_tail_long_prettier_divergence

The **non-terminal** tail boundary after an inline element whose content is a nested inline
**element** (not text), at the exact 100/101 boundary. Another inline element follows the tail, so
the terminal-hug rule (`inline_wide_content_trailing_long`) does not apply — what this fixture pins
is that the boundary's **space** spelling is nevertheless a **per-width fill decision**, measured
from the closing tag's own column (its newline spelling is layout-keyed — preserved beside the
multiline element, as `output_prettier`'s dual-stability records):

- **100 chars** — the element fits inline and collapses; its line is exactly 100, so the tail
  cannot join it and takes its own line (the control: hug-when-fits still means *fits*).
- **101 chars** — the element lays out block-style (both tags intact), and the tail **hugs the
  intact closing tag**: `</span> text2 <span>text3</span>, text4`.

Both reach one fixed point from every space-spelled authoring: `unformatted_ours_compact`
(everything on one line) converges in **one pass** — the 100 case folds and drops its tail, the
101 case goes block-style and hugs it. (The newline-spelled tail holds its own fixed point —
`output_prettier.svelte`'s dual-stability below.)

The prose twin next door is `inline_wide_content_text_sibling_long` — the same per-width tail
answer for a wide element whose content is **prose**. What separates the two is what forces the
answer: an element-content element's width-broken form emits boundary newlines the next parse
reads as the Tier-2 expansion signal — it regenerates as a *statically* multiline element — so its
width-broken and statically-broken renderings must answer the tail boundary identically, or the
document converges only on pass 2. For the prose twin the per-width answer is a design choice (its
width breaks reflow rather than regenerate). An element inside a **leading wrap** (a spaced comment
or whitespace-only separator before it) once kept a joint element+boundary measurement instead, so
that the two boundaries meeting on it resolved outside-in; that is retired — see
`inline_sibling_drop_tail_flow_long` and `inline_sibling_drop_tail_wide_long`.

## Prettier's forms

Prettier groups the tail boundary WITH the element, so once the element is multiline the tail
always drops — it never holds the hugged form:

| file | authoring | claim |
| --- | --- | --- |
| `output_prettier.svelte` | prettier from `input` | re-breaks the hugged tail to its own line; a form **both** formatters now keep stable — the tail's authored newline after this multiline-rendering, unwrapped element is preserved (the layout-keyed rule), dual-stable beside `input`'s hug |
| `unformatted_ours_compact.svelte` | everything on one line | tsv → `input` in one pass; prettier does not reach `input` from it |
| `divergent_variant_compact.svelte` | prettier from the compact authoring | the 101 case dangles the closing `>` (`</span⏎>`) with the tail on its own line; prettier keeps it, tsv rewrites it to `output_prettier`'s form |

The boundary tsv folds is inter-node whitespace that renders as one space either way, so the
output renders identically to the input.

⚠️ A regression here is **invisible to every idempotency-shaped gate on the hugged side** — the
own-line form is prettier's own stable output, so F1, the fuzzer and the round-trip all pass
through it. What catches a regression is `input.svelte` failing to be a tsv fixed point, and the
`authoring:audit` mutants of the compact authoring, which caught the pass-2-only convergence this
fixture's one-pass claim now pins.

## Reason

Design choice: the tail boundary after a multiline inline element is render-free inter-node
whitespace, so tsv converges its **space** spelling onto the per-width answer — hug when it
fits, break when it does not — while an authored newline is layout-keyed (preserved beside the
multiline element); prettier's element-grouped boundary always breaks and so holds a distinct
stable form per authoring. For this fixture's element-content shape the per-width answer
is additionally forced by idempotence — see the regeneration argument above.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
