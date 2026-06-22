# flush_body_param_wrap_prettier_divergence

A `{#snippet}` whose parameter list nearly fills the line (the open tag is 100 chars) and whose body
is authored **flush** (`{#snippet fn(…)}<div>text</div>{/snippet}`). The flush body can't share the
open-tag line, so the two formatters resolve it differently:

- **tsv** keeps the params **inline** (they fit at 100) and drops the body to its **own line**.
- **prettier** **wraps the params** one-per-line — sacrificing the inline fit — to keep the body
  **hugging** the `)}` (`)}<div>text</div>{/snippet}`).

tsv's call keeps the params readable on one line and puts the body where block layout puts it;
prettier over-wraps the params just to attach the body. This is the standalone counterpart to the
"params inline + body expand vs prettier's param-wrap" note for snippets. The `unformatted_ours_compact`
(the flush authoring) and `prettier_variant_compact` (prettier's params-wrapped form) both normalize
to `input.svelte` under tsv in one pass.

See [conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).
