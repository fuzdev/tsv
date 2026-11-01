# tsv_html

HTML spec data and the answers the HTML spec gives — today the WHATWG named character references
table, matched as the tokenizer matches it — part of [tsv](https://github.com/fuzdev/tsv),
published for tsv's Svelte support ([`tsv_svelte`](https://crates.io/crates/tsv_svelte)). The
long-term target is a spec-faithful HTML library.

The public signatures follow 0.x semver like every tsv crate: a breaking change ships only in a
minor release. What the functions answer may change in any release, tracking the spec and what tsv
needs.

API docs: [docs.rs/tsv_html](https://docs.rs/tsv_html).

Minimum supported Rust version: 1.94; raising it ships in a minor release. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: `0BSD AND BSD-3-Clause` — tsv's own code is
[0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE), and the WHATWG HTML Living Standard's list
of named character references, compiled in as source code, is BSD-3-Clause. See
[NOTICE](https://github.com/fuzdev/tsv/blob/main/crates/tsv_html/NOTICE), which also ships in this
crate.
