/// Integration tests for CLI commands — each test spawns the `tsv` binary.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

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

static BUILD: Once = Once::new();

/// Run the built `tsv` binary with `cwd` as its working directory — needed for
/// ignore-file tests that pass a relative target like `.` (resolved against the
/// cwd; the format root is then derived from that target, never the cwd itself),
/// since the `cargo run` helper above always runs in the workspace root. The
/// binary is built once on first use.
/// Test helper; panicking on spawn/build failure is the desired behavior.
#[allow(clippy::expect_used)]
fn tsv_in_dir(cwd: &Path, args: &[&str]) -> std::process::Output {
    BUILD.call_once(|| {
        let status = Command::new("cargo")
            .args(["build", "-p", "tsv_cli", "-q"])
            .status()
            .expect("Failed to build tsv_cli");
        assert!(status.success(), "tsv_cli build failed");
    });
    let bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/tsv");
    Command::new(bin)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("Failed to execute tsv binary")
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
fn test_parse_no_locations_omits_loc() {
    // `--no-locations` emits the span-only wire: `start`/`end` offsets, no `loc`.
    let full = tsv(&[
        "parse",
        "--content",
        "const x = 42;",
        "--parser",
        "typescript",
    ]);
    let no_loc = tsv(&[
        "parse",
        "--content",
        "const x = 42;",
        "--parser",
        "typescript",
        "--no-locations",
    ]);
    assert!(full.status.success() && no_loc.status.success());
    let full_out = String::from_utf8_lossy(&full.stdout);
    let no_loc_out = String::from_utf8_lossy(&no_loc.stdout);
    // The default wire carries the `loc` object; the span-only wire drops it but
    // keeps offsets and the rest of the payload.
    assert!(
        full_out.contains(r#""loc":{"#),
        "default wire should carry loc"
    );
    assert!(
        !no_loc_out.contains(r#""loc":{"#),
        "no-locations wire must not carry a loc object: {no_loc_out}"
    );
    assert!(
        no_loc_out.contains(r#""start":0"#) && no_loc_out.contains(r#""type":"Program"#),
        "no-locations wire keeps offsets + payload: {no_loc_out}"
    );
}

#[test]
fn test_parse_no_locations_pretty_reparses() {
    // `--pretty --no-locations` rides the reparse-the-bytes pretty path — assert
    // it's tab-indented AND loc-free (the only place the two branches combine).
    let output = tsv(&[
        "parse",
        "--content",
        "const x = 42;",
        "--parser",
        "typescript",
        "--pretty",
        "--no-locations",
    ]);
    assert!(
        output.status.success(),
        "pretty no-locations should succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\n\t"),
        "pretty output is tab-indented: {stdout}"
    );
    assert!(
        !stdout.contains(r#""loc""#),
        "no loc key in pretty output: {stdout}"
    );
}

#[test]
fn test_parse_no_locations_svelte_omits_name_loc() {
    // Svelte also drops the element/attribute/directive `name_loc`.
    let full = tsv(&["parse", "--content", "<div>x</div>", "--parser", "svelte"]);
    let no_loc = tsv(&[
        "parse",
        "--content",
        "<div>x</div>",
        "--parser",
        "svelte",
        "--no-locations",
    ]);
    let full_out = String::from_utf8_lossy(&full.stdout);
    let no_loc_out = String::from_utf8_lossy(&no_loc.stdout);
    assert!(
        full_out.contains(r#""name_loc""#),
        "default Svelte wire carries name_loc"
    );
    assert!(
        !no_loc_out.contains(r#""name_loc""#) && !no_loc_out.contains(r#""loc":{"#),
        "no-locations Svelte wire drops name_loc and loc: {no_loc_out}"
    );
}

#[test]
fn test_parse_no_locations_css_is_noop() {
    // `parseCss` emits no `loc`, so `--no-locations` is a documented no-op for CSS
    // — byte-identical to the default wire.
    let full = tsv(&["parse", "--content", "a { color: red }", "--parser", "css"]);
    let no_loc = tsv(&[
        "parse",
        "--content",
        "a { color: red }",
        "--parser",
        "css",
        "--no-locations",
    ]);
    assert_eq!(
        full.stdout, no_loc.stdout,
        "CSS no-locations must equal the default wire"
    );
}

#[test]
fn test_parse_no_locations_composes_with_goal_script() {
    // `--goal` drives the parser, `--no-locations` the writer — orthogonal, so the
    // two combine (the `sourceType` still follows the goal; no loc is emitted).
    let output = tsv(&[
        "parse",
        "--content",
        "var await = 1;",
        "--parser",
        "typescript",
        "--goal",
        "script",
        "--no-locations",
    ]);
    assert!(
        output.status.success(),
        "goal + no-locations should succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""sourceType":"script""#),
        "sourceType follows the goal: {stdout}"
    );
    assert!(
        !stdout.contains(r#""loc":{"#),
        "no-locations still drops loc: {stdout}"
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
fn test_parse_goal_script_accepts_await_identifier() {
    // At Script goal, `await` is an ordinary identifier (`var await = 1`), and the
    // public AST's `sourceType` follows the goal.
    let output = tsv(&[
        "parse",
        "--content",
        "var await = 1;",
        "--parser",
        "typescript",
        "--goal",
        "script",
    ]);

    assert!(output.status.success(), "Script-goal parse should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""sourceType":"script""#),
        "Program sourceType should follow the goal: {stdout}"
    );
}

#[test]
fn test_parse_goal_module_rejects_await_identifier() {
    // The same source is rejected at Module goal — `await` is reserved there.
    let output = tsv(&[
        "parse",
        "--content",
        "var await = 1;",
        "--parser",
        "typescript",
        "--goal",
        "module",
    ]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "Module-goal `var await` should be rejected"
    );
}

#[test]
fn test_parse_goal_defaults_to_module() {
    // No `--goal` flag → Module (rejects `var await`), matching the explicit case.
    let output = tsv(&[
        "parse",
        "--content",
        "var await = 1;",
        "--parser",
        "typescript",
    ]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "Default goal should be Module"
    );
}

#[test]
fn test_parse_goal_invalid_value() {
    let output = tsv(&[
        "parse",
        "--content",
        "x;",
        "--parser",
        "typescript",
        "--goal",
        "bogus",
    ]);

    // parse reports usage errors with its own exit-1 convention (see
    // `test_parse_invalid_syntax`).
    assert_eq!(
        output.status.code(),
        Some(1),
        "Invalid --goal should exit 1 for parse"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid --goal"),
        "Should report invalid --goal: {stderr}"
    );
}

#[test]
fn test_format_goal_script() {
    // `await => 1` is a single-param arrow at Script goal; formats with arrowParens.
    let output = tsv(&[
        "format",
        "--content",
        "await => 1;",
        "--parser",
        "typescript",
        "--goal",
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
fn test_format_goal_invalid_value() {
    let output = tsv(&[
        "format",
        "--content",
        "x;",
        "--parser",
        "typescript",
        "--goal",
        "bogus",
    ]);

    // format uses exit 2 for argument/usage errors (distinct from parse's 1).
    assert_eq!(
        output.status.code(),
        Some(2),
        "Invalid --goal should exit 2 for format"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid --goal"),
        "Should report invalid --goal: {stderr}"
    );
}

#[test]
fn test_format_goal_rejected_in_path_mode() {
    // `--goal` is content/stdin-only; with a path argument it's a usage error
    // (file paths are always formatted as modules).
    let dir = temp_dir("format_goal_path");
    let file = dir.join("a.ts");
    fs::write(&file, "const x = 1;\n").expect("write temp file");

    let output = tsv(&[
        "format",
        "--goal",
        "script",
        file.to_str().expect("utf8 path"),
    ]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "`--goal` with a path should be a usage error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--goal applies to --content/--stdin"),
        "Should explain the path-mode restriction: {stderr}"
    );
    let _ = fs::remove_dir_all(&dir);
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

/// Create a temp dir that looks like a git repo root — a `.git` marker directory
/// is all `find_repo_root` checks for, so this turns on gitignore-aware discovery
/// without needing a real `git` binary.
/// Test helper; panicking on IO failure is the desired behavior.
#[allow(clippy::unwrap_used)]
fn git_repo(name: &str) -> PathBuf {
    let dir = temp_dir(name);
    fs::create_dir(dir.join(".git")).unwrap();
    dir
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
    // names the pruned dir + the heuristic, and points at the dir-level escape.
    // (the dir is named format-root-relative, so outside a repo it carries the
    // path from the filesystem root — assert on the stable phrasing, not `!build/`)
    assert!(
        stderr.contains("build is skipped by tsv's build-output heuristic"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("re-include the directory itself"),
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
    // inside a repo the repo-root `.prettierignore` IS read (drop-in compat) and
    // honored, so there is nothing to warn about.
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
fn test_format_nested_prettierignore_outside_repo_does_not_warn() {
    // the warning is bounded to the TARGET ROOT: a `.prettierignore` nested in a
    // subdirectory is not read by prettier either, so it's no divergence — no
    // warning (and tsv doesn't honor it).
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
