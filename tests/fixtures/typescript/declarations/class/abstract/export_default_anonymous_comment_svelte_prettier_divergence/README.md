# Divergence: comment between `abstract` and `class` of an anonymous default-exported class

`export default abstract /* c */ class {}` — an anonymous `abstract` class as the default export,
with a block comment between its two keywords.

**Parser (Svelte):** acorn-typescript rejects the anonymous abstract default export in every form
(`export default abstract class {}` included): it parses the `abstract` class as a statement,
which demands a name. tsc accepts it — `export default` takes a class declaration with no name —
and tsv parses it as the `ClassDeclaration` acorn builds for the named class, with `id: null`
(`expected_ours.json`; the canonical parser errors → `expected_svelte.json`).

**Formatter (prettier):** tsv keeps the comment between the keywords, where the author wrote it,
as for every modifier-keyword gap
([declaration_keyword_name](../../../../syntax/comments/declaration_keyword_name_prettier_divergence/));
prettier relocates it past `class` to the position the name would take
(`export default abstract class /* c */ {}`, `output_prettier.svelte`, its fixed point).

See [conformance_svelte.md](../../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
