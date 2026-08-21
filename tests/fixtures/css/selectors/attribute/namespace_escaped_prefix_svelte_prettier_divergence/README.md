# namespace_escaped_prefix_svelte_prettier_divergence

An escape inside an attribute selector's namespace prefix. The prefix is an
`<ident-token>` like any other, so it may carry escapes — and one of them can spell the
very character that separates it from the name:

- an identity escape of the separator — `[a\|b|attr]`, prefix `a\|b`
- its hex spelling — `[a\7c b|attr]`, the same prefix
- an ordinary escape — `[a\41 b|attr]`, prefix `a\41 b`

Per [css-syntax-3 §4.3.7](https://drafts.csswg.org/css-syntax/#consume-escaped-code-point)
an escape's payload is *content*, so the `|` in `a\|b` is part of the prefix's name and
the `<wq-name>`'s separator is the **next** one. tsv locates it by stepping forward from
the prefix token's own span rather than by scanning for the first `|`, which is what makes
the reading escape-proof; the same step also carries it past a separator comment
([separator_comment](../../namespace/separator_comment_svelte_divergence/)).

The prefix is then emitted **verbatim**, like the attribute name beside it
([escaped_identity](../escaped_identity/)) and unlike the AST's `namespace`, which
half-decodes it the way Svelte's `read_identifier` does (hex escapes decode, identity
escapes keep the `\`). Re-emitting a decoded prefix would not merely relocate an escape:
`a\|b` decodes to `a|b`, whose two `|` re-parse as a namespace *and* a `|=` matcher, so
the output is no longer the selector that was written — and, with no value after it, no
longer parses at all.

## Svelte divergence

Svelte's `parseCss` does not support attribute namespaces in any spelling (`Expected token
]`), so `expected_svelte.json` records the parse error — the same shape as
[namespace](../namespace_svelte_divergence/), which it rejects even without an escape.

See [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Prettier divergence

Prettier **loses content** on the identity-escaped spelling: `[a\|b|attr]` comes back as
`[a\|b]` (`output_prettier.svelte`) — the separator and the attribute name are gone, and
the selector now matches an entirely different element. Its selector parser reads the
escaped `|` as the separator, exactly the bug this fixture pins tsv against, and then
drops the remainder. The other two spellings survive, which is what isolates the cause:
only the form whose *raw bytes* contain a `|` before the real separator is corrupted.

The other spellings prettier prints verbatim, agreeing with tsv.

⚠️ Prettier's output for this rule is **not** a form tsv can reach or a claim that can be
widened. `[a\|b]` is a prettier **fixed point**, so the truncation is silent — no later
pass raises anything, and what the file keeps is simply a different selector. The related
`[svg|a\|b]` degrades further, to the unclosed `[svg|a\]`, which prettier's own next pass
rejects (`Unclosed bracket`). Only the pass-1 output is recorded, and only for this input.

See [conformance_prettier_css.md §CSS: Selectors](../../../../../../docs/conformance_prettier_css.md#css-selectors).

## Fixture Structure

- `expected_ours.json` — tsv's AST (source of truth); `namespace` is half-decoded like
  every other selector name
- `expected_svelte.json` — Svelte's rejection
- `output_prettier.svelte` — prettier's output: `|attr` dropped from the first rule

## Related

- [namespace](../namespace_svelte_divergence/) — attribute namespaces without escapes
- [escaped_identity](../escaped_identity/) — the same verbatim-vs-half-decoded rule on the attribute *name*
- [prefix](../../namespace/prefix/) — the bare type-selector spelling, where the same escaped prefix is fixtured against Svelte directly (it accepts those)
- [separator_comment](../../namespace/separator_comment_svelte_divergence/) — the other thing that can sit between a prefix and its separator
