# relational_region_inner_gt_clarity_pair_prettier_divergence

The operand behind a `>` that may close a type-argument region prints bare: the clarity pair both formatters put around an `await`, a mixed-arithmetic operand or a called function expression is, there, the `(` that turns two comparisons into one generic call.

tsv: `fn(a < b, c > await d)`
Prettier: `fn(a < b, c > (await d))`, which is the call `fn(a<b, c>(await d))`

## Reason

**Semantic preservation.** A comma is the type-argument separator, so a `>` in a later sibling closes the region an earlier sibling's `<` opened, and a `(` (or a template) past that `>` commits the list on any line — to tsc, to acorn-typescript and to tsv's own parse. Written `fn(a < b, c > await d)` the line is two comparisons to all three; printed with the pair it is one argument, `a<b, c>(await d)`, and in a `<script lang="ts">` the compiled call loses an argument. `audit_signature.txt` pins prettier writing that loss out on its own second pass, for every cell.

Nothing between the `<` and the `>` can be reworded, and the family's pair around a chain's `<` operand ends a region of the `>`'s OWN left operand, which a sibling's `<` is not. What can be left out is the `(`: a clarity pair is one the tree does not need, so the operand prints as the bare spelling the author wrote. The pairs in question are the ones that print FIRST behind the `>` — an `await` operand's, a left operand's under mixed arithmetic (`(d % e) + f`, `(d * e) / f`, `(d << e) << f`, `(d + e) << f`), both at once in `((await d) % e) + f`, and the one around a function expression that is called or used as a tag (`(function () {})()`, `` (function () {})`t` ``), which only a statement's first token needs.

The cells sweep what carries the region: call, `new` and optional-call arguments, array elements, a sequence, a `for` head and a template expression; a `<` deeper in its sibling and a sibling between the two; and a `>>` behind two nested `<` or a `>>>` behind three, which the type grammar scans a `>` at a time (`fn(a < b < c, d >> (await e))` is the call `a<b<c, d>>(await e)`). A second chain in the later sibling is the same cell with one more pair in play: `c < d > await e` takes the family's pair around its own `<` operand, which ends that region and leaves the first sibling's open, so the operand still prints bare. Prettier strips that pair as well, and its `fn(a < b, c < d > (await e))` is the call `c<d>(await e)`.

**A pair the author wrote stays.** Behind a region that reads as a list it is no pair but the generic call's own argument list (`fn(a<b, c>(await d))`, which every parser reads as that call and tsv prints as one). The rule is keyed on which regions are open, a deliberate superset of what any parser's list grammar takes, so it also reaches comparisons no list can be read from — an operator or a call stands inside the region (`b1`, `b2`, `fn(a < b(), …)`, `fn(a < b + 1, …)`). There the pair is harmless and each authoring keeps its own: `a < b && c > (await d)` and `a < b && c > await d` are both fixed points, where prettier adds the pair to the second.

The controls hold the rule's edges: an operand that takes no pair (`-d`, `!d`, `typeof d`, `new D()`); the single chain, whose own pair ends its region on a `)` (`b3`, [relational_chain_type_arg_parens_kept_shell](../relational_chain_type_arg_parens_kept_shell_prettier_divergence/)); a `<` inside a call or an array, whose closer ends the region; a later statement, and a later module item (`b4`, `b5`); a shift behind fewer regions than it closes (a `>>` behind one `<`, a `>>>` behind the two a `<<` opens); and a `<=`, which opens none.

`unformatted_ours_compact` is the space-free authoring of every cell. `unformatted_ours_bare_operand` is `input` with the pair left out of each control no region reaches, where it is added back — the half of the rule that withholding behind every `>` would fail.

The width half is [relational_region_inner_gt_clarity_pair_long](../relational_region_inner_gt_clarity_pair_long_prettier_divergence/); the cells where prettier's output is one tsc rejects outright are [relational_region_inner_gt_clarity_pair_recovered_list](../relational_region_inner_gt_clarity_pair_recovered_list_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
