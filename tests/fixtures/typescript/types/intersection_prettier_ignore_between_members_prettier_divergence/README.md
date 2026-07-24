# intersection_prettier_ignore_between_members_prettier_divergence

An own-line `// prettier-ignore` between intersection members freezes only the
**following** member (`{b:2}` here). tsv keeps the directive where the author wrote
it — on its own line between the members — and freezes just that member.

**Prettier** relocates the directive and cannot hold a freeze-preserving fixed
point:

- pass 1 (`output_prettier.svelte`) — the directive trails the preceding `&`
  (`{ a: 1 } & // prettier-ignore⏎{b:2} & { c: 3 };`), still freezing the member;
- its **fixed point** (`audit_signature.txt`, pass 2) — the short intersection
  collapses inline, the directive floats to the statement end, and the freeze is
  **lost** (`{b:2}` reformats to `{ b: 2 }`).

So prettier is non-idempotent here and its stable form drops the freeze entirely;
tsv preserves both the directive's authored position and the member freeze. The
union analog (`union_prettier_ignore_between_members`) is clean in both tools — only
the intersection between-member position provokes prettier's relocation, because
prettier has no intersection printer.

## Reason

Per Comment Position Philosophy, tsv keeps the directive where the author placed it
and freezes the member it precedes, rather than relocating the comment across the
`&` (and, at prettier's fixed point, losing the freeze).

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive)
and §Comment relocation.
