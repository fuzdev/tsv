//! gitignore-style path matching for tsv's file discovery.
//!
//! tsv is non-configurable for *style*, but file *scope* is the one sanctioned
//! carve-out: gitignore-shaped files say which files `tsv format` reformats.
//! This crate is the shared, pure-Rust matcher behind that feature — used
//! natively by `tsv_cli` and exposed to the JS CLI and VS Code extension through
//! `tsv_wasm`. It implements the
//! [gitignore(5) pattern format](https://git-scm.com/docs/gitignore#_pattern_format),
//! the same grammar prettier matches via its `ignore` dependency and git matches
//! for `.gitignore`.
//!
//! Two layers of API:
//!
//! - [`IgnoreRules`] — one ignore file's rules, matched against paths relative
//!   to that file's root. The single-file primitive.
//! - [`IgnoreStack`] — a hierarchical, git-faithful evaluator: a stack of
//!   per-directory `.gitignore` layers plus a parallel stack of per-directory tsv
//!   layers (`.formatignore`), evaluated against format-root-relative paths with
//!   git's last-match-wins + parent-directory-prune semantics (oracle:
//!   `git check-ignore`). At each path level every `.gitignore` layer is
//!   evaluated, then every tsv layer, so a tsv `!` can re-include a gitignore'd
//!   path (subject to git's parent-dir rule) and a deeper layer wins over a
//!   shallower one.
//!
//! Locating the files, resolving paths relative to them, and walking directories
//! are the callers' jobs. The crate is layer-agnostic: the caller decides which
//! files become layers (tsv reads `.formatignore` hierarchically and, at the repo
//! root only, a `.prettierignore` shadowed by a `.formatignore`).
//!
//! ```
//! use tsv_ignore::IgnoreStack;
//!
//! let mut stack = IgnoreStack::new();
//! stack.push_gitignore("", "build/\n*.log\n"); // root .gitignore
//! stack.push_gitignore("a", "!*.log\n"); // a/.gitignore re-includes logs
//! stack.push_tsv("", "*.snap\n"); // root tsv layer, applied after the gitignores
//! assert!(stack.is_ignored("build/out.js", false));
//! assert!(stack.is_ignored("debug.log", false));
//! assert!(!stack.is_ignored("a/debug.log", false)); // a deeper `!` wins
//! assert!(stack.is_ignored("test/x.snap", false)); // via the tsv layer
//! ```

mod glob;

use glob::{PathSeg, Seg, match_segments, parse_segment};

/// A compiled set of ignore rules, applied in source order with last-match-wins
/// semantics (a later `!` negation re-includes an earlier exclusion).
#[derive(Debug, Default)]
pub struct IgnoreRules {
    rules: Vec<Rule>,
}

#[derive(Debug)]
struct Rule {
    segs: Vec<Seg>,
    /// `!`-prefixed: re-includes a path an earlier rule excluded.
    negated: bool,
    /// trailing-`/`: matches directories only.
    dir_only: bool,
}

impl Rule {
    fn matches(&self, path: &[PathSeg<'_>], is_dir: bool) -> bool {
        if self.dir_only && !is_dir {
            return false;
        }
        match_segments(&self.segs, path)
    }

    /// For a negated rule, its fixed leading path — the run of literal (non-glob)
    /// segments before the first wildcard or `**` — expressed relative to the
    /// rule's layer. `None` for a non-negated rule. A *floating* negation
    /// (`!name`, which parses to a leading `**`) yields an empty path, so it is
    /// never anchored strictly under any directory. Powers
    /// [`IgnoreStack::has_negation_under`].
    fn negation_leading_path(&self) -> Option<Vec<String>> {
        self.negated
            .then(|| self.segs.iter().map_while(Seg::literal).collect())
    }
}

impl IgnoreRules {
    /// Parses the text of one ignore file into a rule set.
    pub fn parse(content: &str) -> Self {
        // `lines()` splits on `\n` and strips a trailing `\r`, handling CRLF
        let rules = content.lines().filter_map(parse_line).collect();
        Self { rules }
    }

    /// Whether there are no rules (no ignore files, or only comments/blanks).
    /// Callers use this to skip per-file matching entirely.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Whether `path` is ignored. `path` is relative to the ignore-file root,
    /// `/`-separated; `is_dir` marks it as a directory (so trailing-`/`
    /// patterns apply).
    ///
    /// Evaluated top-down over the path's ancestors: as soon as an ancestor
    /// directory is excluded, the path is ignored and no deeper `!` can
    /// re-include it — matching git's "cannot re-include a file if a parent
    /// directory is excluded" rule. This also means a file under a directory
    /// that matches any rule (e.g. `build/`) is reported ignored without the
    /// caller having to test the directory separately.
    pub fn is_ignored(&self, path: &str, is_dir: bool) -> bool {
        walk_ancestors_ignored(path, is_dir, |prefix, dir| self.last_match(prefix, dir))
    }

