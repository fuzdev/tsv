# Stripped operand paren, line comment, at a template island's end — width boundary

[binary_stripped_operand_paren_line_comment_island_end](../binary_stripped_operand_paren_line_comment_island_end_prettier_divergence/)
at print width: the comment the value defers to its end counts against the line it ends,
as a comment written after the value does.

- An expression tag (`c1`, `c2`): at 100 columns the chain stays on one line; at 101 it
  breaks, and the comment still ends its last line.
- A sole hugged arrow in an `{#await … then}` head (`c3`, `c4`): at 100 columns the call
  stays flat and the comment ends the head's line before `then`; at 101 the call breaks
  and the comment flushes inside it, where the call's own break ends its line — the embed
  end then has nothing to flush, and `then` stays behind the `)`.
- The same arrow in a `{let}` tag (`c5`, `c6`): at 100 columns the comment ends the
  declaration and the `;` carries it; at 101 the call breaks and takes the comment, and the
  `;` is dropped with it — the `;` is keyed on whether a comment reached the value's end
  in the layout the render chose.

- **tsv**: the stripped-pair authoring (`unformatted_ours_inside_pair`) reaches
  `input.svelte` in one pass.
- **prettier**: deletes the comment from `input.svelte` where it ends a value
  (`output_prettier.svelte`), and from the stripped authoring prints it past the closer
  (`audit_signature_inside_pair.txt`).

See
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes)
(a `//` the value defers to its own end),
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).
