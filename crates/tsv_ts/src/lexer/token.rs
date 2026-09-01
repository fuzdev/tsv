// Token types for TypeScript/JS lexer

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeywordKind {
    // Declaration keywords
    Const = 0,
    Let = 1,
    Var = 2,
    // Literal keywords
    True = 3,
    False = 4,
    Null = 5,
    Undefined = 6,
    // Type keywords
    Number = 7,
    String = 8,
    Boolean = 9,
    Any = 10,
    Void = 11,
    Never = 12,
    Unknown = 13,
    Object = 14,
    Symbol = 15,
    Bigint = 16,
    // Expression keywords
    New = 17,
    // Binary operator keywords
    Instanceof = 18,
    In = 19,
    // Control flow keywords
    Return = 20,
    If = 30,
    Else = 31,
    For = 32,
    While = 33,
    Do = 34,
    Switch = 35,
    Case = 36,
    Default = 37,
    Break = 38,
    Continue = 39,
    Try = 40,
    Catch = 41,
    Finally = 42,
    Throw = 43,
    // Declaration keywords (continued)
    Function = 21,
    Class = 22,
    Enum = 49,
    // Unary keyword operators
    Typeof = 23,
    Delete = 24,
    // Async/await keywords
    Async = 25,
    Await = 26,
    // Class keywords
    This = 50,
    Super = 27,
    Extends = 28,
    // Module keywords
    Export = 29,
    Import = 44,
    From = 45,
    As = 46,
    Satisfies = 47,
    // Generator keywords
    Yield = 48,
    // Debugger
    Debugger = 51,
    // Sloppy-mode-only statement, reserved everywhere (see `KEYWORDS`)
    With = 52,
}

impl KeywordKind {
    /// Returns the string representation of the keyword
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            KeywordKind::Const => "const",
            KeywordKind::Let => "let",
            KeywordKind::Var => "var",
            KeywordKind::True => "true",
            KeywordKind::False => "false",
            KeywordKind::Null => "null",
            KeywordKind::Undefined => "undefined",
            KeywordKind::Number => "number",
            KeywordKind::String => "string",
            KeywordKind::Boolean => "boolean",
            KeywordKind::Any => "any",
            KeywordKind::Void => "void",
            KeywordKind::Never => "never",
            KeywordKind::Unknown => "unknown",
            KeywordKind::Object => "object",
            KeywordKind::Symbol => "symbol",
            KeywordKind::Bigint => "bigint",
            KeywordKind::New => "new",
            KeywordKind::Instanceof => "instanceof",
            KeywordKind::In => "in",
            KeywordKind::Return => "return",
            KeywordKind::If => "if",
            KeywordKind::Else => "else",
            KeywordKind::For => "for",
            KeywordKind::While => "while",
            KeywordKind::Do => "do",
            KeywordKind::Switch => "switch",
            KeywordKind::Case => "case",
            KeywordKind::Default => "default",
            KeywordKind::Break => "break",
            KeywordKind::Continue => "continue",
            KeywordKind::Try => "try",
            KeywordKind::Catch => "catch",
            KeywordKind::Finally => "finally",
            KeywordKind::Throw => "throw",
            KeywordKind::Function => "function",
            KeywordKind::Class => "class",
            KeywordKind::Enum => "enum",
            KeywordKind::Typeof => "typeof",
            KeywordKind::Delete => "delete",
            KeywordKind::Async => "async",
            KeywordKind::Await => "await",
            KeywordKind::This => "this",
            KeywordKind::Super => "super",
            KeywordKind::Extends => "extends",
            KeywordKind::Export => "export",
            KeywordKind::Import => "import",
            KeywordKind::From => "from",
            KeywordKind::As => "as",
            KeywordKind::Satisfies => "satisfies",
            KeywordKind::Yield => "yield",
            KeywordKind::Debugger => "debugger",
            KeywordKind::With => "with",
        }
    }

    /// Returns true if this is a type keyword (number, string, boolean, etc.)
    #[inline]
    pub const fn is_type_keyword(self) -> bool {
        matches!(
            self,
            KeywordKind::Number
                | KeywordKind::String
                | KeywordKind::Boolean
                | KeywordKind::Any
                | KeywordKind::Void
                | KeywordKind::Never
                | KeywordKind::Unknown
                | KeywordKind::Object
                | KeywordKind::Symbol
                | KeywordKind::Bigint
                | KeywordKind::Null
                | KeywordKind::Undefined
        )
    }

    /// Returns true if this keyword can be used as an identifier in certain contexts.
    ///
    /// These are "contextual keywords" that only have keyword semantics in specific
    /// syntactic positions. In other positions (like variable names), they're valid identifiers.
    ///
    /// Examples:
    /// - `let async = 1;` - `async` is an identifier
    /// - `async function f() {}` - `async` is a keyword
    /// - `let from = 'x';` - `from` is an identifier
    /// - `import x from 'y';` - `from` is a keyword
    #[inline]
    pub const fn can_be_identifier(self) -> bool {
        matches!(
            self,
            // Contextual keywords that can be identifiers
            KeywordKind::Async
                | KeywordKind::Await
                | KeywordKind::From
                | KeywordKind::As
                | KeywordKind::Satisfies
                | KeywordKind::Let
                | KeywordKind::Yield
                // Type keywords are also valid identifiers in value positions
                | KeywordKind::Number
                | KeywordKind::String
                | KeywordKind::Boolean
                | KeywordKind::Any
                // ⚠️ `void` is deliberately NOT here, for the same reason
                // [`KeywordKind::can_be_binding_name`] omits it: it is a genuine
                // `ReservedWord`, so `Identifier : IdentifierName but not ReservedWord`
                // excludes it at the PRODUCTION level — not a deferrable early error.
                // tsc's parser rejects (`function void() {}` → TS1359/TS1109/TS1005) and
                // so does acorn. It sat here once, and the only position that could see
                // it was a function name, which reaches this predicate rather than the
                // binding one — so `function void() {}` parsed while `var void = 1`,
                // `class void {}` and `function f(void)` all correctly rejected.
                | KeywordKind::Never
                | KeywordKind::Unknown
                | KeywordKind::Object
                | KeywordKind::Symbol
                | KeywordKind::Bigint
                // `undefined` is a global identifier, not a ReservedWord
                | KeywordKind::Undefined
        )
    }

    /// Returns true if this keyword can be used as a binding name (variable name, parameter).
    ///
    /// Exactly [`KeywordKind::can_be_identifier`] minus `await`, and the difference
    /// is load-bearing: `await`'s bar is *goal-dependent* (reserved under
    /// `Goal::Module`, an ordinary identifier at `Goal::Script` `[~Await]`), so
    /// every caller answers it with its own `Parser::await_is_identifier` arm
    /// ordered **after** the arm that consults this predicate. Folding `await` in
    /// here would make it a binding name at Module goal.
    ///
    /// `let` and `yield` ARE binding names. Neither is excluded by a *production*:
    /// `let` is not a `ReservedWord` at all, and `BindingIdentifier[Yield, Await] :
    /// Identifier | `yield` | `await`` admits `yield` unconditionally (ecma262
    /// §sec-identifiers — the `[Yield]` bar is written as an early error, not a
    /// production guard, so that ASI can't split `let ⏎ await 0;`). Their only bar
    /// is the strict-mode Static Semantics bullet in
    /// §sec-identifiers-static-semantics-early-errors, which tsv defers to the
    /// diagnostics layer — the same bullet, and the same deferral, that already let
    /// `implements` / `interface` / `package` / `private` / `static` be binding
    /// names here (those lex as plain identifiers and never reach this predicate).
    /// Real tsc's parser accepts both in every `BindingIdentifier` position and
    /// prettier formats them.
    ///
    /// ⚠️ `void` is NOT here: it is a genuine `ReservedWord`, so `Identifier :
    /// IdentifierName but not ReservedWord` excludes it in a production.
    ///
    /// ⚠️ Two callers read this set in an *expression* context, where `yield` may be
    /// the operator instead — the single-param arrow start and object-literal
    /// shorthand both re-gate it on the parser's `in_yield`. See
    /// `Parser::parse_primary_expression`.
    ///
    /// Examples:
    /// - `const as = 1;` - valid, `as` can be a binding name
    /// - `function fn(yield: string) {}` - valid, the strict early error is deferred
    /// - `const await = 1;` - INVALID at Module goal, handled by the callers
    #[inline]
    pub const fn can_be_binding_name(self) -> bool {
        matches!(
            self,
            // Fully contextual keywords that can be binding names
            KeywordKind::Async
                | KeywordKind::From
                | KeywordKind::As
                | KeywordKind::Satisfies
                // Strict-mode-reserved by an early error only, so tsv defers
                | KeywordKind::Let
                | KeywordKind::Yield
                // Contextual type keywords are also valid binding names in value
                // positions (`let string = 1`, `class any {}`). `void` is NOT among
                // them — it is a reserved unary operator, not a contextual keyword, so
                // `let void`/`class void {}` are syntax errors (acorn/tsc reject).
                | KeywordKind::Number
                | KeywordKind::String
                | KeywordKind::Boolean
                | KeywordKind::Any
                | KeywordKind::Never
                | KeywordKind::Unknown
                | KeywordKind::Object
                | KeywordKind::Symbol
                | KeywordKind::Bigint
                // `undefined` is a global identifier, not a ReservedWord — it is
                // a valid binding name (`var undefined;`). The strict-mode
                // restriction on `undefined` is a runtime concern, not parse-time.
                | KeywordKind::Undefined
        )
        // NOTE: `await` is excluded (goal-dependent, resolved by each caller's own
        // arm) and `void` is excluded (a `ReservedWord`, barred by a production).
    }
}

