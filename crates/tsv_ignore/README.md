# tsv_ignore

gitignore-faithful hierarchical ignore matching (`.gitignore`, `.formatignore`, `.prettierignore`)
behind `tsv format`'s file discovery — part of [tsv](https://github.com/fuzdev/tsv).

The matcher is pure: locating the ignore files, reading them and walking directories are the
caller's jobs. Its contract is `git check-ignore`.

API docs: [docs.rs/tsv_ignore](https://docs.rs/tsv_ignore).

Minimum supported Rust version: 1.94. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: [0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE)
