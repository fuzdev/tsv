# foreign_lang_frozen_prettier_divergence

A top-level `<style>` whose `lang` names anything other than `css` freezes — the body's
bytes ride out verbatim, never reprinted — even when it is unformatted. Prettier formats
scss/less with real parsers, which tsv does not have.

```svelte
<!-- tsv -->                    <!-- prettier -->
<style lang="scss">             <style lang="scss">
	a{color:red} /* c */        	a {
</style>                        		color: red;
                                	} /* c */
                                </style>
```

The sibling plain fixture [style/foreign_lang](../foreign_lang/) pins the *coinciding*
case — a body already shaped the way prettier's less parser would emit it. This fixture
pins the trade itself: on content the real parser would reshape, tsv keeps the author's
bytes, because formatting a language it cannot parse means guessing with the CSS printer,
and the guess corrupts (see the nested twin, where that was live). The body still parses
as CSS — canonical Svelte runs `parseCss` over every top-level style body regardless of
`lang`, so a top-level freeze only ever sees CSS-parseable content — but that fact
licenses nothing about *printing* it, which is why the freeze does not re-indent either.
See
[conformance_prettier_svelte.md §Svelte: Foreign-language embedded bodies](../../../../../docs/conformance_prettier_svelte.md#svelte-foreign-language-embedded-bodies).

## Related

- [style/foreign_lang](../foreign_lang/) — the coinciding already-shaped case (plain)
- [style/foreign_lang_verbatim](../foreign_lang_verbatim/) — sass and stylus, where prettier freezes too and the bytes agree
- [elements/style_foreign_lang_nested](../../elements/style_foreign_lang_nested_prettier_divergence/) — the nested position, where the gate replaced a live corruption