impl fmt::Display for KeywordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Number,
    String,
    Identifier,
    Keyword(KeywordKind),
    Equals,
    Colon,
    Semicolon,
    Comma,
    BraceOpen,          // {
    BraceClose,         // }
    BracketOpen,        // [
    BracketClose,       // ]
    ParenOpen,          // (
    ParenClose,         // )
    Arrow,              // =>
    Dot,                // .
    DotDotDot,          // ...
    Minus,              // -
    MinusMinus,         // --
    Plus,               // +
    PlusPlus,           // ++
    Star,               // *
    StarStar,           // **
    Slash,              // /
    Percent,            // %
    Caret,              // ^
    Tilde,              // ~
    LeftShift,          // <<
    RightShift,         // >>
    UnsignedRightShift, // >>>
    LessThan,           // <
    GreaterThan,        // >
    LessThanEquals,     // <=
    GreaterThanEquals,  // >=
    EqualsEquals,       // ==
    EqualsEqualsEquals, // ===
    BangEquals,         // !=
    BangEqualsEquals,   // !==
    Ampersand,          // &
    AmpersandAmpersand, // &&
    Pipe,               // |
    PipePipe,           // ||
    QuestionQuestion,   // ??
    QuestionDot,        // ?. (optional chaining)
    Bang,               // !
    Question,           // ?
    // Compound assignment operators
    PlusEquals,               // +=
    MinusEquals,              // -=
    StarEquals,               // *=
    SlashEquals,              // /=
    PercentEquals,            // %=
    StarStarEquals,           // **=
    LeftShiftEquals,          // <<=
    RightShiftEquals,         // >>=
    UnsignedRightShiftEquals, // >>>=
    AmpersandEquals,          // &=
    PipeEquals,               // |=
    CaretEquals,              // ^=
    AmpersandAmpersandEquals, // &&=
    PipePipeEquals,           // ||=
    QuestionQuestionEquals,   // ??=
    /// `content_start` is the byte offset where the comment's content begins
    /// (delimiters excluded): `start + 2` for `//` and `/* */`, `start` for a
    /// `#!` hashbang (whose content includes the `#!`). The end is derived by
    /// the parser (`end - 2` for block comments, `end` otherwise). Carrying the
    /// content start here keeps the lexer the single owner of delimiter widths.
    Comment {
        is_block: bool,
        content_start: u32,
    },
    // Template literal tokens
    // NoSubstitutionTemplate: `content` (no ${} interpolation)
    NoSubstitutionTemplate,
    // TemplateHead: `content${  (starts template with interpolation)
    TemplateHead,
    // TemplateMiddle: }content${  (between interpolations)
    TemplateMiddle,
    // TemplateTail: }content`  (ends template after interpolation)
    TemplateTail,
    // Regular expression literal: /pattern/flags
    // Pattern and flags are stored in token.decoded as "pattern\0flags" (null-separated)
    RegexLiteral,
    At,   // @ for decorators
    Hash, // # for private identifiers
    Eof,
}

