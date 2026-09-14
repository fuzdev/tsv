# predicate_is_mixed_trailing_comment_prettier_divergence

The mixed / trailing extension of
[predicate_is_line_comment](../predicate_is_line_comment_prettier_divergence/): a
redundant paren shell around a type-predicate type (`x is (…)`) whose leading gap
holds a **block before the line comment** (mixed), or whose trailing gap holds a
**block after the type** (trailing).

**tsv**: strips the shell and hangs the run at the same fixed point the bare
authoring settles on — the block trails `is` inline, the line comment forces the
type onto the next line, and a trailing block trails the type before the body `{`:

```
function f(x): x is /* b */ // c
	A {}

function g(x): x is // c
	B /* t */ {}
```

**Prettier**: relocates the block before `is` (`x /* b */ is A`) and floats the
line comment to trail the body `{}` (`{} // c`); the trailing block stays inline:

```
function f(x): x /* b */ is A {} // c
function g(x): x is B /* t */ {} // c
```

The third case is the same seam at a run with no `//` in it at all: a **multi-line**
block the author GLUED ahead of a single-line one and then broke after
(`is /* a⏎b */ /* c */⏎A`). The multi-line body cannot print flat, so the run's break is
forced — but it is the RUN's break, not either comment's, and the per-comment own-line
rule answers no for both halves (the first is glued, the second single-line). The gate
reads the run-level question beside it
(`Printer::broke_after_run_holds_multiline_block`), the same question the shelled
spelling's ownership claim reads, so both authorings hang:

```
function h(x): x is /* a
b */ /* c */
	A {}
```

Without it the two disagreed across passes — the shell's claim took the run, the gap
declined to lay it out, and the type welded back onto the `*/` line on the reparse.

**Prettier has no usable answer at that case**: it relocates the whole run across `is`
(`x /* a⏎b */ /* c */ is A`), putting a line break ahead of the predicate — **output no
parser accepts** (`A type predicate is only allowed in return type position`), so
prettier throws on its own pass-1 result. That truncated chain is what rule F4b
tolerates; see
[conformance_prettier.md §Prettier bug index](../../../../../docs/conformance_prettier.md#prettier-bug-index).

Per Comment Position Philosophy, tsv keeps the comments associated with the
predicate type. The `unformatted_ours_*` variants verify the paren shells are
idempotent under tsv.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
