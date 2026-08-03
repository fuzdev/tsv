# Divergence: destructuring-rename key→`:` line comment (preserve, content loss guard)

A line comment in a destructuring rename's key→`:` gap (`{ a // c⏎: b }`, also in
a parameter pattern). tsv keeps the comment after the key and drops `: local` to a
continuation line **indented one level** (uniform forced-continuation indent); the
pattern breaks because the comment forces a line. Without the break, the `//`
would swallow `: b } = o;` — content loss, not a layout choice.

Prettier has **two destinations from this one gap, keyed on the authoring**:

- From the forced-break authoring (input) it **hoists** the comment to the
  property's own leading line, expanding the pattern
  (`{⏎\t// c⏎\ta: b⏎}` — `output_prettier.svelte`).
- From the **own-line** authoring it instead crosses the `:` and hangs the
  comment leading the local name (`{⏎\ta:⏎\t\t// c⏎\t\tb⏎}` —
  `divergent_variant_own_line.svelte`, one pass, prettier-stable). That landing
  is **not** tsv-stable — a destructuring pattern has no multiline preservation,
  so tsv re-collapses it keeping the comment in its relocated after-`:` slot,
  trailing the `:`, the local on a continuation line indented one level
  (`{ a: // c⏎\t\tb }`, a third stable form). Three stable forms coexist,
  keyed on which side of the `:` the comment was authored; only prettier moves
  a comment between them.

```ts
// tsv (preserve + continuation indent)   // prettier (hoist to leading)
const {                                   const {
	a // c                                	// c
		: b                               	a: b
} = o;                                    } = o;
```

- `unformatted_ours_compact.svelte` — the flush authoring: tsv normalizes it to
  input; prettier hoists (one pass to `output_prettier.svelte`'s form).
- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv pulls the
  comment up to trail the key and reaches input — one pass; prettier takes it
  to the cross-`:` hang instead (one pass, no intermediate).

The pattern face of the object-property key→`:` line comment
([property_key_colon_line_comment](../../objects/property_key_colon_line_comment_prettier_divergence/));
the own-line **block** sibling is
[rename_key_colon_own_line_block_comment](../rename_key_colon_own_line_block_comment_prettier_divergence/).
See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
