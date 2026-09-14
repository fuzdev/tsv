//! `tsv format` in single-input mode (`--content` / `--stdin`): the per-language
//! routes, `--check`, the `--source-type` goal axis and its refusals, the flags path
//! mode owns, and the byte-level input contracts (empty input, CRLF folding, invalid
//! UTF-8, content outranking stdin).

use std::fs;
use std::process::Command;

use crate::common::{FORMATTED_TS, UNFORMATTED_TS, built_tsv, temp_dir, tsv, tsv_stdin};

#[test]
fn test_format_command_typescript() {
    let output = tsv(&[
        "format",
        "--content",
        "const    x    =    42;",
        "--parser",
        "typescript",
    ]);

    assert!(output.status.success(), "Format command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should normalize whitespace
    assert!(
        stdout.contains("const x = 42;"),
        "Should format TypeScript code"
    );
}

#[test]
fn test_format_command_svelte() {
    let output = tsv(&[
        "format",
        "--content",
        "<div>test</div>",
        "--parser",
        "svelte",
    ]);

    assert!(output.status.success(), "Format command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("<div>test</div>"),
        "Should format Svelte code"
    );
}

#[test]
fn test_format_command_css() {
    let output = tsv(&["format", "--content", "body{color:red;}", "--parser", "css"]);

    assert!(output.status.success(), "Format command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should format CSS with proper spacing
    assert!(stdout.contains("color: red;"), "Should format CSS code");
}

#[test]
fn test_format_source_type_script() {
    // `await => 1` is a single-param arrow at Script goal; formats with arrowParens.
    let output = tsv(&[
        "format",
        "--content",
        "await => 1;",
        "--parser",
        "typescript",
        "--source-type",
        "script",
    ]);

    assert!(output.status.success(), "Script-goal format should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("(await) => 1;"),
        "Should format the `await` arrow param: {stdout}"
    );
}

#[test]
fn test_format_source_type_invalid_value() {
    let output = tsv(&[
        "format",
        "--content",
        "x;",
        "--parser",
        "typescript",
        "--source-type",
        "bogus",
    ]);

    // format uses exit 2 for argument/usage errors (distinct from parse's 1).
    assert_eq!(
        output.status.code(),
        Some(2),
        "Invalid --source-type should exit 2 for format"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid --source-type"),
        "Should report invalid --source-type: {stderr}"
    );
}

#[test]
fn test_format_source_type_rejected_in_path_mode() {
    // `--source-type` is content/stdin-only; with a path argument it's a usage error
    // (path mode resolves the source type per file — see the fallback test above).
    let dir = temp_dir("format_source_type_path");
    let file = dir.join("a.ts");
    fs::write(&file, "const x = 1;\n").expect("write temp file");

    let output = tsv(&[
        "format",
        "--source-type",
        "script",
        file.to_str().expect("utf8 path"),
    ]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "`--source-type` with a path should be a usage error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--source-type applies to --content/--stdin"),
        "Should explain the path-mode restriction: {stderr}"
    );
}

#[test]
fn test_format_missing_parser() {
    let output = tsv(&["format", "--content", "<div>test</div>"]);

    assert!(
        !output.status.success(),
        "Format without --parser should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--parser") || stderr.contains("Error"),
        "Should report missing parser option"
    );
}

#[test]
fn test_format_check_content_dirty_exits_one() {
    let output = tsv(&[
        "format",
        "--check",
        "--content",
        "const   x   =   1;\n",
        "--parser",
        "typescript",
    ]);
    assert_eq!(output.status.code(), Some(1));
    // Check mode never prints content
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
}

