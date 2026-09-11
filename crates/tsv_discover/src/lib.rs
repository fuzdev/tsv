//! tsv's file-discovery **policy** — the per-directory and per-file decisions
//! `tsv format` makes while walking a tree, as pure functions over a
//! [`tsv_ignore::IgnoreStack`] (the matcher) plus the entry name and its
//! format-root-relative path.
//!
//! This is the single home of the build-output heuristic, the always-pruned
//! safety nets, the formattable-extension check (both as a discovery filter and
//! as the unsupported-extension error for a named file argument), the
//! heuristic-shadow warning text, the `.prettierignore`-shadowed warning, the
//! `.prettierignore`-outside-a-repo warning, and the gate on a path an argument names
//! (bounded by the ignore files alone, with its warning). The three discovery
//! surfaces — the native CLI (`tsv_cli`), the
//! WASM CLI (`tsv_wasm`'s `npm/cli.js`), and the VS Code extension — call it
//! instead of reimplementing the decision, so they agree **by construction**
//! rather than by hand-mirrored constants and templates.
//!
//! Everything here is **pure**: no filesystem access. Locating directories,
//! reading ignore files, resolving the format root, and walking the tree stay in
//! each caller; only the *verdict* is shared. The matcher this builds on
//! ([`tsv_ignore`]) stays a pure gitignore(5) matcher and deliberately does
//! **not** absorb this policy (the `dist`/`build`/`target` list, the hidden-dir
//! rule, the safety nets, the warning).
//!
//! tsv is non-configurable for *style*; file *scope* (which files get
//! reformatted) is the one sanctioned carve-out. This crate is the policy half
//! of that carve-out. It is **not** a language abstraction — no `Language`
//! trait, registry, or dispatch — so it doesn't touch tsv's "Closed Scope, Open
//! Convention" stance.
//!
//! ```
//! use tsv_discover::{DirVerdict, classify_dir, should_format_file};
//! use tsv_ignore::IgnoreStack;
//!
//! let mut stack = IgnoreStack::new();
//! stack.push_gitignore("", "dist/\n"); // a .gitignore present → heuristic off
//!
//! // gitignored `dist` → pruned; clean `src` → descend
//! assert_eq!(classify_dir("dist", "dist", false, &stack), DirVerdict::Prune);
//! assert_eq!(classify_dir("src", "src", false, &stack), DirVerdict::Descend);
//! // a non-ignored `.ts` file → format it
//! assert!(should_format_file("app.ts", "src/app.ts", &stack));
//! ```

use std::path::Path;
use tsv_ignore::{IgnoreSource, IgnoreStack};

/// Directory names skipped during discovery in **every** mode — VCS metadata and
/// `node_modules`. Catastrophic or pointless to recurse, and not reliably listed
/// in `.gitignore` (a committed `node_modules`, a jj-colocated `.jj`). `.git` is
/// matched as a directory name here; its worktree/submodule *file* form is a file
/// and is never recursed regardless. `.sl` is Sapling's metadata directory — the
/// same VCS set prettier always ignores (`.git`/`.sl`/`.svn`/`.hg`/`.jj`).
pub const SAFETY_NET_DIRS: [&str; 6] = ["node_modules", ".git", ".sl", ".hg", ".svn", ".jj"];

/// Heuristic-only directory skips — tsv's fallback guess at "generated /
/// vendored / build output", applied **only** while no `.gitignore` governs the
/// directory (`heuristic_active`). Hidden directories (a leading `.`) are skipped
/// by the same heuristic. Once a `.gitignore` is in play the project's own rules
/// decide, so these are recursed unless the project ignores them.
pub const HEURISTIC_DIRS: [&str; 3] = ["dist", "build", "target"];

/// The file extensions tsv formats — the discovery filter behind
/// [`is_formattable`]. The whole JS/TS family (`ts`/`mts`/`cts`/`js`/`mjs`/`cjs`)
/// parses as TypeScript, a syntactic superset of JavaScript — the same dispatch
/// an explicitly named file gets from `ParserType::from_extension`, so a walk and
/// a named path agree on every extension. JSX/TSX is out of tsv's scope, so `jsx`
/// and `tsx` are deliberately absent; JSX inside a `.js` file is a parse error
/// rather than a silent skip. Compound forms like `.svelte.ts` are covered by the
/// `ts` entry (`Path::extension` yields the final component).
pub const FORMATTABLE_EXTENSIONS: [&str; 8] =
    ["ts", "mts", "cts", "js", "mjs", "cjs", "svelte", "css"];

/// The discovery verdict for one child **directory**: descend into it, prune it
/// (skip its whole subtree), or prune it **and** surface a diagnostic.
#[derive(Debug, PartialEq, Eq)]
pub enum DirVerdict {
    /// Recurse into the directory.
    Descend,
    /// Skip the directory and its subtree.
    Prune,
    /// Skip the directory and surface a non-fatal stderr warning: the
    /// build-output heuristic pruned a directory that an *anchored* tsv-layer `!`
    /// was trying to re-include *into* (a silent no-op). The caller fetches the
    /// message from [`heuristic_shadow_warning`], which also takes the format root's
    /// display path outside a repo — context the per-directory verdict has no other
    /// use for.
    PruneWithWarning,
}

/// Whether a file name has a [formattable extension](FORMATTABLE_EXTENSIONS)
/// (the JS/TS family, `.svelte`, `.css` — compound forms like `.svelte.ts` are
/// covered by the `.ts` match). Matches `Path::extension`, so a bare dotfile like
/// `.ts` is a stem with no extension and is **not** formattable. Accepts a bare
/// name or a whole path — `Path::extension` reads the final component either way.
pub fn is_formattable(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| FORMATTABLE_EXTENSIONS.contains(&ext))
}

/// The error for an **explicitly named file argument** whose extension tsv does
/// not format, or `None` when [`is_formattable`] accepts it.
///
/// A file argument is held to this before anything else — before the ignore files
/// bound it ([`excluded_argument_warning`]) — because the parser dispatch behind it
/// has no "unknown" arm: everything that isn't `.svelte` or `.css` is handed to the
/// TypeScript parser. Without this
/// gate a named `.json`/`.md`/extensionless file is parsed as TypeScript, which
/// usually fails with a baffling syntax error and occasionally *succeeds* —
/// rewriting a file tsv doesn't support (a top-level-array `.json` reprints as a
/// TS expression statement, semicolon and all, which is no longer valid JSON).
/// Prettier draws the same line, refusing with "No parser could be inferred".
///
/// Returned as an **argument** error, so the run fails upfront with nothing
/// written, alongside the not-a-file-or-directory check. The extension list is
/// rendered from [`FORMATTABLE_EXTENSIONS`], so a new language flows into the
/// message. Produced **once**, here — like [`heuristic_shadow_warning`] — so the
/// native CLI and the WASM CLI emit the identical text. `parse <file>` refuses with
/// the same message (the parser dispatch behind a path is the same one), which is
/// why it says what tsv *handles* rather than what it formats.
pub fn unsupported_extension_error(path: &str) -> Option<String> {
    (!is_formattable(path)).then(|| {
        let list = formattable_extension_list(", ");
        format!("{path}: unsupported file extension (tsv handles {list})")
    })
}

/// [`FORMATTABLE_EXTENSIONS`] rendered as dotted names joined by `separator`
/// (`".ts, .mts, …"`, `".ts/.mts/…"`).
///
/// Here rather than at each message because the list is prose in more than one
/// sentence — this crate's unsupported-extension refusal and the CLI's
/// nothing-in-scope error — and a hand-typed copy is how a ninth language ships with
/// two messages naming eight. The separator varies because the sentences do; the set
/// never does.
#[must_use]
pub fn formattable_extension_list(separator: &str) -> String {
    let mut list = String::new();
    for ext in FORMATTABLE_EXTENSIONS {
        if !list.is_empty() {
            list.push_str(separator);
        }
        list.push('.');
        list.push_str(ext);
    }
    list
}

