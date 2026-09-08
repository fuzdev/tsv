# function_type_return_hug_union_line_comment_prettier_divergence

A line comment trailing a function type's `=>` ahead of a **hugging** union
return — a reference member (`Map<…> | null`) or a brace member (`{ … } | null`).

Both formatters keep the comment trailing the `=>`; tsv indents the union one
level (§Uniform Forced-Continuation Indent — the layout every keyword→value gap
takes, and the one a non-union return already takes at this very gap), where
prettier leaves it flush. A hugging union prints with no group of its own, so it
answers this gap exactly as a simple type does; a **non-hugging** union (`C`)
breaks after the `=>` and hangs the comment with its members, its own layout,
matching prettier.

Both forms are stable under their respective formatters.

## Reason

Per Comment Position Philosophy, the comment does not move in either formatter;
only the continuation's indent differs, which is the uniform rule rather than a
per-construct choice.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
