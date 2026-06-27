# css_atrule_decl_format_ignore_prettier_divergence

`/* format-ignore */` suppresses formatting of a declaration placed **directly
inside an at-rule block** — the CSS-nesting case where `@media`/`@supports`/etc.
holds declarations of its parent rule, not a nested rule. This is tsv's
tool-neutral spelling of the directive, honored alongside `prettier-ignore`. It
complements [css_nested](../css_nested_prettier_divergence/), which covers a
`format-ignore`d **rule** inside `@media` and a `format-ignore`d declaration
inside a *nested rule*; the declaration directly in the at-rule body is a
distinct printer path.

tsv preserves the marked declaration verbatim and keeps formatting the rest of
the block — the following `background` declaration is a control that still
normalizes:

```svelte
<style>
	div {
		@media screen {
			/* format-ignore */
			color:   red;
			background: blue;
		}
	}
</style>
```

Prettier doesn't recognize `format-ignore`, so it collapses the marked
declaration's spacing too (see `output_prettier.svelte`) — `color:   red` →
`color: red`. That one declaration is the entire divergence; `background: blue`
is unchanged by both tools.

## Reason

`format-ignore` is a tsv-native directive (the `prettier-ignore` family is also
honored for drop-in compatibility). See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive)
and [directives.md](../../../../../../docs/directives.md).
