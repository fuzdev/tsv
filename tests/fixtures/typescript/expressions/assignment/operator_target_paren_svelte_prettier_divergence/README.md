# parenthesized operator assignment target - Svelte and prettier divergence

This fixture pins that a *parenthesized* operator expression parses as an assignment
target and keeps its parens: a unary (`(-a) = 1`, `(typeof a) += 1`, `(void a) = 1`), an
update (`(++a) = 1`, `(a++) ??= 1`), an `await` (`(await a) = 1`), a binary
(`(a + b) = 1`, `(a < b) = 1`, `(a in b) = 1`) and a logical expression
(`(a ?? b) = 1`) — as the whole target, as a destructuring default's target
(`[(-a) = 1] = x`, `({ a: (-b) = 1 } = x)`), and nested where the assignment itself takes
a pair (a call argument, a template hole, an arrow's concise body) or none (a `for` init).

## Why tsv differs

**From Svelte (acorn-typescript).** The parenthesized form is a
`ParenthesizedExpression`, which is a `PrimaryExpression` and so a
`LeftHandSideExpression`: the production `LeftHandSideExpression = AssignmentExpression`
derives it, and only the `AssignmentTargetType` early error (*invalid* for a
parenthesized non-reference) refuses it — the static-semantic class tsv defers, the
same one as `foo() = bar` in
[nonsimple_target](../nonsimple_target_svelte_divergence/). tsc's parser accepts every
line; its checker rejects them. acorn enforces the early error (`Assigning to rvalue`).
See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

**From prettier.** Prettier strips every target pair (`output_prettier.svelte`), and each
stripped target is no longer a `LeftHandSideExpression` on the left of `=` — `-a = 1`,
and `[-a = 1] = x` alike, is a *grammar* error, which tsc's parser (TS1005), acorn and
prettier's own second pass all reject. tsv keeps the pair, the one spelling that
re-parses as the same program. See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript).

The bare spellings are the `input_invalid_*` files of
[operator_target](../operator_target/), rejected by both parsers. The
assignment-tier counterpart, whose stripped form parses as a *different* program, is
[assignment_tier_target_paren](../assignment_tier_target_paren_svelte_prettier_divergence/).