/// Whether a directory `name` is an always-pruned [safety net](SAFETY_NET_DIRS)
/// (`.git`/`node_modules`/`.sl`/`.hg`/`.svn`/`.jj`). A **complete, context-free**
/// decision — safety nets prune in every mode, with no ignore-file or heuristic
/// override — so a caller doing its own walk can short-circuit on it before
/// building an [`IgnoreStack`]. (The build-output heuristic, by contrast, is
/// contextual — it needs the stack and `heuristic_active` — so it has no
/// standalone predicate; use [`classify_dir`].)
pub fn is_safety_net(name: &str) -> bool {
    SAFETY_NET_DIRS.contains(&name)
}

/// Classify a child **directory** during discovery. `name` is its final path
/// segment; `child_rel` is its format-root-relative, `/`-separated path;
/// `heuristic_active` is true while no `.gitignore` governs this level; `stack`
/// is the matcher built from the ancestor chain. Pure — no filesystem access.
///
/// The order mirrors discovery: the [safety nets](SAFETY_NET_DIRS) prune
/// unconditionally; then, only while the heuristic is active, a hidden or
/// [build-output](HEURISTIC_DIRS) directory prunes unless an explicit tsv-layer
/// `!` re-includes it — and if instead an *anchored* `!dir/<file>` was trying to
/// reach inside it (a no-op under the prune, by git's parent-directory rule), the
/// verdict is [`DirVerdict::PruneWithWarning`]; finally the matcher prunes anything it
/// ignores. Otherwise, descend.
pub fn classify_dir(
    name: &str,
    child_rel: &str,
    heuristic_active: bool,
    stack: &IgnoreStack,
) -> DirVerdict {
    // `heuristic_active` implies no `.gitignore` layer is pushed, so the
    // `is_reincluded` consulted below sees only the tsv layer. That implication is load-bearing (a stray `.gitignore` negation would
    // otherwise leak into the heuristic override), so the CLI/JS walkers' threading
    // of `heuristic_active` is enforced here, not trusted. The per-file
    // `is_path_pruned` replay assembles the full ancestor stack up front rather
    // than pushing incrementally, so it can't satisfy this whole-stack invariant
    // and calls `classify_dir_inner` directly — sound for a different reason (see
    // there), with `heuristic_active` reconstructed per level.
    debug_assert!(
        !heuristic_active || !stack.has_gitignore_layers(),
        "heuristic_active with a .gitignore layer pushed: is_reincluded would consult .gitignore negations in the heuristic override",
    );
    classify_dir_inner(name, child_rel, heuristic_active, stack)
}

/// The [`classify_dir`] decision without its `heuristic_active ⟹ no .gitignore
/// layer` whole-stack debug-assert. A top-down traversal ([`classify_dir`]) pushes
/// layers incrementally, so when it classifies a level no deeper `.gitignore` is
/// present and the assert holds. The per-file [`is_path_pruned`] replay instead
/// assembles the file's *full* ancestor stack once and reconstructs each level's
/// `heuristic_active` itself — so a deeper `.gitignore` may sit in the stack while
/// a shallower level's heuristic is active. That is still faithful: the matcher's
/// `last_match_at` only consults layers whose anchor is a prefix of the queried
/// path (a deeper layer fails to `relativize`), so the leaf / re-include queries
/// see exactly the ancestors they would mid-walk; and `negation_under`, picking
/// `Prune` vs `PruneWithWarning`, reads only layers anchored above the directory it
/// asks about, the same ancestors again.
fn classify_dir_inner(
    name: &str,
    child_rel: &str,
    heuristic_active: bool,
    stack: &IgnoreStack,
) -> DirVerdict {
    // safety nets prune unconditionally
    if is_safety_net(name) {
        return DirVerdict::Prune;
    }
    // the heuristic prunes hidden + build-output dirs only while no `.gitignore`
    // governs this level — but an explicit tsv-layer `!` re-include overrides the
    // guess (an explicit directive beats it).
    if heuristic_active
        && (name.starts_with('.') || HEURISTIC_DIRS.contains(&name))
        && !stack.is_reincluded(child_rel, true)
    {
        // a tsv-layer `!child_rel/<file>` re-include is a silent no-op under this
        // prune (git's parent-dir rule); the warning points at the dir-level
        // escape that works.
        return if stack.negation_under(child_rel).is_some() {
            DirVerdict::PruneWithWarning
        } else {
            DirVerdict::Prune
        };
    }
    // a dir's own ignore files don't classify the dir itself, so this tests it
    // against the stack-so-far (its ancestors). The leaf-only query is exact here
    // because discovery only reaches a directory whose ancestors are already
    // cleared — it prunes ignored dirs before descending, and the caller gates the
    // initial root with a full `is_ignored`. That drops the O(depth) ancestor
    // re-walk that dominated discovery (~70% of `--list` on a deep tree); see
    // `IgnoreStack::is_ignored_leaf`'s contract.
    if stack.is_ignored_leaf(child_rel, true) {
        return DirVerdict::Prune;
    }
    DirVerdict::Descend
}

/// Whether `rel` — a format-root-relative, `/`-separated path — is skipped because
/// some STRICT ancestor directory would be pruned by the traversal (a
/// [safety net](SAFETY_NET_DIRS), the build-output heuristic, or the matcher)
/// before the walk reaches it; `rel` itself is never graded. The per-file companion to
/// [`classify_dir`] for a consumer that has **no top-down traversal**: the VS Code
/// extension formats one open document at a time, so it can't thread
/// `heuristic_active` down a walk. The CLIs never ask it of a path an argument named —
/// the safety nets and the heuristic grade no such path — and bound one by the ignore
/// files alone ([`excluded_argument_warning`]).
///
/// `stack` is the matcher assembled from the file's full ancestor chain (every
/// `.gitignore` / tsv layer from the root down — the same stack a file-level
/// [`is_ignored`](tsv_ignore::IgnoreStack::is_ignored) check uses). This walks the
/// ancestor directories shallow→deep, reconstructing each level's
/// `heuristic_active` from the stack's own pushed `.gitignore` anchors
/// ([`gitignore_anchors`](tsv_ignore::IgnoreStack::gitignore_anchors)) — off at a
/// level once a `.gitignore` anchored *above* it is present — and returns `true`
/// at the first ancestor [`classify_dir`] would not `Descend` into. It does the
/// **directory** half of discovery only; pair it with
/// [`is_ignored(rel, false)`](tsv_ignore::IgnoreStack::is_ignored) for the
/// file-level match (a file no directory prunes may still be ignored by a rule).
///
/// Equivalent to running [`classify_dir`] down a real single-path walk — it calls
/// [`classify_dir_inner`] so the full-stack assembly skips the incremental-walk
/// assert; see that fn for why the assembled stack stays faithful.
pub fn is_path_pruned(rel: &str, stack: &IgnoreStack) -> bool {
    first_pruned_ancestor(rel, stack).is_some()
}

/// The warning a walk raises on the way down to `rel`: the [`heuristic_shadow_warning`]
/// for the first ancestor directory [`is_path_pruned`] stops at, when that directory's
/// verdict is [`DirVerdict::PruneWithWarning`]. `None` when no ancestor prunes `rel`, and
/// when the one that does is a plain prune — a safety net, the matcher, or the heuristic
/// with no tsv-layer re-include written under it. The per-file companion to
/// [`classify_dir`] + [`heuristic_shadow_warning`], for [`is_path_pruned`]'s consumer: a
/// document skipped because the heuristic prunes its directory, while a re-include written
/// to reach it does nothing, is a misconfiguration that consumer can name without a walk.
///
/// Only the first pruned ancestor is asked, since a walk stops there. The matcher prune is
/// the reason the verdict is read rather than [`heuristic_shadow_warning`] alone: a
/// re-include under a directory a rule excludes is as inert, but that warning would name
/// the heuristic as the cause. `stack` is [`is_path_pruned`]'s; `loose_root` is
/// [`heuristic_shadow_warning`]'s.
pub fn path_heuristic_shadow_warning(
    rel: &str,
    loose_root: Option<&str>,
    stack: &IgnoreStack,
) -> Option<String> {
    let (dir, verdict) = first_pruned_ancestor(rel, stack)?;
    if verdict == DirVerdict::PruneWithWarning {
        heuristic_shadow_warning(&dir, loose_root, stack)
    } else {
        None
    }
}

