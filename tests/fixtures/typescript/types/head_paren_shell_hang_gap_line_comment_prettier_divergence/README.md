# head_paren_shell_hang_gap_line_comment_prettier_divergence

The leading-edge paren shell of
[suffix_head_paren_shell_line_comment](../suffix_head_paren_shell_line_comment_prettier_divergence/)
at the two gaps in that family whose **form** diverges as well: a function type's `=>`
return gap, and a type-parameter `extends` constraint whose value takes a required pair. The
member and branch gaps, where tsv and prettier agree on the form, are
[head_paren_shell_member_gap_line_comment](../head_paren_shell_member_gap_line_comment_prettier_divergence/).

tsv:

```ts
type A = () => // c1
	B[];
```

**Prettier**: the same placement with the continuation left **flush** at the construct's own
indent (`() => // c1⏎B[];`) — the indent-only difference this rule diverges on everywhere.
At the constraint (c4) it also trails the comment onto the `extends` line, where tsv keeps
the own line the author wrote.

## Reason: the indent is the enclosing gap's, uniformly

A `//` runs to end of line, so the value cannot stay on the keyword's line; tsv drops it one
level so the continuation reads as part of the construct rather than as a sibling. See
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

What these gaps add to the rule is **who prints the run**. A shell left to itself emits a
bare `hardline` at whatever indent it was built at and a `flush_break` that opens the group
it lands in; the reparse finds the comment in the enclosing gap, where that gap's own rule
applies, so the two passes disagreed. The enclosing gap claims the run and the shell
declines its own copy — and at c4 so does the **required pair** the constraint puts around
its conditional value, which was a third emitter for the same comment. A pair required
around the *shell* is still excluded, as the sibling fixture says: there the pair opens
around the run and is that run's emitter. A pair required around something the shell merely
sits inside is not that case.

## Cases

**c1–c3** — the `=>` return gap over all three suffix/composite descent links: an array
type's element, an indexed access's object, a conditional's check type.

**c4** — the type-parameter `extends` constraint. Its value is a conditional, which the
constraint position parenthesizes; the shell inside the check type is redundant and strips,
so the run belongs to the `extends` gap and the required pair closes around the bare
conditional. The own-line placement is preserved here, as at every constraint-gap comment.

## Files

`unformatted_ours_paren_shell.svelte` carries the paren authoring — the shell each gap
claims — which reaches `input` under tsv only; prettier flattens both authorings to its own
form. The two authorings of one comment reaching one fixed point is that variant's whole
claim.
