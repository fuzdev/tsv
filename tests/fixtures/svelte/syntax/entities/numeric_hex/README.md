# Numeric Hex Entities Test

Tests hexadecimal numeric character references in HTML/Svelte templates.

## Note on Uppercase X

The HTML5 spec allows both lowercase (`&#x41;`) and uppercase (`&#X41;`) hex entity syntax.

- **tsv**: Supports both per HTML5 spec
- **Svelte**: Only decodes lowercase `&#x41;`, treats uppercase `&#X41;` as literal text

Uppercase `&#X` is excluded from this fixture to match Svelte's behavior in tests, but the tsv decoder correctly handles both forms per spec.
