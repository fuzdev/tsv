use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Component, Path, PathBuf};
use tsv_discover::{
    DirVerdict, classify_dir, prettierignore_outside_repo_warning, prettierignore_shadowed_warning,
    should_format_file, unsupported_extension_error,
};
use tsv_ignore::IgnoreStack;

/// tsv's native ignore file, discovered hierarchically (one per directory, in a
/// repo from the repo root down, outside a repo from the filesystem root down).
const FORMATIGNORE_FILE: &str = ".formatignore";

/// Prettier's ignore file — read hierarchically **inside a git repo** (one per
/// directory, from the repo root down, like `.formatignore`) for drop-in compat,
/// and **shadowed by presence** of a *sibling* `.formatignore`: when both sit in
/// one directory the `.formatignore` is used alone for that directory, even if
/// it's present-but-unreadable (so a read error can't silently demote tsv's native
/// file to prettier's — see [`read_ignore_file`]); the shadow is flagged by a
/// heads-up ([`prettierignore_shadowed_warning`]). Never read outside a git repo —
/// there a target-root `.prettierignore` triggers a heads-up warning instead (see
/// [`prettierignore_outside_repo_warning`]).
const PRETTIERIGNORE_FILE: &str = ".prettierignore";

/// The git ignore file, discovered hierarchically (one per directory) and only
/// inside a git repo — matching git, which honors `.gitignore` only in a worktree.
const GITIGNORE_FILE: &str = ".gitignore";

/// Where a walk's in-scope files go.
///
/// The walk finds files one directory at a time and has no use for the set as a
/// whole, so the destination is abstract: [`discover_files`] collects into a
/// `Vec` and sorts, while [`discover_into`] hands each file straight to a
/// consumer that can start work on it. Implementors that buffer flush on
/// [`FileSink::flush`], which the walk calls at every directory boundary.
pub trait FileSink {
    fn push(&mut self, path: PathBuf);
    /// Called when the walk finishes a directory — a buffering sink should
    /// release what it holds so a consumer isn't waiting on the next directory.
    fn flush(&mut self) {}
}

impl FileSink for Vec<PathBuf> {
    fn push(&mut self, path: PathBuf) {
        Vec::push(self, path);
    }
}

/// The walk's output while it runs: files go to the sink as they're found,
/// diagnostics accumulate here. Both entry points build their result from this.
struct Walk<'a> {
    files: &'a mut dyn FileSink,
    errors: Vec<String>,
    warnings: Vec<String>,
}

/// The non-file half of a walk's result — reported by the caller, and sorted +
/// deduplicated (overlapping roots re-walk a shared subtree, so the same failure
/// would otherwise report twice).
pub struct Diagnostics {
    /// `path: detail` messages — reported by the caller, counted as errors.
    pub errors: Vec<String>,
    /// Non-fatal diagnostics — reported to stderr by the caller but **not**
    /// counted as errors (no effect on the exit code or stdout): the
    /// shadow warning ([`tsv_discover::shadow_warning`]), the
    /// `.prettierignore`-shadowed-by-a-sibling-`.formatignore` warning
    /// ([`prettierignore_shadowed_warning`]), the `.prettierignore`-outside-a-repo
    /// warning ([`prettierignore_outside_repo_warning`]), the excluded-argument warning
    /// for a named path an ignore file puts out of scope
    /// ([`tsv_discover::excluded_argument_warning`]), and the unreadable-ignore-file
    /// warning (a present `.gitignore`/`.formatignore`/`.prettierignore` whose read
    /// failed; see [`read_ignore_file`]).
    pub warnings: Vec<String>,
    /// Whether every argument was a named file an ignore rule excluded — what lets a caller
    /// tell such a run (not an error: a pre-commit hook with only ignored files staged)
    /// from one that found nothing.
    pub all_arguments_excluded: bool,
}

/// Result of expanding path arguments: the files to format, sorted and deduplicated,
/// plus the walk's [`Diagnostics`].
pub struct Discovered {
    pub files: Vec<PathBuf>,
    pub diagnostics: Diagnostics,
}

