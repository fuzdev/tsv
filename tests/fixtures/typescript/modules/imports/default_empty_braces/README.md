# default_empty_braces

An empty named group after a default (or namespace) binding —
`import a, {} from 'x'` — carries no specifiers, so tsv drops it (and its comma)
to match prettier: `import a from 'x'`. The binding→`from` gap is then handled
exactly like a plain default import, so a comment anywhere in the dropped region
collapses to one position before `from` (`import b /* c */ from 'y'`) — a plain
match, and idempotent (the empty group is dropped in a single pass rather than
the comment being duplicated each reformat). A *bare* empty group with no
binding (`import {} from 'x'`) is preserved — see [basic](../basic/).

The `unformatted_*` variants exercise compact and spaced authoring of the dropped
group, including a comment in the dropped region (after the comma); each
normalizes to input under both formatters in one pass.

There is no type-only case here: `import type c, {} from 'z'` is invalid
TypeScript (a type-only import cannot specify both a default import and named
bindings), so although tsv leniently parses it (matching acorn-typescript),
prettier rejects it outright — there is no valid type-import form that carries a
droppable empty group.
