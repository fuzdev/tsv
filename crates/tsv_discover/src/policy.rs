//! The walk policy: what a discovery prunes, what it formats, and the per-file replay of
//! both for a consumer with no walk of its own — the [safety nets](SAFETY_NET_DIRS), the
//! build-output [heuristic](HEURISTIC_DIRS), the [extension set](FORMATTABLE_EXTENSIONS),
//! and the verdicts over them ([`classify_dir`], [`should_format_file`],
//! [`is_path_pruned`], with [`path_shadow_warning`] the per-file replay of the walk's
//! shadow warning beside the last). Pure — no filesystem access; the matcher is
//! `tsv_ignore`'s.

use crate::quote::quote_path;
use crate::warnings::shadow_warning;
use std::path::Path;
use tsv_ignore::IgnoreStack;

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
    /// Skip the directory and surface a non-fatal stderr warning: the directory is
    /// pruned — by the build-output heuristic or by an ignore rule — while an
    /// *anchored* tsv-layer `!` was trying to re-include *into* it, a silent no-op under
    /// git's parent-directory rule. The caller fetches the message from
    /// [`shadow_warning`], which reads the cause off the stack and also takes the format
    /// root's display path outside a repo — context the per-directory verdict has no
    /// other use for.
    PruneWithWarning,
}

/// Whether a file name has a [formattable extension](FORMATTABLE_EXTENSIONS)
/// (the JS/TS family, `.svelte`, `.css` — compound forms like `.svelte.ts` are
/// covered by the `.ts` match). Matches `Path::extension`, so a bare dotfile like
/// `.ts` is a stem with no extension and is **not** formattable. Accepts a bare
/// name or a whole path — `Path::extension` reads the final component either way.
///
/// The extension is read without regard to ASCII case (`A.TS`, `styles.CSS`), as
/// prettier infers a parser from the lowercased name: a tree that passed through a
/// case-insensitive filesystem carries such names, and each is the same kind of file.
/// Every extension dispatch in both `tsv` bins reads the case the same way
/// (`ParserType::from_extension`, `tsv_ts::Goal::from_extension`, `cli.js`'s twins), so a
/// walk and a named path agree on what `A.TS` is.
pub fn is_formattable(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            FORMATTABLE_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(ext))
        })
}