/// Expand files and directories into a sorted, deduplicated list of files to format.
///
/// All root arguments are validated upfront: any that don't resolve to a file or
/// directory, and any **file** argument whose extension tsv doesn't format
/// ([`unsupported_extension_error`]), fail the whole run before anything is
/// formatted (`Err` carries one message per bad argument). Traversal errors below
/// a valid root are non-fatal — collected in `Discovered::diagnostics` while the rest
/// of the tree continues.
///
/// A path an argument names is bounded by the ignore files alone: a file or directory
/// root a rule excludes — through an ancestor directory, or at itself — is skipped, with the
/// warning [`tsv_discover::excluded_argument_warning`] calls for (none for a file a
/// `.formatignore` or `.prettierignore` rule excludes), while the safety nets and the
/// build-output heuristic, which prune what a walk *discovers*, never grade one. A file
/// argument is held to its extension first: the parser dispatch behind it has no unknown
/// arm, so an unsupported extension is an argument error rather than a silent TypeScript
/// parse. A **directory** argument is a scope, not a target, so the extension check
/// doesn't apply to it — its contents are filtered by the walk. Directories recurse with
/// the extension filter. Symlinks inside directories are not followed (cycle safety) —
/// pass them explicitly to format their targets.
///
/// # Ignore semantics (two regimes, keyed on `.git`)
///
/// For each directory root, the **format root** is the boundary the walk is
/// anchored on, resolved from the root argument (never the cwd, so a target's
/// scope is the same however its path is spelled and wherever tsv runs):
///
/// - **inside a git repo** (a `.git` ancestor exists), the format root is the
///   **repo root** — a hard stop, nothing above it is read, so `tsv format
///   --check` is reproducible across machines. Discovery honors `.gitignore`
///   hierarchically and repo-rooted like git (`git check-ignore` parity on a
///   case-sensitive filesystem); `.formatignore` and `.prettierignore` both
///   hierarchically from the repo root down (each `.prettierignore` shadowed by a
///   sibling `.formatignore`) as drop-in compat. The tsv files override
///   `.gitignore`.
/// - **outside a git repo**, the format root is the **filesystem root**:
///   `.gitignore` is not consulted at all (as git itself does), and
///   `.formatignore` is honored hierarchically from the filesystem root down — so
///   a `~/.formatignore` is global config for loose files. `.prettierignore` is
///   repo-only and not read here — but a `.prettierignore` in the **target root**
///   raises a non-fatal heads-up (rename it to `.formatignore`, or `git init`),
///   since prettier would have honored it
///   ([`prettierignore_outside_repo_warning`]).
///
/// Inside a repo both `.formatignore` and `.prettierignore` are hierarchical (a
/// file in any directory governs its subtree, deeper wins; a `.prettierignore` is
/// shadowed by a sibling `.formatignore`). The **safety nets**
/// (`tsv_discover::SAFETY_NET_DIRS`) are pruned wherever a walk finds them.
///
/// When a `.gitignore` governs a directory, that is the authority and the
/// `tsv_discover::HEURISTIC_DIRS` + hidden-directory **heuristic is off** — so a real source
/// `build/` (not gitignored) is formatted. Its mere *presence* is the
/// declaration: an empty or comments-only `.gitignore` still turns the heuristic
/// off (and thus formats a non-ignored `dist/`), matching git, for which an empty
/// `.gitignore` ignores nothing. With no `.gitignore` in scope the heuristic is
/// the fallback "not source" guess, except that an explicit tsv-layer `!`
/// re-include overrides it. Because the format root is found by
/// walking up, the repo-root ignore files apply even when tsv is invoked on a
/// subdirectory, and a subdirectory named directly is bounded by the same ignore
/// rules as when it is reached via an ancestor.
///
/// With multiple roots, overlapping spellings of the same file (`src` vs
/// `./src`, absolute vs relative, symlink aliases) are deduplicated by canonical
/// path, keeping the first spelling in sorted order. A single root can't produce
/// duplicates (symlinks aren't followed), and neither can roots that don't overlap —
/// so the per-file `realpath` runs only when one canonical root is an ancestor-or-self
/// of another, or a file argument is present (a file can be named twice, or sit under
/// a directory root); `tsv format src lib` pays one `realpath` per ROOT, not per file.
pub fn discover_files(paths: &[String]) -> Result<Discovered, Vec<String>> {
    let args = classify_args(paths)?;
    let mut files: Vec<PathBuf> = Vec::new();
    let diagnostics = walk_args(&args, &mut files);
    files.sort_by_cached_key(|p| path_sort_key(p));
    files.dedup();
    if roots_can_overlap(&args) {
        let mut seen: HashSet<PathBuf> = HashSet::with_capacity(files.len());
        files.retain(|p| seen.insert(fs::canonicalize(p).unwrap_or_else(|_| p.clone())));
    }
    Ok(Discovered { files, diagnostics })
}

/// Whether two of `args` can yield the same file: a file argument among them (it
/// can repeat, or sit under a directory root), or one canonical directory root an
/// ancestor-or-self of another. A `canonicalize` failure after [`classify_args`]
/// read the argument is a race, and reads as "may overlap".
fn roots_can_overlap(args: &[Arg<'_>]) -> bool {
    if args.len() < 2 {
        return false;
    }
    let mut roots = Vec::with_capacity(args.len());
    for &arg in args {
        let Arg::Dir(path) = arg else {
            return true;
        };
        let Ok(canonical) = fs::canonicalize(path) else {
            return true;
        };
        roots.push(canonical);
    }
    roots.iter().enumerate().any(|(i, a)| {
        roots
            .iter()
            .enumerate()
            .any(|(j, b)| i != j && b.starts_with(a))
    })
}

/// The walk behind [`discover_files`], with the files handed to `sink` as they
/// are found rather than returned as a set.
///
/// Same scope rules, same file set, same diagnostics — the only difference is
/// that **nothing orders or deduplicates the files**: they arrive in walk order
/// (directory listing order, depth-first), and `sink` owns whatever ordering its
/// consumer needs. `discover_files` is this plus the sort, the exact-duplicate
/// `dedup`, and the canonical-path dedup that multiple roots require.
///
/// A caller that streams therefore takes on two obligations:
/// - **report in sorted-path order anyway** (the `format` command's contract) —
///   sort at the end, don't emit as results land;
/// - **only stream a single root.** Overlapping roots (`tsv format src src/a.ts`)
///   can yield the same file twice, and the dedup that removes it needs the whole
///   set. One root can't: symlinks aren't followed, so each entry is visited once.
pub fn discover_into(
    paths: &[String],
    sink: &mut dyn FileSink,
) -> Result<Diagnostics, Vec<String>> {
    let args = classify_args(paths)?;
    Ok(walk_args(&args, sink))
}

/// Classify each path argument with one `stat` apiece, kept for everything after —
/// the walk's dispatch and [`roots_can_overlap`] read the same answer rather than a
/// second query that could disagree with it.
///
/// Both argument errors fail the run upfront, all reported, nothing written: a path
/// that resolves to neither a file nor a directory, and a **file** argument tsv
/// doesn't format. The extension check applies only to file arguments — a directory
/// is a scope, and its contents are filtered by the walk — and a file argument is held
/// to it first, before the ignore files (see `unsupported_extension_error`: the parser
/// dispatch behind it has no unknown arm, so an unsupported extension would be parsed
/// as TypeScript). `Ok` carries every argument, in order.
fn classify_args(paths: &[String]) -> Result<Vec<Arg<'_>>, Vec<String>> {
    let mut args = Vec::with_capacity(paths.len());
    let mut bad = Vec::new();
    for p in paths {
        match fs::metadata(p) {
            Ok(metadata) if metadata.is_dir() => args.push(Arg::Dir(p)),
            Ok(metadata) if metadata.is_file() => match unsupported_extension_error(p) {
                Some(error) => bad.push(error),
                None => args.push(Arg::File(p)),
            },
            _ => bad.push(format!("{p}: not a file or directory")),
        }
    }
    if bad.is_empty() { Ok(args) } else { Err(bad) }
}

