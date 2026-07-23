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

Per Comment Position Philosophy, tsv keeps the comments associated with the
predicate type. The `unformatted_ours_*` variants verify the paren shells are
idempotent under tsv.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
