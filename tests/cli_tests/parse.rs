//! `tsv parse`: the content, stdin and file input arms, `--pretty`, `--locations`,
//! the `--source-type` goal axis, parser selection and the argument refusals.

use std::fs;

use crate::common::{temp_dir, tsv, tsv_stdin};

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
fn test_parse_default_is_span_only_and_locations_adds_loc() {
    // The default wire is span-only: `start`/`end` offsets, no `loc`. `--locations`
    // adds the per-node `loc` object (the Rust emitter) and keeps the offsets.
    let span = tsv(&[
        "parse",
        "--content",
        "const x = 42;",
        "--parser",
        "typescript",
    ]);
    let located = tsv(&[
        "parse",
        "--content",
        "const x = 42;",
        "--parser",
        "typescript",
        "--locations",
    ]);
    assert!(span.status.success() && located.status.success());
    let span_out = String::from_utf8_lossy(&span.stdout);
    let located_out = String::from_utf8_lossy(&located.stdout);
    assert!(
        !span_out.contains(r#""loc""#),
        "the default wire must not carry a loc key: {span_out}"
    );
    assert!(
        span_out.contains(r#""start":0"#) && span_out.contains(r#""type":"Program"#),
        "the default wire keeps offsets + payload: {span_out}"
    );
    assert!(
        located_out.contains(r#""loc":{"#) && located_out.contains(r#""start":0"#),
        "--locations carries loc beside the offsets: {located_out}"
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
fn test_parse_pretty_locations_reindents_the_loc_wire() {
    // `--pretty --locations` rides the re-indent-the-bytes pretty path over the loc
    // wire — assert it's tab-indented AND loc-bearing (the only place the two combine).
    let output = tsv(&[
        "parse",
        "--content",
        "const x = 42;",
        "--parser",
        "typescript",
        "--pretty",
        "--locations",
    ]);
    assert!(output.status.success(), "pretty --locations should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\n\t"),
        "pretty output is tab-indented: {stdout}"
    );
    assert!(
        stdout.contains(r#""loc": {"#),
        "loc key in pretty output: {stdout}"
    );
}

#[test]
fn test_parse_svelte_name_loc_rides_locations() {
    // Svelte's element/attribute/directive `name_loc` is a location field too: absent
    // from the default wire, present under `--locations`.
    let span = tsv(&["parse", "--content", "<div>x</div>", "--parser", "svelte"]);
    let located = tsv(&[
        "parse",
        "--content",
        "<div>x</div>",
        "--parser",
        "svelte",
        "--locations",
    ]);
    let span_out = String::from_utf8_lossy(&span.stdout);
    let located_out = String::from_utf8_lossy(&located.stdout);
    assert!(
        !span_out.contains(r#""name_loc""#) && !span_out.contains(r#""loc""#),
        "the default Svelte wire carries neither name_loc nor loc: {span_out}"
    );
    assert!(
        located_out.contains(r#""name_loc""#) && located_out.contains(r#""loc":{"#),
        "--locations Svelte wire carries name_loc and loc: {located_out}"
    );
}

#[test]
fn test_parse_css_loc_rides_locations() {
    // `parseCss` emits no `loc`, but tsv's CSS `--locations` wire carries one on every
    // node like the other two languages; the default wire carries none.
    let span = tsv(&["parse", "--content", "a { color: red }", "--parser", "css"]);
    let located = tsv(&[
        "parse",
        "--content",
        "a { color: red }",
        "--parser",
        "css",
        "--locations",
    ]);
    let span_out = String::from_utf8_lossy(&span.stdout);
    let located_out = String::from_utf8_lossy(&located.stdout);
    assert!(
        !span_out.contains(r#""loc""#),
        "the default CSS wire carries no loc: {span_out}"
    );
    assert!(
        located_out.contains(r#""loc":{"#),
        "--locations CSS wire carries loc: {located_out}"
    );
}

#[test]
fn test_parse_locations_composes_with_source_type_script() {
    // `--source-type` drives the parser, `--locations` the writer — orthogonal, so the
    // two combine (the `sourceType` still follows the goal; `loc` is emitted).
    let output = tsv(&[
        "parse",
        "--content",
        "var await = 1;",
        "--parser",
        "typescript",
        "--source-type",
        "script",
        "--locations",
    ]);
    assert!(
        output.status.success(),
        "source type + --locations should succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""sourceType":"script""#),
        "sourceType follows the goal: {stdout}"
    );
    assert!(
        stdout.contains(r#""loc":{"#),
        "--locations still emits loc: {stdout}"
    );
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

/// `parse` reads one file: a directory is refused by name, ahead of the extension check
/// and of a `--parser` override, rather than read and reported as a file that could not
/// be read.
#[test]
fn test_parse_refuses_a_directory_by_name() {
    let dir = temp_dir("parse_directory");
    fs::create_dir_all(dir.join("src.ts")).unwrap();
    let path = dir.join("src.ts");
    let path = path.to_str().unwrap();
    let expected = format!("Error: {path}: is a directory (one file is expected)\n");
    for args in [
        vec!["parse", path],
        vec!["parse", "--parser", "typescript", path],
        vec!["parse", "--pretty", path],
    ] {
        let output = tsv(&args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            expected,
            "{args:?}"
        );
        assert!(output.stdout.is_empty(), "{args:?}");
    }
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
fn test_parser_ts_alias() {
    // `ts` is an accepted alias for `typescript`.
    let output = tsv(&["parse", "--content", "const x = 1;", "--parser", "ts"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(r#""type":"Program"#),
        "`--parser ts` should parse as TypeScript"
    );
}
