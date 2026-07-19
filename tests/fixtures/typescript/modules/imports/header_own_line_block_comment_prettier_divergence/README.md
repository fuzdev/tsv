# import header own-line block comment

A single-line **block** comment in a module-header gap trails inline and the value
reflows onto the header line — from **any** authored position, including one the
author wrote on its own line, and including one preceded by a blank line
(`unformatted_ours_own_line`, `unformatted_ours_own_line_blank`).

A block comment forces nothing (`import a from /* c */ 'm'` is legal on one line), so
a break around it is ordinary layout, not the comment's doing. A module header is a
**keyword→value** gap, so it follows the shared `comment_hangs_next` rule its
`export default` / `export =` siblings use: only a *line* comment (runs to
end-of-line) or a *multiline* block the author broke after hangs the value. See
[conformance_prettier.md §Authored breaks in value
position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position);
the blank yields with the break it belonged to.

Prettier instead preserves the break and relocates the comment, to a different target
per gap — into the braces as a specifier-trailing comment (specifiers→`from`), into
the braces as a leading comment (keyword→`{`), or kept flat on the header line
(default binding→`from`). Its first pass is unstable, so the chain is pinned as
`prettier_intermediate_to_variant_*` → `variant_*` (both formatters hold the
second-pass form stable).

The glued authoring of these same gaps lives in the regular
[keyword_comment](../keyword_comment/) fixture; the *line*-comment cases, which hang
with a continuation indent, live in
[source_line_comment](../source_line_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
