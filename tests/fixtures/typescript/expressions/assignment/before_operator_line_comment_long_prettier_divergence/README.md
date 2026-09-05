# Divergence: assignment before-operator line comment at breaking width

The width face of
[before_operator_line_comment](../before_operator_line_comment_prettier_divergence/) —
the same rule (`a // c⏎\t= 1`), asked where the value no longer fits on the
continuation line the comment forces.

tsv keeps the comment after the target and drops the operator and value to a
continuation line indented one level. The value's layout is then decided **on that
line**: a logical chain that does not fit breaks its operands **flush with the
operator**, since the continuation is already the value's indent. An assignment's
value is one of prettier's `shouldIndentIfInlining` positions, so a non-inlining
chain there takes no second level — the continuation arm answers it the same way
the ordinary layout does, which is the whole point of the pair below.

```ts
// tsv (preserve + continuation indent)      // prettier (relocate)
a // c1                                      a = xxx ?? yyy ?? zzz; // c1
	= xxx ?? yyy ?? zzz;

b // c2                                      b = // c2
	= xxx ??                                     xxx ?? yyy ?? zzz;
	yyy ??
	zzz;
```

Both cases are the 100/101 boundary of tsv's continuation line: at 100 the value
stays on it, at 101 its operands break — flush, not indented again.

Prettier's two destinations differ from each other because 100/101 is a boundary
for prettier's *own* layout too: at 100 the whole assignment fits on one line, so
the comment trails the statement — at **106 chars**, over the print width
(`◆print_width`); at 101 prettier breaks after the operator and the comment trails
the `=` instead. tsv's answer is one form for both.

**Why tsv preserves rather than trails:** stated in full on the sibling — prettier's
end-of-statement relocation **merges** a second comment already trailing the
statement onto one line (`b = 2; // c1 // c2`, where `// c2` becomes text), so
trailing would re-import that loss.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Uniform Forced-Continuation Indent.
