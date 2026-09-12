# param_annotation_prettier_ignore_own_line_prettier_divergence

An own-line directive in a **parameter's** `:`→type gap freezes the type, and the parameter
list breaks rather than hugging:

```ts
function f(
	p:
		// prettier-ignore
		{x:   1}
) {}
```

The sole-parameter **hug** is what the directive declines. That layout prints the `:`→type
run inline (`f(p: // prettier-ignore`), which glues the directive to the parameter head — a
placement tsv's own floor reads as **inert**, so the freeze would be lost on the second pass
and the author's bytes normalized once. The breakable path honors the directive through the
annotation's own route, which is where a declarator, a class property and a signature member
already honor it, so declining keeps the sole-parameter spelling agreeing with its siblings.
The three cells are the three parameter hosts: a function declaration, a function type, and
a method signature.

## Why tsv differs

Prettier hugs and relocates the directive to trail the parameter head
(`output_prettier.svelte`), freezing the type from there. That form is self-stable for
prettier but not for tsv: a head-trailing directive is inert under tsv's placement
classification, so tsv's second pass would reformat the very type the directive froze.

`unformatted_ours_paren_shell.svelte` writes the directive **inside** the type's redundant
paren shell (`p: (⏎// prettier-ignore⏎{x:   1})`): the shell strips, the run keeps its own
line, and the inner freezes, so the shelled authoring converges in one pass onto the bare
authoring's fixed point — one fixed point per formatter, not per authoring. Prettier hugs
that spelling too, so only tsv lands on `input`.

## Reason

◆comment_preservation ◆prettier_bug — the author's own-line placement is the only one that
holds the freeze across a second pass. Sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On single-child type positions*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
