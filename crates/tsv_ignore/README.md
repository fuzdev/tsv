# tsv_ignore

gitignore-faithful hierarchical ignore matching (`.gitignore`, `.formatignore`, `.prettierignore`)
behind `tsv format`'s file discovery — part of [tsv](https://github.com/fuzdev/tsv).

The matcher is pure: locating the ignore files, reading them and walking directories are the
caller's jobs. Its contract is `git check-ignore --no-index` (no index: a tracked file a rule
matches is ignored), less the differences its docs list.

```rust
use tsv_ignore::{IgnoreStack, PathKind};

let mut stack = IgnoreStack::new();
stack.push_gitignore("", "build/\n*.log\n"); // the root .gitignore
stack.push_formatignore("", "!keep.log\n"); // read after every .gitignore
assert!(stack.is_ignored("build/out.js", PathKind::File));
assert!(!stack.is_ignored("keep.log", PathKind::File));
```

API docs: [docs.rs/tsv_ignore](https://docs.rs/tsv_ignore).

Minimum supported Rust version: 1.94; raising it ships in a minor release. Release notes:
[CHANGELOG.md](https://github.com/fuzdev/tsv/blob/main/CHANGELOG.md).

License: [0BSD](https://github.com/fuzdev/tsv/blob/main/LICENSE)
