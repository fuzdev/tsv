# Decorator type arguments with no call, on the decorated construct's own line (`@g<number> class A {}`) — Svelte Divergence

A decorator's type arguments need no trailing call — `@a.b<number>` is a
`TSInstantiationExpression`, pinned by the sibling
[call_type_arguments](../call_type_arguments/). That fixture writes it the way both
formatters do, with the decorated construct on the **next line**. Written on the
**same line** the same expression is not an instantiation at all, and this fixture
pins the boundary.

## Why tsv Differs

The `>` closing a type-argument list is the same character as the relational
operator, so every parser decides between them by what follows. The rule is
acorn's own (`tokenCanStartExpression && !hasPrecedingLineBreak`): a follow token
that can start an expression makes the `<` a comparison — unless a line terminator
intervenes, where ASI ends the statement and leaves the `<…>` an instantiation.
`class` starts an expression (`const C = class {}`), so on one line `@g<number>
class A {}` reads as `@g` followed by `<` — and a decorator's expression cannot
take a binary operator, so the parse fails.

**tsc's parser rejects it too** — `TS1146 Declaration expected.`, a real
`parseDiagnostics` entry, not a checker error — and so does prettier (its
`typescript` parser is tsc's). The same holds for a same-line class member
(`class A { @dec<T> m() {} }` → TS1146) and a same-line parameter decorator. Move
the construct to the next line and all three accept.

```typescript
@g<number> class A {} // ❌ tsv, tsc (TS1146), prettier — ✅ acorn-typescript
@g<number>
class A {} // ✅ everywhere
```

acorn-typescript accepts the same-line form because its decorator path reads the
type arguments directly (`parseMaybeDecoratorArguments`), never reaching the
follow-token test it applies to an ordinary instantiation expression — the same
shape as its missing `hasPrecedingLineBreak` guard on a tuple element's `?` (see
[tuple_optional_marker_line_break](../../../types/tuple_optional_marker_line_break_svelte_divergence/)).
tsv applies the test uniformly, matching tsc and prettier.

Because the canonical parser accepts this input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject). This
`tsv_rejects.txt` fixture pins the divergence from the other side: tsv rejects
(`tsv_rejects.txt` substring), while `expected_svelte.json` proves acorn still
accepts.

**Upstream**: @sveltejs/acorn-typescript — `parseMaybeDecoratorArguments` commits
to a type-argument reading without the follow-token / line-break test its own
`tsParseTypeArgumentsInExpression` callers apply.

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Same-line decorator type arguments).
