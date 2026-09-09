# let_reference_for_head_prettier_divergence

`let` in a **for-head**, in every spelling where no binding follows it. `Identifier :
IdentifierName but not ReservedWord` admits `let` — it is no `ReservedWord` — so a `let`
that heads no binding is an ordinary `IdentifierReference`, and the head it opens is an
expression rather than a `LexicalDeclaration`. What separates the two readings is a
lookahead, and ecma262 spells every one of them out:

- `ForStatement : for ( [lookahead ∉ { let [ }] Expression? ; … )` — so `for (let; ;)`,
  `for (let = 3; ;)` and `for (let instanceof o; ;)` are expression inits
- `ForInOfStatement`'s in-form carries `[lookahead ≠ let []` — a for-in left may start
  with `let` whatever follows, so `for (let in o)` and `for (let.x in o)` are lefts
- its of-form carries `[lookahead ∉ { let, async of }]`, which is why the `of` spellings
  are the `input_invalid_*` files below

The `goal` marker selects `Goal::Script`: acorn admits `let` as a reference there and bars
it under `sourceType: 'module'` (a strict-mode early error), so this file's `expected.json`
is the ordinary canonical AST. tsv defers that early error and reads the same shapes at
either goal — the sibling
[statement_head_paren](../../statements/expression/statement_head_paren_svelte_divergence/)
pins the module half of the same family.

## What the parens pin

A for-in / for-of **left** whose leftmost token is `let` keeps a paren around it, and the
printer recomputes that from the AST rather than preserving the author's: the of-form's own
`[lookahead ∉ { let }]` makes a bare `for (let of x)` a syntax error however the head
continues, and prettier draws one line across both head forms
(`startsWithNoLookaheadToken` finds any enclosing for-in/of), so tsv does too. The C-style
init takes the narrower restriction — only `let [` — and stays bare.

`unformatted_ours_bare_let_head.ts` writes the two for-in lefts **without** the paren, the
spelling the in-form admits. It normalizes to `input.ts` under tsv alone, and it is what
pins the reading: a parser that committed `for (let` to a declaration would stop at the
`in`.

## Why tsv Differs

tsc's parser commits `for (let` to a declaration on the keyword alone
(`parseForOrForInOrForOfStatement` tests `token() === LetKeyword` with no lookahead) and
then demands a binding, so prettier's `typescript` parser (typescript-estree) **rejects**
four of the five bare no-binding heads this fixture covers — with a different message each:

```
for (let in o)            Only a single variable declaration is allowed in a 'for...in' statement.
for (let.x in o)          Variable declaration expected.
for (let = 3; ;)          Variable declaration expected.
for (let instanceof o; ;) 'instanceof' is not allowed as a variable declaration name.
```

(The two for-in lefts are bare in `unformatted_ours_bare_let_head.ts`; `input.ts` writes
them parenthesized, and those forms prettier does format — see §What the parens pin.)

The fifth head, `for (let; ;)`, prettier **parses**: tsc builds the `ForStatement` with a
`VariableDeclarationList` holding **zero** declarations and reports an empty
`parseDiagnostics`, and prettier's printer then throws on that empty list —
`InvalidDocError: Unexpected doc 'undefined', Expected it to be 'string' or 'object'`. A
prettier bug rather than a parse rejection, but the same outcome here: prettier is no
formatting oracle for this file, so there is no `output_prettier.*`.

`prettier_rejects.txt` pins the first error the file draws — `Variable declaration
expected.`, raised at the `for (let = 3; ;)` head during the parse, ahead of any printing,
so the printer bug never gets to speak on this input. Rule F6 live-verifies that prettier
still rejects the input with that message.

Prettier's own other routes agree with tsv: under `babel` and `acorn` it formats every line
of `input.ts`, and it prints the bare for-in lefts with exactly the parens tsv adds
(`for (let in o)` → `for ((let) in o)`). Four of the five heads are the `typescript`
route's *parser*, then; only `for (let; ;)` reaches its printer at all.

See [conformance_prettier_ts.md §Prettier rejects valid input](../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input)
— which carries both the per-head messages and the `◆prettier_bug` note on `for (let; ;)`
— and the frame's decision rules in
[conformance_prettier.md](../../../../../docs/conformance_prettier.md).

## The rejections

Three `input_invalid_*` files pin the of-form's `[lookahead ∉ { let }]`, which both parsers
hold at either goal:

- `for (let of x)` — the head reads as a declaration of the binding `of`, which then has no
  iterable
- `for (let.x of y)` — the expression reading is barred outright by the lookahead; only
  `for ((let).x of y)` says it
- `for await (let of x)` — for-await carries the same restriction, spelled
  `[lookahead ≠ let]`
