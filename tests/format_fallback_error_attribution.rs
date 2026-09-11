// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! When BOTH grammars reject a source the format fallback was handed, the error it
//! reports is the attempt's whose grammar the file was written against: the module
//! attempt's when the script attempt died on a **goal gate**, else the one that reached
//! **furthest into the source**, the module attempt's on a tie.
//!
//! `tsv_ts::parse_with_goal_or_fallback(source, None, …)` parses at `Module` and retries at
//! `Script` only if that fails. A source both reject has two errors, and which one the user
//! sees decides whether the diagnosis names the file's own mistake or the construct the
//! retry exists to accept: a legacy sloppy script with a typo fails the module parse on its
//! `with` / legacy literal / `await` name — the very thing the script retry then admits —
//! and fails the script parse on the typo. Reporting the module error there points at the
//! wrong line. Position alone is not the rule, though: a module whose real error sits
//! *ahead* of its `export` (definitions first, exports at the bottom) has a script attempt
//! that reaches further, and reporting that would blame a valid `export` line. So a script
//! attempt that died on `import` / `export` / `import.meta` — the constructs only a module
//! holds, marked at their sites (`ParseError::is_goal_gated`) — settles the file as a module
//! outright, and only past that does the further error win: a broken script's module
//! attempt dies early at the first sloppy construct, and a broken module's script attempt
//! dies at its top-level `await` — the one module-only construct with no gate, since at
//! `Script` the word is an identifier and the attempt fails on whatever follows it.
//!
//! prettier reads the same way in practice: its babel parser tolerates `StrictWith`,
//! `StrictOctalLiteral` and `StrictNumericEscape` at the module goal through error
//! recovery, so its module parse never fails on the sloppy constructs and the error it
//! reports on such a file is the typo (`src/language-js/parse/babel.js`,
//! `allowedReasonCodes`).
//!
//! Not fixturable: `input_invalid_*` asserts only that both parsers reject, never *where*
//! — the same reason `for_await_rejection.rs` exists. The rejections themselves are pinned
//! by the `typescript/script_goal/*` fixtures' `input_invalid_*` files.
//!
//! Which tests carry the rule and which are controls: `a_sloppy_script_with_a_typo_…`
//! fails with the attribution reverted to "the module's error, always";
//! `a_module_with_its_error_ahead_of_its_export_…` and the second half of `a_mixed_file_…`
//! fail with it reverted to position alone; `a_tie_…` fails with the tie-break's direction
//! flipped. `a_module_with_a_typo_…` (the module error is the further one anyway) and
//! `a_named_goal_…` (no fallback runs) hold under every rule and pin the surrounding
//! contract.

use bumpalo::Bump;

/// The rendered fallback error's `(line, column, message)`, read back off the caret form.
/// A positionless render carries no located line, so this panics rather than silently
/// reading the message line as a location.
fn fallback_error_at(source: &str) -> (usize, usize, String) {
    let arena = Bump::new();
    let err = tsv_ts::parse_with_goal_or_fallback(source, None, &arena)
        .expect_err("the input must parse under neither goal");
    let rendered = err.to_string();
    let mut lines = rendered.lines();
    let message = lines
        .next()
        .expect("a rendered error opens with its message")
        .to_string();
    let located = lines
        .next()
        .expect("a rendered error carries a `line:col source` line");
    let (position, _) = located.split_once(' ').unwrap_or((located, ""));
    let (line, column) = position
        .split_once(':')
        .expect("the located line is `line:col`");
    (
        line.parse().expect("line number"),
        column.parse().expect("column number"),
        message,
    )
}

/// The typo every case below carries: an unclosed object pattern in a parameter list,
/// which fails at end of file under both goals.
const TYPO: &str = "function f( {\n";
const TYPO_MESSAGE: &str = "Expected property key, found end of file";

/// A legacy sloppy script with a typo: the module attempt dies on the sloppy construct
/// (which the script retry admits), the script attempt on the typo. The typo is the
/// file's own error.
#[test]
fn a_sloppy_script_with_a_typo_reports_the_typo() {
    for (label, head) in [
        ("with statement", "with (o) {\n\ta.b = 1;\n}\n"),
        ("legacy octal literal", "var n = 010;\n"),
        ("legacy string escape", "var s = \"\\8\";\n"),
        ("await as a binding name", "var await = 1;\n"),
    ] {
        let source = format!("{head}{TYPO}");
        let typo_line = head.lines().count() + 2; // EOF, one past the typo's own line
        let (line, column, message) = fallback_error_at(&source);
        assert_eq!(
            message, TYPO_MESSAGE,
            "{label}: the reported error must be the typo, not the sloppy construct the script retry accepts"
        );
        assert_eq!(
            (line, column),
            (typo_line, 1),
            "{label}: the caret sits at the typo's end of file"
        );
    }
}

