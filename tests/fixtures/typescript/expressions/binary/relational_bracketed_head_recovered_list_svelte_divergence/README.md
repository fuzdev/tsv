# Bracketed type-argument region tsc recovers up to the `>` — Svelte Divergence

A `<` region opening on a `(` is a type-argument list only where **tsc** reads one, so
tsv grades that head's body and `a < (b || c) > (t, u)` reads as the comparison chain
both oracles read. The bodies here are the other side of that line: each is one tsc
claims and then rejects, so tsv rejects too — the `(` head by its grade, the `{` and
`[` heads by committing without one.

tsc does not guess at a `<`. `parseTypeArgumentsInExpression` parses the list for
real, with `parseDelimitedList`'s error RECOVERY (`src/compiler/parser.ts`): a token
no element can start is skipped (`abortParsingListOrMoveToNextToken`) and the parse
resumes, and the region is claimed whenever that recovery still lands on the `>` —
reporting the errors rather than backing off. It abandons the region only where the
recovery cannot get there, and that is the one condition under which the `<` is the
comparison operator.

Which token abandons is therefore per-DELIMITER, because each body is a different
list context with a different element start. A `-` on digits and a `*` open a type
of their own in a tuple, so `[b - 1]` and `[b * c]` carry the list to the `>` where
`(b - 1)` and `(b * c)` do not; a `<<` is re-scanned into the `<` of a nested
argument list, so it never ends a type under any head. Each line below is a body tsc
lands the `>` on, so the compiler rejects it — `tsv_rejects.txt` pins tsv's own error
on the first, and tsv rejects the rest alike.

acorn-typescript instead backs off the failed list and reads the comparison chain,
which `expected_svelte.json` records, so the divergence is pinned from both sides.

Only the first line is a `(` head, and it is the one tsv's own grade decides; the
`{`- and `[`-headed lines reject because those heads read no body at all and commit
on the follower, which happens to agree with the compiler here. Where it does not,
the result is the over-rejection
[relational_bracketed_head_ungraded_body](../relational_bracketed_head_ungraded_body_svelte_divergence/)
records.

Because the canonical parser accepts the input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject), and with no
accepted parse there is nothing for a formatter to claim — hence no `expected.json`
and no format-claim siblings. Prettier's own TypeScript parser rejects the input for
the same reason tsc does.

The accepting side of the same grade — the bodies tsc does abandon — is
[relational_paren_head_value_body](../relational_paren_head_value_body/),
and the parameter-list family tsc claims without reading a body at all is
[relational_paren_head_param_list_svelte_divergence](../relational_paren_head_param_list_svelte_divergence/).

The **printer** side of the same region is separate and unaffected: a shell the
printer keeps still takes a paren pair around the `>`'s left operand, so tsv never
emits one of these bare. See
[conformance_prettier_ts.md §Relational chain type-argument parens](../../../../../../docs/conformance_prettier_ts.md).

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections (bracketed type-argument region tsc claims and acorn-typescript
abandons).
