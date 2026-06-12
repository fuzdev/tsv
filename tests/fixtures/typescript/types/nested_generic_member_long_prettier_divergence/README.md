# nested_generic_member_long_prettier_divergence

A union member that is a nested generic (`Outer<Inner<…>>`) whose type
arguments wrap. Prettier wraps each union member's type doc in `align(2, …)`
(`union-type.js`) to offset the 2-char `| ` prefix; with `--use-tabs` that
lands the wrapped inner row at a whole tab but the closing `>` at `2 tabs + 2
spaces` (a sub-tab alignment).

tsv: closing `>` at a whole tab (`3 tabs`)
Prettier: closing `>` at `2 tabs + 2 spaces`

Both are the same visual width at `tab_width = 2`; only the representation
differs (whole tabs vs tabs-then-spaces).

## Reason

tsv renders all indentation as whole tabs and never mixes tabs with alignment
spaces — Prettier's sub-tab alignment is rounded up to a tab. This keeps
indentation tab-width-agnostic. See
[docs/conformance_prettier.md](../../../../docs/conformance_prettier.md) §Tabs-only
alignment (no sub-tab spaces).
