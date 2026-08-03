# expr_multibyte_prettier_divergence

Multibyte comment text (CJK + an astral-plane emoji) in an expression tag, in both comment
shapes — the byte↔UTF-16 offset translation `expected.json` pins is the fixture's subject, and
the line-comment case additionally spans a `//` whose end is a newline rather than a delimiter.

The layout claim is the shared braced-head one: a leading line comment forces the break, so
tsv hangs the value one level in where prettier leaves it flush.

```svelte
{// 中文😀 line comment before expression
	a}
```

Prettier: `{// 中文😀 line comment before expression⏎a}`. The block-comment case forces no
break and both formatters agree, so it doubles as the control.

## Reason

See [conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
The divergence is not multibyte-specific — this fixture inherits it from the shared rule and
exists to pin the offsets.

## Related

- [expression_tag_line_comment](../expression_tag_line_comment_prettier_divergence/) — the same pair in ASCII
- [expr_leading_line](../expr_leading_line_prettier_divergence/) — the whole braced family swept at once
- [utf8_multibyte](../utf8_multibyte/) — multibyte content outside comments
