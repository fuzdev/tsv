# suffix_head_paren_shell_line_comment_prettier_divergence

A redundant paren shell carrying a leading `//` sits at the leading printed **edge** of a
keyword→value's value — the element of an array type (`(⏎// c⏎A)[]`), the object of an
indexed access (`(⏎// c⏎A)['k']`), the check type of a conditional
(`(⏎// c⏎A) extends B ? C : D`) — rather than being the whole value. The shell strips, the
comment lands in the enclosing keyword→value gap, and that gap's continuation indent
applies: one level, the same one the bare (paren-free) authoring already settles on.

tsv:

```ts
let a: // c1
	A[];
```

**Prettier**: the same placement, the continuation left **flush** at the construct's own
indent (`let a: // c1⏎A[];`) — the indent-only difference this rule diverges on everywhere.

## Reason: the indent is the enclosing gap's, uniformly

A `//` runs to end of line, so the type cannot stay on the keyword's line; tsv drops it one
level so the continuation reads as part of the construct rather than as a sibling. See
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

The **leading edge** is what this fixture adds to the rule. The keyword→value hang seam
(`Printer::keyword_value_stripped_paren_hang`) must fire here too, not only where the shell
*is* the value; otherwise a shell one link down is stripped by its own emitter instead — which
emits a bare `hardline` at whatever indent it is built at, with the enclosing gap's indent never
applied. That flush form is nobody's fixed point: the reparse finds the comment in the
keyword→value gap, where the hang applies the indent, so the first pass and the second
disagree — an F1 violation at every site below. The comment is authored in one gap and must
be emitted by one emitter, the enclosing gap's, on both passes; the shell declines its own
copy for the duration of the build.

The links are exactly the type constructors that print their head first and at the enclosing
gap's own indent — plus a redundant paren layer, which is not a constructor at all but does
strip, so the descent peels it (c16–c17). A position that **requires** the pair is not one of
them: there the pair is emitted open around the run by its own emitter, which would be a
second emitter for the same comments — `(⏎// c⏎A | B)[]` keeps its parens and is unaffected.

The same rule at the gaps this seam does not serve is
[head_paren_shell_member_gap_line_comment](../head_paren_shell_member_gap_line_comment_prettier_divergence/)
and
[head_paren_shell_hang_gap_line_comment](../head_paren_shell_hang_gap_line_comment_prettier_divergence/).

The shell is stripped, not retained: a **leading** run leaves nothing behind the type for a
later comment to weld onto, so hoisting it into the keyword→value gap is lossless. The
retention rule is keyed on the shell's **trailing** gap only —
[redundant_paren_shell_line_comment](../redundant_paren_shell_line_comment_prettier_divergence/)
is that half, and its `c24` control is this one's paren-free sibling.

## Cases

**c1–c8** — the trailing placement, at every keyword→value gap the seam serves: a `: T`
annotation over an **array** (c1, the repro shape) and over an **indexed access** (c2), a
type-alias `=` RHS (c3), an `as` cast (c4), a mapped-type `]:` value (c5), a type-parameter
`=` default (c6), a return-type annotation (c7), and a type-predicate `is` (c8).

**c9–c13** — the **own-line** placement, at the five gaps whose emitter preserves it
(`append_keyword_value_line_comments`). Two authorings, two fixed points: the strip does not
move the comment relative to the keyword, so the shell authoring lands exactly where the
paren-free authoring of the same placement lands. The `: T` annotation and return type are
absent here on purpose — their emitter trails the first comment unconditionally
(`build_continuation_indent`), so they have one fixed point for both authorings.

**c14–c15** — the conditional **check** type, the third descent link, with and without an
array suffix between it and the shell (the links compose, so `(⏎// c⏎A)[][]` descends twice).

**c16–c17** — a redundant paren **layer** between the gap and the shell (`((⏎// c⏎A)[])`),
in both placements. The layer is itself a shell the comment-free rule strips, so it is a
descent link like the three above; leaving it out of the descent meant one extra pair from
the author suppressed the whole rule at every gap here.

## Files

`unformatted_ours_paren_shell.svelte` carries the paren authoring — the shell the seam
strips — which reaches `input` under tsv only; prettier flattens both authorings to its own
form. The two authorings of one comment reaching one fixed point is that variant's whole
claim. Its own placement per case mirrors `input`'s, since a placement is authorship the
strip preserves rather than a form the strip picks.
