//! The source type strip: TypeScript source in, JS source out, by blanking.
//!
//! Every type-only range becomes whitespace — one space per UTF-16 unit, with each line
//! terminator inside it kept verbatim — so every surviving character keeps its line and
//! UTF-16 column and no sourcemap is needed. The only other bytes written are single
//! `;` / `(` / `)` substitutions inside a blanked range, where pure blanking would let ASI
//! (or a line break before `=>` or a call's `(`) change the parse. TypeScript with runtime
//! semantics (`enum`, a value `namespace`, parameter properties, `import =`, `export =`,
//! `<T>x`) is refused, never transformed.
//!
//! The verdicts are TypeScript's erasure semantics, the ones ts-blank-space and Node's
//! `module.stripTypeScriptTypes` share. They are not the Svelte compiler's: that product
//! erases a tree, follows Svelte's own oracle, and shares no code with this one.
//!
//! The parse goal is the format path's (`strip_in`): a named goal is exact, and an
//! unset one reads the source as a module, retried as a script only if that fails. No
//! verdict below depends on the goal; a script's `with` body is one more lone statement
//! body.
//!
//! Two passes after the parse: one walk over the AST (`walk`) collects the type-only ranges,
//! the hazard substitutions and the refusals, locating the bytes the AST holds no position
//! for (a modifier keyword, a `?` / `!` mark, a paren the parser stripped) with
//! `tsv_lang::source_scan`; then one copy of the source (`blank`) writes the blanks. The
//! strip shares nothing with the printer.
//!
//! **The hazards.** Each substitution is one character inside a blanked range:
//!
//! - a `;` starting a statement or class member blanked whole after JS that does not end in
//!   `;` (`let x = y⏎type T = 1⏎(z)`), in ts-blank-space's cases, plus one: a statement
//!   blanked whole as the lone body of an `if` / loop / label / `with` always becomes `;`,
//!   so the output stays JS and the statement after it cannot become the body
//!   (`if (a) g()`). The one such body tsc's grammar allows, as the body of an `if`, a loop
//!   or a `with`, is an overload signature, which ts-blank-space blanks plain;
//! - a `;` starting a bodiless signature (an overload, an abstract method, an index
//!   signature) after an expression left open — a statement or field initializer with no
//!   `;` — where the line after it would continue that expression: it opens with `(`, `[`,
//!   a template, `+` / `-` (not `++` / `--`), `/`, `*`, or a key `in` / `instanceof`, and
//!   only with an operator after a postfix `++` / `--`. ts-blank-space misses these;
//! - a `;` in place of a statement-final `as` / `satisfies` (`let x = y as T⏎(z)`);
//! - a `;` in place of a blanked first modifier before a computed key (`a = 1⏎public [k]`),
//!   or, after a field whose initializer has no `;`, before a generator method or a key
//!   named `in` / `instanceof` (`a = x⏎private in`, which would read `x in …`);
//! - a `)` moved to the end of an arrow's blanked return type when a line break precedes
//!   the `=>`, and a `(` moved to the start of blanked type parameters when a line break
//!   precedes the parameters' `(`.
//!
//! A line break here is any ECMAScript line terminator (LF, CR, U+2028, U+2029), where
//! ts-blank-space tests LF alone.
//!
//! **Assumed tsc-checked.** The parser accepts some shapes tsc rejects that carry a value —
//! a class field `declare x = 1` or `abstract x = 1`, `declare let a = 1`, a bodied member
//! of a `declare class` or `declare namespace`. Each is blanked whole, as ts-blank-space
//! blanks it (Node refuses some), so the value it would have carried is silently gone;
//! refusing them is the diagnostics layer's job, not the strip's.
//!
//! The fixtures under `tests/fixtures_strip/` define the output, graded by
//! `tests/strip_fixtures_tests.rs`.

mod blank;
mod scan;
mod walk;

use std::borrow::Cow;
use std::fmt;

use tsv_lang::{Error, ErrorKind, ErrorReport, Span};

/// Strip the types from TypeScript `source`, keeping every surviving character at its line
/// and UTF-16 column.
///
/// No goal is named, so the source is read as [`format()`](crate::format()) reads it: as a
/// module, retried as a script only if that fails (see [Goals](crate#goals)). The output is
/// JS at the goal the source was read at; nothing the walk decides depends on the goal.
/// [`strip_with_goal`] names one exactly.
///
/// # Errors
///
/// [`StripError::Parse`] when `source` does not parse at either goal (the error reported
/// is the one [Goals](crate#goals) picks); [`StripError::Unsupported`] when it holds
/// TypeScript syntax that type blanking cannot erase.
pub fn strip(source: &str) -> Result<Stripped<'_>, StripError> {
    strip_one_shot(source, None)
}

