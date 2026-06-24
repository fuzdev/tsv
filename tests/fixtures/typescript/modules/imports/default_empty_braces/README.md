# default_empty_braces

An empty named group after a default (or namespace) binding —
`import a, {} from 'x'` — carries no specifiers, so tsv drops it (and its comma)
to match prettier: `import a from 'x'`. The binding→`from` gap is then handled
exactly like a plain default import, so a comment anywhere in the dropped region
collapses to one position before `from` (`import b /* c */ from 'y'`) — a plain
match, and idempotent (the empty group is dropped in a single pass rather than
the comment being duplicated each reformat). A *bare* empty group with no
binding (`import {} from 'x'`) is preserved — see [basic](../basic/).

The `unformatted_*` variants cover the three comment positions around the
dropped group (before the comma, after it, after the braces); each normalizes to
input under both formatters in one pass.
