# tsv_discover

> tsv's file-discovery **policy** — pure verdict functions over a `tsv_ignore`
> matcher. Zero external deps.

The single home of the decisions `tsv format`'s directory walk makes: the
always-pruned safety nets, the build-output heuristic, the formattable-extension
check, the heuristic-shadow warning, and the `.prettierignore`-outside-a-repo
warning. The three discovery surfaces — the
native CLI (`tsv_cli`), the WASM CLI (`crates/tsv_wasm/npm/cli.js`), and the VS
Code extension — call into it instead of reimplementing the decision, so they
agree **by construction** rather than by hand-mirrored constants and templates
(the drift here caused real extension bugs).

## Architecture Position

Depends only on [`tsv_ignore`](../tsv_ignore/CLAUDE.md) (the matcher) — **no other
`tsv_*` crates, no external crates**. Same zero-dep discipline as `tsv_ignore`:
keeps the format-only WASM artifact (`@fuzdev/tsv_format_wasm`) tiny and sidesteps
the dependency-approval gate.

**Policy vs. matcher.** `tsv_ignore` stays a pure gitignore(5) matcher and must
**not** absorb tsv discovery policy — the `dist`/`build`/`target` list, the
hidden-dir rule, the safety nets, the warning text all live **here**. The split:
`tsv_ignore` answers "does *this rule set* ignore this path"; `tsv_discover`
answers "should the *tsv walk* prune/descend/format this entry", which layers the
heuristic and safety nets on top of the matcher.

**Pure, not FS-bound.** Everything here is a decision over already-resolved
inputs (the entry name, the format-root-relative path, whether the heuristic is
active at this level, and a built `IgnoreStack`). It touches no filesystem.
Reading directories, reading ignore files, finding the repo/format root, and the
recursion driver stay in each caller — that's why the same verdict works across
the WASM boundary, where disk access is impossible.

**This is file *scope*, not language dispatch.** tsv's "Closed Scope, Open
Convention" forbids a central `Language` trait / registry / dyn dispatch. This
crate adds none of that — it is the policy half of the one sanctioned *scope*
carve-out (which files get reformatted), so it doesn't reopen that stance. See
[docs/architecture.md §Closed Scope, Open Convention](../../docs/architecture.md#closed-scope-open-convention).

## Public API

The full discovery vocabulary is public — usually consumed via the verdicts, but
the pieces are exposed for inspection, reuse, and third-party `tsv_*` discovery
crates (the open-convention stance):

- `SAFETY_NET_DIRS` / `HEURISTIC_DIRS` / `FORMATTABLE_EXTENSIONS` — the three
  name/extension-set constants.
- `is_formattable(name)` — `.ts`/`.svelte`/`.css` extension check (matches
  `Path::extension`; a bare `.ts` dotfile is a stem, not formattable).
- `is_safety_net(name)` — whether a directory name is an always-pruned safety net.
  A **complete, context-free** decision (safety nets prune in every mode, no
  override), so a caller walking its own tree can short-circuit before building an
  `IgnoreStack`. The build-output heuristic has **no** standalone predicate by
  design — it's contextual (needs the stack + `heuristic_active`), so `classify_dir`
  is its only entry point.
- `classify_dir(name, child_rel, heuristic_active, &IgnoreStack) -> DirVerdict`
  — the per-directory decision: safety-net prune → heuristic prune (with the
  shadow-warning sub-case) → matcher prune → descend.
- `should_format_file(name, child_rel, &IgnoreStack) -> bool` — the per-file
  decision (`is_formattable && !ignored`).

  Both the matcher prune and the file check use the leaf-only
  `IgnoreStack::is_ignored_leaf` (not the ancestor-walking `is_ignored`): discovery
  only reaches an entry whose ancestor directories are already cleared, so the
  re-walk is redundant — dropping it roughly halves the matcher's self-time, which
  dominates discovery on deep trees. **This relies on the caller gating the
  initial `root` with a full `is_ignored`** (a directory under an ignored ancestor,
  e.g. `tsv format build/sub` with a gitignored `build/`, isn't walk-cleared) —
  `tsv_cli`'s `collect_root` and `cli.js`'s do exactly that. See
  `is_ignored_leaf`'s contract in [`tsv_ignore`](../tsv_ignore/CLAUDE.md).
- `DirVerdict { Descend, Prune, PruneWithWarning(String) }` — `PruneWithWarning`
  carries the full warning string, so the native caller reports it without
  re-deriving.
- `heuristic_shadow_warning(d) -> String` — the one warning template. Produced
  once here; `classify_dir` carries it in `PruneWithWarning`. Exposed `pub`
  because the WASM binding fetches it directly (the JS↔WASM boundary can't carry
  a tagged-union payload as one primitive — see below).
- `prettierignore_outside_repo_warning(dir, in_repo, has_prettierignore,
  has_formatignore) -> Option<String>` — the heads-up when, **outside a git
  repo**, a `.prettierignore` sits in the **target root** with no sibling
  `.formatignore` shadowing it. tsv reads `.formatignore` (not `.prettierignore`)
  outside a repo, so prettier would have honored a file tsv silently skips; the
  message points at the rename / `git init` fixes. Returns `None` otherwise. The
  caller gates on *position* (only the target root — a nested or ancestor
  `.prettierignore` is not this case) and passes the presence flags it already
  read from the directory listing, so the check costs no extra filesystem access.
  Both decision and text live here, so the native CLI and WASM binding stay in
  lockstep.

## Consumers

- **`tsv_cli`** (`cli/discover.rs`) — natively, in `collect_recursive`: matches
  `classify_dir`'s `DirVerdict` and pushes any `PruneWithWarning` text into the
  `Discovered::warnings` channel; uses `should_format_file` for the file branch;
  and, at the target root only, pushes any `prettierignore_outside_repo_warning`
  into the same channel. The FS walk, format-root resolution, and ignore-file
  reading stay there.
- **`tsv_wasm`** — the `format`-gated `IgnoreStack` wrapper exposes
  `classify_dir(name, child_rel, heuristic_active) -> string`
  (`"descend"|"prune"|"prune_warn"`), `should_format_file(name, child_rel) ->
  bool`, `heuristic_shadow_warning(dir) -> string`, and
  `prettierignore_outside_repo_warning(dir, in_repo, has_prettierignore,
  has_formatignore) -> string | undefined`. The string-tag encoding
  (rather than a wasm-bindgen enum or a returned struct) needs no
  `patch_npm_package.ts` change and allocates no JS class on the common
  descend path; the `prune_warn` arm fetches the text via the separate method.
  `npm/cli.js` calls these and keeps no policy of its own.

## Behavior is pinned, not asserted

This crate is a **behavior-preserving extraction** of the decision that lived
inline in `discover.rs` / `cli.js`. The shared discovery-parity table
(`tests/discovery/scenarios.json`) runs through both walkers
(`tests/discovery_parity.rs` native, `scripts/test_npm.ts` WASM), the matcher is
pinned against real `git check-ignore` (`tsv_ignore`'s `git_oracle`), and the
heuristic-shadow CLI tests cover the warning — so a regression fails a pinned
test, it isn't merely hoped against. The unit tests here additionally cover each
verdict branch in isolation.
