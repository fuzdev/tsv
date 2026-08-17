# head_paren_shell_member_gap_line_comment_prettier_divergence

The leading-edge paren shell of
[suffix_head_paren_shell_line_comment](../suffix_head_paren_shell_line_comment_prettier_divergence/),
at the **member and branch** gaps rather than at a keyword→value gap: a union member's `|`
gap, a tuple element's and a type argument's list gap, and a conditional's `?` / `:` branch
gap. The shell strips there too, so its `//` lands in that gap — and the gap's own emitter,
not the shell, is what must print it.

tsv, and **prettier**, at every case here:

```ts
type A =
	| B
	// c1
	| C[];
```

The **input is prettier-stable**, so the divergence this directory documents is the
normalization one: prettier does not bring the paren authoring to it. Its own fixed point
relocates the run to trail the *previous* member
([union_infix_pipe_line_comment](../comments/union_infix_pipe_line_comment_prettier_divergence/)),
so the whole chain from `unformatted_ours_paren_shell.svelte` is pinned by
`audit_signature_paren_shell.txt` instead. Where the form itself also diverges — the
function type's `=>` return gap and a type-parameter `extends` constraint — the cases live
in
[head_paren_shell_hang_gap_line_comment](../head_paren_shell_hang_gap_line_comment_prettier_divergence/).

## Reason: one gap, one emitter, on both passes

A shell left to itself emits a bare `hardline` at whatever indent it was built at and a
`flush_break` that opens the group it lands in — neither of which is the enclosing gap's
answer. The reparse finds the comment in that gap, where the gap's own rule applies, so the
first pass and the second disagreed at every gap below. Two authorings of one comment must
reach one fixed point, and the fixed point is the one the bare (paren-free) authoring
already settles on: the enclosing gap claims the run and the shell declines its own copy.

The two disagreements this fixture pins are the two things a shell gets wrong on its own:

- **whose indent** — a union member's run belongs on its own line *above* the `| `
  (`| B⏎// c1⏎| C[]`), where that gap's own run already goes, not hung after it
  (`| // c1⏎  C[]`); a branch's run takes the branch gap's continuation indent, which the
  shell left flush.
- **whose break** — a stripped shell's `flush_break` opened the enclosing conditional's
  `? :` (c4, c5), a break the reparse cannot reproduce because the shell is gone. Where the
  enclosing layout breaks that conditional anyway — a nested conditional inside a branch —
  only the indent moves (c7, c8), which is what makes the two halves separable.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

**c1–c2** — a union **non-first** member whose leading edge is a shell under an array suffix
and under an indexed access. The member gap's run and the hoisted shell run are one seam, so
both go above the `| `.

**c3** — the same shell one link further down: an **intersection's first member**. The
intersection prints that member at the enclosing gap's own indent, so it is a descent link
like the suffixes.

**c4–c6** — a tuple element and a type argument, over a conditional's **check** type and
over an intersection head. The list gap already prints the run at the element's own indent;
what the shell added was the break.

**c7–c10** — a conditional's `?` and `:` branch gaps, over both shapes. The outer
conditional's layout already breaks a nested one, so c7/c8 isolate the indent half.

**c11** — a redundant paren **layer** over the shell, plus two composed descent links
(`((⏎// c11⏎K2)[])['k']`). The descent peels the layer, so an author's extra pair changes
nothing.

⚠️ A union's **first** member is deliberately absent. The union reads its own leading region
— everything from its start to that member — from four emitters across three layout paths,
so it keeps the run rather than handing it to the enclosing gap; the residual is that the
paren authoring reaches the union's own first-member form (`| // c⏎  L[]`) where the
paren-free authoring of the same comment lands ahead of the union (`// c⏎L[] | N`). Two
authorings, two fixed points, both stable.

## Files

`unformatted_ours_paren_shell.svelte` carries the paren authoring — the shell each gap
claims — which reaches `input` under tsv only. The two authorings of one comment reaching
one fixed point is that variant's whole claim. Its own placement per case mirrors `input`'s,
since a placement is authorship the strip preserves rather than a form the strip picks.
