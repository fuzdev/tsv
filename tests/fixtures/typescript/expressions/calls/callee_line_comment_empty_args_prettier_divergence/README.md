# callee_line_comment_empty_args_prettier_divergence

A line comment between a callee and its empty argument list (`call // c⏎()`) runs
to end of line, so the `()` cannot stay on it. tsv keeps the comment on the head
line and drops the argument list to a continuation line **indented one level** —
the uniform forced-continuation indent. Prettier breaks there too, but keeps the
continuation **flush**, and additionally relocates the surrounding syntax in one of
the shapes.

```
// tsv                          // prettier
call // c1                      call // c1
	();                           ();

call // c6                      call // c6
	?.();                         ?.();

a.b // c8                       a
	();                             .b // c8
                                  ();
```

Left inline this is **content loss**, not a layout preference: `call // c⏎()`
formatted to `call // c();` swallowed the call's own parens and the `;` into the
comment, and the mangled output was a fixed point.

## Reason

Two separable differences share this gap:

- **Continuation indent** (every shape). tsv's own layout choice, applied wherever
  a line comment splits a construct's head from its tail, so the tail reads as
  part of its construct rather than as a sibling statement. See
  [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
- **Member chain.** A comment in this gap makes prettier expand the enclosing
  member chain; tsv keeps the chain flat and breaks only where the `//` requires
  it. The comment trails the same member either way — only the chain layout
  differs, and tsv's is the narrower break.

An **optional call** places the gap's comment **before `?.`** for either callee
kind (`call // c⏎\t?.()`, `obj.m // c⏎\t?.()`), `?.` fused into the argument
list's opening `?.(` — the side prettier picks, in both printers. Either
authoring normalizes to it (`unformatted_ours_indent.svelte` carries the
after-`?.` forms): `?.` is pure structure in this gap and the comment trails the
callee either way, so which side it sits on carries no authorship signal and the
normalization is lossless. The optional shapes then diverge only by the two
differences above — the continuation indent (c6) and the chain layout (c7).

A **block** comment forces nothing (`call /* c10 */()` stays inline, matching
prettier) — it is pinned here as the control for the line-comment rule. Its
optional forms stay inline too, glued before `?.`: `call /* c11 */?.()` matches
prettier byte-for-byte; `obj.m /* c12 */?.()` differs only in the chain staying
flat where prettier expands it.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