/// A module with a typo: the script attempt dies early on the module-only construct, the
/// module attempt reaches the typo. A control — the module error is the further one, so
/// either rule reports it.
#[test]
fn a_module_with_a_typo_reports_the_typo() {
    for (label, head) in [
        ("import declaration", "import x from 'y';\nlet y = 1;\n"),
        ("top-level await expression", "await x;\n"),
        ("import.meta", "let m = import.meta;\n"),
    ] {
        let source = format!("{head}{TYPO}");
        let typo_line = head.lines().count() + 2;
        let (line, column, message) = fallback_error_at(&source);
        assert_eq!(
            message, TYPO_MESSAGE,
            "{label}: the module attempt's error is the further one"
        );
        assert_eq!((line, column), (typo_line, 1), "{label}");
    }
}

/// A module whose real error sits AHEAD of the construct that makes it a module: the
/// script attempt gets past the error the module attempt died on and dies further along,
/// on the `export` / `import` / `import.meta` — a goal gate. Position would report that
/// gate, blaming a valid line of a file that is unambiguously a module; the gate settles
/// the file as a module and the module attempt's error is reported.
#[test]
fn a_module_with_its_error_ahead_of_its_export_reports_its_own_error() {
    for (label, tail) in [
        ("export at the bottom", "export { n, f };\n"),
        ("import after the definitions", "import y from 'y';\n"),
        (
            "import.meta after the definitions",
            "let m = import.meta;\n",
        ),
    ] {
        let source = format!("var n = 010;\nfunction f() {{\n\treturn n;\n}}\n{tail}");
        let (line, column, message) = fallback_error_at(&source);
        assert_eq!(
            message, "Legacy octal literals are not allowed in strict mode. Use '0o' for octal.",
            "{label}: the script attempt's death on a goal gate proves the file a module, so its module error is the file's own"
        );
        assert_eq!((line, column), (1, 9), "{label}");
    }
}

/// Both attempts fail at the SAME position with different messages: the tie goes to the
/// module attempt. `await 010;` is such a tie — at `Module` it is an await expression
/// over a legacy octal literal (the strict disallowance, at the literal), at `Script`
/// `await` is an identifier and the literal that follows it is a missing `;` (at the same
/// literal). The two goals are asked alone first so the case is a tie by evidence, not by
/// assumption: the tie-break is pinned only if the script attempt's message at that
/// position is a different one.
#[test]
fn a_tie_reports_the_module_error() {
    const MODULE_MESSAGE: &str =
        "Legacy octal literals are not allowed in strict mode. Use '0o' for octal.";
    let source = "await 010;\n";
    let arena = Bump::new();
    let module = tsv_ts::parse_with_goal(source, tsv_ts::Goal::Module, &arena)
        .expect_err("the octal literal rejects at Module");
    let script = tsv_ts::parse_with_goal(source, tsv_ts::Goal::Script, &arena)
        .expect_err("the missing `;` rejects at Script");
    assert_eq!(
        module.position(),
        script.position(),
        "the case must be a tie"
    );
    assert!(module.to_string().starts_with(MODULE_MESSAGE), "{module}");
    assert!(script.to_string().starts_with("Expected ';'"), "{script}");

    let (line, column, message) = fallback_error_at(source);
    assert_eq!(message, MODULE_MESSAGE);
    assert_eq!((line, column), (1, 7));
}

/// A file invalid under both grammars with no typo at all — a module-only construct and
/// a sloppy-only construct in one file. Either error is a half-truth, and the script
/// attempt dies on the module-only construct — a goal gate — wherever it sits, so the
/// ORDER does not decide: both spellings report the `with`, the module attempt's error
/// (the first is the case `tests/cli_tests.rs` and `scripts/test_npm.ts` pin; the second
/// is where position alone would have reported the `import`).
#[test]
fn a_mixed_file_reports_the_module_error_whatever_the_order() {
    const WITH_MESSAGE: &str = "The 'with' statement is not allowed in strict mode";
    let (line, column, message) = fallback_error_at("import x from 'y';\nwith (a) {\n\tb;\n}\n");
    assert_eq!(message, WITH_MESSAGE);
    assert_eq!((line, column), (2, 1));

    let (line, column, message) =
        fallback_error_at("with (o) {\n\ta.b = 1;\n}\nlet y = 1;\nimport x from 'y';\n");
    assert_eq!(message, WITH_MESSAGE);
    assert_eq!((line, column), (1, 1));
}

/// The control: a set goal is exact and takes no retry, so its error is its own whatever
/// the other grammar would have said.
#[test]
fn a_named_goal_reports_its_own_error() {
    let source = "with (o) {\n\ta.b = 1;\n}\nfunction f( {\n";
    let arena = Bump::new();
    let module = tsv_ts::parse_with_goal_or_fallback(source, Some(tsv_ts::Goal::Module), &arena)
        .expect_err("with rejects at Module")
        .to_string();
    assert!(
        module.starts_with("The 'with' statement is not allowed in strict mode"),
        "{module}"
    );
    let script = tsv_ts::parse_with_goal_or_fallback(source, Some(tsv_ts::Goal::Script), &arena)
        .expect_err("the typo rejects at Script")
        .to_string();
    assert!(script.starts_with(TYPO_MESSAGE), "{script}");
}
