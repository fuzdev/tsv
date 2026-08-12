# Divergence: assignment before-operator line comment (preserve, lossless)

A line comment between an assignment's target and its operator (`a // c⏎= 1;`).
tsv keeps the comment after the target and drops the operator and value to a
continuation line **indented one level** (uniform forced-continuation indent).
Prettier **relocates** the comment past the value to end-of-statement
(`a = 1; // c`).

```ts
// tsv (preserve + continuation indent)   // prettier (relocate to end-of-line)
a // c                                    a = 1; // c
	= 1;
```

**Why tsv preserves rather than trails:** when a second comment already trails the
statement (`b // c1⏎= 2; // c2`), prettier's relocation **merges both onto one
line** — `b = 2; // c1 // c2`, where `// c2` becomes text inside `// c1`
(information loss). tsv keeps the two distinct. Trailing the before-operator
comment would re-import that loss, so tsv preserves position.

The assignment-expression face of the before-`=` initializer family — the same rule
the [variable declarator](../../../declarations/variable/declarator_before_eq_line_comment_prettier_divergence/),
[class property](../../../declarations/class/property_before_eq_line_comment_prettier_divergence/),
[enum member](../../../declarations/enum/member_before_eq_line_comment_prettier_divergence/),
[type alias](../../../types/aliases/name_before_eq_line_comment_prettier_divergence/) and
[binding default](../../destructuring/default_equals_line_comment_prettier_divergence/)
answer. A member target and a compound operator (`+=`) take it unchanged: the
operator is whatever the author wrote, and the continuation carries it. A
single-line block in the same gap forces nothing and stays inline in both
formatters (`a /* c */ = b`, the plain [assignment_comment](../assignment_comment/)).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent.
