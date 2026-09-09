# A hashbang ahead of a `"use strict"` prologue — Svelte Divergence

A `HashbangComment` is a comment (ecma262 sec-hashbang): it is not a statement, so the
directive prologue is still the script's first statements and `'use strict'` on the
second line turns strict mode on for the whole script. The `010` below it is a
`LegacyOctalIntegerLiteral` in strict code, a syntax error.

**tsv** rejects (`Leading-zero literals are not allowed in strict mode`). V8 agrees —
a real `#!` file with this content throws `SyntaxError: Octal literals are not allowed
in strict mode` — and prettier's `typescript` parser rejects `010` under every
directive.

## Why tsv Differs

**acorn accepts.** Its strict-directive detector is a regex pre-scan over the raw
source from the body's first position (`strictDirective`), and that scan skips
whitespace and ordinary comments but not the hashbang line, so the directive is never
seen and the script stays sloppy. tsv reads the prologue from the parsed statements
after the hashbang has been consumed as trivia, the same way it reads one behind any
other comment.

Only the *directive* is at stake: acorn and tsv agree the hashbang is trivia (pinned by
[syntax/comments/hashbang](../../syntax/comments/hashbang/)) and agree on
`'use strict'` with no hashbang ahead of it
([use_strict_prologue](../use_strict_prologue/)).

See [conformance_svelte.md §TypeScript Corrections](../../../../../docs/conformance_svelte.md#typescript-corrections).
