use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use tsv_discover::{DirVerdict, classify_dir, should_format_file};
use tsv_ignore::IgnoreStack;

/// tsv's native ignore file, discovered hierarchically (one per directory, in a
/// repo from the repo root down, outside a repo from the filesystem root down).
const FORMATIGNORE_FILE: &str = ".formatignore";

/// The repo-root-only tsv files, in precedence order: a repo-root `.formatignore`
/// (tsv's native name), used alone when present so tsv can be scoped independently
/// of prettier; otherwise a repo-root `.prettierignore` for drop-in compatibility.
/// Only consulted at the repo root — `.prettierignore` is never hierarchical, and
/// neither is read outside a git repo (use `.formatignore`).
const TSV_ROOT_FILES: [&str; 2] = [".formatignore", ".prettierignore"];

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
    /// counted as errors (no effect on the exit code or stdout). Currently the
    /// heuristic-shadow warning (see [`tsv_discover::heuristic_shadow_warning`]).
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
///   case-sensitive filesystem); `.formatignore` hierarchically from the repo
///   root down; and a repo-root `.prettierignore` (shadowed by a repo-root
///   `.formatignore`) as drop-in compat. The tsv files override `.gitignore`.
/// - **outside a git repo**, the format root is the **filesystem root**:
///   `.gitignore` is not consulted at all (as git itself does), and
///   `.formatignore` is honored hierarchically from the filesystem root down — so
///   a `~/.formatignore` is global config for loose files. `.prettierignore` is
///   repo-only and not read here.
///
/// `.formatignore` is hierarchical (a file in any directory governs its subtree,
/// deeper wins); `.prettierignore` is single and repo-root-only. The
/// **safety nets** (`tsv_discover::SAFETY_NET_DIRS`) are always pruned.
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
/// filesystem root — and preloads the [`IgnoreStack`] for the ancestor chain
/// from there down to `root`: `.formatignore` at each level (with a repo-root
/// `.prettierignore` shadow), and `.gitignore` at each level when in a repo. The
/// heuristic is seeded off once any `.gitignore` is in scope. `root`'s display
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

    // preload the ancestor chain, format root → `root` (shallow→deep)
    for ancestor in ancestor_chain(&format_root, &root_abs) {
        let anchor = rel_to(&format_root, &ancestor);
        // tsv layer: `.formatignore` everywhere; at the repo root only, a
        // `.prettierignore` it shadows (drop-in compat, never hierarchical)
        let tsv_content = if in_repo && anchor.is_empty() {
            read_first(&ancestor, &TSV_ROOT_FILES)
        } else {
            read_optional(&ancestor.join(FORMATIGNORE_FILE))
        };
        if let Some(content) = tsv_content {
            stack.push_tsv(&anchor, &content);
        }
        // `.gitignore` layer: only inside a git repo
        if in_repo && let Some(content) = read_optional(&ancestor.join(GITIGNORE_FILE)) {
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

    collect_recursive(root, &base_rel, in_repo, &mut stack, heuristic_active, out);
}

fn collect_recursive(
    dir: &Path,
    // `dir` relative to the format root, `/`-joined (`""` = the format root).
    // Child paths extend it without re-deriving from disk.
    dir_rel: &str,
    // whether the format root is a git repo — `.gitignore` is read only then.
    in_repo: bool,
    stack: &mut IgnoreStack,
    heuristic_active: bool,
    out: &mut Discovered,
) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            out.errors
                .push(format!("{}: read_dir failed: {e}", dir.display()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                out.errors
                    .push(format!("{}: read_dir entry failed: {e}", dir.display()));
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(e) => {
                out.errors
                    .push(format!("{}: file_type failed: {e}", entry.path().display()));
                continue;
            }
        };
        let path = entry.path();
        let name = entry.file_name();
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
            match classify_dir(&name, &child_rel, heuristic_active, stack) {
                DirVerdict::Prune => continue,
                DirVerdict::PruneWithWarning(warning) => {
                    out.warnings.push(warning);
                    continue;
                }
                DirVerdict::Descend => {}
            }
            // entering the dir: push its `.formatignore` (hierarchical) and, in a
            // repo, its `.gitignore` (which turns the heuristic off for its
            // subtree); pop both on the way back out
            let tsv_pushed = match read_optional(&path.join(FORMATIGNORE_FILE)) {
                Some(content) => {
                    stack.push_tsv(&child_rel, &content);
                    true
                }
                None => false,
            };
            let git_pushed = in_repo
                && match read_optional(&path.join(GITIGNORE_FILE)) {
                    Some(content) => {
                        stack.push_gitignore(&child_rel, &content);
                        true
                    }
                    None => false,
                };
            collect_recursive(
                &path,
                &child_rel,
                in_repo,
                stack,
                heuristic_active && !git_pushed,
                out,
            );
            if git_pushed {
                stack.pop_gitignore();
            }
            if tsv_pushed {
                stack.pop_tsv();
            }
        } else if file_type.is_file() && should_format_file(&name, &child_rel, stack) {
            out.files.push(path);
        }
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

/// Reads `dir/<name>` for the first readable `name`, or `None` if none exist —
/// the `.formatignore`-shadows-`.prettierignore` resolution.
fn read_first(dir: &Path, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| read_optional(&dir.join(name)))
}

/// Reads a file to a string, mapping any IO error (absent, unreadable) to `None`.
fn read_optional(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}
