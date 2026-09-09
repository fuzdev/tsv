# let_reference_statement_position_prettier_divergence

`let` as the **whole body** of a single-statement position — a label, an `if` arm, a
`while` / `for` body. `LabelledItem : Statement | FunctionDeclaration` and every loop and
`if` arm takes a `Statement`, and a `LexicalDeclaration` is not one, so `let` there can only
be the `IdentifierReference` that `Identifier : IdentifierName but not ReservedWord` admits
(`let` is no `ReservedWord`). Nothing may follow it in the same statement, and
`ExpressionStatement`'s `[lookahead ∉ { …, let [ }]` is the one spelling that could — hence
the `input_invalid_*` files below.

The `goal` marker selects `Goal::Script`: acorn admits `let` as a reference there and bars
it under `sourceType: 'module'` (a strict-mode early error tsv defers), so `expected.json`
is the ordinary canonical AST.

## What the variant pins

`input.ts` writes each body with its own `;`, which both formatters keep.
`unformatted_ours_asi.ts` writes the same statements with the semicolon left to
**automatic semicolon insertion** — the line terminator after `let` is what closes the
statement — and only tsv normalizes it back to `input.ts`.
`audit_signature_asi.txt` records prettier's own chain from that variant: it is the marker
of last resort, written here because prettier's output is a fixed point **tsv cannot
format** — four of its lines are not ECMAScript, so no `output_prettier.*` or `*_variant_*`
can hold them.

## Why tsv Differs

Prettier's `typescript` parser (typescript-estree) is tsc's, and tsc commits `let` to a
declaration on the keyword alone wherever a statement begins. So it reads each ASI spelling
as one labelled or nested **declaration** and prints it welded:

```typescript
L: let          →  L: let a = 1;
a = 1;

while (a) let   →  while (a) let c = 1;
c = 1;

for (;;) let    →  for (;;) let d = 1;
d = 1;

if (a) let      →  if (a) let {};
{
}
```

Each of those is two statements in the source and one in the output, so the line break's
meaning is lost — and none of the four results is ECMAScript: `L: let a = 1;`,
`while (a) let c = 1;`, `for (;;) let d = 1;` and `if (a) let {};` are all syntax errors
under acorn and under the spec, since a `LexicalDeclaration` is not a `Statement`. The
input's fifth spelling — the `if (a) let;` / `else b;` pair — is the only one prettier
leaves alone, having no following statement to weld onto it. tsv reads the ASI the grammar
requires and keeps the two statements two.

Prettier's own other routes agree with tsv: under `babel` and `acorn` it prints
`L: let;` + `a = 1;` and `if (a) let;` + `{}` exactly as tsv does. The divergence is the
`typescript` route's parser, not prettier's printer.

See [conformance_prettier_ts.md §Statement-position `let` prettier reads as a declaration](../../../../../docs/conformance_prettier_ts.md#statement-position-let-prettier-reads-as-a-declaration),
and the frame's decision rules in
[conformance_prettier.md](../../../../../docs/conformance_prettier.md).

## The rejections

Three `input_invalid_*` files pin the boundary both parsers hold: a declaration is barred in
these positions, so the `let [` lookahead — the one spelling that survives it — is a syntax
error rather than a second reading.

- `L: let [a] = b;` — the label body
- `if (a) let [a] = b;` — the `if` arm
- `while (a) let x = 1;` — the loop body, where the binding follows on the same line, so no
  semicolon may be inserted and neither reading closes
