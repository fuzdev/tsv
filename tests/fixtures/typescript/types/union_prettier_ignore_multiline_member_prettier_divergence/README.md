# union_prettier_ignore_multiline_member_prettier_divergence

A `// prettier-ignore` freezing the first union member, where that frozen member
spans multiple lines (an object type literal kept verbatim).

**tsv** keeps the union broken one member per line — its layout whenever a member
spans lines — so the frozen slice and the next member sit on their own `|` lines:

```ts
type U =
	// prettier-ignore
	| {
			a:   1
			b: 2
	  }
	| (b1 & b2);
```

**Prettier** glues the next member onto the frozen slice's last line, dropping the
leading `|` (`output_prettier.svelte`):

```ts
type U =
	// prettier-ignore
	{
			a:   1
			b: 2
	  } | (b1 & b2);
```

## Reason

Holding the union's per-line layout when a frozen member is multi-line keeps the
frozen slice visually delimited and the members uniformly presented, rather than
splicing an already-reformatted member onto the last line of an opaque verbatim
block. A single-line frozen member keeps the width-decided layout (matching prettier
— see the ordinary `union_prettier_ignore_first_member` fixture).

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
