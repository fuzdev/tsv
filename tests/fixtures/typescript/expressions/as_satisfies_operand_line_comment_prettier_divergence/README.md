# as_satisfies_operand_line_comment_prettier_divergence

A line comment between a cast's operand and its `as`/`satisfies` keyword
(`(x // c⏎) as A`).

**tsv**: keeps the grouping parens that hold the comment, so it stays exactly
where the author wrote it:

```
const a = (
	x // c1
) as A;
```

**Prettier**: floats the comment out past the whole statement:

```
const a = x as A; // c1
```

## Reason

Unlike every other site in
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent),
this gap **cannot** take a continuation line: a line break before `as` /
`satisfies` performs ASI and ends the expression, so `x // c⏎as A` is a syntax
error (`parser.ts`'s `parseBinaryExpressionRest` breaks out of the cast on
`scanner.hasPrecedingLineBreak()`; tsv, acorn-typescript and tsc all reject it).
A comment can therefore only ever reach this gap from *inside* a grouping paren
shell — and the shell has to survive for the comment to keep its place. Where the
sibling keyword→value gap **strips** a redundant shell
([as_satisfies_value_line_comment](../as_satisfies_value_line_comment_prettier_divergence/)),
this one keeps it: a shell is redundant only when the bare form can still express
the comment's position.

Left inline this is **content loss**, not a layout preference: `(1 // c⏎) as const;`
formatted to `1 // c as const;`, pulling the ` as const;` code into the comment.
The mangled output still parses (as a bare `1` statement), so it was a fixed point
and idempotency and round-trip were both blind to it.

An operand that needs the parens anyway (`(a + b // c9⏎) as A`) takes no second
pair — the required paren is the one that holds the comment.

Two stacked line comments (c7/c8) show why the position is preserved rather than
floated: prettier merges them onto one line **and reorders them**
(`x as // c8 // c7⏎A;` on the first pass, `x as A; // c8 // c7` on the second —
`audit_signature.txt` pins the chain), so `// c7` becomes text of `// c8`. tsv
keeps both distinct, per
[§Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
principle 5.

A **multiline block** carries a line terminator of its own, so it keeps the shell
for the same reason a `//` does — stripping it puts a real line break in the gap
and the output does not re-parse at all (prettier still does this; §Prettier bug
index). The retained shell **expands**, giving the operand its own indented line
rather than gluing it to the `(`, so the two authorings of one comment reach a
single form instead of disagreeing at the same gap — the rule a breaking paren
already follows elsewhere.

The shell has **two** gaps, and the other one keeps it for a different reason:
nothing else emits the `(`→operand gap. A comment there that is neither glued to
the operand (which would make it `owned_by_node`, printed from the operand's own
doc) nor inside the operand's span belongs to no node at all, so stripping the
shell **dropped** it outright — `( // c⏎x) as A` formatted to `x as A`, the
comment simply gone, and the census is what saw it. Retaining on either gap is one
rule for one shell rather than two half-rules that disagree about `( // b⏎x // c⏎)`
(where the trailing comment survived and the leading one did not). A `//` on the
`(` line stays on it (`( // c12`); every other leading comment takes the ordinary
run, so an own-line block keeps its own line and a glued one leads the operand
inline.

A **block** comment forces nothing (`x /* c11 */ as A` stays inline without parens,
matching prettier) — it is pinned here as the control for the line-comment rule.

The gap the printer scans runs to the **keyword**, so it spans the `)` and holds
**two** runs belonging to two emitters: the pair's own, and — past the `)` — the
enclosing gap's. Both are emitted where they were written (`(⏎x // c20⏎) /* c21 */ as A`),
and the split is the outermost `)`, so a comment between two nested closers is
inside the one pair that survives the collapse. Only a single-line block can be
outside: anything occupying a second line there puts a line terminator before the
keyword, which is unparseable. Reading the window as one run instead is what the
earlier gate did — it asked whether a `)` followed the *last* comment, so one
comment written outside the pair flipped the whole question false and the shell
emitted **nothing**, dropping the run inside the pair along with it. Where no
leading comment forced the shell, the same reading produced output that does not
re-parse: `(⏎x // c20⏎) /* c21 */ as A` became `x // c20⏎/* c21 */ as A`, whose
line break before `as` ends the expression.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
