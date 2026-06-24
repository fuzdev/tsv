# ES2024 RegExp v-flag Set Operations

## Why tsv Diverges

Svelte's parser (based on acorn) does not yet support ES2024 v-flag set operations:

- **Set subtraction**: `[a-z--[aeiou]]`
- **Set intersection**: `[\p{Letter}&&\p{ASCII}]`
- **Nested character classes**: `[[0-9][a-f]]`
- **String literals**: `[\q{abc|def}]`

These are valid ES2024 syntax. Prettier accepts and formats them correctly.

## Status

- **tsv parser**: Full ES2024 v-flag support
- **Svelte/acorn**: `Invalid regular expression: Unterminated character class`
- **Prettier**: Formats correctly (source of truth for formatting)

## References

- [TC39 RegExp v flag proposal](https://github.com/tc39/proposal-regexp-v-flag)
- [MDN: Unicode sets](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Regular_expressions/Unicode_character_class_escape#unicode_sets_mode)

See [conformance_svelte.md](../../../../../../../docs/conformance_svelte.md) §TypeScript Corrections.
