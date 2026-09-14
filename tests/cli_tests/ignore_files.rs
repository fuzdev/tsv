//! Ignore-file scoping and the warnings it emits: `.gitignore` / `.formatignore` /
//! `.prettierignore` precedence and shadowing, the build-output heuristic, the format
//! root's independence from the cwd, and the C-quoting every printed path takes.

use std::fs;
use std::process::Command;

use crate::common::{
    FORMATTED_TS, UNFORMATTED_TS, built_tsv, canonical_display, git_repo, temp_dir, to_posix, tsv,
    tsv_in_dir,
};
#[cfg(unix)]
use crate::common::{mode_bits_are_enforced, set_mode};

/// An explicitly named file is bounded by the ignore files, as the walk that would reach it
/// is: one a `.formatignore` rule excludes is skipped quietly and left untouched, and a run
/// whose every argument was such a file is not the empty-run error (a pre-commit hook with
/// only ignored files staged). A directory the rules exclude still is, and warns, naming
/// the rule's file.
///
/// **A deliberate hand-mirrored pair with `scripts/test_npm.ts`** — one of the two the
/// shared table (`tests/discovery/scenarios.json`) cannot take, both for the same reason.
/// That runner invokes `format --list`, where an empty scope is a valid answer that exits
/// 0, while what these pin is the **format action's** exit code: 0 here when every
/// argument was an excluded file, and 2 with nothing written for the other one, the
/// unsupported-extension refusal (whose *discovery* half the table does hold, as an
/// `error` case). The table took the pairs that were purely about the walk once it grew a
/// multi-argument case; widening it to drive the writing action would mix the walk's
/// question with the command's, so these two stay — visibly, rather than as silent second
/// copies.
#[test]
fn test_format_explicit_file_an_ignore_rule_excludes_is_skipped() {
    let dir = temp_dir("excluded_explicit_file");
    fs::write(dir.join(".formatignore"), "b.ts\ngen/\n").unwrap();
    let gen_dir = dir.join("gen");
    fs::create_dir_all(&gen_dir).unwrap();
    let b = dir.join("b.ts");
    let c = dir.join("c.ts");
    let d = gen_dir.join("d.ts");
    for path in [&b, &c, &d] {
        fs::write(path, "const   x=1").unwrap();
    }

    let output = tsv(&["format", b.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    assert!(!stderr.contains("warning"), "stderr: {stderr}");
    assert_eq!(fs::read_to_string(&b).unwrap(), "const   x=1");

    // beside a file that formats, the run is the ordinary one
    let output = tsv(&["format", b.to_str().unwrap(), c.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(fs::read_to_string(&c).unwrap(), "const x = 1;\n");
    assert_eq!(fs::read_to_string(&b).unwrap(), "const   x=1");

    // a directory the rules exclude is still the empty-run error, warned
    let output = tsv(&["format", gen_dir.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr: {stderr}");
    assert!(
        stderr.contains(
            ".formatignore, so nothing under it is formatted; narrow or negate that rule to format it"
        ),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("No files to format"), "stderr: {stderr}");
    assert_eq!(fs::read_to_string(&d).unwrap(), "const   x=1");
}

// The two behaviors that used to sit here — one ignore scope moved across file
// arguments, and a named path reading no ignore file inside an excluded directory —
// are in the SHARED table now (`tests/discovery/scenarios.json`, the
// `file_arguments_share_one_scope_moved_per_directory` and
// `explicit_path_reads_no_ignore_file_inside_an_excluded_directory` scenarios), which
// gained a multi-argument case shape for exactly this. They were a hand-mirrored pair —
// one test here, one in `scripts/test_npm.ts` — which is the drift that table exists to
// remove: it now holds them for all three walkers from one place.

/// A named file whose name holds a line feed is warned about like any other a `.gitignore`
/// excludes, but offered no re-include lines: the name would split each one across two
/// lines, which no ignore file can hold. Unix only, as Windows forbids the character in a
/// file name.
#[cfg(unix)]
#[test]
fn test_format_named_file_with_a_line_break_gets_no_reinclude_lines() {
    let dir = temp_dir("line_break_name");
    fs::create_dir_all(dir.join(".git")).unwrap();
    fs::create_dir_all(dir.join("build")).unwrap();
    fs::write(dir.join(".gitignore"), "build/\n").unwrap();
    let file = dir.join("build/a\nb.ts");
    fs::write(&file, UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", file.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    assert!(output.stdout.is_empty(), "stderr: {stderr}");
    // the argument is quoted, so the warning stays one line
    assert!(
        stderr.contains(
            "/build/a\\nb.ts\" is inside build, which a rule in the repo-root .gitignore excludes, so it is not formatted; no ignore-file line can spell the control character in its path, so narrow that rule to format it"
        ),
        "stderr: {stderr}"
    );
    assert_eq!(stderr.lines().count(), 1, "stderr: {stderr}");
    assert!(!stderr.contains('`'), "stderr: {stderr}");
}

/// A path holding a control character or a double quote is printed C-quoted, as `git
/// ls-files` prints one (`core.quotePath=false`), everywhere a path is printed: the
/// `--list` and changed-path lines on stdout, the per-file error line, a traversal error,
/// an ignore-file warning and the warning naming an excluded argument. Every other path —
/// a space, a backslash, a character outside ASCII — prints as it is, and the re-include
/// PATTERNS a warning offers stay literal. Unix only: Windows forbids the characters.
#[cfg(unix)]
#[test]
fn test_format_quotes_a_path_holding_a_control_character_wherever_it_prints_it() {
    if !mode_bits_are_enforced() {
        return;
    }
    let dir = temp_dir("quoted_paths");
    fs::create_dir_all(dir.join(".git")).unwrap();
    fs::create_dir_all(dir.join("build")).unwrap();
    fs::write(dir.join(".gitignore"), "build/\n").unwrap();
    fs::write(dir.join("a\nb.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("c\rd.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("e\"f.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("g\u{1b}h.ts"), "const x = \n").unwrap(); // a parse error
    fs::write(dir.join("i\\j.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("k l.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("mé.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("build/n\to.ts"), UNFORMATTED_TS).unwrap();

    let list = tsv_in_dir(dir.path(), &["format", "--list", "."]);
    assert_eq!(list.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&list.stdout)
            .lines()
            .collect::<Vec<_>>(),
        [
            r#""./a\nb.ts""#,
            r#""./c\rd.ts""#,
            r#""./e\"f.ts""#,
            r#""./g\033h.ts""#,
            "./i\\j.ts",
            "./k l.ts",
            "./mé.ts",
        ]
    );

    let check = tsv_in_dir(dir.path(), &["format", "--check", "."]);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert_eq!(check.status.code(), Some(2), "stderr: {stderr}");
    assert_eq!(
        String::from_utf8_lossy(&check.stdout)
            .lines()
            .collect::<Vec<_>>(),
        [
            r#""./a\nb.ts""#,
            r#""./c\rd.ts""#,
            r#""./e\"f.ts""#,
            "./i\\j.ts",
            "./k l.ts",
            "./mé.ts",
        ]
    );
    assert!(
        stderr
            .lines()
            .any(|line| line.starts_with(r#"error: "./g\033h.ts": "#)),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("6 would change, 0 unchanged, 1 errors"),
        "stderr: {stderr}"
    );

    // the argument warning quotes the path in prose and spells the patterns literally
    let named = tsv_in_dir(dir.path(), &["format", "--list", "build/n\to.ts"]);
    let stderr = String::from_utf8_lossy(&named.stderr);
    assert_eq!(named.status.code(), Some(0), "stderr: {stderr}");
    assert_eq!(
        stderr.trim_end(),
        "warning: \"build/n\\to.ts\" is inside build, which a rule in the repo-root .gitignore excludes, so it is not formatted; re-include it by adding `!/build/`, `/build/*` and `!/build/n\to.ts`, in that order, to the repo-root .formatignore"
    );

    // a traversal error and an ignore-file warning name their directory quoted too
    fs::create_dir_all(dir.join("lo\nck")).unwrap();
    fs::create_dir_all(dir.join("we\nird")).unwrap();
    fs::write(dir.join("we\nird/.formatignore"), b"\xff\n").unwrap();
    fs::write(dir.join("we\nird/w.ts"), FORMATTED_TS).unwrap();
    let _lock_mode = set_mode(&dir.join("lo\nck"), 0o000);
    let walk = tsv_in_dir(dir.path(), &["format", "--list", "."]);
    let stderr = String::from_utf8_lossy(&walk.stderr);
    assert_eq!(walk.status.code(), Some(2), "stderr: {stderr}");
    // the walk canonicalizes its root before printing any path under it, so the
    // expectation resolves the same way (`canonical_display` carries the host facts)
    let root = canonical_display(&dir);
    assert!(
        stderr.contains(&format!("error: \"{root}/lo\\nck\": read_dir failed: ")),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "warning: could not read \"{root}/we\\nird/.formatignore\" ("
        )),
        "stderr: {stderr}"
    );
    assert!(
        String::from_utf8_lossy(&walk.stdout).contains("\"./we\\nird/w.ts\"\n"),
        "stdout: {}",
        String::from_utf8_lossy(&walk.stdout)
    );
    // every diagnostic is exactly one line: two `error:`/`warning:` heads, one line each
    let heads = stderr
        .lines()
        .filter(|l| l.starts_with("error: ") || l.starts_with("warning: "))
        .count();
    assert_eq!(heads, stderr.lines().count(), "stderr: {stderr}");
}

/// The sorted-path order both listings promise is code-point order — what the native
/// sort key's bytes give — on every name, including one the two bins encode
/// differently: an astral-plane character sits above every BMP one by code point but
/// below U+E000..U+FFFF by UTF-16 unit, which is what a plain JS sort would read. The
/// JS pin is `scripts/test_npm.ts`'s "sorted-path order is code-point order".
#[test]
fn test_format_lists_in_code_point_order() {
    let dir = temp_dir("code_point_order");
    fs::create_dir_all(dir.join(".git")).unwrap();
    fs::write(dir.join(".gitignore"), "*.ts\n").unwrap();
    // U+FF10 (fullwidth zero) and U+1F600 (a grinning face): bytes EF BC 90 vs F0 9F 98 80,
    // UTF-16 units FF10 vs D83D DE00
    fs::write(dir.join("\u{ff10}.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("\u{1f600}.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("\u{ff10}.css"), "a{}\n").unwrap();
    fs::write(dir.join("\u{1f600}.css"), "a{}\n").unwrap();

    let list = tsv_in_dir(dir.path(), &["format", "--list", "."]);
    assert_eq!(list.status.code(), Some(0));
    assert_eq!(
        to_posix(&String::from_utf8_lossy(&list.stdout)),
        "./\u{ff10}.css\n./\u{1f600}.css\n"
    );
    // the diagnostics channel is sorted as whole strings, which agrees with the path
    // order for names at one depth (across a separator boundary the two can differ:
    // `a-b/x.ts` sorts before `a/y.ts` as text, after it as a path)
    let named = tsv_in_dir(
        dir.path(),
        &["format", "--list", "\u{1f600}.ts", "\u{ff10}.ts"],
    );
    let stderr = String::from_utf8_lossy(&named.stderr);
    assert_eq!(named.status.code(), Some(0), "stderr: {stderr}");
    let heads: Vec<&str> = stderr
        .lines()
        .map(|line| line.split(" is excluded").next().unwrap())
        .collect();
    assert_eq!(heads, ["warning: \u{ff10}.ts", "warning: \u{1f600}.ts"]);
}

/// A bad path argument and a `parse` read failure quote the path the same way.
#[cfg(unix)]
#[test]
fn test_argument_errors_quote_a_path_holding_a_control_character() {
    let dir = temp_dir("quoted_argument_errors");
    let missing = dir.join("no\nfile.ts");
    let missing = missing.to_str().unwrap();
    let root = dir.path().to_str().unwrap();

    let format = tsv(&["format", missing]);
    assert_eq!(format.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&format.stderr).trim_end(),
        format!("error: \"{root}/no\\nfile.ts\": not a file or directory")
    );

    let parse = tsv(&["parse", missing]);
    assert_eq!(parse.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&parse.stderr).starts_with(&format!(
            "Error: Error reading file \"{root}/no\\nfile.ts\": "
        )),
        "stderr: {}",
        String::from_utf8_lossy(&parse.stderr)
    );

    let json = dir.join("da\"ta.json");
    fs::write(&json, "[]").unwrap();
    let unsupported = tsv(&["format", json.to_str().unwrap()]);
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&unsupported.stderr).starts_with(&format!(
            "error: \"{root}/da\\\"ta.json\": unsupported file extension"
        )),
        "stderr: {}",
        String::from_utf8_lossy(&unsupported.stderr)
    );
}

/// A traversal error names its directory absolutely, as every ignore-file warning does,
/// so two spellings of one root (`t ./t`) report an unreadable subdirectory once.
#[cfg(unix)]
#[test]
fn test_format_traversal_error_reports_once_across_spellings() {
    if !mode_bits_are_enforced() {
        return;
    }
    let dir = temp_dir("traversal_error_dedup");
    fs::create_dir_all(dir.join("t/locked")).unwrap();
    fs::write(dir.join("t/a.ts"), FORMATTED_TS).unwrap();
    // empty, so the mode alone makes it unreadable
    let _lock_mode = set_mode(&dir.join("t/locked"), 0o000);
    let output = tsv_in_dir(dir.path(), &["format", "--list", "t", "./t"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr: {stderr}");
    let reports: Vec<&str> = stderr
        .lines()
        .filter(|l| l.contains("read_dir failed"))
        .collect();
    assert_eq!(reports.len(), 1, "stderr: {stderr}");
    let absolute = dir.join("t/locked");
    assert!(
        reports[0].contains(absolute.to_str().unwrap()),
        "the error names the directory absolutely: {stderr}"
    );
    // the overlap dedup keeps the first spelling in sorted order, `./t` ahead of `t`
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .collect::<Vec<_>>(),
        ["./t/a.ts"]
    );
}

/// A named symbolic link to a directory is walked as a directory but graded by the
/// matcher as git grades it — a link: `git check-ignore foo` with a `foo/` rule says not
/// ignored, since a directory-only pattern matches no link, while a `foo` rule does match
/// it. One a rule does exclude is then counted as an excluded *file* argument, so a run
/// naming only such links exits 0 like one naming only excluded files.
#[cfg(unix)]
#[test]
fn test_format_grades_a_symlinked_directory_argument_as_a_link() {
    let dir = temp_dir("symlinked_dir_argument");
    fs::create_dir_all(dir.join(".git")).unwrap();
    fs::write(dir.join(".gitignore"), "foo/\nbar\n").unwrap();
    fs::create_dir(dir.join("real")).unwrap();
    fs::write(dir.join("real/a.ts"), UNFORMATTED_TS).unwrap();
    std::os::unix::fs::symlink("real", dir.join("foo")).unwrap();
    std::os::unix::fs::symlink("real", dir.join("bar")).unwrap();

    // `foo/` does not match the link, so the argument is in scope and walked
    let foo = tsv_in_dir(dir.path(), &["format", "--list", "foo"]);
    assert_eq!(foo.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&foo.stdout), "foo/a.ts\n");
    assert_eq!(String::from_utf8_lossy(&foo.stderr), "");

    // `bar` matches the link itself: excluded with the warning a `.gitignore` rule gets,
    // whose re-include line names the link (`!/bar`, not `!/bar/`), and the run exits 0
    // as it does when every argument is an excluded file
    let bar = tsv_in_dir(dir.path(), &["format", "--check", "bar"]);
    let stderr = String::from_utf8_lossy(&bar.stderr);
    assert_eq!(bar.status.code(), Some(0), "stderr: {stderr}");
    assert_eq!(String::from_utf8_lossy(&bar.stdout), "");
    assert_eq!(
        stderr,
        "warning: bar is excluded by a rule in the repo-root .gitignore, so it is not formatted; re-include it by adding `!/bar` to the repo-root .formatignore\n0 would change, 0 unchanged\n"
    );
}

#[test]
fn test_format_heuristic_shadow_warns_for_anchored_negation() {
    // #5 diagnostic: with no `.gitignore` (heuristic regime), a `.formatignore`
    // `!build/keep.ts` is a silent no-op — the heuristic prunes `build/` before
    // descending, and git's parent-dir rule bars re-including a file under an
    // excluded dir. Behavior is unchanged (build/ stays pruned); we only warn,
    // pointing at the dir-level escape. Fires in `--list` too.
    let dir = temp_dir("heuristic_shadow_warn");
    fs::create_dir_all(dir.join("build")).unwrap();
    fs::write(dir.join(".formatignore"), "!build/keep.ts\n").unwrap();
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("build/keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    // warning is non-fatal: exit code stays 0, stdout (the --list set) stays clean
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a.ts"), "stdout: {stdout}");
    // build/ is still pruned — the re-include did NOT take effect
    assert!(!stdout.contains("keep.ts"), "build/ still pruned: {stdout}");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning:"), "stderr: {stderr}");
    // names the pruned dir + the heuristic, and points at the dir-level escape in the
    // file the re-include was written in. Outside a repo both paths read absolutely,
    // while the line stays relative to that file's directory and anchored
    assert!(
        stderr.contains("build is skipped by tsv's build-output heuristic"),
        "stderr: {stderr}"
    );
    assert!(
        to_posix(&stderr).contains(
            "/.formatignore does nothing; re-include it by adding `!/build/`, `/build/*` and `!/build/keep.ts`, in that order, to "
        ),
        "stderr: {stderr}"
    );
}

#[test]
fn test_format_heuristic_shadow_no_warning_for_floating_or_dir_reinclude() {
    // a *floating* `!keep.ts` targets any depth, not `build/` specifically, so it
    // must NOT warn just because a keep.ts sits under a pruned build/
    let dir = temp_dir("heuristic_shadow_floating");
    fs::create_dir_all(dir.join("build")).unwrap();
    fs::write(dir.join(".formatignore"), "!keep.ts\n").unwrap();
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("build/keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning:"), "floating: {stderr}");
    // build/ is still pruned (the floating `!` doesn't re-include the dir)
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("keep.ts"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    // the dir-level escape `!build/` re-includes build/ — no prune, no warning,
    // and the file is now in scope
    let dir = temp_dir("heuristic_shadow_dir_reinclude");
    fs::create_dir_all(dir.join("build")).unwrap();
    fs::write(dir.join(".formatignore"), "!build/\n").unwrap();
    fs::write(dir.join("build/keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning:"), "dir-reinclude: {stderr}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("keep.ts"),
        "build/ formatted: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn test_format_heuristic_shadow_silent_with_gitignore() {
    // with a `.gitignore` present the heuristic is OFF, so build/ is governed by
    // git rules, not the heuristic — `!build/keep.ts` is no longer shadowed by a
    // heuristic prune, so there is nothing to warn about.
    let dir = git_repo("heuristic_shadow_gitignore");
    fs::create_dir_all(dir.join("build")).unwrap();
    // an unrelated .gitignore turns the heuristic off (presence is the signal)
    fs::write(dir.join(".gitignore"), "node_modules/\n").unwrap();
    fs::write(dir.join(".formatignore"), "!build/keep.ts\n").unwrap();
    fs::write(dir.join("build/keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning:"), "gitignore regime: {stderr}");
    // heuristic off → build/ is formatted (the file is in scope)
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("keep.ts"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn test_format_prettierignore_outside_repo_warns() {
    // issue #1 footgun: outside a git repo tsv reads `.formatignore` but not
    // `.prettierignore`, so a prettier user's `.prettierignore` is silently
    // skipped. Discovery is unchanged (the would-be-ignored file stays in scope),
    // but we DO warn, pointing at the rename / `git init` fixes. Fires in `--list`.
    let dir = temp_dir("prettierignore_outside_repo_warns");
    fs::write(dir.join(".prettierignore"), "ignored.ts\n").unwrap();
    fs::write(dir.join("ignored.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    // warning is non-fatal: exit code stays 0, stdout (the --list set) is unchanged
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `.prettierignore` is NOT honored outside a repo → both files stay in scope
    assert!(stdout.contains("ignored.ts"), "not honored: {stdout}");
    assert!(stdout.contains("keep.ts"), "stdout: {stdout}");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning:"), "stderr: {stderr}");
    assert!(
        stderr.contains(".prettierignore in") && stderr.contains("is not read outside a git repo"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("rename it to .formatignore"),
        "stderr: {stderr}"
    );
}

#[test]
fn test_format_prettierignore_outside_repo_no_warn_with_formatignore() {
    // a sibling `.formatignore` means the native file was adopted, so the
    // `.prettierignore` is vestigial — no warning. And `.formatignore` IS honored.
    let dir = temp_dir("prettierignore_outside_repo_formatignore");
    fs::write(dir.join(".prettierignore"), "p.ts\n").unwrap();
    fs::write(dir.join(".formatignore"), "f.ts\n").unwrap();
    fs::write(dir.join("p.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("f.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning:"), "suppressed: {stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `.formatignore` honored (f.ts pruned); `.prettierignore` still unread (p.ts kept)
    assert!(!stdout.contains("f.ts"), "formatignore honored: {stdout}");
    assert!(stdout.contains("p.ts"), "prettierignore unread: {stdout}");
    assert!(stdout.contains("keep.ts"), "stdout: {stdout}");
}

#[test]
fn test_format_prettierignore_in_repo_no_warn() {
    // inside a repo `.prettierignore` IS read (drop-in compat, hierarchically)
    // and honored, so there is nothing to warn about.
    let dir = git_repo("prettierignore_in_repo_no_warn");
    fs::write(dir.join(".prettierignore"), "ignored.ts\n").unwrap();
    fs::write(dir.join("ignored.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning:"), "in repo: {stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // honored: ignored.ts pruned, keep.ts in scope
    assert!(!stdout.contains("ignored.ts"), "honored: {stdout}");
    assert!(stdout.contains("keep.ts"), "stdout: {stdout}");
}

#[test]
fn test_format_prettierignore_shadowed_by_sibling_formatignore_warns() {
    // inside a repo, a `.formatignore` beside a `.prettierignore` shadows it (one
    // tsv layer per directory) — the `.prettierignore`'s rules go unread there.
    // That's a silent surprise for a prettier migration, so tsv warns (non-fatal),
    // pointing at merging the patterns into `.formatignore`. Discovery follows the
    // `.formatignore`: `af.ts` pruned, `pf.ts` (only in the shadowed prettierignore)
    // stays in scope.
    let dir = git_repo("prettierignore_shadowed_warns");
    fs::write(dir.join(".formatignore"), "af.ts\n").unwrap();
    fs::write(dir.join(".prettierignore"), "pf.ts\n").unwrap();
    fs::write(dir.join("af.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("pf.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    // non-fatal: exit stays 0, the --list set follows the .formatignore
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("af.ts"), ".formatignore applied: {stdout}");
    assert!(
        stdout.contains("pf.ts"),
        "shadowed prettierignore unread: {stdout}"
    );
    assert!(stdout.contains("keep.ts"), "stdout: {stdout}");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning:"), "stderr: {stderr}");
    assert!(
        stderr.contains(".prettierignore in")
            && stderr.contains("shadowed by a sibling .formatignore"),
        "stderr: {stderr}"
    );
}

#[test]
fn test_format_prettierignore_alone_in_repo_not_shadowed_no_warn() {
    // control: a `.prettierignore` with NO sibling `.formatignore` is read (honored)
    // and there is nothing to shadow — no warning.
    let dir = git_repo("prettierignore_alone_no_shadow_warn");
    fs::write(dir.join(".prettierignore"), "pf.ts\n").unwrap();
    fs::write(dir.join("pf.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("shadowed"), "no shadow warning: {stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("pf.ts"),
        "prettierignore honored: {stdout}"
    );
    assert!(stdout.contains("keep.ts"), "stdout: {stdout}");
}

#[test]
fn test_format_overlapping_roots_name_a_shadowed_prettierignore_once() {
    // a directory reached by two walks — as an argument (`.`) and as the preloaded
    // ancestor of another root (`sub`), or under two spellings of one argument (`.` and
    // `./`) — is named by its absolute path in both, so the walk's exact-string dedup
    // collapses the warning to one line rather than one per spelling
    let dir = git_repo("overlapping_roots_shadow_once");
    fs::write(dir.join(".formatignore"), "af.ts\n").unwrap();
    fs::write(dir.join(".prettierignore"), "pf.ts\n").unwrap();
    fs::create_dir(dir.join("sub")).unwrap();
    fs::write(dir.join("sub/keep.ts"), FORMATTED_TS).unwrap();
    let expected = format!(".prettierignore in {} is shadowed", canonical_display(&dir));

    for roots in [[".", "sub"], [".", "./"]] {
        let output = tsv_in_dir(&dir, &["format", "--list", roots[0], roots[1]]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{roots:?}: {stderr}");
        let shadowed: Vec<&str> = stderr
            .lines()
            .filter(|line| line.contains("is shadowed"))
            .collect();
        assert_eq!(
            shadowed.len(),
            1,
            "{roots:?}: one warning per directory: {stderr}"
        );
        assert!(
            shadowed[0].contains(&expected),
            "{roots:?}: named by its absolute path: {stderr}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn test_format_relative_root_under_a_deleted_cwd_is_refused() {
    // a relative root has nothing absolute to resolve against once the working directory
    // is gone (Linux still resolves `..` inside a removed directory, so the argument
    // itself stats fine). Walked anyway, it anchored no format root and read no
    // ancestor's ignore files — the repo root's `.formatignore` went unread and
    // `a/skip.ts` was listed — so the root is refused instead
    let dir = git_repo("deleted_cwd_relative_root");
    fs::write(dir.join(".formatignore"), "a/skip.ts\n").unwrap();
    fs::create_dir_all(dir.join("a/gone")).unwrap();
    fs::write(dir.join("a/skip.ts"), FORMATTED_TS).unwrap();
    fs::write(dir.join("a/keep.ts"), FORMATTED_TS).unwrap();

    let output = Command::new("sh")
        .args([
            "-c",
            r#"cd a/gone && rmdir ../gone && exec "$0" format --list .."#,
        ])
        .arg(built_tsv())
        .current_dir(&*dir)
        .output()
        .expect("spawn sh");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr: {stderr}");
    assert!(
        stderr.contains(
            "error: ..: cannot resolve a relative path: the working directory is unavailable"
        ),
        "stderr: {stderr}"
    );
    assert!(!stdout.contains("skip.ts"), "stdout: {stdout}");
}

#[test]
fn test_format_nested_prettierignore_outside_repo_does_not_warn() {
    // the warning is bounded to the TARGET ROOT, and OUTSIDE a repo tsv's regime is
    // `.formatignore`-only at every depth — so a nested `.prettierignore` here is not
    // read (no warning, not honored). Inside a repo it WOULD be read hierarchically
    // (see test_format_nested_prettierignore_in_repo_is_honored).
    let dir = temp_dir("nested_prettierignore_outside_repo");
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("sub/.prettierignore"), "x.ts\n").unwrap();
    fs::write(dir.join("sub/x.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning:"), "nested: {stderr}");
    // nested `.prettierignore` is not honored → sub/x.ts still in scope
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("x.ts"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn test_format_nested_prettierignore_in_repo_is_honored() {
    // inside a repo `.prettierignore` is hierarchical (like `.formatignore`): a
    // nested `sub/.prettierignore` prunes its own subtree, and a subdir invocation
    // still walks up to the repo root and picks it up (the drop-in-compat point for
    // monorepos that run prettier per-package).
    let dir = git_repo("nested_prettierignore_in_repo");
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("sub/.prettierignore"), "subskip.ts\n").unwrap();
    fs::write(dir.join("sub/subskip.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("sub/keep.ts"), UNFORMATTED_TS).unwrap();

    // from the repo root
    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("subskip.ts"),
        "nested prettierignore honored: {stdout}"
    );
    assert!(stdout.contains("keep.ts"), "stdout: {stdout}");

    // and from the subdirectory directly — still walks up to `.git`, same result
    let sub_output = tsv(&["format", "--list", dir.join("sub").to_str().unwrap()]);
    let sub_stdout = String::from_utf8_lossy(&sub_output.stdout);
    assert!(
        !sub_stdout.contains("subskip.ts"),
        "subdir invocation honors nested: {sub_stdout}"
    );
    assert!(
        sub_stdout.contains("keep.ts"),
        "subdir stdout: {sub_stdout}"
    );
}

#[test]
fn test_format_unreadable_formatignore_warns_and_drops_rules() {
    // a present `.formatignore` that can't be read (here invalid UTF-8 — the most
    // likely real trigger) is no longer silently treated as absent: tsv warns and
    // drops its rules (the file it would have ignored stays in scope), rather than
    // silently formatting an excluded file.
    let dir = temp_dir("unreadable_formatignore");
    // a valid pattern line then invalid UTF-8 bytes → strict read_to_string fails
    fs::write(dir.join(".formatignore"), b"ignored.ts\n\xff\xfe").unwrap();
    fs::write(dir.join("ignored.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning:"), "stderr: {stderr}");
    assert!(
        stderr.contains("could not read")
            && stderr.contains(".formatignore")
            && stderr.contains("ignore rules are not applied"),
        "stderr: {stderr}"
    );
    // rules dropped → the would-be-ignored file is still in scope
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ignored.ts"), "rules dropped: {stdout}");
    assert!(stdout.contains("keep.ts"), "stdout: {stdout}");
}

#[test]
fn test_format_unreadable_gitignore_warns_and_keeps_heuristic_on() {
    // the consequential case: a present `.gitignore` normally turns the
    // build-output heuristic OFF (so `build/` would be formatted). If it's
    // unreadable, tsv warns and does NOT push it — the heuristic stays ON and
    // `build/` is pruned. The warning makes that otherwise-silent swing visible.
    let dir = git_repo("unreadable_gitignore");
    fs::write(dir.join(".gitignore"), b"\xff\xfe\xfa").unwrap();
    fs::create_dir_all(dir.join("build")).unwrap();
    fs::write(dir.join("build/keep.ts"), UNFORMATTED_TS).unwrap();
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/app.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not read") && stderr.contains(".gitignore"),
        "stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // heuristic stayed on (unreadable .gitignore not pushed) → build/ pruned
    assert!(!stdout.contains("keep.ts"), "build pruned: {stdout}");
    // normal source still discovered
    assert!(stdout.contains("app.ts"), "stdout: {stdout}");
}

#[test]
fn test_format_unreadable_formatignore_still_shadows_prettierignore() {
    // Decision: precedence is by PRESENCE, not readability. At the repo root a
    // present-but-unreadable `.formatignore` still shadows `.prettierignore` — tsv
    // warns and applies *no* tsv rules, rather than silently falling back to
    // prettier's file. So `.prettierignore`'s pattern must NOT take effect.
    let dir = git_repo("unreadable_formatignore_shadows");
    fs::write(dir.join(".formatignore"), b"\xff\xfe").unwrap(); // present, unreadable
    fs::write(dir.join(".prettierignore"), "p_ignored.ts\n").unwrap(); // would prune, if read
    fs::write(dir.join("p_ignored.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("keep.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not read") && stderr.contains(".formatignore"),
        "stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // no fallback: `.prettierignore` was not read, so p_ignored.ts stays in scope
    assert!(
        stdout.contains("p_ignored.ts"),
        "no fallback to prettierignore: {stdout}"
    );
    assert!(stdout.contains("keep.ts"), "stdout: {stdout}");
}

#[test]
fn test_format_target_scope_is_cwd_independent() {
    // #4: a non-git project's own `.formatignore` is honored whether you cd
    // into it or name it by path from an unrelated cwd — the format root is the
    // filesystem root, derived from the target, never the cwd. (`.prettierignore`
    // is repo-only, so the native `.formatignore` is what governs loose files.)
    // `gen/` is not a heuristic dir, so the ignore file is the only thing that
    // can skip it.
    let base = temp_dir("scope_cwd_indep");
    let proj = base.join("proj");
    let other = base.join("other");
    fs::create_dir_all(proj.join("gen")).unwrap();
    fs::create_dir_all(&other).unwrap();
    fs::write(proj.join(".formatignore"), "gen/\n").unwrap();
    fs::write(proj.join("src.ts"), UNFORMATTED_TS).unwrap();
    fs::write(proj.join("gen/out.ts"), UNFORMATTED_TS).unwrap();

    // (a) cd into proj and list "."; (b) from a sibling cwd, list proj by path
    let from_inside = tsv_in_dir(&proj, &["format", "--list", "."]);
    let from_outside = tsv_in_dir(&other, &["format", "--list", proj.to_str().unwrap()]);

    for (label, out) in [("inside", &from_inside), ("outside", &from_outside)] {
        assert_eq!(
            out.status.code(),
            Some(0),
            "{label} stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("src.ts"), "{label}: src.ts is in scope");
        assert!(
            !stdout.contains("out.ts"),
            "{label}: gen/ honored regardless of cwd"
        );
    }
}
