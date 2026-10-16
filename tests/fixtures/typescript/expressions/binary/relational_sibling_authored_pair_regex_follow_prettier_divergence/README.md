# relational_sibling_authored_pair_regex_follow_prettier_divergence

[relational_sibling_authored_pair](../relational_sibling_authored_pair_prettier_divergence/) with a regular expression behind the later element's `>`: the pair around the earlier comparison stays, and prettier's stripped output is one no parser accepts.

tsv: `fn((a < b), c > /r/.test(d))`
Prettier: `fn(a < b, c > /r/.test(d))`

## Reason

**Semantic preservation.** Without the pair, the `<` and the `>` bound a type-argument list, and a `/` past the `>` is no expression that list can be followed by — tsc reports `Expression expected`, acorn-typescript `Unexpected token`, and tsv rejects the line too. So prettier's output is not a program, in a call and in an array alike. tsv keeps the authored pair, which ends the first element before the comma, and the line stays the two comparisons the author wrote. The rule does not read what follows the `>`: the regex is one more follower the same pair answers.

Prettier throws on its own output, so its chain stops at `output_prettier.svelte` and no `audit_signature.txt` can follow it — the erroring pass F4b tolerates.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "An authored pair around an earlier comma sibling") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index.
