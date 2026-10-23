# non_null_callee_long_prettier_divergence

A member chain whose call is made on a non-null callee (`obj.a().b().fnnn!(…)`). tsv lays the
chain out as it lays out the same chain without the `!`: the call is one of the chain's links, so
the chain breaks one call per line wherever its twin without the `!` does — past the print width,
or at any width when it has more than two calls and an argument that is not trivial
(`obj.a().b().fnnn!((q) => q)`, `printMemberChain`'s expansion rule). Prettier ends the chain at
the `!`.

- **tsv**: `const v2 = obj⏎\t.a()⏎\t.b()⏎\t.fnnn!(z…);` — the same as `obj.a().b().fnnn(z…)`,
  where both formatters break the chain
- **Prettier**: `const v2 = obj.a().b().fnnn!(⏎\tz…⏎);` — the chain stays on one line and the
  argument list breaks

Mid-chain the same holds: tsv breaks `obj.a().b().fnnn!(x).c().d(z…)` one call per line, as it
does (and prettier does) without the `!`, where prettier makes `obj.a().b().fnnn!(x)` the
chain's head (`obj.a().b().fnnn!(x)⏎\t.c()⏎\t.d(z…)`); a `!` on a curried call mid-chain
(`obj.a().b().fnnn(x)!(y).c().d(z…)`) likewise.

A `!` on the callee of the chain's last, outermost call when that callee is itself a call
(`obj.a().b().fnnn(x)!(z…)`) is not this divergence: prettier ends the chain at the innermost
call with or without the `!`, and tsv lays it out as the curried call without the `!` too — see
[curried_call_non_null_callee_long](../curried_call_non_null_callee_long/).

Prettier's `isMemberish` (`utilities/is-memberish.js`) admits a member expression and a bind
expression, not a `TSNonNullExpression`, so a call on a non-null callee skips `printMemberChain`
and prints `[callee, args]`, and the chain's walk (`printMemberChain`'s `rec`) stops at such a
call, printing it whole as the head. The `!` asserts a type and nothing else; the call it sits
in is a link of the chain either way.

At exactly 100 the chain fits on one line in both formatters. The fixture pins the declarator
(the 100/101 pair and the no-`!` twin), the mid-chain call (beside its twin), the width-free
cells (beside their twins), a commented empty-bodied block arrow at 101 (beside its twin), the
mid-chain curried `!` (beside its twin), an assignment, a return, an argument and a Svelte
template expression.

## Reason

Design choice: a type-only `!` does not change a chain's layout. Prettier's own pair shows the
inconsistency — `obj.a().b().fnnn(z…)` breaks the chain, `obj.a().b().fnnn!(z…)` keeps it on
one line.

See [conformance_prettier_ts.md §TypeScript](../../../../../../../docs/conformance_prettier_ts.md#typescript) (Non-null callee in a member chain).
