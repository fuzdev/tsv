# sloppy_legacy_octal_literal_prettier_divergence

The two leading-zero numeric literals ECMAScript keeps for **sloppy Script** code, in every
position a numeric literal reaches: `LegacyOctalIntegerLiteral` (`010`, `0777` — octal digits
only, read in base 8) and `NonOctalDecimalIntegerLiteral` (`08`, `089`, `08.5` — an `8` or `9`
in the run demotes the literal to decimal). The `goal` marker selects `Goal::Script`, and the
file's directive prologue holds no `"use strict"` — so the script stays sloppy and every
literal below is legal.

The positions covered: an initializer, an object-literal key, a computed member index, a
binary operand, a `TSLiteralType`, and a type-literal member key.

## What the directive lines pin

Prettier is no oracle here, so this file is also where the **positive** half of the
strictness rules lives — every line below turns strict mode *off* or leaves it off, and the
`010` beside it is the assertion. Each would be a parse error if the rule broke:

- **A template literal is not a directive candidate.** `` `use strict`; `` heads the file: it
  is not a `StringLiteral`, so it closes the program's directive prologue instead of opening a
  strict scope, and no string statement below it can reopen one.
- **A parenthesized string is never a directive**, at the head of a function body
  (`function paren()`) as much as anywhere else — the statement must open with a quote.
- **A Use Strict Directive is the exact code point sequence** `use strict`, with no escapes
  and no line continuations (ecma262 sec-directive-prologues), so the escaped spelling in
  `function esc()` is an ordinary directive and its body stays sloppy.
- **A function body's prologue is its own strictness scope.** `function fn()` declares
  `'use strict'`, and the mode is restored on the way out, so `const h = 010` after it is
  legal.
- **A class is strict by construction** (ecma262 sec-class-definitions) and that is restored
  on the way out too — `const i = 010` follows `class C {}`.
- **A plain nested block carries no directive prologue at all**, so the string statement at
  the head of the trailing block turns nothing on.

`unformatted_ours_bare_non_directive_strings.ts` writes the two non-directive string
statements — the one past the closed program prologue and the one heading the nested block —
*bare*, without the parens tsv adds back. It normalizes to `input.ts` under tsv alone
(prettier throws on the file), and it is the only spelling that pins those two positions: a
bare `'use strict'` there would be a directive if either rule broke, and the `010` beside it
would stop parsing.

## Why tsv Differs

Strict code disallows both forms by **production**, not by early error (ecma262
sec-strict-mode-of-ecmascript), and sloppy Script code is exactly where the productions live.
tsv's Script goal is spec-conforming, so it parses them and formats them **verbatim** — the
number printer normalizes nothing here, and prettier's own printer would not either.

Prettier's `typescript` parser (typescript-estree) **rejects** them, under every filepath and
every option, because tsc's scanner has no sloppy mode for numeric literals — it raises the
error unconditionally:

```
Octal literals are not allowed. Use the syntax '0o10'.
Decimals with leading zeros are not allowed.
```

so prettier cannot serve as a formatting oracle here — there is no `output_prettier.*`.
`prettier_rejects.txt` pins the first error the file draws; rule F6 live-verifies that prettier
still rejects the input with that message, failing loudly if prettier is ever relaxed or the
error morphs.

**Acorn** accepts every case at `sourceType: 'script'` and rejects every one at
`'module'`, so `expected.json` is the ordinary canonical AST and this is not a Svelte
divergence. tsv agrees on both sides: a Svelte `<script>` is always a module, so nothing here
is reachable from a `.svelte` file.

See [conformance_prettier_ts.md §Sloppy-script literals prettier refuses](../../../../../docs/conformance_prettier_ts.md#sloppy-script-literals-prettier-refuses),
and the frame's decision rules in
[conformance_prettier.md](../../../../../docs/conformance_prettier.md).

## The rejections

Five `input_invalid_*` files pin the boundaries both parsers hold, in every mode:

- `07.5` — a `LegacyOctalIntegerLiteral` admits no fraction, so the `.` is left for the next
  token and the statement does not close
- `0777e2` — nor an exponent, so `e2` is an identifier abutting a number
- `010n` — nor a BigInt suffix
- `0_1`, `08_1` — neither `LegacyOctalIntegerLiteral` (`0_1`) nor
  `NonOctalDecimalIntegerLiteral` (`08_1`) carries a `[Sep]` parameter, so a separator in the integer run is a syntax error (one in a *fraction* is
  ordinary, which is why `08.5` is a positive above)

The Module-goal half — the same forms rejected because a module is always strict — is
[literals/numeric/leading_zero_invalid](../../expressions/literals/numeric/leading_zero_invalid/),
and the directive that turns a Script strict is [use_strict_prologue](../use_strict_prologue/).
