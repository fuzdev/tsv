//! `tsv strip`: the type strip on one input — a file path, `--content` or `--stdin` — to
//! stdout, `--check`, the parse goal (`--source-type`, else the module grammar retried as a
//! script), the exit codes (0 stripped, 1 refused, 2 a parse or usage error), and the
//! argument refusals. The JS mirror (`cli.js`) is held to the same rows by
//! `scripts/strip_suite.ts`, and to this binary byte for byte by `scripts/test_napi_npm.ts`.

use std::fs;
use std::process::Output;

use crate::common::{temp_dir, tsv, tsv_in_dir, tsv_stdin};

/// The run's exit code, stdout and stderr, each read as UTF-8.
/// Test helper; panicking on a signal death or non-UTF-8 output is the desired behavior.
#[expect(clippy::expect_used)]
fn outcome(output: &Output) -> (i32, String, String) {
    (
        output.status.code().expect("exited, not signaled"),
        String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8"),
        String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8"),
    )
}

/// Assert a run stripped to exactly `code` on stdout, with exit 0 and nothing on stderr.
fn assert_stripped(output: &Output, code: &str) {
    let (status, stdout, stderr) = outcome(output);
    assert_eq!(status, 0, "stderr: {stderr}");
    assert_eq!(stdout, code);
    assert_eq!(stderr, "");
}

/// Assert a run exited `expected` with nothing on stdout and stderr opening with `prefix`.
fn assert_refused(output: &Output, expected: i32, prefix: &str) {
    let (status, stdout, stderr) = outcome(output);
    assert_eq!(status, expected, "stderr: {stderr}");
    assert_eq!(stdout, "", "nothing is printed on a failure");
    assert!(stderr.starts_with(prefix), "stderr: {stderr:?}");
}

#[test]
fn test_strip_content_prints_the_code_verbatim() {
    assert_stripped(
        &tsv(&["strip", "--content", "let x: number = 1;\n"]),
        "let x         = 1;\n",
    );
}

#[test]
fn test_strip_adds_no_trailing_newline() {
    assert_stripped(&tsv(&["strip", "--content", "let x: T"]), "let x   ");
}

#[test]
fn test_strip_type_free_input_is_unchanged() {
    assert_stripped(
        &tsv(&["strip", "--content", "const x = 1;\n"]),
        "const x = 1;\n",
    );
}

#[test]
fn test_strip_stdin() {
    assert_stripped(
        &tsv_stdin(
            &["strip", "--stdin"],
            "function f<T>(a: T): T { return a; }\n",
        ),
        "function f   (a   )    { return a; }\n",
    );
}

#[test]
fn test_strip_statement_final_assertion_hazard() {
    assert_stripped(
        &tsv(&["strip", "--content", "let x = y as T\n(z)\n"]),
        "let x = y;    \n(z)\n",
    );
}

