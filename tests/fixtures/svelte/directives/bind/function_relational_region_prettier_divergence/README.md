# function_relational_region_prettier_divergence

A Svelte function binding (`bind:value={get, set}`) prints its two operands bare, with no sequence pair around them, so the comma between them is a comma-list separator — the type-argument separator — and between a getter holding a `<` and a setter holding a `>` that may close the region the `<` opens, the pairs the printer otherwise decides print as the author wrote them: a setter's lone parameter written without a pair keeps none, a pair written behind `typeof` or a sign stays, and so does a pair written around an operand holding the `<`. Prettier adds the first and strips the others, and its output is one generic call where the author wrote a getter and a setter.

tsv: `bind:value={() => lo < hi, v => v > (lo + hi) / 2 && set(v)}`, `bind:value={() => a < b, (v) => typeof (v) > (a, b)}`, `bind:value={(() => lo < hi), (v) => v > (lo + hi) / 2 && set(v)}`
Prettier: `bind:value={() => lo < hi, (v) => v > (lo + hi) / 2 && set(v)}`, `bind:value={() => a < b, (v) => typeof v > (a, b)}`, `bind:value={() => lo < hi, (v) => v > (lo + hi) / 2 && set(v)}`

## Reason

**Semantic preservation.** Under `<script lang="ts">` Svelte reads the value with acorn-typescript, which, like tsc, reads past a `<` as a type-argument list wherever it can complete one. `(v) => v` is a function type's spelling and `typeof v` a type query, so with prettier's pair the value is `lo<hi, (v) => v>(lo + hi) / 2 && set(v)` — ONE expression where there were two — and Svelte's compiler rejects the component (`Can only bind to an Identifier or MemberExpression or a {get, set} pair`). The bare spellings are the same program as the paired ones, and every parser reads them as the two operands. A pair around the getter is the author's guard the other way round: inside `(() => lo < hi)` the `<` ends at the `)`, and with the pair stripped it reaches the setter's `>`, the same one generic call. Prettier's own second pass prints its output as the generic call (`lo<hi, (v) => v>(lo + hi)`), the chain `audit_signature.txt` pins.

The host prints the operands one by one, with its own separators and comments, but builds them in one TypeScript printer, after the comma-list rules have read the operands as the list they are, and prints each operand inside the pair those rules keep around it (except an operand a `prettier-ignore` freezes, which prints its own source slice and so without that pair) — the rules a sequence's operands follow in a `<script>` ([relational_region_arrow_param_pair](../../../../typescript/expressions/binary/relational_region_arrow_param_pair_prettier_divergence/), [relational_region_prefix_shell](../../../../typescript/expressions/binary/relational_region_prefix_shell_prettier_divergence/)). The cells:

- **The setter's bare parameter** — the realistic binding on one line, and the width-broken one, whose operands each take their own line.
- **A pair behind `typeof` or a unary `-`**, kept as written.
- **An operand's own pair around a comparison** — the getter's, around a value and around an arrow the author parenthesized to guard it (`(() => lo < hi)`), plain, behind a glued block comment, beside a frozen setter, and ahead of a line comment that breaks the value open.
- **A comment beside the setter** — a block comment glued ahead of it, and a line comment after the getter's comma, which breaks the value open.
- **Controls** — where the getter holds no `<` (`() => lo`), the setter no closer (`(v) => set(v)`), or the getter only a `<=`, no region opens, and the pair stays as both formatters print it.

## Variants

`unformatted_ours_compact` is the space-free authoring of every cell; tsv prints each as the input.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "No pair of the printer's own between the two") and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index.