/// [`strip`] at exactly `goal`, with no Module-to-Script fallback: a source the goal's
/// grammar rejects is a [`StripError::Parse`] even when the other goal would accept it.
///
/// # Errors
///
/// [`StripError::Parse`] when `source` does not parse at `goal`; [`StripError::Unsupported`]
/// as [`strip`].
pub fn strip_with_goal(source: &str, goal: crate::Goal) -> Result<Stripped<'_>, StripError> {
    strip_one_shot(source, Some(goal))
}

/// The one-shot strips' arena: a fresh one sized to the source, dropped on return — the
/// [`Stripped`] output borrows only `source`.
fn strip_one_shot(source: &str, goal: Option<crate::Goal>) -> Result<Stripped<'_>, StripError> {
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    strip_in(source, goal, &arena)
}

/// The strip over a caller-owned `arena` — the hidden primitive beneath [`strip`] and
/// [`strip_with_goal`], for a hot loop that keeps one arena warm across sources (the
/// bindings' per-thread arena), reset by the caller between calls.
///
/// The parse is `__private::parse_with_goal_or_fallback`'s: `Some(goal)` is exact, and
/// `None` parses at `Module`, retried at `Script` only if that fails.
///
/// # Errors
///
/// As [`strip`].
pub fn strip_in<'s>(
    source: &'s str,
    goal: Option<crate::Goal>,
    arena: &bumpalo::Bump,
) -> Result<Stripped<'s>, StripError> {
    let program = crate::__private::parse_with_goal_or_fallback(source, goal, arena)
        .map_err(StripError::Parse)?;
    let mut walk = walk::Walk::new(source, arena);
    walk.program(&program);
    let mut edits = walk.finish()?;
    let code = if edits.is_empty() {
        Cow::Borrowed(source)
    } else {
        Cow::Owned(blank::apply(source, &mut edits))
    };
    Ok(Stripped { code })
}

/// A successful [`strip`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stripped<'s> {
    /// The JS. `Cow::Borrowed(source)` when nothing is type-only: no copy.
    pub code: Cow<'s, str>,
    // TODO: `imports` — the static import and re-export specifiers the walk visits, for a
    // parallel module loader. Reserved as shape; it lands with a consumer.
}

/// Why [`strip`] produced no output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StripError {
    /// The source does not parse.
    Parse(Error),
    /// The source holds TypeScript syntax that type blanking cannot erase: a construct
    /// with runtime semantics, or an assertion whose blanking would regroup the expression.
    Unsupported {
        /// What was refused.
        kind: UnsupportedKind,
        /// Its byte range in the source; [`crate::WIRE_COORDINATES`] converts each end to
        /// a line and UTF-16 column.
        span: Span,
    },
}

/// The TypeScript [`strip`] refuses: syntax that type blanking cannot erase, because
/// erasure would have to transform it rather than blank it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnsupportedKind {
    /// A non-`declare` `enum`, `const enum` included.
    Enum,
    /// A non-`declare` `namespace` holding a value member.
    ValueNamespace,
    /// A parameter property: `public` / `private` / `protected` / `readonly` / `override`
    /// on a constructor parameter. The span is the first such modifier.
    ParameterProperty,
    /// `import x = require('y')` / `import x = A.B`, `import type` and `export import`
    /// included.
    ImportEquals,
    /// `export = x`.
    ExportEquals,
    /// An angle-bracket type assertion, `<T>x`.
    TypeAssertion,
    /// An `as` / `satisfies` whose blanking would change how the expression around it
    /// groups: a binary operator after the assertion that binds tighter than the one inside
    /// it, so the blanked `a + b as T / c` reads `a + (b / c)` (a different program); or a
    /// unary operand of `**`, so the blanked `-a as T ** 2` reads `-a ** 2` (a syntax
    /// error). The span is the assertion expression.
    AssertionGrouping,
}

