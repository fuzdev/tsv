# with_keyword_comment_prettier_divergence

A comment in an import's attributes header — between the source and the `with`
keyword, or between `with` and the attributes `{` — is preserved where the user
placed it.

**Prettier** (`output_prettier.svelte`): keeps a block comment already before
`with` in place, relocates a `with`→`{` block comment back to before `with`, and
floats a line comment after `with` past the `;`:

```
import a from './a' /* c1 */ with {type: 'json'};
import c from './c' /* c3 */ with {type: 'json'};
import d from './d' with {type: 'json'}; // c4
```

**tsv**: preserves each comment where the user placed it:

```
import a from './a' /* c1 */ with {type: 'json'};
import c from './c' with /* c3 */ {type: 'json'};
import d from './d' with // c4
	{type: 'json'};
```

The source→`with` block comment (c1) is dual-stable — both formatters keep it in
place. The `with`→`{` block comment (c3) relocates to before `with` in Prettier;
the `with`→`{` line comment (c4) floats past `;` (the before-semicolon/float-out
rule). Per Comment Position Philosophy. When the c4 line comment forces the
`{…}` onto its own line, tsv indents that continuation one level (a single
statement spanning lines). Sibling of the import `from_comment`
divergence (the gap one token earlier).

The **line comment between the source and `with`** (the c2 form) is a different
beast — prettier's `typescript` parser *throws* on it, so it has no
`output_prettier` oracle and lives in the sibling
`with_keyword_comment_line_prettier_divergence` (rule F6).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
