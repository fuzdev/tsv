# Comment Fixtures

This directory contains fixtures for **universal comment behavior** that applies across all syntactic contexts.

## What Belongs Here

- **Basic syntax**: Block comments (`/* */`), line comments (`//`), nesting, consecutive comments
- **Universal formatting rules**: Blank line preservation, trailing comments, inline positioning
- **Cross-cutting edge cases**: Patterns spanning multiple statement/declaration types (e.g., `declaration_head_body_comment` tests function, class, interface, enum, namespace)
- **JSDoc**: Basic JSDoc patterns that aren't tied to specific declarations
- **Program-level**: Comments at file start/end, empty statements
- **Encoding**: UTF-8 handling, special characters in comments

## What Does NOT Belong Here

Feature-specific comment behavior belongs with that feature:

| Feature | Location |
|---------|----------|
| Chain comments | `expressions/calls/chained/` |
| Arrow function comments | `expressions/arrow/` |
| Class member comments | `statements/class/` |
| Loop comments (for/while/do-while) | `statements/for/`, `statements/while/`, `statements/do_while/` |
| Control flow comments (if/switch/try) | `statements/if/`, `statements/switch/`, `statements/try/` |
| Type comments | `types/comments/` |
| Function declaration comments | `declarations/function/` |

## Naming Convention

Use `*_comment` suffix when the fixture primarily tests comment behavior within a feature directory.
