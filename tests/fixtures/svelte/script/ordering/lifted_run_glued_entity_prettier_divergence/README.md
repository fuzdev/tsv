# lifted_run_glued_entity_prettier_divergence

A hoisted section written between two texts with nothing between them, where the text before it
ends in a character reference left open — an `&` followed only by characters that could continue
one — and the text after it continues it: `x&amp<script>…</script>;y`. Svelte decodes each text
node on its own, so the two render `x&` and `;y`. The section goes to the top, the two texts join
as one glued text (as [lifted_run_glued](../lifted_run_glued/) pins), and the next parse decodes
that text whole: `x&amp;y` is `x&y`, a different page.

tsv spells the first character after the seam as a character reference — `x&amp&#x3B;y`. It
decodes to the same character, and the `&` that opens it ends the open reference in front of it,
so the joined text renders exactly as the two did. The form is its own fixed point under both
formatters.

Prettier joins the texts as written, landing on the reference the halves never spelled: each
`variant_*` file is its output from the `unformatted_ours_*` of the same name, a render change its
own next pass keeps.

## Cases

Each `unformatted_ours_*` variant writes one authoring into one line of `input.svelte`:

- `unformatted_ours_script.svelte` — a module `<script>`, completing a named reference with its
  `;` (`x&amp` + `;y`).
- `unformatted_ours_style.svelte` — a `<style>`, completing a name (`x&am` + `p;y`).
- `unformatted_ours_options.svelte` — `<svelte:options>`, opening a numeric reference after a
  bare `&` (`x&` + `#32;y`).
- `unformatted_ours_comment.svelte` — the instance `<script>` with its glued comment travelling
  with it, extending a reference that already decodes to a longer one (`x&not` + `in;y`).
- `unformatted_ours_hex.svelte` — `<svelte:options>`, completing a hex reference (`x&#x` + `41;y`).
- `unformatted_ours_ignore.svelte` — a module `<script>` after a text a `format-ignore` freezes;
  the frozen text is the joined one, respelled.
- `unformatted_ours_script_style.svelte` — a module `<script>` and a `<style>` back to back, one
  seam, completing a numeric reference (`x&#3` + `2;y`).
- `unformatted_ours_multi_seam.svelte` — a module `<script>` and a `<style>` at two seams of one
  text (`x&` + `a` + `mp;y`). The first seam joins `x&a`, which is still no reference; the second
  would complete one, so the character respelled is the one after the second.
- `unformatted_ours_range.svelte` — a module `<script>` inside a `format-ignore` range, cut out of
  the range's verbatim slice at a glued seam.
- `unformatted_ours_range_mixed.svelte` — the same range, with a `<style>` between two texts
  outside it too, so the range is cut in the document the join rewrites.
- `unformatted_ours_range_multi_seam.svelte` — two seams of one text inside a range.

`unformatted_no_reference.svelte` is the control: `x&amp` + `x;y` joins to `x&ampx;y`, where the
reference `&amp` still ends at the seam, so both formatters print the join as written.

The zero-code edge, where tsv respells a seam that renders the same either way, is
[lifted_run_glued_entity_zero_code](../lifted_run_glued_entity_zero_code_svelte_prettier_divergence/).

With the comment glued to its section, prettier also drops the template's last node — the loss
[lifted_run_glued_comment](../lifted_run_glued_comment_prettier_divergence/) pins.

## Reason

◆prettier_bug. See
[conformance_prettier_svelte.md §Svelte: Root section ordering](../../../../../../docs/conformance_prettier_svelte.md#svelte-root-section-ordering)
and [conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index).
