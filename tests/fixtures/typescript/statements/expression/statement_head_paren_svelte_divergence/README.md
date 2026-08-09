# statement-head paren preservation - Svelte divergence

Parens that carry a **statement's reading**. These are not redundant grouping parens,
and stripping them changes what the statement *is* — so the printer must keep them.

## Why the parens are load-bearing

Two ECMAScript lookahead restrictions make a leading token decide the whole statement:

- `ExpressionStatement : [lookahead ∉ { {, function, async function, class, let [ }]
  Expression ;` — so `let[a] = 1;` is a **`VariableDeclaration`** (an array binding
  pattern), while `(let)[a] = 1;` is an **assignment** to the member `let[a]`. The same
  applies to a `for` init and a for-in/of left.
- `ForInOfStatement`'s `[lookahead ∉ { let }]` on the `of` form — `for (let of foo);`
  is a syntax error, and only `for ((let) of foo);` says what the author meant.

The for-of / for-in **left** carries the stronger rule: the restriction is on the head's
leftmost token whatever the shape follows, so `(let).a`, `(let)[a]` and `(let)().a` all
keep the paren, while the **right** of `of` is an ordinary expression position where
`let` needs none. The cases without parens (`foo[let[a]] = 1;`, `let.let[x].foo();`,
`a[1] + (let[2] = 2);`) are the boundary: not statement-initial, so nothing to preserve.

## Why tsv Differs from Svelte

**Acorn-typescript** enforces the strict-mode early error that bars `let` as a name and
rejects every input here:

```typescript
(let)[a] = 1; // ❌ acorn
```

tsv defers that early error (see the sibling
[strict_reserved_name](../../../declarations/variable/strict_reserved_name_svelte_divergence/)),
so it parses these and must then print them correctly. acorn-typescript is tsv's
AST-**shape** target but not its correctness oracle; for validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

## Expected behavior

- **tsv**: every paren above survives, and the input is a fixed point
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: agrees byte-for-byte on all of them — these shapes are prettier's own
  `tests/format/js/identifier/{parentheses,for-of}/` suite, which `corpus:compare:format`
  grades from the other side

`unformatted_paren_placement.svelte` pins the **normalization**: both formatters move
the paren onto the identifier rather than the member, so `for ((let.a) of foo);` and
`for ((let[a]) of foo);` become `for ((let).a of foo);` / `for ((let)[a] of foo);`.

⚠️ **Why a fixture is not sufficient on its own.** The re-meaning half of this family is
**valid, idempotent and comment-clean** — `(let[a] = 1);` printed as `let[a] = 1;`
reparses happily as a `VariableDeclaration` instead of an assignment — so round-trip,
F1, the ledger, the census and the fuzzer are all blind to it. This fixture pins the
bytes; `tests/statement_head_paren_preservation.rs` additionally asserts the **node
types**, which is what actually catches a silent re-meaning.