/// The first STRICT ancestor directory of `rel` the traversal would not descend into, with
/// its verdict — the replay behind [`is_path_pruned`] and
/// [`path_heuristic_shadow_warning`].
fn first_pruned_ancestor(rel: &str, stack: &IgnoreStack) -> Option<(String, DirVerdict)> {
    let segments = tsv_ignore::split_segments(rel);
    // a root-level file (or empty path) has no ancestor directories to prune
    if segments.len() < 2 {
        return None;
    }
    let git_anchors = stack.gitignore_anchors();
    let mut child_rel = String::new();
    for &name in &segments[..segments.len() - 1] {
        if child_rel.is_empty() {
            child_rel.push_str(name);
        } else {
            child_rel.push('/');
            child_rel.push_str(name);
        }
        let heuristic_active = !gitignore_above(&git_anchors, &child_rel);
        let verdict = classify_dir_inner(name, &child_rel, heuristic_active, stack);
        if verdict != DirVerdict::Descend {
            return Some((child_rel, verdict));
        }
    }
    None
}

/// Whether any `.gitignore` anchor in `anchors` sits at a **strict ancestor**
/// directory of `dir`. The root anchor `""` is an ancestor of every non-root
/// directory; an anchor *equal* to `dir` is the directory's own `.gitignore`
/// (pushed only when descending *into* it during a real walk, so it doesn't
/// classify the directory itself) and does **not** count. This reconstructs how
/// the traversal turns `heuristic_active` off for a directory once a `.gitignore`
/// governs a level at or above it.
fn gitignore_above(anchors: &[String], dir: &str) -> bool {
    anchors.iter().any(|anchor| {
        anchor.is_empty()
            || (dir.len() > anchor.len()
                && dir.starts_with(anchor.as_str())
                && dir.as_bytes()[anchor.len()] == b'/')
    })
}

/// Whether a child **file** should be formatted: it has a formattable extension
/// and the matcher does not ignore it. `name` is its final path segment;
/// `child_rel` is its format-root-relative, `/`-separated path. Pure — no
/// filesystem access. (A named file *argument* never reaches this: both halves are
/// asked of the argument itself — its extension by [`unsupported_extension_error`],
/// its ignore rules by [`excluded_argument_warning`], which walks the ancestors this
/// leaf query takes as cleared.)
///
/// Uses the leaf-only [`is_ignored_leaf`](tsv_ignore::IgnoreStack::is_ignored_leaf):
/// the discovery walk only reaches a file whose ancestor directories are already
/// cleared (it prunes ignored dirs before descending, and the caller gates the
/// root), so the ancestor walk would be redundant — see [`classify_dir`].
pub fn should_format_file(name: &str, child_rel: &str, stack: &IgnoreStack) -> bool {
    is_formattable(name) && !stack.is_ignored_leaf(child_rel, false)
}

/// The stderr warning when the build-output heuristic prunes a directory that a
/// tsv-layer `!` rule was trying to re-include *into*, or `None` when no such rule is
/// written ([`IgnoreStack::negation_under`], which is how [`classify_dir`] reaches
/// [`DirVerdict::PruneWithWarning`]). The re-include is a silent no-op — git's
/// parent-directory rule (matched in the `.gitignore` regime) bars re-including a
/// descendant of an excluded directory — so the message points at the dir-level escape
/// that does work, for the file the rule was written in (the deepest holding one, whose
/// rules are read last), spelled relative to that file's directory: `!/dist/` in
/// `pkg/.formatignore` for a pruned `pkg/dist`. Every line is anchored with a leading
/// `/` — without which a one-segment `!dist/` re-includes a `dist` at every depth — and
/// spells its path literally ([`pattern_path`]); spelled from the format root instead,
/// the lines would do nothing in a nested file,
/// and outside a repo — where the format root is the filesystem root — in any file.
///
/// `d` is the pruned directory relative to the format root, `/`-separated; `loose_root`
/// is [`excluded_argument_warning`]'s, the format root's display path outside a git
/// repo. Produced **once**, here, so the native CLI and the WASM binding emit the
/// identical text.
pub fn heuristic_shadow_warning(
    d: &str,
    loose_root: Option<&str>,
    stack: &IgnoreStack,
) -> Option<String> {
    let negation = stack.negation_under(d)?;
    let segments = tsv_ignore::split_segments(d);
    let dir = path_display(&segments, loose_root);
    let rule_file = ignore_file_display(
        &segments[..negation.anchor_depth],
        negation.source,
        loose_root,
    );
    let rel = pattern_path(&segments[negation.anchor_depth..]);
    Some(format!(
        "{dir} is skipped by tsv's build-output heuristic, so a re-include under it in {rule_file} does nothing; re-include the directory itself there with `!/{rel}/` (then `/{rel}/*` + `!/{rel}/<file>` to select files within it)"
    ))
}

/// The stderr warning when a `.prettierignore` sits in the **target root**
/// directory but the run is **outside a git repo** — where tsv reads
/// `.formatignore` (hierarchically) but never `.prettierignore`. Prettier would
/// honor a cwd-level `.prettierignore`; tsv silently would not, so the message
/// points at the two fixes (rename to `.formatignore`, or `git init`). `dir` is
/// the target root's display path.
///
/// Returns `None` unless **all** hold: outside a repo (`!in_repo`), a
/// `.prettierignore` is present (`has_prettierignore`), and no sibling
/// `.formatignore` supersedes it (`!has_formatignore` — its presence means the
/// native file was adopted, so the `.prettierignore` is vestigial and silence is
/// correct).
///
/// The caller invokes this **once, at the target root**. An ancestor
/// `.prettierignore` of a subdirectory target is deliberately *not* this case
/// (outside a repo there is no boundary to bound an upward search), and a nested
/// `.prettierignore` below the target root is not scanned either: outside a repo
/// tsv's regime is `.formatignore`-only at every depth, so this is one courtesy
/// heads-up at the entry point rather than a per-directory warning. (Inside a repo
/// the warning never fires — there tsv *does* read `.prettierignore`,
/// hierarchically.) Presence-only — an empty or comments-only
/// `.prettierignore` still warns (rare, and the message still points at the right
/// fix); the caller learns presence from the directory listing it already holds,
/// so this costs no extra filesystem access. Produced **once**, here, like
/// `heuristic_shadow_warning`, so the native CLI and the WASM binding emit the
/// identical text.
pub fn prettierignore_outside_repo_warning(
    dir: &str,
    in_repo: bool,
    has_prettierignore: bool,
    has_formatignore: bool,
) -> Option<String> {
    (!in_repo && has_prettierignore && !has_formatignore).then(|| {
        format!(
            ".prettierignore in {dir} is not read outside a git repo (tsv reads .formatignore there); rename it to .formatignore, or run `git init`, for it to apply"
        )
    })
}

/// The heads-up when, **inside a git repo**, a directory holds both a
/// `.formatignore` and a `.prettierignore`: the sibling `.formatignore` shadows
/// the `.prettierignore` (one tsv layer per directory, the native file adopted),
/// so the `.prettierignore`'s rules go unread in that directory — the drop-in
/// counterpart to Prettier, which applies *both* files. The message points at
/// merging the patterns into `.formatignore`. Returns `None` otherwise. Unlike
/// [`prettierignore_outside_repo_warning`] (bounded to the target root), this
/// fires at **every** directory the walk reaches — a shadow is per-directory, not
/// an entry-point condition. Presence-only, from flags the caller already read
/// from the directory listing (an empty or comments-only `.prettierignore` still
/// warns — rare, and the fix still applies), so it costs no extra filesystem
/// access. Both decision and text live here, so the native CLI and WASM binding
/// stay in lockstep. Same argument order as `prettierignore_outside_repo_warning`.
pub fn prettierignore_shadowed_warning(
    dir: &str,
    in_repo: bool,
    has_prettierignore: bool,
    has_formatignore: bool,
) -> Option<String> {
    (in_repo && has_prettierignore && has_formatignore).then(|| {
        format!(
            ".prettierignore in {dir} is shadowed by a sibling .formatignore and is not applied; move its patterns into .formatignore to keep them"
        )
    })
}

