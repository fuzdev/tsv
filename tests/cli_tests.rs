/// Integration tests for CLI commands — each test spawns the `tsv` binary.
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Run the tsv binary with the given arguments.
/// Test helper; panicking on spawn failure is the desired behavior.
#[allow(clippy::expect_used)]
fn tsv(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "-p", "tsv_cli", "-q"])
        .args(args)
        .output()
        .expect("Failed to execute command")
}

/// Run the tsv binary, piping `input` to its stdin (for `--stdin` mode).
/// Test helper; panicking on spawn/IO failure is the desired behavior.
#[allow(clippy::expect_used)]
fn tsv_stdin(args: &[&str], input: &str) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("cargo")
        .args(["run", "-p", "tsv_cli", "-q"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn command");
    // Writing then dropping the handle closes the pipe so the child sees EOF.
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input.as_bytes())
        .expect("Failed to write to stdin");
    child.wait_with_output().expect("Failed to wait for output")
}

/// Create a fresh temp directory unique to this test.
/// Test helper; panicking on IO failure is the desired behavior.
#[allow(clippy::expect_used)]
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tsv_cli_tests_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create temp dir");
    dir
}

const UNFORMATTED_TS: &str = "const   x   =   1;\n";
const FORMATTED_TS: &str = "const x = 1;\n";

#[test]
fn test_parse_command_with_content() {
    let output = tsv(&[
        "parse",
        "--content",
        "const x = 42;",
        "--parser",
        "typescript",
    ]);

    assert!(output.status.success(), "Parse command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""type":"Program"#),
        "Should output AST JSON"
    );
    assert!(stdout.contains(r#""type":"VariableDeclaration"#));
}

#[test]
fn test_parse_command_with_pretty() {
    let output = tsv(&[
        "parse",
        "--content",
        "const x = 42;",
        "--parser",
        "typescript",
        "--pretty",
    ]);

    assert!(output.status.success(), "Parse command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Pretty output is tab-indented (the whole point of the pretty path).
    assert!(
        stdout.contains("\n\t"),
        "Pretty output should be tab-indented: {stdout}"
    );

    // The compact form of the same input must NOT be tab-indented.
    let compact = tsv(&[
        "parse",
        "--content",
        "const x = 42;",
        "--parser",
        "typescript",
    ]);
    assert!(
        !String::from_utf8_lossy(&compact.stdout).contains("\n\t"),
        "Compact output should not be tab-indented"
    );
}

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
fn test_unknown_command() {
    let output = tsv(&["unknown-command"]);

    assert!(!output.status.success(), "Unknown command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unrecognized argument"),
        "Should report unknown command"
    );
}

#[test]
fn test_parse_invalid_syntax() {
    let output = tsv(&["parse", "--content", "const x = ", "--parser", "typescript"]);

    // parse exits 1 on a parse error (distinct from format's 2 for errors).
    assert_eq!(
        output.status.code(),
        Some(1),
        "Invalid syntax should exit 1"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Parse error") || stderr.contains("error"),
        "Should report parse error"
    );
}

#[test]
fn test_parse_missing_parser() {
    let output = tsv(&["parse", "--content", "<div>test</div>"]);

    // Missing --parser is a resolve error → exit 1 (parse's error code).
    assert_eq!(
        output.status.code(),
        Some(1),
        "Parse without --parser should exit 1"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--parser") || stderr.contains("Error"),
        "Should report missing parser option"
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

#[test]
fn test_format_nonexistent_path() {
    let output = tsv(&["format", "/nonexistent/tsv_cli_test_path"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("tsv_cli_test_path"));
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
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("unreadable_subdir");
    let locked = dir.join("locked");
    fs::create_dir_all(&locked).unwrap();
    fs::write(dir.join("a.ts"), UNFORMATTED_TS).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let output = tsv(&["format", dir.to_str().unwrap()]);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(output.status.code(), Some(2));
    // The sibling file is still formatted despite the traversal error
    assert_eq!(fs::read_to_string(dir.join("a.ts")).unwrap(), FORMATTED_TS);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("locked"), "stderr: {stderr}");
    assert!(!stderr.contains("Error: Error"), "stderr: {stderr}");
    assert!(stderr.contains("1 errors"), "stderr: {stderr}");
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
    std::os::unix::fs::symlink(&dir, &link).unwrap();

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
fn test_no_command() {
    let output = tsv(&[]);

    assert!(!output.status.success(), "No command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("subcommand") || stderr.contains("--help"),
        "Should show usage/help message"
    );
}

#[test]
fn test_parse_file_autodetects_parser() {
    let dir = temp_dir("parse_autodetect");
    let cases: [(&str, &str, &str); 3] = [
        ("a.ts", "const x = 1;\n", r#""type":"Program"#),
        ("b.svelte", "<div>x</div>\n", r#""type":"Root"#),
        ("c.css", "a {\n\tcolor: red;\n}\n", r#""type":"StyleSheet"#),
    ];
    for (name, src, marker) in cases {
        let file = dir.join(name);
        fs::write(&file, src).unwrap();
        // No --parser: the parser is auto-detected from the extension.
        let output = tsv(&["parse", file.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(0), "{name} should parse");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(marker),
            "{name}: missing {marker} in {stdout}"
        );
    }
}

#[test]
fn test_parse_stdin() {
    let output = tsv_stdin(
        &["parse", "--stdin", "--parser", "typescript"],
        "const x = 1;\n",
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""type":"Program"#),
        "stdin parse should emit an AST: {stdout}"
    );
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
fn test_parser_ts_alias() {
    // `ts` is an accepted alias for `typescript`.
    let output = tsv(&["parse", "--content", "const x = 1;", "--parser", "ts"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(r#""type":"Program"#),
        "`--parser ts` should parse as TypeScript"
    );
}

#[test]
fn test_format_content_with_paths_errors() {
    // --content cannot be combined with file path arguments.
    let output = tsv(&[
        "format",
        "--content",
        "const x=1;",
        "--parser",
        "typescript",
        "somefile.ts",
    ]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be combined"),
        "stderr should explain the conflict: {stderr}"
    );
}
