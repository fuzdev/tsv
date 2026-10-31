# tsv_discover

tsv's file-discovery policy (the build-output heuristic, safety-net pruning and the warnings) over
[`tsv_ignore`](https://crates.io/crates/tsv_ignore) — part of [tsv](https://github.com/fuzdev/tsv).

**Published for tsv's own tools**: the `tsv` CLI and its JS bindings share its discovery decisions.
A tool that wants gitignore matching alone should depend on `tsv_ignore`.

The public signatures follow 0.x semver like every tsv crate: a breaking change ships only in a
minor release. The verdicts and the warnings' wording follow `tsv format`'s behavior and may change
in any release; `#[doc(hidden)]` items are outside semver.

API docs: [docs.rs/tsv_discover](https://docs.rs/tsv_discover).

Minimum supported Rust version: 1.94; raising it ships in a minor release. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: [0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE)