/// Every line terminator is kept verbatim — CRLF and U+2028 here — and the file is never
/// written: the strip reads, and prints.
#[test]
fn test_strip_file_keeps_line_terminators_and_leaves_the_file() {
    let dir = temp_dir("strip_file_terminators");
    let path = dir.join("a.ts");
    let source = "const a = (b as string).length;\r\ninterface I { x: 1 }\u{2028}export {};\n";
    fs::write(&path, source).unwrap();
    assert_stripped(
        &tsv(&["strip", path.to_str().unwrap()]),
        "const a = (b          ).length;\r\n                    \u{2028}export {};\n",
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

#[test]
fn test_strip_file_keeps_a_leading_bom() {
    let dir = temp_dir("strip_file_bom");
    let path = dir.join("a.ts");
    fs::write(&path, "\u{FEFF}let x: T = 1;\n").unwrap();
    assert_stripped(
        &tsv(&["strip", path.to_str().unwrap()]),
        "\u{FEFF}let x    = 1;\n",
    );
}

#[test]
fn test_strip_keeps_a_hashbang() {
    assert_stripped(
        &tsv(&["strip", "--content", "#!/usr/bin/env node\nlet x: T = 1;\n"]),
        "#!/usr/bin/env node\nlet x    = 1;\n",
    );
}

/// A `.svelte.ts` rune module is TypeScript by its extension.
#[test]
fn test_strip_svelte_ts_file() {
    let dir = temp_dir("strip_svelte_ts");
    let path = dir.join("counter.svelte.ts");
    fs::write(&path, "let count: number = $state(0);\n").unwrap();
    assert_stripped(
        &tsv(&["strip", path.to_str().unwrap()]),
        "let count         = $state(0);\n",
    );
}

#[test]
fn test_strip_refusal_exits_1_naming_the_construct() {
    assert_refused(
        &tsv(&["strip", "--content", "enum E { A }\n"]),
        1,
        "Error: cannot strip an enum\n1:1 enum E { A }\n",
    );
}

/// Each refused construct, by the first line of its message and the `line:col` header of
/// its span's start (1-based column, UTF-16 units, as a parse error's header counts).
#[test]
fn test_strip_refusal_names_every_construct() {
    for (source, first, header) in [
        ("enum E { A }\n", "cannot strip an enum", "1:1"),
        (
            "namespace N { export const a = 1; }\n",
            "cannot strip a namespace with a value member",
            "1:1",
        ),
        (
            "class A { constructor(public x: number) {} }\n",
            "cannot strip a parameter property",
            "1:23",
        ),
        (
            "import x = require('y');\n",
            "cannot strip an 'import =' declaration",
            "1:1",
        ),
        (
            "export = x;\n",
            "cannot strip an 'export =' assignment",
            "1:1",
        ),
        (
            "const a = 1;\nconst b = <T>c;\n",
            "cannot strip an angle-bracket type assertion",
            "2:11",
        ),
        (
            "let v = a + b as T / c;\n",
            "cannot strip an assertion whose blanking would regroup its expression",
            "1:9",
        ),
        (
            "let \u{1d49c}: T = 1; enum E {}\n",
            "cannot strip an enum",
            "1:16",
        ),
    ] {
        let (status, stdout, stderr) = outcome(&tsv(&["strip", "--content", source]));
        assert_eq!(status, 1, "{source:?}: {stderr}");
        assert_eq!(stdout, "", "{source:?}");
        let mut lines = stderr.lines();
        assert_eq!(
            lines.next(),
            Some(format!("Error: {first}").as_str()),
            "{source:?}"
        );
        let second = lines.next().unwrap_or_default();
        assert!(
            second.starts_with(&format!("{header} ")),
            "{source:?}: header {second:?}"
        );
    }
}

#[test]
fn test_strip_file_refusal_names_its_line() {
    let dir = temp_dir("strip_file_refusal");
    let path = dir.join("a.ts");
    fs::write(&path, "const a = 1;\nconst b = <T>c;\n").unwrap();
    assert_refused(
        &tsv(&["strip", path.to_str().unwrap()]),
        1,
        "Error: cannot strip an angle-bracket type assertion\n2:11 const b = <T>c;\n",
    );
}

/// A parse error is the parser's own, printed as `tsv parse` prints it — but exit 2, the
/// error code, where 1 is the refusal verdict.
#[test]
fn test_strip_parse_error_exits_2() {
    let output = tsv(&["strip", "--content", "let = ;\n"]);
    assert_refused(
        &output,
        2,
        "Parse error: Expected expression, found ';'\n1:7 let = ;\n",
    );
    let parse = tsv(&["parse", "--content", "let = ;\n", "--parser", "ts"]);
    assert_eq!(
        output.stderr, parse.stderr,
        "the same error text as `tsv parse`"
    );
}

/// With no `--source-type`, the strip reads the goal as `format` does: a module, retried
/// as a script only if that fails — so a legacy sloppy script strips.
#[test]
fn test_strip_retries_a_failed_module_parse_as_a_script() {
    for (source, code) in [
        ("with (a) {}\n", "with (a) {}\n"),
        (
            "fs.chmodSync(p as string, 0755);\n",
            "fs.chmodSync(p          , 0755);\n",
        ),
        ("var await: number = 1;\n", "var await         = 1;\n"),
    ] {
        assert_stripped(&tsv(&["strip", "--content", source]), code);
    }
}

/// When both grammars reject the source, the error is the one `format` reports: here the
/// sloppy script's own typo on line 2, not the `with` the retry admits.
#[test]
fn test_strip_failed_retry_reports_the_scripts_typo() {
    let source = "with (o) {}\nlet = ;\n";
    let output = tsv(&["strip", "--content", source]);
    assert_refused(
        &output,
        2,
        "Parse error: Expected expression, found ';'\n2:7 let = ;\n",
    );
    let format = tsv(&["format", "--content", source, "--parser", "ts"]);
    assert_eq!(
        output.stderr, format.stderr,
        "the same error text as `tsv format`"
    );
}

/// A named `--source-type` is exact: no retry either way.
#[test]
fn test_strip_source_type_is_exact() {
    assert_stripped(
        &tsv(&[
            "strip",
            "--source-type",
            "script",
            "--content",
            "var await: number = 1;\n",
        ]),
        "var await         = 1;\n",
    );
    assert_refused(
        &tsv(&[
            "strip",
            "--source-type",
            "module",
            "--content",
            "with (a) {}\n",
        ]),
        2,
        "Parse error: The 'with' statement is not allowed in strict mode\n",
    );
    assert_refused(
        &tsv(&[
            "strip",
            "--source-type",
            "script",
            "--content",
            "import type { A } from 'a';\n",
        ]),
        2,
        "Parse error: 'import' is only allowed in a module\n",
    );
    assert_stripped(
        &tsv_stdin(
            &["strip", "--stdin", "--source-type", "script"],
            "with (o as any) { x; }\n",
        ),
        "with (o       ) { x; }\n",
    );
}

#[test]
fn test_strip_source_type_invalid_value() {
    assert_refused(
        &tsv(&["strip", "--source-type", "sloppy", "--content", "let x: T;"]),
        2,
        "Error: invalid --source-type 'sloppy' (expected 'script' or 'module')\n",
    );
}

/// `--source-type` is for `--content` / `--stdin`, as on `format`: a path takes the goal
/// its extension settles, or the fallback.
#[test]
fn test_strip_source_type_beside_a_path_is_refused() {
    let dir = temp_dir("strip_source_type_path");
    let path = dir.join("a.ts");
    fs::write(&path, "let y: T;\n").unwrap();
    assert_refused(
        &tsv(&["strip", "--source-type", "script", path.to_str().unwrap()]),
        2,
        "Error: --source-type applies to --content/--stdin; a file path's goal is its extension's (.mjs/.mts a module, any other JS/TS extension the module grammar retried as a script)\n",
    );
}

/// A path whose extension settles the goal — `.mts`, an ES module by name — takes no
/// script retry, as `format` reads it; `.cts` / `.ts` keep the fallback.
#[test]
fn test_strip_file_extension_settles_the_goal() {
    let dir = temp_dir("strip_extension_goal");
    let source = "with (o as any) { x; }\n";
    for name in ["a.mts", "A.MTS"] {
        let path = dir.join(name);
        fs::write(&path, source).unwrap();
        assert_refused(
            &tsv(&["strip", path.to_str().unwrap()]),
            2,
            "Parse error: The 'with' statement is not allowed in strict mode\n",
        );
    }
    for name in ["a.cts", "a.ts"] {
        let path = dir.join(name);
        fs::write(&path, source).unwrap();
        assert_stripped(
            &tsv(&["strip", path.to_str().unwrap()]),
            "with (o       ) { x; }\n",
        );
    }
}

#[test]
fn test_strip_check_prints_nothing() {
    for source in ["let x: T = 1;\n", "const x = 1;\n"] {
        let (status, stdout, stderr) = outcome(&tsv(&["strip", "--check", "--content", source]));
        assert_eq!(
            (status, stdout.as_str(), stderr.as_str()),
            (0, "", ""),
            "{source:?}"
        );
    }
    assert_refused(
        &tsv(&["strip", "--check", "--content", "enum E { A }\n"]),
        1,
        "Error: cannot strip an enum\n",
    );
    let dir = temp_dir("strip_check_parse_error");
    let path = dir.join("a.ts");
    fs::write(&path, "let = ;\n").unwrap();
    assert_refused(
        &tsv(&["strip", "--check", path.to_str().unwrap()]),
        2,
        "Parse error: ",
    );
}

#[test]
fn test_strip_no_input_is_a_usage_error() {
    assert_refused(
        &tsv(&["strip"]),
        2,
        "Error: No input provided. Use a file path, --content, or --stdin\n",
    );
}

#[test]
fn test_strip_content_beside_a_path_is_refused() {
    let dir = temp_dir("strip_content_and_path");
    let path = dir.join("a.ts");
    fs::write(&path, "let y: T;\n").unwrap();
    assert_refused(
        &tsv(&["strip", "--content", "let x: T;", path.to_str().unwrap()]),
        2,
        "Error: --content/--stdin cannot be combined with a file path\n",
    );
}

/// The strip is TypeScript's alone: a `.svelte` or `.css` path is an argument error naming
/// the extensions it takes, not a TypeScript parse of the markup.
#[test]
fn test_strip_refuses_non_typescript_extensions() {
    let dir = temp_dir("strip_extensions");
    for (name, content) in [
        ("a.svelte", "<script lang=\"ts\">let x: T;</script>\n"),
        ("a.css", "a {}\n"),
        ("README.md", "# x\n"),
    ] {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        let (status, stdout, stderr) = outcome(&tsv(&["strip", path.to_str().unwrap()]));
        assert_eq!(status, 2, "{name}: {stderr}");
        assert_eq!(stdout, "", "{name}");
        assert!(
            stderr.contains("unsupported file extension (tsv strip handles .ts, .mts, .cts)"),
            "{name}: {stderr}"
        );
    }
}

/// A JavaScript path is refused, not read as TypeScript — where `f(a < b, c > (d))` is a
/// generic call whose type arguments the strip would blank, JavaScript reads two
/// comparisons. The message says why.
#[test]
fn test_strip_refuses_javascript_extensions() {
    let dir = temp_dir("strip_javascript_extensions");
    for name in ["a.js", "a.mjs", "a.cjs", "A.MJS"] {
        let path = dir.join(name);
        fs::write(&path, "f(a < b, c > (d));\n").unwrap();
        let path = path.to_str().unwrap();
        assert_refused(
            &tsv(&["strip", path]),
            2,
            &format!(
                "Error: {path}: a JavaScript file has no types to strip (tsv strip handles .ts, .mts, .cts)\n"
            ),
        );
    }
}

#[test]
fn test_strip_directory_is_refused() {
    let dir = temp_dir("strip_directory");
    let (status, _, stderr) = outcome(&tsv(&["strip", dir.to_str().unwrap()]));
    assert_eq!(status, 2, "{stderr}");
    assert!(
        stderr.contains("is a directory (one file is expected)"),
        "{stderr}"
    );
}

#[test]
fn test_strip_missing_file() {
    let dir = temp_dir("strip_missing_file");
    assert_refused(
        &tsv_in_dir(&dir, &["strip", "missing_file_for_strip.ts"]),
        2,
        "Error: Error reading file missing_file_for_strip.ts: ",
    );
}

/// argh's own refusals exit 1, as for every command.
#[test]
fn test_strip_argument_parser_refusals() {
    assert_refused(
        &tsv(&["strip", "a.ts", "b.ts"]),
        1,
        "Unrecognized argument: b.ts",
    );
    assert_refused(
        &tsv(&["strip", "--parser", "ts", "--content", "let x: T;"]),
        1,
        "Unrecognized argument: --parser",
    );
}

#[test]
fn test_strip_help() {
    let (status, stdout, _) = outcome(&tsv(&["help", "strip"]));
    assert_eq!(status, 0);
    assert!(stdout.starts_with("Usage: tsv strip"), "{stdout}");
    assert!(stdout.contains("--source-type"), "{stdout}");
    let (status, top, _) = outcome(&tsv(&["--help"]));
    assert_eq!(status, 0);
    assert!(
        top.lines()
            .any(|line| line.trim_start().starts_with("strip ")),
        "the top-level help lists strip: {top}"
    );
}
