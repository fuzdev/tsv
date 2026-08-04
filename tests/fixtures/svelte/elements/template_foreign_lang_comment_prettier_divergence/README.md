# template_foreign_lang_comment_prettier_divergence

A `//` the author put on the **tag-name line** of a foreign `<template lang="…">` head stays
there; prettier moves it to its own line.

```svelte
<!-- tsv -->                    <!-- prettier -->
<template // c                  <template
	lang="pug"                  	// c
>                               	lang="pug"
                                >
```

That is the general attribute-list rule, not a `<template>`-specific one — see
[attributes/comment_same_line](../../attributes/comment_same_line_prettier_divergence/), which
pins it for ordinary elements. This fixture is where it is pinned for the foreign head, whose
layout comes from a different builder. Every other comment position in that head matches
prettier and is the plain
[template_foreign_lang_comment](../template_foreign_lang_comment/).

## Reason

Comment placement is a deliberate authoring choice and tsv preserves it. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the catalog entry in
[§Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [attributes/comment_same_line](../../attributes/comment_same_line_prettier_divergence/) — the same rule on ordinary elements
- [elements/template_foreign_lang_comment](../template_foreign_lang_comment/) — the head's other comment positions, which match prettier
- [elements/template_foreign_lang_long](../template_foreign_lang_long/) — the head's width-driven wrap, the layout these comments break into
- [elements/template_foreign_lang](../template_foreign_lang/) — the plain foreign template (no comment)
