# import_equals_type

`import type X = require(...)` is refused (`UnsupportedKind::ImportEquals`) like a value import-equals; the span is the declaration. ts-blank-space refuses it, and tsc's `erasableSyntaxOnly` rejects it (TS1294).

## Divergence

Node blanks the whole declaration, since `import type` makes it type-only. tsv follows ts-blank-space and tsc, which treat every import-equals as syntax type blanking does not erase.