/// The error for an **explicitly named file argument** whose extension tsv does
/// not format, or `None` when [`is_formattable`] accepts it.
///
/// A file argument is held to this before anything else — before the ignore files
/// bound it ([`excluded_argument_warning`](crate::excluded_argument_warning)) — because the parser dispatch behind it
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
/// message. Produced **once**, here — like [`shadow_warning`] — so the
/// native CLI and the WASM CLI emit the identical text. `parse <file>` refuses with
/// the same message (the parser dispatch behind a path is the same one), which is
/// why it says what tsv *handles* rather than what it formats.
#[must_use]
pub fn unsupported_extension_error(path: &str) -> Option<String> {
    (!is_formattable(path)).then(|| {
        let list = formattable_extension_list(", ");
        format!(
            "{}: unsupported file extension (tsv handles {list})",
            quote_path(path)
        )
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
/// `!` re-includes it; finally the matcher prunes a directory a rule excludes. Under
/// either prune, an *anchored* `!dir/<file>` trying to reach inside the directory is a
/// no-op (git's parent-directory rule), and the verdict is then
/// [`DirVerdict::PruneWithWarning`]. Otherwise, descend.
///
/// The matcher half is the **leaf-only** query
/// ([`is_ignored_leaf`](tsv_ignore::IgnoreStack::is_ignored_leaf)): it is exact only
/// once the directory's ancestors are cleared, which holds for every directory a walk
/// descends into but not for a walk's root — a caller gates its root with the full
/// [`is_ignored`](tsv_ignore::IgnoreStack::is_ignored) first, as both CLIs do.
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
    debug_assert!(
        child_rel.rsplit('/').next() == Some(name),
        "classify_dir: {name:?} is not the last segment of child_rel {child_rel:?}",
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
        // a tsv-layer `!child_rel/<file>` is as inert under this prune as under the
        // heuristic's; the warning names the rule as the cause
        return if stack.negation_under(child_rel).is_some() {
            DirVerdict::PruneWithWarning
        } else {
            DirVerdict::Prune
        };
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
/// files alone ([`excluded_argument_warning`](crate::excluded_argument_warning)).
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

/// The warning a walk raises on the way down to `rel`: the [`shadow_warning`]
/// for the first ancestor directory [`is_path_pruned`] stops at, when that directory's
/// verdict is [`DirVerdict::PruneWithWarning`]. `None` when no ancestor prunes `rel`, and
/// when the one that does is a plain prune — a safety net, or the heuristic or a rule
/// with no tsv-layer re-include written under the directory. The per-file companion to
/// [`classify_dir`] + [`shadow_warning`], for [`is_path_pruned`]'s consumer: a
/// document skipped because its directory is pruned, while a re-include written to reach
/// it does nothing, is a misconfiguration that consumer can name without a walk.
///
/// Only the first pruned ancestor is asked, since a walk stops there, and the verdict is
/// read rather than [`shadow_warning`] asked directly so a safety-net prune stays silent
/// as it does on a walk. `stack` is [`is_path_pruned`]'s; `loose_root` is
/// [`shadow_warning`]'s.
#[must_use]
pub fn path_shadow_warning(
    rel: &str,
    loose_root: Option<&str>,
    stack: &IgnoreStack,
) -> Option<String> {
    let (dir, verdict) = first_pruned_ancestor(rel, stack)?;
    if verdict == DirVerdict::PruneWithWarning {
        shadow_warning(&dir, loose_root, stack)
    } else {
        None
    }
}

/// The first STRICT ancestor directory of `rel` the traversal would not descend into, with
/// its verdict — the replay behind [`is_path_pruned`] and
/// [`path_shadow_warning`].
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
/// its ignore rules by [`excluded_argument_warning`](crate::excluded_argument_warning), which walks the ancestors this
/// leaf query takes as cleared.)
///
/// Uses the leaf-only [`is_ignored_leaf`](tsv_ignore::IgnoreStack::is_ignored_leaf):
/// the discovery walk only reaches a file whose ancestor directories are already
/// cleared (it prunes ignored dirs before descending, and the caller gates the
/// root), so the ancestor walk would be redundant — see [`classify_dir`].
pub fn should_format_file(name: &str, child_rel: &str, stack: &IgnoreStack) -> bool {
    debug_assert!(
        child_rel.rsplit('/').next() == Some(name),
        "should_format_file: {name:?} is not the last segment of child_rel {child_rel:?}",
    );
    is_formattable(name) && !stack.is_ignored_leaf(child_rel, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{stack_from, tsv_stack};
    use tsv_ignore::IgnoreStack;

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
        // the case of the extension is not a different kind of file: prettier infers a
        // parser from the lowercased name, and tsv reads it the same way
        assert!(is_formattable("A.TS"));
        assert!(is_formattable("styles.CSS"));
        assert!(is_formattable("App.Svelte"));
        assert!(is_formattable("legacy.MJS"));
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
    fn path_shadow_warning_names_a_reinclude_under_the_pruned_ancestor() {
        // loose: the heuristic prunes `dist`, which `!dist/keep.ts` was written to reach
        // into — the walk's per-directory warning, asked of a file under it
        let stack = stack_from(&[], &[("", "!dist/keep.ts\n")]);
        let warning = shadow_warning("dist", None, &stack);
        assert!(warning.is_some());
        assert_eq!(path_shadow_warning("dist/keep.ts", None, &stack), warning);
        // every file under the directory is skipped by the same prune, as the walk warns
        // once at the directory whichever file the rule names
        assert_eq!(
            path_shadow_warning("dist/deep/other.ts", None, &stack),
            warning
        );
        // nested, and outside a repo: the deeper ancestor is the one asked
        let stack = stack_from(&[], &[("pkg", "!dist/keep.ts\n")]);
        let warning = shadow_warning("pkg/dist", Some("/"), &stack);
        assert!(warning.is_some());
        assert_eq!(
            path_shadow_warning("pkg/dist/keep.ts", Some("/"), &stack),
            warning
        );
    }

    #[test]
    fn path_shadow_warning_is_silent_for_a_plain_prune() {
        // nothing prunes the path, or it has no ancestor to prune
        let stack = stack_from(&[], &[("", "!dist/keep.ts\n")]);
        assert_eq!(path_shadow_warning("src/app.ts", None, &stack), None);
        assert_eq!(path_shadow_warning("keep.ts", None, &stack), None);
        // the heuristic prunes with no re-include written under the directory
        assert!(is_path_pruned("dist/keep.ts", &IgnoreStack::new()));
        assert_eq!(
            path_shadow_warning("dist/keep.ts", None, &IgnoreStack::new()),
            None
        );
        // a safety net prunes whatever is written under it, and says nothing
        let stack = stack_from(&[], &[("", "!node_modules/pkg/a.ts\n")]);
        assert!(is_path_pruned("node_modules/pkg/a.ts", &stack));
        assert_eq!(
            path_shadow_warning("node_modules/pkg/a.ts", None, &stack),
            None
        );
        // the matcher prunes a directory that is no heuristic one: the re-include under it
        // is as inert, and the warning names the rule as the cause
        let stack = stack_from(&[], &[("", "vendored/\n!vendored/keep.ts\n")]);
        assert!(is_path_pruned("vendored/keep.ts", &stack));
        let warning = shadow_warning("vendored", None, &stack);
        assert!(
            warning
                .as_deref()
                .is_some_and(|w| w.contains("excluded by a rule"))
        );
        assert_eq!(
            path_shadow_warning("vendored/keep.ts", None, &stack),
            warning
        );
        // no re-include under a matcher-pruned directory: nothing to say
        let stack = stack_from(&[("", "vendored/\n")], &[]);
        assert!(is_path_pruned("vendored/keep.ts", &stack));
        assert_eq!(path_shadow_warning("vendored/keep.ts", None, &stack), None);
        // a `.gitignore` above turns the heuristic off, so the walk enters `dist`
        let stack = stack_from(&[("", "# nothing\n")], &[("", "!dist/keep.ts\n")]);
        assert!(!is_path_pruned("dist/keep.ts", &stack));
        assert_eq!(path_shadow_warning("dist/keep.ts", None, &stack), None);
    }

    #[test]
    fn path_shadow_warning_asks_only_the_first_pruned_ancestor() {
        // the walk stops at `.cache`, so the warning is about `.cache`, not `.cache/dist`
        let stack = stack_from(&[], &[("", "!.cache/dist/keep.ts\n")]);
        assert_eq!(
            path_shadow_warning(".cache/dist/keep.ts", None, &stack),
            shadow_warning(".cache", None, &stack)
        );
        // a rule written inside the pruned directory is one the walk never reads
        let stack = stack_from(&[], &[("dist", "!sub/keep.ts\n")]);
        assert!(is_path_pruned("dist/sub/keep.ts", &stack));
        assert_eq!(path_shadow_warning("dist/sub/keep.ts", None, &stack), None);
    }

    #[test]
    #[should_panic(expected = "child_rel")]
    fn classify_dir_asserts_the_name_is_the_paths_last_segment() {
        let stack = IgnoreStack::new();
        let _ = classify_dir("src", "build", true, &stack);
    }
}
