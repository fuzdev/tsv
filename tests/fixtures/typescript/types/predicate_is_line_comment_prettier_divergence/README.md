# predicate_is_line_comment_prettier_divergence

A line comment after a type predicate's `is` keyword, before the predicate type
(`function f(x): x is // c\n\tT`).

**tsv**: keeps the comment after `is`, with the predicate type on the next line:

```
function f(x): x is // c
	T {
	return true;
}
```

**Prettier**: relocates the comment to trail the function body's opening `{`
(`output_prettier.svelte`); it is non-idempotent doing so — a second pass moves
the comment onto its own line inside the body (`audit_signature.txt` pins the
two-pass chain):

```
function f(x): x is T { // c
	return true;
}
```

## Reason

Per Comment Position Philosophy: the user wrote the comment after `is`, so tsv
keeps it associated with the predicate rather than floating it past the
predicate type onto the body brace. Both forms are idempotent in their
respective formatters. A same-line block comment (`x is /* c */ T`) stays inline
in both formatters and is not a divergence (see the regular
[type_operator_keyword_comment](../type_operator_keyword_comment/) fixture,
which covers `x is /* c */ A`); only a line comment after `is` diverges.

Previously tsv emitted the comment inline and **swallowed the predicate type**
(`function f(x): x is // c T {` — `T {` absorbed into the comment, a
non-idempotent content loss); keeping it on the `is` line via `line_suffix` with
the predicate type on the next line fixes the loss and preserves the user's
placement.

A redundant paren wrapping the predicate type with the line comment inside
(`x is (// c\n T)`, and the double-nested `((…))`) strips to this same fixed
point — the `unformatted_ours_single_paren` / `unformatted_ours_double_parens`
variants verify the paren form is idempotent too.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
