# body_prettier_ignore_head_prettier_divergence

An own-line directive in a `label:`→body gap freezes the body statement — the node that
follows the directive, over its own span. The label and its `:` are parent-owned and stay
outside the frozen slice, so they still normalize:

```ts
lll:
// prettier-ignore
for (;;) {
	fn(  bbb  );
	break lll;
}
```

Prettier freezes the **whole labeled statement**, label included: its comment attaches to
the `LabeledStatement` (a `:` begins no node in its model), so a spaced label survives the
format (`lll   :`) where tsv normalizes it. `prettier_variant_label_spaces.svelte` pins
exactly that — prettier keeps the spaced label, tsv reduces it to `input`. Without the
directive prettier normalizes the same label, so the widening is the freeze's doing rather
than a standing label rule.

The directive's own line is also kept rather than pulled onto the label's (`lll: // c`, the
placement an ordinary comment takes here): a head-trailing directive is inert under the
placement floor, so the relocated form would lose the freeze on the second pass. **Both
spellings** behave alike — placement keys the freeze, not the comment's spelling.

An author **blank** between the directive and the body survives when the frozen body
opens no `{` (the `ooo:` case) — not an ignore rule at all, but the header→body gap's
brace rule, which this gap is the only reachable brace-less caller of. Both formatters
keep it, so that case is a control rather than part of the divergence; the drop it
replaced was a `{`-shaped licence applied to a body that has no `{`. See
[conformance_prettier_ts_comments.md §"No blank above a body block's `{`"](../../../../../../docs/conformance_prettier_ts_comments.md).

## Reason

Rule A binds a directive to the node that follows it, which at this gap is the body, not
the statement that encloses both; ◆design_choice. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
