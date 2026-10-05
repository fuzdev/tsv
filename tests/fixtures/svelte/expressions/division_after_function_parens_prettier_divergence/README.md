# division_after_function_parens_prettier_divergence

tsv never prints a function or class body's `}` directly before a `/` operator. A function or
class expression that is the LEFT operand of `/` keeps its paren pair, in every position, and
so does one that left operand ends on — under a prefix operator, as the right operand of an
operator that binds at least as tightly, under an angle-bracket assertion. tsv supplies the
pair where the author wrote the operand bare (`unformatted_ours_bare.svelte`):

```svelte
<!-- tsv -->                              <!-- prettier -->
<div>{(function () {}) / 2}</div>          <div>{function () {} / 2}</div>
<div>{c ? x : (function () {}) / 2}</div>  <div>{c ? x : function () {} / 2}</div>
<div>{x + (function () {}) / 2}</div>      <div>{x + function () {} / 2}</div>
<div>{!(function () {}) / 2}</div>         <div>{!function () {} / 2}</div>
```

## Why tsv differs

◆prettier_bug ◆parser_compat. acorn — Svelte's parser for every `<script>` and template
expression — decides each `/` by a token-context heuristic, not the grammar: where its
tokenizer takes the `function` or `class` keyword for a statement's, it reads the `/` after
the body's `}` as a regex. Among those positions: a template expression's first token (the
text tag, `data-attr`, the spread, `{#if}`, `{#each}`); an anonymous `async function` and a
decorated class, after any token (`typeof async function () {} / 2`, `!@dec class {} / 2`); a
`yield` operand; and a function after a conditional's `:` outside any `(…)`, object literal or
`${…}` — in a template expression and at statement level in the `<script>` alike, inside an
array too (`[c ? y : class {} / 2]`). There prettier's output does not parse: Svelte rejects it
(`Invalid regular expression flag`). The `input_invalid_*` files are those bare forms, one
position each — the leading function and class, `async` behind an operator, the conditional's
alternate at the top level and in an array, a `yield` operand, the spread's value and the
`{#each}` key. tsv rejects them too: its scan for a template expression's end reads the `/`
after a `}` as acorn's tokenizer does, and in each the misread regex runs on past the
expression's own `}` to the `/` of the closing tag. (That scan is not the parse: a bare form
whose misread regex ends inside the expression — `{function () {} / 2 / 3}` — tsv still
accepts where Svelte rejects it, a tracked over-acceptance.)

◆design_choice. Elsewhere (`x + function () {} / 2`, `c ? class {} / 2 : x`,
`fn(c ? x : function () {} / 2)`, `` `${function () {} / 2}` ``, `!function () {} / 2`,
`a / function () {} / 2`, a named or generator `async function`) the bare form parses, and
tsv keeps the pair anyway: one rule over every function and class expression, rather than a
printer modeling acorn's context stack, where a single miss is a component Svelte rejects.

With no `/` printed after the body the operand stays bare in both tools — as the right
operand (`2 / function () {}`), ahead of another operator (`function () {} * 2`,
`!function () {} * 2`), and where the left operand ends on a pair of its own
(`(a * function () {}) / 2`) — carried as controls, unchanged in `output_prettier.svelte`. At
a statement's start (`(function () {}) / 2;`) the pair is prettier's own expression-statement
rule too, and both tools print it once. After `export default` prettier wraps the whole
division instead (`export default (function () {} / 2);`); tsv's pair around the dividend
already opens the value on a `(`, so it adds no second one. A decorated class takes the pair
broken open, the decorator on a line of its own.

The pair holds a block comment its operand owns, where prettier hoists the comment ahead of
it (`(/* c */ function () {}) / 2;` → `/* c */ (function () {}) / 2;`); one after the operand
prints past the `)`, as at every binary operand's pair.

## Reason

◆prettier_bug ◆parser_compat ◆design_choice. See
[conformance_prettier_ts.md §TypeScript](../../../../../docs/conformance_prettier_ts.md#typescript)
(Function / class expression before `/`),
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Every glued block comment is owned) and
[conformance_prettier.md §Prettier bug index](../../../../../docs/conformance_prettier.md#prettier-bug-index).
