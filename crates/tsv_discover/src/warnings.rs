//! The diagnostics discovery raises, built once here so the native CLI and the JS CLI emit
//! identical text: the excluded-argument warning ([`excluded_argument_warning`]), the
//! heuristic-shadow warning ([`shadow_warning`]) and the re-include ladders both offer
//! ([`reinclude_lines`], [`shadow_reinclude_lines`]), the `.prettierignore` heads-ups, the
//! symlinked-`.gitignore` warning and the unresolvable-root error.

use crate::quote::{UNSPELLABLE_REASON, line_can_spell, quote_path, quote_path_owned};
use std::fmt::Write as _;
use tsv_ignore::{IgnoreSource, IgnoreStack, Negation, NegationTail};

/// The stderr warning for a pruned directory that a tsv-layer `!` rule was trying to
/// re-include *into*, or `None` when no such rule is written
/// ([`IgnoreStack::negation_under`], which is how [`classify_dir`](crate::classify_dir) reaches
/// [`DirVerdict::PruneWithWarning`](crate::DirVerdict::PruneWithWarning)). The re-include is a silent no-op — git's
/// parent-directory rule — whether the build-output heuristic pruned the directory or an
/// ignore rule excluded it, and the text says which. The remedy is the lines that reach
/// what the rules named, for the deepest file holding one (whose rules are read last),
/// spelled relative to that file's directory and anchored ([`shadow_reinclude_lines`]):
/// the pruned directory re-included, its contents excluded again, and so on one level at
/// a time down to the deepest directory each rule's literal path names, then every such
/// rule's own pattern with a leading `/` — `!/dist/`, `/dist/*`, `!/dist/keep.ts` for
/// `!dist/keep.ts`, with `!/dist/sub/`, `/dist/sub/*` in between for `!dist/sub/keep.ts`.
/// Every line is anchored (without which a one-segment `!dist/` re-includes a `dist` at
/// every depth) and spells its path literally ([`pattern_path`]); spelled from the format
/// root instead, the lines would do nothing in a nested file, and outside a repo — where
/// the format root is the filesystem root — in any file. A tsv layer is read after every
/// `.gitignore` and a later line wins within a file, so the lines override the excluding
/// rule wherever it sits — except in a tsv file *deeper* than the re-include's, which is
/// read after it: that rule is the one to narrow or negate, and the warning says so
/// instead — adding the lines that pass a `.gitignore` rule standing behind it
/// ([`IgnoreStack::gitignore_exclusion`]), which narrowing alone would leave in force.
///
/// `d` is the pruned directory relative to the format root, `/`-separated; `loose_root`
/// is [`excluded_argument_warning`]'s, the format root's display path outside a git
/// repo. Produced **once**, here, so the native CLI and the WASM binding emit the
/// identical text.
#[must_use]
pub fn shadow_warning(d: &str, loose_root: Option<&str>, stack: &IgnoreStack) -> Option<String> {
    let negation = stack.negation_under(d)?;
    let segments = tsv_ignore::split_segments(d);
    let dir = path_display(&segments, loose_root);
    let rule_file = ignore_file_display(
        &segments[..negation.anchor_depth],
        negation.source,
        loose_root,
    );
    // the pruned directory's depth relative to the rules' file, where the lines start:
    // "adding …, in that order, to <file>"; the directory escape alone where no line can
    // spell a rule; and nothing — the clause below in its place — where none can spell
    // the directory itself
    let below = &segments[negation.anchor_depth..];
    let adding = match shadow_reinclude_lines(&negation, below.len()) {
        Some(lines) => Some(format!(
            "adding {}, in that order, to {rule_file}",
            quoted_list(&lines)
        )),
        None if below.iter().all(|segment| line_can_spell(segment)) => Some(format!(
            "adding `!/{}/` to {rule_file} first, since no ignore-file line can spell the control character in the re-include",
            pattern_path(below)
        )),
        None => None,
    };
    let cause = match stack.exclusion(d, true) {
        Some(exclusion) => {
            let excluding_file = ignore_file_display(
                &segments[..exclusion.anchor_depth],
                exclusion.source,
                loose_root,
            );
            if exclusion.source != IgnoreSource::Gitignore
                && exclusion.anchor_depth > negation.anchor_depth
            {
                // a tsv rule in a file deeper than the re-includes' is read after any line
                // added there, so it is the user's to narrow — and where a `.gitignore`
                // rule stands behind it, narrowing alone leaves that one in force, so the
                // lines that pass it go beside the narrowing: one warning, not one per rerun
                let remedy = match stack.gitignore_exclusion(d, true) {
                    None => "narrow or negate that rule to format it".to_string(),
                    Some(git) => {
                        let git_file = ignore_file_display(
                            &segments[..git.anchor_depth],
                            git.source,
                            loose_root,
                        );
                        let excluded = if git.depth == segments.len() {
                            "it".to_string()
                        } else {
                            path_display(&segments[..git.depth], loose_root)
                        };
                        match &adding {
                            Some(adding) => format!(
                                "narrow or negate that rule, and re-include it past the rule in {git_file} that excludes {excluded} by {adding}"
                            ),
                            None => format!(
                                "narrow or negate that rule; no re-include past the rule in {git_file} that excludes {excluded} can be offered, since {UNSPELLABLE_REASON}"
                            ),
                        }
                    }
                };
                return Some(format!(
                    "{dir} is excluded by a rule in {excluding_file}, so a re-include under it in {rule_file} does nothing; {remedy}"
                ));
            }
            format!("excluded by a rule in {excluding_file}")
        }
        None => "skipped by tsv's build-output heuristic".to_string(),
    };
    let remedy = match adding {
        Some(adding) => format!("re-include it by {adding}"),
        None => UNSPELLABLE_REASON.to_string(),
    };
    Some(format!(
        "{dir} is {cause}, so a re-include under it in {rule_file} does nothing; {remedy}"
    ))
}

