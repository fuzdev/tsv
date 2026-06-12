# trailing_member_comment_prettier_divergence

Prettier relocates line comments before trailing member access in call chains. tsv preserves comments where the user placed them.

tsv: `.filter((x) => x)\n// comment\n.length` (preserved)
Prettier: `// comment\nitems.filter((x) => x).length` (relocated before chain)

## Reason

tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and other chain comment contexts.
