# binding_annotation_comment_prettier_divergence

A comment inside an `{#await … then BINDING: T}` / `{:then BINDING: T}` /
`{:catch BINDING: T}` binding's **type annotation** is preserved where the author
wrote it. prettier-plugin-svelte silently drops it.

tsv: `{#await promise then a: /* c */ A}` (preserved)
Prettier: `{#await promise then a: A}` (comment dropped)

The await face of
[each/context_annotation_comment](../../each/context_annotation_comment_prettier_divergence/),
and the reason it needs its own fixture: the two heads reach the annotation by
different routes. `{#each}` bounds the pattern before the annotation and parses
the annotation itself, so the pattern node's span stops at the bare binding;
`{:then}` / `{:catch}` hand the whole binding to one sub-parse
(`parse_pattern_with_comments`), which swallows the annotation. An **identifier**
binding is where the two part company — its span stays on the bare name in both
routes, so a window anchored on that span would end before the annotation and
leave its comment attached nowhere. The window runs to the end of the binding
(`tsv_ts::pattern_binding_end`), annotation included.

Covered: the then-shorthand with an identifier binding, a `{:then}` destructuring
binding, and a `{:catch}` identifier binding. The wire is a **parser match** —
canonical attaches the comment inside the annotation as `leadingComments`, and
tsv reproduces it.

The four `input_invalid_*` files pin the region's **two** edges, at both call
sites (the then-shorthand reader and the branch reader). Canonical's
`read_type_annotation` opens with `allow_whitespace(); eat(':')`, so only
whitespace may sit on either side of the annotation — a comment after the bare
pattern (`then a /* c */: A`) and one after the whole binding
(`then a: A /* c */`) are both parse errors there, and tsv rejects both.

Neither reading covers the other, which is why the reader takes two. The pattern
node's span is the **bare** binding for every kind — `attach_pattern_type_annotation`
leaves it there, and only a destructuring pattern's wire `end` widens at emit time
(an annotated identifier stays bare on the wire too) — so a bare-span reading alone
covers the near edge and calls the whole far tail an annotation, while a
`pattern_binding_end` reading alone steps past the `:` and re-opens the near one.
Either single reading accepts a head with the comment silently eaten by the
sub-parse's lookahead, a loss no gate could see: a comment that is never registered
never existed as far as the print-once ledger knows. Same region, far edge only, at
[each/context_annotation_comment](../../each/context_annotation_comment_prettier_divergence/).

## Prettier divergence (formatter)

User comments are valuable and shouldn't be silently removed; they are
syntactically valid here. prettier-plugin-svelte prints the block head from a
comment-blind path and drops them. See
[conformance_prettier_svelte.md §Svelte: destructuring binding-pattern comments](../../../../../../docs/conformance_prettier_svelte.md#svelte-destructuring-binding-pattern-comments).

## Related

- [context_annotation_comment](../../each/context_annotation_comment_prettier_divergence/) — the `{#each}` route, which parses the annotation separately
- [destructure_comment](../destructure_comment_prettier_divergence/) — the same heads' binding-pattern interiors
