# tsv_lang

**Support crate for tsv** — no stability promise; depend on
[`tsv_ts`](https://crates.io/crates/tsv_ts), [`tsv_css`](https://crates.io/crates/tsv_css) or
[`tsv_svelte`](https://crates.io/crates/tsv_svelte).

The shared foundation of tsv's language crates. Its public items — the error types (`Error`,
`ErrorKind`, `ErrorReport`, `Result`), the wire coordinates (`WireCoordinates`, `WirePoint`,
`LineRule`, `LeadingBom`) and the sized-stack pair (`STACK_SIZE`, `run_with_stack`) — are
re-exported by the language crates whose APIs use them; the rest is hidden plumbing, as are
the `_internal_*` features, which change in any release outside semver. Part of [tsv](https://github.com/fuzdev/tsv).

API docs: [docs.rs/tsv_lang](https://docs.rs/tsv_lang).

Minimum supported Rust version: 1.94. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: [0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE)
