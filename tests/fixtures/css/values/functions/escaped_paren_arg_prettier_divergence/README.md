# escaped_paren_arg_prettier_divergence

A function whose argument list contains an **escaped** `(` (`fn(a\(b, 0.1)`).

Per CSS Syntax 3 a `\` followed by anything but a newline "is a valid escape"
(§"Check if two code points are a valid escape") whose escaped code point is the character
itself (§"Consume an escaped code point"), and §"Consume an ident sequence" consumes escapes
as ident content. So `a\(b` is the single ident `a(b`, not a nesting paren, and the
function's own parens balance at the final *unescaped* `)`. tsv steps every escape whole
while it locates that closing paren, so the value parses as a function and its arguments
normalize like any other — `fn(a\(b,.10)` → `fn(a\(b, 0.1)`.

Prettier's CSS parser (postcss) miscounts the escaped `(` — the same bug that makes it
*throw* on [escaped_close_paren_arg](../escaped_close_paren_arg_prettier_divergence/)
(`fn(a\)b, 0.1)` → `Unbalanced parenthesis`) and on
[url_escaped_paren](../url_escaped_paren_prettier_divergence/). Here it doesn't throw;
it **stops normalizing** and emits the declaration verbatim, so `fn(a\(b,.10)` keeps
its `.10`. The identical value with an unescaped ident in place of `a\(b` normalizes on
both sides.

No `output_prettier.*`: on tsv's canonical form (`input`) prettier agrees, so the
divergence lives only in `prettier_variant_compact` — a form prettier keeps stable that
tsv normalizes to `input`.

See [conformance_prettier_css.md §CSS: Values](../../../../../../docs/conformance_prettier_css.md#css-values).
