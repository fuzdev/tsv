//! Differential conformance: [`IgnoreStack`] vs real `git check-ignore`.
//!
//! The unit tests in `src/lib.rs` hard-code their git expectations ("pinned out
//! of band"); this suite instead builds the same nested-`.gitignore` trees on
//! disk in a throwaway git repo and asserts our matcher agrees with git itself,
//! candidate by candidate — so the git-faithfulness claim is checked, not
//! asserted. Requires a `git` binary (skipped with a note if absent). git runs
//! with `core.excludesFile=/dev/null` so no machine-global ignore interferes;
//! parity holds on case-sensitive filesystems (see the crate's "Known edges").

// Test harness: unwrap/expect/panic on setup or oracle failure is the desired
// behavior (a broken fixture or unusable git should fail loudly), matching the
// crate's other test modules.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use tsv_ignore::IgnoreStack;

/// Whether a `git` binary is callable — lets a git-less environment skip rather
/// than hard-fail.
fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// `git check-ignore`: exit 0 => ignored, 1 => not ignored.
fn git_ignored(repo: &Path, path: &str) -> bool {
    let out = Command::new("git")
        .args(["check-ignore", "-q", "--no-index", path])
        .current_dir(repo)
        .output()
        .expect("run git check-ignore");
    match out.status.code() {
        Some(0) => true,
        Some(1) => false,
        other => panic!("git check-ignore unexpected exit {other:?} for {path}"),
    }
}

/// A unique temp repo path (no temp-dir dependency).
fn fresh_repo(tag: &str) -> std::path::PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("tsv_git_oracle_{tag}_{}_{n}", std::process::id()))
}

/// Build a git repo with the given `.gitignore` layers, materialize every
/// candidate on disk (so directory-ness is real), then assert our `IgnoreStack`
/// matches `git check-ignore` for each candidate.
///
/// `layers`: `(anchor_dir, gitignore_content)`, shallowest-first (anchor `""` =
/// repo root). `candidates`: `(path, is_dir)`.
fn assert_matches_git(tag: &str, layers: &[(&str, &str)], candidates: &[(&str, bool)]) {
    if !git_available() {
        eprintln!("git_oracle[{tag}]: `git` not found — skipping");
        return;
    }
    let repo = fresh_repo(tag);
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(&repo).unwrap();
    let run = |args: &[&str]| {
        let o = Command::new("git")
            .args(args)
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    };
    run(&["init", "-q"]);
    run(&["config", "core.excludesFile", "/dev/null"]);

    let mut stack = IgnoreStack::new();
    for (anchor, content) in layers {
        let dir = if anchor.is_empty() {
            repo.clone()
        } else {
            repo.join(anchor)
        };
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".gitignore"), content).unwrap();
        stack.push_gitignore(anchor, content);
    }

    for (path, is_dir) in candidates {
        let full = repo.join(path);
        if *is_dir {
            fs::create_dir_all(&full).unwrap();
        } else {
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, b"x").unwrap();
        }
    }

    let mut mismatches = Vec::new();
    for (path, is_dir) in candidates {
        let ours = stack.is_ignored(path, *is_dir);
        let theirs = git_ignored(&repo, path);
        if ours != theirs {
            mismatches.push(format!("{path} (dir={is_dir}): ours={ours} git={theirs}"));
        }
    }
    let _ = fs::remove_dir_all(&repo);
    assert!(
        mismatches.is_empty(),
        "git_oracle[{tag}]: {} mismatch(es) vs `git check-ignore`:\n{}",
        mismatches.len(),
        mismatches.join("\n"),
    );
}

#[test]
fn nested_negation_and_parent_prune() {
    assert_matches_git(
        "nested",
        &[
            (
                "",
                "*.log\nbuild/\n!keep.log\n/rootonly.ts\nfloat.ts\nfoo/**\n!foo/keep.ts\na/**/\nlogs/**/\nbar/\n!bar/\n",
            ),
            ("a", "!*.log\nb/\n"),
            ("a/b", "!special.ts\n"),
        ],
        &[
            ("debug.log", false),
            ("keep.log", false),
            ("a/debug.log", false), // a/.gitignore !*.log re-includes
            ("rootonly.ts", false),
            ("a/rootonly.ts", false), // /rootonly.ts anchored to root only
            ("float.ts", false),
            ("a/float.ts", false),
            ("build", true),
            ("a/build/y.ts", false), // build/ floats to any depth
            ("foo", true),           // foo/** never matches foo itself
            ("foo/keep.ts", false),  // !foo/keep.ts re-includes
            ("foo/other.ts", false),
            ("a/b", true),
            ("a/b/special.ts", false), // parent a/b excluded => !special.ts can't re-include
            ("a/file.ts", false),      // a/**/ is dir-only, spares direct files
            ("a/c/file.ts", false),    // a/**/ excludes subdirs
            ("logs", true),
            ("logs/today.log", false),
            ("bar", true), // bar/ then !bar/ re-includes
            ("bar/x.ts", false),
        ],
    );
}

#[test]
fn glob_and_character_classes() {
    assert_matches_git(
        "glob",
        &[
            (
                "",
                "[a-c]at.ts\nfile[!0-9].ts\ncaret[^0-9].ts\nlit[ab^].ts\nweird[]].ts\ndash[a-].ts\na**b.ts\n**z.ts\nq?.ts\ngen/**/*.snap\ndeep/\nnest/*\n!nest/keep.ts\nstar*\n",
            ),
            ("deep", "!keep/\n"),
        ],
        &[
            ("aat.ts", false),
            ("dat.ts", false), // out of [a-c]
            ("fileX.ts", false),
            ("file5.ts", false),  // in [!0-9] excludes => not ignored
            ("caretA.ts", false), // [^0-9] negates like [!0-9]
            ("caret5.ts", false), // in [^0-9] excludes => not ignored
            ("lit^.ts", false),   // ^ is a literal member when not first
            ("lita.ts", false),
            ("litc.ts", false),   // out of [ab^]
            ("weird].ts", false), // []] matches literal ]
            ("dash-.ts", false),  // [a-] literal dash
            ("axxb.ts", false),   // a**b collapses to a*b
            ("ab.ts", false),
            ("fooz.ts", false), // **z floats
            ("q1.ts", false),
            ("q12.ts", false),
            ("gen/a/b.snap", false),
            ("gen/x.snap", false), // middle ** matches zero dirs
            ("deep/file.ts", false),
            ("deep/keep/inner.ts", false), // deep/ excluded => deeper !keep/ blocked
            ("nest/a.ts", false),
            ("nest/keep.ts", false), // !nest/keep.ts re-includes
            ("starfish", false),
        ],
    );
}

#[test]
fn gitignore_doc_cascade() {
    // The canonical gitignore(5) example: "exclude everything except foo/bar".
    assert_matches_git(
        "cascade",
        &[("", "/*\n!/foo\n/foo/*\n!/foo/bar\nx/**/**/y.ts\n")],
        &[
            ("a.ts", false),
            ("top/inner.ts", false),
            ("foo", true),
            ("foo/x.ts", false),
            ("foo/bar", true),
            ("foo/bar/deep.ts", false),
            ("foo/bar/sub/deep.ts", false),
            ("x/y.ts", false),
            ("x/m/y.ts", false),
            ("x/m/n/y.ts", false),
        ],
    );
}
