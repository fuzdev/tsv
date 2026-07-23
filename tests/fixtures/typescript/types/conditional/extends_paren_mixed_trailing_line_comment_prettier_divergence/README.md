# extends_paren_mixed_trailing_line_comment_prettier_divergence

A redundant paren shell around a conditional type's `extends`-type whose leading
gap holds a **line** comment together with either a **leading block** (mixed,
`(/* b */ // c\n B)`) or a **trailing block** after the type (trailing,
`(// c\n B /* t */)`), and the double-nested `((…))` forms.

Unlike the pure-line paren shell — which trails the comment on the extends-type
(`A extends B // c`, the non-divergence
[extends_paren_leading_line_comment](../extends_paren_leading_line_comment/),
matching prettier's relocation) — a mixed or trailing shell cannot trail its run
on the type losslessly: trailing a leading block would move it from leading to
trailing, and trailing the trailing case would reorder the two comments (a `//`
must end its line). So tsv **hangs** the run at the same fixed point the bare
(paren-free) authoring settles on — the shared keyword-to-value hang seam:

```ts
// mixed
type T1 = A extends /* b */ // c
	B
	? X
	: Y;

// trailing
type T2 = A extends // c
	B /* t */
	? X
	: Y;
```

The leading block trails `extends` inline, the line comment forces the
extends-type onto its own indented line, and a trailing block stays inline at the
type after it — every comment kept in the position the author wrote it, in source
order.

**Prettier** floats the comments out of the extends clause: the leading block
moves before `extends` (`A /* b */ extends B // c`), and the trailing case reorders
to `A extends B /* t */ // c` — the `output_prettier.svelte`.

The `unformatted_ours_*` variants are the paren shells; tsv normalizes them to
`input` in one pass, prettier does not (N6). The pure-line shell keeps its own
trail-on-inner canonical (`extends_paren_leading_line_comment`) — only the
mixed / trailing shapes hang.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
