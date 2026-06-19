# CLI Architecture

## Command Pattern

The CLI uses [argh](https://crates.io/crates/argh) for declarative arg parsing:

- Each command is a `FromArgs` struct in its own module under `src/cli/commands/`
- `cli::TopLevel` holds the `Subcommand` enum; `main.rs` calls `argh::from_env()` and dispatches
- argh has no struct-flattening attribute, so the shared input fields (`--content`, `--stdin`, `--parser`, file path) are declared per command and assembled into `cli::input::InputArgs` for resolution

**Adding Commands**: Create `src/cli/commands/newcmd.rs` with a `FromArgs` struct and a `run()` method, add a variant to `Subcommand` in `cli/mod.rs`.

## Shared Infrastructure

`tsv_cli` exports CLI infrastructure as a library, reused by `tsv_debug` for consistent UX:

- Input handling (file, `--content`, `--stdin`) — `cli/input.rs`
- File/directory discovery with extension filter, gitignore-aware ignore evaluation (hierarchical `.gitignore` + repo-root `.formatignore`/`.prettierignore`), and the non-git heuristic fallback — `cli/discover.rs`
- JSON utilities (tab-indented serialization) — `json_utils.rs`

## Binary Structure

- **`tsv` (production)**: Pure Rust, no external tool dependencies
  - Crates: `tsv_cli`
  - Commands: `parse`, `format`
- **`tsv` npm bin (WASM)**: `crates/tsv_wasm/npm/cli.js`, shipped in
  `@fuzdev/tsv_wasm` — a hand-written Node mirror of this CLI's contract
  (subcommands, flags, exit codes, output streams, traversal rules;
  single-threaded — `--jobs` is accepted for drop-in parity and ignored).
  Behavioral changes to `format`/`parse` here must be mirrored there and
  in `scripts/test_npm.ts`'s CLI tests.
- **`tsv_debug` (development)**: Uses embedded Deno sidecar for external tools
  - Reuses `tsv_cli` infrastructure
  - Commands: `check`, `compare`, `ast_diff`, `line_width`, `canonical_parse`, `format_prettier`, `fixture_init`, `fixtures_validate`, `fixtures_update`, `fixtures_update_parsed`, `fixtures_update_formatted`, `fixtures_audit`, `ts_fixture_audit`, `conformance_audit`, `metrics`, `profile`, `json_profile`, `test262`

### External Tools (via Embedded Deno Sidecar)

`tsv_debug` calls these external tools via an embedded Deno sidecar (spawned lazily on first use; bulk commands spawn a small pool of sidecar processes — see `crates/tsv_debug/CLAUDE.md`):

1. **prettier** + **prettier-plugin-svelte**
   - Used by: `compare`, `format_prettier`, fixture management
   - Purpose: Format code, compare outputs, validate formatter behavior

2. **svelte**
   - Used by: `canonical_parse`, `ast_diff`, fixture management
   - Purpose: Parse Svelte code with official compiler

3. **acorn** + **@sveltejs/acorn-typescript**
   - Used by: `canonical_parse`, `ast_diff`, fixture management
   - Purpose: Parse TypeScript code (matches Svelte's TS parser)

Versions are pinned (exact) in `crates/tsv_debug/src/deno/sidecar.ts` — the source of truth; they are not repeated here. `benches/deno/deno.json` pins the same versions independently for the bench harness; keep the two in sync.

## Input Handling

All content-processing commands support three input methods:

- **File path**: `command <file>` - Auto-detects parser/type from extension
- **Content**: `command --content <string> --parser <type>` - Requires explicit `--parser svelte|typescript|css`
- **Stdin**: `command --stdin --parser <type>` - Requires explicit `--parser svelte|typescript|css`

Implemented in `tsv_cli/src/cli/input.rs`

## Multi-File Formatting

`tsv format` accepts any mix of files and directories:

- **Discovery**: directories recurse over `.ts`/`.svelte`/`.css` (compound forms like `.svelte.ts` included). The **safety nets** `.git`, `node_modules`, `.hg`, `.svn`, `.jj` are always pruned. Explicit args are trusted: file args are included regardless of extension (and regardless of the ignore files), and an ignored/hidden dir passed explicitly recurses. Symlinks inside directories are not followed; pass them explicitly.
- **Ignore files (two regimes, keyed on `.git`)**: for each directory root, the **format root** (the scope boundary, derived from the argument — the cwd never participates) is the **repo root** inside a git tree (a hard stop where the upward walk ends, so nothing above the repo is read and `--check` is reproducible) or the **filesystem root** outside one. **Inside a repo**, discovery honors **`.gitignore`** hierarchically and repo-rooted exactly like git ([gitignore syntax](https://git-scm.com/docs/gitignore#_pattern_format), matched against `git check-ignore` on case-sensitive filesystems); **`.formatignore`** hierarchically (one per directory from the repo root down, deeper wins) applied after `.gitignore` so its `!` can re-include a gitignore'd path (subject to git's parent-directory rule); and a single repo-root **`.prettierignore`** (drop-in compat; a repo-root `.formatignore` shadows it). **Outside a repo**, `.gitignore` and `.prettierignore` are not read (as git itself does) and only `.formatignore` governs, hierarchically from the filesystem root down — so a `~/.formatignore` is global config for loose files. When a `.gitignore` is in scope it is **authoritative** and the heuristic below is off; with no `.gitignore` the heuristic — hidden directories plus `dist`/`build`/`target` — is the fallback "not source" guess, except that an explicit tsv-layer `!` re-include overrides it. **To selectively re-include under a pruned (or otherwise ignored) directory, re-include the directory itself first**: `!dist/` admits the whole directory, then `dist/*` + `!dist/keep.ts` narrows it back to just the files you want. A bare `!dist/keep.ts` (without `!dist/`) is a **no-op** — the heuristic prunes `dist` before descending, mirroring git's parent-directory rule (a gitignored `dist/` likewise blocks a later `!dist/keep.ts`). tsv emits a **stderr warning** (non-fatal — no effect on the exit code, stdout, or `--list`/`--check` output) when the heuristic prunes a directory that an anchored tsv-layer `!` was trying to re-include into, pointing at this `!dir/` escape. Ignored directories are pruned (whole subtree skipped). Because the boundary is found by walking up, the repo-root rules apply even from a subdirectory, and formatting a subdirectory directly gives the same result as formatting it via an ancestor. The regime is decided **once at the target root**, so pointing tsv at a tree that *contains* repos (a non-repo directory with `.git` subdirectories below it) does not honor the inner repos' `.gitignore`s — run tsv per repo. The `--check` reproducibility guarantee assumes the ignore files are **committed**: a local/uncommitted `.formatignore` or `.prettierignore` (or git's unread `.git/info/exclude` / `core.excludesFile`) makes a clean CI checkout disagree. Shared with the WASM CLI / editors via the `tsv_ignore` crate's `IgnoreStack` (the matcher) and the `tsv_discover` crate's verdict (the per-directory prune/descend policy — heuristic, safety nets, the shadow warning), so all three surfaces agree by construction rather than by hand-mirrored logic. See `cli/discover.rs`.
- **Fail-fast args, isolated traversal**: path args that don't resolve to a file or directory fail the whole run before anything is written (every bad arg reported); traversal errors below a valid root (e.g. an unreadable subdirectory) report to stderr and discovery continues.
- **Deduplication**: with multiple path args, overlapping spellings of the same file (`src` vs `./src`, absolute vs relative, symlink aliases) dedupe by canonical path, keeping the first spelling in sorted order. A single root can't produce duplicates, so the canonicalization cost is skipped.
- **In-place writes**: files are rewritten only when output differs (no mtime churn). `--content`/`--stdin` keep printing to stdout.
- **`--check`**: lists files that would change without writing; exits 1 if any would. For CI. Also works with `--content`/`--stdin` (nothing printed to stdout; the exit code is the API) for editor integrations.
- **`--list`**: prints the discovered in-scope files (one per line) without formatting — a read-only view of the set `format` would touch, after the ignore files are applied. Path mode only (errors with `--content`/`--stdin`) and mutually exclusive with `--check`. Unlike the format action, an empty scope is a valid answer (exit 0, no output) rather than the "no supported files" error; traversal errors still exit 2. Useful for debugging ignore-file scoping and for scripting over the set.
- **Parallelism**: files format concurrently on `std::thread::scope` workers pulling from a shared atomic next-index counter over the sorted file list — dynamic load balancing with no thread-pool dependency. `--jobs N` overrides the worker count (default: available parallelism); path mode only, an error with `--content`/`--stdin`.
- **Error isolation**: a per-file read/parse/write error (or panic, caught via `catch_unwind` — effective only in builds with `panic = "unwind"`; release uses `panic = "abort"`) reports to stderr and processing continues.
- **Deterministic reporting**: changed paths print to stdout in sorted-path order regardless of completion order; errors (traversal and per-file) and the summary line go to stderr.
- **Exit codes**: 0 clean, 1 would-change (`--check` only), 2 errors.
