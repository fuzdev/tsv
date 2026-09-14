# type_param_keyword_own_line_comment_prettier_divergence

An **own-line** leading comment between a type parameter's `extends`/`=` keyword
and its constraint/default value (`R extends\n// c\nA`, `U =\n// c\nV`).

**tsv** keeps the comment on its own line, in the indented value block.
**Prettier** pulls the first leading comment up onto the keyword line, and is
**non-idempotent** getting there — its first pass lands the value at the param's
own indent, and a second pass adds the extra indent (`audit_signature.txt` pins
the pass-2 fixed point). tsv stays idempotent.

The **block-run** face of the same gap is here too: a run the author GLUED — a
multi-line block ahead of a single-line one — and then BROKE AFTER
(`W = /* c1\nd1 */ /* c2 */\nX`). The multi-line body cannot print flat, so the
break is the RUN's whichever comment carries it, and **tsv** hangs the value under
the keyword exactly as it does behind the `//` above. **Prettier** relocates the
whole run across the keyword, binding it to the parameter name
(`W /* c1\nd1 */ /* c2 */ = X`), and pulls the value back onto the run's closing
line. The shelled authoring of that run (`unformatted_ours_paren_shell.svelte`)
normalizes to `input.svelte` under tsv in one pass.

## Reason

Per Comment Position Philosophy: the user wrote the comment on its own line, so
tsv preserves that placement rather than collapsing it onto the keyword line. The
glued block run is the same rule read run-wide — the author's break after the run
is where they put it, and moving the run across the keyword rebinds it to the
parameter name, a different thing to have commented.

A comment that is *already* on the keyword line (`U = // c\n…`) tsv emits inline
via `line_suffix` (zero width, so a long trailing comment never forces a preceding
constraint union to break); prettier instead breaks the `extends` constraint under
a long trailing comment, so that case is a separate divergence — see
[type_param_keyword_line_comment](../type_param_keyword_line_comment_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
