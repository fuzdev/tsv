# context_annotation_optional_member_svelte_divergence

An **optional member** inside a block binding's type annotation
(`{#each xs as e: { a?: number }}`). Svelte's own reader corrupts it; tsv reads
the real source, so this is a divergence in tsv's favour.

Svelte builds a block binding's `: T` by tricking acorn into parsing a synthetic
expression (`1-parse/read/context.js`, `read_type_annotation`), and part of that
trick is a **rewrite of the remaining template**:

```js
parser.template.slice(parser.index).replace(/\?\s*:/g, ':');
```

The comment above it explains why — acorn-TS would otherwise read a following
parameter as a sequence expression and choke on its `?:` — but the substitution
is applied to the *source acorn then measures positions in*. So for any
annotation containing an optional member or parameter, Svelte:

- loses the `optional: true` flag, the `?` having been deleted before the parse;
- reports every offset after the `?` one byte short, because the string it
  measured is one byte shorter than the document.

That second effect escapes the annotation, because the head reader measures from
it. The two `}` at the end of the head close different things — the type
literal's at 96, the block head's at 97 — and canonical's `TSTypeAnnotation.end`
comes back 96, one short of the 97 that would take in the type literal's own
brace. The `{#each}` reader then consumes *that* brace as the head's closer, so
the body's first `Text` node begins at 97, on the head's real closing brace, and
its `data`/`raw` open with a stray `}`.

`expected_ours.json` is the real source's reading: `optional: true`, true
offsets, and a body `Text` that starts after the head. The `<script>` above is
the **null control** — the same `{ a?: number }` type in an ordinary TS
position, where acorn-typescript parses it directly with no rewrite, and both
sides agree exactly.

The adjacent-colon spelling every other fixture uses is unaffected: without a
`?:` in the annotation the rewrite matches nothing and the two readings are
identical, which is why this stayed invisible until an injected input produced
it (see [audits.md §Wire-Injection](../../../../../../docs/audits.md#wire-injection-audit-wireaudit)).

See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
