# tsv_ignore

> gitignore-style path matching for tsv's file discovery — pure functions, zero deps.

The shared matcher behind tsv's one configuration carve-out: file *scope*.
tsv's style is non-configurable, but gitignore-shaped files decide which files
`tsv format` reformats. This crate implements only the matching; locating ignore
files, resolving relative paths, and walking directories belong to the callers.

Two layers: `IgnoreRules` matches one file's rules against root-relative paths
(the primitive — crate-private, since every consumer reaches it through the
stack); `IgnoreStack` layers many per-directory files into a hierarchical,
git-faithful evaluator (the surface the CLI / WASM / extension actually use for
`.gitignore`-aware discovery).

## Architecture Position

Zero dependencies (no `tsv_*` crates, no external crates — not even the `ignore`
crate). Hand-rolled on purpose: keeps the format-only WASM artifact
(`@fuzdev/tsv_format_wasm`) tiny by avoiding `regex`/`globset`, and sidesteps the
dependency-approval gate.

Consumers:

- `tsv_cli` — natively, in directory discovery (`cli/discover.rs`): walks up for
  the repo root, pushes/pops an `IgnoreStack` while recursing.
- `tsv_wasm` — wraps `IgnoreStack` in a `#[wasm_bindgen]` class (gated on the
  `format` feature) so the JS CLI (`crates/tsv_wasm/npm/cli.js`) and the VS Code
  extension share the exact same matcher — agreement by construction, never two
  implementations drifting.
- `tsv_napi` — the same wrapper as a `#[napi]` class, also `format`-gated, so the
  native `@fuzdev/tsv` carries the identical surface (method for method, down to
  `undefined` rather than `null` for the maybe-a-warning methods) and one JS CLI
  source drives either package.
- `tsv_discover` — the policy layer over the matcher; the crate's only `tsv_*` dep.
- `tsv_debug` — `conformance_audit`'s repo walk builds an `IgnoreStack` to honor
  `.gitignore` while checking Markdown links.

The `#[wasm_bindgen]` wrapper lives in `tsv_wasm`, not here; this crate stays
binding-agnostic.

**Matcher, not policy.** This crate answers only "does *this rule set* ignore
this path." tsv's discovery *policy* — the build-output heuristic, the
always-pruned safety nets, the formattable-extension check, the heuristic-shadow
warning — lives one layer up in [`tsv_discover`](../tsv_discover/CLAUDE.md), which
builds on `IgnoreStack` (consuming `is_ignored_leaf` / `is_reincluded` /
`negation_under` / `has_gitignore_layers` / `gitignore_anchors`, and `is_ignored` /
`exclusion` / `tsv_exclusion` — the ancestor-walking answer and its witnesses — with `tsv_layer_source` and
`split_segments`, to bound and warn about a path an argument named). Keeping that policy out of here is deliberate: `IgnoreStack`
stays a pure gitignore(5) matcher, reusable beyond tsv's own discovery rules, and
the three surfaces share the prune *decision* through `tsv_discover` rather than
re-deriving it from these primitives.

## Public API

`IgnoreRules` — the single-file primitive behind each layer, `pub(crate)`:

- `IgnoreRules::parse(content)` — compile one ignore file's text (a leading UTF-8
  BOM is skipped, as git does).
- `IgnoreRules::is_empty()` — the stack skips per-file matching when true.
- `IgnoreRules::is_ignored(path, is_dir)` — `path` relative to the ignore-file
  root, `/`-separated; test-only, since the stack layers `last_match` itself.

`IgnoreStack` — the hierarchical, git-faithful evaluator the surfaces use. It
holds two parallel per-directory layer stacks (`.gitignore` and tsv):

- `IgnoreStack::new()` — an empty stack.
- `IgnoreStack::push_gitignore(anchor, content)` / `pop_gitignore()` — add/drop
  one directory's `.gitignore`, anchored at `anchor` (relative to the format
  root, `""` = root). Push shallow-first; pop on a DFS unwind.
