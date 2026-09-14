//! `tsv format` in path mode: discovery over directories and named files, `--check`,
//! `--list`, `--jobs`, argument refusals, the goal fallback a path takes, per-file read
//! and write failures, and the filesystem shapes the walk has to survive (symlinks,
//! hard links, non-UTF-8 names, deep nesting).

use std::fs;
use std::process::Command;

use crate::common::{
    FORMATTED_TS, UNFORMATTED_TS, built_tsv, git_repo, temp_dir, to_posix, tsv, tsv_in_dir,
    tsv_stdin,
};
#[cfg(unix)]
use crate::common::{mode_bits_are_enforced, set_mode};

#[test]
fn test_format_directory_recursive_in_place() {
    let dir = temp_dir("dir_recursive");
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::create_dir_all(dir.join("node_modules")).unwrap();
    fs::create_dir_all(dir.join("dist")).unwrap();
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("sub/b.svelte"), "<div   >hi</div>\n").unwrap();
    // `{color:red}` is CSS, not a format placeholder
    #[allow(clippy::literal_string_with_formatting_args)]
    fs::write(dir.join("c.css"), "body{color:red}\n").unwrap();
    fs::write(dir.join("README.md"), "#   hi\n").unwrap();
    fs::write(dir.join("node_modules/x.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("dist/y.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", dir.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Matching files formatted in place
    assert_eq!(fs::read_to_string(dir.join("a.ts")).unwrap(), FORMATTED_TS);
    assert_eq!(
        fs::read_to_string(dir.join("sub/b.svelte")).unwrap(),
        "<div>hi</div>\n"
    );
    assert!(
        fs::read_to_string(dir.join("c.css"))
            .unwrap()
            .contains("color: red;")
    );
    // Excluded dirs and non-matching extensions untouched
    assert_eq!(
        fs::read_to_string(dir.join("node_modules/x.ts")).unwrap(),
        UNFORMATTED_TS
    );
    assert_eq!(
        fs::read_to_string(dir.join("dist/y.ts")).unwrap(),
        UNFORMATTED_TS
    );
    assert_eq!(
        fs::read_to_string(dir.join("README.md")).unwrap(),
        "#   hi\n"
    );

    // Changed paths reported on stdout in sorted order
    let stdout = String::from_utf8_lossy(&output.stdout);
    let a_pos = stdout.find("a.ts").expect("a.ts listed");
    let c_pos = stdout.find("c.css").expect("c.css listed");
    let b_pos = stdout.find("b.svelte").expect("b.svelte listed");
    assert!(a_pos < c_pos && c_pos < b_pos, "sorted order: {stdout}");
    assert!(!stdout.contains("README.md"));
    assert!(!stdout.contains("node_modules"));
}

#[test]
fn test_format_explicit_file_writes_in_place() {
    let dir = temp_dir("explicit_file");
    let file = dir.join("a.ts");
    fs::write(&file, UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", file.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), FORMATTED_TS);
    // Formatted source goes to the file, not stdout; stdout lists the changed path
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a.ts"));
    assert!(!stdout.contains("const x"));
}

#[test]
fn test_format_check_does_not_write() {
    let dir = temp_dir("check_dirty");
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--check", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(dir.join("a.ts")).unwrap(),
        UNFORMATTED_TS
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("a.ts"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("would"));
}

#[test]
fn test_format_check_clean_exits_zero() {
    let dir = temp_dir("check_clean");
    fs::write(dir.join("a.ts"), FORMATTED_TS).unwrap();

    let output = tsv(&["format", "--check", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
}

#[test]
fn test_format_error_isolation() {
    let dir = temp_dir("error_isolation");
    fs::write(dir.join("bad.ts"), "const x = \n").unwrap();
    fs::write(dir.join("good.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    // The valid file is still formatted despite the sibling parse error
    assert_eq!(
        fs::read_to_string(dir.join("good.ts")).unwrap(),
        FORMATTED_TS
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("bad.ts"));
}

#[test]
fn test_format_multiple_paths() {
    let dir1 = temp_dir("multi_1");
    let dir2 = temp_dir("multi_2");
    let file1 = dir1.join("a.ts");
    fs::write(&file1, UNFORMATTED_TS).unwrap();
    fs::write(dir2.join("b.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", file1.to_str().unwrap(), dir2.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(&file1).unwrap(), FORMATTED_TS);
    assert_eq!(fs::read_to_string(dir2.join("b.ts")).unwrap(), FORMATTED_TS);
}

#[test]
fn test_format_skips_write_when_unchanged() {
    let dir = temp_dir("skip_unchanged");
    let file = dir.join("a.ts");
    fs::write(&file, FORMATTED_TS).unwrap();
    // Read-only: any write attempt would fail, so exit 0 proves no write happened
    let mut perms = fs::metadata(&file).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&file, perms).unwrap();

    let output = tsv(&["format", dir.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_format_jobs_one() {
    let dir = temp_dir("jobs_one");
    for name in ["a.ts", "b.ts", "c.ts"] {
        fs::write(dir.join(name), UNFORMATTED_TS).unwrap();
    }

    let output = tsv(&["format", "--jobs", "1", dir.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for name in ["a.ts", "b.ts", "c.ts"] {
        assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), FORMATTED_TS);
    }
}

/// `--jobs 0` is a width, not an opt-out — it means the same as `--jobs 1`.
/// Both discovery paths must agree on that: a **directory** streams into the
/// pool and an **explicit file argument** collects first, and only the collected
/// one used to clamp. Unclamped, the streamed path left every file unclaimed and
/// reported a "worker thread panicked" error for a worker it never spawned.
#[test]
fn test_format_jobs_zero_means_one() {
    let dir = temp_dir("jobs_zero");
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("b.ts"), UNFORMATTED_TS).unwrap();

    let streamed = tsv(&["format", "--jobs", "0", dir.to_str().unwrap()]);
    assert_eq!(
        streamed.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&streamed.stderr)
    );
    assert_eq!(fs::read_to_string(dir.join("a.ts")).unwrap(), FORMATTED_TS);
    assert_eq!(fs::read_to_string(dir.join("b.ts")).unwrap(), FORMATTED_TS);

    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    let collected = tsv(&["format", "--jobs", "0", dir.join("a.ts").to_str().unwrap()]);
    assert_eq!(
        collected.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&collected.stderr)
    );
    assert_eq!(fs::read_to_string(dir.join("a.ts")).unwrap(), FORMATTED_TS);
}

/// A `--jobs` count the OS refuses must **narrow the pool**, not fail the run.
///
/// `--jobs` is a user-supplied number, and the pool's spawn used to `expect`, so this
/// was the one `format` argument that answered with a panic where every other bad one
/// exits 2 with a message. On the streamed path the panic was worse than a crash: it
/// unwound past the queue's `finish`, leaving every already-spawned worker parked on
/// the condvar while `thread::scope` waited to join them — the process hung holding
/// the whole pool's stacks. Both discovery paths are covered here: one directory root
/// streams, two spellings of it collect (the dedup is set-wide).
///
/// The refusal is provoked through **address space** (`ulimit -v`), which is the one
/// limit a worker's stack reservation is not free against — `cli/stack.rs` names it as
/// exactly that. `exec` hands the limit straight to tsv with no fork in between, and
/// ~98 MiB is under the 128 MiB that even the *narrowest* machine's `--jobs` ceiling
/// (4 workers × `STACK_SIZE`) reserves, so the refusal is reached whatever the core
/// count, while leaving tsv itself room to run (measured: it still formats at 48 MiB).
/// Both of the refusal's outcomes are a pass — the pool narrows to what it got, or it
/// comes up empty and the calling thread formats — and the run must be **clean either
/// way**.
///
/// The warning assertion is what keeps this test from passing vacuously: the first
/// spelling of it reached for `ulimit -u`, which `/bin/sh` (dash) does not support, so
/// the limit was never applied and the test passed against the *unfixed* code too.
/// Requiring the warning means a constraint that fails to bite fails the test instead
/// of quietly proving nothing.
///
/// Linux-only for the same reason: `RLIMIT_AS` is reliably enforced there, where macOS
/// largely ignores it — a `#[cfg(unix)]` test would be back to proving nothing on half
/// its platforms. `cargo test --workspace` gates on ubuntu.
#[cfg(target_os = "linux")]
#[test]
fn test_format_jobs_beyond_the_thread_limit_narrows_the_pool() {
    let dir = temp_dir("jobs_over_limit");
    // More files than the cap, so the collected path's clamp (to the file count) still
    // lands above it — both paths must ask for more threads than they can have.
    let names: Vec<String> = (0..40).map(|i| format!("f{i}.ts")).collect();
    let write_all = || {
        for name in &names {
            fs::write(dir.join(name), UNFORMATTED_TS).unwrap();
        }
    };
    let assert_all_formatted = || {
        for name in &names {
            assert_eq!(
                fs::read_to_string(dir.join(name)).unwrap(),
                FORMATTED_TS,
                "{name} was left unformatted"
            );
        }
    };
    let bin = built_tsv().display().to_string();
    let run = |targets: &str| {
        let script = format!("ulimit -v 100000; exec \"{bin}\" format --jobs 500 {targets}");
        let output = Command::new("sh")
            .arg("-c")
            .arg(script)
            .output()
            .expect("Failed to execute sh");
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            !stderr.contains("panicked"),
            "a refused thread must not panic: {stderr}"
        );
        // The limit must actually have bitten, or the run below proves nothing.
        assert!(
            stderr.contains("format workers started") || stderr.contains("could not start format"),
            "the pool was never refused a thread, so this run tested nothing: {stderr}"
        );
        // Both warnings this run provokes — the ceiling's and the pool's — are
        // diagnostics, so stdout stays the greppable list of changed paths.
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            !stdout.contains("warning:"),
            "a warning reached stdout, where only changed paths belong: {stdout}"
        );
        assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    };

    let target = dir.to_str().unwrap();
    write_all();
    run(&format!("\"{target}\""));
    assert_all_formatted();

    // The same root twice is the collected path — overlapping roots need the
    // canonical-path dedup, which needs the whole set in hand.
    write_all();
    run(&format!("\"{target}\" \"{target}\""));
    assert_all_formatted();
}

#[test]
fn test_format_nonexistent_path() {
    let output = tsv(&["format", "/nonexistent/tsv_cli_test_path"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("tsv_cli_test_path"));
}

/// An explicitly named file is held to the extension check before anything else: the
/// parser dispatch behind a path has no unknown arm, so without this
/// gate a `.json` file is parsed as TypeScript — usually a baffling syntax error,
/// and for a top-level-array JSON a *successful* rewrite into a TS expression
/// statement (semicolon and all), which is no longer valid JSON.
#[test]
fn test_format_explicit_file_rejects_unsupported_extension() {
    let dir = temp_dir("unsupported_extension");
    let json = dir.join("list.json");
    // valid TS as well as valid JSON — the case that used to be silently rewritten
    fs::write(&json, "[1,   2,    3]\n").unwrap();

    let output = tsv(&["format", json.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported file extension"),
        "stderr: {stderr}"
    );
    // the message names what tsv does format
    assert!(stderr.contains(".svelte"), "stderr: {stderr}");
    // untouched
    assert_eq!(fs::read_to_string(&json).unwrap(), "[1,   2,    3]\n");
}

/// The extension check is an *argument* error, so it fails the run upfront with
/// nothing written — the same contract as an unresolvable path — and every bad
/// argument is reported in one pass.
#[test]
fn test_format_unsupported_extension_fails_run_upfront() {
    let dir = temp_dir("unsupported_extension_upfront");
    let ts = dir.join("a.ts");
    fs::write(&ts, UNFORMATTED_TS).unwrap();
    let notes = dir.join("notes.md");
    fs::write(&notes, "# notes\n").unwrap();

    let output = tsv(&[
        "format",
        ts.to_str().unwrap(),
        notes.to_str().unwrap(),
        "/nonexistent/tsv_cli_upfront",
    ]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("notes.md"), "stderr: {stderr}");
    assert!(
        stderr.contains("not a file or directory"),
        "stderr: {stderr}"
    );
    // the valid sibling argument is not formatted — the run failed before any write
    assert_eq!(fs::read_to_string(&ts).unwrap(), UNFORMATTED_TS);
}

/// A **directory** argument is a scope, not a target, so the extension check
/// doesn't apply to it — its contents are filtered by the walk, which skips the
/// unsupported files rather than failing the run.
#[test]
fn test_format_directory_arg_skips_unsupported_extensions() {
    let dir = temp_dir("unsupported_extension_in_dir");
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("data.json"), "[1,   2,    3]\n").unwrap();

    let output = tsv(&["format", dir.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(dir.join("a.ts")).unwrap(), FORMATTED_TS);
    assert_eq!(
        fs::read_to_string(dir.join("data.json")).unwrap(),
        "[1,   2,    3]\n"
    );
}

#[test]
fn test_format_parser_flag_with_paths_errors() {
    let dir = temp_dir("parser_with_paths");
    let file = dir.join("a.ts");
    fs::write(&file, UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", "--parser", "typescript", file.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--parser"));
    // Nothing written
    assert_eq!(fs::read_to_string(&file).unwrap(), UNFORMATTED_TS);
}

#[test]
fn test_format_skips_hidden_dirs() {
    let dir = temp_dir("hidden_dirs");
    fs::create_dir_all(dir.join(".svelte-kit/types")).unwrap();
    fs::create_dir_all(dir.join(".hidden")).unwrap();
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join(".svelte-kit/types/x.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join(".hidden/y.ts"), UNFORMATTED_TS).unwrap();

    let output = tsv(&["format", dir.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(dir.join("a.ts")).unwrap(), FORMATTED_TS);
    // Hidden dirs (generated output like .svelte-kit) are not recursed
    assert_eq!(
        fs::read_to_string(dir.join(".svelte-kit/types/x.ts")).unwrap(),
        UNFORMATTED_TS
    );
    assert_eq!(
        fs::read_to_string(dir.join(".hidden/y.ts")).unwrap(),
        UNFORMATTED_TS
    );

    // An explicit hidden-dir argument is trusted and recursed
    let output = tsv(&["format", dir.join(".hidden").to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(dir.join(".hidden/y.ts")).unwrap(),
        FORMATTED_TS
    );
}

#[cfg(unix)]
#[test]
fn test_format_unreadable_subdir_reports_and_continues() {
    if !mode_bits_are_enforced() {
        return;
    }

    let dir = temp_dir("unreadable_subdir");
    let locked = dir.join("locked");
    fs::create_dir_all(&locked).unwrap();
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    let mode = set_mode(&locked, 0o000);

    let output = tsv(&["format", dir.to_str().unwrap()]);
    drop(mode);

    assert_eq!(output.status.code(), Some(2));
    // The sibling file is still formatted despite the traversal error
    assert_eq!(fs::read_to_string(dir.join("a.ts")).unwrap(), FORMATTED_TS);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("locked"), "stderr: {stderr}");
    assert!(!stderr.contains("Error: Error"), "stderr: {stderr}");
    assert!(stderr.contains("1 errors"), "stderr: {stderr}");
}

#[test]
fn test_format_dedup_overlapping_path_args() {
    let dir = temp_dir("dedup_overlap");
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    // Same dir under a second spelling that lexical comparison can't unify
    let alias = format!(
        "{}/../{}",
        dir.display(),
        dir.file_name().unwrap().to_str().unwrap()
    );

    let output = tsv(&["format", "--check", dir.to_str().unwrap(), &alias]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("1 would change"));
}

#[cfg(unix)]
#[test]
fn test_format_dedup_symlink_alias() {
    let dir = temp_dir("dedup_symlink");
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    let link =
        std::env::temp_dir().join(format!("tsv_cli_tests_dedup_link_{}", std::process::id()));
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(dir.path(), &link).unwrap();

    let output = tsv(&[
        "format",
        "--check",
        dir.to_str().unwrap(),
        link.to_str().unwrap(),
    ]);
    let _ = fs::remove_file(&link);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
}

/// Linux-only, not `#[cfg(unix)]`: APFS and HFS+ enforce valid UTF-8 in a filename, so
/// macOS refuses to CREATE the fixture (`EILSEQ`) and the case is unobservable there
/// rather than merely untested. `cargo test --workspace` gates on ubuntu.
#[cfg(target_os = "linux")]
#[test]
fn test_format_walks_non_utf8_names_by_their_own_bytes() {
    // a file or directory name that is not UTF-8 is joined onto the walk's paths as the
    // bytes it has: the lossy U+FFFD spelling the matcher reads names nothing on disk, so
    // emitting it would report a phantom `No such file or directory` and lose the subtree
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let dir = temp_dir("non_utf8_names");
    let file = dir.join(OsStr::from_bytes(b"fo\xffo.ts"));
    let sub = dir.join(OsStr::from_bytes(b"di\xffr"));
    fs::write(&file, UNFORMATTED_TS).unwrap();
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("g.ts"), UNFORMATTED_TS).unwrap();

    let list = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(
        list.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    assert!(
        list.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&list.stdout).lines().count(), 2);
    let lists = |needle: &[u8]| list.stdout.windows(needle.len()).any(|w| w == needle);
    assert!(
        lists(b"fo\xffo.ts"),
        "{}",
        String::from_utf8_lossy(&list.stdout)
    );
    assert!(
        lists(b"di\xffr/g.ts"),
        "{}",
        String::from_utf8_lossy(&list.stdout)
    );

    let output = tsv(&["format", dir.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), FORMATTED_TS);
    assert_eq!(fs::read_to_string(sub.join("g.ts")).unwrap(), FORMATTED_TS);
}

#[test]
fn test_format_jobs_above_the_ceiling_is_announced() {
    // an explicit --jobs past `4 × logical` is clamped, and said so — the flag means
    // "use this width", so a quietly narrower run would be the worse answer
    let dir = temp_dir("jobs_ceiling");
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    let output = tsv(&[
        "format",
        "--check",
        "--jobs",
        "1000000000",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: --jobs 1000000000 exceeds this machine's ceiling; using "),
        "stderr: {stderr}"
    );
}

#[test]
fn test_format_invalid_utf8_file_is_reported_and_left_alone() {
    // the read is strict UTF-8: a lossy one would write U+FFFD back over the author's
    // bytes and call the file formatted
    let dir = temp_dir("invalid_utf8_file");
    let bad = dir.join("bad.ts");
    let bytes = b"const x = '\xff';\n";
    fs::write(&bad, bytes).unwrap();
    fs::write(dir.join("ok.ts"), UNFORMATTED_TS).unwrap();
    let output = tsv(&["format", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("read failed: stream did not contain valid UTF-8"),
        "stderr: {stderr}"
    );
    assert_eq!(fs::read(&bad).unwrap(), bytes);
    // the error is isolated: the sibling still formats
    assert_eq!(fs::read_to_string(dir.join("ok.ts")).unwrap(), FORMATTED_TS);
}

/// `--jobs` sizes the pool that formats; `--list` spawns none, so a width there is the
/// same category error as with `--content`, refused the same way — not accepted silently
/// past the clamp warning.
#[test]
fn test_format_list_with_jobs_is_refused() {
    let dir = temp_dir("list_jobs");
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    let output = tsv(&["format", "--list", "--jobs", "2", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error: --jobs applies to formatting; --list reports the in-scope set without formatting"),
        "stderr: {stderr}"
    );
}

/// A write error that is neither a closed nor a slow reader — a full disk — aborts
/// loudly, with a verdict that is none of 0/1/2: a report lost with nothing said about it
/// would be worse than a crash. Pinned so the exit code stays out of the verdict range.
#[cfg(unix)]
#[test]
fn test_format_write_error_aborts_loudly() {
    use std::process::Stdio;
    let Ok(full) = fs::OpenOptions::new().write(true).open("/dev/full") else {
        return;
    };
    let dir = temp_dir("write_error");
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    let output = Command::new(built_tsv())
        .args(["format", "--list", "."])
        .current_dir(dir.path())
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .expect("run tsv");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !matches!(output.status.code(), Some(0..=2)),
        "a lost report must not read as a verdict: {:?}, stderr: {stderr}",
        output.status
    );
    assert!(
        stderr.contains("failed printing to stdout"),
        "stderr: {stderr}"
    );
}

/// Two hard links to one inode inside a walked tree are two names in scope: nothing reads
/// inodes, so each name is read and formatted on its own. A `--check` lists both; a
/// sequential format rewrites the inode through whichever name the walk reaches first
/// and finds the other already formatted, so it reports one name — which one is the
/// walk's order, not the sort's — and a second run has nothing left.
#[cfg(unix)]
#[test]
fn test_format_hard_links_are_two_names_in_scope() {
    let dir = temp_dir("hard_links");
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    fs::hard_link(dir.join("a.ts"), dir.join("b.ts")).unwrap();

    let check = tsv_in_dir(dir.path(), &["format", "--check", "."]);
    assert_eq!(
        check.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let stdout = String::from_utf8_lossy(&check.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), ["./a.ts", "./b.ts"]);

    let format = tsv_in_dir(dir.path(), &["format", "--jobs", "1", "."]);
    assert_eq!(
        format.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&format.stderr)
    );
    let stdout = String::from_utf8_lossy(&format.stdout);
    let changed: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        changed.len(),
        1,
        "one rewrite reaches both names, stdout: {stdout}"
    );
    assert!(
        changed[0] == "./a.ts" || changed[0] == "./b.ts",
        "stdout: {stdout}"
    );
    assert_eq!(fs::read_to_string(dir.join("a.ts")).unwrap(), FORMATTED_TS);
    assert_eq!(fs::read_to_string(dir.join("b.ts")).unwrap(), FORMATTED_TS);

    let again = tsv_in_dir(dir.path(), &["format", "."]);
    assert_eq!(again.status.code(), Some(0));
    assert!(
        again.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&again.stdout)
    );
}

/// Linux-only for the same reason as
/// `test_format_walks_non_utf8_names_by_their_own_bytes`: the fixture name cannot exist
/// on a macOS filesystem.
#[cfg(target_os = "linux")]
#[test]
fn test_format_non_utf8_argument_is_refused_at_the_argv_boundary() {
    // a path ARGUMENT that is not UTF-8 is refused before any command runs — argh reads
    // `&str`, so the walk's byte-faithful join reaches such a name only through a
    // directory. The refusal is a plain exit 1 with the name spelled lossily, not a panic
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let dir = temp_dir("non_utf8_argument");
    let file = dir.join(OsStr::from_bytes(b"fo\xffo.ts"));
    fs::write(&file, UNFORMATTED_TS).unwrap();
    let output = Command::new(built_tsv())
        .arg("format")
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("Invalid utf8: "), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
    assert_eq!(fs::read_to_string(&file).unwrap(), UNFORMATTED_TS);

    // the one diagnostic whose name the caller could not have cleaned is quoted like
    // every other printed path, so a line feed in it does not split the line
    let output = Command::new(built_tsv())
        .arg("format")
        .arg(OsStr::from_bytes(b"bad\nname\xff.ts"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Invalid utf8: \"bad\\nname\u{fffd}.ts\"\n"
    );
}

/// The listing's separators are the **host's**. Every other path assertion in this suite
/// runs the output through `to_posix` first, which is exactly what makes this pin
/// necessary: normalizing both sides answers "are these the same path", never "is the
/// spelling native", so a regression to `/` on Windows would pass the whole suite in
/// silence. Windows-only because it is the one host where the two spellings differ.
/// The contract itself is `cli::out::path_bytes`': a listed line must name the file on
/// disk, for a script to hand straight back to the shell.
#[cfg(windows)]
#[test]
fn test_format_lists_native_separators() {
    let dir = temp_dir("native_separators");
    fs::create_dir(dir.join("sub")).unwrap();
    fs::write(dir.join("sub").join("a.ts"), UNFORMATTED_TS).unwrap();

    let list = tsv_in_dir(dir.path(), &["format", "--list", "."]);
    assert_eq!(list.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains('\\'), "native separators: {stdout}");
    assert!(!stdout.contains('/'), "no posix separators: {stdout}");
}

/// The case of an extension is not a different kind of file — prettier infers a parser
/// from the lowercased name — so `A.TS` is walked, formats when named, dispatches to the
/// right parser (`App.SVELTE` is Svelte, not TypeScript), and settles the module goal
/// (`legacy.MJS`) as its lowercase spelling does, on both commands.
#[test]
fn test_extensions_are_read_without_regard_to_case() {
    let dir = temp_dir("extension_case");
    fs::write(dir.join("A.TS"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("App.SVELTE"), "<div   >x</div>\n").unwrap();
    fs::write(dir.join("styles.CSS"), "a{color:red}\n").unwrap();
    fs::write(dir.join("legacy.MJS"), "with (a) {}\n").unwrap();
    fs::write(dir.join("README.MD"), "# not code\n").unwrap();

    let list = tsv_in_dir(dir.path(), &["format", "--list", "."]);
    assert_eq!(list.status.code(), Some(0));
    assert_eq!(
        to_posix(&String::from_utf8_lossy(&list.stdout)),
        "./A.TS\n./App.SVELTE\n./legacy.MJS\n./styles.CSS\n"
    );

    let named = tsv_in_dir(dir.path(), &["format", "--check", "A.TS", "App.SVELTE"]);
    assert_eq!(
        named.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&named.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&named.stdout), "A.TS\nApp.SVELTE\n");

    // a module by its own name takes no script retry, whatever its case
    let module = tsv_in_dir(dir.path(), &["format", "--check", "legacy.MJS"]);
    assert_eq!(module.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&module.stderr).contains("error: legacy.MJS:"),
        "{}",
        String::from_utf8_lossy(&module.stderr)
    );

    let md = tsv_in_dir(dir.path(), &["format", "README.MD"]);
    assert_eq!(md.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&md.stderr).contains("unsupported file extension"),
        "{}",
        String::from_utf8_lossy(&md.stderr)
    );

    let parsed = tsv_in_dir(dir.path(), &["parse", "styles.CSS"]);
    assert_eq!(
        parsed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&parsed.stderr)
    );
    assert!(String::from_utf8_lossy(&parsed.stdout).contains("\"type\":\"StyleSheetFile\""));
}

#[cfg(unix)]
#[test]
fn test_format_does_not_follow_a_symlink_cycle() {
    // a directory symlink pointing back up the tree is not followed: the walk terminates
    // and lists each real file once. `test_format_dedup_symlink_alias` covers a link
    // NAMED as an argument; this is the link a walk discovers
    let dir = temp_dir("symlink_cycle");
    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("a.ts"), UNFORMATTED_TS).unwrap();
    std::os::unix::fs::symlink("..", sub.join("loop")).unwrap();
    std::os::unix::fs::symlink(".", sub.join("self")).unwrap();

    let list = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(
        list.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains("sub/a.ts"), "stdout: {stdout}");

    let output = tsv(&["format", dir.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(fs::read_to_string(sub.join("a.ts")).unwrap(), FORMATTED_TS);
}

#[test]
fn test_format_missing_arg_fails_fast() {
    let dir = temp_dir("missing_fail_fast");
    let file = dir.join("a.ts");
    fs::write(&file, UNFORMATTED_TS).unwrap();

    let output = tsv(&[
        "format",
        "/nonexistent/tsv_missing_one",
        "/nonexistent/tsv_missing_two",
        file.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    // Every bad argument is reported, and nothing is written
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("tsv_missing_one"), "stderr: {stderr}");
    assert!(stderr.contains("tsv_missing_two"), "stderr: {stderr}");
    assert_eq!(fs::read_to_string(&file).unwrap(), UNFORMATTED_TS);
}

#[test]
fn test_format_list_is_readonly_and_exit_codes() {
    // --list is a read-only binary contract: it prints the in-scope set, writes
    // nothing, and exits 0 — including for an all-ignored (empty) target, unlike
    // the format action which treats "nothing found" as a usage error (exit 2).
    // *Which* files the ignore files admit is pinned for both CLIs by the shared
    // table in tests/discovery_parity.rs; this only covers the --list contract.
    let dir = git_repo("list_readonly");
    fs::write(dir.join(".gitignore"), "build/\n").unwrap();
    fs::create_dir_all(dir.join("build")).unwrap();
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    fs::write(dir.join("build/out.ts"), UNFORMATTED_TS).unwrap();

    let listed = tsv(&["format", "--list", dir.to_str().unwrap()]);
    assert_eq!(
        listed.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&listed.stderr)
    );
    assert!(String::from_utf8_lossy(&listed.stdout).contains("a.ts"));
    // --list never writes — the listed file is left exactly as-is
    assert_eq!(
        fs::read_to_string(dir.join("a.ts")).unwrap(),
        UNFORMATTED_TS
    );

    // an all-ignored target lists nothing and still exits 0
    let empty = tsv(&["format", "--list", dir.join("build").to_str().unwrap()]);
    assert_eq!(
        empty.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&empty.stderr)
    );
    assert!(String::from_utf8_lossy(&empty.stdout).trim().is_empty());
}

#[test]
fn test_format_list_rejects_check_and_single_mode() {
    // --list is path-mode and output-only: it can't combine with --check, and
    // --content/--stdin have nothing to discover
    let combo = tsv(&["format", "--list", "--check", "."]);
    assert_eq!(combo.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&combo.stderr).contains("--list and --check"),
        "stderr: {}",
        String::from_utf8_lossy(&combo.stderr)
    );

    let single = tsv(&[
        "format",
        "--list",
        "--content",
        "const x=1",
        "--parser",
        "typescript",
    ]);
    assert_eq!(single.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&single.stderr).contains("--list applies to file paths"),
        "stderr: {}",
        String::from_utf8_lossy(&single.stderr)
    );
}

/// Nesting depth for the stack-ceiling tests below.
///
/// Chosen against the two profiles that bound it. The floor: a 1 MiB main-thread stack
/// — what Windows gives every process, the linker writing it into the executable
/// header — is worth roughly **50** levels in a **debug** build, the profile `cargo test`
/// runs. That one is derived rather than measured (1 MiB over the ~21 KiB a nesting
/// level measures at in debug), since every route reserves its own stack and none is
/// left to read an inherited one off; Windows frames differ somewhat, so read it as the
/// order of magnitude either way. Anything past that fails on a route that inherits the
/// platform default. The ceiling: the reservation in `cli::stack` clears ~1,570 levels in
/// that same debug build, so 200 leaves room for per-target frame differences while
/// staying far out of reach of an unsized thread.
const NESTED_DEPTH: usize = 200;

/// `const x = ((((…1…))));` nested [`NESTED_DEPTH`] deep — the cheapest input whose
/// cost is linear in one number, and the shape the parser and the printer both recurse
/// on.
fn deeply_nested_ts() -> String {
    format!(
        "const x = {}1{};\n",
        "(".repeat(NESTED_DEPTH),
        ")".repeat(NESTED_DEPTH)
    )
}

/// Assert the run neither aborted nor crashed, whatever it decided about the input.
///
/// A stack overflow is not a panic and not an error exit — it kills the process by
/// signal, so `status.code()` is `None` on Unix and a large value on Windows. The
/// contract under test is only "reached a verdict"; which verdict is the other tests'
/// business.
fn assert_reached_a_verdict(label: &str, out: &std::process::Output) {
    let code = out.status.code();
    assert!(
        matches!(code, Some(0 | 1)),
        "{label}: expected a verdict, got {code:?}; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn test_deeply_nested_input_survives_every_route() {
    // Every route runs on the one reservation `cli::stack` states, so the depth a
    // route reaches cannot depend on which route it is. Before that was true, the
    // pool reserved its own stack while `--content`/`--stdin` and all of `parse`
    // inherited the platform default — an 8x difference between two routes of one
    // binary on one Windows machine, where the default is 1 MiB.
    let src = deeply_nested_ts();
    let dir = temp_dir("deep_nesting");
    let file = dir.join("deep.ts");
    fs::write(&file, &src).unwrap();
    let path = file.to_str().unwrap();

    // `--content` and `--stdin`: the routes that never reach the format pool.
    assert_reached_a_verdict(
        "format --content",
        &tsv(&[
            "format",
            "--check",
            "--parser",
            "typescript",
            "--content",
            &src,
        ]),
    );
    assert_reached_a_verdict(
        "format --stdin",
        &tsv_stdin(
            &["format", "--check", "--parser", "typescript", "--stdin"],
            &src,
        ),
    );
    // A file path: the format pool.
    assert_reached_a_verdict("format <path>", &tsv(&["format", "--check", path]));
    // `parse`, which has no pool at all and runs a second recursion (the wire-JSON
    // writer) on the same stack.
    assert_reached_a_verdict("parse <path>", &tsv(&["parse", path]));
    assert_reached_a_verdict(
        "parse --content",
        &tsv(&["parse", "--parser", "typescript", "--content", &src]),
    );
}

/// The Unix half of the reservation check: the same depth under a **1 MiB** main-thread
/// stack, which is what Windows gives every process and what `ulimit -s` can reproduce
/// here.
///
/// Without this the sibling test above is vacuous on Linux and macOS — a dev box hands
/// the main thread 8 MiB or more, which already clears [`NESTED_DEPTH`] whether or not
/// the reservation is in effect, so only the Windows CI leg would ever grade it. Under
/// 1 MiB the inherited stack reaches under 30 levels in this profile, so a route that
/// lost its reservation aborts here instead.
///
/// `ulimit` is a shell builtin, so this shells out; a machine without a working
/// `sh -c 'ulimit -s'` skips rather than fails, since the property under test is the
/// binary's, not the shell's.
#[cfg(unix)]
#[test]
fn test_deeply_nested_input_survives_a_1mib_main_stack() {
    let src = deeply_nested_ts();
    let bin = built_tsv();
    let bin = bin.to_str().unwrap();

    // Prove the harness itself works before trusting a pass out of it: a shell that
    // silently ignores `ulimit -s` would make every assertion below vacuous.
    let probe = Command::new("sh")
        .args(["-c", "ulimit -s 1024 && ulimit -s"])
        .output();
    let Ok(probe) = probe else {
        eprintln!("skipping: no usable `sh`");
        return;
    };
    if String::from_utf8_lossy(&probe.stdout).trim() != "1024" {
        eprintln!("skipping: `sh` did not apply `ulimit -s 1024`");
        return;
    }

    for (label, args) in [
        (
            "format --content",
            vec!["format", "--check", "--parser", "typescript", "--content"],
        ),
        (
            "parse --content",
            vec!["parse", "--parser", "typescript", "--content"],
        ),
    ] {
        let mut argv = vec!["-c", "ulimit -s 1024; exec \"$@\"", "--", bin];
        argv.extend_from_slice(&args);
        argv.push(&src);
        let out = Command::new("sh").args(&argv).output().unwrap();
        assert_reached_a_verdict(&format!("{label} @ 1 MiB"), &out);
    }
}

#[test]
fn test_format_path_falls_back_to_script() {
    // Path mode leaves the source type unset, so a file the module grammar rejects is
    // retried as a script: `with` and a `var await` binding are sloppy-script-only.
    let dir = temp_dir("format_path_script_fallback");
    let file = dir.join("legacy.js");
    fs::write(&file, "var   await=1;\nwith (a) {\n\tb;\n}\n").expect("write temp file");
    let path = file.to_str().expect("utf8 path");

    let output = tsv(&["format", path]);
    assert!(
        output.status.success(),
        "a script-only file must format through the fallback: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let formatted = fs::read_to_string(&file).expect("read formatted file");
    assert!(
        formatted.contains("var await = 1;"),
        "should format the script-only binding: {formatted}"
    );
    assert!(
        formatted.contains("with (a) {"),
        "should keep the with statement: {formatted}"
    );

    // Second pass: the formatted file is its own fixed point, so nothing changes.
    let again = tsv(&["format", path]);
    assert!(again.status.success(), "second pass should succeed");
    assert_eq!(
        fs::read_to_string(&file).expect("read twice-formatted file"),
        formatted,
        "the fallback's output must be byte-stable"
    );
    assert!(
        String::from_utf8_lossy(&again.stdout).trim().is_empty(),
        "an unchanged file prints no path"
    );
}

#[test]
fn test_format_path_both_goals_fail_reports_the_module_error() {
    // Broken under both grammars: `with` fails the module parse, the `import`
    // declaration fails the script retry — a goal gate, which settles the file as a
    // module, so the module's error is the reported one, at line 2
    // (`tests/format_fallback_error_attribution.rs` pins the rule; this pins the CLI's
    // rendering of it).
    let dir = temp_dir("format_path_both_fail");
    let file = dir.join("broken.js");
    fs::write(&file, "import x from 'y';\nwith (a) {\n\tb;\n}\n").expect("write temp file");

    let output = tsv(&["format", file.to_str().expect("utf8 path")]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "a file invalid under both source types is an error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("The 'with' statement is not allowed in strict mode"),
        "should report the module parse error: {stderr}"
    );
    assert!(
        !stderr.contains("'import' is only allowed in a module"),
        "should not report the script retry's goal-gate error: {stderr}"
    );
}

#[test]
fn test_format_path_module_only_extension_takes_no_script_retry() {
    // `.mjs` and `.mts` are ES modules by their own name (Node loads a `.mjs` as ESM
    // unconditionally; tsc maps both to `ModuleKind.ESNext`), so the module-then-script
    // fallback has nothing to fall back to — `tsv_ts::Goal::from_extension`. The same
    // bytes in a `.js` still format, which is what makes this the extension's doing and
    // not a parser change.
    let dir = temp_dir("format_path_module_only_ext");
    let sloppy = "with (a) {\n\tb;\n}\n";

    for name in ["a.mjs", "a.mts"] {
        let file = dir.join(name);
        fs::write(&file, sloppy).expect("write temp file");
        let output = tsv(&["format", file.to_str().expect("utf8 path")]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name} is a module by extension, so a sloppy-script body is an error"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("The 'with' statement is not allowed in strict mode"),
            "{name} should report the module parse error: {stderr}"
        );
        assert_eq!(
            fs::read_to_string(&file).expect("read back"),
            sloppy,
            "{name} must be left untouched"
        );
    }

    for name in ["a.js", "a.ts", "a.cjs", "a.cts"] {
        let file = dir.join(name);
        fs::write(&file, sloppy).expect("write temp file");
        let output = tsv(&["format", file.to_str().expect("utf8 path")]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{name} settles no goal, so the script retry still reaches it"
        );
    }
}

#[test]
fn test_format_path_module_only_extension_still_formats_valid_module() {
    // The narrowing touches only sources the MODULE grammar rejects: a `.mjs` that is
    // a module formats exactly as before.
    let dir = temp_dir("format_path_module_only_ext_valid");
    let file = dir.join("a.mjs");
    fs::write(&file, "import x from 'y';\nconst   z=1\n").expect("write temp file");

    let output = tsv(&["format", file.to_str().expect("utf8 path")]);

    assert_eq!(output.status.code(), Some(0), "a valid module must format");
    assert_eq!(
        fs::read_to_string(&file).expect("read back"),
        "import x from 'y';\nconst z = 1;\n",
        "the file must be formatted in place"
    );
}

/// A file the worker cannot read, and one it cannot write back, are each ONE per-file
/// error — reported on stderr with the failing step named, counted in the summary, and
/// stepped over: the sibling still formats, the read-only file keeps its bytes, and the
/// run exits 2. A `--check` over the same tree writes nothing, so the read-only file is
/// simply one that would change, and only the unreadable one is an error.
#[cfg(unix)]
#[test]
fn test_format_per_file_read_and_write_failures_report_and_continue() {
    if !mode_bits_are_enforced() {
        return;
    }

    let dir = temp_dir("file_permissions");
    fs::write(dir.join("noread.ts"), UNFORMATTED_TS).unwrap();
    let noread_mode = set_mode(&dir.join("noread.ts"), 0o000);
    fs::write(dir.join("readonly.ts"), UNFORMATTED_TS).unwrap();
    let readonly_mode = set_mode(&dir.join("readonly.ts"), 0o444);
    fs::write(dir.join("ok.ts"), UNFORMATTED_TS).unwrap();

    let check = tsv(&["format", "--check", dir.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert_eq!(check.status.code(), Some(2), "stderr: {stderr}");
    assert!(
        stderr.contains("noread.ts: read failed: "),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains("write failed"), "stderr: {stderr}");
    assert!(
        stderr.contains("2 would change, 0 unchanged, 1 errors"),
        "stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&check.stdout);
    let listed: Vec<_> = stdout
        .lines()
        .map(|line| to_posix(line).rsplit('/').next().unwrap().to_string())
        .collect();
    assert_eq!(listed, ["ok.ts", "readonly.ts"], "stdout: {stdout}");

    let output = tsv(&["format", dir.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr: {stderr}");
    assert!(
        stderr.contains("noread.ts: read failed: "),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("readonly.ts: write failed: "),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("1 formatted, 0 unchanged, 2 errors"),
        "stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().count() == 1 && stdout.contains("ok.ts"),
        "only the formatted file is reported: {stdout}"
    );

    drop(noread_mode);
    drop(readonly_mode);
    assert_eq!(fs::read_to_string(dir.join("ok.ts")).unwrap(), FORMATTED_TS);
    assert_eq!(
        fs::read_to_string(dir.join("readonly.ts")).unwrap(),
        UNFORMATTED_TS,
        "a failed write leaves the file as it was"
    );
    assert_eq!(
        fs::read_to_string(dir.join("noread.ts")).unwrap(),
        UNFORMATTED_TS
    );
}

/// A dangling symbolic link named as an argument is a bad argument — `not a file or
/// directory`, since what it would be graded through does not exist — and a bad
/// argument fails the run upfront, with `--list` and without: the valid file named
/// beside it is left unformatted. `parse` reaches the read and reports that instead.
#[cfg(unix)]
#[test]
fn test_dangling_symlink_argument_fails_the_run_upfront() {
    let dir = temp_dir("dangling_symlink");
    std::os::unix::fs::symlink("nowhere.ts", dir.join("dangling.ts")).unwrap();
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    let dangling = dir.join("dangling.ts");
    let a = dir.join("a.ts");

    for args in [
        vec!["format", dangling.to_str().unwrap(), a.to_str().unwrap()],
        vec![
            "format",
            "--list",
            dangling.to_str().unwrap(),
            a.to_str().unwrap(),
        ],
    ] {
        let output = tsv(&args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(
            stderr.contains("dangling.ts: not a file or directory"),
            "{args:?}: {stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "{args:?}: nothing listed or reported"
        );
    }
    assert_eq!(
        fs::read_to_string(&a).unwrap(),
        UNFORMATTED_TS,
        "a bad argument fails the run before anything is written"
    );

    let parsed = tsv(&["parse", dangling.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&parsed.stderr);
    assert_eq!(parsed.status.code(), Some(1), "stderr: {stderr}");
    assert!(
        stderr.contains("Error: Error reading file ") && stderr.contains("dangling.ts"),
        "stderr: {stderr}"
    );
}

/// `--list` reports a traversal error the way the format action does — the unreadable
/// directory on stderr, exit 2 — and still lists everything the walk reached.
#[cfg(unix)]
#[test]
fn test_format_list_traversal_error_still_lists_and_exits_two() {
    if !mode_bits_are_enforced() {
        return;
    }

    let dir = temp_dir("list_unreadable_subdir");
    let locked = dir.join("locked");
    fs::create_dir_all(&locked).unwrap();
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    let mode = set_mode(&locked, 0o000);

    let output = tsv(&["format", "--list", dir.to_str().unwrap()]);
    drop(mode);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr: {stderr}");
    assert!(
        stderr.contains("locked: read_dir failed: "),
        "stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().count() == 1 && stdout.contains("a.ts"),
        "the reachable file is still listed: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(dir.join("a.ts")).unwrap(),
        UNFORMATTED_TS
    );
}
