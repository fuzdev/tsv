# value_prettier_ignore_trailing_head_prettier_divergence

A `prettier-ignore` directive **trailing an object property's `:`**
(`k: // prettier-ignore⏎\tvalue`).

**tsv** reads the directive as inert: it shares its line with the key, and tsv honors a
directive only when it is alone on its line, so the comment stays where the author put it
and the value formats normally. **Prettier** keeps the directive on the `:` line too but
freezes the value — `prettier_variant_frozen.svelte` holds that prettier-stable form
(`fn([⏎0, 0,⏎1, 0⏎])` kept verbatim), which tsv normalizes to `input.svelte`.

The declarator `=` case is the control: prettier leaves the same placement inert there, so
both formatters reflow the value — prettier answers one placement two ways across the two
heads.

## Reason

The placement rule is total: a directive freezes the following construct only when it is
alone on its line. Honoring the trailing spelling at the `:` would make one placement mean
two things across the `=` and the `:`, and the own-line spelling — which both formatters
honor — is one line away. The annotation head's twin is
[union_prettier_ignore_trailing_annotation_head](../../../types/union_prettier_ignore_trailing_annotation_head_prettier_divergence/).

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