impl TokenKind {
    /// Whether this token is a *binding-name word* — a plain identifier or a
    /// keyword-lexed word valid as a binding name (see
    /// [`KeywordKind::can_be_binding_name`]): the contextual keywords (`string`,
    /// `any`, …) **and** `let` / `yield`, whose only bar is a strict-mode early
    /// error tsv defers. Note the second group means a `ReservedWord` (`yield`)
    /// passes — this predicate is keyed on the *name* set, not on reserved-ness.
    /// Excludes `await` (a binding name only at Script `[~Await]`, handled at the
    /// sites that care) and the non-word binding starters (`[`, `{`, `...`,
    /// `this`).
    #[inline]
    pub fn is_binding_name_word(&self) -> bool {
        match self {
            TokenKind::Identifier => true,
            TokenKind::Keyword(kw) => kw.can_be_binding_name(),
            _ => false,
        }
    }
}

// TODO: Consider refining Display implementation for better error messages
// Current approach: Quoted tokens like '=', lowercase for others
// Alternative: Could match TypeScript/JS terminology more closely
// Examples:
// - "identifier token" instead of "identifier"
// - "number literal" instead of "number"
// - "string literal" instead of "string"
// Trade-off: Current is concise, alternative is more descriptive
// Usage in errors: "Expected property key, found {token_kind}"
impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Number => write!(f, "number"),
            TokenKind::String => write!(f, "string"),
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::Keyword(kw) => write!(f, "'{kw}'"),
            TokenKind::Equals => write!(f, "'='"),
            TokenKind::Colon => write!(f, "':'"),
            TokenKind::Semicolon => write!(f, "';'"),
            TokenKind::Comma => write!(f, "','"),
            TokenKind::BraceOpen => write!(f, "'{{'"),
            TokenKind::BraceClose => write!(f, "'}}'"),
            TokenKind::BracketOpen => write!(f, "'['"),
            TokenKind::BracketClose => write!(f, "']'"),
            TokenKind::ParenOpen => write!(f, "'('"),
            TokenKind::ParenClose => write!(f, "')'"),
            TokenKind::Arrow => write!(f, "'=>'"),
            TokenKind::Dot => write!(f, "'.'"),
            TokenKind::DotDotDot => write!(f, "'...'"),
            TokenKind::Minus => write!(f, "'-'"),
            TokenKind::MinusMinus => write!(f, "'--'"),
            TokenKind::Plus => write!(f, "'+'"),
            TokenKind::PlusPlus => write!(f, "'++'"),
            TokenKind::Star => write!(f, "'*'"),
            TokenKind::StarStar => write!(f, "'**'"),
            TokenKind::Slash => write!(f, "'/'"),
            TokenKind::Percent => write!(f, "'%'"),
            TokenKind::Caret => write!(f, "'^'"),
            TokenKind::Tilde => write!(f, "'~'"),
            TokenKind::LeftShift => write!(f, "'<<'"),
            TokenKind::RightShift => write!(f, "'>>'"),
            TokenKind::UnsignedRightShift => write!(f, "'>>>'"),
            TokenKind::LessThan => write!(f, "'<'"),
            TokenKind::GreaterThan => write!(f, "'>'"),
            TokenKind::LessThanEquals => write!(f, "'<='"),
            TokenKind::GreaterThanEquals => write!(f, "'>='"),
            TokenKind::EqualsEquals => write!(f, "'=='"),
            TokenKind::EqualsEqualsEquals => write!(f, "'==='"),
            TokenKind::BangEquals => write!(f, "'!='"),
            TokenKind::BangEqualsEquals => write!(f, "'!=='"),
            TokenKind::Ampersand => write!(f, "'&'"),
            TokenKind::AmpersandAmpersand => write!(f, "'&&'"),
            TokenKind::Pipe => write!(f, "'|'"),
            TokenKind::PipePipe => write!(f, "'||'"),
            TokenKind::QuestionQuestion => write!(f, "'??'"),
            TokenKind::QuestionDot => write!(f, "'?.'"),
            TokenKind::Bang => write!(f, "'!'"),
            TokenKind::Question => write!(f, "'?'"),
            TokenKind::PlusEquals => write!(f, "'+='"),
            TokenKind::MinusEquals => write!(f, "'-='"),
            TokenKind::StarEquals => write!(f, "'*='"),
            TokenKind::SlashEquals => write!(f, "'/='"),
            TokenKind::PercentEquals => write!(f, "'%='"),
            TokenKind::StarStarEquals => write!(f, "'**='"),
            TokenKind::LeftShiftEquals => write!(f, "'<<='"),
            TokenKind::RightShiftEquals => write!(f, "'>>='"),
            TokenKind::UnsignedRightShiftEquals => write!(f, "'>>>='"),
            TokenKind::AmpersandEquals => write!(f, "'&='"),
            TokenKind::PipeEquals => write!(f, "'|='"),
            TokenKind::CaretEquals => write!(f, "'^='"),
            TokenKind::AmpersandAmpersandEquals => write!(f, "'&&='"),
            TokenKind::PipePipeEquals => write!(f, "'||='"),
            TokenKind::QuestionQuestionEquals => write!(f, "'??='"),
            TokenKind::Comment { is_block, .. } => {
                if *is_block {
                    write!(f, "block comment")
                } else {
                    write!(f, "line comment")
                }
            }
            TokenKind::NoSubstitutionTemplate => write!(f, "template literal"),
            TokenKind::TemplateHead => write!(f, "template head"),
            TokenKind::TemplateMiddle => write!(f, "template middle"),
            TokenKind::TemplateTail => write!(f, "template tail"),
            TokenKind::RegexLiteral => write!(f, "regular expression"),
            TokenKind::At => write!(f, "'@'"),
            TokenKind::Hash => write!(f, "'#'"),
            TokenKind::Eof => write!(f, "end of file"),
        }
    }
}

