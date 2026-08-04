# declarations_prettier_ignore_head_prettier_divergence

An own-line directive in the keyword→first-declarator gap freezes the first declarator
(Rule A — the gap opens just past `const`/`let`/`var`, so the first member freezes like
any other), and tsv keeps the directive on **its own line**:

```ts
const
	// prettier-ignore
	aaa   =   1,
	bbb = 2;
```

Prettier pulls it flush against the keyword (`const // prettier-ignore`) and still freezes,
because its ignore check reads comment *attachment* rather than placement
(`output_prettier.svelte`, prettier-stable). tsv cannot follow: a directive sharing its line
with anything else is inert under the placement floor, so the relocated form would lose the
freeze on tsv's own second pass. `divergent_variant_flush` pins that — prettier keeps the
flush form frozen, tsv reads the directive as inert and reformats both declarators, landing
on a third stable form.

Both spellings behave identically; the block spelling needs the same broken, indented gap for
the same reason. A `for` header's init clause is the same declarator list and anchors its
first item at the same keyword end. When the gap already holds another comment the two tools
agree, since that comment takes the keyword line and the directive is own-line either way (the
ordinary `syntax/comments/variable_keyword_line_comment` fixture).

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
