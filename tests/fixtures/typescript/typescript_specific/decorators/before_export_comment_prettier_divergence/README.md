# before_export_comment_prettier_divergence

A decorator written *before* `export` makes the whole thing one declaration whose span opens at
the decorator, so the `export`→`class` gap sits **inside** the declaration. A comment there is
kept where the author wrote it, trailing `export` with the continuation indented — the uniform
declaration-header rule:

```ts
@dec
export // c1
	class C {}
```

Prettier hoists a line comment above `export` (past the decorator) and trails a block comment on
the decorator instead.

Because the gap is inside the declaration rather than before it, there is no following node for
Rule A to bind to, so an own-line directive there **freezes nothing** — in prettier or in tsv;
it only keeps its own line. `unformatted_ours_spaces.svelte` pins that: the spaced class heads
normalize under tsv despite the directive. This is the mirror image of the sanctioned
decorator→declaration gap in a program body.

## Reason

Comment position is authorship signal; ◆comment_preservation. A scan over an inverted range
(the declaration's span starts at the decorator, *before* the `export` keyword a keyword-anchored
scan begins at) finds none of the three comments here and **drops** them all;
◆content_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