// Token design with escape handling
// - `decoded`: owned string for escape-processed values (only allocated when needed)
// - Raw text: extracted via source[start..end] on demand (zero duplication)
//
// This follows the "single source of truth" principle from docs/architecture.md:
// "Raw strings are NEVER duplicated in the AST" - applies to tokens too (pre-AST).
// A 16-byte POD: small enough to return from `next_token` in registers (SysV ABI
// returns ≤16-byte integer aggregates in `rax:rdx` — no `Copy` needed) and store
// straight into the parser's `current_*` fields, with no heap-owning field to
// move. The rare decoded string (escapes only) lives out-of-band on the lexer
// (`Lexer::decode_scratch` / `decoded_str`), so the per-token value carried on the hot
// pump is just classification + span. Left non-`Copy` (like the original) so
// `TokenKind` can stay non-`Copy` and avoid a `trivially_copy_pass_by_ref` cascade
// on the many `&TokenKind` params; moving an 8-byte `TokenKind` field is just as cheap.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    /// Byte offsets into the lexer's source. `u32` (not `usize`) keeps `Token`
    /// 16 bytes; source length is capped < 4 GB upstream (`ParseError::FileTooLarge`).
    pub start: u32,
    pub end: u32,
}

// Guards the hot-path invariant: `Token` is a 16-byte `Copy` POD (returns in
// registers, no heap-owning field). Anything that re-bloats it — re-adding a
// `String`/`Box` field, widening `start`/`end` to `usize` — fails the build here.
const _: () = assert!(size_of::<Token>() == 16);

/// The reserved-word set — the independent oracle the SWAR matcher is validated
/// against, **test-only**. Production recognition is the per-length compare arms
/// in [`keyword_swar`] (length ≤ 8) and [`keyword_swar_long`] (length 9/10); there
/// is no runtime keyword table. The unit tests cross-check those arms — plus
/// [`KEYWORD_MIN_LEN`]/[`KEYWORD_MAX_LEN`] and [`KEYWORD_LENGTHS_BY_FIRST_LETTER`] —
/// against this list, so adding a keyword to one without the other fails the suite.
#[cfg(test)]
static KEYWORDS: &[(&str, KeywordKind)] = &[
    // Declaration keywords
    ("const", KeywordKind::Const),
    ("let", KeywordKind::Let),
    ("var", KeywordKind::Var),
    // Literal keywords
    ("true", KeywordKind::True),
    ("false", KeywordKind::False),
    ("null", KeywordKind::Null),
    ("undefined", KeywordKind::Undefined),
    // Type keywords
    ("number", KeywordKind::Number),
    ("string", KeywordKind::String),
    ("boolean", KeywordKind::Boolean),
    ("any", KeywordKind::Any),
    ("void", KeywordKind::Void),
    ("never", KeywordKind::Never),
    ("unknown", KeywordKind::Unknown),
    ("object", KeywordKind::Object),
    ("symbol", KeywordKind::Symbol),
    ("bigint", KeywordKind::Bigint),
    // Expression keywords
    ("new", KeywordKind::New),
    // Binary operator keywords
    ("instanceof", KeywordKind::Instanceof),
    ("in", KeywordKind::In),
    // Control flow keywords
    ("return", KeywordKind::Return),
    ("if", KeywordKind::If),
    ("else", KeywordKind::Else),
    ("for", KeywordKind::For),
    ("while", KeywordKind::While),
    ("do", KeywordKind::Do),
    ("switch", KeywordKind::Switch),
    ("case", KeywordKind::Case),
    ("default", KeywordKind::Default),
    ("break", KeywordKind::Break),
    ("continue", KeywordKind::Continue),
    ("try", KeywordKind::Try),
    ("catch", KeywordKind::Catch),
    ("finally", KeywordKind::Finally),
    ("throw", KeywordKind::Throw),
    // Declaration keywords (continued)
    ("function", KeywordKind::Function),
    ("class", KeywordKind::Class),
    ("enum", KeywordKind::Enum),
    // Unary keyword operators
    ("typeof", KeywordKind::Typeof),
    ("delete", KeywordKind::Delete),
    // Async/await keywords
    ("async", KeywordKind::Async),
    ("await", KeywordKind::Await),
    // Class keywords
    ("this", KeywordKind::This),
    ("super", KeywordKind::Super),
    ("extends", KeywordKind::Extends),
    // Module keywords
    ("export", KeywordKind::Export),
    ("import", KeywordKind::Import),
    ("from", KeywordKind::From),
    ("as", KeywordKind::As),
    ("satisfies", KeywordKind::Satisfies),
    // Generator keywords
    ("yield", KeywordKind::Yield),
    // Debugger
    ("debugger", KeywordKind::Debugger),
    // Sloppy-mode-only statement — a `ReservedWord`, so it can never be a name
    ("with", KeywordKind::With),
    // Deliberately NOT listed: the contextual keywords (`interface`, `type`, `namespace`,
    // `declare`, `abstract`, `async`, …) lex as plain identifiers and the parser
    // recognizes them by value at the sites where they can begin a construct (see
    // `parser/statement`). Minting keyword tokens for them would break every
    // Identifier-expecting site (`x.type`, `const namespace = 1`).
];

