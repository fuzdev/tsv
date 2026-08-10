# shorthand initializer, name comment - Svelte divergence

A shorthand property carrying an initializer (`{ a = 1 }`) with a block comment
between the name and the `=`. The comment keeps its authored position: the
property's own doc prints the whole `a1 = 1`, so the list's element→comma seam
must anchor past the initializer, not at the key.

## Why tsv Differs

`PropertyDefinition : CoverInitializedName` **is** a grammar production — it
exists so `ObjectLiteral` can cover `ObjectAssignmentPattern` — and the rejection
is layered on top as a Static Semantics early error: *"It is a Syntax Error if any
source text is matched by this production"*
([ecma262 §13.2.5.1](https://tc39.es/ecma262/#sec-object-initializer-static-semantics-early-errors)).
Per tsv's permissive-parser stance the parser defers it to the diagnostics layer,
so the formatter keeps formatting everything well-formed — and prettier formats
it, which is the practical accept test.

**Acorn-typescript** (used by Svelte's parser) enforces the early error and rejects:

```typescript
({ a = 1 }); // ❌ acorn: "Shorthand property assignments are valid only in destructuring patterns"
```

acorn-typescript is tsv's AST-**shape** target but not its correctness oracle; for
validity the oracle is tsc, which reports its own error here. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

## Expected behavior

- **tsv parser**: parses, as a `Property` with `shorthand: true` whose `value` is
  an `AssignmentExpression` spanning the whole `a1 = 1` (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats to exactly this input — so the fixture also pins formatting
  agreement, which is the claim a Rust test cannot make against a live oracle

The **destructuring** spelling of the same binding (`const { a /* c */ = 1 } = x`)
is an ordinary pattern both parsers accept, and is covered by
[stripped_paren_interior_comment](../../destructuring/stripped_paren_interior_comment/);
this fixture is the only reachable spelling of the *literal* one, since every
valid `{ a = 1 }` is refined to a pattern before it is printed.
