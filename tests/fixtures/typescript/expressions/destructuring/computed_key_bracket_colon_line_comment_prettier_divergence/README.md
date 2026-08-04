# Divergence: destructuring computed-key `]`→`:` line comment (preserve, content loss guard)

A line comment in a destructuring computed key's `]`→`:` gap (`{ [x] // c⏎: a }`,
also in a parameter pattern). tsv keeps the comment after the `]` and drops
`: local` to a continuation line **indented one level** (uniform
forced-continuation indent); the pattern breaks because the comment forces a
line. Without the break, the `//` would swallow `: a } = o;` — content loss, not
a layout choice.

Prettier has **two destinations from this one gap, keyed on the authoring**:

- From the forced-break authoring (input) it **hoists** the comment to the
  property's own leading line, expanding the pattern
  (`{⏎\t// c⏎\t[x]: a⏎}` — `output_prettier.svelte`).
- From the **own-line** authoring it instead crosses the `:` and hangs the
  comment leading the local name (`{⏎\t[x]:⏎\t\t// c⏎\t\ta⏎}` —
  `divergent_variant_own_line.svelte`, one pass, prettier-stable). That landing
  is **not** tsv-stable — a destructuring pattern has no multiline preservation,
  so tsv re-collapses it keeping the comment in its relocated after-`:` slot,
  trailing the `:`, the local on a continuation line indented one level
  (`{ [x]: // c⏎\t\ta }`, a third stable form). Three stable forms coexist,
  keyed on which side of the `:` the comment was authored; only prettier moves
  a comment between them.

```ts
// tsv (preserve + continuation indent)   // prettier (hoist to leading)
const {                                   const {
	[x] // c                              	// c
		: a                               	[x]: a
} = o;                                    } = o;
```

- `unformatted_ours_compact.svelte` — the flush authoring: tsv normalizes it to
  input; prettier hoists (one pass to `output_prettier.svelte`'s form).
- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv pulls the
  comment up to trail the `]` and reaches input — one pass; prettier takes it
  to the cross-`:` hang instead (one pass, no intermediate).

The pattern face of the object-literal computed-key `]`→`:` line comment
([computed_key_bracket_colon_line_comment](../../objects/computed_key_bracket_colon_line_comment_prettier_divergence/));
the rename sibling sharing the same continuation seam is
[rename_key_colon_line_comment](../rename_key_colon_line_comment_prettier_divergence/);
the after-`]` inline block sibling is
[computed_key_bracket_comment](../computed_key_bracket_comment_prettier_divergence/).
See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
