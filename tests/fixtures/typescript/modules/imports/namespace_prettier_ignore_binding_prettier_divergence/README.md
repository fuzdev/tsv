# namespace_prettier_ignore_binding_prettier_divergence

An own-line directive in the `import`→binding gap freezes the whole `* as ns` namespace
binding — the node it precedes, so the `as` join rides inside the slice — and tsv keeps
the directive on **its own line**:

```ts
import
	// prettier-ignore
	*   as   ns1 from './a';
```

Prettier pulls it flush against the keyword (`import // prettier-ignore`) and still freezes,
because its ignore check reads comment *attachment* rather than placement
(`output_prettier.svelte`, prettier-stable). tsv cannot follow: a directive sharing its line
with anything else is inert under the placement floor, so the relocated form would lose the
freeze on tsv's own second pass. `divergent_variant_flush` pins that — prettier keeps the
flush form frozen, tsv reads the directive as inert and reformats the binding, landing on a
third stable form (the case whose gap already holds another comment stays frozen there, since
that comment takes the keyword line and the directive is own-line either way).

A default binding is a bare identifier with no interior whitespace to preserve, so its freeze
is invisible; the case is carried to pin the directive's own line, which is a placement rule
rather than a freeze one. The same rule at the declaration headers is
[declarations_prettier_ignore_head](../../../declarations/variable/declarations_prettier_ignore_head_prettier_divergence/).

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