impl UnsupportedKind {
    /// The construct's wire name — snake_case, the `construct` a JS refusal carries
    /// (`enum`, `value_namespace`, `parameter_property`, `import_equals`, `export_equals`,
    /// `type_assertion`, `assertion_grouping`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Enum => "enum",
            Self::ValueNamespace => "value_namespace",
            Self::ParameterProperty => "parameter_property",
            Self::ImportEquals => "import_equals",
            Self::ExportEquals => "export_equals",
            Self::TypeAssertion => "type_assertion",
            Self::AssertionGrouping => "assertion_grouping",
        }
    }

    /// The construct as a refusal's message names it, after `cannot strip `.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Enum => "an enum",
            Self::ValueNamespace => "a namespace with a value member",
            Self::ParameterProperty => "a parameter property",
            Self::ImportEquals => "an 'import =' declaration",
            Self::ExportEquals => "an 'export =' assignment",
            Self::TypeAssertion => "an angle-bracket type assertion",
            Self::AssertionGrouping => "an assertion whose blanking would regroup its expression",
        }
    }
}

impl StripError {
    /// What kind of failure this is: a refusal ([`ErrorKind::Refusal`]) for
    /// [`StripError::Unsupported`], and the parse error's own kind for
    /// [`StripError::Parse`] — a syntax error, a depth limit or a source over the size cap.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Parse(error) => error.kind(),
            Self::Unsupported { .. } => ErrorKind::Refusal,
        }
    }

    /// The error's message rendered against `source`, the text that was stripped: a parse
    /// error's own text (what goes wrong, then the `line:column` header, the line and a
    /// caret), and a refusal as `cannot strip <the construct>` over the same header,
    /// line and caret at the refused span's start, in [`crate::WIRE_COORDINATES`] — one
    /// shape for both, so a caller prints either the same way.
    #[must_use]
    pub fn message(&self, source: &str) -> String {
        match self {
            Self::Parse(error) => format!("{error:#}"),
            Self::Unsupported { kind, span } => {
                let report = kind.report(*span, source);
                format!("{}\n{}", report.summary, report.frame)
            }
        }
    }
}

impl UnsupportedKind {
    /// A refusal of this construct over `span` as a binding reports it
    /// ([`ErrorReport::refusal`]): the summary `cannot strip <the construct>`, the frame
    /// [`StripError::message`] prints under it, and the span, in
    /// [`crate::WIRE_COORDINATES`]. Infallible, unlike a parse error's
    /// [`Error::report`]: a refusal always has its span.
    #[must_use]
    pub fn report(self, span: Span, source: &str) -> ErrorReport {
        ErrorReport::refusal(
            &format!("cannot strip {}", self.description()),
            source,
            span.start as usize,
            span.end as usize,
            crate::WIRE_COORDINATES,
        )
    }
}

/// A parse error as its [`Error`] displays — one line, the frame under `{:#}` — and a
/// refusal's summary alone, without its position (a refusal's position is the source's to
/// render: [`StripError::message`]).
impl fmt::Display for StripError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => fmt::Display::fmt(error, f),
            Self::Unsupported { kind, .. } => write!(f, "cannot strip {}", kind.description()),
        }
    }
}

