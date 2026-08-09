# strict-mode-reserved word as a name - Svelte divergence

The nine strict-mode-reserved words used as ordinary names — here each of the eight
non-`yield`-operator ones in a `var` binding and a parameter, plus the shapes where the
word has a *competing syntactic role* to resolve.

## Why tsv Differs

Nothing excludes these at the **production** level. ecma262 bars them in a single
bullet of §sec-identifiers-static-semantics-early-errors — a Static Semantics **early
error** — and tsv defers early errors to the diagnostics layer, so it parses all of them
as names, as tsc's parser and prettier do. `BindingIdentifier[Yield, Await]` even
readmits `yield` with *no* guard; the spec writes that bar as an early error too, and
its own note says why (so ASI cannot split `let ⏎ await 0;`).

**Acorn-typescript** enforces the early error and rejects every case:

```typescript
var let = 1; // ❌ acorn
```

acorn-typescript is tsv's AST-**shape** target but not its correctness oracle; for
validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

## The competing roles, resolved with tsc's own lookaheads

The last four cases are the subtle ones — the word's real role still wins where it
should, and each resolution is tsc's:

- `class implements {}` **names the class**. After `class`, `implements` opens a
  heritage clause only if an identifier-or-keyword follows (tsc's `isImplementsClause`),
  so the anonymous-with-heritage form stays a heritage clause — that is the sibling
  [export_default_implements](../../class/export_default_implements_svelte_divergence/)
- `constructor(private x)` is a **parameter property**, while `fn_private(private)` is a
  parameter *named* `private`. In a parameter list an accessibility keyword is a modifier
  only if a binding follows it on the same line (tsc's `canFollowModifier`) — the rule
  `readonly`/`override` already used
- `enum yield {}` and `infer let` are the two positions where the word is keyword-lexed
  by tsv's own lexer, so they exercise a different code path than the words the lexer
  already leaves as plain identifiers

## Expected behavior

- **tsv parser**: every occurrence is a plain `Identifier` in a name position (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats all of them, and to exactly this input

**Scope.** This fixture pins the prettier oracle and the AST for a representative set.
The exhaustive matrix — nine words across ~45 positions, plus the three-channel
`BindingIdentifier` / `IdentifierReference` / `LabelIdentifier` distinction and the
node-type assertions a fixture cannot make (an over-permissive parser can accept a
widened word while building the *wrong node* for it) — stays in
`tests/strict_reserved_word_as_name.rs`. The halves acorn *does* accept are their own
fixtures: [contextual_keyword_name](../../../statements/labeled/contextual_keyword_name/),
[infer contextual_keyword_name](../../../types/infer/contextual_keyword_name/),
[heritage_yield](../../../types/interfaces/heritage_yield/).
