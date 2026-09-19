# statement_cast_paren_prettier_divergence

A word that tsc reads as the head of a declaration keeps its paren pair when it is the
operand of an `as` / `satisfies` cast heading an expression statement:

```ts
// tsv                        // prettier (tsc rejects every line)
(namespace) as T;             namespace as T;
(namespace) satisfies T;      namespace satisfies T;
(accessor) as T;              accessor as T;
(readonly) satisfies T;       readonly satisfies T;
for ((using) as T; ;) {}      for (using as T; ;) {}
```

## Why tsv differs

◆prettier_bug. Prettier keeps the pair for the words on its own list (`type`, `module`,
`interface`, `let`, `using`, … — see [statement_cast_paren](../statement_cast_paren/)) and
strips it for these, and every stripped line is a declaration head to tsc:

- `namespace as T;` — a `namespace` declaration missing its body (`'{' expected.`). tsv's own
  parser commits to the same reading, so tsv could not reparse this output either;
- `accessor as T;` / `readonly satisfies T;` — a modifier ahead of a declaration
  (`Declaration or statement expected.`);
- `for (using as T; ;)` — a `using` declaration binding a name `as` (`',' expected.`), and
  the same for acorn at `ecmaVersion: 'latest'`. Prettier's pair rule stops at an expression
  STATEMENT, and a `for` init is not one; the `using` rule itself matches at statement
  position.

acorn at the canonical parsers' ES2025 accepts every bare line, which is why no reparse of
the output sees the loss. The pair is the spelling every reader agrees on.

## Reason

◆prettier_bug. See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Statement-head cast parens) and
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index).