/// The walk over the arguments [`classify_args`] accepted: a file argument goes to
/// `sink` unless an ignore file excludes it ([`collect_file`]), a directory is walked.
fn walk_args(args: &[Arg<'_>], sink: &mut dyn FileSink) -> Diagnostics {
    // canonical cwd so it compares cleanly with canonicalized roots below; `None` when it
    // cannot be resolved (a deleted working directory), which only a relative root that
    // fails to canonicalize ever asks for
    let cwd = std::env::current_dir().and_then(fs::canonicalize).ok();

    // accumulate directly into the walk struct so it threads one `&mut` sink
    // rather than a parallel set of vectors
    let mut out = Walk {
        files: sink,
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    // every file argument is graded in one scope, moved from each one's directory to the
    // next's rather than rebuilt per directory
    let mut file_scope = FileScope::default();
    let mut excluded_files = 0;
    for &arg in args {
        match arg {
            // the extension check is `classify_args`'s, already applied
            Arg::File(path) => {
                if collect_file(path, cwd.as_deref(), &mut file_scope, &mut out) {
                    excluded_files += 1;
                }
            }
            Arg::Dir(path) => collect_root(Path::new(path), cwd.as_deref(), &mut out),
        }
    }
    out.files.flush();
    // dedupe both channels: overlapping roots re-walk the shared subtree, so the
    // same unreadable path / pruned directory would otherwise report twice. The
    // strings are byte-identical only for the same underlying failure, so this
    // collapses duplicates without hiding a distinct one.
    out.errors.sort();
    out.errors.dedup();
    out.warnings.sort();
    out.warnings.dedup();
    Diagnostics {
        errors: out.errors,
        warnings: out.warnings,
        all_arguments_excluded: !args.is_empty() && excluded_files == args.len(),
    }
}

/// One path argument and what it resolved to, decided once by [`classify_args`].
#[derive(Clone, Copy)]
enum Arg<'a> {
    File(&'a str),
    Dir(&'a str),
}

/// Set up the ignore evaluation for one directory `root`, then recurse into it.
///
/// Resolves the **format root** — the repo root inside a git tree, else the
/// filesystem root — and preloads the [`IgnoreStack`] for the ancestors *above*
/// `root` (format root down to `root`'s parent): `.formatignore` at each level
/// (and, inside a repo, a `.prettierignore` it shadows per-directory), and
/// `.gitignore` at each level when in a repo. `root` itself, and everything below it, reads its own ignore
/// files in [`collect_recursive`] from the directory listing it already fetches,
/// so an ignore-file-free subtree costs no speculative opens. The heuristic is
/// seeded off once any `.gitignore` above `root` is in scope. `root`'s display
/// spelling is preserved for the emitted paths; matching uses the
/// format-root-relative path threaded down the walk.
fn collect_root(root: &Path, cwd: Option<&Path>, out: &mut Walk<'_>) {
    let Some(root_abs) = absolute_named_path(root, cwd) else {
        out.errors.push(tsv_discover::unresolvable_root_error(
            &root.to_string_lossy(),
        ));
        return;
    };
    let (format_root, in_repo) = format_root_of(&root_abs);

    // `root` relative to the format root (an ancestor-or-self of `root_abs`, so
    // this never fails; `""` means `root` *is* the format root).
    let base_rel = rel_to(&format_root, &root_abs);

    // Preload the ancestors *above* `root` (format root → `root`'s parent). `root`
    // itself is excluded: `collect_recursive` reads its ignore files from the
    // listing it fetches anyway.
    let chain = ancestor_chain(&format_root, &root_abs);
    let (mut stack, heuristic_active) = preload_ancestors(
        &chain[..chain.len() - 1],
        &format_root,
        in_repo,
        &mut out.warnings,
    );

    // A named root is bounded by the ignore files alone, gated here once with the full,
    // ancestor-walking matcher: a root a rule excludes — through an ancestor (`tsv format
    // build/sub` with a gitignored `build/`) or at itself — puts nothing under it in
    // scope, and the run says so. The recursion's leaf-only query (`tsv_discover` calls
    // `is_ignored_leaf`) is exact only once an entry's ancestors are cleared, which holds
    // for everything the walk descends into but not for `root`, so this gate is also
    // what keeps that query sound. (The format root itself — `base_rel` empty — has no
    // ancestors and is never excluded.)
    //
    // The safety nets and the build-output heuristic grade neither the root nor its
    // ancestors, in either regime: they prune what a walk discovers, and the caller
    // named this directory — so `tsv format node_modules/pkg` or `dist/sub` walks what
    // `tsv format node_modules` or `dist` walks there. Below the root they classify every
    // child as usual.
    let loose_root = loose_root(&format_root, in_repo);
    if stack.is_ignored(&base_rel, true) {
        if let Some(warning) = tsv_discover::excluded_argument_warning(
            &root.to_string_lossy(),
            &base_rel,
            true,
            loose_root.as_deref(),
            &stack,
        ) {
            out.warnings.push(warning);
        }
        return;
    }

    collect_recursive(
        WalkDir {
            listed: root,
            abs: &root_abs,
        },
        &base_rel,
        true,
        loose_root.as_deref(),
        &mut stack,
        heuristic_active,
        out,
    );
}

/// A stack loaded with the ignore layers of `dirs` — a directory root's ancestors, from
/// `format_root` down to its parent ([`collect_root`]), reading nothing inside one a rule
/// excludes ([`push_dir_layers`]), which puts the root out of scope — and whether the
/// build-output heuristic is still on below them (no `.gitignore` among them was read).
fn preload_ancestors(
    dirs: &[PathBuf],
    format_root: &Path,
    in_repo: bool,
    warnings: &mut Vec<String>,
) -> (IgnoreStack, bool) {
    let mut stack = IgnoreStack::new();
    let mut heuristic_active = true;
    for dir in dirs {
        if push_dir_layers(dir, format_root, in_repo, &mut stack, warnings)
            .is_some_and(|pushed| pushed.gitignore)
        {
            heuristic_active = false;
        }
    }
    (stack, heuristic_active)
}

/// Push the ignore layers of `dir` — a directory no listing is held for — onto `stack`,
/// anchored relative to `format_root`, or `None`, reading nothing, when a rule in the
/// layers already pushed excludes `dir`, itself or through an ancestor: the walk prunes
/// such a directory without listing
/// it, so it never reads — nor warns about — the ignore files inside, which could bring
/// nothing under the directory back in scope anyway (git's parent-directory rule). A
/// named file's quiet skip depends on it: an unreadable or symlinked file, or a shadow,
/// inside the directory that excludes the file would otherwise be the run's only output.
/// The one preload a named path gets, for a directory root's ancestors
/// ([`preload_ancestors`]) and for each directory a file argument's scope moves into
/// ([`FileScope::enter`]); `stack` must already hold what this pushed for every ancestor of
/// `dir`.
///
/// The gate is all this adds to [`push_layers`], which the descent reaches too: the
/// preload differs from a walked directory only in [probing](IgnorePresence::probe) for
/// the files a listing would have named.
fn push_dir_layers(
    dir: &Path,
    format_root: &Path,
    in_repo: bool,
    stack: &mut IgnoreStack,
    warnings: &mut Vec<String>,
) -> Option<PushedLayers> {
    let anchor = rel_to(format_root, dir);
    if stack.is_ignored(&anchor, true) {
        return None;
    }
    Some(push_layers(
        dir,
        &anchor,
        in_repo,
        IgnorePresence::probe(dir, in_repo),
        stack,
        warnings,
    ))
}

/// Which of one directory's ignore files are there — the facts [`push_layers`] decides
/// over, so the two walks learn presence their own way (a [probe](Self::probe) for a
/// directory no listing is held for, the listing itself in [`collect_recursive`]) and
/// read the same files either way.
///
/// `prettierignore` is presence, not a decision to read: `tsv_layer_content` consults it
/// only inside a repo, and outside one it feeds the target root's heads-up
/// ([`prettierignore_outside_repo_warning`]) instead. It is also the one field a caller
/// may leave `false` unasked — see [`Self::probe`].
#[derive(Clone, Copy, Default)]
struct IgnorePresence {
    formatignore: bool,
    prettierignore: bool,
    gitignore: GitignorePresence,
}

impl IgnorePresence {
    /// The presence at `dir` probed a file at a time — what a directory no listing is
    /// held for costs. `.formatignore` and `.prettierignore` take the descent's own rule
    /// ([`is_ignore_file`]), `.gitignore` git's ([`GitignorePresence::at`]).
    ///
    /// Outside a repo `.prettierignore` is left `false` unprobed: nothing reads it there
    /// but the target root's heads-up, which only the descent raises — and outside a repo
    /// the format root is the *filesystem* root, so an ancestor chain is long enough that
    /// a stat per level is worth not paying.
    fn probe(dir: &Path, in_repo: bool) -> Self {
        Self {
            formatignore: is_ignore_file(&dir.join(FORMATIGNORE_FILE)),
            prettierignore: in_repo && is_ignore_file(&dir.join(PRETTIERIGNORE_FILE)),
            gitignore: if in_repo {
                GitignorePresence::at(&dir.join(GITIGNORE_FILE))
            } else {
                GitignorePresence::Absent
            },
        }
    }
}

/// Push one directory's ignore layers onto `stack` at `anchor`, returning which it
/// pushed — the single statement of the ladder both walks run, so a preload and a
/// descent cannot drift on it.
///
/// The tsv layer first ([`tsv_layer_content`], which also warns about a
/// `.prettierignore` its sibling shadows), then the `.gitignore`, which pushes only from
/// a regular file: a symlinked one git does not follow and an unreadable one has no
/// rules to apply, so both warn and push nothing — leaving the build-output heuristic on
/// for the subtree, which the warning is what makes visible. Nothing here is gated on
/// `in_repo` beyond what `tsv_layer_content` gates itself: outside a repo
/// `presence.gitignore` is already [`GitignorePresence::Absent`], by the rule each caller
/// read it with.
///
/// `dir` is the directory's **absolute** path — the one spelling its ignore files are
/// read by and every ignore-file diagnostic names it by, whichever root or argument
/// spelling reached it (see [`tsv_layer_content`]).
fn push_layers(
    dir: &Path,
    anchor: &str,
    in_repo: bool,
    presence: IgnorePresence,
    stack: &mut IgnoreStack,
    warnings: &mut Vec<String>,
) -> PushedLayers {
    let mut pushed = PushedLayers::default();
    if let Some(layer) = tsv_layer_content(
        dir,
        presence.formatignore,
        presence.prettierignore,
        in_repo,
        warnings,
    ) {
        layer.push_onto(stack, anchor);
        pushed.tsv = true;
    }
    match presence.gitignore {
        GitignorePresence::Absent => {}
        GitignorePresence::Symlink => warnings.push(tsv_discover::gitignore_symlink_warning(
            &dir.join(GITIGNORE_FILE).to_string_lossy(),
        )),
        GitignorePresence::File => {
            if let Some(content) = read_ignore_file(&dir.join(GITIGNORE_FILE), warnings) {
                stack.push_gitignore(anchor, &content);
                pushed.gitignore = true;
            }
        }
    }
    pushed
}

/// Which layers one directory pushed onto a stack — what taking it back off pops.
#[derive(Clone, Copy, Default)]
struct PushedLayers {
    tsv: bool,
    gitignore: bool,
}

impl PushedLayers {
    fn pop_from(self, stack: &mut IgnoreStack) {
        if self.tsv {
            stack.pop_tsv();
        }
        if self.gitignore {
            stack.pop_gitignore();
        }
    }
}

/// The ignore scope file arguments are graded in: the layers from a format root down
/// through the directory the latest file argument sat in. Each argument moves it to its
/// own directory — popping back to the two directories' common ancestor and pushing
/// down — so an ignore file above many named files is read and parsed once, where a
/// scope built per directory re-read every ancestor's for each (a hook naming files
/// spread over many directories parsed the repo-root `.gitignore` once per directory) —
/// and none inside a directory a rule excludes is read at all ([`push_dir_layers`]).
#[derive(Default)]
struct FileScope {
    /// Empty until the first file argument moves the scope.
    format_root: PathBuf,
    in_repo: bool,
    stack: IgnoreStack,
    /// The directories on the way to the latest file argument, format root first, with what
    /// each pushed onto `stack` — `None` for one a rule excludes and every one below it,
    /// whose ignore files are never read.
    dirs: Vec<(PathBuf, Option<PushedLayers>)>,
}

impl FileScope {
    /// Move the scope to `dir`, a file argument's canonical directory.
    fn enter(&mut self, dir: &Path, warnings: &mut Vec<String>) {
        if self.dirs.last().is_some_and(|(held, _)| held == dir) {
            return;
        }
        let (format_root, in_repo) = format_root_of(dir);
        if format_root != self.format_root || in_repo != self.in_repo {
            *self = Self {
                format_root,
                in_repo,
                ..Self::default()
            };
        }
        let chain = ancestor_chain(&self.format_root, dir);
        let shared = self
            .dirs
            .iter()
            .zip(&chain)
            .take_while(|((held, _), wanted)| held == *wanted)
            .count();
        for (_, pushed) in self.dirs.drain(shared..).rev() {
            if let Some(pushed) = pushed {
                pushed.pop_from(&mut self.stack);
            }
        }
        for level in chain.into_iter().skip(shared) {
            let pushed = push_dir_layers(
                &level,
                &self.format_root,
                self.in_repo,
                &mut self.stack,
                warnings,
            );
            self.dirs.push((level, pushed));
        }
    }
}

/// One file argument: into the sink unless an ignore rule excludes it — through an
/// ancestor directory or at the file itself, as it would exclude the file from the walk
/// that reached it — in which case it is skipped, with the warning the rule's file calls
/// for ([`tsv_discover::excluded_argument_warning`], which keeps a `.formatignore` or
/// `.prettierignore` rule's skip quiet). Returns whether a rule excluded it. The safety
/// nets and the build-output heuristic never apply: they prune what a walk discovers, and
/// the caller named this file.
///
/// The scope is the file's canonical directory's, resolved as a directory root's is
/// ([`collect_root`]) and moved there from the previous file argument's ([`FileScope`]).
fn collect_file(path: &str, cwd: Option<&Path>, scope: &mut FileScope, out: &mut Walk<'_>) -> bool {
    let file = Path::new(path);
    let Some(file_abs) = absolute_named_path(file, cwd) else {
        out.errors.push(tsv_discover::unresolvable_root_error(path));
        return false;
    };
    // An absolute path to a *file* always has a parent — only the filesystem root has
    // none, and `classify_args` already answered `is_file()`. Falling back to the path
    // itself keeps the arm total the one way that still grades the file: scoping on it
    // resolves the same format root and the same `rel`, where an arm that pushed the
    // file unchecked would do the one thing this module refuses everywhere else — take
    // a named path with no format root and none of its ancestors' rules
    // (`absolute_named_path`'s `None` exists to prevent exactly that). A directory
    // probe on a file is harmless: the ignore-file stats simply fail.
    let dir_abs = file_abs.parent().unwrap_or(&file_abs);
    scope.enter(dir_abs, &mut out.warnings);
    let rel = rel_to(&scope.format_root, &file_abs);
    if !scope.stack.is_ignored(&rel, false) {
        out.files.push(PathBuf::from(path));
        return false;
    }
    if let Some(warning) = tsv_discover::excluded_argument_warning(
        path,
        &rel,
        false,
        loose_root(&scope.format_root, scope.in_repo).as_deref(),
        &scope.stack,
    ) {
        out.warnings.push(warning);
    }
    true
}

/// The format root's display path when it is the filesystem root — outside a git repo,
/// where a warning names a path absolutely ([`tsv_discover::excluded_argument_warning`]) —
/// and `None` inside one.
fn loose_root(format_root: &Path, in_repo: bool) -> Option<Cow<'_, str>> {
    (!in_repo).then(|| format_root.to_string_lossy())
}

/// A named path made absolute, **at the location it was typed**: its parent directory
/// canonicalized (so `..` and a linked ancestor resolve once) and its own name kept, so a
/// path that is itself a symbolic link is graded — by the ignore files, and for the format
/// root above it — where the link sits, not where it points. That is how git reads
/// `check-ignore link.ts` and prettier its ignore files: a rule naming `link.ts` excludes
/// it, and a link into a gitignored `build/` is not under `build/`. (Resolving the link
/// instead graded the target, and spelled a warning's re-include lines for a path the
/// argument never named.) The dedup across overlapping arguments still keys on the
/// canonical path, since two spellings of one file are the same file wherever either is
/// graded. A path with no name of its own (`.`, `..`, a root) canonicalizes whole.
/// Falls back to [`absolutize`] when the parent will not canonicalize; `None` is a
/// relative path with no working directory to join it onto, which the caller refuses
/// ([`tsv_discover::unresolvable_root_error`]): taken as named, it would anchor on no
/// format root and read none of its ancestors' ignore files, silently widening the scope.
fn absolute_named_path(path: &Path, cwd: Option<&Path>) -> Option<PathBuf> {
    let lexical = match (path.parent(), path.file_name()) {
        // `Path::parent` of a bare name is `""`: the working directory, canonical already
        (Some(parent), Some(name)) if parent.as_os_str().is_empty() => {
            cwd.map(|cwd| cwd.join(name))
        }
        (Some(parent), Some(name)) => fs::canonicalize(parent).ok().map(|p| p.join(name)),
        _ => fs::canonicalize(path).ok(),
    };
    lexical.or_else(|| absolutize(path, cwd))
}

/// The format root above `dir` and whether it is a repo root: inside a git repo the repo
/// root is the boundary (reproducible — nothing above it is read); outside one, the
/// filesystem root (so an ancestor `.formatignore` is honored — the filesystem is the API
/// for loose files).
fn format_root_of(dir: &Path) -> (PathBuf, bool) {
    match find_repo_root(dir) {
        Some(repo_root) => (repo_root, true),
        None => (filesystem_root(dir), false),
    }
}

/// The tsv layer one directory contributes, read by the precedence the two walks
/// share: `.formatignore` whenever present (every level, in or out of a repo);
/// inside a repo, a `.prettierignore` is the drop-in fallback at every level
/// (hierarchical, like `.formatignore`), used solely when no *sibling*
/// `.formatignore` is present. Precedence is by PRESENCE, not readability — a
/// present-but-unreadable `.formatignore` still shadows: `read_ignore_file` warns
/// and yields no rules rather than silently falling through to `.prettierignore`.
///
/// A shadowed `.prettierignore` is warned about here too, at whichever directory the
/// shadow sits — the ancestor preload and the descent both reach this, so the
/// warning fires whether the shadowing directory is walked or only preloaded
/// (targeting `pkg` under a repo root holding both files warns like targeting `.`).
/// One statement of the ladder, so the two callers cannot drift on it; they differ
/// only in how presence was learned (a listing, or an [`is_ignore_file`] probe).
///
/// `dir` is the directory's **absolute** path — an ancestor as preloaded, a walked
/// directory as its [`WalkDir::abs`] — never an argument spelling: the one
/// spelling its ignore files are read by and its diagnostics name it by, whichever root
/// or spelling reached it. Diagnostics dedupe by exact string ([`walk_args`]), so a
/// directory named once as an argument and once as a preloaded ancestor
/// (`tsv format . sub`), or under two argument spellings (`tsv format . ./`), would
/// otherwise warn once per spelling.
fn tsv_layer_content(
    dir: &Path,
    has_formatignore: bool,
    has_prettierignore: bool,
    in_repo: bool,
    warnings: &mut Vec<String>,
) -> Option<TsvLayer> {
    if let Some(warning) = prettierignore_shadowed_warning(
        &dir.to_string_lossy(),
        in_repo,
        has_prettierignore,
        has_formatignore,
    ) {
        warnings.push(warning);
    }
    let (file, prettierignore) = if has_formatignore {
        (FORMATIGNORE_FILE, false)
    } else if in_repo && has_prettierignore {
        (PRETTIERIGNORE_FILE, true)
    } else {
        return None;
    };
    let content = read_ignore_file(&dir.join(file), warnings)?;
    Some(TsvLayer {
        content,
        prettierignore,
    })
}

/// One directory's tsv layer: its content, and which of the two files it was read from —
/// the file a warning about one of its rules names.
struct TsvLayer {
    content: String,
    prettierignore: bool,
}

impl TsvLayer {
    /// Push the layer onto `stack` at `anchor`, as the file it was read from.
    fn push_onto(&self, stack: &mut IgnoreStack, anchor: &str) {
        if self.prettierignore {
            stack.push_prettierignore(anchor, &self.content);
        } else {
            stack.push_formatignore(anchor, &self.content);
        }
    }
}

/// One directory the walk descends into, under both of its spellings.
#[derive(Clone, Copy)]
struct WalkDir<'a> {
    /// As the root argument spelled it — the prefix of every path the walk emits and
    /// every traversal error names, so `tsv format src` lists `src/a.ts`.
    listed: &'a Path,
    /// Its absolute path — the one spelling its ignore files are read by and every
    /// ignore-file diagnostic names it by, whichever root or argument spelling reached
    /// it (see [`tsv_layer_content`]).
    abs: &'a Path,
}

fn collect_recursive(
    dir: WalkDir<'_>,
    // `dir` relative to the format root, `/`-joined (`""` = the format root).
    // Child paths extend it without re-deriving from disk.
    dir_rel: &str,
    // true only for the directory tsv was pointed at (the root of this walk),
    // false for every descendant — gates the target-root-only
    // `.prettierignore`-outside-a-repo warning.
    is_target_root: bool,
    // the format root's display path outside a git repo, `None` inside one — what a
    // warning names a path by, and so also whether the format root is a git repo
    // (`.gitignore` is read only then)
    loose_root: Option<&str>,
    stack: &mut IgnoreStack,
    // whether the build-output heuristic is active at *this* dir's level (no
    // `.gitignore` governs `dir` or above). `dir`'s own `.gitignore`, read below,
    // turns it off for `dir`'s children.
    heuristic_active: bool,
    out: &mut Walk<'_>,
) {
    let in_repo = loose_root.is_none();
    // Materialize the listing once: it's used twice — to read THIS dir's own
    // ignore files (opening one only when the listing actually contains it, so an
    // ignore-file-free dir costs zero speculative `open`s) before classifying its
    // children, then to walk the entries. Only the name and the file type are
    // kept, not the `DirEntry`: an entry holds the directory handle open, so a
    // retained listing would pin one `DIR*` per level for the whole recursion
    // below it, and its name is an owned `OsString` already. Paths are rebuilt
    // with `dir.listed.join(&name)`, which is exactly what `DirEntry::path` does.
    let mut entries: Vec<(OsString, fs::FileType)> = Vec::new();
    match fs::read_dir(dir.listed) {
        Ok(read_dir) => {
            for entry in read_dir {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        out.errors
                            .push(format!("{}: read_dir entry failed: {e}", dir.abs.display()));
                        continue;
                    }
                };
                match entry.file_type() {
                    Ok(file_type) => entries.push((entry.file_name(), file_type)),
                    Err(e) => out.errors.push(format!(
                        "{}: file_type failed: {e}",
                        dir.abs.join(entry.file_name()).display()
                    )),
                }
            }
        }
        // named by the absolute path, as every ignore-file warning names its directory,
        // so `tsv format t ./t` reports an unreadable `t/locked` once, not once per spelling
        Err(e) => {
            out.errors
                .push(format!("{}: read_dir failed: {e}", dir.abs.display()));
            return;
        }
    }

    // Single pass over the listing for the ignore-file presence flags this dir
    // needs, rather than a linear scan per name. `read_dir` order is arbitrary
    // (not sorted), so there's nothing to short-circuit on; one pass bounds the
    // cost on a large directory regardless of how many names we check.
    let mut presence = IgnorePresence::default();
    for (name, file_type) in &entries {
        let n = name.as_os_str();
        let present = if n == OsStr::new(FORMATIGNORE_FILE) {
            &mut presence.formatignore
        } else if n == OsStr::new(PRETTIERIGNORE_FILE) {
            &mut presence.prettierignore
        } else {
            if in_repo && n == OsStr::new(GITIGNORE_FILE) {
                // git's presence rule, which the listing's unfollowed file type answers
                presence.gitignore = GitignorePresence::of_listing(*file_type);
            }
            continue;
        };
        // the preload's presence rule (`is_ignore_file`): a listing's file type does
        // not follow a symlink, so only a link costs the `stat` that asks what it
        // points at
        *present =
            file_type.is_file() || (file_type.is_symlink() && is_ignore_file(&dir.abs.join(n)));
    }
    // the ladder itself is `push_layers`, shared with the ancestor preload
    // (`push_dir_layers`) — this path differs only in having read presence off the
    // listing instead of probing for it
    let pushed = push_layers(
        dir.abs,
        dir_rel,
        in_repo,
        presence,
        stack,
        &mut out.warnings,
    );
    // outside a git repo a target-root `.prettierignore` is silently skipped (tsv
    // reads `.formatignore` there) — warn, from the listing already in hand (no
    // extra stat), pointing at the rename / `git init` fixes. Bounded to the
    // target root: outside a repo tsv's regime is `.formatignore`-only at every
    // depth (the hierarchical `.prettierignore` read is repo-only), so this is a
    // courtesy heads-up at the entry point, not a per-directory scan — and a subdir
    // target has no `.git` boundary to anchor an upward walk on. The preload raises
    // it for no ancestor, which is why `IgnorePresence::probe` can leave
    // `prettierignore` unasked outside a repo where this reads it.
    if is_target_root
        && let Some(warning) = prettierignore_outside_repo_warning(
            &dir.abs.to_string_lossy(),
            in_repo,
            presence.prettierignore,
            presence.formatignore,
        )
    {
        out.warnings.push(warning);
    }
    // this dir's own `.gitignore`, if `push_layers` pushed one, turns the heuristic off
    // for its children — an unreadable or symlinked one pushes nothing, so the heuristic
    // stays on for the subtree and the warning it raised is what makes that visible
    let child_heuristic = heuristic_active && !pushed.gitignore;

    for (name_os, file_type) in &entries {
        // the matcher and the extension test read the name as text, lossily for the
        // rare non-UTF-8 one — the paths the walk emits and descends into are joined
        // from the raw `OsStr` below, since a U+FFFD spelling names nothing on disk
        let name = name_os.to_string_lossy();
        // Only a directory (to classify) or a formattable file (to take a verdict)
        // needs the relative path — and on an app repo most entries are neither (the
        // lockfiles, the `.md`, the images, and any symlink, which the walk never
        // follows), so the extension is read ahead of the `String` the verdict takes.
        let wanted =
            file_type.is_dir() || (file_type.is_file() && tsv_discover::is_formattable(&name));
        if !wanted {
            continue;
        }
        let child_rel = if dir_rel.is_empty() {
            name.to_string()
        } else {
            format!("{dir_rel}/{name}")
        };

        if file_type.is_dir() {
            // the per-directory prune/descend decision — safety nets, the
            // build-output heuristic (+ its shadow warning), and the matcher —
            // lives in `tsv_discover` so the native CLI, the WASM CLI, and the VS
            // Code extension share one verdict. The FS walk + layer push/pop stay
            // here.
            match classify_dir(&name, &child_rel, child_heuristic, stack) {
                DirVerdict::Prune => continue,
                DirVerdict::PruneWithWarning => {
                    out.warnings
                        .extend(tsv_discover::shadow_warning(&child_rel, loose_root, stack));
                    continue;
                }
                DirVerdict::Descend => {}
            }
            // the child reads its own ignore files (and pushes/pops its own layers)
            // when we recurse into it. The paths are built only here, so a pruned dir
            // or a non-formatted file never pays for the allocations.
            let listed = dir.listed.join(name_os);
            let abs = dir.abs.join(name_os);
            collect_recursive(
                WalkDir {
                    listed: &listed,
                    abs: &abs,
                },
                &child_rel,
                false,
                loose_root,
                stack,
                child_heuristic,
                out,
            );
        } else if should_format_file(&name, &child_rel, stack) {
            out.files.push(dir.listed.join(name_os));
        }
    }

    // a directory boundary is the natural release point for a buffering sink:
    // whatever this directory found is complete, and a streaming consumer
    // shouldn't have to wait for the next one. A no-op for a collecting sink,
    // and for a buffer that is already empty.
    out.files.flush();

    pushed.pop_from(stack);
}

