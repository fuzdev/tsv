# union_parens_object_prettier_divergence

A union member that is a parenthesized object type (`( { … } )`) which breaks
multi-line. Prettier wraps each union member's type doc in `align(2, …)`
(`union-type.js`); with `--use-tabs` the object members land at whole tabs but
the closing `}` at `2 tabs + 2 spaces` (a sub-tab alignment).

tsv: closing `}` at a whole tab
Prettier: closing `}` at `2 tabs + 2 spaces`

Both are the same visual width at `tab_width = 2`; only the representation
differs (whole tabs vs tabs-then-spaces).

## Reason

tsv renders all indentation as whole tabs and never mixes tabs with alignment
spaces — Prettier's sub-tab alignment is rounded up to a tab. This keeps
indentation tab-width-agnostic. See
[docs/conformance_prettier.md](../../../../docs/conformance_prettier.md) §Tabs-only
alignment (no sub-tab spaces).
