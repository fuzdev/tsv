# callee_comment_optional_args_prettier_divergence

An **optional call with arguments** splits the callee→`(` gap at its `?.`, and both
formatters preserve the side the author wrote on — the position itself is a match,
pinned in [callee_comment_optional_args](../callee_comment_optional_args/). What is
left here are the two differences the empty-argument twin already carries, reached
through the same gap.

```
// tsv                          // prettier
fn // c1                        fn // c1
	?.(a);                        ?.(a);

obj.m /* c2 */?.(a);            obj
                                  .m /* c2 */
                                  ?.(a);

obj.m // c3                     obj
	?.(a);                        .m // c3
                                  ?.(a);
```

## Reason

- **Continuation indent** (c1). A line comment runs to end of line, so the argument
  list cannot stay on it. tsv drops it to a continuation line **indented one level**;
  prettier keeps it flush. See
  [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
- **Member chain** (c2). A comment in this gap makes prettier expand the enclosing
  member chain; tsv keeps the chain flat and breaks only where the comment requires
  it. The comment trails the same callee either way — only the chain layout differs,
  and tsv's is the narrower break.

c3 is both at once, and shows they are independent: tsv takes the continuation break
the `//` forces and nothing more, where prettier expands the chain around it.

A comment the author put on **its own line** in this gap normalizes differently in the
two formatters, and both settle (`unformatted_ours_own_line.svelte` carries that
authoring). tsv collapses the break and trails the callee — own-line-ness is authoring
signal for a *leading* position, not a trailing one, and this comment trails the callee
whichever line it sits on — reaching `input.svelte`. Prettier moves it inside the parens
to lead the first argument instead (`variant_own_line.svelte`), because with a following
node present its own-line handler attaches there. That form is a fixed point for **both**,
so it is the divergence's second stable shape rather than a third answer.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