/// Whether `path` names an ignore file: a regular file, reached through a symlink
/// the way reading it is. A directory of that name holds no rules — reading it would
/// fail with `EISDIR` and warn about rules that were never there — and a dangling
/// link is absent. The one presence rule both walks apply: the ancestor preload
/// probes with it, and the descent asks it of a listing entry that is a symlink.
fn is_ignore_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// How an in-tree `.gitignore` is present — by git's rule rather than
/// [`is_ignore_file`]'s. git does not follow a symbolic link to one: it warns that it
/// cannot access the file and applies none of its rules. So a link is its own answer,
/// which the walk reports ([`tsv_discover::gitignore_symlink_warning`]) and otherwise
/// treats as a file whose rules could not be read; anything but a regular file or a link
/// is absent, as for the tsv layer.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum GitignorePresence {
    /// The default so [`IgnorePresence`] can start from one: a directory outside a repo
    /// never asks, and a listing that never names a `.gitignore` never sets it.
    #[default]
    Absent,
    File,
    Symlink,
}

impl GitignorePresence {
    /// The presence a directory listing's own, unfollowed, file type states.
    fn of_listing(file_type: fs::FileType) -> Self {
        if file_type.is_symlink() {
            GitignorePresence::Symlink
        } else if file_type.is_file() {
            GitignorePresence::File
        } else {
            GitignorePresence::Absent
        }
    }

