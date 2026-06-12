# heritage_last_item_line_comment_prettier_divergence

Prettier relocates line comments between the last heritage item and `{` into the class/interface body.

tsv: preserves each comment before `{` with a forced line break (`J // c\n{}`), keeping every comment on its own line
Prettier: moves the comments inside the body (`J {\n\t// c\n}`)

## Reason

tsv treats user comment placement as intentional — the comment annotates the heritage item, not the body. When more than one comment precedes `{`, each is kept on its own line: collapsing them onto the heritage line would absorb a following comment into the first line comment's text (`// c1 // c2` reparses as a single comment), destroying the comment boundary — a content/semantic loss, not just a position change. Consistent with tsv's handling across while, try/catch, if/else, switch, for, do-while, labeled statements, and call chains.

## Cases

- `extends C // c\n{}` — single trailing comment kept before `{`
- `extends C // c1\n// c2\n{}` — multiple line comments, each on its own line (not merged)
- `class extends C // c1\n// c2\n{}` — same for class **expressions**
- `extends C // c1\n/* c2 */\n{}` — a block comment following a line comment stays on its own line
