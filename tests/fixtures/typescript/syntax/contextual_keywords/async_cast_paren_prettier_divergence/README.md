# async_cast_paren_prettier_divergence

The identifier `async` as the operand of an `as` / `satisfies` cast keeps its paren pair, at
every position:

```ts
// tsv                          // prettier (acorn rejects every line)
(async) as T;                   async as T;
a = (async) as T;               a = async as T;
fn((async) satisfies T);        fn(async satisfies T);
```

## Why tsv differs

◆prettier_bug ◆parser_compat. Bare, the cast puts a word right after `async` on its line,
and acorn-typescript — tsv's parse oracle and the parser Svelte itself uses — reads
`async <word>` as the head of an async arrow (`async as => …`) and then rejects the line
(`Unexpected token`). tsc and tsv read the cast either way, so prettier's own TypeScript
parser accepts its output and nothing on its side notices. The pair is the spelling every
parser reads alike.

Only the operand of a cast is in the class: a member (`async.b as T`) or an operator word
(`async in b`) puts no identifier after `async`, so both stay bare in both tools and are
carried as controls, unchanged in `output_prettier.svelte`. A cast chain wraps only its
innermost operand (`(async) as T as U`).

## Reason

◆prettier_bug. See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Statement-head cast parens) and
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index).
