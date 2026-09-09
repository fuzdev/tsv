# nonsimple_params_directive_svelte_prettier_divergence

A `"use strict"` directive in a function whose parameter list is **not simple** — a
default, a rest, or a pattern (`function fn(a = 010) { 'use strict'; }`) — with a legacy
octal literal in the parameter default. What the fixture pins is the **scope the
parameters parse under**: the outer (sloppy) one, so the default's `010` is legal, while
the same literal in the body would be rejected
([use_strict_prologue](../use_strict_prologue/input_invalid_nonsimple_params_directive_body_octal.ts)
pins that half, where both parsers reject). The function, arrow and method spellings all
take the reading.

## Why tsv differs from acorn

ecma262 makes a Use Strict Directive in a function with non-simple parameters a Syntax
Error (sec-function-definitions-static-semantics-early-errors; the rule exists precisely
because the parameters have already been parsed under the outer mode). acorn enforces
it (`Illegal 'use strict' directive in function with non-simple parameter list`), so
`expected_svelte.json` is the error marker. tsv **defers** it — a deferred early error
like the rest of the strict-mode list in
[checklist_typescript.md §Early errors that still parse](../../../../../docs/checklist_typescript.md)
— and honors the directive for the body. The parameters are parsed under the outer mode
on both sides; the divergence is only whether the directive is then refused.

## Why prettier rejects

prettier's `typescript` parser is tsc, whose scanner refuses a leading-zero literal in
every mode (`Octal literals are not allowed`), so there is no prettier oracle for the
input and `prettier_rejects.txt` pins the refusal.

See [conformance_svelte.md §TypeScript Corrections](../../../../../docs/conformance_svelte.md#typescript-corrections)
and [conformance_prettier_ts.md §Sloppy-script literals prettier refuses](../../../../../docs/conformance_prettier_ts.md#sloppy-script-literals-prettier-refuses).
