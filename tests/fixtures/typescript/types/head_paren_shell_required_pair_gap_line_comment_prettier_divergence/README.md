# head_paren_shell_required_pair_gap_line_comment_prettier_divergence

The leading-edge paren shell of
[head_paren_shell_member_gap_line_comment](../head_paren_shell_member_gap_line_comment_prettier_divergence/)
at the two positions whose value takes a pair the position itself **requires**: an
**optional tuple element** and an **intersection's non-first member**. The type-parameter
`extends` constraint is the sibling that already answers this (c4 of
[head_paren_shell_hang_gap_line_comment](../head_paren_shell_hang_gap_line_comment_prettier_divergence/));
these two are the ones that never asked.

tsv, at c1:

```ts
type A = [
	(
		// c1
		B extends C ? D : E
	)?
];
```

## Reason: one authoring's fixed point is the other's

The shell strips, so its `//` ends up in the required pair's own leading gap — which is
exactly where the reparse finds it, and where the author would have written it bare. The
pair's emitter therefore owns the run and the shell declines its copy; left to itself the
shell emitted a bare `hardline` at whatever indent it was built at plus a `flush_break` that
opened the group it landed in, so the two passes disagreed (an F1 violation, not a
divergence). `input` is the fixed point the **bare** authoring already settles on —
`unformatted_ours_paren_shell.svelte` and `unformatted_ours_paren_layer.svelte` are the same
comment written one and two paren layers out, and both must land here.

⚠️ A required pair around the **shell itself** is still excluded, as the sibling fixture
says: there the pair opens around the run and *is* that run's emitter. What these two
positions have is a pair required around something the shell merely sits inside. Both are
one predicate with two arms (`Printer::required_paren_open_run`), because they are the same
fact one link apart — the shell strips, so its run lands just inside the pair either way.

⚠️ The EDGE arm's whole licence is that these positions have **no enclosing gap** to widen
instead, so it declines wherever one already claimed the run. A union member's `|` gap does
exactly that for an intersection member whose own first member is the shell
(`| (⏎// c⏎I⏎) & J`, in
[head_paren_shell_member_gap_line_comment](../head_paren_shell_member_gap_line_comment_prettier_divergence/)):
opening the pair as well printed the comment above the `| ` **and** inside the pair —
[comments.md](../../../../../docs/comments.md) hazard 3. The gates that ask the question run
before any claim is set, so the filter decides only at emission, which is the only place two
emitters could collide.

Both positions render that pair the one way every other required-pair position does — the
shape `Printer::build_open_required_paren_doc` produces: the run on its own line where the
author gave it one (or on the `(`'s line, one normalizing space in, where they glued it), the
value indented into the shell, and the `)` on its own line. The intersection member used to
weld instead (`(// c3⏎↹K extends L ? M : N);` — no space, no indent, `)` on the value's
line), which is the third form that shape exists to prevent. Which LINE the comment takes
stays the author's;
[union_intersection_retained_paren_leading_line_comment](../union_intersection_retained_paren_leading_line_comment_prettier_divergence/)
is what pins that split — every member kind there, first and later alike, keeps the line it
was written on and takes this same shape around it.

## Prettier

Prettier hoists the comment out in front of the pair at the optional element
(`[⏎↹// c1⏎↹(B extends C ? D : E)?⏎]`, re-binding it from the operand to the whole element —
the divergence [optional_element_paren_leading_line_comment](./../tuple/optional_element_paren_leading_line_comment_prettier_divergence/)
already catalogs) and lifts it onto the `&` line at the intersection member
(`J & // c3`). Neither form is reachable from the paren authorings in one pass: the chains
are pinned by `audit_signature_paren_shell.txt` and `audit_signature_paren_layer.txt`, and
the optional element's chain **drops the required pair** on its second pass
(`B extends C ? D : E?`), which is prettier corrupting its own output.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

**c1–c2** — an optional tuple element, over a conditional's check type and over an
intersection head.

**c3–c4** — an intersection's non-first member, over the same two shapes.

## Files

`unformatted_ours_paren_shell.svelte` carries the shell at the leading edge, spelled as
tightly as the grammar allows: bare at the optional element (`[(⏎// c1⏎B) extends C ? D :
E?]`), and with the author's pair at the intersection member, where dropping it would
re-associate the type (`J & (K) extends L ? M : N` is a conditional, not an intersection).
`unformatted_ours_paren_layer.svelte` adds one more redundant layer at every case — the
authoring that used to stop the descent dead.
