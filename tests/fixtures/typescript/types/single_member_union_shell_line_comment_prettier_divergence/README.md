# single_member_union_shell_line_comment_prettier_divergence

A union with ONE member — the leading-operator spelling `| X`, which prettier drops in
postprocess — whose member is a redundant paren shell holding a leading `//`. The union
collapses to its member, so the comment changes only **where** it renders; it never changes
which parens survive, and the collapsed union contributes no `|` of its own.

tsv, at `A`:

```ts
type A = // c1
	B | C;
```

## Reason: a comment never changes which parens are retained

The comment-free `| (B | C)` already collapses to `B | C` — the lone member prints in the
union's own position, so its precedence parens fall away
([single_member_union_intersection_paren](../single_member_union_intersection_paren/)). A
`//` inside that shell is not supposed to change that answer, only where the run lands once
the parens are gone, which is the rule the whole paren family states
(`Printer::build_required_paren_operand_doc`).

It did change it. The `//` routed the union to the forced-multiline member layout ahead of
the collapse branch, and that layout emits a `| ` per member and asks the retained-paren
rule of the member as if the union had two: the pair came back **retained** and a `| ` was
**fabricated**, purely because of the comment (`| ( // c1⏎↹↹B | C⏎↹  );`). At `Y`, where the
member's pair really is required, the two passes formatted different trees outright — pass 1
indented under a break after `=` that pass 2 removed (an F1 violation, not a divergence).

The router now asks its paren trigger only of a **multi-member** union: with one member the
question is about the MEMBER — which parens it keeps and where its run lands — and the answer
is the enclosing gap's, since a one-member union is a descent link of the leading-edge seam
(`Printer::head_stripped_paren_shell`). The run lands in that gap, at its own indent, which
is where the reparse — finding the collapsed member — also puts it. `C1` is the same
authoring without the `|`, the form every case above lands on.

The union's own leading `|`→member gap is a different question and still routes: a `//`
written *there* (`| // c⏎A`) takes the multiline layout, keeping the pipe.

The **delimiter-line** question is not this one and is answered from the source: at `M` and
`P` the author wrote `| (` between the `[` / `(` and the comment, so nothing is glued to the
delimiter and the run takes its own line (prettier's placement exactly). The bare authoring
glues, and keeps the delimiter's line — the divergence
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
catalogs for tuple `[` and the paren shells.

## Prettier

Prettier drops the whole run onto its own line after the `=` / `:`, where tsv trails the
first comment on the operator and indents the continuation — the placement
[intersection_leading_line_comment](../intersection_leading_line_comment_prettier_divergence/)
catalogs. At `Y` prettier hoists the run out in front of the required pair
(`// c8⏎(Z | A1) &⏎↹B1`), the relocation
[head_paren_shell_union_inner_line_comment](../head_paren_shell_union_inner_line_comment_prettier_divergence/)
catalogs.

## Files

`unformatted_ours_leading_operator.svelte` is the authored form — the `|`, the shell and the
comment all present; tsv normalizes it to `input.svelte`. Prettier reaches no single-form
fixed point from it, so `audit_signature_leading_operator.txt` pins the whole chain; and
prettier is non-idempotent on its OWN output here — `Y`'s required pair rides the `=` line on
pass 1 and drops below it on pass 2 — which `audit_signature.txt` pins.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