/// The stderr warning for an in-tree `.gitignore` that is a **symbolic link**.
///
/// git does not follow one — gitignore(5): "Git does not follow symbolic links when
/// accessing a .gitignore file in the working tree", which keeps the file reading the same
/// from the index or a tree as from disk — and warns that it cannot access it, applying
/// none of its rules. tsv's `.gitignore` regime is git's, so the discovery walks treat such
/// a file exactly as they treat an unreadable one: its rules are not applied, the
/// build-output heuristic stays on for its subtree, and this says so. `path` is the link's
/// own path. The tsv layer (`.formatignore` / `.prettierignore`) keeps reading through
/// links, as prettier does, so only this name takes the warning. Produced **once**, here,
/// so the native CLI and the WASM binding emit the identical text.
pub fn gitignore_symlink_warning(path: &str) -> String {
    format!(
        "{path} is a symbolic link, which git does not follow in a working tree; its ignore rules are not applied"
    )
}

/// The traversal error for a **relative** directory root that cannot be made absolute
/// because the working directory itself cannot be resolved — it was deleted out from under
/// the run. Walked anyway, the root would anchor on no format root and read none of its
/// ancestors' ignore files, silently widening the scope, so the walk refuses it instead.
/// `root` is the argument as given. Produced **once**, here, so the native CLI and the
/// WASM binding emit the identical text.
pub fn unresolvable_root_error(root: &str) -> String {
    format!("{root}: cannot resolve a relative path: the working directory is unavailable")
}

/// The stderr warning for a path an argument **named** — a file, or a directory root —
/// that an ignore file puts out of scope, or `None`: when no rule excludes it, and when
/// the skip is one to keep quiet.
///
/// Whether a named path is in scope is the matcher's answer alone
/// ([`IgnoreStack::is_ignored`]). The safety nets and the build-output heuristic prune
/// only what a walk *discovers* — a guess tsv makes about a tree never overrides a path
/// someone typed — while an ignore rule is one the user or their repo wrote, and it bounds
/// a named path exactly as it bounds the walk that would have reached it: through any
/// ancestor directory, and at the path itself.
///
/// What this decides is whether saying so helps. A named **file** a `.formatignore` or
/// `.prettierignore` excludes is skipped quietly, as prettier skips it, whether or not a
/// `.gitignore` excludes it too: those files exist to say what not to format, and a
/// pre-commit hook handing over its staged files names such a file on every commit that
/// touches one. Every other exclusion warns — a `.gitignore` rule is about version
/// control, so one excluding a named file is a surprise, and a named directory is a scope
/// someone typed — naming the file whose rule did it and how to undo it. A tsv rule is the
/// user's to narrow, and is the one named wherever it excludes the path at all
/// ([`IgnoreStack::tsv_exclusion`]): re-including the path past a `.gitignore` would leave
/// that rule standing, and, added to the same file, override it. A path only a
/// `.gitignore` excludes gets the lines that re-include it and nothing beside it
/// ([`reinclude_lines`]), for the repo root's own tsv file (read after every `.gitignore`,
/// so its `!` wins; its `.prettierignore` where that is the file the root reads, since a
/// `.formatignore` created beside it would shadow every rule in it).
///
/// `display` is the argument as given; `rel` is the path relative to the format root,
/// `/`-separated; `is_dir` says which kind of argument it is; `stack` holds the layers
/// from the format root down through the path's parent directory, stopping at a directory
/// a rule excludes — the walk never reads the ignore files inside one, so a rule in one
/// can still exclude the path once that directory is re-included. `loose_root` is the
/// format root's display path outside a git repo, where the format root is the
/// filesystem root and a path is named absolutely; inside one it is `None`, and paths read
/// relative to the repo root. Produced **once**, here, so the native CLI and the WASM
/// binding emit the identical text.
pub fn excluded_argument_warning(
    display: &str,
    rel: &str,
    is_dir: bool,
    loose_root: Option<&str>,
    stack: &IgnoreStack,
) -> Option<String> {
    let exclusion = stack
        .tsv_exclusion(rel, is_dir)
        .or_else(|| stack.exclusion(rel, is_dir))?;
    let by_gitignore = exclusion.source == IgnoreSource::Gitignore;
    if !is_dir && !by_gitignore {
        return None;
    }
    let segments = tsv_ignore::split_segments(rel);
    let consequence = if is_dir {
        "so nothing under it is formatted"
    } else {
        "so it is not formatted"
    };
    let rule_file = ignore_file_display(
        &segments[..exclusion.anchor_depth],
        exclusion.source,
        loose_root,
    );
    let remedy = if by_gitignore {
        let root_file = stack
            .tsv_layer_source("")
            .unwrap_or(IgnoreSource::Formatignore)
            .file_name();
        let lines = reinclude_lines(&segments, exclusion.depth, is_dir);
        let order = if lines.len() > 1 {
            ", in that order,"
        } else {
            ""
        };
        format!(
            "re-include it by adding {}{order} to the repo-root {root_file}",
            quoted_list(&lines)
        )
    } else {
        "narrow or negate that rule to format it".to_string()
    };
    Some(if exclusion.depth == segments.len() {
        format!("{display} is excluded by a rule in {rule_file}, {consequence}; {remedy}")
    } else {
        let dir = path_display(&segments[..exclusion.depth], loose_root);
        format!(
            "{display} is inside {dir}, which a rule in {rule_file} excludes, {consequence}; {remedy}"
        )
    })
}

/// The ignore-file lines that put `segments` — a path whose first `depth` segments name
/// the directory or file a `.gitignore` rule excluded — back in scope and nothing beside
/// it, for a tsv layer at the format root. A path the rule matched itself takes one line
/// (`!/src/a.gen.ts`); one under an excluded directory takes that directory re-included,
/// its contents excluded again, and so on one level at a time down to the path
/// (`!/build/`, `/build/*`, `!/build/a.ts`), because git's parent-directory rule ignores a
/// `!` under a directory that is still excluded. Every line is anchored with a leading
/// `/`, without which a one-segment pattern matches at every depth, and spells its path
/// literally ([`pattern_path`]).
fn reinclude_lines(segments: &[&str], depth: usize, is_dir: bool) -> Vec<String> {
    let anchored = |len: usize| {
        let slash = if len < segments.len() || is_dir {
            "/"
        } else {
            ""
        };
        format!("/{}{slash}", pattern_path(&segments[..len]))
    };
    let mut lines = vec![format!("!{}", anchored(depth))];
    for len in depth..segments.len() {
        lines.push(format!("/{}/*", pattern_path(&segments[..len])));
        lines.push(format!("!{}", anchored(len + 1)));
    }
    lines
}

/// Path segments `/`-joined as a literal ignore-file pattern: in each, the glob
/// metacharacters `*`, `?`, `[` and `]` and the escape `\` are backslash-escaped, and so
/// are trailing spaces, which gitignore(5) trims from a line — so a `[slug]` route
/// directory names itself rather than a one-character class. A leading `#` or `!` needs
/// no escape: every line spells its path after a `/`.
fn pattern_path(segments: &[&str]) -> String {
    let mut pattern = String::new();
    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            pattern.push('/');
        }
        let kept = segment.trim_end_matches(' ');
        for c in kept.chars() {
            if matches!(c, '*' | '?' | '[' | ']' | '\\') {
                pattern.push('\\');
            }
            pattern.push(c);
        }
        pattern.push_str(&"\\ ".repeat(segment.len() - kept.len()));
    }
    pattern
}

/// `items` backticked and joined as prose: `` `a` ``, `` `a` and `b` ``,
/// `` `a`, `b` and `c` ``.
fn quoted_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("`{item}`")).collect();
    match quoted.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => quoted.concat(),
    }
}

/// The ignore file `source` names in the directory `dir` (format-root-relative segments),
/// as a user reads it: `the repo-root .gitignore` or `src/.formatignore` inside a repo, and
/// by its absolute path outside one.
fn ignore_file_display(dir: &[&str], source: IgnoreSource, loose_root: Option<&str>) -> String {
    if loose_root.is_none() && dir.is_empty() {
        return format!("the repo-root {}", source.file_name());
    }
    let mut segments = dir.to_vec();
    segments.push(source.file_name());
    path_display(&segments, loose_root)
}

