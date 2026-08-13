# cast target with a default, in a for-of/for-in head - Svelte divergence

A destructuring default whose target is a type assertion (`[(c as T) = 1]`), used as a
no-declaration `for`-of / `for`-in head. The same patterns outside a for-head are
ordinary and live in the sibling
[cast_target_destructure_default](../cast_target_destructure_default/); this fixture
exists only because the for-head position is where acorn-typescript rejects them.

## Why tsv Differs

Both readings run the same conversion, but from opposite `isBinding` sides.
acorn-typescript converts the inner `=` in *assignment* mode (a cast target is legal
there, and since 1.0.13 the assertion node is **preserved** rather than unwrapped), then
the enclosing for-head converts the whole pattern again in *binding* mode — where a cast
target raises "Unexpected type cast in parameter position". Before 1.0.13 the first pass
erased the assertion, so the second pass never saw one and the head parsed; keeping the
node is what put it back in the second pass's way.

```typescript
for ([(c as T) = 1] of arr) {
} // ❌ acorn-typescript 1.0.13 (accepted at 1.0.12)
```

**tsc accepts** all four forms with no parse diagnostic, and prettier formats them —
which is tsv's accept test. So tsv converts the inner `=` under assignment rules even
inside a for-head, and the assertion survives. acorn-typescript is tsv's AST-**shape**
target, not its correctness oracle. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

The **bare** (default-less) cast target in a for-head is a different case and is *not*
here: `for ((x as T) of arr)` is rejected by tsv too, pinned by
[cast_target](../cast_target/)'s `input_invalid_cast_for_of.svelte`.

## Expected behavior

- **tsv parser**: parses all four, keeping the assertion as the `AssignmentPattern`
  `left` (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats all four, and to exactly this input
