# rest_optional_param_prettier_divergence

A rest parameter written with an optional `?` marker (`(...a?)`), in every
position acorn-typescript accepts it — a value-position arrow, an interface call
/ construct signature, and a function type:

```
const arrow = (...a?) => 1;
type Fn = (...a?: unknown[]) => void;
```

**Prettier** strips the `?` on every rest parameter (`(...a?)` → `(...a)`),
silently deleting a token the source wrote — the same information-loss shape as
its `import defer` phase-drop.

`(...a?)` is invalid TypeScript (tsc reports **TS1047** "a rest parameter cannot
be optional"), but that is a *deferred grammar-check* error: tsc's own parser
stores the `?` on the parameter node regardless of the `...` and reports TS1047
later during checking (`checker.ts` `checkGrammarParameterList`), exactly like the
already-deferred TS1051 (`set x(a?)`). Per tsv's permissive-parser stance, tsv
accepts the syntax and preserves the author's `?` rather than dropping it. Plain
rest (`...b`) is unaffected.

See [conformance_prettier.md §TypeScript](../../../../../docs/conformance_prettier.md#typescript).
