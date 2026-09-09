# A legacy octal literal in a class's decorator list — Svelte Divergence

All parts of a `ClassDeclaration` or `ClassExpression` are strict mode code (ecma262
sec-strict-mode-code), and the decorators proposal places the `DecoratorList`
inside those productions (`ClassDeclaration : DecoratorList? class BindingIdentifier
ClassTail`). So a `LegacyOctalIntegerLiteral` in a decorator argument is a syntax error
even in a sloppy script, exactly as one in the heritage clause is
(`class C extends (010) {}`, pinned by
[use_strict_prologue](../use_strict_prologue/input_invalid_class_heritage_octal.ts)).

**tsv** rejects (`Leading-zero literals are not allowed in strict mode`): the class's
strictness scope opens at the first decorator, not at the `class` keyword. prettier's
`typescript` parser rejects `010` everywhere.

## Why tsv Differs

**acorn-typescript accepts.** It parses the decorator list in the enclosing scope and
turns `strict` on only inside `parseClass`, so a decorator's expression is graded
under the outer (sloppy) mode. A decorator *inside* the body — `@d(010) m() {}` — is
rejected on both sides, since the body is past the point acorn flips the flag.

See [conformance_svelte.md §TypeScript Corrections](../../../../../docs/conformance_svelte.md#typescript-corrections).
