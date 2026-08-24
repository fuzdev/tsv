# head_paren_shell_frozen_gap_line_comment_prettier_divergence

The leading-edge paren shell of
[head_paren_shell_member_gap_line_comment](../head_paren_shell_member_gap_line_comment_prettier_divergence/)
and
[head_paren_shell_hang_gap_line_comment](../head_paren_shell_hang_gap_line_comment_prettier_divergence/),
inside an item an alone-on-line `prettier-ignore` **freezes**. The gaps there widen their own
comment window over the shell's leading run so the enclosing gap prints it; a frozen item is
the one place that must not happen.

tsv, and **prettier**, at c1:

```ts
type A = B<
	// prettier-ignore
	( // c1
		C)[]
>;
```

The frozen slices here are spelled the way the printer spells an open shell everywhere else
— one space after the `(`, the value indented into it — because a frozen slice is *authored*
bytes and the author is this fixture. Nothing enforces that shape under a freeze; matching it
is what keeps the fixture about the widening rather than about a second spelling.

The directive's own line ends the gap's run. The comment inside the shell is not the gap's
to print — it rides out inside the frozen slice, exactly as the shell's `(` and `)` do.

## Reason: a claim reaches a doc that is BUILT, never one that is SLICED

A frozen item is emitted as a verbatim source slice over its own span, so every byte inside
it — the shell, its `//`, the author's own indentation — is already on the page. The
enclosing gap's widened window is paired with a suppression that tells the shell's emitter
to decline its copy ([comments.md](../../../../../docs/comments.md) §The element-comma seam,
`Printer::with_claimed_shell_leading_run`), and that suppression only reaches a doc the
printer builds. Against a slice there is no emitter to suppress: the window widens, the gap
prints the run, and the slice prints it again — one comment, two emitters
([comments.md](../../../../../docs/comments.md) hazard 3).

So the window must be taken on the **unwidened** span wherever the item may freeze, and the
freeze verdict itself must be read from that same unwidened window — a widened one lets the
shell's run change the alone-on-line reading the directive is graded by.

## Prettier

Two of these gaps carry the own-line-preservation divergence this whole family already has:
at the conditional's `?` / `:` branch gaps (c3, c4) and the function type's `=>` return gap
(c5), prettier relocates the directive to trail the operator and dedents the frozen slice,
where tsv keeps the author's own line. A head-trailing directive is **inert** under tsv's
placement classification, so prettier's form would lose the freeze on tsv's second pass. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).

At the type-argument list (c1, c2), the type-alias `=` RHS (c6), the tuple element (c7) and
the union member (c8), tsv and prettier agree on the whole form.

## Cases

**c1–c2** — a type-argument list's first and later argument. The list bounds every gap
window off its item spans, so this is where widening one is most visible.

**c3–c4** — a conditional's `?` and `:` branch gaps.

**c5** — a function type's `=>` return gap.

**c6** — a type-alias `=` RHS whose shell is an **intersection's first member**. The freeze
slices that member, so the intersection's own first-member hoist has nothing to relocate;
the whole member, paren and comment, is verbatim.

This is the one gap no enclosing emitter can see: a head declines for a composite child
(`single_child_frozen`) precisely so the member rules apply one level down, so the `=` gap
does not know its RHS's first member will freeze. The leading-edge descent therefore asks
the intersection itself, in `Printer::head_stripped_paren_shell`'s intersection link. And
the freeze must then actually happen: the hoist that relocates a first member's in-shell
line comment must not win over it, or the `//` spelling loses the freeze outright while the
block spelling (`(/* c */ U) & V`,
[union_prettier_ignore_paren_shell_comment](../union_prettier_ignore_paren_shell_comment_prettier_divergence/))
kept it — the union's member loop already suppressed its own stripped run for a frozen
member, and this was the one path that did not. The frozen slice spans lines, so it forces
the intersection's broken layout (`LeadingRunFreeze::multiline` — a verbatim span is
`will_break`-opaque, so the trigger is explicit); prettier holds that form too.

**c7–c8** — a tuple element's and a union member's gap, as controls: both already read the
freeze verdict before widening, and the fixture pins that they keep doing so.

## Files

`output_prettier.svelte` records prettier's relocated directives at c3–c5. Its form is
self-stable — no `audit_signature.txt` — so the only chain here is tsv's own.
