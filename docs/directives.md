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

### Placement

Where the comment sits decides what it freezes. The rule is total and has three
cases:

- **On its own line** (nothing before it on its line): the directive freezes the
  construct that follows it.
- **Glued directly before a value** (same line, only whitespace — or other
  comments — between the directive and what follows; block spelling only, since a
  line comment runs to the end of its line): the directive freezes that value
  whole.
- **Anything else is inert.** A directive *trailing* code on its line — after a
  statement, a list member, a separator, or a declaration head
  (`type A = // format-ignore`) — does nothing: the surrounding code formats
  normally. tsv only ever honors a directive that *precedes* its target.

### On union and intersection type members

The directives also target individual **members** of a union or intersection
type: an own-line directive in the list's leading gap or between members freezes
the **next member** only, and a glued block directive freezes the member it sits
against — the rest of the list keeps formatting normally, separators included.

```ts
type T =
	// format-ignore
	| { x:1, y:2 }   // ← frozen verbatim
	| B              // ← formatted normally
	| C;
```

A directive glued before the whole value (`type T = /* format-ignore */ A | B`)
freezes the whole union instead.

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
The two spellings are honored identically at every honored position — which
positions are honored is decided by [placement](#placement), never by spelling.

## See also

- [conformance_prettier.md §Format-ignore directive](./conformance_prettier.md#format-ignore-directive) — why the `format-ignore` spelling diverges from prettier
