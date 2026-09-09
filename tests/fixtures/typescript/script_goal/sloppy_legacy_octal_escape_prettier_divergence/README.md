# sloppy_legacy_octal_escape_prettier_divergence

The two legacy string escapes ECMAScript keeps for **sloppy** code:
`LegacyOctalEscapeSequence` (`\7`, `\101`, `\00`, `\377` — one to three octal digits read in
base 8) and `NonOctalDecimalEscapeSequence` (`\8`, `\9` — the digit itself). The `goal` marker
selects `Goal::Script`, and the file's directive prologue holds no `"use strict"`, so the
script stays sloppy and every escape below is legal.

The positions covered: a directive, an initializer, an object-literal key, a computed member
index, a binary operand, a `TSLiteralType`, a type-literal member key, and an import type's
specifier. Every string literal in the grammar routes through one consumption seam, so the
gate is stated once and these positions are what prove it.

## What the file pins beyond acceptance

- **A directive is a directive whatever its content.** `'\7';` heads the file: it is an
  unparenthesized string-literal expression statement, so it is a directive (the wire carries
  its raw text in `directive`) — and because a Use Strict Directive is the exact code point
  sequence `use strict`, it turns nothing on and the prologue stays open.
- **`\0` is the legacy form only when a decimal digit follows it.** `'\08'` is a legacy
  escape; a bare `'\0'` is the NUL escape and stays legal in strict code, which
  [string/legacy_escape_invalid](../../expressions/literals/string/legacy_escape_invalid/)
  pins from the other side.
- **A legacy octal escape reads at most three digits, and three only when the first is
  `0`-`3`** (`LegacyOctalEscapeSequence :: ZeroToThree OctalDigit OctalDigit`), so `'\400'`
  is `\40` then a literal `0` and `'\412'` is `\41` then a literal `2`. `expected.json`
  carries the decoded values, so these two lines are the pin on the escape decoder's digit
  count as much as on the parse.

`unformatted_ours_double_quotes.ts` writes the whole file with double quotes. It normalizes to
`input.ts` under tsv alone (prettier throws on the file), and it pins that quote normalization
still runs over a literal carrying a legacy escape — tsv requotes the delimiters and copies
the escape bytes through verbatim.

## Why tsv Differs

Strict code disallows both forms by **production**, not by early error (ecma262
sec-strict-mode-of-ecmascript, and the Annex C restatement), and sloppy Script code is exactly
where the productions live. tsv's Script goal is spec-conforming, so it parses them and prints
them **verbatim** — no escape is ever rewritten.

Prettier's `typescript` parser **rejects** them, under every filepath and every option, because
tsc's scanner has no sloppy mode for string escapes — it raises the error unconditionally:

```
Octal escape sequences are not allowed. Use the syntax '\x07'.
Escape sequence '\8' is not allowed.
```

so prettier cannot serve as a formatting oracle here — there is no `output_prettier.*`.
`prettier_rejects.txt` pins the first error the file draws; rule F6 live-verifies that prettier
still rejects the input with that message, failing loudly if prettier is ever relaxed or the
error morphs.

**Acorn** accepts every case at `sourceType: 'script'` and rejects every one at `'module'`, so
`expected.json` is the ordinary canonical AST and this is not a Svelte divergence. tsv agrees
on both sides: a Svelte `<script>` is always a module, so nothing here is reachable from a
`.svelte` file.

See [conformance_prettier_ts.md §Sloppy-script literals prettier refuses](../../../../../docs/conformance_prettier_ts.md#sloppy-script-literals-prettier-refuses),
and the frame's decision rules in
[conformance_prettier.md](../../../../../docs/conformance_prettier.md).

## The rejections

Four `input_invalid_*` files pin where strict mode reaches, each rejected by both parsers:

- `input_invalid_use_strict.ts` — a `'use strict'` directive turns the rest of the script
  strict, so the `'\7'` below it is a syntax error
- `input_invalid_retroactive_prologue.ts` — `'\7'; 'use strict';`: the directive that turns
  strict mode on comes **after** the literal it condemns. The prologue's earlier literals are
  graded by a verdict reached later (ecma262 sec-string-literals: implementations must enforce
  the strict rules for such literals), so this is a syntax error too
- `input_invalid_retroactive_function_prologue.ts` — the same trap in a function body's own
  prologue, inside a sloppy script
- `input_invalid_class_body.ts` — a class body is strict by construction (ecma262
  sec-class-definitions), so a legacy escape in a class member key is rejected even in a
  sloppy script

The Module-goal half — the same forms rejected because a module is always strict — is
[string/legacy_escape_invalid](../../expressions/literals/string/legacy_escape_invalid/), and
the directive that turns a Script strict is [use_strict_prologue](../use_strict_prologue/).
