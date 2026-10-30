# tsv_ts

TypeScript and JavaScript formatting, type stripping, and parsing to acorn-typescript's wire JSON —
part of [tsv](https://github.com/fuzdev/tsv) ([tsv.fuz.dev](https://tsv.fuz.dev/)).

- `format` formats one source; a `Formatter` formats many on one thread, keeping its arenas
  warm between calls.
- `strip` erases the types, keeping every surviving character at its line and column.
- `parse_to_json` parses to the wire JSON AST (the default `json` feature).
- `run_with_stack` runs a call on a thread reserving `STACK_SIZE`, for a thread whose stack
  may be too small for the deepest input tsv accepts.

This release exposes no AST: parsing returns the wire JSON.

The formatter is non-configurable by design: Prettier's style with `printWidth` 100, tabs, single
quotes, and no trailing commas.

API docs: [docs.rs/tsv_ts](https://docs.rs/tsv_ts).

Minimum supported Rust version: 1.94. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: [0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE)
