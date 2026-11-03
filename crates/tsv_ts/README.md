# tsv_ts

TypeScript and JavaScript formatting, type stripping, import records, and parsing to
acorn-typescript's wire JSON — part of [tsv](https://github.com/fuzdev/tsv)
([tsv.fuz.dev](https://tsv.fuz.dev/)).

- `format` formats one source; a `Formatter` formats many on one thread, keeping its arenas
  warm between calls.
- `strip` erases the types, keeping every surviving character at its line and column.
- `parse_imports` lists the modules a source requests — static, re-export, dynamic,
  `import x = require()` — with their spans, decoded specifiers and type-only verdicts.
- `parse_to_json` parses to the wire JSON AST (the default `json` feature).
- `run_with_stack` runs a call on a thread reserving `STACK_SIZE`, for a thread whose stack
  may be too small for the deepest input tsv accepts.

```rust
use tsv_ts::{Formatter, format, parse_imports, strip};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(format("const x:number=42")?, "const x: number = 42;\n");
    let mut formatter = Formatter::new(); // warm arenas, for many sources on one thread
    assert_eq!(formatter.format("let y=1")?, "let y = 1;\n");
    assert_eq!(strip("let x: number = 1;")?.code, "let x         = 1;");
    let source = "import type {T} from './t';";
    let imports = parse_imports(source)?;
    assert_eq!(imports.records[0].specifier_value(source).as_deref(), Some("./t"));
    Ok(())
}
```

This release exposes no AST: parsing returns the wire JSON — the AST as JSON in the canonical
parser's shape (acorn-typescript's), its `start` / `end` UTF-16 offsets, span-only by default (no
`loc`; the `locations` feature adds it), unlike an error's `span()`, a UTF-8 byte range.

JSX and TSX are not supported: JSX is never read as JSX, so a source holding it fails
to parse or is misread.

The formatter is non-configurable by design: Prettier's style with `printWidth` 100, tabs, single
quotes, and no trailing commas. Formatting output is not frozen: it changes between releases as
formatting bugs are fixed.

API docs: [docs.rs/tsv_ts](https://docs.rs/tsv_ts).

Minimum supported Rust version: 1.94; raising it ships in a minor release. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: [0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE)
