# Directives

tsv honors in-source comments that suppress formatting for a piece of code. The
directives are recognized in every language tsv formats — TypeScript (`<script>`
and the JS/TS family: `.ts` / `.svelte.ts` / `.mts` / `.cts` / `.js` / `.mjs` /
`.cjs`), CSS (`<style>` and `.css`), and Svelte templates.

Like everything else in tsv, the directives are **not configurable**: they are
always active and cannot be turned off.

## `format-ignore`

Put a `format-ignore` comment immediately before a construct to emit it verbatim
instead of formatting it. The marked construct keeps its original spacing, line
breaks, and alignment; everything else in the file is formatted normally.

```svelte
<script lang="ts">
	// format-ignore
	const matrix = [
		1, 0, 0,
		0, 1, 0,
		0, 0, 1,
	];
</script>

<style>
	/* format-ignore */
	.grid   {   grid-template:   'a b' 1fr / auto;   }
</style>

<!-- format-ignore -->
<div    class="a"    data-attr="value" />
```

The comment delimiters follow the host language — `//` or `/* … */` in
TypeScript, `/* … */` in CSS, and `<!-- … -->` in Svelte templates.

## `format-ignore-start` / `format-ignore-end`

In Svelte templates, a pair of range markers preserves every node between them:

```svelte
<!-- format-ignore-start -->
<div   >  hand   laid   out  </div>
<span  >  and   this   too  </span>
<!-- format-ignore-end -->
```

A range only takes effect at the top level of the template; markers nested inside
an element are treated as ordinary comments.

## `prettier-ignore` compatibility

For compatibility with prettier-authored code, tsv also honors the
`prettier-ignore` family — `prettier-ignore`, `prettier-ignore-start`, and
`prettier-ignore-end` — identically. `format-ignore` is the canonical tsv
spelling; `prettier-ignore` is kept so existing codebases keep working unchanged.
Either spelling works in any position.

## See also

- [conformance_prettier.md §Format-ignore directive](./conformance_prettier.md#format-ignore-directive) — why the `format-ignore` spelling diverges from prettier
