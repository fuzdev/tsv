# type_keyword_comment_prettier_divergence

Comments around the `type` keyword of a type-only re-export with named specifiers —
between `export` and `type`, or between `type` and the specifier braces — are
preserved where the user placed them.

**Prettier**: relocates them *into* the braces, as the first specifier's leading
comment (`output_prettier.svelte`); a line comment also expands the braces:

```
export type { /* c1 */ A } from './a';
export type { /* c2 */ B } from './b';
export type {
	// c3
	C
} from './c';
```

**tsv**: preserves them where the user placed them — a block comment trails inline,
a line comment stays on its own line with the following token after it:

```
export /* c1 */ type { A } from './a';
export type /* c2 */ { B } from './b';
export // c3
type { C } from './c';
export type // c4
{ D } from './d';
```

The `export type /* type */ { E }` case sits in the same `type`→braces position as
c2, but its comment **text is the `type` keyword itself** (scan robustness): tsv's
keyword finder locates the *real* `type` keyword, not the copy inside the comment,
so the comment is preserved rather than dropped — a naive reverse keyword search
would match the comment's `type` and mislocate the gap.

Per Comment Position Philosophy, the user's chosen position is preserved. The
export sibling of `modules/imports/type_keyword_comment_prettier_divergence`, and the
non-empty counterpart of `empty_type_keyword_comment_prettier_divergence`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
