# ambient namespace function body - Svelte divergence

A plain `function f() {}` **with a body** inside a `declare namespace` / `declare
module` body, alongside the `export function` spelling it must match.

## Why tsv Differs

Such a function carries no `declare` keyword of its own, so grammatically it is an
ordinary function *declaration with a body*. Its ambient-context violation — tsc
**TS1183** "An implementation cannot be declared in ambient contexts" — is a
static-semantic early error, which tsv defers to the diagnostics layer per its
permissive-parser stance so the formatter keeps formatting everything well-formed.
prettier formats it, which is the accept test.

**Acorn-typescript** (used by Svelte's parser) enforces TS1183 and rejects:

```typescript
declare namespace N {
	function f() {} // ❌ acorn: "An implementation cannot be declared in ambient contexts."
}
```

acorn-typescript is tsv's AST-**shape** target but not its correctness oracle; for
validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

## Expected behavior

- **tsv parser**: a `FunctionDeclaration` whose `body` is a `BlockStatement` — *not* a
  bodiless `TSDeclareFunction`, so the body survives the round-trip rather than being
  dropped as a signature (see `expected_ours.json`). The `module` keyword spelling and
  the `export function` form yield the same node, which is the consistency target
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats to exactly this input

**Two boundaries, deliberately included.** A *bodiless* signature in the same position
stays a `TSDeclareFunction` — that is the ordinary
[declare](../declare/) fixture, not a divergence, since acorn accepts it. And a
**top-level** `declare function f() {}` HAS the `declare` keyword, which grammatically
forces a bodiless signature: prettier rejects a body there and so does tsv, pinned here
by `input_invalid_top_level_declare_body.svelte`.
