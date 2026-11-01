# implied_end_tag_gap_prettier_divergence

The boundary after a format-ignored node is **printed once**. When the frozen node is an
element whose **end tag is implied** — a `<li>` closed by the next `<li>` or by `</ul>`, a
`<tr><td>` closed by `</tbody>`, a `<b>` closed by its parent's `</p>` — the element's span runs
up to that boundary, so the whitespace in front of it is part of the frozen slice. Keeping that
run and printing the structural break too emits the same boundary twice, and each pass folds the
printer's break into the next pass's slice, so the gap grows by a line forever.

This is the trailing-edge twin of
[directive_gap_text](../directive_gap_text_prettier_divergence/), where the parser folds the gap
in front of a frozen **text** into the node itself.

Prettier has the same defect and **never converges** here (`prettier_nonconvergent.txt`), so
there is no oracle to compare against; tsv's claim is the one every fixture makes anyway —
`input.svelte` formats to itself.

## Reason

Content preservation and F1. The slice gives up that trailing whitespace run, and the printer
prints the boundary once, at its own indentation — whether the follower is the parent's close or
a sibling. Dropping the run is render-free: Svelte trims an element's trailing whitespace.

A chain that reaches a `<pre>` — a `<pre>` closed by its parent's `</div>`, a `<li><pre>` closed
by `</ul>` — is the exception: there the run is the `<pre>`'s **content**, so the slice keeps it,
and the closers are written out right after it (`</pre>`, then each chain element outward), the
way tsv and prettier both close the unfrozen element (`<div><pre>a⏎</div>` prints
`<pre>a⏎</pre>⏎</div>`). The last two cases of `input.svelte` are that explicit form;
`unformatted_ours_implied_close.svelte` spells them with the `<pre>` closed implicitly, which
renders the same (the `<pre>` holds `a⏎` either way) and normalizes to input. Prettier keeps the
run and prints its own break after it, adding a line inside the `<pre>` — a render change — on
every pass.

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).

## Files

- `prettier_nonconvergent.txt` — prettier grows the gap on every pass, so no
  `output_prettier.svelte` exists.
- `unformatted_ours_implied_close.svelte` — the `<pre>` cases with their end tags implied; tsv
  writes the closers back out, prettier grows a line inside the `<pre>` on every pass.

## Related

- [directive_gap_text](../directive_gap_text_prettier_divergence/) — the leading-edge gap, folded
  into a frozen text
