# tsv_svelte

Svelte formatting and parsing to the wire JSON of Svelte's modern parser (TypeScript in `<script>`,
CSS in `<style>`) — part of [tsv](https://github.com/fuzdev/tsv)
([tsv.fuz.dev](https://tsv.fuz.dev/)).

- `format` formats one source; a `Formatter` formats many on one thread, keeping its arenas
  warm between calls.
- `parse_to_json` parses to the wire JSON AST (the default `json` feature).
- `run_with_stack` runs a call on a thread reserving `STACK_SIZE`, for a thread whose stack
  may be too small for the deepest input tsv accepts.

```rust
use tsv_svelte::{Formatter, format};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(format("<p  class=\"a\">x</p>")?, "<p class=\"a\">x</p>\n");
    let mut formatter = Formatter::new(); // warm arenas, for many sources on one thread
    assert_eq!(formatter.format("<script>let y=1</script>")?, "<script>\n\tlet y = 1;\n</script>\n");
    Ok(())
}
```

This release exposes no AST: parsing returns the wire JSON — the AST as JSON in the canonical
parser's shape (Svelte's modern parser's), its `start` / `end` UTF-16 offsets, span-only by default
(no `loc`; the `locations` feature adds it), unlike an error's `span()`, a UTF-8 byte range.

The formatter is non-configurable by design: Prettier's style with `printWidth` 100, tabs, single
quotes, and no trailing commas. Formatting output is not frozen: it changes between releases as
formatting bugs are fixed.

API docs: [docs.rs/tsv_svelte](https://docs.rs/tsv_svelte).

Minimum supported Rust version: 1.94; raising it ships in a minor release. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: [0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE)
