# Divergence: `import`→`defer` phase-keyword comment (preserve)

A comment between `import` and the `defer` phase keyword (the import-phase proposal). tsv keeps
it where the author wrote it; prettier **relocates** it past `defer`.

```ts
// tsv (preserve)                          // prettier (relocate past the keyword)
import /* c1 */ defer * as ns1 from './a'; import defer /* c1 */ * as ns1 from './a';
```

**Why tsv preserves:** the gap between `import` and the phase keyword is a position the author
chose, and a keyword is not a *pure separator* — the one sanctioned reason to trail. This is the
same rule already applied to the `type` modifier in the same header slot
([empty_type_keyword_comment](../empty_type_keyword_comment_prettier_divergence/)); `type` and the
phase keywords are mutually exclusive occupants of that slot, so they follow one rule rather than
two. A line comment stays on its own line with the continuation indented one level (the uniform
module-header rule).

Only `defer` is covered: prettier's parser rejects `import source` outright (`'=' expected`), so
that phase has no oracle and no `output_prettier.*` is possible for it.

## Parser divergence

Svelte's parser (acorn-typescript) rejects `import defer` outright — the import-phase proposal is
not implemented there, so this is a pre-existing tsv over-acceptance, not anything about the
comment. Hence `expected_ours.json` + `expected_svelte.json` and the `_svelte_prettier_divergence`
suffix. See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
