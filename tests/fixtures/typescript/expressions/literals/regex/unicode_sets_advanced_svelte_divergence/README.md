# RegExp v-flag set operations — tsv defers the invalid-regex early error

## Why tsv Diverges

**Not because tsv implements the `v` flag — it does not.** The regex body and flag run are
**opaque** to tsv's lexer: it scans only far enough to find the literal's closing `/` and never
consults the Pattern grammar (see
[checklist_typescript.md §Regular Expressions](../../../../../../../docs/checklist_typescript.md#regular-expressions)).
Every regex below is accepted the same way any byte sequence between two slashes is accepted.

Exactly **one** construct here actually diverges, and the reason inverts what it looks like:

- `/[a-z--[aeiou]]/v` is **invalid ECMAScript**. Per ecma262 §sec-patterns,
  `ClassSubtraction :: ClassSetOperand -- ClassSetOperand` and
  `ClassSetOperand :: NestedClass | ClassStringDisjunction | ClassSetCharacter` — a
  **`ClassSetRange` is not a `ClassSetOperand`**, so a range may not be the left operand of `--`.
  V8 agrees: `new RegExp('[a-z--[aeiou]]', 'v')` throws. The valid spelling nests the range as an
  operand: `/[[a-z]--[aeiou]]/v`.
- **Svelte's parser is therefore correct to reject it.** tsv accepts it only by deferring the
  `IsValidRegularExpressionLiteral` early error, exactly as it defers every other early error (see
  [checklist_typescript.md §Out of Scope](../../../../../../../docs/checklist_typescript.md#out-of-scope)).
  The file fails on this first regex, which is why the whole fixture reads as a divergence.

The other four constructs are **not divergences at all** — Svelte's parser accepts each on its own
against the pinned oracle: set intersection, nested classes, string literals, and the complex
nested form. They remain in the input only because the fixture predates this finding.

## Status

- **tsv parser**: accepts — regex bodies are never parsed (opacity), *not* `v`-flag support
- **Svelte/acorn**: `Invalid regular expression: Unterminated character class` — **correct**; acorn
  supports the `v` flag and the other set operations, and rejects only the invalid subtraction
- **Prettier**: formats it, but its parser is regex-opaque too, so that is not evidence of validity

## References

- [TC39 RegExp v flag proposal](https://github.com/tc39/proposal-regexp-v-flag)
- ecma262 §sec-patterns — `ClassSubtraction` / `ClassSetOperand` / `ClassSetRange`

See [conformance_svelte.md](../../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.
