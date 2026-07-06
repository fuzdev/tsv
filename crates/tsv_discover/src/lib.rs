//! tsv's file-discovery **policy** — the per-directory and per-file decisions
//! `tsv format` makes while walking a tree, as pure functions over a
//! [`tsv_ignore::IgnoreStack`] (the matcher) plus the entry name and its
//! format-root-relative path.
//!
//! This is the single home of the build-output heuristic, the always-pruned
//! safety nets, the formattable-extension check, the heuristic-shadow warning
//! text, and the `.prettierignore`-outside-a-repo warning. The three discovery
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
use tsv_ignore::IgnoreStack;

/// Directory names skipped during discovery in **every** mode — VCS metadata and
/// `node_modules`. Catastrophic or pointless to recurse, and not reliably listed
/// in `.gitignore` (a committed `node_modules`, a jj-colocated `.jj`). `.git` is
/// matched as a directory name here; its worktree/submodule *file* form is a file
/// and is never recursed regardless.
pub const SAFETY_NET_DIRS: [&str; 5] = ["node_modules", ".git", ".hg", ".svn", ".jj"];

/// Heuristic-only directory skips — tsv's fallback guess at "generated /
/// vendored / build output", applied **only** while no `.gitignore` governs the
/// directory (`heuristic_active`). Hidden directories (a leading `.`) are skipped
/// by the same heuristic. Once a `.gitignore` is in play the project's own rules
/// decide, so these are recursed unless the project ignores them.
pub const HEURISTIC_DIRS: [&str; 3] = ["dist", "build", "target"];

/// The file extensions tsv formats — the discovery filter behind
/// [`is_formattable`]. Compound forms like `.svelte.ts` are covered by the `ts`
/// entry (`Path::extension` yields the final component).
pub const FORMATTABLE_EXTENSIONS: [&str; 3] = ["ts", "svelte", "css"];

/// The discovery verdict for one child **directory**: descend into it, prune it
/// (skip its whole subtree), or prune it **and** surface a diagnostic.
#[derive(Debug, PartialEq, Eq)]
pub enum DirVerdict {
    /// Recurse into the directory.
    Descend,
    /// Skip the directory and its subtree.
    Prune,
    /// Skip the directory and surface this non-fatal stderr warning: the
    /// build-output heuristic pruned a directory that an *anchored* tsv-layer `!`
    /// was trying to re-include *into* (a silent no-op). The string is the full
    /// message (see [`heuristic_shadow_warning`]); the caller reports it without
    /// re-deriving it.
    PruneWithWarning(String),
}

/// Whether a file name has a [formattable extension](FORMATTABLE_EXTENSIONS)
/// (`.ts`, `.svelte`, `.css` — compound forms like `.svelte.ts` are covered by the
/// `.ts` match). Matches `Path::extension`, so a bare dotfile like `.ts` is a stem
/// with no extension and is **not** formattable.
pub fn is_formattable(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| FORMATTABLE_EXTENSIONS.contains(&ext))
}

