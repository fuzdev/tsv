# Tuple rest `...`→type gap, format-ignore head

An own-line directive in a tuple rest element's `...`→type gap freezes the element type —
the head rule with `...` as the delimiter, and the type-side twin of the spread's
`...`→argument gap
([spread argument](../../../expressions/spread/argument_prettier_ignore_head_prettier_divergence/)).
tsv keeps the directive **own-line**, where the author put it, and hangs the frozen type
below it:

```ts
type A = [
	...
		// prettier-ignore
		{b:   1}[]
];
```

Prettier freezes the same span but **relocates** the directive to trail the `...`
(`...// prettier-ignore`) and dedents the frozen slice (`output_prettier.svelte`).

## Why tsv differs

A directive sharing the `...`'s line is **inert** under tsv's placement floor, which reads
only a directive alone on its line. Following prettier's relocation would therefore cost the
freeze on tsv's own second pass: pass 1 would print the frozen bytes under a welded
directive, pass 2 would read no freeze and normalize them — a silent loss with nothing
dropped and no gate firing. The authored own-line placement is the idempotent fixed point.
Prettier's relocated form is not self-stable either: its **own** second pass collapses the
block spelling's element onto one line (`audit_signature.txt`), keeping the freeze but not
the layout.

The freeze is **composite-transparent**. A Union / Intersection operand (`K`) declines the
head freeze, because the directive is **adjacent** to it: the composite's own leading run
reaches back across whitespace alone and claims it, so the member rule applies instead. The
head still owns the directive's own-line emission — which is what `K` pins — so the two can
never both claim one directive. Prettier answers that case by wrapping the operand in parens
it invents and freezing the whole composite.

## Expected behavior

- **tsv**: the directive keeps its own line, the element type prints verbatim one level in,
  and the input is a fixed point. Both spellings behave alike (`G`) — placement keys the
  freeze, not the spelling — and a multi-line slice keeps its authored lines (`I`). The last
  case pins the mirror: a directive the author **glued** to the `...` is inert, so the comment
  keeps the line it was written on and the type normalizes.
  `unformatted_ours_spaces.svelte` perturbs whitespace outside every frozen slice; tsv
  normalizes it to input (prettier does not — it relocates the directive).
  `unformatted_ours_paren_shell.svelte` writes the directive **inside** the operand's
  redundant paren shell (`...(⏎// prettier-ignore⏎…⏎)`): the shell strips, the run keeps its
  own line and the inner freezes, so the shelled authoring converges in one pass onto the
  bare authoring's fixed point — one fixed point per formatter, not per authoring. Prettier
  strips the shell too, but with its own relocation, so it converges onto
  `output_prettier.svelte` instead; the variant is `_ours_` because only tsv lands on
  `input`.
- **prettier**: honors the freeze with the comment pulled onto the `...` line
  (`output_prettier.svelte`), including the glued placement, and is not idempotent on its own
  output (`audit_signature.txt`).

An **ordinary** comment in this gap is relocated onto the `...` line by both formatters and
is not affected by this rule —
[rest_single_member_head_line_comment](../rest_single_member_head_line_comment/). The
sibling type heads are
[named tuple `label:`](../../named_tuple_prettier_ignore_own_line_prettier_divergence/) and
[the prefix type operators](../../type_operator_prettier_ignore_operand_prettier_divergence/);
a glued directive elsewhere in a tuple is
[tuple_prettier_ignore_glued_inert](../../tuple_prettier_ignore_glued_inert_prettier_divergence/).

## Reason

◆comment_preservation — tsv preserves the authored line wherever relocating it would cost the
freeze on the next pass. Sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On single-child type positions*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
