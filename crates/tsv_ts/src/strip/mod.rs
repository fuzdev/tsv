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
//!   blanked whole as the lone body of an `if` / loop / label always becomes `;`, so the
//!   output stays JS and the statement after it cannot become the body (`if (a) g()`). The
//!   one such body tsc's grammar allows, as the body of an `if` or a loop, is an overload
//!   signature, which ts-blank-space blanks plain;
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

use tsv_lang::{ParseError, Span};

/// Strip the types from TypeScript `source` (module goal), keeping every surviving
/// character at its line and UTF-16 column.
///
/// The caller owns `arena`, as with [`crate::parse`]: a loader keeps one per worker and
/// resets it between modules.
///
/// # Errors
///
/// [`StripError::Parse`] when `source` does not parse; [`StripError::Unsupported`] when it
/// holds TypeScript syntax that type blanking cannot erase.
pub fn strip<'s>(source: &'s str, arena: &bumpalo::Bump) -> Result<Stripped<'s>, StripError> {
    let program = crate::parse(source, arena).map_err(StripError::Parse)?;
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
    Parse(ParseError),
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
    /// The error's message rendered against `source`, the text that was stripped: a parse
    /// error's own text (what goes wrong, then the `line:column` header, the line and a
    /// caret), and a refusal as `cannot strip <the construct>` over the same header,
    /// line and caret at the refused span's start, in [`crate::WIRE_COORDINATES`] — one
    /// shape for both, so a caller prints either the same way.
    #[must_use]
    pub fn message(&self, source: &str) -> String {
        match self {
            Self::Parse(error) => error.to_string(),
            Self::Unsupported { kind, span } => ParseError::refusal(
                &format!("cannot strip {}", kind.description()),
                source,
                span.start as usize,
                crate::WIRE_COORDINATES,
            )
            .to_string(),
        }
    }
}

/// The message without its position (the position is the source's to render:
/// [`StripError::message`]).
impl fmt::Display for StripError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(f),
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
        let arena = bumpalo::Bump::new();
        match strip(source, &arena) {
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
        assert_eq!(error.message(source), parse.to_string());
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
