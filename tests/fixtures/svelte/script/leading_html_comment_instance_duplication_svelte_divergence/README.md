# Parser divergence: leading HTML comment duplicated onto the instance script

A leading HTML comment (`<!-- @component … -->`, the component/module doc
comment) before a `<script module>` + instance `<script>` pair is attached by
Svelte to **both** the module Program *and* the instance Program
(`module.content.leadingComments` **and** `instance.content.leadingComments`) —
the comment is duplicated across the two script roots. (With a single lifted root there is nothing to copy onto and tsv
matches Svelte.)

The second root need not be a **script**: a lifted `<script>` / `<style>` is
never appended to the fragment, so canonical's backwards walk steps over one as
if it were absent, and `<!-- c --><script/><style/>` puts the comment on the
instance *and* on the stylesheet (as does the reverse order). tsv stops at the
hole a lifted tag leaves between two fragment nodes, so the nearest root keeps
it in every arrangement — pinned as controls in
[svelte_preceding_comment.rs](../../../../svelte_preceding_comment.rs), beside
the gap-class rule that shares the same reader.

**tsv attaches the comment once, to the nearest script Program** — the module
Program — and does not copy it onto the instance Program (`expected_ours.json`
vs `expected_svelte.json`). The comment is never lost: it is also present as a
`Comment` node in the fragment in both parsers, so the distinct-comment set is
identical and `ast_diff` confirms semantic (code) equivalence. This is the same
anti-duplication stance tsv takes on the module-script comment leak
([../module_comment_instance_duplication_svelte_divergence](../module_comment_instance_duplication_svelte_divergence/))
and on acorn-typescript's backtrack-reparse comment duplication.

Formatting is unaffected: the formatter locates comments by position and emits
each once at the author's placement.

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md)
§Comment Attachment Differences.
