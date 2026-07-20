# rhs_line_comment_same_line_prettier_divergence

A `//` line comment the author placed on the type-alias `=` line stays there; prettier
moves it to its own line.

tsv:

```ts
type T = // c
	Ref;
```

Prettier relocates `// c` onto its own line:

```ts
type T =
	// c
	Ref;
```

Because a `//` comment runs to end of line, the RHS must drop to the next line either
way — the only question is whether the comment leads that line or trails the `=`. tsv
preserves where the author wrote it; prettier always leads. The choice is independent of
whether the RHS itself breaks (the `type U` union case relocates identically).

tsv is the more self-consistent side here: it keeps a same-line `//` on the `=` line for
both a type-alias `=` and a variable declarator `=` (`const x = // c⏎ref`, which both
formatters keep), whereas prettier keeps it for the declarator but relocates it only for
the type alias. A block comment glued after `=` (`type T = /* c */ Ref`) and an own-line
comment (`type T =⏎// c⏎Ref`) are preserved as-written by both — see
[rhs_leading_comment](../rhs_leading_comment/) and
[rhs_break_leading_comment_long](../rhs_break_leading_comment_long/).

## Reason

Comment placement is a deliberate authoring choice and tsv preserves it. This is the
type-alias `=` analog of the attribute-list
[comment_same_line](../../../../svelte/attributes/comment_same_line_prettier_divergence/)
divergence. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [rhs_leading_comment](../rhs_leading_comment/) — glued/own-line block + own-line line comment after `=` (all match prettier)
- [rhs_break_leading_comment_long](../rhs_break_leading_comment_long/) — glued block comment leading a break-after-`=` RHS (matches prettier)
- [comment_same_line](../../../../svelte/attributes/comment_same_line_prettier_divergence/) — the same-line `//` attribute-list analog
