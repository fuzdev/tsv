# tsv_discover

tsv's file-discovery policy (the build-output heuristic, safety-net pruning and the warnings) over
[`tsv_ignore`](https://crates.io/crates/tsv_ignore) — part of [tsv](https://github.com/fuzdev/tsv).

**No public API.** The crate exists for tsv's own tools — the `tsv` CLI and its WASM and N-API
bindings, which share its discovery decisions — and every item in it is hidden from the docs:
outside semver, free to change in any release. A tool that wants gitignore matching alone should
depend on `tsv_ignore`.

Minimum supported Rust version: 1.94; raising it ships in a minor release. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: [0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE)