- `IgnoreStack::push_formatignore(anchor, content)` /
  `push_prettierignore(anchor, content)` / `pop_tsv()` — add/drop one directory's tsv
  layer, evaluated after every `.gitignore`. The two pushes match identically; the
  layer records which file it was read from, for a diagnostic to name.
- `IgnoreStack::is_ignored(path, is_dir)` — `path` relative to the format root;
  walks the ancestor prefixes (git's parent-directory prune). The arbitrary-path
  query, and `exclusion` with its witness dropped — one prefix walk
  (`first_excluded_prefix`) serves both.
- `IgnoreStack::is_ignored_leaf(path, is_dir)` — like `is_ignored` but evaluates
  only `path`'s **own** last-match, **no ancestor walk**. Equivalent to
  `is_ignored` *only when every ancestor is already known not-ignored* — which
  tsv's discovery guarantees (it prunes ignored dirs before descending and gates
  the root with the full ancestor-walking answer, `exclusion`), letting it skip the O(depth) re-walk per entry
  (the matcher dominates discovery; this roughly halves its self-time on a deep
  tree). A sharp contract — see Known edges.
- `IgnoreStack::is_reincluded(path, is_dir)` — the per-path `!`-negation polarity
  (no ancestor prune), so a caller's heuristic can defer to an explicit re-include.
- `IgnoreStack::negation_under(prefix) -> Option<Negation>` — the deepest **tsv
  layer** holding a negation anchored *strictly under* `prefix` (its layer anchor +
  leading literal segments has `prefix` as a strict prefix), as `Negation {
  anchor_depth, source }`: the directory holding the rule's file, and the file. Only
  anchored negations count — a floating `!keep.ts` (leading `**`) and a dir-self
  `!dist/` both return `None` — and only layers anchored strictly above `prefix` (a
  walk pruning `prefix` never reads the files at or below it). Lets a caller warn when
  its heuristic prunes a directory a `!dir/<file>` re-include was targeting (a no-op),
  naming the file the escape belongs in — the deepest, whose rules are read last.
  `.gitignore` layers are not consulted.
- `IgnoreStack::has_gitignore_layers()` — whether any `.gitignore` layer is pushed
  (true even for an empty one — mere presence turns a caller's heuristic off, as in
  git). `tsv_discover` uses it to assert its `heuristic_active ⟹ no .gitignore layer`
  invariant.
- `IgnoreStack::gitignore_anchors()` — the format-root-relative directory anchors
  (`/`-joined; `""` = root) of the pushed `.gitignore` layers. Lets a per-file
  discovery replay with **no** top-down walk (the VS Code extension, via
  `tsv_discover::is_path_pruned`) reconstruct each ancestor's `heuristic_active` —
  the heuristic is off at a level once a `.gitignore` anchored above it is present.
- `IgnoreStack::is_empty()` — callers skip per-path matching when true.
- `IgnoreStack::exclusion(path, is_dir) -> Option<Exclusion>` — the witness behind
  `is_ignored`: the shallowest excluded prefix of `path` (`Exclusion::depth`), the
  directory holding the excluding rule's file (`anchor_depth`), both counted in
  `split_segments` segments, and the file itself
  (`IgnoreSource::{Gitignore, Formatignore, Prettierignore}`, with `file_name()`). A
  diagnostic query off the hot path: `tsv_discover` reads it for a path an argument
  named, to warn which directory put the path out of scope and whose rule did.
- `IgnoreStack::tsv_exclusion(path, is_dir) -> Option<Exclusion>` — `exclusion` over the
  tsv layers alone, every `.gitignore` layer set aside. `Some` only where `exclusion` is
  (tsv layers are read last), and the same answer wherever `exclusion`'s witness is a tsv
  layer. `tsv_discover` prefers it for a named path: a `.formatignore`/`.prettierignore`
  rule excluding the path is the one to report even where a `.gitignore` excludes it
  first, since re-including past the `.gitignore` would leave that rule standing.
