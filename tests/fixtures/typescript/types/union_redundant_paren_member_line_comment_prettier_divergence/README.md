# union_redundant_paren_member_line_comment_prettier_divergence

A leading **line** comment inside a **redundant** parenthesized union member — one
whose parens the comment-free rule strips (`a | (// c\n b) | d`, where `(b)`
collapses to `b`). Because the parens don't survive, the comment cannot stay
"inside" them the way it does for a **retained** paren member (union / intersection
/ function / conditional — see
[union_intersection_retained_paren_leading_line_comment](../union_intersection_retained_paren_leading_line_comment_prettier_divergence/)).

**tsv** keeps the comment with the member it leads, on its own line before the
`| ` — the same own-line slot a between-member comment takes, and the later-member
analog of the first member's after-`|` placement:

```ts
type Mid =
	| a
	// c
	| b
	| d;
```

**Prettier** relocates the comment across the member boundary to **trail the
previous member** (`| a // c`), keeping the rest inline — `variant_trailing.svelte`:

```ts
type Mid =
	| a // c
	| b
	| d;
```

Both forms are dual-stable (each formatter keeps its own), so this is a
`variant_*` divergence: the two formatters land on different stable shapes from the
same authored input. `unformatted_ours_redundant_paren.svelte` is that authored
input (`a | (// c\n b) | d`, parens present) — tsv normalizes it to `input.svelte`,
prettier to `variant_trailing.svelte`. `Nested` shows a doubled redundant paren
(`p | ((// c\n q)) | r`) collapsing the same way.

Per Comment Position Philosophy, tsv associates the comment with the member it
documents (`b`) rather than hoisting it onto a different member (`a`). This is the
stripped-paren counterpart of the retained-paren keep-inside rule: the comment
never changes which parens are retained, only where it renders once they are.

Before this was pinned, tsv emitted the comment **after** the `| ` on the first
pass (`| // c\n b`, the strip path's inline placement — correct only for the first
member) and drifted it before the `| ` on the next — a non-idempotency.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
