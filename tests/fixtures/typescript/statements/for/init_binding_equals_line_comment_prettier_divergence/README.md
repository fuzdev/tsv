# init_binding_equals_line_comment_prettier_divergence

A line comment between a for-init binding and its `=` (`for (let a // c⏎= 0; …)`).
tsv keeps the comment after the binding and drops `= value` to a continuation line
**indented one level** (uniform forced-continuation indent). Prettier **relocates**
the comment past the value to the end of the init clause's line
(`let a = 0; // c`).

```
// tsv (preserve + continuation indent)   // prettier (relocate to end-of-clause)
for (                                     for (
	let a // c                              let a = 0; // c
		= 0;                                  a < 10;
	a < 10;                                 a++
	a++                                   ) {}
) {}
```

**Why tsv preserves rather than trails:** when a second comment already trails the
init clause (`let b // c1⏎= 0; // c2`), prettier's relocation **merges both onto one
line** — `let b = 0; // c1 // c2`, where `// c2` becomes text inside `// c1`
(information loss). tsv keeps the two distinct. Trailing the before-`=` comment would
re-import that loss, so tsv preserves position.

At a **later declarator** the rule holds at that declarator's own level. Prettier's
relocation there is not idempotent: the float removes the very break its comment was
forcing, so pass 2 collapses the declarator list back onto one line
(`let e = 0, f = 1; // c` — the chain `audit_signature.txt` pins).

The for-init face of the variable-declarator
[declarator_before_eq_line_comment](../../../declarations/variable/declarator_before_eq_line_comment_prettier_divergence/),
whose gap this one answers identically — one rule across the two constructs. The
same gap's **block** comment forces no break and stays in place in both formatters,
the non-divergent [init_binding_equals_comment](../init_binding_equals_comment/).

See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation
and [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Uniform Forced-Continuation Indent.
