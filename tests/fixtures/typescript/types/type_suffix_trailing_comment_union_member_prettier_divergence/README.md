# Divergence: member-position type-suffix trailing comment stays inside its region

The member-position face of
[type_suffix_trailing_comment](../../declarations/variable/type_suffix_trailing_comment_prettier_divergence/):
a trailing line comment at the end of an indexed access's brackets where the access is a
**union member** (`T[K // c1⏎] | B`), or in a redundant paren shell around the **last** (or
sole) member of a union / intersection (`B | (A // c2⏎)`). tsv keeps the comment inside the
region and drops the closer to its own line. **Prettier carries it out** — to trail the
member (`| T[K] // c1`), or past the `;` for the last member (`type A2 = B | A; // c2`),
stripping the shell on the way.

```ts
// tsv (comment stays inside)   // prettier (carried out past the `;`)
type A2 =                       type A2 = B | A; // c2
	| B
	| (
			A // c2
	  );
```

## Reason

Same rule as the alias/annotation-position fixtures — the bracketed type regions keep a
trailing comment inside in both formatters, so the indexed access answers the question the
same way in member position; carrying it out re-binds the comment from the index to the whole
member.

The **last-member paren shell** is where the carve-out that lets a member shell strip
(`(a // c⏎) | b` → `| a // c⏎| b`, matching prettier —
[union_intersection_parens_line_comment](../union_intersection_parens_line_comment/)) reaches
the end of its argument. That strip is lossless only because a per-member break ends the line
right after the member, flushing the deferred comment where it was written. No separator
follows the last member, so its line ends only after the `;` — the escaped comment re-binds
to the whole statement, and the break the shell forced is one the reparse cannot reproduce,
the parens being gone: prettier's own form here is **non-idempotent** (`type A2 =⏎↹| B⏎↹| A; // c2`
collapses to `type A2 = B | A; // c2` on its second pass). Retaining the shell keeps the
comment inside the construct it was written in and is a fixed point in one pass.

`unformatted_ours_flat.svelte` carries the flat authorings (plus the one-member-union `|`
for `A4`), which reach `input` under tsv only.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
