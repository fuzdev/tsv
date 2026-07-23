# inline_closing_text_prettier_divergence

An inline component preceded by same-line text, followed by glued trailing text (`.`). tsv
breaks at the whitespace boundary before the `<Comp>` so it starts a **fresh line** and
collapses inline (`<Comp>MDN</Comp>.`), rather than dangling its opening tag at the end of the
text line and dropping the content. The glued `.` stays with `</Comp>`.

tsv: `<Comp>` moves to its own line and collapses; the opening tag never dangles after a space.
Prettier: keeps the opening tag on the text line and dangles it (`<Comp …"⏎\t>MDN</Comp⏎>.`) —
see `prettier_variant_dangle.svelte` (prettier keeps that form; tsv normalizes it to `input.svelte`).
`unformatted_ours_compact.svelte` is the compact authoring (tsv → `input.svelte`, prettier → dangle).

The boundary before `<Comp>` is inter-node whitespace (render-free under Svelte 5), so the break
is render-equivalent.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
