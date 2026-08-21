# style_foreign_lang_nested_prettier_divergence

A nested `<style>` whose `lang` names anything other than `css` freezes — the same
verbatim freeze the top-level `<style>` applies, one rule at both positions. Prettier
formats the body with its real less parser. The second block is the same divergence at
the whitespace-only size: a frozen body keeps its two delimiter lines, where prettier's
formattable arm gives it one.

```svelte
<!-- tsv -->                    <!-- prettier -->
<div>                           <div>
	<style lang="less">         	<style lang="less">
		@color: red; /* c */    		@color: red; /* c */
		a{color:@color}         		a {
	</style>                    			color: @color;
	<style lang="less">         		}
                                	</style>
	</style>                    	<style lang="less">
</div>                          	</style>
                                </div>
```

Before the gate, this position parsed the body as CSS unconditionally and the CSS
printer reprinted what it half-understood: `@color: red;` became the at-rule
`@color : red;` — no longer a less variable declaration, content corruption from the
shipped formatter, and a self-contradiction against the top-level gate one element up.
Unlike the top level there is no parse constraint here — nested script/style content is
raw text to canonical Svelte and to tsv — so genuinely non-CSS bodies reach this freeze,
which is why it copies rather than re-indents: `lang="sass"` and `lang="stylus"` are made
of indentation, and shifting such a body off column 0 changes what it says. See
[conformance_prettier_svelte.md §Svelte: Foreign-language embedded bodies](../../../../../docs/conformance_prettier_svelte.md#svelte-foreign-language-embedded-bodies).

## Related

- [style/foreign_lang_verbatim](../../style/foreign_lang_verbatim/) — the indentation-significant bodies this shape protects, at both positions
- [style/foreign_lang_frozen](../../style/foreign_lang_frozen_prettier_divergence/) — the top-level freeze-vs-format divergence
- [elements/script_foreign_lang_nested](../script_foreign_lang_nested/) — the `<script>` side of the nested gate, agreeing