- `IgnoreStack::tsv_layer_source(anchor) -> Option<IgnoreSource>` — which file the tsv
  layer at `anchor` was read from. `tsv_discover` asks it of the format root, so a
  remedy names the file the root reads rather than one whose creation would shadow it.
- `split_segments(path) -> Vec<&str>` — a path's meaningful segments (empty and `.`
  components dropped): the unit every query splits a path into and `Exclusion`'s
  depths count in, public so a caller slicing by those depths splits the same way.

**Which** files feed the stack is the caller's choice. tsv reads `.formatignore`
hierarchically (one tsv layer per directory) and, inside a git repo,
`.prettierignore` hierarchically too — each shadowed by a *sibling* `.formatignore`
in the same directory; `.gitignore` layers are pushed only inside a git repo. The
crate matches every tsv layer the same way, whichever file it came from; it records
the file only so a diagnostic can name it.

## Distinctives

- **gitignore(5) pattern format** — the same grammar prettier matches via its
  `ignore` dependency: `!` negation (last-match-wins), `/` anchoring,
  trailing-`/` dir-only, `**` (leading/trailing/interior), `*`/`?`/`[...]`
  within a segment, `#` comments, escapes, trailing-space trimming. Reference:
  https://git-scm.com/docs/gitignore#_pattern_format
