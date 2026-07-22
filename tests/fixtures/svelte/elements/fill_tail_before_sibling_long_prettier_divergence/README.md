# fill_tail_before_sibling_long_prettier_divergence

Tail text after an `{expression}` tag that is followed by an **inline sibling** rather than
ending the element. That makes the text node non-last, so the fill carries a `trailing_line`
as well as its `leading_line` — **both** separators, giving it odd length. Its trailing `line`
lands in the fill renderer's last-item slot while its leading one still occupies a content
slot, so this shape crosses both halves of the parity rule at once.

```
tsv       `…aaaa ~{xxxxxxxxxx}` / `cc <b>x</b>`   all ≤ 100
Prettier  `…aaaa ~{xxxxxxxxxx} cc` / `<b>x</b>`   101 chars
```

This is the shape that caught a **non-idempotency**: an earlier fix that measured only the
final pair left this one measuring a content-slot `line` by itself, which always "fits" (1
column flat). Pass 1 emitted a 101-char line and pass 2 — measuring from a different column —
moved the break, so the format had no fixed point. Both passes are pinned here by `input`
being its own format output.

`unformatted_ours_joined` holds the joined authoring, which tsv rewrites to `input` and
prettier reformats to its own 101-char form.

`divergent_variant_prettier_form` is that prettier form. Prettier keeps it stable; tsv rewrites
it to a distinct **third** stable form (`cc` and `<b>x</b>` each on their own line), because the
newline prettier leaves between them is an authored fill boundary tsv preserves — the same
authoring-as-intent rule as
[fill_break_before_expr_long](../fill_break_before_expr_long_prettier_divergence/)'s
`divergent_variant_overflow`.

## Reason

Print width is a hard limit in tsv. See
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