impl std::error::Error for StripError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Unsupported { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StripError, UnsupportedKind, strip};

    fn strip_error(source: &str) -> StripError {
        match strip(source) {
            Err(error) => error,
            Ok(stripped) => panic!("expected a strip error, stripped to {:?}", stripped.code),
        }
    }

    /// A refusal's message reads like a parse error's: the construct, then the `line:col`
    /// header of the span's start (1-based column, UTF-16 units), the line, a caret.
    #[test]
    fn refusal_message_names_the_construct_at_its_point() {
        let source = "let \u{1d49c}: T = 1;\nenum E {}\n";
        let error = strip_error(source);
        assert!(matches!(
            error,
            StripError::Unsupported {
                kind: UnsupportedKind::Enum,
                ..
            }
        ));
        assert_eq!(error.to_string(), "cannot strip an enum");
        assert_eq!(
            error.message(source),
            "cannot strip an enum\n2:1 enum E {}\n    ^ here"
        );
    }

    #[test]
    fn parse_error_message_is_the_parse_errors_own() {
        let source = "let = ;\n";
        let error = strip_error(source);
        let StripError::Parse(parse) = &error else {
            panic!("expected a parse error: {error:?}");
        };
        assert_eq!(error.message(source), format!("{parse:#}"));
    }

    fn stripped(source: &str) -> String {
        match strip(source) {
            Ok(stripped) => stripped.code.into_owned(),
            Err(error) => panic!("expected {source:?} to strip: {}", error.message(source)),
        }
    }

    /// The rendered parse error the strip reports for `source`, and the one the parse at
    /// `goal` reports, which it must be.
    fn parse_errors(source: &str, goal: crate::Goal) -> (String, String) {
        let StripError::Parse(stripped) = strip_error(source) else {
            panic!("expected a parse error for {source:?}");
        };
        let arena = bumpalo::Bump::new();
        let Err(parsed) = crate::__private::parse_with_goal(source, goal, &arena) else {
            panic!("expected {source:?} to fail at {goal:?}");
        };
        (stripped.to_string(), parsed.to_string())
    }

    /// With no goal named, a source the module grammar rejects is retried as a script, as
    /// the format path's parse retries it: a legacy sloppy script strips. Each expected
    /// output is ts-blank-space's, byte for byte.
    #[test]
    fn an_unset_goal_retries_a_failed_module_parse_as_a_script() {
        for (source, code) in [
            (
                "fs.chmodSync(p as string, 0755);\n",
                "fs.chmodSync(p          , 0755);\n",
            ),
            ("with (o as any) { x; }\n", "with (o       ) { x; }\n"),
            ("var await: number = 1;\n", "var await         = 1;\n"),
            ("let n = 08 as number;\n", "let n = 08          ;\n"),
            (
                "module.exports = function (mode: number = 0644) { return mode; };\n",
                "module.exports = function (mode         = 0644) { return mode; };\n",
            ),
            // the script's own hazard sites take the same `;` the module's do
            (
                "var await: any;\nawait\ntype T = 1\n(z)\n",
                "var await     ;\nawait\n;         \n(z)\n",
            ),
        ] {
            assert_eq!(stripped(source), code, "{source:?}");
        }
    }

    /// A decorated static block keeps its decorators, as any member's, and their type
    /// arguments blank; the output is ts-blank-space's, byte for byte.
    #[test]
    fn a_static_block_decorator_blanks_its_type_arguments() {
        assert_eq!(
            stripped("class A { @a<T>() static {} }\n"),
            "class A { @a   () static {} }\n"
        );
    }

    /// A `with` body is a lone statement body like an `if`'s: a statement blanked whole
    /// there becomes `;`, so the statement after it cannot become the body. ts-blank-space
    /// blanks the overload plain, which hands `f()` to the `with`.
    #[test]
    fn a_with_body_blanked_whole_becomes_an_empty_statement() {
        assert_eq!(
            stripped("with (o) type T = 1;\nf();\n"),
            "with (o) ;          \nf();\n"
        );
        assert_eq!(
            stripped("with (o) function f(): void;\nf();\n"),
            "with (o) ;                  \nf();\n"
        );
    }

    /// When both grammars reject the source, the error is the one the format path's
    /// fallback reports: the script attempt's when it reached further without dying on a
    /// module-only construct — a broken sloppy script's own typo, not the `with` the retry
    /// admits — and otherwise the module attempt's.
    #[test]
    fn a_failed_retry_reports_the_attempt_the_source_was_written_for() {
        let (stripped, script) = parse_errors("with (o) {}\nlet = ;\n", crate::Goal::Script);
        assert_eq!(stripped, script, "the sloppy script's typo");
        // a goal gate proves a module: its error is the file's own, though it sits first
        let (stripped, module) =
            parse_errors("let x: number = 0755;\nexport {};\n", crate::Goal::Module);
        assert_eq!(stripped, module, "the module's legacy octal");
        let (stripped, module) = parse_errors(
            "import type { A } from 'a';\nlet x: A = 0755;\n",
            crate::Goal::Module,
        );
        assert_eq!(stripped, module, "a type-only import is a goal gate too");
        // a script with a `"use strict"` prologue is strict at either goal: a tie, the module's
        let (stripped, module) = parse_errors(
            "\"use strict\"; let x: number = 010;\n",
            crate::Goal::Module,
        );
        assert_eq!(stripped, module, "a strict script");
    }

    /// Control: the retry runs on a module-parse failure alone, so a module-valid source
    /// strips as the module it is — a top-level `await` stays an expression.
    #[test]
    fn a_module_valid_source_is_never_read_as_a_script() {
        assert_eq!(stripped("await f() as T;\n"), "await f()     ;\n");
        assert_eq!(
            stripped("let x: number = 1;\nexport { x };\n"),
            "let x         = 1;\nexport { x };\n"
        );
    }

    /// A chain of links — members, calls, `new`, tagged templates, `!`, instantiations,
    /// `as` / `satisfies` — blanks each link's TypeScript, its operand's and its arguments',
    /// and a statement-final assertion inside one takes its `;`.
    #[test]
    fn a_chain_blanks_every_links_typescript() {
        for (source, code) in [
            (
                "x = new a.b<T>(c satisfies D)!.e;\n",
                "x = new a.b   (c            ) .e;\n",
            ),
            ("x = a!.b![\"c\"]!(d!)!;\n", "x = a .b [\"c\"] (d ) ;\n"),
            (
                "x = a<T>`q`.b<U>`r${c as D}`;\n",
                "x = a   `q`.b   `r${c     }`;\n",
            ),
            (
                "x = a.b<T>(c)<U>(d).e as F as G;\n",
                "x = a.b   (c)   (d).e          ;\n",
            ),
            (
                "x = (a as T).b(c!)?.d<E>?.(f as G);\n",
                "x = (a     ).b(c )?.d   ?.(f     );\n",
            ),
            ("x = f<T>;\n", "x = f   ;\n"),
            ("y = a.b as T\n[1].c\n", "y = a.b;    \n[1].c\n"),
            (
                "class C { p = a.b as T\n[k]: number = 1 }\n",
                "class C { p = a.b;    \n[k]         = 1 }\n",
            ),
        ] {
            assert_eq!(stripped(source), code, "{source:?}");
        }
    }

    /// The same chains under twenty more links, past the links a recursion walks
    /// (`CHAIN_FRAMES` in `walk.rs`): their links are walked by the chain loop, which blanks
    /// them as the recursion does.
    #[test]
    fn a_chain_past_the_recursion_blanks_as_the_recursion_does() {
        let links = ".z".repeat(20);
        for (chain, code) in [
            (
                "new a.b<T>(c satisfies D)!.e",
                "new a.b   (c            ) .e",
            ),
            ("a!.b![\"c\"]!(d!)!", "a .b [\"c\"] (d ) "),
            ("a<T>`q`.b<U>`r${c as D}`", "a   `q`.b   `r${c     }`"),
            (
                "(a.b<T>(c)<U>(d).e as F as G)",
                "(a.b   (c)   (d).e          )",
            ),
            (
                "(a as T).b(c!)?.d<E>?.(f as G)",
                "(a     ).b(c )?.d   ?.(f     )",
            ),
            ("a[b as K]!.c", "a[b     ] .c"),
            ("(a satisfies T).b", "(a            ).b"),
        ] {
            let source = format!("x = {chain}{links};\n");
            assert_eq!(
                stripped(&source),
                format!("x = {code}{links};\n"),
                "{source:?}"
            );
        }
    }

    /// An assertion's grouping check runs before its operand is walked, so a refusal inside
    /// the operand starting where the assertion does is not the one reported: the
    /// assertion's is, over its whole span. On the recursion path: a chain past
    /// `CHAIN_FRAMES` holds no assertion a binary operator follows, so none of its
    /// assertions regroups.
    #[test]
    fn a_chain_assertion_is_refused_before_its_operand_is_walked() {
        let source = "x = <T>a + b as U * c;\n";
        let StripError::Unsupported { kind, span } = strip_error(source) else {
            panic!("expected a refusal");
        };
        assert_eq!(kind, UnsupportedKind::AssertionGrouping);
        assert_eq!(
            &source[span.start as usize..span.end as usize],
            "<T>a + b as U"
        );
    }

    /// A chain over a thousand links long strips on a small stack: its links cost heap, not
    /// a frame each.
    #[test]
    fn a_long_chain_strips_without_a_frame_per_link() {
        let source = format!("x = a{}{};\n", ".b!".repeat(1000), " as T".repeat(100));
        let stripped = std::thread::Builder::new()
            .stack_size(256 << 10)
            .spawn(move || {
                strip(&source)
                    .map(|stripped| stripped.code.into_owned())
                    .map_err(|error| error.message(&source))
            })
            .expect("spawn")
            .join()
            .expect("the strip ran");
        let expected = format!("x = a{}{};\n", ".b ".repeat(1000), " ".repeat(500));
        assert_eq!(stripped, Ok(expected));
    }

    #[test]
    fn every_kind_has_a_snake_case_name() {
        for (kind, name) in [
            (UnsupportedKind::Enum, "enum"),
            (UnsupportedKind::ValueNamespace, "value_namespace"),
            (UnsupportedKind::ParameterProperty, "parameter_property"),
            (UnsupportedKind::ImportEquals, "import_equals"),
            (UnsupportedKind::ExportEquals, "export_equals"),
            (UnsupportedKind::TypeAssertion, "type_assertion"),
            (UnsupportedKind::AssertionGrouping, "assertion_grouping"),
        ] {
            assert_eq!(kind.name(), name);
            assert!(!kind.description().is_empty());
        }
    }
}
