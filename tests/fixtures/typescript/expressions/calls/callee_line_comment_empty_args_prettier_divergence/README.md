# callee_line_comment_empty_args_prettier_divergence

A line comment between a callee and its empty argument list (`call // c⏎()`) runs
to end of line, so the `()` cannot stay on it. tsv keeps the comment on the head
line and drops the argument list to a continuation line **indented one level** —
the uniform forced-continuation indent. Prettier breaks there too, but keeps the
continuation **flush**, and additionally relocates the surrounding syntax in two of
the shapes.

```
// tsv                          // prettier
call // c1                      call // c1
	();                           ();

call?. // c6                    call // c6
	();                           ?.();

a.b // c8                       a
	();                             .b // c8
                                  ();
```

Left inline this is **content loss**, not a layout preference: `call // c⏎()`
formatted to `call // c();` swallowed the call's own parens and the `;` into the
comment, and the mangled output was a fixed point.

## Reason

Three separable differences share this gap:

- **Continuation indent** (every shape). tsv's own layout choice, applied wherever
  a line comment splits a construct's head from its tail, so the tail reads as
  part of its construct rather than as a sibling statement. See
  [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
- **Optional call.** Prettier normalizes every authoring to the
  comment-before-`?.` form. tsv normalizes too, but by callee kind, so the two
  optional shapes here are not a single rule — see the note below.
- **Member chain.** A comment in this gap makes prettier expand the enclosing
  member chain; tsv keeps the chain flat and breaks only where the `//` requires
  it. The comment trails the same member either way — only the chain layout
  differs, and tsv's is the narrower break.

A **block** comment forces nothing (`call /* c10 */()` stays inline, matching
prettier) — it is pinned here as the control for the line-comment rule.

## Known inconsistency: which side of `?.` (c6 vs c7)

tsv's two call-argument printers answer this gap differently, and both cases are
pinned above so a future fix lands as a visible diff:

- **Plain callee** (c6) — the call printer emits `?.` onto the callee before the
  gap's comments, so the comment lands **after** `?.` (`call?. // c⏎\t()`). The
  `call // c⏎?.()` authoring normalizes to it.
- **Member callee** (c7) — the member-chain printer fuses `?.` into the argument
  list's opening `?.(`, so the comment lands **before** `?.`
  (`obj.m // c⏎\t?.()`). The `obj.m?. // c⏎()` authoring normalizes to it, as
  `unformatted_ours_indent.svelte` shows.

Neither path preserves the authored side, and prettier picks before-`?.` for both.
Converging them is a behavior decision, not a bug fix, so this fixture records the
split rather than assuming an answer. Both forms are stable and lossless.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
