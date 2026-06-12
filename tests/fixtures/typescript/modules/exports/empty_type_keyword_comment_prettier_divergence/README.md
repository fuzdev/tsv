# empty_type_keyword_comment_prettier_divergence

Comments around the `type` keyword of an empty type-only re-export — between
`export` and `type`, or between `type` and the empty specifier braces — are
preserved where the user placed them.

**Prettier**: relocates them to after `from` (`output_prettier.svelte`):

```
export type {} from /* c1 */ './a';
export type {} from // c2
'./b';
export type {} from /* c3 */ './c';
export type {} from // c4
'./d';
```

**tsv**: preserves them where the user placed them — a block comment trails inline,
a line comment stays on its own line with `type {}` following:

```
export /* c1 */ type {} from './a';
export // c2
type {} from './b';
export type /* c3 */ {} from './c';
export type // c4
{} from './d';
```

Per Comment Position Philosophy, the user's chosen position is preserved. Both
positions are dual-stable in our formatter. The export sibling of
`modules/imports/empty_type_keyword_comment_prettier_divergence`.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
