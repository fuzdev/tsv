# union_fn_type_member_long_prettier_divergence

A union member that is a parenthesized function type (`( (params) => ret )`)
whose parameter list breaks multi-line. Prettier wraps each union member's type
doc in `align(2, …)` (`union-type.js`); with `--use-tabs` the broken params land
at whole tabs but the closing `) => void)` at `2 tabs + 2 spaces` (a sub-tab
alignment).

tsv: closing `) => void)` at a whole tab
Prettier: closing `) => void)` at `2 tabs + 2 spaces`

Both are the same visual width at `tabWidth = 2`; only the representation
differs (whole tabs vs tabs-then-spaces).

`Short` (inline) and `Fit` (params at exactly 100, stays inline) are the
non-diverging siblings — the divergence only appears once the params break
(`Brk`, at 101). This covers the parenthesized-default member path
(`build_type_doc_maybe_parens`), distinct from the object / generic member
shapes in the sibling fixtures; constructor types (`new (…) => T`) take the
same path.

## Reason

tsv renders all indentation as whole tabs and never mixes tabs with alignment
spaces — Prettier's sub-tab alignment is rounded up to a tab. This keeps
indentation tab-width-agnostic. See
[docs/conformance_prettier.md](../../../../docs/conformance_prettier.md) §Tabs-only
alignment (no sub-tab spaces).