    /// The presence at `path`, probed without following a link — the ancestor preload's
    /// question, asked with no listing to read it from.
    fn at(path: &Path) -> Self {
        fs::symlink_metadata(path).map_or(GitignorePresence::Absent, |metadata| {
            GitignorePresence::of_listing(metadata.file_type())
        })
    }
}

/// The nearest ancestor of `start` (inclusive) that holds a `.git` entry (dir
/// *or* file — worktrees and submodules use a `.git` file) — the repo root — or
/// `None` if there is no git tree above `start`.
fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

/// The filesystem root above `start` (the topmost ancestor — `/` on Unix, a
/// prefix root on Windows). The format-root fallback outside a git repo, so the
/// `.formatignore` walk spans the whole path and the cwd never enters.
fn filesystem_root(start: &Path) -> PathBuf {
    start
        .ancestors()
        .last()
        .map_or_else(|| start.to_path_buf(), Path::to_path_buf)
}

/// The chain of directories from `format_root` (inclusive) down to `leaf`
/// (inclusive), shallowest first. `format_root` must be an ancestor-or-equal of
/// `leaf`.
fn ancestor_chain(format_root: &Path, leaf: &Path) -> Vec<PathBuf> {
    let mut chain = Vec::new();
    let mut cur = leaf;
    loop {
        chain.push(cur.to_path_buf());
        if cur == format_root {
            break;
        }
        match cur.parent() {
            Some(parent) => cur = parent,
            None => break,
        }
    }
    chain.reverse();
    chain
}

