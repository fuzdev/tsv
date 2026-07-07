use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Component, Path, PathBuf};
use tsv_discover::{
    DirVerdict, classify_dir, prettierignore_outside_repo_warning, prettierignore_shadowed_warning,
    should_format_file,
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

/// Result of expanding path arguments: the files to format plus any non-fatal
/// traversal errors (unreadable directories/entries) and discovery warnings.
/// All three are sorted.
pub struct Discovered {
    pub files: Vec<PathBuf>,
    /// `path: detail` messages — reported by the caller, counted as errors.
    pub errors: Vec<String>,
    /// Non-fatal diagnostics — reported to stderr by the caller but **not**
    /// counted as errors (no effect on the exit code or stdout): the
    /// heuristic-shadow warning ([`tsv_discover::heuristic_shadow_warning`]), the
    /// `.prettierignore`-shadowed-by-a-sibling-`.formatignore` warning
    /// ([`prettierignore_shadowed_warning`]), the `.prettierignore`-outside-a-repo
    /// warning ([`prettierignore_outside_repo_warning`]), and the
    /// unreadable-ignore-file warning (a present
    /// `.gitignore`/`.formatignore`/`.prettierignore` whose read failed; see
    /// [`read_ignore_file`]).
    pub warnings: Vec<String>,
}

/// Expand files and directories into a sorted, deduplicated list of files to format.
///
/// All root arguments are validated upfront: any that don't resolve to a file or
/// directory fail the whole run before anything is formatted (`Err` carries one
/// message per bad argument). Traversal errors below a valid root are non-fatal —
/// collected in `Discovered::errors` while the rest of the tree continues.
///
/// Explicit file arguments are always included regardless of extension *and*
/// regardless of the ignore files (the caller named them, so the ignore files —
/// which govern *discovery* — don't apply). Directories recurse with the
/// extension filter. Symlinks inside directories are not followed (cycle safety)
/// — pass them explicitly to format their targets.
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
/// (`tsv_discover::SAFETY_NET_DIRS`) are always pruned.
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
/// subdirectory, and formatting a subdirectory directly gives the same result as
/// formatting it via an ancestor.
///
/// With multiple roots, overlapping spellings of the same file (`src` vs
/// `./src`, absolute vs relative, symlink aliases) are deduplicated by canonical
/// path, keeping the first spelling in sorted order. A single root can't produce
/// duplicates (symlinks aren't followed), so the canonicalization cost is skipped.
pub fn discover_files(paths: &[String]) -> Result<Discovered, Vec<String>> {
    let bad: Vec<String> = paths
        .iter()
        .filter(|p| {
            let path = Path::new(p.as_str());
            !path.is_file() && !path.is_dir()
        })
        .map(|p| format!("{p}: not a file or directory"))
        .collect();
    if !bad.is_empty() {
        return Err(bad);
    }

    // canonical cwd so it compares cleanly with canonicalized roots below
    let cwd = std::env::current_dir()
        .and_then(fs::canonicalize)
        .unwrap_or_else(|_| PathBuf::from("."));

    // accumulate directly into the result struct so the walk threads one `&mut`
    // sink rather than a parallel set of vectors
    let mut out = Discovered {
        files: Vec::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    for path_str in paths {
        let path = PathBuf::from(path_str);
        if path.is_file() {
            // explicit file argument — bypasses the ignore files
            out.files.push(path);
        } else {
            collect_root(&path, &cwd, &mut out);
        }
    }
    out.files.sort_by_cached_key(|p| path_sort_key(p));
    out.files.dedup();
    if paths.len() > 1 {
        let mut seen: HashSet<PathBuf> = HashSet::with_capacity(out.files.len());
        out.files
            .retain(|p| seen.insert(fs::canonicalize(p).unwrap_or_else(|_| p.clone())));
    }
    // dedupe both channels: overlapping roots re-walk the shared subtree, so the
    // same unreadable path / pruned directory would otherwise report twice. The
    // strings are byte-identical only for the same underlying failure, so this
    // collapses duplicates without hiding a distinct one.
    out.errors.sort();
    out.errors.dedup();
    out.warnings.sort();
    out.warnings.dedup();
    Ok(out)
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
fn collect_root(root: &Path, cwd: &Path, out: &mut Discovered) {
    let root_abs = fs::canonicalize(root).unwrap_or_else(|_| absolutize(root, cwd));
    // inside a git repo, the repo root is the boundary (reproducible: nothing
    // above it is read); outside one, the filesystem root (so an ancestor
    // `.formatignore` is honored — the filesystem is the API for loose files).
    let repo_root = find_repo_root(&root_abs);
    let in_repo = repo_root.is_some();
    let format_root = repo_root.unwrap_or_else(|| filesystem_root(&root_abs));

    let mut stack = IgnoreStack::new();
    let mut heuristic_active = true;
    // `root` relative to the format root (an ancestor-or-self of `root_abs`, so
    // this never fails; `""` means `root` *is* the format root).
    let base_rel = rel_to(&format_root, &root_abs);

    // Preload the ancestors *above* `root` (format root → `root`'s parent). `root`
    // and every directory below it read their own ignore files in
    // `collect_recursive`, from the listing they already fetch — so an
    // ignore-file-free subtree (the common case) costs zero speculative `open`s.
    // Ancestors above `root` aren't listed (we don't walk them), so they keep the
    // direct open. `root` is excluded here to avoid reading its ignores twice.
    let chain = ancestor_chain(&format_root, &root_abs);
    for ancestor in &chain[..chain.len() - 1] {
        let anchor = rel_to(&format_root, ancestor);
        // tsv layer: `.formatignore` everywhere; inside a repo, a `.prettierignore`
        // it shadows per-directory (hierarchical, like `.formatignore` — deeper
        // wins). No listing for ancestors, so the read outcome stands in for
        // presence: a present-but-unreadable `.formatignore` (`Unreadable`, warned)
        // shadows `.prettierignore` just as in the listed case — only a genuinely
        // `Absent` one falls through.
        let tsv_content = if in_repo {
            match read_ignore_file(&ancestor.join(FORMATIGNORE_FILE), &mut out.warnings) {
                IgnoreRead::Content(content) => Some(content),
                IgnoreRead::Unreadable => None,
                IgnoreRead::Absent => {
                    read_ignore_file(&ancestor.join(PRETTIERIGNORE_FILE), &mut out.warnings)
                        .content()
                }
            }
        } else {
            read_ignore_file(&ancestor.join(FORMATIGNORE_FILE), &mut out.warnings).content()
        };
        if let Some(content) = tsv_content {
            stack.push_tsv(&anchor, &content);
        }
        // `.gitignore` layer: only inside a git repo
        if in_repo
            && let IgnoreRead::Content(content) =
                read_ignore_file(&ancestor.join(GITIGNORE_FILE), &mut out.warnings)
        {
            stack.push_gitignore(&anchor, &content);
            heuristic_active = false;
        }
    }

    // The recursion uses the leaf-only matcher query (`tsv_discover` calls
    // `is_ignored_leaf`), which is exact only when an entry's ancestors are
    // already cleared. That holds for everything the walk *descends* into, but
    // NOT for `root` itself when it sits under an ignored ancestor (e.g.
    // `tsv format build/sub` with a gitignored `build/`). Gate it once here with
    // the full, ancestor-walking `is_ignored`: an ignored root means nothing
    // under it is in scope. (The format root itself — `base_rel` empty — has no
    // ancestors to clear and is never ignored.)
    if !base_rel.is_empty() && stack.is_ignored(&base_rel, true) {
        return;
    }

    collect_recursive(
        root,
        &base_rel,
        true,
        in_repo,
        &mut stack,
        heuristic_active,
        out,
    );
}

fn collect_recursive(
    dir: &Path,
    // `dir` relative to the format root, `/`-joined (`""` = the format root).
    // Child paths extend it without re-deriving from disk.
    dir_rel: &str,
    // true only for the directory tsv was pointed at (the root of this walk),
    // false for every descendant — gates the target-root-only
    // `.prettierignore`-outside-a-repo warning.
    is_target_root: bool,
    // whether the format root is a git repo — `.gitignore` is read only then.
    in_repo: bool,
    stack: &mut IgnoreStack,
    // whether the build-output heuristic is active at *this* dir's level (no
    // `.gitignore` governs `dir` or above). `dir`'s own `.gitignore`, read below,
    // turns it off for `dir`'s children.
    heuristic_active: bool,
    out: &mut Discovered,
) {
    // Materialize the listing once: it's used twice — to read THIS dir's own
    // ignore files (opening one only when the listing actually contains it, so an
    // ignore-file-free dir costs zero speculative `open`s) before classifying its
    // children, then to walk the entries. Each entry's name is taken once here and
    // reused below.
    let mut entries: Vec<(fs::DirEntry, OsString)> = Vec::new();
    match fs::read_dir(dir) {
        Ok(read_dir) => {
            for entry in read_dir {
                match entry {
                    Ok(entry) => {
                        let name = entry.file_name();
                        entries.push((entry, name));
                    }
                    Err(e) => out
                        .errors
                        .push(format!("{}: read_dir entry failed: {e}", dir.display())),
                }
            }
        }
        Err(e) => {
            out.errors
                .push(format!("{}: read_dir failed: {e}", dir.display()));
            return;
        }
    }

    // Single pass over the listing for the ignore-file presence flags this dir
    // needs, rather than a linear scan per name. `read_dir` order is arbitrary
    // (not sorted), so there's nothing to short-circuit on; one pass bounds the
    // cost on a large directory regardless of how many names we check. An ignore
    // file's *content* is still opened only when present (below), so an
    // ignore-file-free dir costs zero speculative opens.
    let (mut has_formatignore, mut has_prettierignore, mut has_gitignore) = (false, false, false);
    for (_, name) in &entries {
        let n = name.as_os_str();
        if n == OsStr::new(FORMATIGNORE_FILE) {
            has_formatignore = true;
        } else if n == OsStr::new(PRETTIERIGNORE_FILE) {
            has_prettierignore = true;
        } else if in_repo && n == OsStr::new(GITIGNORE_FILE) {
            has_gitignore = true;
        }
    }
    // tsv layer: `.formatignore` whenever present (every level, in or out of a
    // repo); inside a repo, a `.prettierignore` is the drop-in fallback at every
    // level (hierarchical, like `.formatignore`), used solely when no *sibling*
    // `.formatignore` is present. Precedence is by presence (the listing), not
    // readability — a present-but-unreadable `.formatignore` still shadows:
    // `read_ignore_file` warns and yields no rules rather than silently falling
    // through to `.prettierignore`.
    let tsv_content = if has_formatignore {
        read_ignore_file(&dir.join(FORMATIGNORE_FILE), &mut out.warnings).content()
    } else if in_repo && has_prettierignore {
        read_ignore_file(&dir.join(PRETTIERIGNORE_FILE), &mut out.warnings).content()
    } else {
        None
    };
    let tsv_pushed = if let Some(content) = tsv_content {
        stack.push_tsv(dir_rel, &content);
        true
    } else {
        false
    };
    // inside a repo, a sibling `.formatignore` shadows this dir's `.prettierignore`
    // (one tsv layer per directory) — its rules go unread here. Warn from the
    // listing already in hand (presence-only, no extra read), pointing at merging
    // the patterns into `.formatignore`. Unlike the outside-repo warning below,
    // this fires at every directory (a shadow is per-directory, not target-root).
    if let Some(warning) = prettierignore_shadowed_warning(
        &dir.to_string_lossy(),
        in_repo,
        has_prettierignore,
        has_formatignore,
    ) {
        out.warnings.push(warning);
    }
    // outside a git repo a target-root `.prettierignore` is silently skipped (tsv
    // reads `.formatignore` there) — warn, from the listing already in hand (no
    // extra stat), pointing at the rename / `git init` fixes. Bounded to the
    // target root: outside a repo tsv's regime is `.formatignore`-only at every
    // depth (the hierarchical `.prettierignore` read is repo-only), so this is a
    // courtesy heads-up at the entry point, not a per-directory scan — and a subdir
    // target has no `.git` boundary to anchor an upward walk on.
    if is_target_root
        && let Some(warning) = prettierignore_outside_repo_warning(
            &dir.to_string_lossy(),
            in_repo,
            has_prettierignore,
            has_formatignore,
        )
    {
        out.warnings.push(warning);
    }
    // `.gitignore` layer: only inside a repo (`has_gitignore` implies `in_repo`);
    // it turns the heuristic off for this dir's children. A present-but-unreadable
    // `.gitignore` warns (inside `read_ignore_file`) and is *not* pushed — so the
    // heuristic stays on for this subtree, which the warning makes visible.
    let git_pushed = if has_gitignore
        && let IgnoreRead::Content(content) =
            read_ignore_file(&dir.join(GITIGNORE_FILE), &mut out.warnings)
    {
        stack.push_gitignore(dir_rel, &content);
        true
    } else {
        false
    };
    let child_heuristic = heuristic_active && !git_pushed;

    for (entry, name) in &entries {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(e) => {
                out.errors
                    .push(format!("{}: file_type failed: {e}", entry.path().display()));
                continue;
            }
        };
        let name = name.to_string_lossy();
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
                DirVerdict::PruneWithWarning(warning) => {
                    out.warnings.push(warning);
                    continue;
                }
                DirVerdict::Descend => {}
            }
            // the child reads its own ignore files (and pushes/pops its own layers)
            // when we recurse into it. `entry.path()` is built only here, so a
            // pruned dir or a non-formatted file never pays for the allocation.
            collect_recursive(
                &entry.path(),
                &child_rel,
                false,
                in_repo,
                stack,
                child_heuristic,
                out,
            );
        } else if file_type.is_file() && should_format_file(&name, &child_rel, stack) {
            out.files.push(entry.path());
        }
    }

    if git_pushed {
        stack.pop_gitignore();
    }
    if tsv_pushed {
        stack.pop_tsv();
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
/// permission error mid-walk) — joined onto `cwd` if relative, otherwise as-is.
/// Unlike `canonicalize` it does not resolve symlinks or `..`; it only feeds the
/// format-root walk, which still lands on an ancestor-or-self of `path`.
fn absolutize(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// `path` relative to `format_root` as a `/`-joined string (the anchor form the
/// `IgnoreStack` layers want). `format_root` is always an ancestor-or-equal of
/// `path`, so the `strip_prefix` never fails; `""` means `path` *is* the root.
fn rel_to(format_root: &Path, path: &Path) -> String {
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
fn path_sort_key(path: &Path) -> Vec<u8> {
    let mut key = Vec::new();
    for component in path.components() {
        key.push(0);
        key.extend_from_slice(component.as_os_str().as_encoded_bytes());
    }
    key
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

/// The outcome of reading an ignore file the walk is about to consult.
enum IgnoreRead {
    /// Read succeeded.
    Content(String),
    /// Not there — nothing to apply, silently (genuinely absent, or deleted
    /// between the directory listing and this read).
    Absent,
    /// Present but unreadable (invalid UTF-8, permissions, …). A non-fatal
    /// warning was pushed and the rules are dropped. Distinct from `Absent` so a
    /// precedence decision (`.formatignore` shadowing `.prettierignore`) doesn't
    /// fall through to a lower-precedence file on a mere read error.
    Unreadable,
}

impl IgnoreRead {
    /// The content if the read succeeded, else `None` (any warning was already
    /// pushed). For callers that don't distinguish `Absent` from `Unreadable`.
    fn content(self) -> Option<String> {
        match self {
            IgnoreRead::Content(c) => Some(c),
            IgnoreRead::Absent | IgnoreRead::Unreadable => None,
        }
    }
}

/// Read an ignore file, classifying the outcome so the walk can both surface a
/// silently-dropped file and keep precedence by *presence*. A `NotFound` error is
/// `Absent` (genuinely missing, or raced away after the listing — not the user's
/// problem, no warning). Any other error (invalid UTF-8 — `read_to_string` is
/// strict — permissions, …) pushes a non-fatal warning to `warnings` (the file is
/// there but its rules won't apply, the exact silent footgun this surfaces) and
/// yields `Unreadable`. Mirrors the JS `read_ignore_file`.
fn read_ignore_file(path: &Path, warnings: &mut Vec<String>) -> IgnoreRead {
    match fs::read_to_string(path) {
        Ok(content) => IgnoreRead::Content(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => IgnoreRead::Absent,
        Err(e) => {
            warnings.push(format!(
                "could not read {} ({e}); its ignore rules are not applied",
                path.display()
            ));
            IgnoreRead::Unreadable
        }
    }
}