    /// The polarity of the last rule matching `path` (one path, its segments
    /// relative to this file's root), or `None` if no rule matches.
    /// `Some(true)` excludes, `Some(false)` re-includes (a `!` negation). This
    /// is the per-level primitive [`IgnoreStack`] layers across files; on its
    /// own it does *not* apply the ancestor prune (the caller iterates prefixes).
    fn last_match(&self, path: &[PathSeg<'_>], is_dir: bool) -> Option<bool> {
        let mut state = None;
        for rule in &self.rules {
            if rule.matches(path, is_dir) {
                state = Some(!rule.negated);
            }
        }
        state
    }
}

/// A hierarchical, git-faithful ignore evaluator: a stack of per-directory
/// `.gitignore` layers plus a parallel stack of per-directory tsv layers.
///
/// [`push_gitignore`](IgnoreStack::push_gitignore) / [`push_tsv`](IgnoreStack::push_tsv)
/// add one directory's `.gitignore` / tsv file, anchored at that directory
/// relative to the format root (the caller's chosen scope root). Layers are
/// pushed shallowest-first (the order a DFS descends), and the CLI
/// [`pop_gitignore`](IgnoreStack::pop_gitignore)s / [`pop_tsv`](IgnoreStack::pop_tsv)s
/// on the way back up; non-traversal callers (the VS Code extension) just push
/// the whole chain.
///
/// [`is_ignored`](IgnoreStack::is_ignored) walks the path's ancestors top-down
/// and, at each level, evaluates every applicable `.gitignore` layer
/// (shallow→deep) then every applicable tsv layer (shallow→deep), last-match
/// winning. So a tsv layer overrides any `.gitignore`, a deeper layer overrides
/// a shallower one of its own kind, and a positive match at an ancestor prunes
/// the subtree before a deeper `!` can re-include it — matching git's "cannot
/// re-include a file whose parent directory is excluded" rule. The gitignore-only
/// behavior is byte-for-byte `git check-ignore` (on case-sensitive filesystems;
/// see the crate docs).
#[derive(Debug, Default)]
pub struct IgnoreStack {
    /// `.gitignore` layers in shallow→deep push order.
    gitignore: Vec<Layer>,
    /// tsv layers (`.formatignore`, plus a repo-root `.prettierignore` the caller
    /// may resolve) in shallow→deep push order, evaluated after every `.gitignore`.
    tsv: Vec<Layer>,
}

/// One directory's ignore file within an [`IgnoreStack`] — a `.gitignore` or a
/// tsv file, anchored at the directory it was read from.
#[derive(Debug)]
struct Layer {
    /// The directory, segments relative to the format root; empty = the root.
    anchor: Vec<String>,
    rules: IgnoreRules,
}

impl Layer {
    /// `prefix` expressed relative to this layer's anchor, or `None` when the
    /// anchor is not a *strict* ancestor of `prefix`. Equal length (the anchor
    /// directory itself) returns `None`: a directory's own `.gitignore` never
    /// classifies that directory — its parent's files do.
    fn relativize<'a, 'p>(&self, prefix: &'p [PathSeg<'a>]) -> Option<&'p [PathSeg<'a>]> {
        if self.anchor.len() >= prefix.len() {
            return None;
        }
        self.anchor
            .iter()
            .zip(prefix)
            .all(|(a, p)| a.as_str() == p.text)
            .then(|| &prefix[self.anchor.len()..])
    }
}

impl IgnoreStack {
    /// An empty stack (ignores nothing until layers are added).
    pub fn new() -> Self {
        Self::default()
    }

    /// Push one directory's `.gitignore`. `anchor` is the directory relative to
    /// the format root, `/`-separated (empty string = the root).
    pub fn push_gitignore(&mut self, anchor: &str, content: &str) {
        self.gitignore.push(Layer {
            anchor: split_path(anchor),
            rules: IgnoreRules::parse(content),
        });
    }

    /// Pop the most recently pushed `.gitignore` layer (a DFS unwinding out of a
    /// directory).
    pub fn pop_gitignore(&mut self) {
        self.gitignore.pop();
    }

    /// Push one directory's tsv file, applied after every `.gitignore`. `anchor`
    /// is the directory relative to the format root, `/`-separated (empty string
    /// = the root). The caller resolves which file's content this is (e.g.
    /// `.formatignore`, or a repo-root `.prettierignore` it shadows).
    pub fn push_tsv(&mut self, anchor: &str, content: &str) {
        self.tsv.push(Layer {
            anchor: split_path(anchor),
            rules: IgnoreRules::parse(content),
        });
    }

    /// Pop the most recently pushed tsv layer (a DFS unwinding out of a directory).
    pub fn pop_tsv(&mut self) {
        self.tsv.pop();
    }

    /// Whether no layer carries any rule — callers skip per-path matching.
    pub fn is_empty(&self) -> bool {
        self.gitignore.iter().all(|l| l.rules.is_empty())
            && self.tsv.iter().all(|l| l.rules.is_empty())
    }

    /// Whether any `.gitignore` layer has been pushed — true even for an
    /// empty/comments-only `.gitignore`, since its mere presence is what turns a
    /// caller's heuristic off (matching git, for which an empty `.gitignore`
    /// still establishes the gitignore regime). tsv's discovery uses this to
    /// assert its `heuristic_active ⟹ no .gitignore layer` invariant.
    pub fn has_gitignore_layers(&self) -> bool {
        !self.gitignore.is_empty()
    }

    /// The format-root-relative directory anchors (`/`-joined; `""` = the root)
    /// of the pushed `.gitignore` layers, shallow→deep. Lets a per-file discovery
    /// replay that has no top-down walk — the VS Code extension formats one open
    /// document at a time — reconstruct `heuristic_active` for each ancestor
    /// directory: the build-output heuristic is off at a level once a `.gitignore`
    /// anchored above it is present. Allocates; off the matcher's hot path.
    pub fn gitignore_anchors(&self) -> Vec<String> {
        self.gitignore
            .iter()
            .map(|layer| layer.anchor.join("/"))
            .collect()
    }

    /// Whether `path` (relative to the format root, `/`-separated) is ignored.
    /// `is_dir` marks `path` itself as a directory so trailing-`/` patterns
    /// apply to it.
    pub fn is_ignored(&self, path: &str, is_dir: bool) -> bool {
        walk_ancestors_ignored(path, is_dir, |prefix, dir| self.last_match_at(prefix, dir))
    }