/// Best-effort absolute form of `path` when `fs::canonicalize` fails (e.g. a
/// permission error mid-walk) — joined onto `cwd` if relative, otherwise as-is,
/// with `.` and `..` resolved lexically. Unlike `canonicalize` it does not resolve
/// symlinks; it only feeds the format-root walk, which still lands on an
/// ancestor-or-self of `path`. The `..` resolution is not cosmetic: `rel_to` keeps
/// `Normal` components alone, so an unresolved `..` would be *deleted* from the
/// format-root-relative path rather than applied, anchoring the walk one directory
/// off.
///
/// `None` for a relative `path` with no `cwd` (the working directory could not be
/// resolved — deleted, say): there is nothing absolute to join it onto, and resolving
/// its `..` against an empty base would pop past the start and drop them.
fn absolutize(path: &Path, cwd: Option<&Path>) -> Option<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    Some(normalized)
}

/// `path` relative to `format_root` as a `/`-joined string (the anchor form the
/// `IgnoreStack` layers want). `format_root` is always an ancestor-or-equal of
/// `path`, so the `strip_prefix` never fails; `""` means `path` *is* the root.
fn rel_to(format_root: &Path, path: &Path) -> String {
    // `""` on a failed strip would read as the root itself and walk the path with none
    // of its ancestors' rules, so the invariant is asserted rather than absorbed
    debug_assert!(
        path.starts_with(format_root),
        "{} is not under its format root {}",
        path.display(),
        format_root.display()
    );
    path.strip_prefix(format_root)
        .map(path_to_rel)
        .unwrap_or_default()
}

