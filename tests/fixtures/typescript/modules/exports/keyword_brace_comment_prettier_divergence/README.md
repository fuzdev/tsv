# keyword_brace_comment_prettier_divergence

A comment between the `export` keyword and the named-specifier `{` is preserved
where the user placed it — before the brace.

- Input: `export /* c1 */ {a} from './a';`
- Prettier (`output_prettier.svelte`): relocates the comment _into_ the braces as
  the first specifier's leading comment — a block comment inline (`export {/* c1 */ a}`),
  a line comment expanding the braces multiline (`export {\n\t// c2\n\tb,\n}`).
- tsv: keeps it before `{` (`export /* c1 */ {a}`; the line comment forces `{` onto
  the next line).

Per Comment Position Philosophy — we preserve user intent when prettier moves a
comment to a different syntactic position. Fixing this also closed a **content-loss
bug**: tsv previously *dropped* this comment entirely (`export /* c */ {a}` →
`export {a}`). Sibling of the import `keyword_brace_comment` divergence and the
export `from_comment` divergence (the gap one token later).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
