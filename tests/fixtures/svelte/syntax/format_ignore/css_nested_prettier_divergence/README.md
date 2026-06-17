# css_nested_format_ignore_prettier_divergence

`/* format-ignore */` works at every CSS nesting level — tsv's tool-neutral
spelling of the directive, honored alongside `prettier-ignore`. This fixture
covers a `format-ignore`d **rule** nested inside `@media` (the at-rule block
path) and a `format-ignore`d **declaration** inside a nested rule. Prettier
doesn't recognize `format-ignore`, so it reformats both anyway.

tsv preserves the marked rule / declaration verbatim:

```svelte
<style>
	@media screen {
		/* format-ignore */
		div   {   color:   red;   }

		span {
			/* format-ignore */
			background:   blue;
			color: red;
		}
	}
</style>
```

Prettier (see `output_prettier.svelte`) expands the rule and collapses the
declaration's spacing, as if the directive weren't there.

## Reason

`format-ignore` is a tsv-native directive (the `prettier-ignore` family is also
honored for drop-in compatibility). See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive)
and [directives.md](../../../../../../docs/directives.md).
