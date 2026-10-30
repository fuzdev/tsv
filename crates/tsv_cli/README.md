# tsv_cli

The `tsv` command-line tool: format, parse and type-strip TypeScript/JS, CSS and Svelte — part of
[tsv](https://github.com/fuzdev/tsv) ([tsv.fuz.dev](https://tsv.fuz.dev/)).

Prebuilt binaries ship in the `@fuzdev/tsv` npm package (`npm i -D @fuzdev/tsv`) and on each [GitHub
Release](https://github.com/fuzdev/tsv/releases).

From source: `cargo install --locked tsv_cli`. A source build does not get the release profile the
prebuilt binaries ship with (LTO, `panic = "abort"`): it runs slower, and an internal panic while
formatting files is reported as that file's error instead of aborting the run.

Usage: [docs/cli.md](https://github.com/fuzdev/tsv/blob/main/docs/cli.md). The library target is the
CLI's internals and makes no stability promise.

Minimum supported Rust version: 1.94. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: [0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE)
