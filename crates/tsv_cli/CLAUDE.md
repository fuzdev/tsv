# tsv_cli

> Production CLI for `tsv` — pure Rust, no Deno

Ships the `tsv` binary (defined in `Cargo.toml` `[[bin]]`). Depends on `tsv_ts`, `tsv_css`, and `tsv_svelte` (each with `convert` features), `tsv_lang` (the `estimated_ast_arena_capacity` parse-arena pre-size), `tsv_ignore` (discovery ignore-file matching) and `tsv_discover` (the per-directory prune/descend policy over it), plus `argh`, `serde`, and `serde_json` — nothing else. `tsv_debug` depends on this crate and reuses its `cli::` library surface for its own command set; keep that surface stable.

For end-user invocation syntax see the [root CLAUDE.md §CLI Usage](../../CLAUDE.md#cli-usage---parse--format). For pattern background see [`docs/cli.md`](../../docs/cli.md).

## Public API

**Binary (`tsv`)** — `main.rs` parses `cli::TopLevel` via `argh::from_env()` and dispatches. Two commands:

- `tsv parse <file|--content|--stdin> [--pretty] [--parser <type>]` — emits public JSON AST to stdout
- `tsv format <paths...|--content|--stdin> [--parser <type>] [--check] [--list] [--jobs <n>]` — formats paths in place (writes only when output differs); `--content`/`--stdin` print to stdout. `--list` prints the discovered in-scope files without formatting (path mode only; mutually exclusive with `--check`; empty scope exits 0)

**Library (`tsv_cli::cli`, `tsv_cli::json_utils`)** — re-exported via `lib.rs` for `tsv_debug` to build its own commands on the same plumbing. The stable items:

- `cli::input::{Input, InputArgs, ParserType}` — unified file / `--content` / `--stdin` plumbing + extension auto-detect; `ParserType::name()` is the canonical lowercase name (`--parser` values, sidecar tool keys)
- `cli::format_source::format_source()` — the in-process parse+format entry point keyed by `ParserType`; the single definition of "format with tsv" shared by the `format` command and `tsv_debug` (`compare`, `ast_diff`, fixture validation)
- `cli::discover::discover_files()` — file/dir expansion with the formattable-extension filter and gitignore-aware ignore evaluation via `tsv_ignore::IgnoreStack` (full two-regime rules: [root CLAUDE.md §Configuration](../../CLAUDE.md#configuration)). Returns `Discovered { files, errors, warnings }` (non-fatal traversal errors; `warnings` are stderr-only diagnostics — e.g. the heuristic-shadow no-op — that don't touch the exit code or stdout), `Err` per unresolvable arg. The FS walk + format-root resolution + ignore-file reading live here; the **pure per-entry verdict** — safety nets, the build-output heuristic + its shadow-warning text, the formattable-extension check — is delegated to `tsv_discover` (`classify_dir`/`should_format_file`), shared with the WASM CLI and the VS Code extension so all three agree by construction
- `json_utils::to_json_with_tabs()` — tab-indented `serde_json` (matches workspace style)

## Distinctives

- **argh for arg parsing.** Each command is a `FromArgs` struct under `cli/commands/`; `cli::TopLevel`/`Subcommand` is the dispatch enum. argh has no struct-flattening attribute, so the shared input fields (`--content`, `--stdin`, `--parser`, file path) are declared per command and assembled into `cli::input::InputArgs` for resolution.
- **Three input modes per command** — file path(s), `--content <str>` (requires explicit `--parser`), `--stdin` (requires explicit `--parser`). `parse` accepts `--parser` on a file path as an override; `format` rejects it (paths always trust the extension). The "preferred for agents" path is `--content`; `--stdin` is supported but discouraged.
- **`format` is multi-file and parallel.** Positional args accept files and directories; directories recurse over `.ts`/`.svelte`/`.css` with gitignore-aware discovery (`cli/discover.rs`) — safety nets always pruned, a cwd-independent **format root** (repo root in a git tree, else the filesystem root), and `.gitignore`/`.formatignore`/`.prettierignore` honored per the [two-regime rules](../../CLAUDE.md#configuration); an **explicit file argument bypasses the ignore files** (they govern discovery, and the caller named that file). Because the format root is cwd-independent, a target directory formats the same however its path is spelled, wherever tsv runs, and whether targeted directly or via an ancestor. Args that don't resolve fail the run upfront (all reported, nothing written); traversal errors below a valid root report and continue. With multiple roots, overlapping spellings of the same file dedupe by canonical path (skipped for a single root — symlinks aren't followed, so no duplicates are possible). Workers pull from a shared atomic next-index counter over the sorted file list (std-only, no rayon); results are reported in sorted-path order regardless of completion order. Per-file errors (and panics, via `catch_unwind` — effective in dev/corpus profiles only since release uses `panic = "abort"`) report and continue.
- **`format` exit codes: 0 clean, 1 would-change (`--check`), 2 errors.** Changed paths go to stdout (greppable), errors and the summary line to stderr. `--check` also works with `--content`/`--stdin` (exit code only, nothing on stdout); `--jobs` is path-mode only and errors otherwise. Other commands keep 0/1.
- **`--pretty` is `parse`-only.** Compact JSON is the default to keep agent/CI output greppable; pretty mode uses `to_json_with_tabs` (not serde's 2-space default) so output matches workspace formatting.
- **Convert features are mandatory.** `tsv_*` deps are declared with `features = ["convert"]` because `parse` always goes through the convert layer (`convert_ast_json_string` for compact output, `convert_ast_json` for `--pretty`); the internal AST is never serialized.
- **Shared with `tsv_debug`.** `tsv_debug` commands import `tsv_cli::cli::input`, `tsv_cli::cli::format_source`, and `tsv_cli::json_utils`. Treat `pub` items in those modules as a contract — moving or renaming them ripples through ~15 debug commands.
