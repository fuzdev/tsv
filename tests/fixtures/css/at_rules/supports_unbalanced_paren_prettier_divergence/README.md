# supports_unbalanced_paren_prettier_divergence

`@supports (margin: 0))` — an `@supports` prelude with an unbalanced closing
paren. Per CSS Syntax 3 an at-rule's prelude is always consumed as component
values; a prelude that isn't a valid `<supports-condition>` makes the rule
evaluate false but does **not** fail parsing. Svelte's `parseCss` accepts it and
stores the raw prelude (`(margin: 0))`), and tsv matches: it structures the valid
`(margin: 0)` part, then re-emits the whole prelude verbatim once the stray `)`
proves it isn't a structurable condition (the conditional-at-rule raw fallback,
shared with `@container`). Stable under tsv.

Prettier's postcss parser **throws** on it:

```
Unbalanced parenthesis
```

so it can't serve as a formatting oracle. `prettier_rejects.txt` pins the error;
rule F6 live-verifies that prettier still rejects the input. The non-condition
preludes prettier *does* accept verbatim (`@supports margin: 0`, `@supports
[margin: 0]`) are the regular fixture
[supports_non_condition_prelude](../supports_non_condition_prelude/); the
`@container` counterpart is
[container_non_condition_prelude](../container_non_condition_prelude/).

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md#prettier-rejects-valid-input)
§Prettier rejects valid input.