/// Shortest reserved word (`as`/`in`/`if`/`do`) is 2 bytes; longest (`instanceof`) is 10.
const KEYWORD_MIN_LEN: usize = 2;
const KEYWORD_MAX_LEN: usize = 10;

/// [`KEYWORD_LENGTHS_BY_FIRST_LETTER`] is indexed by first letter and **shifted by the
/// identifier's length**, so every admitted length must be a valid `u16` shift. The
/// length gate in [`keyword_at`] is what guarantees it — this pins the two together, so
/// adding a longer reserved word fails the build here rather than silently wrapping the
/// shift in release and rejecting the new word.
const _: () = assert!(KEYWORD_MAX_LEN < u16::BITS as usize);

/// Bit `len` of entry `b - b'a'` is set when some reserved word begins with
/// lowercase-ASCII letter `b` **and** is `len` bytes long — the pre-filter's whole
/// question in one `u16`, since a keyword's length and first letter are both free
/// at the call site (the length is the identifier's span, the first byte is already
/// loaded to compute the index).
///
/// ⭐ **The length half is what earns the table.** A first-letter-only mask admits
/// every 9- or 10-byte identifier starting with one of seventeen letters into the
/// long path, and a 6-byte one starting with `f` into the length-6 compare arm — both
/// are then rejected one whole compare chain later. Keying on the pair rejects
/// **39.8%** of the identifiers that reach here on a TypeScript corpus (163,833 of
/// 411,775 per pass) and **82.6%** of the ones that would otherwise take the length-9/10
/// path, for the same four instructions the letter-only test cost: the `u16` load
/// replaces a register-materialized immediate, and a letter with no reserved words at
/// all still rejects at every length (its entry is zero).
///
/// Kept exactly in sync with `KEYWORDS` by `prefilter_admits_every_keyword`, which
/// re-derives every entry.
#[rustfmt::skip]
const KEYWORD_LENGTHS_BY_FIRST_LETTER: [u16; 26] = {
    /// The reserved-word lengths for one first letter, as a bit per length.
    const fn lens(lengths: &[usize]) -> u16 {
        let mut m = 0u16;
        let mut i = 0;
        while i < lengths.len() {
            m |= 1 << lengths[i];
            i += 1;
        }
        m
    }
    [
        lens(&[2, 3, 5]),       // a: as, any, await/async
        lens(&[5, 6, 7]),       // b: break, bigint, boolean
        lens(&[4, 5, 8]),       // c: case, const/class/catch, continue
        lens(&[2, 6, 7, 8]),    // d: do, delete, default, debugger
        lens(&[4, 6, 7]),       // e: else/enum, export, extends
        lens(&[3, 4, 5, 7, 8]), // f: for, from, false, finally, function
        0,                      // g
        0,                      // h
        lens(&[2, 6, 10]),      // i: in/if, import, instanceof
        0,                      // j
        0,                      // k
        lens(&[3]),             // l: let
        0,                      // m
        lens(&[3, 4, 5, 6]),    // n: new, null, never, number
        lens(&[6]),             // o: object
        0,                      // p
        0,                      // q
        lens(&[6]),             // r: return
        lens(&[5, 6, 9]),       // s: super, string/symbol/switch, satisfies
        lens(&[3, 4, 5, 6]),    // t: try, true/this, throw, typeof
        lens(&[7, 9]),          // u: unknown, undefined
        lens(&[3, 4]),          // v: var, void
        lens(&[4, 5]),          // w: with, while
        0,                      // x
        lens(&[5]),             // y: yield
        0,                      // z
    ]
};

/// Encode up to 8 ASCII bytes of `s` as a little-endian `u64` — the SWAR key for a
/// keyword of length ≤ 8. Used only inside `const { … }` so each keyword constant
/// is materialized at compile time, never re-run at the call site.
const fn keyword_encode(s: &str) -> u64 {
    let b = s.as_bytes();
    let mut w = 0u64;
    let mut i = 0;
    while i < b.len() {
        w |= (b[i] as u64) << (i * 8);
        i += 1;
    }
    w
}

/// Encode up to 16 ASCII bytes of `s` as a little-endian `u128` — the SWAR key for a
/// length-9/10 keyword. The `u64` [`keyword_encode`] can't hold 9+ bytes; this is its
/// wide sibling, used only inside `const { … }` in [`keyword_swar_long`].
const fn keyword_encode_wide(s: &str) -> u128 {
    let b = s.as_bytes();
    let mut w = 0u128;
    let mut i = 0;
    while i < b.len() {
        w |= (b[i] as u128) << (i * 8);
        i += 1;
    }
    w
}

