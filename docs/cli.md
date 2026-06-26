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
  - Commands: `check`, `compare`, `ast_diff`, `line_width`, `canonical_parse`, `format_prettier`, `fixture_init`, `fixtures_validate`, `fixtures_update`, `fixtures_update_parsed`, `fixtures_update_formatted`, `fixtures_audit`, `ts_fixture_audit`, `conformance_audit`, `swallow_audit`, `scan_audit`, `authoring_audit`, `metrics`, `profile`, `json_profile`, `test262`

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

Versions are pinned (exact) in `crates/tsv_debug/src/deno/sidecar.ts` — the source of truth; they are not repeated here. `benches/js/package.json` pins the same versions independently for the bench harness; keep the two in sync.

## Input Handling

All content-processing commands support three input methods:

- **File path**: `command <file>` - Auto-detects parser/type from extension
- **Content**: `command --content <string> --parser <type>` - Requires explicit `--parser svelte|typescript|css`
- **Stdin**: `command --stdin --parser <type>` - Requires explicit `--parser svelte|typescript|css`

Implemented in `tsv_cli/src/cli/input.rs`

## Multi-File Formatting

`tsv format` accepts any mix of files and directories:

- **Discovery**: directories recurse over `.ts`/`.svelte`/`.css` (compound forms like `.svelte.ts` included). The **safety nets** `.git`, `node_modules`, `.hg`, `.svn`, `.jj` are always pruned. Explicit args are trusted: file args are included regardless of extension (and regardless of the ignore files), and an ignored/hidden dir passed explicitly recurses. Symlinks inside directories are not followed; pass them explicitly.
- **Ignore files (two regimes, keyed on `.git`)**: for each directory root, the **format root** — the scope boundary, derived from the argument, never the cwd — is the **repo root** inside a git tree (a hard stop where the upward walk ends, so nothing above the repo is read and `--check` is reproducible) or the **filesystem root** outside one. The regime is decided **once at the target root**, and any ignored directory is pruned (its whole subtree is skipped).
  - **Inside a repo**, discovery honors, relative to the repo root:
    - **`.gitignore`** — hierarchical and repo-rooted exactly like git ([gitignore syntax](https://git-scm.com/docs/gitignore#_pattern_format), matched against `git check-ignore` on case-sensitive filesystems). This goes beyond Prettier, which reads only one `.gitignore` and one `.prettierignore`, both relative to its own directory (the cwd by default), and ignores nested ones entirely.
    - **`.formatignore`** — hierarchical (one per directory from the repo root down, deeper wins), applied after `.gitignore` so its `!` can re-include a gitignore'd path (subject to git's parent-directory rule).
    - a single repo-root **`.prettierignore`** — drop-in compat, shadowed by the *presence* of a repo-root `.formatignore` (used alone when present, even if that `.formatignore` is present-but-unreadable — a read error can't silently demote tsv's native file to prettier's).
  - **Outside a repo**, `.gitignore` and `.prettierignore` are not read (as git itself does); only `.formatignore` governs, hierarchically from the filesystem root down — so a `~/.formatignore` is global config for loose files. A `.prettierignore` in the **target root** (the directory tsv was pointed at, where prettier would have read it) raises a non-fatal stderr warning — rename it to `.formatignore`, or `git init` — without changing what gets formatted. The warning is bounded to the target root: a nested `.prettierignore` is not read by prettier either, and an ancestor of a subdirectory target has no repo boundary to anchor on.
  - **Heuristic fallback**: a `.gitignore` in scope is **authoritative** and turns the heuristic off; with no `.gitignore`, the heuristic — hidden directories plus `dist`/`build`/`target` — is the fallback "not source" guess, except that an explicit tsv-layer `!` re-include overrides it.
  - **Re-include idiom**: to selectively re-include under a pruned (or otherwise ignored) directory, re-include the directory itself first — `!dist/` admits the whole directory, then `dist/*` + `!dist/keep.ts` narrows it back to just the files you want. A bare `!dist/keep.ts` (without `!dist/`) is a **no-op** — the heuristic prunes `dist` before descending, mirroring git's parent-directory rule (a gitignored `dist/` likewise blocks a later `!dist/keep.ts`). tsv emits a **stderr warning** for this case (non-fatal — no effect on the exit code, stdout, or `--list`/`--check` output), pointing at the `!dir/` escape.
  - **Subdirectory invocation**: because the boundary is found by walking up, the repo-root rules apply even from a subdirectory, and formatting a subdirectory directly gives the same result as formatting it via an ancestor. But a tree that *contains* repos (a non-repo directory with `.git` subdirectories below it) does not honor the inner repos' `.gitignore`s — run tsv per repo.
  - **Unreadable ignore files**: a `.gitignore`/`.formatignore`/`.prettierignore` that is present but can't be read (invalid UTF-8 — reading is strict UTF-8 on both the native and WASM CLIs — or a permission error) is **not** silently treated as absent: tsv emits a non-fatal stderr warning and drops that file's rules (so an unreadable `.gitignore` also leaves the build-output heuristic *on* for its subtree). A file that genuinely isn't there, or is deleted between the directory listing and the read, stays silent. This is also a `--check` reproducibility hazard — surfacing it is the point.
  - **`--check` reproducibility** assumes the ignore files are **committed**: a local/uncommitted `.formatignore` or `.prettierignore` (or git's unread `.git/info/exclude` / `core.excludesFile`) makes a clean CI checkout disagree.
  - **Shared by construction**: the matcher is the `tsv_ignore` crate's `IgnoreStack`; the per-directory prune/descend policy (heuristic, safety nets, the shadow warning) is the `tsv_discover` crate's verdict. The WASM CLI and editors call into the same two crates, so all three surfaces agree rather than hand-mirroring the logic. See `cli/discover.rs`.
- **Fail-fast args, isolated traversal**: path args that don't resolve to a file or directory fail the whole run before anything is written (every bad arg reported); traversal errors below a valid root (e.g. an unreadable subdirectory) report to stderr and discovery continues.
- **Deduplication**: with multiple path args, overlapping spellings of the same file (`src` vs `./src`, absolute vs relative, symlink aliases) dedupe by canonical path, keeping the first spelling in sorted order. A single root can't produce duplicates, so the canonicalization cost is skipped.
- **In-place writes**: files are rewritten only when output differs (no mtime churn). `--content`/`--stdin` keep printing to stdout.
- **`--check`**: lists files that would change without writing; exits 1 if any would. For CI. Also works with `--content`/`--stdin` (nothing printed to stdout; the exit code is the API) for editor integrations.
- **`--list`**: prints the discovered in-scope files (one per line) without formatting — a read-only view of the set `format` would touch, after the ignore files are applied. Path mode only (errors with `--content`/`--stdin`) and mutually exclusive with `--check`. Unlike the format action, an empty scope is a valid answer (exit 0, no output) rather than the "no supported files" error; traversal errors still exit 2. Useful for debugging ignore-file scoping and for scripting over the set.
- **Parallelism**: files format concurrently on `std::thread::scope` workers pulling from a shared atomic next-index counter over the sorted file list — dynamic load balancing with no thread-pool dependency. `--jobs N` overrides the worker count (default: available parallelism); path mode only, an error with `--content`/`--stdin`.
- **Error isolation**: a per-file read/parse/write error (or panic, caught via `catch_unwind` — effective only in builds with `panic = "unwind"`; release uses `panic = "abort"`) reports to stderr and processing continues.
- **Deterministic reporting**: changed paths print to stdout in sorted-path order regardless of completion order; errors (traversal and per-file) and the summary line go to stderr.
- **Exit codes**: 0 clean, 1 would-change (`--check` only), 2 errors.
