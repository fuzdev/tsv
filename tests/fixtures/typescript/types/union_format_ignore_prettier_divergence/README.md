# union_format_ignore_prettier_divergence

tsv honors its tool-neutral `// format-ignore` at a union member position identically
to `// prettier-ignore`; prettier recognizes only its own family, so it reformats the
`format-ignore`d member.

Each block pairs the two spellings to isolate the divergence. `type A` freezes its
first member with `// format-ignore` — tsv keeps `(a1&a2)` verbatim, prettier reformats
it to `(a1 & a2)` (the whole divergence). `type B` is the control: `// prettier-ignore`
is honored by **both** tools, so its frozen `(c1&c2)` is unchanged in
`output_prettier.svelte`.

```svelte
<script lang="ts">
	type A =
		// format-ignore
		(a1&a2) | (b1 & b2);   // tsv freezes a1&a2; prettier reformats to a1 & a2

	type B =
		// prettier-ignore
		(c1&c2) | (d1 & d2);   // both tools freeze c1&c2
</script>
```

## Reason

`format-ignore` is a tsv-native directive (the `prettier-ignore` family is also honored
for drop-in compatibility). This is the union-member analog of the existing
`format-ignore` fixtures under `tests/fixtures/svelte/syntax/format_ignore/`.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive)
and [directives.md](../../../../../docs/directives.md).
