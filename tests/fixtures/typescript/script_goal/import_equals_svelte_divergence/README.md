# import-equals at `Goal::Script` - Svelte divergence

This fixture pins that a TypeScript **import-equals** declaration parses in a
standalone strict *script* (the `goal` marker selects `Goal::Script`): the namespace
alias (`import x = foo.bar`), the `require` form (`import y = require('y')`), and a
binding named `await` (`import await = foo.await`).

## Why tsv Differs

`import x = A.B` / `import x = require('y')` is **not** an ES `ImportDeclaration`. It
predates ES modules, it is how a script or a namespace aliases, and nothing about it is
a `ModuleItem` — so the "`import` is only allowed in a module" rule does not reach it.
tsc agrees, and asserts it in its own corpus:
`tests/cases/conformance/externalModules/topLevelAwait.2.ts` is exactly this shape,
commented *"await allowed in import=namespace when not a module"*, and compiles with no
`.errors.txt` baseline. Prettier formats all three to precisely this input.

The `await` binding follows for free rather than needing a rule of its own: at `Script`
goal `await` is an ordinary identifier, so once import-equals is reachable the binding
parses like any other name. At `Goal::Module` it is correctly rejected — `await` is
reserved at a module's top level — which the sibling `module_only_constructs_invalid`
fixture's ES-import cases complement from the other side.

**Acorn-typescript** rejects all three:

```typescript
import x = foo.bar; // ❌ acorn: "'import' and 'export' may appear only with 'sourceType: module'"
```

That message is the tell: it is *base acorn's* ES-grammar check firing before the
TypeScript plugin ever sees the statement, not a TypeScript judgement — a slip rather
than a choice. acorn-typescript is tsv's AST-**shape** target but not its correctness
oracle; for validity the oracle is tsc. See
[conformance_svelte.md §TypeScript Corrections](../../../../../docs/conformance_svelte.md#typescript-corrections).

## Expected behavior

- **tsv parser**: parses all three as `TSImportEqualsDeclaration` (see `expected_ours.json`)
- **Svelte/acorn**: fails to parse (see `expected_svelte.json` with `{"error": "failed to parse"}`)
- **prettier**: formats all three, and to exactly this input

Every genuine ES import shape still rejects at this goal — `import x from 'y'`,
`import * as ns from 'y'`, `import { a } from 'y'`, and the side-effect `import 'y'` —
which is what `../module_only_constructs_invalid/` pins.
