# js_css_format_ignore_prettier_divergence

`// format-ignore` / `/* format-ignore */` suppress formatting of the next
construct — tsv's tool-neutral spelling of the directive, honored alongside
`prettier-ignore`. The directive works in `<script>` (TypeScript) and `<style>`
(CSS) alike. Prettier doesn't recognize `format-ignore`, so it reformats the
construct anyway.

Each block pairs `format-ignore`d constructs with a `prettier-ignore` control to
show the divergence precisely. tsv honors **both** spellings, so `input.svelte`
keeps every marked construct verbatim (idempotent). Prettier honors only its own
`prettier-ignore`, so in `output_prettier.svelte` the `prettier-ignore`d
statement / rule is preserved unchanged while the `format-ignore`d ones are
reformatted — those are the entire divergence:

```svelte
<script>
	// format-ignore
	const   a  =   {   b:   1,   c:   2   };   // tsv keeps; prettier reformats

	// prettier-ignore
	const   h  =   {   i:   5,   j:   6   };   // tsv keeps; prettier keeps
</script>

<style>
	/* format-ignore */
	div   {   color:   red;   background:   blue;   }   /* tsv keeps; prettier expands */

	/* prettier-ignore */
	p   {   color:   green;   margin:   0;   }   /* tsv keeps; prettier keeps */
</style>
```

## Reason

`format-ignore` is a tsv-native directive (the `prettier-ignore` family is also
honored for drop-in compatibility). See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive)
and [directives.md](../../../../../../docs/directives.md).
