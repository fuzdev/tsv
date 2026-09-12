/// Integration tests for CLI commands — each test spawns the `tsv` binary.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

/// Run the tsv binary with the given arguments.
/// Test helper; panicking on spawn failure is the desired behavior.
#[allow(clippy::expect_used)]
fn tsv(args: &[&str]) -> std::process::Output {
    // the binary `built_tsv` built once, not a `cargo run` per call: a cargo
    // invocation per test is slow, and a `cargo run -p tsv_cli` resolves features
    // for that package alone where the outer `cargo test --workspace` unified them,
    // so a call could relink the binary under a sibling test mid-spawn
    Command::new(built_tsv())
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
    let mut child = Command::new(built_tsv())
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

/// Create a fresh temp directory unique to this test, removed when the test leaves.
///
/// Returns the [`TempTree`] guard rather than a bare path so the cleanup rides the
/// scope exit — see that type for why a trailing `remove_dir_all` cleans up the wrong
/// half of the runs.
/// Test helper; panicking on IO failure is the desired behavior.
#[allow(clippy::expect_used)]
fn temp_dir(name: &str) -> TempTree {
    let dir = std::env::temp_dir().join(format!("tsv_cli_tests_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create temp dir");
    TempTree(dir)
}

static BUILD: Once = Once::new();

/// Path to the built `tsv` binary, built once on first use. Spelled with
/// `EXE_SUFFIX` rather than leaning on Windows' implicit `.exe` resolution,
/// since a test may need the path as a *file* (to copy) and not only as
/// something to spawn.
/// Test helper; panicking on build failure is the desired behavior.
#[allow(clippy::expect_used)]
fn built_tsv() -> PathBuf {
    BUILD.call_once(|| {
        let status = Command::new("cargo")
            .args(["build", "-p", "tsv_cli", "-q"])
            .status()
            .expect("Failed to build tsv_cli");
        assert!(status.success(), "tsv_cli build failed");
    });
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("target/debug/tsv{}", std::env::consts::EXE_SUFFIX))
}

/// Run the built `tsv` binary with `cwd` as its working directory — needed for
/// ignore-file tests that pass a relative target like `.` (resolved against the
/// cwd; the format root is then derived from that target, never the cwd itself),
/// since the `cargo run` helper above always runs in the workspace root.
/// Test helper; panicking on spawn failure is the desired behavior.
#[allow(clippy::expect_used)]
fn tsv_in_dir(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(built_tsv())
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
fn test_parse_pretty_has_no_depth_ceiling_of_its_own() {
    // `--pretty` re-indents the compact bytes rather than reading them back through
    // a `serde_json::Value`, whose default recursion limit refused ~60 nested arrays
    // the compact route had just emitted. 300 levels is well past that limit and well
    // inside the parser's own ceiling (~25,000 arrays on the sized stack).
    let n = 300;
    let source = format!("const x = {}{};", "[".repeat(n), "]".repeat(n));
    let output = tsv(&[
        "parse",
        "--content",
        &source,
        "--parser",
        "typescript",
        "--pretty",
    ]);
    assert!(
        output.status.success(),
        "deep pretty parse should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\n\t"), "pretty output is tab-indented");
    assert_eq!(
        stdout.matches("\"type\": \"ArrayExpression\"").count(),
        n,
        "every nesting level reaches the output"
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
fn test_parse_no_locations_composes_with_source_type_script() {
    // `--source-type` drives the parser, `--no-locations` the writer — orthogonal, so the
    // two combine (the `sourceType` still follows the goal; no loc is emitted).
    let output = tsv(&[
        "parse",
        "--content",
        "var await = 1;",
        "--parser",
        "typescript",
        "--source-type",
        "script",
        "--no-locations",
    ]);
    assert!(
        output.status.success(),
        "source type + no-locations should succeed"
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
fn test_parse_source_type_script_accepts_await_identifier() {
    // At Script goal, `await` is an ordinary identifier (`var await = 1`), and the
    // public AST's `sourceType` follows the goal.
    let output = tsv(&[
        "parse",
        "--content",
        "var await = 1;",
        "--parser",
        "typescript",
        "--source-type",
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
fn test_parse_source_type_module_rejects_await_identifier() {
    // The same source is rejected at Module goal — `await` is reserved there.
    let output = tsv(&[
        "parse",
        "--content",
        "var await = 1;",
        "--parser",
        "typescript",
        "--source-type",
        "module",
    ]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "Module-goal `var await` should be rejected"
    );
}

#[test]
fn test_parse_source_type_defaults_to_module() {
    // No `--source-type` flag → Module (rejects `var await`), matching the explicit case.
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
fn test_parse_source_type_invalid_value() {
    let output = tsv(&[
        "parse",
        "--content",
        "x;",
        "--parser",
        "typescript",
        "--source-type",
        "bogus",
    ]);

    // parse reports usage errors with its own exit-1 convention (see
    // `test_parse_invalid_syntax`).
    assert_eq!(
        output.status.code(),
        Some(1),
        "Invalid --source-type should exit 1 for parse"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid --source-type"),
        "Should report invalid --source-type: {stderr}"
    );
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

/// `--help`'s extension list IS [`tsv_discover::FORMATTABLE_EXTENSIONS`], not a copy.
///
/// argh renders `--help` from doc comments, and a doc comment is a string literal — so
/// `FormatCommand`'s `paths` doc cannot build its list from the const the way
/// `unsupported_extension_error` and the nothing-in-scope error do
/// ([`tsv_discover::formattable_extension_list`]). That leaves two spellings of one fact,
/// and `--help` is the one that drifts silently: nothing else reads it, so a ninth
/// language would ship with a refusal naming nine and a help text naming eight. This is
/// what fails instead.
///
/// The comparison strips ALL whitespace from the help text rather than matching the line:
/// argh wraps at word boundaries, so a longer list moves to its own line — a real
/// rendering, not a drift — and the rendered list holds no whitespace of its own, which
/// makes the stripped `contains` exact rather than merely lenient.
#[test]
fn test_format_help_extension_list_is_rendered_from_the_const() {
    let output = tsv(&["format", "--help"]);
    let help: String = String::from_utf8_lossy(&output.stdout)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let list = tsv_discover::formattable_extension_list("/");
    assert!(
        help.contains(&list),
        "`tsv format --help` must name the formattable extensions as `{list}` (rendered from \
         tsv_discover::FORMATTABLE_EXTENSIONS); the `paths` doc comment in \
         crates/tsv_cli/src/cli/commands/format.rs has drifted from the const"
    );
}

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
    assert!(
        stderr.contains(
            "a\nb.ts is inside build, which a rule in the repo-root .gitignore excludes, so it is not formatted; no ignore-file line can hold the line break in its path, so narrow that rule to format it"
        ),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains('`'), "stderr: {stderr}");
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

/// `parse <file>` takes the same extension check as `format <file>`, and the same
/// message: the dispatch behind a path has no unknown arm, so without it a `.md` is
/// parsed as TypeScript and the user reads a syntax error about their prose. An
/// explicit `--parser` is the override — the caller named the grammar, so the name of
/// the file no longer decides.
#[test]
fn test_parse_file_rejects_unsupported_extension_unless_parser_is_named() {
    let dir = temp_dir("parse_unsupported_extension");
    let md = dir.join("notes.md");
    fs::write(&md, "x = 1\n").unwrap();

    let output = tsv(&["parse", md.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1), "parse keeps its 0/1 codes");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported file extension") && stderr.contains(".svelte"),
        "stderr: {stderr}"
    );
    assert!(output.stdout.is_empty(), "nothing parsed");

    let output = tsv(&["parse", md.to_str().unwrap(), "--parser", "typescript"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "--parser overrides the extension"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).starts_with(r#"{"type":"Program""#),
        "the named grammar parsed the file"
    );
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

#[cfg(unix)]
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

#[cfg(unix)]
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
fn test_version_flag() {
    let output = tsv(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("tsv {}\n", env!("CARGO_PKG_VERSION")),
        "exact `tsv <workspace version>` line — the npm cli.js mirrors it from its package.json"
    );
    assert!(output.stderr.is_empty());
}

/// The command name in usage/help/error text is pinned to `tsv`, never derived
/// from `argv[0]` — which is what `argh::from_env` does, and what printed
/// `Usage: tsv.exe format` on Windows. Running a renamed copy varies exactly
/// that dimension on any platform, so the property is provable here rather than
/// only in a Windows CI leg.
#[test]
#[allow(clippy::expect_used)]
fn test_command_name_is_independent_of_argv0() {
    let dir = temp_dir("argv0_name");
    let renamed = dir.join(format!("tsv_renamed{}", std::env::consts::EXE_SUFFIX));
    fs::copy(built_tsv(), &renamed).expect("Failed to copy the tsv binary");

    // ⚠️ Retry on ETXTBSY. `fs::copy` above closes its destination handle before
    // returning, but this harness runs tests on parallel threads and most of them
    // spawn a child process: between another thread's fork and its exec the child
    // holds a *copy* of every open descriptor, so a fork that straddles the copy
    // leaves the new inode write-open in that child until it execs. The kernel's
    // check is on the inode's writecount, so exec'ing the fresh binary in that
    // window fails with `ExecutableFileBusy` — a race in the harness, never in
    // the binary. The window closes with the other child's exec, so a bounded
    // retry is the fix; the copy itself cannot avoid it.
    let run = |args: &[&str]| {
        for _ in 0..50 {
            match Command::new(&renamed).args(args).output() {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                result => return result.expect("Failed to execute the renamed tsv binary"),
            }
        }
        panic!("the renamed tsv binary stayed ExecutableFileBusy for a second");
    };

    let help = run(&["help", "format"]);
    assert_eq!(help.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(
        stdout.starts_with("Usage: tsv format"),
        "renamed binary must still name itself `tsv`: {stdout}"
    );

    // The error path carries the same name (argh's `Run <cmd> --help` line).
    let bad = run(&["format", "--parser", "bogus", "--content", "x"]);
    assert_eq!(bad.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        stderr.contains("Run tsv --help for more information."),
        "stderr: {stderr}"
    );
}

#[test]
fn test_version_is_top_level_only() {
    // Subcommands don't take --version — argh's unrecognized-argument error, which
    // the JS mirror's transcription of argh repeats word for word (exit 1).
    let output = tsv(&["format", "--version"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Unrecognized argument: --version"),
        "should fail as an unknown subcommand argument, not print a version"
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
fn git_repo(name: &str) -> TempTree {
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
    // names the pruned dir + the heuristic, and points at the dir-level escape in the
    // file the re-include was written in. Outside a repo both paths read absolutely,
    // while the line stays relative to that file's directory and anchored
    assert!(
        stderr.contains("build is skipped by tsv's build-output heuristic"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains(
            "/.formatignore does nothing; re-include the directory itself there with `!/build/`"
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
    let canonical = fs::canonicalize(&*dir).unwrap();
    let expected = format!(".prettierignore in {} is shadowed", canonical.display());

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

/// A tree big enough to fill a pipe: 1,200 unformatted files whose **names** are long
/// enough that the changed-path report clears the 64 KiB pipe buffer.
///
/// Both numbers are load-bearing and independent. 1,200 files with ordinary names is
/// only ~54 KB of paths and does **not** reproduce the closed-pipe bug — the whole
/// report fits in the buffer, the single write succeeds, and nothing ever sees `EPIPE`.
/// The 150-byte name brings it to ~190 KB. The same pair of thresholds is why the JS
/// mirror's rows (`scripts/test_npm.ts`) are shaped this way.
///
/// A fresh tree per call, because a second run has nothing to report: the files are
/// formatted in place, so the report that has to overflow the pipe exists only once.
#[cfg(unix)]
fn pipe_overflow_tree(name: &str) -> TempTree {
    long_name_tree(name, PIPE_TREE_FILES)
}

/// `count` unformatted files with 150-byte names — the lever the pipe and socket rows
/// turn, since what has to overflow the buffer is the changed-path report. Test
/// helper; panicking on IO failure is the desired behavior.
#[cfg(unix)]
#[allow(clippy::expect_used)]
fn long_name_tree(name: &str, count: usize) -> TempTree {
    let tree = temp_dir(name);
    let pad = "p".repeat(150);
    for i in 0..count {
        fs::write(tree.path().join(format!("{pad}_{i}.ts")), UNFORMATTED_TS)
            .expect("write seed file");
    }
    tree
}

/// A temp tree that is removed when the test leaves — **assertion failure included**.
///
/// `temp_dir` names its directory after the pid, so a leak is never reclaimed by a
/// later run, and a `remove_dir_all` written after the assertions only cleans up the
/// runs that passed: the wrong half, since a failing run is the one a developer
/// re-runs. The pipe tests' trees are 1,200 files each, so the leak is measured in
/// thousands of files per red test. Same shape as `ReleasePoolOnUnwind` in the format
/// command — the cleanup belongs on the way out, not on the happy path.
///
/// Every temp tree in this file is one, because a guard two call sites use is a
/// convention the other forty do not follow: `temp_dir` returns it, so a test cannot
/// opt out by forgetting. It [`Deref`]s to `Path`, so `&dir` and `dir.join(..)` read
/// exactly as they did against the bare `PathBuf`.
struct TempTree(PathBuf);

impl TempTree {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for TempTree {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
const PIPE_TREE_FILES: usize = 1200;

/// A piped run: the shell's output plus the **CLI's own** exit code.
#[cfg(unix)]
struct PipedRun {
    out: std::process::Output,
    /// `tsv`'s own status, which the pipeline otherwise hides behind the last
    /// command's. Captured through a file rather than `PIPESTATUS`, which is a bashism
    /// — `/bin/sh` is dash on most Linux distributions, where it expands to nothing.
    code: i32,
}

/// Run `tsv <tsv_args> <pipeline>` through `sh` with `dir` as the cwd.
///
/// A pipe needs a shell: `std::process::Command` can spawn a child but not hand its
/// stdout to a second process that then *exits early*, which is the condition under
/// test. The status file is a sibling of `dir`, not inside it, so it cannot perturb a
/// tree the run is formatting.
/// Test helper; panicking on spawn failure is the desired behavior.
#[cfg(unix)]
#[allow(clippy::expect_used)]
fn tsv_piped(dir: &Path, tsv_args: &str, pipeline: &str) -> PipedRun {
    let bin = built_tsv();
    let status_file = dir.with_extension("status");
    let _ = fs::remove_file(&status_file);
    let script = format!(
        "{{ '{}' {tsv_args}; echo $? > '{}'; }} {pipeline}",
        bin.display(),
        status_file.display()
    );
    let out = Command::new("sh")
        .args(["-c", &script])
        .current_dir(dir)
        .output()
        .expect("Failed to execute shell");
    let code = fs::read_to_string(&status_file)
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(-1);
    let _ = fs::remove_file(&status_file);
    PipedRun { out, code }
}

/// A panic notice in either stream means the run aborted rather than stopping.
#[cfg(unix)]
fn assert_no_pipe_panic(label: &str, run: &PipedRun) {
    let stderr = String::from_utf8_lossy(&run.out.stderr);
    let stdout = String::from_utf8_lossy(&run.out.stdout);
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Broken pipe"),
        "{label}: the CLI crashed on a closed pipe: {stderr}"
    );
    assert!(
        !stdout.contains("panicked"),
        "{label}: the CLI crashed on a closed pipe: {stdout}"
    );
}

/// A consumer that exits early (`| head`) stops the output and nothing else.
///
/// Rust sets `SIGPIPE` to `SIG_IGN`, so the write returns `EPIPE` rather than killing
/// the process — and `println!` turned that into a panic (exit 134 under the release
/// profile's `panic = "abort"`, 101 under an unwinding one) *after* every file had
/// already been rewritten: the changes landed and the report of them became a crash
/// notice. So `EPIPE` stops the write and the run finishes on its own terms, which is
/// what `ls | head` looks like from the caller's side and what the JS mirror already
/// did (`cli.js`'s `write_fd`; `scripts/test_npm.ts` carries the twin rows).
///
/// **Both fds**, in two shapes that fail differently: bare `| head` closes stdout while
/// stderr stays on the terminal, grading the changed-path write; `2>&1 | head` closes
/// the *same* pipe for both, grading the summary — the write a stdout-only fix leaves
/// panicking one line further down.
#[cfg(unix)]
#[test]
fn test_format_closed_pipe_consumer_is_not_a_crash() {
    for (label, pipeline) in [
        ("stdout_only", "| head -2 >/dev/null"),
        ("both_fds", "2>&1 | head -2 >/dev/null"),
    ] {
        let tree = pipe_overflow_tree(&format!("closed_pipe_{label}"));
        let run = tsv_piped(tree.path(), "format .", pipeline);
        assert_no_pipe_panic(label, &run);
        assert_eq!(run.code, 0, "{label}: format reported a failure");
        // The report is truncated, not the work: every file is still formatted.
        let formatted = fs::read_dir(tree.path())
            .unwrap()
            .filter(|e| fs::read_to_string(e.as_ref().unwrap().path()).unwrap() == FORMATTED_TS)
            .count();
        assert_eq!(
            formatted, PIPE_TREE_FILES,
            "{label}: files were left unformatted"
        );
    }
}

/// `--list`'s single buffered write is the largest the CLI makes, and it takes the
/// same quiet stop: a listing cut short by `| head` is not an error.
#[cfg(unix)]
#[test]
fn test_format_list_closed_pipe_is_not_a_crash() {
    let tree = pipe_overflow_tree("closed_pipe_list");
    let run = tsv_piped(tree.path(), "format --list .", "| head -2 >/dev/null");
    assert_no_pipe_panic("--list", &run);
    assert_eq!(
        run.code,
        0,
        "--list reported a failure: {}",
        String::from_utf8_lossy(&run.out.stderr)
    );
    // and listed nothing it formatted
    let pad = "p".repeat(150);
    assert_eq!(
        fs::read_to_string(tree.path().join(format!("{pad}_0.ts"))).unwrap(),
        UNFORMATTED_TS
    );
}

/// The closed pipe does not overwrite the exit code, because for `--check` the exit
/// code **is** the API.
///
/// This is why a `BrokenPipe` write goes quiet instead of reporting 141, or restoring
/// `SIGPIPE` to `SIG_DFL` and dying by the signal: either of those answers "the reader
/// left" to a caller that asked "would anything change?".
#[cfg(unix)]
#[test]
fn test_format_check_exit_code_survives_a_closed_pipe() {
    let tree = pipe_overflow_tree("closed_pipe_check");
    let run = tsv_piped(tree.path(), "format --check .", "| head -2 >/dev/null");
    assert_no_pipe_panic("--check", &run);
    assert_eq!(
        run.code,
        1,
        "--check must still report would-change through a closed pipe, stderr: {}",
        String::from_utf8_lossy(&run.out.stderr)
    );
    // And `--check` still wrote nothing.
    let pad = "p".repeat(150);
    assert_eq!(
        fs::read_to_string(tree.path().join(format!("{pad}_0.ts"))).unwrap(),
        UNFORMATTED_TS
    );
}

/// The other end of the same rule: a consumer that is merely **slow** gets everything.
///
/// `EPIPE` is the only write error the CLI absorbs, so a reader that drains late must
/// still receive every line — otherwise "stop quietly" would have become "stop early",
/// which is a silently truncated report rather than a fixed crash.
#[cfg(unix)]
#[test]
fn test_format_slow_pipe_consumer_gets_every_line() {
    let tree = pipe_overflow_tree("slow_pipe");
    let run = tsv_piped(tree.path(), "format .", "| { sleep 1; cat; }");
    assert_no_pipe_panic("slow consumer", &run);
    let stderr = String::from_utf8_lossy(&run.out.stderr);
    assert_eq!(run.code, 0, "the run died: {stderr}");
    assert_eq!(
        String::from_utf8_lossy(&run.out.stdout).lines().count(),
        PIPE_TREE_FILES,
        "the changed-path list was truncated, stderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!("{PIPE_TREE_FILES} formatted")),
        "stderr: {stderr}"
    );
}

/// `parse`'s wire goes through the same writer, so a closed reader is not a parse error.
///
/// This one never panicked — `parse` wrote through a hand-locked handle and answered a
/// failed write with `exit(1)`, its *parse-error* code, so `tsv parse big.ts | head` was
/// indistinguishable from invalid syntax. One writer for both commands settles it at 0,
/// the answer `cli.js` gives.
#[cfg(unix)]
#[test]
fn test_parse_closed_pipe_is_not_a_parse_error() {
    let tree = temp_dir("parse_closed_pipe");
    let path = tree.path().join("big.ts");
    // Enough statements that the wire clears the 64 KiB pipe buffer by a wide margin.
    use std::fmt::Write as _;
    let mut src = String::new();
    for i in 0..2000 {
        let _ = writeln!(src, "const x{i} = {i};");
    }
    fs::write(&path, &src).unwrap();

    let run = tsv_piped(tree.path(), "parse big.ts", "| head -c 100 >/dev/null");
    assert_no_pipe_panic("parse", &run);
    assert_eq!(
        run.code,
        0,
        "a closed reader is not a parse failure, stderr: {}",
        String::from_utf8_lossy(&run.out.stderr)
    );

    // A real parse error still reports 1 — the closed-pipe answer didn't swallow it.
    fs::write(&path, "const bad = ;\n").unwrap();
    let broken = tsv_in_dir(tree.path(), &["parse", "big.ts"]);
    assert_eq!(broken.status.code(), Some(1));
}

/// A **non-blocking** stdout is a slow consumer, not a failure: the run waits it out and
/// every line arrives.
///
/// Whether the fd blocks is a property of the open file description, which the child
/// shares with its parent — and a Node parent that opens its own piped
/// `process.stdout` while `tsv` runs (a task runner logging beside an async child)
/// flips that description to non-blocking under it. A full pipe then answers `EAGAIN` where a
/// blocking one would park the write, and `write_all` panicked on it (exit 134 under
/// `panic = "abort"`) after every file had already been rewritten — the same crash
/// `cli.js`'s `write_fd` was fixed for. A `UnixStream` pair stands in for the flipped
/// pipe: the child's end is set non-blocking before it is handed over as stdout, the
/// reader holds the other end and drains late, and a socket's send buffer fills and
/// answers `EAGAIN` exactly as a pipe's does — once it is full. A socket buffers
/// ~200 KiB on Linux where a pipe buffers 64 KiB, so this tree is `SOCKET_TREE_FILES`
/// deep rather than `PIPE_TREE_FILES`: the 1,200-file report fits in a socket whole, and
/// the row then passes with the `EAGAIN` arm removed (it was checked; it did). The
/// reader keys on the report itself rather than on a clock: its first byte arrives only
/// once the report's first write has landed and filled the buffer, so however long the
/// formatting took, the writer's next attempt meets a full socket.
/// The read side of the same rule: a non-blocking stdin — flipped under the child by a
/// parent that opens its own piped `process.stdin` — is waited out, not reported as
/// `Resource temporarily unavailable` the moment the writer pauses.
#[cfg(unix)]
#[test]
fn test_format_stdin_non_blocking_is_waited_out_not_a_read_error() {
    use std::io::Write as _;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;

    let (reader, mut writer) = UnixStream::pair().expect("socket pair");
    reader.set_nonblocking(true).expect("set O_NONBLOCK");
    let child = Command::new(built_tsv())
        .args(["format", "--stdin", "--parser", "typescript"])
        .stdin(Stdio::from(OwnedFd::from(reader)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tsv");
    // the slow writer: the first half, a pause long enough for the child to find the
    // socket empty, then the rest and EOF
    writer.write_all(b"const   x   =").expect("first half");
    std::thread::sleep(std::time::Duration::from_millis(200));
    writer.write_all(b"   1;\n").expect("second half");
    drop(writer);
    let output = child.wait_with_output().expect("wait");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), FORMATTED_TS);
}

#[cfg(unix)]
#[test]
fn test_format_non_blocking_stdout_is_waited_out_not_a_crash() {
    use std::io::Read as _;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;

    const SOCKET_TREE_FILES: usize = 6000;
    let tree = long_name_tree("non_blocking_stdout", SOCKET_TREE_FILES);
    let (mut reader, writer) = UnixStream::pair().expect("socket pair");
    writer.set_nonblocking(true).expect("set O_NONBLOCK");
    let mut child = Command::new(built_tsv())
        .args(["format", "--check", "."])
        .current_dir(tree.path())
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tsv");
    // the slow consumer: take one byte, which comes only once the report's first write
    // has filled the socket, then leave the rest undrained while the writer retries
    let mut first = [0_u8; 1];
    reader
        .read_exact(&mut first)
        .expect("the report's first byte");
    std::thread::sleep(std::time::Duration::from_millis(200));
    let mut stdout = String::from_utf8(first.to_vec()).expect("an ASCII path byte");
    reader.read_to_string(&mut stdout).expect("drain stdout");
    let status = child.wait().expect("wait");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("piped stderr")
        .read_to_string(&mut stderr)
        .expect("read stderr");

    assert!(
        !stderr.contains("panicked") && !stderr.contains("temporarily unavailable"),
        "the CLI crashed on a non-blocking stdout: {stderr}"
    );
    assert_eq!(
        status.code(),
        Some(1),
        "--check still reports would-change: {stderr}"
    );
    assert_eq!(
        stdout.lines().count(),
        SOCKET_TREE_FILES,
        "the changed-path list was truncated, stderr: {stderr}"
    );
}
