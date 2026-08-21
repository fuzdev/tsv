# foreign_lang_frozen_prettier_divergence

A `<script>` whose `lang`/`type` names anything outside the JS/TS family freezes — the
body's bytes ride out verbatim, at both positions. Prettier freezes only its five-name
denylist (`coffee`/`coffeescript`/`styl`/`stylus`/`sass`); every other name falls through
to `babel-ts`, and a name ending in `json`/`importmap` to its JSON parser.

```svelte
<!-- tsv -->                        <!-- prettier -->
<script lang="foo">                 <script lang="foo">
let  a  =  1                        	let a = 1;
</script>                           </script>

<div>                               <div>
	<script type="application/json">	<script type="application/json">
{ "b" :2 }                          		{ "b": 2 }
	</script>                       	</script>
</div>                              </div>
```

Three cells, one rule. `lang="foo"` is the **unknown-name** case, where reflowing is least
defensible: the name tsv has never heard of is exactly the one whose body it knows nothing
about, and prettier's fallthrough would hand it to a JavaScript printer on the strength of
having no entry in a hand-maintained list. The two JSON cells are the **can't-print** case:
prettier reaches for its JSON parser there and hard-**errors** on a body that is not JSON
(landing on its degraded error-swallow path), so freezing is the better answer rather than
merely the available one. tsv's rule needs no list — a body declared as anything other than
a language this position formats is the author's own bytes.

The freeze is verbatim rather than re-indented for the reason the rule itself gives:
rewriting indentation is formatting. Nested content is raw text to canonical Svelte and to
tsv alike, so nothing has established anything about these bodies, and the top-level one is
only known to have parsed — which licenses a rewrite as *safe*, never as *right*. See
[conformance_prettier_svelte.md §Svelte: Foreign-language embedded bodies](../../../../../docs/conformance_prettier_svelte.md#svelte-foreign-language-embedded-bodies).

## Related

- [script/foreign_lang](../foreign_lang/) — `lang="coffee"`, where prettier freezes too and the outputs agree
- [script/lang_js_family](../lang_js_family/) — the other side of the set: every spelling that names JS/TS, formatted at all three positions
- [style/foreign_lang_frozen](../../style/foreign_lang_frozen_prettier_divergence/) — the same freeze-vs-format trade one tag over
- [elements/template_foreign_lang_unknown](../../elements/template_foreign_lang_unknown_prettier_divergence/) — the unknown-name case at the third tag
