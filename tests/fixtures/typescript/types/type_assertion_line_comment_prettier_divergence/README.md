# Line comments in an angle-bracket type assertion's cast

Companion to the block-comment sibling
[`../type_assertion_comment`](../type_assertion_comment/),
covering **line** comments in the cast `<Type>`. A `//` runs to end-of-line, so
it forces the cast to break (a block comment can hug inline) and exposes two
relocations the block form doesn't.

Four positions, exercised across three casts:

- **own-line after `<`** (`a`: `<⏎\t// c⏎\tstring`) — both formatters keep it on
  its own line. **Match.**
- **trailing the type, before `>`** (`a`/`b`: `string // d⏎>`) — both keep it
  trailing the type. **Match.**
- **trailing `<`** (`b`: `< // c`) — tsv keeps the comment on the `<` line;
  prettier moves it to its own line. **Divergence**, consistent with every other
  open-delimiter trailing line comment (cf. the type-parameter `<`).
- **trailing `>`, before the expression** (`c`: `<string> // c`) — tsv keeps the
  comment after `>` leading the expression and drops the expression to a
  continuation line one indent in; prettier relocates it across `>` into the cast,
  trailing the type. **Divergence.**

## Formatter divergence (`_prettier`)

tsv treats the cast's `<` and `>` as semantic boundaries and preserves each
comment where the author wrote it; prettier relocates the trailing-`<` and
trailing-`>` comments (`output_prettier.svelte`). See
[conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment relocation (Angle-bracket type assertion).
