# template_foreign_lang_nested_prettier_divergence

A foreign `<template lang="…">` body keeps the author's own columns at every nesting
depth. Prettier indents the body's **first line** to the element's indent and leaves
every other line where it was.

```svelte
<!-- tsv -->                    <!-- prettier, pass 1 -->
<div>                           <div>
	<template lang="coffee">    	<template lang="coffee">
if cond                         	if cond
  x = 1                           x = 1
	</template>                 	</template>
</div>                          </div>
```

Prettier is **non-convergent** here: the inserted indent becomes part of the body it
re-reads next pass, and its leading strip only removes blanks *before* the delimiter's
newline, never after it — so pass 2 emits `\t\tif cond`, pass 3 `\t\t\tif cond`, without
end. `prettier_nonconvergent.txt` records that (rule F5); there is no fixed point to pin.

The divergence has a narrow trigger, and the trigger is prettier's own: it prints a
foreign template through two different paths. `lang="pug"` takes `preformattedBody`,
whose `literalline` leaves the body at column 0 — that path agrees with tsv, and is the
plain [template_foreign_lang_body](../template_foreign_lang_body/). The five names in
prettier's `unsupportedLanguages` list (`coffee`, `coffeescript`, `styl`, `stylus`,
`sass`) take the element printer's raw arm instead, whose `hardline` is what inserts the
indent. At depth 0 the two paths coincide, so only a nested template can tell them apart.

tsv has one path for every foreign language, and it is the `literalline` one.

## Reason

The body is a whitespace-significant language tsv does not parse, so its lines mean
something only relative to each other — indenting one and not the rest changes what the
body says. Preserving the authored columns is also the only stable answer, as prettier's
own oscillation shows. See
[conformance_prettier_svelte.md §Svelte: Elements](../../../../../docs/conformance_prettier_svelte.md#svelte-elements).

## Related

- [elements/template_foreign_lang_body](../template_foreign_lang_body/) — the body-emission rule itself, where prettier agrees
- [elements/template_foreign_lang](../template_foreign_lang/) — the plain foreign template (no comment, no nesting)
- [style/foreign_lang](../../style/foreign_lang/) — the `<style>` side of the same preserve-verbatim rule
