# Arrow return-type function-type comment divergence

An arrow whose return type is a **function type** needs a disambiguating pair
around it — `(x: T): ((y: T) => T) =>`, never `(x: T): (y: T) => T =>`, whose
second `=>` the reparse would read as the arrow's own. The pair belongs to the
position, not to the type, so the author's own shell is redundant *as a type*
and prettier treats the printed pair as synthesized: it strips the shell and
hoists whatever was inside it out. tsv keeps every comment where it was
written, the answer the whole required-pair family gives.

| authored | tsv | prettier |
| --- | --- | --- |
| `: /* c1 */ ((y: T) => T)` | unchanged (a match) | unchanged |
| `: (/* c2 */ (y: T) => T)` | unchanged — inside the pair | `: /* c2 */ ((y: T) => T)` |
| `: ((y: T) => T /* c3 */)` | unchanged — inside the pair | `: ((y: T) => T) /* c3 */` |
| `: // c4` then the type | continuation-indented under `:` | flush under `:` |
| `: (⏎ (y: T) => T // c5⏎)` | the pair opens, the `//` stays inside | `: ((y: T) => T) => // c5` |

The two line-comment cases are why the position is not merely taste. `// c5`
deferred past the `)` lands on a line the reparse cannot re-break, and prettier
is **not idempotent** there: its own second pass moves the comment again, onto
its own line below the signature (pinned in `audit_signature.txt`). Retaining
the pair and flushing the `//` inside it is a one-pass fixed point.

An authored shell whose leading gap holds a `//` (`: (// c⏎(y: T) => T)`) is
**not** a third form: the `:`→type hang strips that shell and lands the run at
the continuation indent, converging on the `// c4` spelling above — pinned by
`unformatted_ours_shell_leading_line.svelte`.

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Arrow return-type function-type pair) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