    /// Whether `path`'s **own** last-match polarity is an exclusion — the leaf
    /// path evaluated against every layer once, with **no ancestor walk**. Unlike
    /// [`is_ignored`](Self::is_ignored) it does not apply git's parent-directory
    /// prune, so a file under an excluded `build/` is reported *not* ignored
    /// unless a rule matches the file path itself.
    ///
    /// # Contract
    ///
    /// Equivalent to [`is_ignored`](Self::is_ignored) **only when every ancestor
    /// directory of `path` is already known not-ignored.** tsv's discovery walk
    /// guarantees that — it prunes ignored directories before descending, and
    /// gates the initial root with a full [`is_ignored`](Self::is_ignored) — so it
    /// uses this O(1)-per-level query instead of re-walking O(depth) ancestor
    /// prefixes per entry (the matcher dominates discovery cost). **Do not** call
    /// it for an arbitrary path whose ancestors haven't been cleared; that is what
    /// [`is_ignored`](Self::is_ignored) is for.
    pub fn is_ignored_leaf(&self, path: &str, is_dir: bool) -> bool {
        let segments = path_segments(path);
        !segments.is_empty() && self.last_match_at(&segments, is_dir) == Some(true)
    }

    /// Whether `path` is explicitly *re-included* — the last rule matching this
    /// exact path is a `!` negation (`Some(false)`), with no ancestor prune
    /// applied. Distinct from `!is_ignored`: a path no rule mentions is neither
    /// ignored nor re-included. tsv's discovery uses this to let an explicit
    /// re-include override the build-output heuristic.
    pub fn is_reincluded(&self, path: &str, is_dir: bool) -> bool {
        let segments = path_segments(path);
        if segments.is_empty() {
            return false;
        }
        self.last_match_at(&segments, is_dir) == Some(false)
    }

    /// Whether some pushed **tsv-layer** rule is a negation (`!`) anchored
    /// *strictly under* `prefix` (a `/`-separated directory path relative to the
    /// format root). "Anchored strictly under" means the rule's fixed leading
    /// path — its layer's anchor plus the rule's run of literal (non-glob)
    /// segments — has `prefix` as a strict prefix (equal or longer fails).
    ///
    /// Only `.gitignore` layers are *not* consulted; among tsv-layer rules only
    /// anchored negations count. A floating `!keep.ts` (parsed with a leading
    /// `**`) targets any depth, so it is never "under" one directory; and
    /// `!dist/` re-includes the directory itself, which is not *under* it.
    ///
    /// tsv's discovery uses this to warn when its build-output heuristic prunes a
    /// directory that a tsv-layer `!` was trying to re-include *into* — that
    /// re-include is a silent no-op, because git's parent-directory rule (matched
    /// in the `.gitignore` regime) blocks re-including a descendant of an excluded
    /// directory. The sanctioned escape is re-including the directory itself
    /// (`!dist/`), which [`is_reincluded`](Self::is_reincluded) detects instead.
    pub fn has_negation_under(&self, prefix: &str) -> bool {
        let prefix_segs = split_segments(prefix);
        if prefix_segs.is_empty() {
            return false;
        }
        self.tsv.iter().any(|layer| {
            layer.rules.rules.iter().any(|rule| {
                rule.negation_leading_path().is_some_and(|leading| {
                    // the fixed path the negation is anchored to is `layer.anchor`
                    // followed by `leading`; `prefix_segs` must be a *strict*
                    // prefix of it (all match, and at least one segment beyond)
                    let mut concrete = layer.anchor.iter().chain(&leading);
                    prefix_segs
                        .iter()
                        .all(|seg| concrete.next().is_some_and(|c| c.as_str() == *seg))
                        && concrete.next().is_some()
                })
            })
        })
    }

    /// The combined last-match polarity across every applicable layer at one
    /// `prefix` — each `.gitignore` layer (shallow→deep) then each tsv layer
    /// (shallow→deep), so a tsv layer wins over `.gitignore` and a deeper layer
    /// wins over a shallower one of its kind. `None` if no layer matches. This is
    /// the per-level primitive; it does *not* apply the ancestor prune
    /// ([`is_ignored`](Self::is_ignored) iterates prefixes,
    /// [`is_reincluded`](Self::is_reincluded) queries the leaf directly).
    fn last_match_at(&self, prefix: &[PathSeg<'_>], is_dir: bool) -> Option<bool> {
        let mut state: Option<bool> = None;
        for layer in self.gitignore.iter().chain(&self.tsv) {
            if let Some(rel) = layer.relativize(prefix)
                && let Some(v) = layer.rules.last_match(rel, is_dir)
            {
                state = Some(v);
            }
        }
        state
    }
}

/// Split a `/`-separated path into its meaningful segments, dropping empty and
/// `.` components (so `""`, `"."`, `"a//b"`, and `"./a"` all normalize cleanly).
/// The shared primitive behind every path-to-segments conversion here.
fn split_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect()
}

/// [`split_segments`] into owned segments, for a layer's stored anchor.
fn split_path(path: &str) -> Vec<String> {
    split_segments(path).into_iter().map(String::from).collect()
}

/// [`split_segments`] into [`PathSeg`]s, collecting each segment's chars once so
/// the glob matcher never re-collects them across rules/prefixes. The form every
/// path query (`is_ignored`, `is_reincluded`) feeds into the matcher.
fn path_segments(path: &str) -> Vec<PathSeg<'_>> {
    split_segments(path).into_iter().map(PathSeg::new).collect()
}