/// A byte key reproducing `Path`'s component-wise ordering as a plain byte
/// string, so one pass of `sort_by_cached_key` replaces the O(n log n) re-parsing
/// of `PathBuf` components that dominates discovery once the matcher is optimized.
/// Each component is prefixed with a `\0` sentinel — which sorts before every real
/// filename byte — so a shorter path at a component boundary sorts first
/// (`a/y.ts` before `a-b/x.ts`), exactly like `Path::cmp` and the WASM CLI's
/// `compare_paths`. A filename contains neither the path separator nor `\0`, so
/// the sentinel is unambiguous.
pub(crate) fn path_sort_key(path: &Path) -> Vec<u8> {
    let mut key = Vec::new();
    for component in path.components() {
        key.push(0);
        key.extend_from_slice(component.as_os_str().as_encoded_bytes());
    }
    key
}

/// The path a [`path_sort_key`] was taken from, rebuilt from its components — for a
/// report slot whose `PathBuf` went with the worker that died holding it, so the report
/// can still name the file. Exact on unix, where a component is its own bytes; a lossy
/// spelling elsewhere, which a diagnostic can bear.
pub(crate) fn path_from_sort_key(key: &[u8]) -> PathBuf {
    let mut path = PathBuf::new();
    for component in key.split(|&b| b == 0).skip(1) {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            path.push(OsStr::from_bytes(component));
        }
        #[cfg(not(unix))]
        path.push(String::from_utf8_lossy(component).as_ref());
    }
    path
}

/// A relative path's `Normal` components joined with `/` (the form ignore rules
/// match against). Empty for the format root itself.
fn path_to_rel(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Read an ignore file: its content, or `None` — silently for a `NotFound` error
/// (genuinely missing, or raced away after the listing — not the user's problem), with
/// a non-fatal warning pushed to `warnings` for any other error (invalid UTF-8 —
/// `read_to_string` is strict — permissions, …: the file is there but its rules won't
/// apply, the exact silent footgun this surfaces). Precedence between a directory's tsv
/// files is decided by *presence* before this read, so the two `None`s need no telling
/// apart. Mirrors the JS `read_ignore_file`.
fn read_ignore_file(path: &Path, warnings: &mut Vec<String>) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(content) => Some(content),
        // not there — nothing to apply, silently (genuinely absent, or deleted between
        // the directory listing and this read)
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        // present but unreadable: warned, and no rules. Which file the directory's tsv
        // layer is read from was decided by PRESENCE before this read
        // (`tsv_layer_content`), so an unreadable `.formatignore` still shadows its
        // sibling `.prettierignore` rather than falling through to it
        Err(e) => {
            warnings.push(format!(
                "could not read {} ({e}); its ignore rules are not applied",
                path.display()
            ));
            None
        }
    }
}