/// SWAR keyword recognition for identifiers of length **2..=8**: the caller packs
/// the identifier's bytes into a little-endian `u64` (`word`, masked to `len`
/// bytes — see `read_keyword_word`) and this matches it against the keyword
/// constants of that length. Returns `None` for non-keywords and for `len` outside
/// 2..=8 — the caller routes the three length-9/10 keywords
/// (`undefined`/`satisfies`/`instanceof`) to [`keyword_swar_long`].
///
/// Recognizes the same length-≤8 reserved words as the `KEYWORDS` oracle, proven in
/// `swar_matches_keyword_table`. Dispatching on `len` first keeps each per-length
/// compare set tiny, and the `const { … }` encodings are compile-time constants so
/// this is pure integer comparison.
#[inline]
// `allow`, not `expect`: the lint fires without it, but the expectation never registers as
// fulfilled — neither on the fn nor on the `use` item itself — so `expect` reads as dead.
#[allow(clippy::enum_glob_use)] // 49 arms — the glob keeps the per-length tables readable
fn keyword_swar(word: u64, len: usize) -> Option<KeywordKind> {
    use KeywordKind::*;
    match len {
        2 => {
            if word == const { keyword_encode("in") } {
                Some(In)
            } else if word == const { keyword_encode("if") } {
                Some(If)
            } else if word == const { keyword_encode("do") } {
                Some(Do)
            } else if word == const { keyword_encode("as") } {
                Some(As)
            } else {
                None
            }
        }
        3 => {
            if word == const { keyword_encode("let") } {
                Some(Let)
            } else if word == const { keyword_encode("var") } {
                Some(Var)
            } else if word == const { keyword_encode("any") } {
                Some(Any)
            } else if word == const { keyword_encode("new") } {
                Some(New)
            } else if word == const { keyword_encode("for") } {
                Some(For)
            } else if word == const { keyword_encode("try") } {
                Some(Try)
            } else {
                None
            }
        }
        4 => {
            if word == const { keyword_encode("true") } {
                Some(True)
            } else if word == const { keyword_encode("null") } {
                Some(Null)
            } else if word == const { keyword_encode("void") } {
                Some(Void)
            } else if word == const { keyword_encode("this") } {
                Some(This)
            } else if word == const { keyword_encode("from") } {
                Some(From)
            } else if word == const { keyword_encode("enum") } {
                Some(Enum)
            } else if word == const { keyword_encode("case") } {
                Some(Case)
            } else if word == const { keyword_encode("else") } {
                Some(Else)
            } else if word == const { keyword_encode("with") } {
                Some(With)
            } else {
                None
            }
        }
        5 => {
            if word == const { keyword_encode("const") } {
                Some(Const)
            } else if word == const { keyword_encode("false") } {
                Some(False)
            } else if word == const { keyword_encode("never") } {
                Some(Never)
            } else if word == const { keyword_encode("super") } {
                Some(Super)
            } else if word == const { keyword_encode("yield") } {
                Some(Yield)
            } else if word == const { keyword_encode("while") } {
                Some(While)
            } else if word == const { keyword_encode("break") } {
                Some(Break)
            } else if word == const { keyword_encode("throw") } {
                Some(Throw)
            } else if word == const { keyword_encode("class") } {
                Some(Class)
            } else if word == const { keyword_encode("async") } {
                Some(Async)
            } else if word == const { keyword_encode("await") } {
                Some(Await)
            } else if word == const { keyword_encode("catch") } {
                Some(Catch)
            } else {
                None
            }
        }
        6 => {
            if word == const { keyword_encode("number") } {
                Some(Number)
            } else if word == const { keyword_encode("string") } {
                Some(String)
            } else if word == const { keyword_encode("object") } {
                Some(Object)
            } else if word == const { keyword_encode("symbol") } {
                Some(Symbol)
            } else if word == const { keyword_encode("bigint") } {
                Some(Bigint)
            } else if word == const { keyword_encode("return") } {
                Some(Return)
            } else if word == const { keyword_encode("switch") } {
                Some(Switch)
            } else if word == const { keyword_encode("typeof") } {
                Some(Typeof)
            } else if word == const { keyword_encode("delete") } {
                Some(Delete)
            } else if word == const { keyword_encode("export") } {
                Some(Export)
            } else if word == const { keyword_encode("import") } {
                Some(Import)
            } else {
                None
            }
        }
        7 => {
            if word == const { keyword_encode("boolean") } {
                Some(Boolean)
            } else if word == const { keyword_encode("unknown") } {
                Some(Unknown)
            } else if word == const { keyword_encode("default") } {
                Some(Default)
            } else if word == const { keyword_encode("finally") } {
                Some(Finally)
            } else if word == const { keyword_encode("extends") } {
                Some(Extends)
            } else {
                None
            }
        }
        8 => {
            if word == const { keyword_encode("continue") } {
                Some(Continue)
            } else if word == const { keyword_encode("function") } {
                Some(Function)
            } else if word == const { keyword_encode("debugger") } {
                Some(Debugger)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// SWAR recognition for the three length-9/10 keywords (`undefined`/`satisfies`,
/// length 9; `instanceof`, length 10). The caller packs the identifier's `len` bytes
/// little-endian into a `u128` (`read_keyword_word_wide`) and this matches it against
/// the keyword constants of that length. A `u128` key (vs the `u64` the `len <= 8`
/// path uses) holds the extra bytes while keeping the hot ≤8 path's compares narrow.
/// Returns `None` for non-keywords and for `len` outside 9..=10; proven against the
/// `KEYWORDS` oracle in `swar_matches_keyword_table`.
#[inline]
fn keyword_swar_long(word: u128, len: usize) -> Option<KeywordKind> {
    use KeywordKind::{Instanceof, Satisfies, Undefined};
    match len {
        9 => {
            if word == const { keyword_encode_wide("undefined") } {
                Some(Undefined)
            } else if word == const { keyword_encode_wide("satisfies") } {
                Some(Satisfies)
            } else {
                None
            }
        }
        10 => {
            if word == const { keyword_encode_wide("instanceof") } {
                Some(Instanceof)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Pack `bytes[start..start+len]` (an identifier, `len` ∈ 2..=8) into a
/// little-endian `u64` keyword key. Fast path: a single 8-byte load when 8 bytes
/// are in bounds (the common case — an identifier is rarely in the file's last 8
/// bytes), masked to `len` bytes. Near EOF, assemble from the `len` identifier
/// bytes (always in bounds: the identifier occupies `[start, start+len)`).
#[inline]
fn read_keyword_word(bytes: &[u8], start: usize, len: usize) -> u64 {
    if start + 8 <= bytes.len() {
        // Eight in-bounds bytes packed little-endian; lowers to one `movq`.
        let word = u64::from_le_bytes([
            bytes[start],
            bytes[start + 1],
            bytes[start + 2],
            bytes[start + 3],
            bytes[start + 4],
            bytes[start + 5],
            bytes[start + 6],
            bytes[start + 7],
        ]);
        if len == 8 {
            word
        } else {
            word & ((1u64 << (len * 8)) - 1)
        }
    } else {
        let mut w = 0u64;
        let mut i = 0;
        while i < len {
            w |= (bytes[start + i] as u64) << (i * 8);
            i += 1;
        }
        w
    }
}

/// Pack `bytes[start..start+len]` (an identifier, `len` ∈ 9..=10) into a little-endian
/// `u128` keyword key — the wide counterpart of [`read_keyword_word`] for the
/// length-9/10 path. A plain byte loop, no 8-byte fast load.
///
/// ⚠️ **This loop is not free, and the sentence that used to stand here — "only the
/// three long keywords reach here, so this path is cold" — is why nobody noticed.**
/// That is true of how often it MATCHES and false of how often it RUNS: only
/// `undefined`/`satisfies`/`instanceof` match, but every 9- or 10-byte identifier the
/// pre-filter admits is packed here first, and the loop lowers to a bounds-checked
/// `shld`/`shl`/`cmov` pair per byte (~14 instructions × 9). Before
/// [`KEYWORD_LENGTHS_BY_FIRST_LETTER`] took the length into the pre-filter it ran
/// **32,668 times per pass** on a TypeScript corpus against **2,757** matches, and its
/// loop body was the hottest line in this file (0.207% / 0.234% of the format and
/// wire boards). The length key now rejects **82.6%** of those calls, which is what
/// makes the remaining ones cheap enough to leave on a byte loop — ⛔ rewriting it
/// off a `u64` head was measured **on top of** that pre-filter and came out *worse*
/// on every channel, the residue being smaller than the rewrite's own tail branches.
///
/// ⭐ The general form: a "this path is cold" comment names a RATE, and the rate it
/// names is usually the success rate while the cost follows the call rate.
#[inline]
fn read_keyword_word_wide(bytes: &[u8], start: usize, len: usize) -> u128 {
    let mut w = 0u128;
    let mut i = 0;
    while i < len {
        w |= (bytes[start + i] as u128) << (i * 8);
        i += 1;
    }
    w
}

/// Reserved-word lookup for the identifier `bytes[start..start+len]`
/// (`len = end - start`). The lexer's single keyword entry point: it applies a cheap
/// pre-filter (length 2..=10, then the reserved-word LENGTHS for that first letter —
/// see [`KEYWORD_LENGTHS_BY_FIRST_LETTER`] — rejecting PascalCase / `_`/`$`-led /
/// non-keyword-letter names *and* every name whose length no keyword of that letter
/// has, all before a single compare arm runs), then recognizes the
/// keyword entirely via SWAR — the 49 keywords of length ≤ 8 through [`keyword_swar`]
/// (`u64` key) and the three length-9/10 keywords
/// (`undefined`/`satisfies`/`instanceof`) through [`keyword_swar_long`] (`u128` key).
/// No hashing on any path.
///
/// `bytes` is the lexer source and `[start, start+len)` a validated identifier, so
/// it is in bounds (a non-ASCII identifier simply matches no ASCII keyword constant
/// and falls through to `None`).
#[inline]
pub fn keyword_at(bytes: &[u8], start: usize, len: usize) -> Option<KeywordKind> {
    if !matches!(len, KEYWORD_MIN_LEN..=KEYWORD_MAX_LEN) {
        return None;
    }
    let idx = bytes[start].wrapping_sub(b'a');
    if idx >= 26 || (KEYWORD_LENGTHS_BY_FIRST_LETTER[idx as usize] >> len) & 1 == 0 {
        return None;
    }
    if len <= 8 {
        keyword_swar(read_keyword_word(bytes, start, len), len)
    } else {
        // The 2 (len 9) + 1 (len 10) keywords: SWAR over a u128.
        keyword_swar_long(read_keyword_word_wide(bytes, start, len), len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `keyword_at` (pre-filter + SWAR) must recognize every reserved word — otherwise
    /// the lexer would misclassify a keyword as an identifier. Also re-derives the length
    /// bounds and the first-letter mask from `KEYWORDS`, so adding or removing a keyword
    /// that shifts either invariant fails here instead of silently corrupting tokenization.
    #[test]
    fn prefilter_admits_every_keyword() {
        let mut derived = [0u16; 26];
        for &(kw, kind) in KEYWORDS {
            assert_eq!(
                keyword_at(kw.as_bytes(), 0, kw.len()),
                Some(kind),
                "keyword_at failed to recognize reserved word `{kw}`"
            );
            let len = kw.len();
            assert!(
                (KEYWORD_MIN_LEN..=KEYWORD_MAX_LEN).contains(&len),
                "reserved word `{kw}` (len {len}) is outside the pre-filter bound \
                 {KEYWORD_MIN_LEN}..={KEYWORD_MAX_LEN}"
            );
            let first = kw.as_bytes()[0];
            assert!(
                first.is_ascii_lowercase(),
                "reserved word `{kw}` does not start lowercase-ASCII"
            );
            derived[(first - b'a') as usize] |= 1u16 << len;
        }
        assert_eq!(
            derived, KEYWORD_LENGTHS_BY_FIRST_LETTER,
            "KEYWORD_LENGTHS_BY_FIRST_LETTER is out of sync with the keyword set"
        );
    }

    /// Non-keywords the gate should reject (most without hashing): PascalCase, sigil-led
    /// and single-char names, and contextual words deliberately absent from `KEYWORDS`.
    /// The pre-filter is a **membership test that decides whether a compare chain
    /// runs at all**, so its class is a failure surface of its own: narrowing it
    /// wrongly turns a reserved word into a plain identifier, and no corpus of
    /// formatted code is obliged to contain the shape that separates the class from
    /// its complement. Grade it against the `KEYWORDS` oracle over generated
    /// near-misses rather than against a spot list — every one-byte substitution at
    /// every position of every reserved word, every proper prefix, and every
    /// one-character extension, over the ASCII identifier alphabet.
    #[test]
    fn keyword_at_matches_the_oracle_on_every_near_miss() {
        fn oracle(s: &str) -> Option<KeywordKind> {
            KEYWORDS
                .iter()
                .find(|(kw, _)| *kw == s)
                .map(|&(_, kind)| kind)
        }
        let alphabet: Vec<u8> = (b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .chain(b'0'..=b'9')
            .chain([b'_', b'$'])
            .collect();
        let mut cases = 0_u32;
        let check = |s: &str| {
            assert_eq!(
                keyword_at(s.as_bytes(), 0, s.len()),
                oracle(s),
                "keyword_at disagrees with the KEYWORDS oracle on `{s}`"
            );
        };
        for &(kw, _) in KEYWORDS {
            check(kw);
            for position in 0..kw.len() {
                for &byte in &alphabet {
                    let mut bytes = kw.as_bytes().to_vec();
                    bytes[position] = byte;
                    check(std::str::from_utf8(&bytes).unwrap());
                    cases += 1;
                }
            }
            for cut in 1..kw.len() {
                check(&kw[..cut]);
            }
            for &byte in &alphabet {
                let mut extended = kw.to_string();
                extended.push(byte as char);
                check(&extended);
            }
        }
        assert!(
            cases > 15_000,
            "the near-miss sweep collapsed to {cases} cases"
        );
    }

    #[test]
    fn prefilter_rejects_non_keywords() {
        for s in [
            "Foo",
            "_private",
            "$x",
            "x",
            "",
            "interface",
            "readonly",
            "namespace",
            "get",
            "kind",
            "map",
        ] {
            assert_eq!(
                keyword_at(s.as_bytes(), 0, s.len()),
                None,
                "`{s}` should not be a keyword"
            );
        }
    }

    /// The SWAR arms must agree with the `KEYWORDS` oracle for every reserved word —
    /// this proves the hand-written per-length compare sets have no typo or omission.
    /// Each word is recognized by exactly one arm: `keyword_swar` (`u64`) for length
    /// ≤ 8, `keyword_swar_long` (`u128`) for length 9/10. The other arm's `len` gate
    /// must reject it, so a keyword can never be matched by the wrong-width path.
    #[test]
    fn swar_matches_keyword_table() {
        for &(kw, kind) in KEYWORDS {
            if kw.len() <= 8 {
                assert_eq!(
                    keyword_swar(keyword_encode(kw), kw.len()),
                    Some(kind),
                    "keyword_swar misclassified reserved word `{kw}`"
                );
                // Out of the long path's 9..=10 scope: its `len` gate must reject it.
                assert_eq!(
                    keyword_swar_long(keyword_encode_wide(kw), kw.len()),
                    None,
                    "keyword_swar_long should reject length-{} word `{kw}`",
                    kw.len()
                );
            } else {
                assert_eq!(
                    keyword_swar_long(keyword_encode_wide(kw), kw.len()),
                    Some(kind),
                    "keyword_swar_long misclassified reserved word `{kw}`"
                );
                // Out of the ≤8 path's scope: `keyword_swar`'s `len` gate must reject it.
                assert_eq!(
                    keyword_swar(0, kw.len()),
                    None,
                    "keyword_swar should reject length-{} word `{kw}`",
                    kw.len()
                );
            }
        }
    }

    /// The production keyword encoders (`read_keyword_word`, a single 8-byte load + mask
    /// or a byte-assembly near EOF, for length ≤ 8; `read_keyword_word_wide` for length
    /// 9/10) must produce the same little-endian word as the compile-time
    /// `keyword_encode`/`keyword_encode_wide` the SWAR constants are built from.
    /// `swar_matches_keyword_table` feeds the compile-time encoders, so without this a
    /// divergence in the runtime readers — the byte order the lexer actually runs —
    /// would pass the unit suite and only surface in the integration gates. Covers the
    /// in-bounds fast path (padded source), the near-EOF assembly path (the keyword as
    /// the final bytes), and the wide length-9/10 reader.
    #[test]
    fn read_keyword_word_matches_keyword_encode() {
        for &(kw, _) in KEYWORDS {
            if kw.len() > 8 {
                // Wide path: read_keyword_word_wide must match the const keyword_encode_wide.
                assert_eq!(
                    read_keyword_word_wide(kw.as_bytes(), 0, kw.len()),
                    keyword_encode_wide(kw),
                    "read_keyword_word_wide disagrees with keyword_encode_wide for `{kw}`"
                );
                continue;
            }
            // Fast path: ≥ 8 bytes in bounds (trailing pad guarantees start + 8 <= len).
            let mut padded = kw.as_bytes().to_vec();
            padded.extend_from_slice(b"________");
            assert_eq!(
                read_keyword_word(&padded, 0, kw.len()),
                keyword_encode(kw),
                "fast-path read_keyword_word disagrees with keyword_encode for `{kw}`"
            );
            // Near-EOF path: the keyword is the trailing bytes (start + 8 > len for
            // len < 8; the three len-8 keywords still exercise the fast branch here).
            assert_eq!(
                read_keyword_word(kw.as_bytes(), 0, kw.len()),
                keyword_encode(kw),
                "EOF-path read_keyword_word disagrees with keyword_encode for `{kw}`"
            );
        }
    }

    /// SWAR must reject non-keywords (including ones that share a keyword's length
    /// and first letter) so it never promotes an identifier to a keyword.
    #[test]
    fn swar_rejects_non_keywords() {
        for s in [
            "value", "index", "props", "Foo", "fromm", "iff", "clas", "functio",
        ] {
            assert_eq!(
                keyword_swar(keyword_encode(s), s.len()),
                None,
                "`{s}` should not be a SWAR keyword"
            );
        }
    }
}