/// The lines [`shadow_warning`] suggests, for every rule of `negation` at once: each
/// directory from the pruned one (`depth` segments into the rules' leading paths) down to
/// the deepest a rule's literal path names is re-included and its contents excluded again
/// (`!/dist/`, `/dist/*`), parents first, and then every rule's own pattern anchored — so
/// a rule the `/dist/*` line would otherwise silence is re-spelled after it. A rule
/// reaching *below* its deepest literal directory ([`NegationTail::Descendants`]:
/// `!dist/**/keep.ts`, `!dist/*/keep.ts`) opens that directory's whole subtree instead —
/// `/dist/**` then `!/dist/**/`, git's idiom for "every directory, no file" — since a
/// `/dist/*` would close the directories the rule reaches into, and no pair is spelled
/// for a directory under one so opened. A directory a rule re-includes **whole** — a
/// directory-only re-include, `!dist/sub/` — takes no close at all: the author asked for
/// the subtree, so nothing under it is excluded again, no pair is spelled below it, and
/// where a `/dist/**` above it has already closed its files they are re-included in turn
/// (`!/dist/sub/**`). Without that, a second rule under the same pruned directory closed
/// what the first had re-included, and the files under `sub` were out of scope for good
/// with the warning gone. Always three lines or more: the pruned directory sits strictly
/// above what every rule reaches, so its own pair comes before the patterns.
///
/// `None` when a line could not spell a rule ([`line_can_spell`]): one ending in a
/// carriage return loses it, since a line's trailing `\r` is stripped with its line ending,
/// and one holding any other control character but a tab — in its literal path or its
/// pattern — is not offered raw. (A rule's own text holds no line feed.)
fn shadow_reinclude_lines(negation: &Negation, depth: usize) -> Option<Vec<String>> {
    if negation.rules.iter().any(|rule| {
        !line_can_spell(&rule.pattern)
            || rule.leading.iter().any(|segment| !line_can_spell(segment))
    }) {
        return None;
    }
    // the directories to open, each with how: closed again below it (`/*`), opened to
    // every depth (`/**` + `!/**/`), or re-included whole; a rule opening a directory
    // wider wins over one opening it narrower
    let mut open: Vec<(&[String], Opening)> = Vec::new();
    fn open_dir<'r>(open: &mut Vec<(&'r [String], Opening)>, dir: &'r [String], how: Opening) {
        match open.iter_mut().find(|(d, _)| *d == dir) {
            Some(entry) => entry.1 = entry.1.max(how),
            None => open.push((dir, how)),
        }
    }
    for rule in &negation.rules {
        // with a glob tail every literal segment is a directory the rule reaches into;
        // otherwise the last names the file or directory itself
        let directories = match rule.tail {
            NegationTail::None => rule.leading.len().saturating_sub(1),
            NegationTail::Children | NegationTail::Descendants => rule.leading.len(),
        };
        for len in depth..=directories {
            let how = if rule.tail == NegationTail::Descendants && len == directories {
                Opening::Descendants
            } else {
                Opening::Children
            };
            open_dir(&mut open, &rule.leading[..len], how);
        }
        // a directory-only re-include names a directory the author wants whole
        if rule.tail == NegationTail::None && rule.pattern.ends_with('/') {
            open_dir(&mut open, &rule.leading, Opening::Whole);
        }
    }
    // parents before children: a prefix sorts first
    open.sort();
    let mut lines = Vec::new();
    // the directories opened to every depth or whole, under which no pair is spelled —
    // pushed parents-first, so the NEAREST such ancestor is the last one recorded
    let mut opened_below: Vec<(&[String], Opening)> = Vec::new();
    for (dir, how) in open {
        let above = opened_below
            .iter()
            .rev()
            .find(|(opened, _)| dir.starts_with(opened))
            .map(|(_, how)| *how);
        let segments: Vec<&str> = dir.iter().map(String::as_str).collect();
        let path = pattern_path(&segments);
        match (how, above) {
            // under a whole directory nothing is closed, so nothing needs opening
            (_, Some(Opening::Whole)) => {}
            // under a directory opened to every depth (the only other kind held below),
            // every directory is open already and only a whole one has files to
            // re-include past the `/**` above it
            (Opening::Whole, Some(_)) => {
                lines.push(format!("!/{path}/**"));
                opened_below.push((dir, Opening::Whole));
            }
            (_, Some(_)) => {}
            (Opening::Whole, None) => {
                lines.push(format!("!/{path}/"));
                opened_below.push((dir, Opening::Whole));
            }
            (Opening::Descendants, None) => {
                lines.push(format!("!/{path}/"));
                lines.push(format!("/{path}/**"));
                lines.push(format!("!/{path}/**/"));
                opened_below.push((dir, Opening::Descendants));
            }
            (Opening::Children, None) => {
                lines.push(format!("!/{path}/"));
                lines.push(format!("/{path}/*"));
            }
        }
    }
    for rule in &negation.rules {
        let line = format!("!/{}", rule.pattern);
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    Some(lines)
}