/// Walk `path`'s ancestors top-down, returning `true` as soon as `polarity`
/// reports a positive match (`Some(true)`) at any prefix. A positive match at
/// an ancestor prunes the subtree before a deeper `!` is ever consulted —
/// git's parent-directory rule; `Some(false)` (re-included here) and `None` (no
/// rule here) fall through to let deeper levels decide. `polarity` is the
/// per-prefix last-match lookup — one file's rules (`IgnoreRules::last_match`)
/// or a whole stack of layers (`IgnoreStack::last_match_at`). Every prefix
/// shorter than the full path is an ancestor directory; `is_dir` marks only the
/// leaf. The shared spine of both `is_ignored` methods.
fn walk_ancestors_ignored(
    path: &str,
    is_dir: bool,
    polarity: impl Fn(&[PathSeg<'_>], bool) -> Option<bool>,
) -> bool {
    let segments = path_segments(path);
    if segments.is_empty() {
        return false;
    }
    let last = segments.len() - 1;
    for k in 0..segments.len() {
        let component_is_dir = k < last || is_dir;
        if polarity(&segments[..=k], component_is_dir) == Some(true) {
            return true;
        }
    }
    false
}

/// Parses one raw line into a rule, or `None` for blanks and comments.
fn parse_line(raw: &str) -> Option<Rule> {
    let mut s = strip_trailing_spaces(raw);
    // a blank line, or a comment (`#`) — an escaped `\#` starts with `\`, so it
    // is not caught here and falls through to a literal in the tokenizer
    if s.is_empty() || s.starts_with('#') {
        return None;
    }

    // leading `!` negates (an escaped `\!` starts with `\` and is left alone)
    let negated = s.starts_with('!');
    if negated {
        s = &s[1..];
    }
    // trailing `/` restricts to directories
    let dir_only = s.ends_with('/');
    if dir_only {
        s = &s[..s.len() - 1];
    }
    // a leading or interior `/` anchors the pattern to the ignore-file root; a
    // pattern with no interior separator floats and matches at any depth
    let leading_slash = s.starts_with('/');
    if leading_slash {
        s = &s[1..];
    }
    // gitignore(5): "a backslash at the end of a pattern is an invalid pattern
    // that never matches." A final *unescaped* backslash is an odd-length trailing
    // run of `\` (an even run is escaped pairs — `foo\\` is a valid literal `foo\`).
    // A never-matching rule contributes nothing to last-match-wins, so dropping it
    // is equivalent to keeping it, and simpler than threading an always-false flag
    // through the matcher. (Checked after the trailing-`/` strip, matching git: a
    // dir-only `foo\/` is likewise invalid.)
    if s.bytes().rev().take_while(|&b| b == b'\\').count() % 2 == 1 {
        return None;
    }
    let anchored = leading_slash || s.contains('/');

    let mut segs: Vec<Seg> = s
        .split('/')
        .filter(|seg| !seg.is_empty())
        .map(parse_segment)
        .collect();
    if segs.is_empty() {
        // the pattern was only slashes (e.g. `/`) — nothing to match
        return None;
    }
    // a floating pattern is equivalent to one prefixed with `**/`
    if !anchored {
        segs.insert(0, Seg::DoubleStar);
    }
    Some(Rule {
        segs,
        negated,
        dir_only,
    })
}

/// Strips trailing spaces from a line, keeping a space escaped with `\`.
/// Per gitignore(5), trailing spaces are ignored unless backslash-quoted. The
/// kept text is always a prefix of `line`, so this borrows rather than
/// allocates; `' '` and `\` are ASCII and never appear as UTF-8 continuation
/// bytes, so the byte scan is correct on any input.
fn strip_trailing_spaces(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b' ' {
        // an odd run of backslashes before this space escapes it — stop trimming
        let backslashes = bytes[..end - 1]
            .iter()
            .rev()
            .take_while(|&&b| b == b'\\')
            .count();
        if backslashes % 2 == 1 {
            break;
        }
        end -= 1;
    }
    &line[..end]
}

#[cfg(test)]
mod tests {
    use super::{IgnoreRules, IgnoreStack};

    fn ig(content: &str) -> IgnoreRules {
        IgnoreRules::parse(content)
    }

    #[test]
    fn empty_rules_ignore_nothing() {
        let rules = ig("");
        assert!(rules.is_empty());
        assert!(!rules.is_ignored("anything.ts", false));
        assert!(!rules.is_ignored("a/b/c.ts", false));
    }

    #[test]
    fn comments_and_blanks_are_skipped() {
        let rules = ig("# a comment\n\n   \n# another\n");
        assert!(rules.is_empty());
    }

    #[test]
    fn floating_name_matches_at_any_depth() {
        let rules = ig("foo.ts\n");
        assert!(rules.is_ignored("foo.ts", false));
        assert!(rules.is_ignored("a/foo.ts", false));
        assert!(rules.is_ignored("a/b/foo.ts", false));
        assert!(!rules.is_ignored("bar.ts", false));
        assert!(!rules.is_ignored("a/foobar.ts", false));
    }

    #[test]
    fn anchored_pattern_only_matches_from_root() {
        let rules = ig("/foo.ts\n");
        assert!(rules.is_ignored("foo.ts", false));
        assert!(!rules.is_ignored("a/foo.ts", false));
    }

    #[test]
    fn interior_slash_anchors_to_root() {
        let rules = ig("src/gen.ts\n");
        assert!(rules.is_ignored("src/gen.ts", false));
        assert!(!rules.is_ignored("a/src/gen.ts", false));
        assert!(!rules.is_ignored("src/sub/gen.ts", false));
    }

    #[test]
    fn star_matches_within_a_segment_only() {
        let rules = ig("*.log\n");
        assert!(rules.is_ignored("debug.log", false));
        assert!(rules.is_ignored("a/b/debug.log", false));
        assert!(!rules.is_ignored("debug.log.txt", false));
        // `*` does not cross `/`
        let rules = ig("src/*.ts\n");
        assert!(rules.is_ignored("src/a.ts", false));
        assert!(!rules.is_ignored("src/sub/a.ts", false));
    }

    #[test]
    fn question_matches_one_char() {
        let rules = ig("file?.ts\n");
        assert!(rules.is_ignored("file1.ts", false));
        assert!(rules.is_ignored("fileA.ts", false));
        assert!(!rules.is_ignored("file.ts", false));
        assert!(!rules.is_ignored("file12.ts", false));
    }

    #[test]
    fn glob_is_code_point_granular() {
        // tsv matches glob metacharacters per Unicode *code point* (a Rust
        // `char`), not per byte (`git check-ignore`) or per UTF-16 code unit
        // (prettier's `ignore`). On non-ASCII these three oracles disagree, so
        // tsv can't match both — see the multibyte bullet in CLAUDE.md. Pinned
        // here so a refactor of the segment matcher can't silently flip the
        // granularity. (Verified out of band: `git check-ignore` does NOT ignore
        // `fileé.ts`; `ignore` v5.3.2 DOES but does NOT ignore `a😀.ts`.)
        let rules = ig("file?.ts\n");
        assert!(rules.is_ignored("fileX.ts", false));
        assert!(rules.is_ignored("fileé.ts", false)); // `é` is one code point; git would not
        assert!(!rules.is_ignored("fileéé.ts", false)); // two → one `?` matches one

        // an astral char is one code point (one `char`) but two UTF-16 units, so
        // a single `?` spans it for tsv but not for prettier's `ignore`
        let rules = ig("a?.ts\n");
        assert!(rules.is_ignored("a😀.ts", false));

        // char-class ranges compare by code point too: `é` (U+00E9) is outside a–z
        let rules = ig("x[a-z].ts\n");
        assert!(rules.is_ignored("xa.ts", false));
        assert!(!rules.is_ignored("xé.ts", false));
    }

    #[test]
    fn char_class_and_range_and_negation() {
        let rules = ig("file[0-9].ts\n");
        assert!(rules.is_ignored("file3.ts", false));
        assert!(!rules.is_ignored("fileA.ts", false));

        let rules = ig("file[!0-9].ts\n");
        assert!(rules.is_ignored("fileA.ts", false));
        assert!(!rules.is_ignored("file3.ts", false));

        let rules = ig("file[abc].ts\n");
        assert!(rules.is_ignored("filea.ts", false));
        assert!(!rules.is_ignored("filed.ts", false));
    }

    #[test]
    fn char_class_caret_negation_and_literal_caret() {
        // `[^...]` negates exactly like `[!...]` — git's matcher accepts both.
        let rules = ig("file[^0-9].ts\n");
        assert!(rules.is_ignored("fileA.ts", false));
        assert!(!rules.is_ignored("file3.ts", false));

        // `^` is the negation marker only as the *first* class character;
        // elsewhere it is a literal member (so `[ab^]` matches `a`, `b`, or `^`).
        let rules = ig("lit[ab^].ts\n");
        assert!(rules.is_ignored("lit^.ts", false));
        assert!(rules.is_ignored("lita.ts", false));
        assert!(!rules.is_ignored("litc.ts", false));
    }

    #[test]
    fn dir_only_matches_directories_not_files() {
        let rules = ig("build/\n");
        // a directory named build, and everything under it
        assert!(rules.is_ignored("build", true));
        assert!(rules.is_ignored("build/out.js", false));
        assert!(rules.is_ignored("a/build/out.js", false));
        assert!(rules.is_ignored("a/build", true));
        // a *file* named build is not matched by a dir-only pattern
        assert!(!rules.is_ignored("build", false));
    }

    #[test]
    fn plain_name_matches_both_file_and_dir_contents() {
        let rules = ig("node_modules\n");
        assert!(rules.is_ignored("node_modules", true));
        assert!(rules.is_ignored("node_modules", false));
        assert!(rules.is_ignored("node_modules/pkg/index.js", false));
        assert!(rules.is_ignored("packages/x/node_modules/pkg/index.js", false));
    }

    #[test]
    fn leading_double_star_matches_any_depth() {
        let rules = ig("**/gen.ts\n");
        assert!(rules.is_ignored("gen.ts", false));
        assert!(rules.is_ignored("a/gen.ts", false));
        assert!(rules.is_ignored("a/b/gen.ts", false));
    }

    #[test]
    fn trailing_double_star_matches_contents() {
        let rules = ig("logs/**\n");
        assert!(rules.is_ignored("logs/today.log", false));
        assert!(rules.is_ignored("logs/sub/today.log", false));
        assert!(!rules.is_ignored("other/today.log", false));
        // git: a trailing `**` matches everything *inside* the anchor (>= 1
        // segment deep), never the anchor directory itself.
        assert!(!rules.is_ignored("logs", true));
    }

    #[test]
    fn trailing_double_star_does_not_match_anchor_dir() {
        // git: `foo/**` matches everything inside foo, never foo itself, so a
        // later `!foo/keep.ts` is allowed to re-include the file (the parent
        // dir is not excluded). The `foo/*` form behaves identically; only the
        // `**` anchor-match broke it. Pinned against `git check-ignore`.
        let rules = ig("foo/**\n");
        assert!(!rules.is_ignored("foo", true));
        assert!(rules.is_ignored("foo/keep.ts", false));
        assert!(rules.is_ignored("foo/sub", true));
        assert!(rules.is_ignored("foo/sub/file.ts", false));

        let rules = ig("foo/**\n!foo/keep.ts\n");
        assert!(!rules.is_ignored("foo/keep.ts", false));
        assert!(rules.is_ignored("foo/sub/file.ts", false));
    }

    #[test]
    fn trailing_double_star_dir_only_spares_direct_files() {
        // git: `foo/**/` (dir-only trailing) ignores only proper subdirectories
        // of foo, leaving files directly in foo formattable.
        let rules = ig("foo/**/\n");
        assert!(!rules.is_ignored("foo", true));
        assert!(!rules.is_ignored("foo/keep.ts", false));
        assert!(rules.is_ignored("foo/sub", true));
        assert!(rules.is_ignored("foo/sub/file.ts", false));
    }

    #[test]
    fn middle_double_star_spans_directories() {
        let rules = ig("a/**/b.ts\n");
        assert!(rules.is_ignored("a/b.ts", false));
        assert!(rules.is_ignored("a/x/b.ts", false));
        assert!(rules.is_ignored("a/x/y/b.ts", false));
        assert!(!rules.is_ignored("x/a/b.ts", false));
    }

    #[test]
    fn multiple_double_stars_span_independently() {
        // more than one `**` forces the matcher's general split-point search (the
        // trailing-anchor fast path only fires when the tail after a `**` has no
        // further `**`); each `**` independently matches zero or more segments.
        let rules = ig("a/**/b/**/c.ts\n");
        assert!(rules.is_ignored("a/b/c.ts", false)); // both `**` match zero
        assert!(rules.is_ignored("a/x/b/y/c.ts", false)); // each matches one
        assert!(rules.is_ignored("a/x/y/b/z/w/c.ts", false)); // each matches several
        assert!(!rules.is_ignored("a/b/x.ts", false)); // trailing `c.ts` absent
        assert!(!rules.is_ignored("a/x/c.ts", false)); // middle `b` absent
        assert!(!rules.is_ignored("x/a/b/c.ts", false)); // anchored `a` not at root
    }

    #[test]
    fn negation_reincludes() {
        let rules = ig("*.ts\n!keep.ts\n");
        assert!(rules.is_ignored("drop.ts", false));
        assert!(rules.is_ignored("a/drop.ts", false));
        assert!(!rules.is_ignored("keep.ts", false));
        assert!(!rules.is_ignored("a/keep.ts", false));
    }

    #[test]
    fn negation_order_is_last_match_wins() {
        // re-include then exclude again
        let rules = ig("*.ts\n!keep.ts\nkeep.ts\n");
        assert!(rules.is_ignored("keep.ts", false));
    }

    #[test]
    fn stack_is_reincluded_reports_explicit_negation_polarity() {
        // is_reincluded = "the last matching rule re-includes this exact path"
        // (per-path polarity, no ancestor prune) — distinct from `!is_ignored`:
        // a path no rule mentions is neither ignored nor re-included. This is the
        // heuristic-override primitive (#5): a tsv `!build/` re-includes the dir.
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "*.ts\n!keep.ts\n");
        assert!(stack.is_reincluded("keep.ts", false)); // `!keep.ts` wins
        assert!(!stack.is_reincluded("drop.ts", false)); // excluded, not re-included
        assert!(!stack.is_reincluded("other.js", false)); // no rule matches

        // a dir-only negation re-includes the directory, not a same-named file
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "!build/\n");
        assert!(stack.is_reincluded("build", true));
        assert!(!stack.is_reincluded("build", false));
        assert!(!stack.is_reincluded("src", true)); // no rule → not re-included

        // a plain exclude is ignored, not re-included
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "build/\n");
        assert!(!stack.is_reincluded("build", true));
        assert!(stack.is_ignored("build", true));
    }

    #[test]
    fn stack_has_negation_under_only_counts_anchored_negations() {
        // an anchored negation strictly under the dir → true (the silent-no-op
        // case the discovery warning targets: `!dist/keep.ts` cannot reach a
        // file under a pruned `dist`)
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "!dist/keep.ts\n");
        assert!(stack.has_negation_under("dist"));
        assert!(!stack.has_negation_under("build")); // unrelated dir

        // re-including the directory ITSELF is not "under" it (that's the
        // sanctioned escape, detected by is_reincluded, not here)
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "!dist/\n");
        assert!(!stack.has_negation_under("dist"));
        assert!(stack.is_reincluded("dist", true)); // the escape works

        // a floating `!keep.ts` parses to a leading `**` — it targets any depth,
        // not a particular dir, so it must NOT trigger just because a keep.ts
        // happens to sit under dist
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "!keep.ts\n");
        assert!(!stack.has_negation_under("dist"));

        // a non-negated exclude is not a re-include attempt
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "dist/keep.ts\n");
        assert!(!stack.has_negation_under("dist"));

        // no rules at all
        let stack = IgnoreStack::new();
        assert!(!stack.has_negation_under("dist"));

        // `.gitignore`-layer negations are never consulted — only tsv layers
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "!dist/keep.ts\n");
        assert!(!stack.has_negation_under("dist"));
    }

    #[test]
    fn stack_has_negation_under_respects_layer_anchor() {
        // a deeper tsv layer's negation is anchored at its own directory: a
        // `!dist/keep.ts` in `foo/.formatignore` is under `foo/dist`, not `dist`
        let mut stack = IgnoreStack::new();
        stack.push_tsv("foo", "!dist/keep.ts\n");
        assert!(stack.has_negation_under("foo/dist"));
        assert!(!stack.has_negation_under("dist"));

        // a root negation with a nested anchored path sits under each strict
        // prefix dir it passes through, but never the leaf path itself (equal,
        // not strictly under) nor an unrelated sibling
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "!src/gen/keep.ts\n");
        assert!(stack.has_negation_under("src/gen"));
        assert!(stack.has_negation_under("src"));
        assert!(!stack.has_negation_under("src/gen/keep.ts"));
        assert!(!stack.has_negation_under("src/other"));
    }

    #[test]
    fn cannot_reinclude_under_excluded_dir() {
        // git: a negation cannot re-include a file whose parent dir is excluded
        let rules = ig("build/\n!build/keep.js\n");
        assert!(rules.is_ignored("build/keep.js", false));
    }

    #[test]
    fn reincluding_the_dir_itself_allows_descendants() {
        let rules = ig("build/\n!build/\n");
        // the directory is re-included, so a file under it is evaluated normally
        assert!(!rules.is_ignored("build/keep.js", false));
    }

    #[test]
    fn escaped_special_characters_are_literal() {
        // leading `#` escaped → a literal name starting with `#`
        let rules = ig("\\#notacomment.ts\n");
        assert!(rules.is_ignored("#notacomment.ts", false));

        // leading `!` escaped → literal, not a negation
        let rules = ig("\\!bang.ts\n");
        assert!(rules.is_ignored("!bang.ts", false));

        // escaped `*` → literal star, not a wildcard
        let rules = ig("a\\*b.ts\n");
        assert!(rules.is_ignored("a*b.ts", false));
        assert!(!rules.is_ignored("axb.ts", false));
    }

    #[test]
    fn trailing_backslash_is_an_invalid_pattern() {
        // gitignore(5): "a backslash at the end of a pattern is an invalid pattern
        // that never matches." A final unescaped backslash matches nothing — not
        // even a file literally ending in `\`. Pinned against `git check-ignore`.
        let rules = ig("bar\\\n");
        assert!(rules.is_empty()); // the lone invalid rule is dropped
        assert!(!rules.is_ignored("bar", false));
        assert!(!rules.is_ignored("bar\\", false));

        // a dir-only `foo\/` is invalid the same way: once the structural trailing
        // `/` is stripped, the pattern still ends in an unescaped backslash. git
        // agrees — it does not match the `foo` directory.
        let rules = ig("foo\\/\n");
        assert!(rules.is_empty());
        assert!(!rules.is_ignored("foo", true));

        // an *even* trailing run is escaped pairs, so `foo\\` is a valid pattern
        // for a literal `foo\` (and git matches it).
        let rules = ig("foo\\\\\n");
        assert!(!rules.is_empty());
        assert!(rules.is_ignored("foo\\", false));
        assert!(!rules.is_ignored("foo", false));
    }

    #[test]
    fn crlf_line_endings_parse_like_lf() {
        // `parse` splits with `str::lines()`, which drops a trailing `\r`, so a
        // CRLF ignore file behaves identically to an LF one — the trailing-`/`
        // and negation markers are detected on the `\r`-stripped line.
        let rules = ig("*.log\r\nbuild/\r\n!keep.log\r\n");
        assert!(rules.is_ignored("debug.log", false));
        assert!(rules.is_ignored("build/out.js", false));
        assert!(!rules.is_ignored("keep.log", false));
        assert!(!rules.is_ignored("src/app.ts", false));
    }

    #[test]
    fn trailing_spaces_are_trimmed_unless_escaped() {
        let rules = ig("foo.ts   \n");
        assert!(rules.is_ignored("foo.ts", false));

        // multibyte content before trimmed trailing spaces still slices on a char
        // boundary (the byte-scan trims only ASCII spaces, never a content byte)
        let rules = ig("café   \n");
        assert!(rules.is_ignored("café", false));

        // an escaped trailing space is part of the name
        let rules = ig("foo\\ \n");
        assert!(rules.is_ignored("foo ", false));
        assert!(!rules.is_ignored("foo", false));
    }

    #[test]
    fn realistic_prettierignore() {
        let rules = ig(
            "# generated\nnode_modules\ndist/\nbuild/\n*.min.js\n.svelte-kit/\ntests/fixtures/\n",
        );
        assert!(rules.is_ignored("dist/app.js", false));
        assert!(rules.is_ignored("src/vendor/jquery.min.js", false));
        assert!(rules.is_ignored("tests/fixtures/x/input.svelte", false));
        assert!(rules.is_ignored(".svelte-kit/generated/root.svelte", false));
        assert!(!rules.is_ignored("src/app.ts", false));
        assert!(!rules.is_ignored("src/main.js", false));
    }

    // --- IgnoreStack (hierarchical) ---
    //
    // The boolean expectations below are pinned against `git check-ignore` on
    // the same tree of nested `.gitignore` files (verified out of band).

    #[test]
    fn stack_single_root_layer_matches_ignore_rules() {
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\n*.min.js\n!keep.min.js\n");
        assert!(stack.is_ignored("build/out.js", false));
        assert!(stack.is_ignored("a/b.min.js", false));
        assert!(!stack.is_ignored("keep.min.js", false));
        assert!(!stack.is_ignored("src/app.ts", false));
    }

    #[test]
    fn stack_empty_and_blank_layers_ignore_nothing() {
        let mut stack = IgnoreStack::new();
        assert!(stack.is_empty());
        stack.push_gitignore("", "");
        stack.push_tsv("", "# just a comment\n");
        assert!(stack.is_empty());
        assert!(!stack.is_ignored("a/x.ts", false));
    }

    /// The full hierarchical oracle tree:
    /// root `.gitignore`: `*.log`, `build/`, `!keep.log`, `/rootonly.ts`, `float.ts`
    /// `a/.gitignore`:    `!*.log`, `b/`
    /// `a/b/.gitignore`:  `!special.ts`
    fn oracle_stack() -> IgnoreStack {
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "*.log\nbuild/\n!keep.log\n/rootonly.ts\nfloat.ts\n");
        stack.push_gitignore("a", "!*.log\nb/\n");
        stack.push_gitignore("a/b", "!special.ts\n");
        stack
    }

    #[test]
    fn stack_deeper_negation_reincludes() {
        let stack = oracle_stack();
        // root `*.log` ignores, but `a/.gitignore`'s `!*.log` re-includes deeper
        assert!(stack.is_ignored("debug.log", false));
        assert!(!stack.is_ignored("a/debug.log", false));
        assert!(!stack.is_ignored("a/keep.log", false));
        // root `!keep.log` re-includes at the root
        assert!(!stack.is_ignored("keep.log", false));
    }

    #[test]
    fn stack_root_anchored_vs_floating() {
        let stack = oracle_stack();
        // `/rootonly.ts` is anchored to the root only
        assert!(stack.is_ignored("rootonly.ts", false));
        assert!(!stack.is_ignored("a/rootonly.ts", false));
        // `float.ts` floats and matches at any depth
        assert!(stack.is_ignored("float.ts", false));
        assert!(stack.is_ignored("a/float.ts", false));
    }

    #[test]
    fn stack_parent_prune_blocks_deeper_negation_across_files() {
        let stack = oracle_stack();
        // `a/b/` is excluded by `a/.gitignore`'s `b/`, so `a/b/.gitignore`'s
        // `!special.ts` CANNOT re-include a file under it — git's parent rule,
        // spanning two separate ignore files.
        assert!(stack.is_ignored("a/b/x.ts", false));
        assert!(stack.is_ignored("a/b/special.ts", false));
        // a dir-only `build/` matches the directory at any depth
        assert!(stack.is_ignored("build/x.ts", false));
        assert!(stack.is_ignored("packages/foo/build/y.ts", false));
    }

    #[test]
    fn stack_deeper_anchor_matches_relative_to_its_directory() {
        let mut stack = IgnoreStack::new();
        // `gen.ts` interior-anchored inside `pkg/` matches `pkg/src/gen.ts`,
        // not a root `src/gen.ts`
        stack.push_gitignore("", "");
        stack.push_gitignore("pkg", "src/gen.ts\n");
        assert!(stack.is_ignored("pkg/src/gen.ts", false));
        assert!(!stack.is_ignored("src/gen.ts", false));
        assert!(!stack.is_ignored("pkg/sub/src/gen.ts", false));
    }

    #[test]
    fn stack_tsv_layer_applies_after_gitignore() {
        // the tsv layer can re-include a gitignore'd directory wholesale…
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\n");
        stack.push_tsv("", "!build/\n");
        assert!(!stack.is_ignored("build/keep.ts", false));

        // …but, like git, a tsv `!build/keep.ts` can NOT re-include a file under
        // a still-excluded `build/` (only re-including the dir itself works)
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\n");
        stack.push_tsv("", "!build/keep.ts\n");
        assert!(stack.is_ignored("build/keep.ts", false));

        // and the tsv layer can exclude something `.gitignore` allows
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "*.snap\n");
        assert!(stack.is_ignored("a/x.snap", false));
        assert!(!stack.is_ignored("a/x.ts", false));
    }

    #[test]
    fn stack_tsv_layer_overrides_a_deeper_gitignore() {
        // the tsv layer is applied after every `.gitignore`, so a repo-root tsv
        // `!` wins even over a deeper `.gitignore` re-exclusion
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\n");
        stack.push_gitignore("a", "build/\n");
        stack.push_tsv("", "!build/\n");
        assert!(!stack.is_ignored("a/build/x.ts", false));
    }

    #[test]
    fn stack_tsv_layers_are_hierarchical() {
        // a deeper tsv layer overrides a shallower one (like `.gitignore`)
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "*.snap\n"); // root tsv ignores snaps everywhere
        stack.push_tsv("a", "!*.snap\n"); // a/ tsv re-includes them under a/
        assert!(stack.is_ignored("x.snap", false));
        assert!(stack.is_ignored("b/x.snap", false));
        assert!(!stack.is_ignored("a/x.snap", false)); // deeper tsv layer wins
        assert!(!stack.is_ignored("a/deep/x.snap", false));

        // a tsv layer's patterns are anchored at its own directory
        let mut stack = IgnoreStack::new();
        stack.push_tsv("pkg", "gen.ts\n"); // floating, but only within pkg/
        assert!(stack.is_ignored("pkg/gen.ts", false));
        assert!(stack.is_ignored("pkg/sub/gen.ts", false));
        assert!(!stack.is_ignored("gen.ts", false)); // outside the pkg anchor
    }

    #[test]
    fn stack_pop_tsv_unwinds_a_layer() {
        let mut stack = IgnoreStack::new();
        stack.push_tsv("", "*.snap\n");
        stack.push_tsv("a", "!*.snap\n");
        assert!(!stack.is_ignored("a/x.snap", false));
        // leaving `a/` drops its re-include, so the root rule applies again
        stack.pop_tsv();
        assert!(stack.is_ignored("a/x.snap", false));
    }

    #[test]
    fn stack_pop_gitignore_unwinds_a_layer() {
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "*.log\n");
        stack.push_gitignore("a", "!*.log\n");
        assert!(!stack.is_ignored("a/x.log", false));
        // leaving `a/` drops its negation, so the root rule applies again
        stack.pop_gitignore();
        assert!(stack.is_ignored("a/x.log", false));
    }

    #[test]
    fn stack_directory_query_for_pruning() {
        // discovery asks `is_ignored(dir, true)` to prune a whole subtree
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\nnode_modules\n");
        assert!(stack.is_ignored("build", true));
        assert!(stack.is_ignored("a/node_modules", true));
        assert!(!stack.is_ignored("src", true));
    }

    #[test]
    fn stack_is_ignored_leaf_skips_the_ancestor_prune() {
        // is_ignored_leaf reports only the leaf path's own last-match — it
        // diverges from is_ignored exactly where an *ancestor* directory is what
        // excludes the path. Discovery relies on the two being equivalent *when
        // ancestors are known clean* (it prunes ignored dirs before descending and
        // gates the root with full is_ignored), so this pins the difference.
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\n");
        // the excluded dir itself: both agree (a rule matches the leaf)
        assert!(stack.is_ignored("build", true));
        assert!(stack.is_ignored_leaf("build", true));
        // a file UNDER it: is_ignored prunes via the `build` ancestor; the
        // leaf-only query does not (no rule matches `build/app.ts` as a file)
        assert!(stack.is_ignored("build/app.ts", false));
        assert!(!stack.is_ignored_leaf("build/app.ts", false));
        // a deeper subdir under it: same divergence, two levels down
        assert!(stack.is_ignored("build/sub", true));
        assert!(!stack.is_ignored_leaf("build/sub", true));

        // a leaf a rule matches directly: both agree at any depth
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "*.log\n");
        assert!(stack.is_ignored("a/b/x.log", false));
        assert!(stack.is_ignored_leaf("a/b/x.log", false));
        assert!(!stack.is_ignored_leaf("a/b/x.ts", false));
    }
}
