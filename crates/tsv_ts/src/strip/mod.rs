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

impl fmt::Display for StripError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(f),
            Self::Unsupported { kind, span } => write!(
                f,
                "TypeScript syntax type blanking cannot erase ({kind:?}) at bytes {}..{}",
                span.start, span.end
            ),
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
