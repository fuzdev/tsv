# Divergence: a pattern's bracket→`?` comment stays on its authored side

A block comment between a destructuring pattern's closing `}`/`]` and its optional `?`
marker. tsv keeps it where the author wrote it — before the marker; prettier moves it.

```ts
// tsv (authored side)                 // prettier
type F1 = ({ a } /* c1 */?: T) => void;  type F1 = ({ a /* c1 */ }?: T) => void;
type F3 = ({} /* c3 */?: V) => void;     type F3 = ({}? /* c3 */ : V) => void;
```

Prettier answers the same gap **two** ways: on a non-empty pattern it relocates the
comment *into the brackets*, re-associating it with the last element, and on an empty
one it relocates *past the `?`* instead. Neither destination is the authored gap, and
the split is an attachment artifact rather than a rule.

The pattern spelling of the binding-head rule the identifier already takes
([param_optional_comment](../../../declarations/function/param_optional_comment_prettier_divergence/)
— `a /* c */?` against prettier's `a? /* c */ :`), and the same refusal the mapped-type
key makes at its own `]`→`?` gap
([mapped_optional_marker_comment](../../../types/mapped_optional_marker_comment_prettier_divergence/)),
where prettier likewise relocates into the brackets. The gap on the far side of the
marker is [pattern_bracket_colon_comment](../pattern_bracket_colon_comment/), where both
formatters keep the comment outside and tsv matches.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