/// Format-root-relative segments as a user reads them: `/`-joined inside a repo, where
/// the format root is the repo root, and joined onto `loose_root` outside one, where the
/// format root is the filesystem root and the relative form would read as a path under
/// the working directory.
fn path_display(segments: &[&str], loose_root: Option<&str>) -> String {
    let Some(root) = loose_root else {
        return segments.join("/");
    };
    let separator = if root.contains('\\') { '\\' } else { '/' };
    let mut display = root.trim_end_matches(['/', '\\']).to_string();
    for segment in segments {
        display.push(separator);
        display.push_str(segment);
    }
    display
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tsv_stack(content: &str) -> IgnoreStack {
        let mut stack = IgnoreStack::new();
        stack.push_formatignore("", content);
        stack
    }

    #[test]
    fn is_formattable_matches_supported_extensions() {
        assert!(is_formattable("a.ts"));
        assert!(is_formattable("a.mts"));
        assert!(is_formattable("a.cts"));
        assert!(is_formattable("a.js")); // the TypeScript parser covers the JS family
        assert!(is_formattable("a.mjs"));
        assert!(is_formattable("a.cjs"));
        assert!(is_formattable("a.svelte"));
        assert!(is_formattable("a.css"));
        assert!(is_formattable("a.svelte.ts")); // .ts wins
        assert!(!is_formattable("a.jsx")); // JSX/TSX is out of scope
        assert!(!is_formattable("a.tsx"));
        assert!(!is_formattable("a.txt"));
        assert!(!is_formattable("a")); // no extension
        assert!(!is_formattable(".ts")); // bare dotfile: a stem, no extension
        assert!(!is_formattable("Makefile"));
    }

    #[test]
    fn unsupported_extension_error_mirrors_is_formattable() {
        // every formattable extension is accepted (no error), on a bare name and
        // on a path — `Path::extension` reads the final component either way
        for ext in FORMATTABLE_EXTENSIONS {
            assert!(
                unsupported_extension_error(&format!("file.{ext}")).is_none(),
                "{ext}"
            );
            assert!(
                unsupported_extension_error(&format!("src/deep/file.{ext}")).is_none(),
                "{ext}"
            );
        }
        // the cases a TypeScript fall-through would swallow
        for path in [
            "package-lock.json",
            "notes.md",
            "Makefile",
            "a.pcss",
            "a.tsx",
        ] {
            assert!(unsupported_extension_error(path).is_some(), "{path}");
        }
    }

    #[test]
    fn unsupported_extension_error_text_is_stable() {
        // pinned verbatim — the native CLI and the WASM CLI emit this exact
        // string, and the extension list is rendered from the constant
        assert_eq!(
            unsupported_extension_error("data.json").unwrap(),
            "data.json: unsupported file extension (tsv handles .ts, .mts, .cts, .js, .mjs, .cjs, .svelte, .css)"
        );
    }

    #[test]
    fn is_safety_net_matches_the_constant() {
        for name in SAFETY_NET_DIRS {
            assert!(is_safety_net(name), "{name}");
        }
        assert!(is_safety_net(".sl")); // Sapling — prettier parity
        assert!(!is_safety_net("src"));
        assert!(!is_safety_net("dist")); // a heuristic dir, not a safety net
        assert!(!is_safety_net(".github")); // a hidden dir is not a safety net
    }

    #[test]
    fn formattable_extensions_back_is_formattable() {
        for ext in FORMATTABLE_EXTENSIONS {
            assert!(is_formattable(&format!("file.{ext}")), "{ext}");
        }
    }

    #[test]
    fn safety_nets_always_prune() {
        // even with the heuristic off and no ignore rules
        let stack = IgnoreStack::new();
        for name in SAFETY_NET_DIRS {
            assert_eq!(
                classify_dir(name, name, false, &stack),
                DirVerdict::Prune,
                "{name}"
            );
        }
    }

    #[test]
    fn heuristic_prunes_hidden_and_build_dirs_when_active() {
        let stack = IgnoreStack::new();
        assert_eq!(
            classify_dir(".cache", ".cache", true, &stack),
            DirVerdict::Prune
        );
        for name in HEURISTIC_DIRS {
            assert_eq!(
                classify_dir(name, name, true, &stack),
                DirVerdict::Prune,
                "{name}"
            );
        }
        // a normal dir descends
        assert_eq!(
            classify_dir("src", "src", true, &stack),
            DirVerdict::Descend
        );
    }

    #[test]
    fn heuristic_off_keeps_hidden_and_build_dirs() {
        // with the heuristic inactive (a `.gitignore` governs), a hidden or
        // build-output dir is not a heuristic prune — only the matcher can prune it
        let stack = IgnoreStack::new();
        assert_eq!(
            classify_dir("build", "build", false, &stack),
            DirVerdict::Descend
        );
        assert_eq!(
            classify_dir(".cache", ".cache", false, &stack),
            DirVerdict::Descend
        );
    }

    #[test]
    fn explicit_dir_reinclude_overrides_heuristic() {
        let stack = tsv_stack("!build/\n");
        assert_eq!(
            classify_dir("build", "build", true, &stack),
            DirVerdict::Descend
        );
    }

    #[test]
    fn anchored_negation_under_pruned_dir_warns() {
        let stack = tsv_stack("!build/keep.ts\n");
        assert_eq!(
            classify_dir("build", "build", true, &stack),
            DirVerdict::PruneWithWarning,
        );
    }

    #[test]
    fn floating_negation_under_pruned_dir_does_not_warn() {
        // a floating `!keep.ts` (parsed with a leading `**`) targets any depth,
        // not `build/` specifically, so it is a plain prune (no warning)
        let stack = tsv_stack("!keep.ts\n");
        assert_eq!(
            classify_dir("build", "build", true, &stack),
            DirVerdict::Prune
        );
    }

    #[test]
    fn matcher_prunes_ignored_non_heuristic_dir() {
        // `vendored/` is not a heuristic dir, so with the heuristic off the matcher
        // is the only thing that can prune it
        let stack = tsv_stack("vendored/\n");
        assert_eq!(
            classify_dir("vendored", "vendored", false, &stack),
            DirVerdict::Prune
        );
        assert_eq!(
            classify_dir("src", "src", false, &stack),
            DirVerdict::Descend
        );
    }

    #[test]
    fn should_format_file_checks_extension_and_ignore() {
        let stack = tsv_stack("gen.ts\n");
        assert!(should_format_file("app.ts", "src/app.ts", &stack)); // formattable, not ignored
        assert!(!should_format_file("notes.md", "notes.md", &stack)); // wrong extension
        assert!(!should_format_file("gen.ts", "src/gen.ts", &stack)); // ignored by the matcher
    }

    #[test]
    fn heuristic_shadow_warning_text_is_stable() {
        // pinned verbatim — the native CLI and the WASM binding both fetch it, so both
        // surfaces emit this exact string. The lines are for the file the re-include
        // was written in, anchored and relative to its directory
        let stack = tsv_stack("!dist/keep.ts\n");
        assert_eq!(
            heuristic_shadow_warning("dist", None, &stack).unwrap(),
            "dist is skipped by tsv's build-output heuristic, so a re-include under it in the repo-root .formatignore does nothing; re-include the directory itself there with `!/dist/` (then `/dist/*` + `!/dist/<file>` to select files within it)"
        );
        let mut stack = IgnoreStack::new();
        stack.push_prettierignore("pkg", "!dist/keep.ts\n");
        assert_eq!(
            heuristic_shadow_warning("pkg/dist", None, &stack).unwrap(),
            "pkg/dist is skipped by tsv's build-output heuristic, so a re-include under it in pkg/.prettierignore does nothing; re-include the directory itself there with `!/dist/` (then `/dist/*` + `!/dist/<file>` to select files within it)"
        );
        // outside a repo the format root is the filesystem root: both paths read
        // absolutely, while the lines stay relative to the rule's file
        let mut stack = IgnoreStack::new();
        stack.push_formatignore("home/u/proj", "!src/build/keep.ts\n");
        assert_eq!(
            heuristic_shadow_warning("home/u/proj/src/build", Some("/"), &stack).unwrap(),
            "/home/u/proj/src/build is skipped by tsv's build-output heuristic, so a re-include under it in /home/u/proj/.formatignore does nothing; re-include the directory itself there with `!/src/build/` (then `/src/build/*` + `!/src/build/<file>` to select files within it)"
        );
        // no re-include written under the directory: nothing to say
        assert_eq!(
            heuristic_shadow_warning("dist", None, &IgnoreStack::new()),
            None
        );
    }

    #[test]
    fn heuristic_shadow_warning_lines_readmit_the_directory_alone() {
        // pasted into the file the warning names (its `<file>` filled in), the lines put
        // the pruned directory back in the walk and only the selected file in scope, and
        // re-include no same-named directory elsewhere — which an unanchored `!dist/` did.
        // Each line spells its path literally, so a `[slug]` segment is no character class
        // and a trailing space survives the line's trim
        for (anchor, dir, rule) in [
            ("", "dist", "!dist/keep.ts\n"),
            ("pkg", "dist", "!dist/keep.ts\n"),
            ("", "[slug]/dist", "!\\[slug\\]/dist/keep.ts\n"),
            ("", ".c ", "!.c\\ /keep.ts\n"),
        ] {
            let under = |path: &str| {
                if anchor.is_empty() {
                    path.to_string()
                } else {
                    format!("{anchor}/{path}")
                }
            };
            let name = dir.rsplit('/').next().unwrap();
            let d = under(dir);
            let mut stack = IgnoreStack::new();
            stack.push_formatignore(anchor, rule);
            let warning = heuristic_shadow_warning(&d, None, &stack).unwrap();
            // the backticked spans are the lines, in order
            let lines: Vec<String> = warning
                .split('`')
                .skip(1)
                .step_by(2)
                .map(|line| line.replace("<file>", "keep.ts"))
                .collect();
            let mut fixed = IgnoreStack::new();
            fixed.push_formatignore(anchor, &format!("{rule}{}\n", lines.join("\n")));
            assert_eq!(
                classify_dir(name, &d, true, &fixed),
                DirVerdict::Descend,
                "{d}: {lines:?}"
            );
            assert!(
                should_format_file("keep.ts", &under(&format!("{dir}/keep.ts")), &fixed),
                "{d}: {lines:?}"
            );
            assert!(
                !should_format_file("other.ts", &under(&format!("{dir}/other.ts")), &fixed),
                "{d}: {lines:?}"
            );
            assert_eq!(
                classify_dir(name, &under(&format!("lib/{name}")), true, &fixed),
                DirVerdict::Prune,
                "{d}: {lines:?}"
            );
        }
    }

    #[test]
    fn prettierignore_outside_repo_warns_only_when_unshadowed_outside_a_repo() {
        // the footgun: outside a repo, a target-root `.prettierignore` with no
        // `.formatignore` beside it is silently skipped → warn
        assert!(prettierignore_outside_repo_warning("proj", false, true, false).is_some());
        // inside a repo `.prettierignore` IS read (hierarchically) → no warning
        assert!(prettierignore_outside_repo_warning("proj", true, true, false).is_none());
        // a sibling `.formatignore` supersedes it (native file adopted) → no warning
        assert!(prettierignore_outside_repo_warning("proj", false, true, true).is_none());
        // no `.prettierignore` present → nothing to warn about
        assert!(prettierignore_outside_repo_warning("proj", false, false, false).is_none());
    }

    #[test]
    fn prettierignore_outside_repo_warning_text_is_stable() {
        // pinned verbatim — the native CLI and the WASM binding emit this exact
        // string, so both surfaces stay in lockstep
        assert_eq!(
            prettierignore_outside_repo_warning(".", false, true, false).unwrap(),
            ".prettierignore in . is not read outside a git repo (tsv reads .formatignore there); rename it to .formatignore, or run `git init`, for it to apply"
        );
    }

    #[test]
    fn prettierignore_shadowed_warns_only_inside_a_repo_with_both_files() {
        // inside a repo, both a `.formatignore` and a `.prettierignore` in one
        // directory: the sibling `.formatignore` shadows → warn
        assert!(prettierignore_shadowed_warning("src", true, true, true).is_some());
        // only `.prettierignore` (no sibling `.formatignore`) → it IS read, nothing shadowed
        assert!(prettierignore_shadowed_warning("src", true, true, false).is_none());
        // only `.formatignore` → nothing to shadow
        assert!(prettierignore_shadowed_warning("src", true, false, true).is_none());
        // outside a repo `.prettierignore` isn't read regardless — the outside-repo
        // warning owns that case, so this one stays quiet
        assert!(prettierignore_shadowed_warning("src", false, true, true).is_none());
    }

    #[test]
    fn prettierignore_shadowed_warning_text_is_stable() {
        // pinned verbatim — the native CLI and the WASM binding emit this exact
        // string, so both surfaces stay in lockstep
        assert_eq!(
            prettierignore_shadowed_warning("src", true, true, true).unwrap(),
            ".prettierignore in src is shadowed by a sibling .formatignore and is not applied; move its patterns into .formatignore to keep them"
        );
    }

    #[test]
    fn gitignore_symlink_warning_text_is_stable() {
        // pinned verbatim: both CLIs emit it, and the VS Code extension restates it by
        // hand against a published binding that predates it
        assert_eq!(
            gitignore_symlink_warning("/repo/.gitignore"),
            "/repo/.gitignore is a symbolic link, which git does not follow in a working tree; its ignore rules are not applied"
        );
    }

    #[test]
    fn excluded_argument_warning_text_is_stable() {
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\n*.gen.ts\n");
        stack.push_gitignore("pkg", "out/\n");
        stack.push_formatignore("", "vendor/\n");
        stack.push_prettierignore("lib", "gen/\n");
        let warn =
            |display, rel, is_dir| excluded_argument_warning(display, rel, is_dir, None, &stack);
        assert_eq!(
            warn("build", "build", true).as_deref(),
            Some(
                "build is excluded by a rule in the repo-root .gitignore, so nothing under it is formatted; re-include it by adding `!/build/` to the repo-root .formatignore"
            )
        );
        assert_eq!(
            warn("build/sub", "build/sub", true).as_deref(),
            Some(
                "build/sub is inside build, which a rule in the repo-root .gitignore excludes, so nothing under it is formatted; re-include it by adding `!/build/`, `/build/*` and `!/build/sub/`, in that order, to the repo-root .formatignore"
            )
        );
        assert_eq!(
            warn("build/a.ts", "build/a.ts", false).as_deref(),
            Some(
                "build/a.ts is inside build, which a rule in the repo-root .gitignore excludes, so it is not formatted; re-include it by adding `!/build/`, `/build/*` and `!/build/a.ts`, in that order, to the repo-root .formatignore"
            )
        );
        assert_eq!(
            warn("src/a.gen.ts", "src/a.gen.ts", false).as_deref(),
            Some(
                "src/a.gen.ts is excluded by a rule in the repo-root .gitignore, so it is not formatted; re-include it by adding `!/src/a.gen.ts` to the repo-root .formatignore"
            )
        );
        // a nested `.gitignore` is named where it sits; the lines still go to the root
        assert_eq!(
            warn("pkg/out/a.ts", "pkg/out/a.ts", false).as_deref(),
            Some(
                "pkg/out/a.ts is inside pkg/out, which a rule in pkg/.gitignore excludes, so it is not formatted; re-include it by adding `!/pkg/out/`, `/pkg/out/*` and `!/pkg/out/a.ts`, in that order, to the repo-root .formatignore"
            )
        );
        // a directory a tsv rule excludes names that rule's file
        assert_eq!(
            warn("vendor/lib", "vendor/lib", true).as_deref(),
            Some(
                "vendor/lib is inside vendor, which a rule in the repo-root .formatignore excludes, so nothing under it is formatted; narrow or negate that rule to format it"
            )
        );
        assert_eq!(
            warn("lib/gen", "lib/gen", true).as_deref(),
            Some(
                "lib/gen is excluded by a rule in lib/.prettierignore, so nothing under it is formatted; narrow or negate that rule to format it"
            )
        );
        // `display` is the argument as given, whatever `rel` normalized it to
        assert!(
            warn("./build", "build", true)
                .is_some_and(|warning| warning.starts_with("./build is excluded by"))
        );
    }

    #[test]
    fn excluded_argument_warning_keeps_a_file_a_tsv_rule_excludes_quiet() {
        let mut stack = IgnoreStack::new();
        stack.push_formatignore("", "skip.ts\ngen/\n");
        stack.push_prettierignore("lib", "p.ts\n");
        for rel in ["skip.ts", "gen/a.ts", "lib/p.ts"] {
            // still out of scope — only the warning is withheld
            assert!(stack.is_ignored(rel, false), "{rel}");
            assert_eq!(
                excluded_argument_warning(rel, rel, false, None, &stack),
                None,
                "{rel}"
            );
        }
        // a directory the same rules exclude still warns
        assert!(excluded_argument_warning("gen", "gen", true, None, &stack).is_some());
    }

    #[test]
    fn excluded_argument_warning_names_the_repo_roots_own_tsv_file() {
        // a root reading `.prettierignore` is not told to create a `.formatignore` beside it,
        // which would shadow every rule in that `.prettierignore`
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\n");
        stack.push_prettierignore("", "vendor/\n");
        assert_eq!(
            excluded_argument_warning("build", "build", true, None, &stack).as_deref(),
            Some(
                "build is excluded by a rule in the repo-root .gitignore, so nothing under it is formatted; re-include it by adding `!/build/` to the repo-root .prettierignore"
            )
        );
        // a nested directory's `.prettierignore` is not the root's file
        let mut nested = IgnoreStack::new();
        nested.push_gitignore("", "build/\n");
        nested.push_prettierignore("src", "x\n");
        assert!(
            excluded_argument_warning("build", "build", true, None, &nested)
                .is_some_and(|warning| warning.ends_with("to the repo-root .formatignore"))
        );
    }

    #[test]
    fn excluded_argument_warning_names_a_tsv_rule_that_excludes_the_path_too() {
        // a `.gitignore` excluding a directory above the path does not hide the user's own
        // rule on it: re-including past the `.gitignore` would leave that rule standing —
        // and, added to the same file, override it — so a file it excludes stays quiet and
        // a directory names it
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "vendor/\n");
        stack.push_gitignore("pkg", "out/\n");
        stack.push_formatignore("", "vendor/x.ts\nvendor/lib/\n");
        stack.push_formatignore("pkg", "out/*.ts\n");
        let warn = |rel, is_dir| excluded_argument_warning(rel, rel, is_dir, None, &stack);
        assert_eq!(warn("vendor/x.ts", false), None);
        assert_eq!(warn("pkg/out/a.ts", false), None);
        assert_eq!(
            warn("vendor/lib", true).as_deref(),
            Some(
                "vendor/lib is excluded by a rule in the repo-root .formatignore, so nothing under it is formatted; narrow or negate that rule to format it"
            )
        );
        // a path only a `.gitignore` excludes still gets the lines that re-include it
        assert!(
            warn("vendor/y.ts", false).is_some_and(|warning| warning.contains("`!/vendor/y.ts`"))
        );
        assert!(warn("pkg/out", true).is_some_and(|warning| warning.contains("`!/pkg/out/`")));
    }

    #[test]
    fn excluded_argument_warning_lines_readmit_exactly_the_named_path() {
        // appended to the repo-root tsv file, the suggested lines put the named path back in
        // scope and leave everything beside it out, at every depth below the exclusion
        const GITIGNORE: &str = "build/\n*.gen.ts\n";
        let cases: &[(&str, bool, &[&str])] = &[
            ("build", true, &[]),
            ("build/a.ts", false, &["build/b.ts", "build/sub/a.ts"]),
            ("build/sub", true, &["build/a.ts", "build/other/a.ts"]),
            (
                "build/sub/deep/a.ts",
                false,
                &[
                    "build/a.ts",
                    "build/sub/a.ts",
                    "build/sub/deep/b.ts",
                    "build/sub/other/a.ts",
                ],
            ),
            ("src/x.gen.ts", false, &["src/y.gen.ts", "x.gen.ts"]),
            // a root-level file: the leading `/` keeps the line from matching at depth
            ("x.gen.ts", false, &["src/x.gen.ts"]),
            // every segment is spelled literally: a bracketed route directory is no
            // character class, a glob character or backslash in a name no pattern, and a
            // trailing space survives the line's trimming
            (
                "build/[slug]/a.ts",
                false,
                &["build/s/a.ts", "build/[slug]/b.ts"],
            ),
            ("build/[id]", true, &["build/i/z.ts"]),
            ("src/q?*.gen.ts", false, &["src/qx.gen.ts"]),
            ("build/back\\slash.ts", false, &["build/backslash.ts"]),
            ("build/space.ts ", false, &["build/space.ts"]),
            // a leading `#` or `!` is spelled after a `/`, where neither is special
            ("build/#x.ts", false, &["build/x.ts"]),
            ("build/!y.ts", false, &["build/y.ts"]),
        ];
        for &(rel, is_dir, still_excluded) in cases {
            let mut stack = IgnoreStack::new();
            stack.push_gitignore("", GITIGNORE);
            let warning = excluded_argument_warning(rel, rel, is_dir, None, &stack)
                .unwrap_or_else(|| panic!("{rel}: no warning"));
            // the backticked spans are the lines, in order
            let lines: Vec<&str> = warning.split('`').skip(1).step_by(2).collect();
            let mut fixed = IgnoreStack::new();
            fixed.push_gitignore("", GITIGNORE);
            fixed.push_formatignore("", &lines.join("\n"));
            assert!(!fixed.is_ignored(rel, is_dir), "{rel}: {lines:?}");
            if is_dir {
                assert!(!fixed.is_ignored(&format!("{rel}/z.ts"), false), "{rel}");
            }
            for other in still_excluded {
                assert!(fixed.is_ignored(other, false), "{rel}: {other} {lines:?}");
            }
        }
    }

    #[test]
    fn excluded_argument_warning_names_paths_absolutely_outside_a_repo() {
        let mut stack = IgnoreStack::new();
        stack.push_formatignore("home/u", "gen/\n");
        assert_eq!(
            excluded_argument_warning("gen/sub", "home/u/gen/sub", true, Some("/"), &stack)
                .as_deref(),
            Some(
                "gen/sub is inside /home/u/gen, which a rule in /home/u/.formatignore excludes, so nothing under it is formatted; narrow or negate that rule to format it"
            )
        );
        let mut windows = IgnoreStack::new();
        windows.push_formatignore("Users/u", "gen/\n");
        assert_eq!(
            excluded_argument_warning("gen\\sub", "Users/u/gen/sub", true, Some("C:\\"), &windows)
                .as_deref(),
            Some(
                "gen\\sub is inside C:\\Users\\u\\gen, which a rule in C:\\Users\\u\\.formatignore excludes, so nothing under it is formatted; narrow or negate that rule to format it"
            )
        );
    }

    #[test]
    fn excluded_argument_warning_reads_the_ignore_files_alone() {
        // the safety nets and the build-output heuristic prune what a walk discovers, never
        // a path an argument named — through an ancestor or at the path itself
        let empty = IgnoreStack::new();
        for (rel, is_dir) in [
            ("node_modules/pkg", true),
            ("node_modules", true),
            ("dist/sub", true),
            (".cache/x/a.ts", false),
            ("build/a.ts", false),
        ] {
            assert!(!empty.is_ignored(rel, is_dir), "{rel}");
            assert_eq!(
                excluded_argument_warning(rel, rel, is_dir, None, &empty),
                None,
                "{rel}"
            );
        }
        // a `.gitignore`'d directory a tsv layer re-includes is in scope
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\n");
        stack.push_formatignore("", "!build/\n");
        assert!(!stack.is_ignored("build/sub", true));
        assert_eq!(
            excluded_argument_warning("build/sub", "build/sub", true, None, &stack),
            None
        );
    }

    #[test]
    fn unresolvable_root_error_text_is_stable() {
        assert_eq!(
            unresolvable_root_error(".."),
            "..: cannot resolve a relative path: the working directory is unavailable"
        );
    }

    /// Assemble a stack the way a per-file consumer (the VS Code extension) does:
    /// every `.gitignore` layer shallow→deep, then every tsv layer shallow→deep.
    fn stack_from(gitignores: &[(&str, &str)], tsvs: &[(&str, &str)]) -> IgnoreStack {
        let mut stack = IgnoreStack::new();
        for (anchor, content) in gitignores {
            stack.push_gitignore(anchor, content);
        }
        for (anchor, content) in tsvs {
            stack.push_formatignore(anchor, content);
        }
        stack
    }

    #[test]
    fn path_pruned_under_gitignored_dir() {
        // a file under a gitignored `dist/` is pruned; a sibling under `src/` is not
        let stack = stack_from(&[("", "dist/\n")], &[]);
        assert!(is_path_pruned("dist/out.ts", &stack));
        assert!(!is_path_pruned("src/app.ts", &stack));
    }

    #[test]
    fn path_pruned_safety_net_with_no_ignore_files() {
        // safety nets prune unconditionally — even an empty stack prunes node_modules
        let stack = IgnoreStack::new();
        assert!(is_path_pruned("node_modules/pkg/index.ts", &stack));
        assert!(!is_path_pruned("src/app.ts", &stack));
    }

    #[test]
    fn root_level_file_is_never_path_pruned() {
        // no ancestor directories to prune (and an empty path is a no-op)
        let stack = stack_from(&[("", "dist/\n")], &[]);
        assert!(!is_path_pruned("app.ts", &stack));
        assert!(!is_path_pruned("", &stack));
    }

    #[test]
    fn loose_regime_heuristic_prunes_build_output_and_hidden() {
        // no `.gitignore` pushed (loose): the heuristic is on, so build/dist/target
        // and hidden dirs prune; a normal `src/` does not
        let stack = stack_from(&[], &[]);
        assert!(is_path_pruned("build/b.ts", &stack));
        assert!(is_path_pruned("dist/d.ts", &stack));
        assert!(is_path_pruned("target/t.ts", &stack));
        assert!(is_path_pruned(".hidden/h.ts", &stack));
        assert!(!is_path_pruned("src/app.ts", &stack));
    }

    #[test]
    fn repo_with_root_gitignore_turns_heuristic_off_for_build() {
        // mirrors the extension scenario: a repo with a root `.gitignore` ignoring
        // `dist/` — `dist/` prunes via the matcher, but `build/` is NOT a heuristic
        // prune (a `.gitignore` governs the level, so the heuristic is off)
        let stack = stack_from(&[("", "dist/\n")], &[]);
        assert!(is_path_pruned("dist/out.ts", &stack));
        assert!(!is_path_pruned("build/src.ts", &stack));
    }

    #[test]
    fn loose_tsv_reinclude_overrides_heuristic() {
        // a `.formatignore` `!build/` re-includes over the build-output heuristic;
        // a non-re-included `dist/` still prunes
        let stack = stack_from(&[], &[("", "!build/\n")]);
        assert!(!is_path_pruned("build/out.ts", &stack));
        assert!(is_path_pruned("dist/d.ts", &stack));
    }

    #[test]
    fn heuristic_active_reconstructed_per_level_from_deeper_gitignore() {
        // the case that proves the per-level reconstruction (and that the full-stack
        // assembly is sound where the incremental-walk assert would forbid it): a
        // `.gitignore` only at `sub/` turns the heuristic off for `sub`'s subtree but
        // not above it — top-level `build/` is still a heuristic prune, `sub/build/`
        // is not. (Calling the assert-bearing `classify_dir` with this full stack
        // would panic in debug; `is_path_pruned` uses `classify_dir_inner`.)
        let stack = stack_from(&[("sub", "# nothing\n")], &[]);
        assert!(is_path_pruned("build/x.ts", &stack)); // heuristic on above `sub`
        assert!(!is_path_pruned("sub/build/x.ts", &stack)); // heuristic off under `sub`'s .gitignore
    }

    #[test]
    fn nested_directory_under_gitignored_ancestor_is_pruned() {
        // git's parent-directory prune reaches an arbitrarily deep descendant
        let stack = stack_from(&[("", "vendored/\n")], &[]);
        assert!(is_path_pruned("vendored/deep/nested/v.svelte", &stack));
        assert!(!is_path_pruned("src/deep/nested/app.svelte", &stack));
    }

    #[test]
    fn path_heuristic_shadow_warning_names_a_reinclude_under_the_pruned_ancestor() {
        // loose: the heuristic prunes `dist`, which `!dist/keep.ts` was written to reach
        // into — the walk's per-directory warning, asked of a file under it
        let stack = stack_from(&[], &[("", "!dist/keep.ts\n")]);
        let warning = heuristic_shadow_warning("dist", None, &stack);
        assert!(warning.is_some());
        assert_eq!(
            path_heuristic_shadow_warning("dist/keep.ts", None, &stack),
            warning
        );
        // every file under the directory is skipped by the same prune, as the walk warns
        // once at the directory whichever file the rule names
        assert_eq!(
            path_heuristic_shadow_warning("dist/deep/other.ts", None, &stack),
            warning
        );
        // nested, and outside a repo: the deeper ancestor is the one asked
        let stack = stack_from(&[], &[("pkg", "!dist/keep.ts\n")]);
        let warning = heuristic_shadow_warning("pkg/dist", Some("/"), &stack);
        assert!(warning.is_some());
        assert_eq!(
            path_heuristic_shadow_warning("pkg/dist/keep.ts", Some("/"), &stack),
            warning
        );
    }

    #[test]
    fn path_heuristic_shadow_warning_is_silent_for_a_plain_prune() {
        // nothing prunes the path, or it has no ancestor to prune
        let stack = stack_from(&[], &[("", "!dist/keep.ts\n")]);
        assert_eq!(
            path_heuristic_shadow_warning("src/app.ts", None, &stack),
            None
        );
        assert_eq!(path_heuristic_shadow_warning("keep.ts", None, &stack), None);
        // the heuristic prunes with no re-include written under the directory
        assert!(is_path_pruned("dist/keep.ts", &IgnoreStack::new()));
        assert_eq!(
            path_heuristic_shadow_warning("dist/keep.ts", None, &IgnoreStack::new()),
            None
        );
        // a safety net prunes whatever is written under it, and says nothing
        let stack = stack_from(&[], &[("", "!node_modules/pkg/a.ts\n")]);
        assert!(is_path_pruned("node_modules/pkg/a.ts", &stack));
        assert_eq!(
            path_heuristic_shadow_warning("node_modules/pkg/a.ts", None, &stack),
            None
        );
        // the matcher prunes a directory that is no heuristic one: the re-include under it
        // is as inert, but the heuristic's warning would misname the cause
        let stack = stack_from(&[], &[("", "vendored/\n!vendored/keep.ts\n")]);
        assert!(is_path_pruned("vendored/keep.ts", &stack));
        assert!(heuristic_shadow_warning("vendored", None, &stack).is_some());
        assert_eq!(
            path_heuristic_shadow_warning("vendored/keep.ts", None, &stack),
            None
        );
        // a `.gitignore` above turns the heuristic off, so the walk enters `dist`
        let stack = stack_from(&[("", "# nothing\n")], &[("", "!dist/keep.ts\n")]);
        assert!(!is_path_pruned("dist/keep.ts", &stack));
        assert_eq!(
            path_heuristic_shadow_warning("dist/keep.ts", None, &stack),
            None
        );
    }

    #[test]
    fn path_heuristic_shadow_warning_asks_only_the_first_pruned_ancestor() {
        // the walk stops at `.cache`, so the warning is about `.cache`, not `.cache/dist`
        let stack = stack_from(&[], &[("", "!.cache/dist/keep.ts\n")]);
        assert_eq!(
            path_heuristic_shadow_warning(".cache/dist/keep.ts", None, &stack),
            heuristic_shadow_warning(".cache", None, &stack)
        );
        // a rule written inside the pruned directory is one the walk never reads
        let stack = stack_from(&[], &[("dist", "!sub/keep.ts\n")]);
        assert!(is_path_pruned("dist/sub/keep.ts", &stack));
        assert_eq!(
            path_heuristic_shadow_warning("dist/sub/keep.ts", None, &stack),
            None
        );
    }
}
