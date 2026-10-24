# assertion_left_of_comparison_chain

An `as` inside a comparison chain that, once blanked, looks like a type-argument list:
`f(a < b, c as any > (d))` is two comparisons, and its strip `f(a < b, c        > (d))` is
right as JS, ts-blank-space's and Node's bytes alike. Re-read as TypeScript, the output's
`a < b, c > (d)` is the generic call `a<b, c>(d)`, so stripping it again blanks the list;
the idempotence check excuses that change, and only that one.
