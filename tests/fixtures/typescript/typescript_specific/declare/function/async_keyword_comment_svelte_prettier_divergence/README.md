# async_keyword_comment_svelte_prettier_divergence

A comment in each gap of an ambient async function head: `declare`→`async`,
`async`→`function`, and `function`→`*`.

The oracle split is the plain
[async](../async_svelte_prettier_divergence/) fixture's — acorn rejects the bare
ambient `async` signature, prettier rejects it with `'async' modifier cannot be used
in an ambient context.` — and that README carries the argument. This fixture exists
for a narrower reason: the `declare`→`async` gap is a seam **no other input reaches**.

`declare` and `async` are the only two modifiers that can share one function head, so
the head emitter (`Printer::push_function_keyword_head`) prints a keyword *sequence*
rather than a single keyword, and each keyword after the first opens a gap that
belongs to that emitter. A caller that printed a modifier itself — or a sequence
collapsed back to one slot — leaves the gap behind it claimed by nobody, which is a
DROPPED comment. `c1` is that gap, and nothing else in the fixture tree occupies it.

`c2` (`async`→`function`) and `c3` (`function`→`*`) are the same seams the concrete
spellings already exercise — `c3` relocating **past** the `*` exactly as
[function_generator_star_comment](../../../../syntax/comments/function_generator_star_comment_prettier_divergence/)
pins for the non-ambient form, since the emitter is shared. They ride along so the
three gaps are asserted as one head rather than one seam at a time.

A **line** comment in the `declare`→`async` or `async`→`function` gap is not a case
here: its newline trips `async`'s `[no LineTerminator here]`, so the ambient reading
is declined before it is committed to and tsv rejects the input, as acorn does.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../../docs/conformance_svelte.md#typescript-corrections)
and [conformance_prettier_ts.md](../../../../../../../docs/conformance_prettier_ts.md)
§Prettier rejects valid input.