- **Ancestor-aware** — `is_ignored` evaluates a path's ancestors top-down, so a
  file under a matched directory (`build/`) is reported ignored without the
  caller testing the directory, and a `!` negation cannot re-include a file
  whose parent directory is excluded (git's rule).
- **Hierarchical, last-match-wins across layers** — `IgnoreStack` layers
  per-directory files repo-rooted like git: at each path level it evaluates every
  applicable `.gitignore` (shallow→deep) then every applicable tsv file
  (shallow→deep), last match winning. So a deeper file overrides a shallower one,
  the tsv layer overrides any `.gitignore`, and the parent-prune holds across
  files. Gitignore-only behavior is byte-for-byte `git check-ignore` (the test
  table is pinned against it), less the known edges below. `IgnoreRules` stays the single-root primitive each
  layer is built from.
- **Case-sensitive** — always, matching prettier's `ignore` and git on a
  case-sensitive filesystem. See the case-insensitivity edge below.

## Known edges

- **Case-insensitive filesystems** — matching is always case-sensitive, but git
  auto-sets `core.ignorecase=true` on macOS/Windows, so `git check-ignore` there
  matches case-insensitively (`build/` ignores a `Build/` directory) while tsv
  does not. So the "byte-for-byte `git check-ignore`" parity holds only on
  case-sensitive filesystems. Deliberate: honoring `core.ignorecase` would mean
  reading machine-local git config, which breaks the reproducibility that keeps
  `tsv format --check` giving the same answer everywhere. Rare in practice
  (ignore patterns almost always match the on-disk casing).

- **`.git/info/exclude` and `core.excludesFile` are not read** — discovery consults
  only `.gitignore` files (plus tsv's `.formatignore`/`.prettierignore`), never git's
  other two ignore sources: per-repo `.git/info/exclude` and the global
  `core.excludesFile` (`~/.config/git/ignore`). So `git check-ignore` can ignore a file
  tsv formats. Deliberate, same reproducibility reason as the case bullet — both are
  uncommitted/local (a clean CI checkout lacks them), so honoring them would make
  `tsv format --check` disagree across machines. The "byte-for-byte `git check-ignore`"
  parity is thus scoped to repos whose only ignore source is committed `.gitignore`
  files; the `git_oracle` runs with `core.excludesFile=/dev/null` on a fresh repo, so it
  holds there.

- **Multibyte granularity** — glob metacharacters (`?`, `*`, `[...]`) match per
  Unicode **code point** (a Rust `char`), whereas `git check-ignore` matches per
  **byte** and prettier's `ignore` per **UTF-16 code unit**. On non-ASCII path
  segments those two oracles themselves disagree, so tsv can't match both: for a
  BMP char (`é`, CJK, …) tsv tracks prettier's `ignore` and diverges from git
  (`file?.ts` ignores `fileé.ts` for tsv + `ignore`, **not** for git — git's `?`
  won't span the 2-byte `é`); for an astral char it diverges from both (`a?.ts`
  ignores `a😀.ts` only for tsv — one code point vs two UTF-16 units). Code-point
  granularity is the saner unit and the common case (BMP) tracks prettier, so this
  is a deliberate divergence, not a bug; the "byte-for-byte `git check-ignore`"
  parity is thus scoped to ASCII segments (the `git_oracle` and unit tests are
  ASCII, with `glob_is_code_point_granular` pinning the multibyte behavior). Rare
  in practice — `?`/classes over multibyte names are unusual, and `*` is unaffected.

- **POSIX bracket classes** (`[[:alpha:]]`) are not supported — prettier's
  matcher doesn't rely on them either. They are read as ordinary class members, so
  `[[:alpha:]]` is the class `[:alph` followed by a literal `]` (git matches one ASCII
  letter), and a malformed `[[:bogus:]]`, which git never matches, matches here.

- **A `/` inside a bracket class** (`a[/]b.ts`, `x[!/]y.ts`) — a pattern is split on
  `/` before its classes are read, so the class is cut open and the rule never
  matches, where git reads the class whole. Near-zero exposure: a path component
  cannot hold the `/` such a class names.

- **A `**` glued to a literal prefix** (`foo**/bar.ts`) is two stars within one
  segment by gitignore(5) — no directory crossing — and tsv follows that. git's
  matcher compares the literal prefix first and matches the rest as a fresh pattern,
  where the `**` then leads and crosses directories, so git also ignores
  `foox/y/bar.ts`. A git quirk against its own documentation, deliberately not
  followed.

- **`is_reincluded` is leaf-only** — it reports the `!`-negation polarity of the
  query path *itself*, with no ancestor walk (unlike `is_ignored`). A caller that
  uses it to override a directory prune — tsv's discovery, against its
  build-output heuristic — therefore honors a re-include of *that directory*
  (`!dist/`) but not a re-include of only a *descendant* (`!dist/keep/` with no
  `!dist/`): the caller prunes `dist` before ever reaching `dist/keep`, so the
  deeper rule is never consulted. Deliberate, and consistent with git's
  parent-directory rule in the `.gitignore` regime — a `.gitignore` `dist/`
  likewise blocks a later `!dist/keep/` (the parent must be re-included first).
  **The idiom to selectively re-include under a pruned/ignored directory is
  `!/dir/` first** (admit the directory, anchored to the ignore file's own directory
  — an unanchored one-segment `!dir/` admits a `dir` at every depth), then `/dir/*` +
  `!/dir/keep.ts` to narrow it back down. A bare `!dir/keep.ts` is a no-op;
  `tsv_discover` uses [`negation_under`](#public-api) to detect that case and warn
  (pointing at this `!/dir/` escape, spelled for the file holding the rule) when the
  build-output heuristic is what pruned `dir`.

- **`is_ignored_leaf` omits the ancestor prune** — it reports only the query
  path's *own* last-match exclusion, so a file under an excluded `build/` reads as
  *not* ignored unless a rule matches the file path itself. It equals `is_ignored`
  **only when the path's ancestors are already cleared**, which the discovery walk
  guarantees: it prunes ignored directories before descending, and the CLI/JS
  walkers gate the initial `root` with the full ancestor-walking answer
  (`exclusion`, so `tsv format build/sub` under a gitignored `build/` still finds
  nothing — the gate catches it, and warns; the per-entry walk below uses the
  cheaper leaf query). It exists purely for
  that hot path — never call it on an arbitrary path whose ancestors haven't been
  cleared. Pinned by `stack_is_ignored_leaf_skips_the_ancestor_prune` and the
  `fully_ignored_target_is_empty` discovery scenario.
