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
//! The fixtures under `tests/fixtures_strip/` define the output, graded by
//! `tests/strip_fixtures_tests.rs`.
//!
//! TODO: the strip walk itself. [`strip`] is the API shape over a placeholder body that
//! parses and returns the source unchanged.

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
    // TODO: walk the program, collect the type-only ranges and the hazard substitutions,
    // and refuse the runtime-semantics nodes; this placeholder strips nothing.
    crate::parse(source, arena).map_err(StripError::Parse)?;
    Ok(Stripped {
        code: Cow::Borrowed(source),
    })
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