/// Whether a directory `name` is an always-pruned [safety net](SAFETY_NET_DIRS)
/// (`.git`/`node_modules`/`.hg`/`.svn`/`.jj`). A **complete, context-free**
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
/// verdict carries the [shadow warning](DirVerdict::PruneWithWarning); finally the
/// matcher prunes anything it ignores. Otherwise, descend.
pub fn classify_dir(
    name: &str,
    child_rel: &str,
    heuristic_active: bool,
    stack: &IgnoreStack,
) -> DirVerdict {
    // `heuristic_active` implies no `.gitignore` layer is pushed, so the
    // `is_reincluded` / `has_negation_under` consulted below see only the tsv
    // layer. That implication is load-bearing (a stray `.gitignore` negation would
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
/// see exactly the ancestors they would mid-walk; and the one place a deeper layer
/// *is* consulted — `has_negation_under`, picking `Prune` vs `PruneWithWarning` —
/// only affects the warning, which a boolean prune query collapses away.
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
        return if stack.has_negation_under(child_rel) {
            DirVerdict::PruneWithWarning(heuristic_shadow_warning(child_rel))
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

/// Whether `rel` — a format-root-relative, `/`-separated **file** path — is
/// skipped because some ancestor directory would be pruned by the traversal (a
/// [safety net](SAFETY_NET_DIRS), the build-output heuristic, or the matcher)
/// before the walk reaches the file. The per-file companion to [`classify_dir`]
/// for a consumer that has **no top-down traversal**: the VS Code extension
/// formats one open document at a time, so it can't thread `heuristic_active`
/// down a walk.
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
    let segments: Vec<&str> = rel
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    // a root-level file (or empty path) has no ancestor directories to prune
    if segments.len() < 2 {
        return false;
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
        if !matches!(
            classify_dir_inner(name, &child_rel, heuristic_active, stack),
            DirVerdict::Descend
        ) {
            return true;
        }
    }
    false
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
/// filesystem access. (An explicitly named file *argument* bypasses this — the
/// ignore files govern *discovery*, which is what this drives.)
///
/// Uses the leaf-only [`is_ignored_leaf`](tsv_ignore::IgnoreStack::is_ignored_leaf):
/// the discovery walk only reaches a file whose ancestor directories are already
/// cleared (it prunes ignored dirs before descending, and the caller gates the
/// root), so the ancestor walk would be redundant — see [`classify_dir`].
pub fn should_format_file(name: &str, child_rel: &str, stack: &IgnoreStack) -> bool {
    is_formattable(name) && !stack.is_ignored_leaf(child_rel, false)
}

/// The stderr warning when the build-output heuristic prunes a directory that a
/// tsv-layer `!` rule was trying to re-include *into*. The re-include is a silent
/// no-op — git's parent-directory rule (matched in the `.gitignore` regime) bars
/// re-including a descendant of an excluded directory — so the message points at
/// the dir-level escape that does work. `d` is the pruned directory, format-root
/// relative. Produced **once**, here: [`classify_dir`] carries it in
/// [`DirVerdict::PruneWithWarning`], and the WASM binding fetches it directly, so
/// no caller re-templates the text.
pub fn heuristic_shadow_warning(d: &str) -> String {
    format!(
        "{d} is skipped by tsv's build-output heuristic, so a `!{d}/<file>` re-include under it does nothing; re-include the directory itself with `!{d}/` (then `{d}/*` + `!{d}/<file>` to select files within it)"
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tsv_stack(content: &str) -> IgnoreStack {
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", content);
        stack
    }

    #[test]
    fn is_formattable_matches_supported_extensions() {
        assert!(is_formattable("a.ts"));
        assert!(is_formattable("a.svelte"));
        assert!(is_formattable("a.css"));
        assert!(is_formattable("a.svelte.ts")); // .ts wins
        assert!(!is_formattable("a.txt"));
        assert!(!is_formattable("a")); // no extension
        assert!(!is_formattable(".ts")); // bare dotfile: a stem, no extension
        assert!(!is_formattable("Makefile"));
    }

    #[test]
    fn is_safety_net_matches_the_constant() {
        for name in SAFETY_NET_DIRS {
            assert!(is_safety_net(name), "{name}");
        }
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
            DirVerdict::PruneWithWarning(heuristic_shadow_warning("build")),
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
        // pinned verbatim — the native CLI carries it and the WASM binding fetches
        // it, so both surfaces emit this exact string
        assert_eq!(
            heuristic_shadow_warning("dist"),
            "dist is skipped by tsv's build-output heuristic, so a `!dist/<file>` re-include under it does nothing; re-include the directory itself with `!dist/` (then `dist/*` + `!dist/<file>` to select files within it)"
        );
    }

    #[test]
    fn prettierignore_outside_repo_warns_only_when_unshadowed_outside_a_repo() {
        // the footgun: outside a repo, a target-root `.prettierignore` with no
        // `.formatignore` beside it is silently skipped → warn
        assert!(prettierignore_outside_repo_warning("proj", false, true, false).is_some());
        // inside a repo the repo-root `.prettierignore` IS read → no warning
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

    /// Assemble a stack the way a per-file consumer (the VS Code extension) does:
    /// every `.gitignore` layer shallow→deep, then every tsv layer shallow→deep.
    fn stack_from(gitignores: &[(&str, &str)], tsvs: &[(&str, &str)]) -> IgnoreStack {
        let mut stack = IgnoreStack::new();
        for (anchor, content) in gitignores {
            stack.push_gitignore(anchor, content);
        }
        for (anchor, content) in tsvs {
            stack.push_tsv(anchor, content);
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
}
