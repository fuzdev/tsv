# keyword_gap_prettier_ignore_own_line_prettier_divergence

An honored directive keeps its own line in **every** declaration-header gap, including the
ones where nothing freezes:

```ts
function
	// prettier-ignore
	fn(a, b) {
		return 1;
	}
```

Prettier pulls it flush against the keyword (`function // prettier-ignore`) and reformats the
declaration — it decides a directive by comment *attachment*, so the line the author gave it
carries no weight. tsv decides by placement, and the invariant is one-sided on purpose: a
directive tsv relocated onto the keyword's line would be **inert**, so tsv never relocates
one, whether or not this particular gap is a freeze position today. That keeps every header
gap eligible to start honoring a directive later without an emitter silently having destroyed
it first — the declarator and namespace-binding gaps, which do freeze, are the same rule
paying off
([declarator head](../../../declarations/variable/declarations_prettier_ignore_head_prettier_divergence/),
[namespace binding](../../../modules/imports/namespace_prettier_ignore_binding_prettier_divergence/)).

`divergent_variant_flush` pins prettier's stable flush form, which tsv keeps flush (it never
moves a comment *up* either) but re-indents to its own continuation hang — a third stable
form, and the pre-existing keyword→value hang divergence, not a new one.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
