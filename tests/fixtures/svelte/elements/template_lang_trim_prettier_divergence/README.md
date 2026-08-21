# template_lang_trim_prettier_divergence

tsv trims the decoded `lang` value before asking what it names, so `lang=" pug "` names
pug and the body freezes. Prettier compares the value untrimmed — `" pug "` matches
neither its pug check nor its denylist — and formats the body as markup.

```svelte
<!-- tsv -->                    <!-- prettier -->
<template lang=" pug ">         <template lang=" pug "> h1 x </template>
h1 x
</template>
```

A value with stray spaces still names the language it names; the trim only ever routes
*toward* the freeze, which is the safe direction, and Svelte's own `lang` regex cannot
match a spaced value either (so neither reading changes what parses). See
[conformance_prettier_svelte.md §Svelte: Foreign-language embedded bodies](../../../../../docs/conformance_prettier_svelte.md#svelte-foreign-language-embedded-bodies).

## Related

- [elements/template_foreign_lang_unknown](../template_foreign_lang_unknown_prettier_divergence/) — the unknown-name side of the same routing question
- [attributes/lang_priority](../../attributes/lang_priority/) — the reader's other two rules (`lang` over `type`, empty = absent), both agreements
