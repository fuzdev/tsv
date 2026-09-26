# parenthesized instantiation shift-assignment target - Svelte and prettier divergence

This fixture pins that a *parenthesized* instantiation expression parses as the target of a
`>>=` / `>>>=` / `/=` compound assignment and keeps its parens: `(f<T>) >>= c`,
`(f<T>) >>>= c`, over a member head (`(a.b<T>) >>= c`) and a nested type argument
(`(f<A<B>>) >>>= c`), nested where the assignment itself is a value (`x = (f<T>) >>= c`, a
call argument), and `(f<T>) /= c`. The `unformatted_ours_divide_assign_line_break` variant
writes that last one bare across a line break (`f<T>⏎/= c`), which tsc's grammar does take as
an instantiation assigned to; tsv joins the line, so it adds the pair.

## Why tsv differs

**From Svelte (acorn-typescript).** The parenthesized form is a `ParenthesizedExpression`,
so a `LeftHandSideExpression`, and only the `AssignmentTargetType` early error refuses it —
the static-semantic class tsv defers, the same one as `f<T> += c` in
[nonsimple_target](../nonsimple_target_svelte_divergence/). tsc's parser accepts every line.
acorn enforces the early error (`Assigning to rvalue`) on every line but the call argument:
inside a call argument list it skips the target check (the list may still be an arrow's
parameters, `maybeInArrowParameters`), so `fn(((f<T>) >>= c))` parses there — the other
lines are what make Svelte reject the component. See
[conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).

**From prettier.** Prettier strips every pair (`output_prettier.svelte`), and tsc's grammar
refuses the stripped `f<T> >>= c`: a `>>=` / `>>>=` is `>`-led, and tsc takes no type
argument list ahead of a `>`-led token (`canFollowTypeArgumentsInExpression` — the close
and the `>` would be ambiguous with a re-scanned `>>`), so the `<…>` falls back to a
comparison with no operand (TS1109 `Expression expected.`) — prettier's own second pass
included. A same-line `/=` refuses the list too, as the head of a regular expression literal
(TS1161 `Unterminated regular expression literal.`); past a line break tsc takes the list,
which is why the variant's `f<T>⏎/= c` parses and the joined `f<T> /= c` prettier prints from
it does not. acorn reads the stripped form as an instantiation target and refuses it only by
the same early error as before (so it still accepts the stripped call argument). tsv keeps
the pair, the one spelling tsc's grammar derives — the assignment members of the follower
set the pair around an instantiation already stays ahead of (`>`, `>=`, `>>`, `>>>`). See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Instantiation expression parens).

The bare spellings are `input_invalid_*` files of
[instantiation_operator_follow](../../../typescript_specific/generics/instantiation_operator_follow/),
rejected by both parsers; the bare call arguments, which acorn accepts, are
[instantiation_shift_assign_call_arg](../../../typescript_specific/generics/instantiation_shift_assign_call_arg_svelte_divergence/)
and [instantiation_divide_assign_call_arg](../../../typescript_specific/generics/instantiation_divide_assign_call_arg_svelte_divergence/).