/// How [`shadow_reinclude_lines`] opens one directory on the way down to what the rules
/// name, widest last: its contents closed again with a `/*` so only the rules' own
/// targets come back, opened to every depth with git's `/**` + `!/**/` idiom for a rule
/// reaching below it, or re-included whole for a directory-only rule naming it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Opening {
    Children,
    Descendants,
    Whole,
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
/// `shadow_warning`, so the native CLI and the WASM binding emit the
/// identical text.
#[must_use]
pub fn prettierignore_outside_repo_warning(
    dir: &str,
    in_repo: bool,
    has_prettierignore: bool,
    has_formatignore: bool,
) -> Option<String> {
    (!in_repo && has_prettierignore && !has_formatignore).then(|| {
        let dir = quote_path(dir);
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
#[must_use]
pub fn prettierignore_shadowed_warning(
    dir: &str,
    in_repo: bool,
    has_prettierignore: bool,
    has_formatignore: bool,
) -> Option<String> {
    (in_repo && has_prettierignore && has_formatignore).then(|| {
        let dir = quote_path(dir);
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
#[must_use]
pub fn gitignore_symlink_warning(path: &str) -> String {
    let path = quote_path(path);
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
#[must_use]
pub fn unresolvable_root_error(root: &str) -> String {
    let root = quote_path(root);
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
/// touches one — and it stays quiet whichever of the two rules sits shallower, since the
/// tsv rule is the author's verdict on the file however the path is bounded. Every other
/// exclusion warns — a `.gitignore` rule is about version control, so one excluding a named
/// file is a surprise, and a named directory is a scope someone typed — naming the file
/// whose rule did it and how to undo it. The rule named is the shallowest exclusion's
/// ([`IgnoreStack::exclusion`]): a tsv rule over a `.gitignore` rule at the same prefix,
/// the `.gitignore` rule where it excludes an ancestor above the tsv rule's. The remedy
/// turns on what *else* excludes the path. Only tsv rules: the rule is the user's to
/// narrow or negate. A `.gitignore` rule anywhere at or below the named one: narrowing a
/// tsv rule would leave it standing, and git's parent-directory rule keeps every `!` under
/// an excluded directory inert, so the remedy is the lines that re-include the path level
/// by level from the shallowest exclusion down ([`reinclude_lines`]) — for the repo root's
/// own tsv file, read after every `.gitignore` so its `!` wins, and where a later line wins
/// over a tsv rule in the same file (the warning says "after that rule" when the named rule
/// is one, and "which also override" a root tsv rule below a named `.gitignore` one). A
/// tsv rule in a *deeper* file is read after the root's lines and stays the user's to
/// narrow, so it is named as the second blocker — one warning stating both, not one per
/// rerun — wherever that file was read: none inside a directory a rule excludes is (the
/// walk that prunes the directory reads none), so a rule in one is named only once the
/// directory is re-included and the path named again. The root file named is its `.prettierignore` where that is the file the root
/// reads, since a `.formatignore` created beside it would shadow every rule in it. A path
/// holding a control character no ignore-file line can spell ([`line_can_spell`]) gets no
/// lines, and the warning says to narrow the rule instead.
///
/// `display` is the argument as given; `rel` is the path relative to the format root,
/// `/`-separated; `is_dir` says which kind of argument it is — as the matcher reads it,
/// so a symbolic link is never one, whatever it points at: a directory-only rule (`foo/`)
/// does not match a link for git, and the lines offered re-include the link itself
/// (`!/foo`), which a `!/foo/` would not; `stack` holds the layers
/// from the format root down through the path's parent directory, stopping at a directory
/// a rule excludes — the walk never reads the ignore files inside one, so a rule in one
/// can still exclude the path once that directory is re-included. `loose_root` is the
/// format root's display path outside a git repo, where the format root is the
/// filesystem root and a path is named absolutely; inside one it is `None`, and paths read
/// relative to the repo root. Produced **once**, here, so the native CLI and the WASM
/// binding emit the identical text.
#[must_use]
pub fn excluded_argument_warning(
    display: &str,
    rel: &str,
    is_dir: bool,
    loose_root: Option<&str>,
    stack: &IgnoreStack,
) -> Option<String> {
    let exclusion = stack.exclusion(rel, is_dir)?;
    let tsv = stack.tsv_exclusion(rel, is_dir);
    // a named file a tsv rule excludes is a deliberate opt-out, skipped quietly, whether
    // or not a `.gitignore` excludes it too — and whichever of the two sits shallower.
    // The directory arm below names both blockers where a `.gitignore` above the tsv
    // rule bounds the path on its own, because its warning spells a remedy; this arm
    // spells none, and the tsv rule is the author's verdict on the file however the path
    // is bounded. A pre-commit hook names such a file on every commit that stages it, so
    // a warning keyed on the `.gitignore` above it would be exactly that noise
    if !is_dir && tsv.is_some() {
        return None;
    }
    // The rule named is the shallowest exclusion's witness — a tsv rule over a
    // `.gitignore` rule at the same prefix (`exclusion` reads the tsv layers last), the
    // `.gitignore` rule where it excludes an ancestor above the tsv rule's. The remedy
    // then depends on what ELSE excludes the path: only where nothing but tsv rules do is
    // "narrow or negate that rule" enough, since narrowing a tsv rule leaves any
    // `.gitignore` rule at or below it standing, and no `!` under an excluded directory
    // can reach the path (git's parent-directory rule). Everywhere a `.gitignore` rule is
    // involved the remedy is the lines that re-include the path level by level from the
    // shallowest exclusion down, for the repo root's own tsv file: read after every
    // `.gitignore`, so its `!` wins there, and a later line wins over a tsv rule in the
    // same file — a tsv rule in a DEEPER file is read after it and stays the user's to
    // narrow, so it is named as the second blocker.
    let named = exclusion;
    let by_gitignore = named.source == IgnoreSource::Gitignore;
    // a `.gitignore` rule ABOVE the named one is no blocker: some tsv `!` already
    // re-includes what it excluded, or the shallowest exclusion would sit there (the
    // re-include idiom itself — `!/dist/`, `/dist/*` — names `dist/sub` this way)
    let gitignore = stack
        .gitignore_exclusion(rel, is_dir)
        .filter(|git| git.depth >= named.depth);
    // a `.gitignore` is read only inside a repo, so a witness in one means the format
    // root is the repo root — what "the repo-root <file>" names, in the remedy below and
    // in `ignore_file_display`
    debug_assert!(
        gitignore.is_none() || loose_root.is_none(),
        "a .gitignore rule excluded {rel} outside a repo"
    );
    let segments = tsv_ignore::split_segments(rel);
    let consequence = if is_dir {
        "so nothing under it is formatted"
    } else {
        "so it is not formatted"
    };
    let rule_file = ignore_file_display(&segments[..named.anchor_depth], named.source, loose_root);
    let root_file = stack
        .tsv_layer_source("")
        .unwrap_or(IgnoreSource::Formatignore)
        .file_name();
    // what a `.gitignore` or a deeper tsv rule excludes, as a clause names it
    let excluded_by = |depth: usize| {
        if depth == segments.len() {
            "it".to_string()
        } else {
            path_display(&segments[..depth], loose_root)
        }
    };
    // the re-include lines from the shallowest exclusion down, as "adding …, in that
    // order, to the repo-root <file>"; `None` where no line can spell the path
    let adding = reinclude_lines(&segments, named.depth, is_dir).map(|lines| {
        let order = if lines.len() > 1 {
            ", in that order,"
        } else {
            ""
        };
        format!(
            "adding {}{order} to the repo-root {root_file}",
            quoted_list(&lines)
        )
    });
    let unspellable = || format!("{UNSPELLABLE_REASON}, so narrow that rule to format it");
    let remedy = match (by_gitignore, tsv, gitignore) {
        // the `.gitignore` rule is the shallowest; a tsv rule may also exclude the path
        // below it, overridden by the lines in the root's file or standing in a deeper one
        (true, tsv, _) => match adding {
            None => unspellable(),
            Some(adding) => {
                let mut remedy = format!("re-include it by {adding}");
                if let Some(tsv) = tsv {
                    let tsv_file =
                        ignore_file_display(&segments[..tsv.anchor_depth], tsv.source, loose_root);
                    let excluded = excluded_by(tsv.depth);
                    if tsv.anchor_depth == 0 {
                        let _ = write!(
                            remedy,
                            ", which also override the rule in {tsv_file} that excludes {excluded}"
                        );
                    } else {
                        let _ = write!(
                            remedy,
                            ", and narrow or negate the rule in {tsv_file} that excludes {excluded} too"
                        );
                    }
                }
                remedy
            }
        },
        // the tsv rule named is the shallowest and a `.gitignore` rule stands at or below
        // it: narrowing the tsv rule alone leaves the path excluded
        (false, Some(tsv), Some(git)) => {
            let git_file =
                ignore_file_display(&segments[..git.anchor_depth], git.source, loose_root);
            let excluded = excluded_by(git.depth);
            match adding {
                None => unspellable(),
                Some(adding) if tsv.anchor_depth == 0 => format!(
                    "re-include it by {adding} after that rule — a rule in {git_file} excludes {excluded} too, so narrowing that rule alone does not admit it"
                ),
                Some(adding) => format!(
                    "narrow or negate that rule, and re-include it past the rule in {git_file} that excludes {excluded} by {adding}"
                ),
            }
        }
        // only tsv rules exclude it: the rule named is the user's own to narrow (a tsv
        // witness with no tsv exclusion cannot happen — the witness IS a tsv rule)
        (false, _, _) => "narrow or negate that rule to format it".to_string(),
    };
    let display = quote_path(display);
    Some(if named.depth == segments.len() {
        format!("{display} is excluded by a rule in {rule_file}, {consequence}; {remedy}")
    } else {
        let dir = path_display(&segments[..named.depth], loose_root);
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
///
/// `None` when no line can spell the path ([`line_can_spell`]): a segment holding a
/// control character other than a tab — a line feed splits every line spelling it, a
/// carriage return ending the file's own line is stripped with the line ending, and the
/// rest are not offered raw.
fn reinclude_lines(segments: &[&str], depth: usize, is_dir: bool) -> Option<Vec<String>> {
    if segments.iter().any(|segment| !line_can_spell(segment)) {
        return None;
    }
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
    Some(lines)
}

/// Path segments `/`-joined as a literal ignore-file pattern: in each, the glob
/// metacharacters `*`, `?`, `[` and `]` and the escape `\` are backslash-escaped, and so
/// are trailing spaces, which gitignore(5) trims from a line — so a `[slug]` route
/// directory names itself rather than a one-character class. A leading `#` or `!` needs
/// no escape: every line spells its path after a `/`.
///
/// No escape spells a control character — a line feed and a trailing carriage return
/// are not a line's to hold, and the rest a warning will not print raw — so the callers
/// keep every one but a tab out of a line ([`line_can_spell`]): [`reinclude_lines`]
/// declines such a path, [`shadow_reinclude_lines`] such a rule.
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
/// the working directory — and quoted as every printed path is ([`quote_path`]). The separator is read off `loose_root`'s own spelling — a
/// backslash anywhere in it means a Windows root — which is sound only because every
/// caller passes the FILESYSTEM root (`/`, `C:\`, a `\\?\` or UNC prefix), never a
/// directory whose name could hold a backslash on posix; asserted in debug builds.
fn path_display(segments: &[&str], loose_root: Option<&str>) -> String {
    let Some(root) = loose_root else {
        return quote_path_owned(segments.join("/"));
    };
    let trimmed = root.trim_end_matches(['/', '\\']);
    debug_assert!(
        trimmed.is_empty() || trimmed.ends_with(':') || root.starts_with("\\\\"),
        "loose_root is the filesystem root, never a directory: {root:?}"
    );
    let separator = if root.contains('\\') { '\\' } else { '/' };
    let mut display = trimmed.to_string();
    for segment in segments {
        display.push(separator);
        display.push_str(segment);
    }
    quote_path_owned(display)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{stack_from, tsv_stack};
    use crate::*;
    use tsv_ignore::IgnoreStack;

    #[test]
    fn shadow_warning_text_is_stable() {
        // pinned verbatim — the native CLI and the WASM binding both fetch it, so both
        // surfaces emit this exact string. The lines are for the file the re-include
        // was written in, anchored and relative to its directory
        let stack = tsv_stack("!dist/keep.ts\n");
        assert_eq!(
            shadow_warning("dist", None, &stack).unwrap(),
            "dist is skipped by tsv's build-output heuristic, so a re-include under it in the repo-root .formatignore does nothing; re-include it by adding `!/dist/`, `/dist/*` and `!/dist/keep.ts`, in that order, to the repo-root .formatignore"
        );
        let mut stack = IgnoreStack::new();
        stack.push_prettierignore("pkg", "!dist/keep.ts\n");
        assert_eq!(
            shadow_warning("pkg/dist", None, &stack).unwrap(),
            "pkg/dist is skipped by tsv's build-output heuristic, so a re-include under it in pkg/.prettierignore does nothing; re-include it by adding `!/dist/`, `/dist/*` and `!/dist/keep.ts`, in that order, to pkg/.prettierignore"
        );
        // outside a repo the format root is the filesystem root: both paths read
        // absolutely, while the lines stay relative to the rule's file
        let mut stack = IgnoreStack::new();
        stack.push_formatignore("home/u/proj", "!src/build/keep.ts\n");
        assert_eq!(
            shadow_warning("home/u/proj/src/build", Some("/"), &stack).unwrap(),
            "/home/u/proj/src/build is skipped by tsv's build-output heuristic, so a re-include under it in /home/u/proj/.formatignore does nothing; re-include it by adding `!/src/build/`, `/src/build/*` and `!/src/build/keep.ts`, in that order, to /home/u/proj/.formatignore"
        );
        // no re-include written under the directory: nothing to say
        assert_eq!(shadow_warning("dist", None, &IgnoreStack::new()), None);
    }

    #[test]
    fn shadow_warning_lines_readmit_the_directory_alone() {
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
            // a tab is the one control character a line spells, as on the argument warning
            ("", ".c\t", "!.c\t/keep.ts\n"),
            // an explicitly anchored rule, and one whose target sits a level down: the
            // ladder must open every directory between the pruned one and the file
            ("", "dist", "!/dist/keep.ts\n"),
            ("", "dist", "!dist/sub/keep.ts\n"),
            ("pkg", "dist", "!dist/sub/deeper/keep.ts\n"),
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
            let warning = shadow_warning(&d, None, &stack).unwrap();
            // the backticked spans are the lines, in order
            let lines: Vec<String> = warning
                .split('`')
                .skip(1)
                .step_by(2)
                .map(String::from)
                .collect();
            let mut fixed = IgnoreStack::new();
            fixed.push_formatignore(anchor, &format!("{rule}{}\n", lines.join("\n")));
            assert_eq!(
                classify_dir(name, &d, true, &fixed),
                DirVerdict::Descend,
                "{d}: {lines:?}"
            );
            // the file the rule names, at whatever depth under the pruned directory
            let keep = rule
                .trim_start_matches('!')
                .trim_start_matches('/')
                .trim_end()
                .replace('\\', "");
            let keep_rel = under(&keep);
            assert!(
                should_format_file("keep.ts", &keep_rel, &fixed),
                "{d}: {lines:?}"
            );
            // every directory the ladder opened on the way is walked, and nothing beside
            // the file is in scope — not a sibling file, not a sibling directory
            let mut opened = String::new();
            for seg in keep_rel
                .rsplit_once('/')
                .map_or("", |(dirs, _)| dirs)
                .split('/')
            {
                if !opened.is_empty() {
                    opened.push('/');
                }
                opened.push_str(seg);
                if opened.len() > d.len() {
                    assert_eq!(
                        classify_dir(seg, &opened, true, &fixed),
                        DirVerdict::Descend,
                        "{d}: {lines:?} at {opened}"
                    );
                }
            }
            assert!(
                !should_format_file("other.ts", &under(&format!("{dir}/other.ts")), &fixed),
                "{d}: {lines:?}"
            );
            assert_eq!(
                classify_dir("sibling", &under(&format!("{dir}/sibling")), true, &fixed),
                DirVerdict::Prune,
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

    /// Every builder that names a path prints it through `quote_path`; the ignore-file
    /// PATTERNS a warning offers stay literal, since they are pasted rather than read.
    #[test]
    fn every_path_a_message_names_is_quoted() {
        assert_eq!(
            unsupported_extension_error("a\nb.json").unwrap(),
            "\"a\\nb.json\": unsupported file extension (tsv handles .ts, .mts, .cts, .js, .mjs, .cjs, .svelte, .css)"
        );
        assert!(
            gitignore_symlink_warning("d\n/.gitignore")
                .starts_with("\"d\\n/.gitignore\" is a symbolic link")
        );
        assert!(unresolvable_root_error("e\"f").starts_with("\"e\\\"f\": cannot resolve"));
        assert!(
            prettierignore_outside_repo_warning("/tmp/x\ty", false, true, false)
                .unwrap()
                .starts_with(".prettierignore in \"/tmp/x\\ty\" is not read")
        );
        assert!(
            prettierignore_shadowed_warning("pkg\u{1b}", true, true, true)
                .unwrap()
                .starts_with(".prettierignore in \"pkg\\033\" is shadowed")
        );
        // a directory in the path, the ignore file named by its directory, and the
        // re-include lines: the first two quoted, the lines spelled literally (a tab is a
        // character a pattern can hold, so it stays a tab there)
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "bu\tild/\n");
        stack.push_gitignore("we\"ird", "out/\n");
        assert_eq!(
            excluded_argument_warning("./bu\tild/x.ts", "bu\tild/x.ts", false, None, &stack)
                .as_deref(),
            Some(
                "\"./bu\\tild/x.ts\" is inside \"bu\\tild\", which a rule in the repo-root .gitignore excludes, so it is not formatted; re-include it by adding `!/bu\tild/`, `/bu\tild/*` and `!/bu\tild/x.ts`, in that order, to the repo-root .formatignore"
            )
        );
        assert_eq!(
            excluded_argument_warning("we\"ird/out", "we\"ird/out", true, None, &stack).as_deref(),
            Some(
                "\"we\\\"ird/out\" is excluded by a rule in \"we\\\"ird/.gitignore\", so nothing under it is formatted; re-include it by adding `!/we\"ird/out/` to the repo-root .formatignore"
            )
        );
        // outside a repo the absolute spelling is what gets quoted
        let mut loose = IgnoreStack::new();
        loose.push_formatignore("home/u", "gen*/\n");
        assert_eq!(
            excluded_argument_warning("gen\r", "home/u/gen\r", true, Some("/"), &loose).as_deref(),
            Some(
                "\"gen\\r\" is excluded by a rule in /home/u/.formatignore, so nothing under it is formatted; narrow or negate that rule to format it"
            )
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
        // a path no ignore-file line can spell gets no lines — and is quoted, so the warning
        // stays one line
        assert_eq!(
            warn("build/a\nb.ts", "build/a\nb.ts", false).as_deref(),
            Some(
                "\"build/a\\nb.ts\" is inside build, which a rule in the repo-root .gitignore excludes, so it is not formatted; no ignore-file line can spell the control character in its path, so narrow that rule to format it"
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
        stack.push_gitignore("", "vendor/\nsame/\n");
        stack.push_gitignore("pkg", "out/\n");
        stack.push_formatignore("", "vendor/x.ts\nvendor/lib/\nsame/\n");
        stack.push_formatignore("pkg", "out/*.ts\n");
        let warn = |rel, is_dir| excluded_argument_warning(rel, rel, is_dir, None, &stack);
        assert_eq!(warn("vendor/x.ts", false), None);
        assert_eq!(warn("pkg/out/a.ts", false), None);
        // a named file a tsv rule excludes only through an ancestor is quiet too
        assert_eq!(warn("vendor/lib/z.ts", false), None);
        // the `.gitignore` excludes `vendor` ABOVE the tsv rule's `vendor/lib`: narrowing
        // the tsv rule alone leaves `vendor` excluded and every `!` under it inert, so the
        // warning undoes the `.gitignore` exclusion and names the tsv rule as the second
        // blocker — one warning, not a second one after the first remedy is followed
        assert_eq!(
            warn("vendor/lib", true).as_deref(),
            Some(
                "vendor/lib is inside vendor, which a rule in the repo-root .gitignore excludes, so nothing under it is formatted; re-include it by adding `!/vendor/`, `/vendor/*` and `!/vendor/lib/`, in that order, to the repo-root .formatignore, which also override the rule in the repo-root .formatignore that excludes it"
            )
        );
        // a tsv rule at the `.gitignore` rule's own depth is the one named (a `!` in its
        // file re-includes past both), but NARROWING it alone leaves the `.gitignore`
        // rule standing, so the remedy is the lines, after that rule
        assert_eq!(
            warn("same", true).as_deref(),
            Some(
                "same is excluded by a rule in the repo-root .formatignore, so nothing under it is formatted; re-include it by adding `!/same/` to the repo-root .formatignore after that rule — a rule in the repo-root .gitignore excludes it too, so narrowing that rule alone does not admit it"
            )
        );
        // a tsv rule excluding a directory BETWEEN the `.gitignore`'d ancestor and the
        // path names that directory as what it excludes
        assert!(warn("vendor/lib/deep", true).is_some_and(|warning| warning.ends_with(
            "to the repo-root .formatignore, which also override the rule in the repo-root .formatignore that excludes vendor/lib"
        )));
        // a tsv rule in a DEEPER file is not overridden by lines in the root's, so it
        // stays the user's to narrow, and is named as the second blocker
        let mut deeper = IgnoreStack::new();
        deeper.push_gitignore("", "pkg/vendor/\n");
        deeper.push_formatignore("", "");
        deeper.push_formatignore("pkg", "vendor/lib/\n");
        assert!(
            excluded_argument_warning("pkg/vendor/lib", "pkg/vendor/lib", true, None, &deeper)
                .is_some_and(|warning| warning.ends_with(
                    "to the repo-root .formatignore, and narrow or negate the rule in pkg/.formatignore that excludes it too"
                ))
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
            // a tab is the one control character a line spells (raw), as `n\to.ts` pins
            // on both CLIs; every other declines (`..._cannot_spell_the_path`)
            (
                "build/t\tab.ts",
                false,
                &["build/tab.ts", "build/t ab.ts", "build/t\tab/z.ts"],
            ),
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
    fn excluded_argument_warning_offers_no_line_that_cannot_spell_the_path() {
        // a control character other than a tab gets no re-include lines: a line feed splits
        // any line spelling it, a carriage return ending a file's own line is stripped with
        // the line ending, and every other one an ignore file could hold only raw — the very
        // byte the warning's own path quoting exists to keep off the terminal (the whole text
        // is pinned in `excluded_argument_warning_text_is_stable`)
        let mut stack = IgnoreStack::new();
        stack.push_gitignore("", "build/\n*.gen.ts\n");
        for (rel, is_dir) in [
            ("build/a\nb.ts", false),
            ("build/a\nb", true),
            ("build/a\nb/c.ts", false),
            ("src/a\nb.gen.ts", false),
            ("build/z.ts\r", false),
            ("build/d\re/f.ts", false),
            ("build/d\r", true),
            ("build/g\u{1b}h.ts", false),
            ("build/i\u{7f}j", true),
            ("build/k\u{1}l/m.ts", false),
            ("build/n\u{0}o.ts", false),
        ] {
            let warning = excluded_argument_warning(rel, rel, is_dir, None, &stack)
                .unwrap_or_else(|| panic!("{rel:?}: no warning"));
            assert!(
                warning.ends_with(
                    "; no ignore-file line can spell the control character in its path, so narrow that rule to format it"
                ),
                "{rel:?}: {warning}"
            );
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

    #[test]
    fn shadow_warning_names_the_rule_that_excludes_the_directory() {
        // pinned verbatim, like the heuristic arm: a re-include written under a directory
        // a RULE excludes is exactly as inert (git's parent-directory rule), and the walk
        // says so, naming the excluding rule's file as the cause. The lines go to the
        // re-include's own file: a tsv layer is read after every `.gitignore`, and a later
        // line wins in the same file
        let stack = stack_from(&[("", "dist/\n")], &[("", "!dist/keep.ts\n")]);
        assert_eq!(
            classify_dir("dist", "dist", false, &stack),
            DirVerdict::PruneWithWarning
        );
        assert_eq!(
            shadow_warning("dist", None, &stack).unwrap(),
            "dist is excluded by a rule in the repo-root .gitignore, so a re-include under it in the repo-root .formatignore does nothing; re-include it by adding `!/dist/`, `/dist/*` and `!/dist/keep.ts`, in that order, to the repo-root .formatignore"
        );
        assert_eq!(
            path_shadow_warning("dist/keep.ts", None, &stack),
            shadow_warning("dist", None, &stack)
        );
        // the excluding rule in the re-include's own file
        let stack = stack_from(&[], &[("", "dist/\n!dist/keep.ts\n")]);
        assert_eq!(
            shadow_warning("dist", None, &stack).unwrap(),
            "dist is excluded by a rule in the repo-root .formatignore, so a re-include under it in the repo-root .formatignore does nothing; re-include it by adding `!/dist/`, `/dist/*` and `!/dist/keep.ts`, in that order, to the repo-root .formatignore"
        );
        // a tsv rule in a DEEPER file than the re-include's is read after it, so no line
        // added there can override it: that rule is the one to narrow
        let stack = stack_from(&[], &[("", "!pkg/dist/keep.ts\n"), ("pkg", "dist/\n")]);
        assert_eq!(
            classify_dir("dist", "pkg/dist", false, &stack),
            DirVerdict::PruneWithWarning
        );
        assert_eq!(
            shadow_warning("pkg/dist", None, &stack).unwrap(),
            "pkg/dist is excluded by a rule in pkg/.formatignore, so a re-include under it in the repo-root .formatignore does nothing; narrow or negate that rule to format it"
        );
        // a glob tail past the pruned directory is as inert, and the last line is the
        // author's own pattern, anchored
        let stack = stack_from(&[("", "dist/\n")], &[("", "!dist/*.ts\n")]);
        assert_eq!(
            shadow_warning("dist", None, &stack).unwrap(),
            "dist is excluded by a rule in the repo-root .gitignore, so a re-include under it in the repo-root .formatignore does nothing; re-include it by adding `!/dist/`, `/dist/*` and `!/dist/*.ts`, in that order, to the repo-root .formatignore"
        );
        // no re-include under it: a plain prune, and nothing to say
        let stack = stack_from(&[("", "dist/\n")], &[]);
        assert_eq!(
            classify_dir("dist", "dist", false, &stack),
            DirVerdict::Prune
        );
        assert_eq!(shadow_warning("dist", None, &stack), None);
    }

    #[test]
    fn shadow_warning_lines_readmit_past_the_excluding_rule() {
        // applied where the warning says, the lines put the file back in scope under each
        // kind of excluding rule — and, where the warning says to narrow instead, dropping
        // the deeper rule is what admits it
        for (gitignores, tsvs, dir, file) in [
            (
                vec![("", "dist/\n")],
                vec![("", "!dist/keep.ts\n")],
                "dist",
                "dist/keep.ts",
            ),
            (
                vec![],
                vec![("", "dist/\n!dist/keep.ts\n")],
                "dist",
                "dist/keep.ts",
            ),
            (
                vec![("", "dist/\n")],
                vec![("", "!dist/sub/keep.ts\n")],
                "dist",
                "dist/sub/keep.ts",
            ),
            (
                vec![("", "dist/\n")],
                vec![("", "!dist/*.ts\n")],
                "dist",
                "dist/keep.ts",
            ),
        ] {
            let stack = stack_from(&gitignores, &tsvs);
            let warning = shadow_warning(dir, None, &stack).unwrap();
            let lines: Vec<&str> = warning.split('`').skip(1).step_by(2).collect();
            let (anchor, rule) = tsvs[0];
            let mut fixed = IgnoreStack::new();
            for (a, content) in &gitignores {
                fixed.push_gitignore(a, content);
            }
            fixed.push_formatignore(anchor, &format!("{rule}{}\n", lines.join("\n")));
            assert!(!fixed.is_ignored(file, false), "{file}: {lines:?}");
            assert!(
                should_format_file("keep.ts", file, &fixed),
                "{file}: {lines:?}"
            );
            // nothing beside what the rule reaches: a sibling directory stays pruned, and
            // a sibling file too unless the rule's own glob admits it
            assert_eq!(
                classify_dir("sibling", "dist/sibling", false, &fixed),
                DirVerdict::Prune,
                "{lines:?}"
            );
            if !rule.contains('*') {
                assert!(
                    !should_format_file("other.ts", "dist/other.ts", &fixed),
                    "{lines:?}"
                );
            }
        }
    }

    #[test]
    fn excluded_argument_warning_lines_readmit_past_a_gitignore_rule_behind_the_tsv_rule() {
        // the tsv rule is the shallowest exclusion and a `.gitignore` rule stands at or
        // below it: the lines go after the tsv rule in the root's file, and re-include
        // past both — one round, where "narrow or negate" took two
        for (git, tsv, rel) in [
            ("same/\n", "same/\n", "same"),
            ("vendor/lib/\n", "vendor/\n", "vendor/lib"),
        ] {
            let stack = stack_from(&[("", git)], &[("", tsv)]);
            let warning = excluded_argument_warning(rel, rel, true, None, &stack).unwrap();
            assert!(warning.contains("after that rule"), "{warning}");
            assert!(
                warning.contains("a rule in the repo-root .gitignore excludes"),
                "{warning}"
            );
            let lines: Vec<&str> = warning.split('`').skip(1).step_by(2).collect();
            let mut fixed = IgnoreStack::new();
            fixed.push_gitignore("", git);
            fixed.push_formatignore("", &format!("{tsv}{}\n", lines.join("\n")));
            assert!(!fixed.is_ignored(rel, true), "{rel}: {lines:?}");
            assert!(
                fixed.is_ignored("vendor/other", true) || rel == "same",
                "{lines:?}"
            );
        }
        // the tsv rule in a deeper file: it is narrowed, and the root's lines re-include
        // past the `.gitignore`
        let stack = stack_from(
            &[("", "pkg/vendor/lib/\n")],
            &[("", ""), ("pkg", "vendor/\n")],
        );
        let warning =
            excluded_argument_warning("pkg/vendor/lib", "pkg/vendor/lib", true, None, &stack)
                .unwrap();
        assert_eq!(
            warning,
            "pkg/vendor/lib is inside pkg/vendor, which a rule in pkg/.formatignore excludes, so nothing under it is formatted; narrow or negate that rule, and re-include it past the rule in the repo-root .gitignore that excludes it by adding `!/pkg/vendor/`, `/pkg/vendor/*` and `!/pkg/vendor/lib/`, in that order, to the repo-root .formatignore"
        );
        let lines: Vec<&str> = warning.split('`').skip(1).step_by(2).collect();
        let mut fixed = IgnoreStack::new();
        fixed.push_gitignore("", "pkg/vendor/lib/\n");
        fixed.push_formatignore("", &lines.join("\n"));
        fixed.push_formatignore("pkg", "");
        assert!(!fixed.is_ignored("pkg/vendor/lib", true));
        // a tsv-only exclusion keeps the short remedy
        let stack = stack_from(&[], &[("", "vendor/\n")]);
        assert!(
            excluded_argument_warning("vendor", "vendor", true, None, &stack)
                .unwrap()
                .ends_with("narrow or negate that rule to format it")
        );
        // and so does a `.gitignore` rule ABOVE the named tsv rule: the re-include idiom
        // applied and a directory under it named — `!/dist/` already passes the
        // `.gitignore`'s `dist/`, so narrowing the `/dist/*` that excludes `dist/sub` is
        // all it takes, and the warning must not claim otherwise
        let stack = stack_from(
            &[("", "dist/\n")],
            &[("", "!/dist/\n/dist/*\n!/dist/keep.ts\n")],
        );
        assert_eq!(
            excluded_argument_warning("dist/sub", "dist/sub", true, None, &stack).unwrap(),
            "dist/sub is excluded by a rule in the repo-root .formatignore, so nothing under it is formatted; narrow or negate that rule to format it"
        );
        let narrowed = stack_from(&[("", "dist/\n")], &[("", "!/dist/\n!/dist/keep.ts\n")]);
        assert!(!narrowed.is_ignored("dist/sub", true));
    }

    #[test]
    fn shadow_warning_lines_open_the_subtree_a_tail_reaches_into() {
        // a rule reaching BELOW its deepest literal directory (`!dist/**/keep.ts`,
        // `!dist/*/keep.ts`) gets git's every-directory-no-file idiom for that directory —
        // `/dist/**` then `!/dist/**/` — since a `/dist/*` would close the directories it
        // reaches into and the file would stay out of scope with the warning gone;
        // applied, exactly what the rule names is in scope, at whatever depth
        for (rule, in_scope, out_of_scope) in [
            (
                "!dist/**/keep.ts\n",
                &["dist/keep.ts", "dist/sub/keep.ts", "dist/sub/deep/keep.ts"][..],
                &["dist/other.ts", "dist/sub/other.ts"][..],
            ),
            (
                "!dist/*/keep.ts\n",
                &["dist/sub/keep.ts"][..],
                &["dist/keep.ts", "dist/sub/deep/keep.ts", "dist/sub/other.ts"][..],
            ),
            (
                "!dist/sub/**\n",
                &["dist/sub/a.ts", "dist/sub/deep/b.ts"][..],
                &["dist/a.ts", "dist/other/c.ts"][..],
            ),
        ] {
            let stack = stack_from(&[("", "dist/\n")], &[("", rule)]);
            let warning = shadow_warning("dist", None, &stack).unwrap();
            let lines: Vec<&str> = warning.split('`').skip(1).step_by(2).collect();
            let mut fixed = IgnoreStack::new();
            fixed.push_gitignore("", "dist/\n");
            fixed.push_formatignore("", &format!("{rule}{}\n", lines.join("\n")));
            for path in in_scope {
                assert!(!fixed.is_ignored(path, false), "{rule}: {path} {lines:?}");
            }
            for path in out_of_scope {
                assert!(fixed.is_ignored(path, false), "{rule}: {path} {lines:?}");
            }
        }
        let stack = stack_from(&[("", "dist/\n")], &[("", "!dist/**/keep.ts\n")]);
        assert_eq!(
            shadow_warning("dist", None, &stack).unwrap(),
            "dist is excluded by a rule in the repo-root .gitignore, so a re-include under it in the repo-root .formatignore does nothing; re-include it by adding `!/dist/`, `/dist/**`, `!/dist/**/` and `!/dist/**/keep.ts`, in that order, to the repo-root .formatignore"
        );
    }

    #[test]
    fn shadow_warning_lines_respell_every_reinclude_under_the_directory() {
        // the `/dist/*` line silences every re-include written before it, so each one is
        // re-spelled after it — else applying the lines for one rule would put another's
        // file out of scope for good, with the warning gone. The directory pairs are the
        // union of what the rules reach into, parents first
        let stack = stack_from(
            &[("", "dist/\n")],
            &[("", "!dist/*.log\n!dist/keep.ts\n!dist/sub/x.ts\n")],
        );
        let warning = shadow_warning("dist", None, &stack).unwrap();
        assert_eq!(
            warning,
            "dist is excluded by a rule in the repo-root .gitignore, so a re-include under it in the repo-root .formatignore does nothing; re-include it by adding `!/dist/`, `/dist/*`, `!/dist/sub/`, `/dist/sub/*`, `!/dist/*.log`, `!/dist/keep.ts` and `!/dist/sub/x.ts`, in that order, to the repo-root .formatignore"
        );
        let lines: Vec<&str> = warning.split('`').skip(1).step_by(2).collect();
        let mut fixed = IgnoreStack::new();
        fixed.push_gitignore("", "dist/\n");
        fixed.push_formatignore(
            "",
            &format!(
                "!dist/*.log\n!dist/keep.ts\n!dist/sub/x.ts\n{}\n",
                lines.join("\n")
            ),
        );
        assert!(!fixed.is_ignored("dist/keep.ts", false));
        assert!(!fixed.is_ignored("dist/sub/x.ts", false));
        assert!(!fixed.is_ignored("dist/a.log", false));
        assert!(fixed.is_ignored("dist/other.ts", false));
        assert!(fixed.is_ignored("dist/sub/other.ts", false));
        // across files: the root's `!pkg/dist/a.ts` is re-spelled for pkg/.formatignore,
        // the deepest file holding one, whose lines are read after the root's rule
        let stack = stack_from(
            &[("pkg", "dist/\n")],
            &[("", "!pkg/dist/a.ts\n"), ("pkg", "!dist/b.ts\n")],
        );
        let warning = shadow_warning("pkg/dist", None, &stack).unwrap();
        assert_eq!(
            warning,
            "pkg/dist is excluded by a rule in pkg/.gitignore, so a re-include under it in pkg/.formatignore does nothing; re-include it by adding `!/dist/`, `/dist/*`, `!/dist/a.ts` and `!/dist/b.ts`, in that order, to pkg/.formatignore"
        );
        let lines: Vec<&str> = warning.split('`').skip(1).step_by(2).collect();
        let mut fixed = IgnoreStack::new();
        fixed.push_gitignore("pkg", "dist/\n");
        fixed.push_formatignore("", "!pkg/dist/a.ts\n");
        fixed.push_formatignore("pkg", &format!("!dist/b.ts\n{}\n", lines.join("\n")));
        assert!(!fixed.is_ignored("pkg/dist/a.ts", false));
        assert!(!fixed.is_ignored("pkg/dist/b.ts", false));
        assert!(fixed.is_ignored("pkg/dist/c.ts", false));
    }

    /// The property the transcript tests above are instances of: applying the offered
    /// lines puts every rule's own target back in scope and nothing no rule names — for
    /// every combination of rule shapes under one pruned directory, not the hand-picked
    /// few — and every ladder line is load-bearing: without it a target is lost, an
    /// unnamed path admitted, or the directory left pruned. The shapes nest each way a
    /// directory can be opened under another — a whole directory under an every-depth
    /// one, an every-depth one under a whole one, a whole one under a whole one — so the
    /// two properties together hold the ladder to what a second rule under the same
    /// directory must not close and what the nearest opened ancestor makes redundant.
    #[test]
    fn shadow_warning_lines_readmit_every_rules_target_for_every_shape_combination() {
        // each shape with the paths it names, which must be in scope once the lines are
        // applied, and the probes it admits beside them (a whole directory admits
        // everything under it); a probe no chosen shape admits must stay out
        let shapes: [(&str, &[&str], &[&str]); 9] = [
            ("!dist/keep.ts", &["dist/keep.ts"], &[]),
            (
                "!dist/sub/",
                &["dist/sub/inside.ts", "dist/sub/deep/inside.ts"],
                &["dist/sub/nope.js", "dist/sub/deep/nope.js"],
            ),
            ("!dist/sub/keep.ts", &["dist/sub/keep.ts"], &[]),
            ("!dist/*.ts", &["dist/any.ts"], &[]),
            // `zzz` is a directory no other shape opens, so these two name a target
            // only their own every-directory line reaches
            (
                "!dist/**/keep.ts",
                &[
                    "dist/keep.ts",
                    "dist/sub/keep.ts",
                    "dist/sub/deep/keep.ts",
                    "dist/zzz/keep.ts",
                ],
                &[],
            ),
            (
                "!dist/*/keep.ts",
                &["dist/sub/keep.ts", "dist/zzz/keep.ts"],
                &[],
            ),
            (
                "!dist/sub/deep/",
                &["dist/sub/deep/inside.ts"],
                &["dist/sub/deep/nope.js"],
            ),
            ("!dist/sub/deep/x.ts", &["dist/sub/deep/x.ts"], &[]),
            // reaches below `sub`, the directory the second shape re-includes whole —
            // into `sub/zzz` too, which only its own every-directory line opens
            (
                "!dist/sub/**/k3.ts",
                &[
                    "dist/sub/k3.ts",
                    "dist/sub/deep/k3.ts",
                    "dist/sub/zzz/k3.ts",
                ],
                &[],
            ),
        ];
        let probes = [
            "dist/nope.js",
            "dist/sub/nope.js",
            "dist/sub/deep/nope.js",
            "dist/zzz/nope.js",
        ];
        let mut combinations = 0;
        for mask in 1u32..(1 << shapes.len()) {
            if mask.count_ones() > 3 {
                continue;
            }
            let chosen: Vec<&(&str, &[&str], &[&str])> = shapes
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, shape)| shape)
                .collect();
            let rules: String = chosen
                .iter()
                .flat_map(|(rule, _, _)| [*rule, "\n"])
                .collect();
            // the rules' own patterns, re-spelled after the ladder by design; every
            // other line is the ladder's, and each of those must be load-bearing
            let patterns: Vec<String> = chosen
                .iter()
                .map(|(rule, _, _)| format!("!/{}", &rule[1..]))
                .collect();
            let out: Vec<&str> = probes
                .iter()
                .copied()
                .filter(|probe| !chosen.iter().any(|(_, _, admits)| admits.contains(probe)))
                .collect();
            for gitignored in [false, true] {
                let mut stack = IgnoreStack::new();
                if gitignored {
                    stack.push_gitignore("", "dist/\n");
                }
                stack.push_formatignore("", &rules);
                let warning = shadow_warning("dist", None, &stack)
                    .unwrap_or_else(|| panic!("{rules:?}: no warning"));
                let lines: Vec<&str> = warning.split('`').skip(1).step_by(2).collect();
                let apply = |lines: &[&str]| {
                    let mut fixed = IgnoreStack::new();
                    if gitignored {
                        fixed.push_gitignore("", "dist/\n");
                    }
                    fixed.push_formatignore("", &format!("{rules}{}\n", lines.join("\n")));
                    fixed
                };
                let fixed = apply(&lines);
                assert_eq!(
                    classify_dir("dist", "dist", !gitignored, &fixed),
                    DirVerdict::Descend,
                    "{rules:?}: {lines:?}"
                );
                for (rule, targets, _) in &chosen {
                    for target in *targets {
                        assert!(
                            !fixed.is_ignored(target, false),
                            "{rules:?}: {rule} names {target}, out of scope after {lines:?}"
                        );
                    }
                }
                for probe in &out {
                    assert!(
                        fixed.is_ignored(probe, false),
                        "{rules:?}: {lines:?} admit {probe}, which no rule names"
                    );
                }
                // every ladder line is load-bearing: without it, the lines no longer
                // do all three of the above
                let readmits = |fixed: &IgnoreStack| {
                    classify_dir("dist", "dist", !gitignored, fixed) == DirVerdict::Descend
                        && chosen.iter().all(|(_, targets, _)| {
                            targets
                                .iter()
                                .all(|target| !fixed.is_ignored(target, false))
                        })
                        && out.iter().all(|probe| fixed.is_ignored(probe, false))
                };
                for (i, line) in lines.iter().enumerate() {
                    if patterns.iter().any(|pattern| pattern == line) {
                        continue;
                    }
                    let without: Vec<&str> = lines
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, line)| *line)
                        .collect();
                    assert!(
                        !readmits(&apply(&without)),
                        "{rules:?}: `{line}` is redundant in {lines:?}"
                    );
                }
                combinations += 1;
            }
        }
        assert_eq!(combinations, 2 * (9 + 36 + 84));
        // the two shapes that broke, pinned as text: a whole directory takes no close,
        // and re-includes its files past a `/**` an every-depth sibling put above it
        let stack = tsv_stack("!dist/sub/\n!dist/sub/keep.ts\n");
        assert!(
            shadow_warning("dist", None, &stack).unwrap().ends_with(
                "re-include it by adding `!/dist/`, `/dist/*`, `!/dist/sub/` and `!/dist/sub/keep.ts`, in that order, to the repo-root .formatignore"
            )
        );
        let stack = tsv_stack("!dist/sub/\n!dist/**/k2.ts\n");
        assert!(
            shadow_warning("dist", None, &stack).unwrap().ends_with(
                "re-include it by adding `!/dist/`, `/dist/**`, `!/dist/**/`, `!/dist/sub/**`, `!/dist/sub/` and `!/dist/**/k2.ts`, in that order, to the repo-root .formatignore"
            )
        );
        // a whole directory under a whole one, both under the every-depth `/dist/**`:
        // `deep` reads its NEAREST opened ancestor, `sub`, re-included whole already, so
        // it spells no `!/dist/sub/deep/**` of its own
        let stack = tsv_stack("!dist/sub/\n!dist/sub/deep/\n!dist/**/k2.ts\n");
        assert!(
            shadow_warning("dist", None, &stack).unwrap().ends_with(
                "re-include it by adding `!/dist/`, `/dist/**`, `!/dist/**/`, `!/dist/sub/**`, `!/dist/sub/`, `!/dist/sub/deep/` and `!/dist/**/k2.ts`, in that order, to the repo-root .formatignore"
            )
        );
    }

    #[test]
    fn shadow_warning_names_a_gitignore_behind_the_deeper_tsv_rule() {
        // the excluding tsv rule sits in a file deeper than the re-include's, so it is the
        // user's to narrow — but a `.gitignore` rule stands behind it, which narrowing
        // alone leaves in force: the lines that pass it go beside the narrowing, one
        // warning rather than a second one on the rerun
        let stack = stack_from(
            &[("pkg", "dist/\n")],
            &[("", "!pkg/dist/keep.ts\n"), ("pkg", "dist/\n")],
        );
        let warning = shadow_warning("pkg/dist", None, &stack).unwrap();
        assert_eq!(
            warning,
            "pkg/dist is excluded by a rule in pkg/.formatignore, so a re-include under it in the repo-root .formatignore does nothing; narrow or negate that rule, and re-include it past the rule in pkg/.gitignore that excludes it by adding `!/pkg/dist/`, `/pkg/dist/*` and `!/pkg/dist/keep.ts`, in that order, to the repo-root .formatignore"
        );
        let lines: Vec<&str> = warning.split('`').skip(1).step_by(2).collect();
        let mut fixed = IgnoreStack::new();
        fixed.push_gitignore("pkg", "dist/\n");
        fixed.push_formatignore("", &format!("!pkg/dist/keep.ts\n{}\n", lines.join("\n")));
        fixed.push_formatignore("pkg", "");
        assert!(!fixed.is_ignored("pkg/dist/keep.ts", false));
        assert!(fixed.is_ignored("pkg/dist/other.ts", false));
        // and where the pruned directory's own name holds a control character, no line
        // past the `.gitignore` rule can be offered either: the narrowing stands alone,
        // and the warning says why (the name quoted, as every printed path is)
        let stack = stack_from(
            &[("pkg", "di\u{1b}st/\n")],
            &[("", "!pkg/di\u{1b}st/keep.ts\n"), ("pkg", "di\u{1b}st/\n")],
        );
        assert_eq!(
            shadow_warning("pkg/di\u{1b}st", None, &stack).unwrap(),
            "\"pkg/di\\033st\" is excluded by a rule in pkg/.formatignore, so a re-include under it in the repo-root .formatignore does nothing; narrow or negate that rule; no re-include past the rule in pkg/.gitignore that excludes it can be offered, since no ignore-file line can spell the control character in its path"
        );
        // a re-include no line can spell — its pattern ends in a carriage return, which a
        // line's end strips, or holds any other control character but a tab, which the
        // warning will not print raw — gets the directory escape alone
        for rule in [
            "!dist/keep.ts\r \n",
            "!dist/ke\rep.ts\n",
            "!dist/k\u{1b}eep.ts\n",
        ] {
            let stack = stack_from(&[], &[("", rule)]);
            assert_eq!(
                shadow_warning("dist", None, &stack).unwrap(),
                "dist is skipped by tsv's build-output heuristic, so a re-include under it in the repo-root .formatignore does nothing; re-include it by adding `!/dist/` to the repo-root .formatignore first, since no ignore-file line can spell the control character in the re-include",
                "{rule:?}"
            );
        }
        // and where the pruned directory's own name holds one, no line at all — the name
        // is quoted, the way every printed path is
        let stack = stack_from(&[], &[("", "!di\u{1b}st/keep.ts\n")]);
        assert_eq!(
            shadow_warning("di\u{1b}st", None, &stack).unwrap(),
            "\"di\\033st\" is skipped by tsv's build-output heuristic, so a re-include under it in the repo-root .formatignore does nothing; no ignore-file line can spell the control character in its path"
        );
    }
}
