# Divergence: type-suffix trailing line comment stays inside the brackets

A line comment at the **end** of an indexed access's brackets (`T[K // c1⏎]`), or at the end
of a redundant paren shell around a type (`(A // c2⏎)`). tsv keeps it inside the region the
author wrote it in and drops the closer to its own line. **Prettier carries it out of the
construct** — past the `=` onto the initializer's line (`const a1: T[K] = // c1⏎\ty;`), or
past the `;` when there is no initializer (`let b: T[K]; // c3`) — and strips the paren shell
on the way.

```ts
// tsv (comment stays inside)   // prettier (carried out past the `=`)
const a1: T[                    const a1: T[K] = // c1
	K // c1                             y;
] = y;
```

## Reason

**Every other bracketed type region already answers this question tsv's way, and prettier
agrees on all of them** — a type literal's `}` (`{ b: T // c⏎}`), a type-argument list's `>`
(`Array<⏎T // c⏎>`), a tuple's `]`, and a function type's `)` all keep the comment inside and
break the closer onto its own line, byte-identically in both formatters. Prettier carries the
comment out only from an indexed access and from a paren shell it is about to strip, so it
answers one question two ways; tsv answers it once. A **value**-position redundant paren is
already retained for exactly this reason (`const e = (⏎x // c⏎);`), so retaining the type-side
shell makes the two sides agree too.

Carrying the comment out is also not lossless. It re-binds the comment from the index (or the
parenthesized type) to the whole statement, and it lands the comment on a line that may
already hold one — where the run renders back to back and the second `//` becomes text of the
first (`const a1: T[K] = 1; // c1 // c2`, irreversibly: the merged form is a fixed point in
both formatters). Keeping the comment inside the brackets is what makes that collision
unreachable rather than something the renderer has to defuse. Per
§Comment Position Philosophy, a relocation that can merge is preserved against.

`unformatted_ours_flat.svelte` carries the flat authoring (`T[K // c1⏎] = y;`), which reaches
`input` under tsv only — prettier carries it out from either authoring, so its own broken form
collapses too.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
