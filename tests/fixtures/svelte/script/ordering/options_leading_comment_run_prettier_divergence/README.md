# options_leading_comment_run_prettier_divergence

A run of two comments directly above `<svelte:options>`. tsv treats the run the way it treats
a run above a `<script>` or `<style>`: every comment leads the section, in authored order,
and travels with it through the canonical reorder. prettier-plugin-svelte lifts only the one
comment glued to the tag (`stripSvelteOptionsComment`) and leaves the rest in the template —
which, after the reorder, prints **below** the options tag: the two comments swap sides.

tsv:

```svelte
<!-- comment1 -->
<!-- comment2 -->
<svelte:options runes />
```

Prettier:

```svelte
<!-- comment2 -->
<svelte:options runes />

<!-- comment1 -->
```

## Reason

Comment position is authorship: the author wrote `comment1` above `comment2` and both above
the tag, and prettier's form reverses that order. Keeping the run whole is lossless and is
what both formatters already do above a `<script>` (see
[root_before_style_blank_line](../../../syntax/comments/root_before_style_blank_line/)). See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)
and the shared frame's
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
