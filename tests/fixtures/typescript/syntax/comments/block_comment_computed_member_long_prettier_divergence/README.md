# block_comment_computed_member_long_prettier_divergence

When a computed member access (`obj?.[expr]` or `obj[expr]`) contains a block comment
(`/* ... */ d`) and the line exceeds print width, Prettier hoists the block comment
out of the brackets to before the member chain. This changes the apparent association
from the bracketed expression to the entire chain.

tsv: keeps block comment inside brackets where the author placed it (semantically correct)
Prettier: hoists block comment before the chain when breaking (changes association)

## Example

```js
// Input (short, both agree):
obj.chain?.[/** @type {string} */ d];

// Long — tsv (preserves comment placement):
obj.chain?.[
  /** @type {string} */ d
];

// Long — Prettier (hoists comment):
/** @type {string} */ obj.chain?.[
  d
];
```

## Reason

Block comments inside computed member brackets are typically associated with the bracketed
expression (e.g., a JSDoc type cast `/** @type {string} */ d`). Hoisting the comment before
the chain changes the apparent target. tsv preserves the author's intent.

Prettier is also not idempotent on this pattern — the first pass keeps the comment inside
brackets, the second pass hoists it. tsv is stable in one pass.

Reason: Comment relocation (comment position). See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
