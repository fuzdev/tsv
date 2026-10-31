# tsv_html

HTML element classification, whitespace rules, and the WHATWG named character reference table — part
of [tsv](https://github.com/fuzdev/tsv), published for tsv's Svelte support
([`tsv_svelte`](https://crates.io/crates/tsv_svelte)).

The crate root is reserved for answers that follow the HTML spec (today the optional-end-tag table
and the custom-element name class); the `svelte` module holds the answers Svelte's compiler and
prettier-plugin-svelte give, which tsv's Svelte support reproduces exactly. The long-term target is
a spec-faithful HTML library.

The public signatures follow 0.x semver like every tsv crate: a breaking change ships only in a
minor release. What the functions answer, and the wording of anything they return, may change in
any release, tracking what tsv needs; `#[doc(hidden)]` items are outside semver.

API docs: [docs.rs/tsv_html](https://docs.rs/tsv_html).

Minimum supported Rust version: 1.94; raising it ships in a minor release. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: `0BSD AND BSD-3-Clause` — tsv's own code is
[0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE), and the WHATWG HTML Living Standard's list
of named character references, compiled in as source code, is BSD-3-Clause. See
[NOTICE](https://github.com/fuzdev/tsv/blob/main/crates/tsv_html/NOTICE), which also ships in this
crate.
