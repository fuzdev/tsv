# template_foreign_lang_unknown_prettier_divergence

A `<template>` whose `lang` names anything other than `html` freezes — including names
prettier has never heard of. Prettier freezes only pug and its five-name denylist
(`coffee`/`coffeescript`/`styl`/`stylus`/`sass`); every other name falls through to its
element printer, which formats the body as ordinary markup.

```svelte
<!-- tsv -->                    <!-- prettier -->
<template lang="foo">           <template lang="foo">
<div   >x</div>                 	<div>x</div>
</template>                     </template>
```

An unknown language name is exactly the case where reflowing is least defensible: the
name `lang="jade"` — pug's older name, absent from prettier's list — would have its
indentation-significant body reflowed as markup there. tsv's rule needs no list: a body
declared as anything other than the language this position formats is the author's own
bytes. See
[conformance_prettier_svelte.md §Svelte: Foreign-language embedded bodies](../../../../../docs/conformance_prettier_svelte.md#svelte-foreign-language-embedded-bodies).

## Related

- [elements/template_foreign_lang](../template_foreign_lang/) — `lang="pug"`, where prettier freezes too and the outputs agree
- [elements/template_lang_trim](../template_lang_trim_prettier_divergence/) — the trim side of the same routing question
