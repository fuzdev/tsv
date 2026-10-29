# Sole multiline template argument behind a `//` in a stripped shell

A `//` after a call's `(`, ahead of a sole multiline template argument written in a grouping
shell (`aaa(// c1⏎(`⏎`))`). The `//` ends its line, so the template starts a line of its own
and the arguments break.

- **tsv**: keeps the `//` on the `(` line (`aaa( // c1⏎→`⏎`⏎)`), the opening-delimiter rule,
  in one pass from the authored form (`unformatted_ours_shell`).
- **prettier**: its first pass reads the stripped `(` before the backtick and hugs the
  template behind the `//` (`prettier_intermediate_to_variant_shell.svelte`); its second
  breaks the arguments and moves the `//` down to lead the template (`output_prettier.svelte`,
  also a tsv fixed point: `variant_own_line.svelte`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Call open paren `(` trailing) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