#[test]
fn test_format_check_content_clean_exits_zero() {
    let output = tsv(&[
        "format",
        "--check",
        "--content",
        FORMATTED_TS,
        "--parser",
        "typescript",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
}

#[test]
fn test_format_jobs_with_content_errors() {
    let output = tsv(&[
        "format",
        "--jobs",
        "2",
        "--content",
        FORMATTED_TS,
        "--parser",
        "typescript",
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--jobs"));
}

/// A `--source-type` the parser has no goal for is refused before stdin is read, so the
/// refusal cannot hang on a writer that has not finished — an open, empty stdin here.
#[test]
fn test_stdin_source_type_refusal_does_not_wait_for_input() {
    use std::process::Stdio;
    for (args, code) in [
        (
            &[
                "format",
                "--stdin",
                "--parser",
                "css",
                "--source-type",
                "module",
            ][..],
            2,
        ),
        (
            &[
                "parse",
                "--stdin",
                "--parser",
                "svelte",
                "--source-type",
                "script",
            ][..],
            1,
        ),
    ] {
        let mut child = Command::new(built_tsv())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn tsv");
        // stdin stays open and empty: a refusal that read first would wait here forever
        let started = std::time::Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().expect("try_wait") {
                break status;
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(5),
                "{args:?} waited on stdin instead of refusing"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let output = child.wait_with_output().expect("wait");
        assert_eq!(status.code(), Some(code), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("--source-type is only supported for typescript"),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn test_format_stdin_to_stdout() {
    let output = tsv_stdin(
        &["format", "--stdin", "--parser", "typescript"],
        UNFORMATTED_TS,
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), FORMATTED_TS);
}

#[test]
fn test_format_check_stdin_dirty_exits_one() {
    // --check + --stdin (editor-integration path): unformatted input exits 1.
    let output = tsv_stdin(
        &["format", "--check", "--stdin", "--parser", "typescript"],
        UNFORMATTED_TS,
    );
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn test_format_content_with_paths_errors() {
    // --content and --stdin cannot be combined with file path arguments — refused
    // before anything is read, so the --stdin arm never waits on its writer.
    for args in [
        vec![
            "format",
            "--content",
            "const x=1;",
            "--parser",
            "typescript",
            "somefile.ts",
        ],
        vec!["format", "--stdin", "--parser", "typescript", "somefile.ts"],
    ] {
        let output = tsv(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("--content/--stdin cannot be combined with file paths"),
            "{args:?}: stderr should explain the conflict: {stderr}"
        );
    }
}

#[test]
fn test_format_source_type_module_rejects_script_only_content() {
    // A SET source type is exact — no fallback. `with` stays a strict-mode error.
    let output = tsv(&[
        "format",
        "--content",
        "with (a) { b; }",
        "--parser",
        "typescript",
        "--source-type",
        "module",
    ]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "an explicit module source type must not fall back"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("The 'with' statement is not allowed in strict mode"),
        "should report the strict-mode error: {stderr}"
    );
}

#[test]
fn test_format_content_unset_source_type_falls_back_to_script() {
    // The `--content` arm is a different call site from path mode
    // (`format_source_with_goal_option`, not `format_source_in`), so the fallback is
    // pinned there too: an UNSET source type formats a script-only source.
    let output = tsv(&[
        "format",
        "--content",
        "with (a) { b }",
        "--parser",
        "typescript",
    ]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "an unset source type must retry as a script: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "with (a) {\n\tb;\n}\n"
    );
}

#[test]
fn test_source_type_refused_on_a_goalless_language() {
    // Svelte hard-wires `Module` and CSS has no goal, so a `--source-type` there is
    // a request that cannot be honored — an error on both commands, as on every
    // binding, never a silent drop. The exit code is each command's own usage code.
    let parse = tsv(&[
        "parse",
        "--content",
        "<div />",
        "--parser",
        "svelte",
        "--source-type",
        "script",
    ]);
    assert_eq!(
        parse.status.code(),
        Some(1),
        "parse: --source-type on svelte is an error"
    );
    let stderr = String::from_utf8_lossy(&parse.stderr);
    assert!(
        stderr.contains("--source-type is only supported for typescript"),
        "parse: should name the restriction: {stderr}"
    );

    let format = tsv(&[
        "format",
        "--content",
        "a {}",
        "--parser",
        "css",
        "--source-type",
        "module",
    ]);
    assert_eq!(
        format.status.code(),
        Some(2),
        "format: --source-type on css is an error"
    );
    let stderr = String::from_utf8_lossy(&format.stderr);
    assert!(
        stderr.contains("--source-type is only supported for typescript"),
        "format: should name the restriction: {stderr}"
    );

    // A path's extension settles the parser ahead of the read, so the refusal does not
    // turn on whether the file exists: a missing `.css` names the flag, not the file.
    let missing = tsv(&["parse", "definitely_missing.css", "--source-type", "script"]);
    assert_eq!(missing.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        stderr.contains("--source-type is only supported for typescript"),
        "parse: the flag is graded before the read: {stderr}"
    );

    // The value is still validated first, whatever the language.
    let bad = tsv(&[
        "parse",
        "--content",
        "a {}",
        "--parser",
        "css",
        "--source-type",
        "commonjs",
    ]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("invalid --source-type"),
        "an invalid value is reported as such"
    );
}

/// An empty input is its own fixed point on every route: `--content ''` prints nothing
/// (no newline is appended to nothing), `--check` reads it as clean, an empty file in
/// path mode is unchanged and left alone, and `parse` emits the empty program.
#[test]
fn test_empty_input_is_its_own_fixed_point() {
    for parser in ["typescript", "css", "svelte"] {
        let output = tsv(&["format", "--content", "", "--parser", parser]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{parser}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "{parser}: stdout {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
        let check = tsv(&["format", "--check", "--content", "", "--parser", parser]);
        assert_eq!(check.status.code(), Some(0), "{parser}: --check");
    }

    let dir = temp_dir("empty_input");
    let names = ["a.ts", "b.css", "c.svelte"];
    for name in names {
        fs::write(dir.join(name), "").unwrap();
    }
    let output = tsv(&["format", dir.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    assert!(
        output.stdout.is_empty(),
        "nothing changed: {:?}",
        output.stdout
    );
    assert!(
        stderr.contains("0 formatted, 3 unchanged"),
        "stderr: {stderr}"
    );
    for name in names {
        assert_eq!(
            fs::read(dir.join(name)).unwrap(),
            b"",
            "{name} is left empty"
        );
    }

    let parsed = tsv(&["parse", "--content", "", "--parser", "typescript"]);
    assert_eq!(parsed.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&parsed.stdout).contains(r#""body":[]"#),
        "the empty program: {}",
        String::from_utf8_lossy(&parsed.stdout)
    );
}

/// `format` folds `<CR><LF>` to `<LF>` ahead of the parse on every route
/// (`format_source_in`), so its output is LF-only in every language — which makes a file
/// whose only difference from its formatted form is the line terminator a *change*:
/// `--check` exits 1 on it and a write rewrites it. `parse` takes the author's bytes as
/// they are: its offsets count each `<CR>`.
#[test]
fn test_format_folds_crlf_to_lf_on_every_route_and_parse_keeps_it() {
    const CRLF_TS: &str = "const a = 1;\r\nconst b = 2;\r\n";
    const LF_TS: &str = "const a = 1;\nconst b = 2;\n";
    for (parser, crlf, lf) in [
        ("typescript", CRLF_TS, LF_TS),
        ("css", "a {\r\n\tb: c;\r\n}\r\n", "a {\n\tb: c;\n}\n"),
        (
            "svelte",
            "<div>a</div>\r\n<p>b</p>\r\n",
            "<div>a</div>\n<p>b</p>\n",
        ),
    ] {
        let content = tsv(&["format", "--content", crlf, "--parser", parser]);
        assert_eq!(
            content.status.code(),
            Some(0),
            "{parser}: {}",
            String::from_utf8_lossy(&content.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&content.stdout),
            lf,
            "{parser}: --content"
        );
        let stdin = tsv_stdin(&["format", "--stdin", "--parser", parser], crlf);
        assert_eq!(
            String::from_utf8_lossy(&stdin.stdout),
            lf,
            "{parser}: --stdin"
        );
        // a CRLF source is not its own fixed point
        let check = tsv(&["format", "--check", "--content", crlf, "--parser", parser]);
        assert_eq!(check.status.code(), Some(1), "{parser}: --check");
    }

    let dir = temp_dir("crlf");
    fs::write(dir.join("a.ts"), CRLF_TS).unwrap();
    let check = tsv(&["format", "--check", dir.to_str().unwrap()]);
    assert_eq!(check.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&check.stdout).contains("a.ts"),
        "the terminator alone is a change: {}",
        String::from_utf8_lossy(&check.stdout)
    );
    let written = tsv(&["format", dir.to_str().unwrap()]);
    assert_eq!(written.status.code(), Some(0));
    assert_eq!(fs::read_to_string(dir.join("a.ts")).unwrap(), LF_TS);
    let again = tsv(&["format", "--check", dir.to_str().unwrap()]);
    assert_eq!(
        again.status.code(),
        Some(0),
        "a second run has nothing left"
    );

    let parsed = tsv(&["parse", "--content", CRLF_TS, "--parser", "typescript"]);
    assert_eq!(parsed.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&parsed.stdout)
            .starts_with(r#"{"type":"Program","start":0,"end":28,"#),
        "parse counts the CRs: {}",
        String::from_utf8_lossy(&parsed.stdout)
    );
}

/// Reading is strict UTF-8 on every input arm, each with its own message: `--stdin`
/// refuses the stream (exit 2 for `format`, 1 for `parse` — each command's own
/// argument-error code), and `parse <file>` refuses the file the way `format <file>`
/// does (`test_format_invalid_utf8_file_is_reported_and_left_alone`). Nothing is
/// repaired with U+FFFD and parsed anyway.
#[test]
fn test_invalid_utf8_on_stdin_and_a_parsed_file_is_refused() {
    use std::io::Write;
    use std::process::Stdio;
    const BAD: &[u8] = b"const a = '\xff';\n";

    for (args, code) in [
        (["format", "--stdin", "--parser", "typescript"], 2),
        (["parse", "--stdin", "--parser", "typescript"], 1),
    ] {
        let mut child = Command::new(built_tsv())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn tsv");
        child
            .stdin
            .take()
            .expect("child stdin")
            .write_all(BAD)
            .expect("write stdin");
        let output = child.wait_with_output().expect("wait");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(code), "{args:?}: {stderr}");
        assert!(
            stderr.contains("Error: Error reading from stdin: stream did not contain valid UTF-8"),
            "{args:?}: {stderr}"
        );
        assert!(output.stdout.is_empty(), "{args:?}: nothing is parsed");
    }

    let dir = temp_dir("invalid_utf8_parse");
    fs::write(dir.join("bad.ts"), BAD).unwrap();
    let parsed = tsv(&["parse", dir.join("bad.ts").to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&parsed.stderr);
    assert_eq!(parsed.status.code(), Some(1), "stderr: {stderr}");
    assert!(
        stderr.contains("Error: Error reading file ")
            && stderr.contains("bad.ts: stream did not contain valid UTF-8"),
        "stderr: {stderr}"
    );
    assert!(parsed.stdout.is_empty());
}

/// `--content` outranks `--stdin` when both are named: the content is formatted and
/// stdin is never read. Pinned with a stdin that stays open and empty, where a read
/// would wait forever.
#[test]
fn test_content_outranks_stdin_without_reading_it() {
    use std::process::Stdio;
    let mut child = Command::new(built_tsv())
        .args([
            "format",
            "--content",
            UNFORMATTED_TS,
            "--parser",
            "typescript",
            "--stdin",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tsv");
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "waited on stdin although --content was given"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let output = child.wait_with_output().expect("wait");
    assert_eq!(
        status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), FORMATTED_TS);
}
