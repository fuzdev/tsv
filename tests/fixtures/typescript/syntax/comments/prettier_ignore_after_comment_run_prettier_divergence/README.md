# prettier-ignore after a comment run divergence

A block comment written before an element's comma, and an own-line directive on
the line below:

```js
x = [
	a,
	b
	/* t */,
	// prettier-ignore
	c  +  d
];
```

The comma moves up against `b`, so the comment leads the next element — and
nothing but that comma stood between it and the directive. Prettier glues the two
onto one line:

```js
x = [
	a,
	b,
	/* t */ // prettier-ignore
	c  +  d
];
```

A directive that shares its line with anything is inert under tsv's placement
rule, so that form would silently drop the freeze on the next pass. tsv keeps the
comment on a line of its own and the directive alone on its line, directly above
what it freezes:

```js
x = [
	a,
	b,
	/* t */
	// prettier-ignore
	c  +  d
];
```

Covered in an array literal, a call, a `new`, an object literal, an array and an
object assignment pattern, a tuple type, type arguments and type parameters, and
with a block directive.

Prettier honors the glued placement. Its first pass glues each comment onto its
directive (`prettier_intermediate_to_divergent_variant_before_comma.svelte`); its
second pass collapses the array holding the block directive onto one line, still
frozen (`x = [a, b, /* t */ /* prettier-ignore */ c  +  d];`), and that is its
fixed point (`divergent_variant_glued.svelte`). tsv reads every glued directive
there as inert and reformats what it would have frozen (`c  +  d` → `c + d`), a
third form. The divergence is in which form the authoring normalizes to
(`unformatted_ours_before_comma.svelte`).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(On argument and element lists: a comment before an own-line directive).
