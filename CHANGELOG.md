# tsv changelog

Covers the npm packages published from this repo — `@fuzdev/tsv_format_wasm`,
`@fuzdev/tsv_parse_wasm`, and `@fuzdev/tsv_wasm`. All move together at the
`Cargo.toml [workspace.package]` version. Each `## Unreleased` section must be
non-empty and carry a `<!-- bump: patch|minor|major -->` marker; `deno task publish
--wetrun --bump <level>` requires `<level>` to match it, then stamps the section
(marker removed) into the released version's section and seeds a fresh empty
`## Unreleased` (reset to `bump: patch`) for the next cycle.

## Unreleased
<!-- bump: minor -->

- formatting is now non-configurable by design - tsv has no config files, CLI flags,
  or runtime options, and none will be added
  (this has no observable API changes because options had been deferred)
- `tsv format` directory discovery is now gitignore-aware
  ([gitignore syntax](https://git-scm.com/docs/gitignore#_pattern_format)), with two
  regimes keyed on `.git`. **Inside a git repo** it honors `.gitignore` hierarchically
  and repo-rooted like git, `.formatignore` hierarchically (one per directory, deeper
  wins) layered on top so its `!` can re-include a gitignore'd path, and a single
  repo-root `.prettierignore` (drop-in compat; a repo-root `.formatignore` shadows it).
  The repo root is a hard boundary — nothing above it is read — so `tsv format --check`
  is reproducible across machines. **Outside a git repo** `.gitignore` and
  `.prettierignore` are not read (as git itself does), and `.formatignore` is honored
  hierarchically from the filesystem root down, so a `~/.formatignore` is global config
  for loose files. The always-skipped safety nets are `.git`, `node_modules`, `.hg`,
  `.svn`, `.jj`; with no `.gitignore` in scope a built-in heuristic (hidden dirs +
  `dist`/`build`/`target`) is the fallback, overridden by an explicit `.formatignore`
  `!`. So a real source `build/` that isn't gitignored is now formatted (the old
  heuristic wrongly skipped it). An explicitly named file is always formatted.
  Formatting a subdirectory directly gives the same result as via an ancestor. A
  file-scope carve-out from the non-configurable stance, never a style option
- `tsv format` now warns (stderr, non-fatal — no effect on the exit code, stdout, or
  `--list`/`--check` output) when its build-output heuristic prunes a directory that an
  anchored `.formatignore` `!dir/<file>` re-include was targeting. Such a re-include is a
  silent no-op (git's parent-directory rule bars re-including a descendant of a pruned
  dir); the warning points at the escape that works — `!dir/` to admit the directory,
  then `dir/*` + `!dir/keep` to narrow it back down. A floating `!keep` (any depth) and a
  dir-level `!dir/` do not warn. Shared across the native CLI, the WASM CLI, and editors
  via the new `IgnoreStack::has_negation_under` matcher query
- `tsv format --list` prints the discovered in-scope files (one per line) without
  formatting — a read-only view of what `format` would touch after the ignore files
  are applied. Path mode only, mutually exclusive with `--check`; an empty scope is a
  valid answer (exit 0). Useful for debugging ignore-file scoping and for scripting
- the format-capable WASM packages (`@fuzdev/tsv_format_wasm`, `@fuzdev/tsv_wasm`)
  export an `IgnoreStack` class so editors and the JS CLI share the exact same
  gitignore-aware discovery rules as the native CLI
- support `format-ignore` as an alias to `prettier-ignore`
  (along with `format-ignore-start` and `format-ignore-end` for templates)
- various conformance fixes to the formatter and parser
- numerous new Prettier divergences including uniform indentation on continuations
- object destructuring patterns in Svelte blocks now hug their braces,
  consistent with `bracketSpacing: false` (which is not respected by prettier-plugin-svelte)
- values in Svelte block tags now consistently use TS printing paths,
  fixing oversights prettier-plugin-svelte
- reduce allocations using `SmallVec` and memoizations

## 0.1.0

- init
- add `@fuzdev/tsv_wasm` — the full tool (format + parse) in one package, with a
  `tsv` bin (`format` + `parse` subcommands mirroring the native CLI's flags and
  exit codes; single-threaded WASM — `--jobs` is accepted and ignored)
- slim `@fuzdev/tsv_parse_wasm` to parse-only (the `format_*` exports and their
  printers move to `@fuzdev/tsv_wasm`; wasm drops from ~2.9 MB to ~1.7 MB raw,
  ~895 KB to ~515 KB gzipped)
