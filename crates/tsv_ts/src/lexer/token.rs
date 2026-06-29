// Token types for TypeScript/JS lexer

use phf::phf_map;
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
        }
    }

    /// Returns true if this is a declaration keyword (const, let, var, function)
    #[inline]
    pub const fn is_declaration_keyword(self) -> bool {
        matches!(
            self,
            KeywordKind::Const | KeywordKind::Let | KeywordKind::Var
        )
    }

    /// Returns true if this is a literal keyword (true, false, null, undefined)
    #[inline]
    pub const fn is_literal_keyword(self) -> bool {
        matches!(
            self,
            KeywordKind::True | KeywordKind::False | KeywordKind::Null | KeywordKind::Undefined
        )
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
                | KeywordKind::Void
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
    /// This is more restrictive than `can_be_identifier()`. Some keywords like `await`,
    /// `yield`, and `let` can be property names but NOT binding names.
    ///
    /// - `await` - cannot be a binding name (reserved in module code)
    /// - `yield` - cannot be a binding name (reserved in strict mode)
    /// - `let` - cannot be a binding name (reserved in strict mode)
    ///
    /// Examples:
    /// - `const as = 1;` - valid, `as` can be a binding name
    /// - `const await = 1;` - INVALID, `await` cannot be a binding name
    /// - `function fn(yield: string) {}` - INVALID, `yield` cannot be a parameter
    #[inline]
    pub const fn can_be_binding_name(self) -> bool {
        matches!(
            self,
            // Fully contextual keywords that can be binding names
            KeywordKind::Async
                | KeywordKind::From
                | KeywordKind::As
                | KeywordKind::Satisfies
                // Type keywords are also valid binding names in value positions
                | KeywordKind::Number
                | KeywordKind::String
                | KeywordKind::Boolean
                | KeywordKind::Any
                | KeywordKind::Void
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
        // NOTE: Await, Yield, Let are NOT included - they cannot be binding names
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
// (`Lexer::decoded` / `take_decoded`), so the per-token value carried on the hot
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

/// Perfect hash map for O(1) keyword lookup
static KEYWORDS: phf::Map<&'static str, KeywordKind> = phf_map! {
    // Declaration keywords
    "const" => KeywordKind::Const,
    "let" => KeywordKind::Let,
    "var" => KeywordKind::Var,
    // Literal keywords
    "true" => KeywordKind::True,
    "false" => KeywordKind::False,
    "null" => KeywordKind::Null,
    "undefined" => KeywordKind::Undefined,
    // Type keywords
    "number" => KeywordKind::Number,
    "string" => KeywordKind::String,
    "boolean" => KeywordKind::Boolean,
    "any" => KeywordKind::Any,
    "void" => KeywordKind::Void,
    "never" => KeywordKind::Never,
    "unknown" => KeywordKind::Unknown,
    "object" => KeywordKind::Object,
    "symbol" => KeywordKind::Symbol,
    "bigint" => KeywordKind::Bigint,
    // Expression keywords
    "new" => KeywordKind::New,
    // Binary operator keywords
    "instanceof" => KeywordKind::Instanceof,
    "in" => KeywordKind::In,
    // Control flow keywords
    "return" => KeywordKind::Return,
    "if" => KeywordKind::If,
    "else" => KeywordKind::Else,
    "for" => KeywordKind::For,
    "while" => KeywordKind::While,
    "do" => KeywordKind::Do,
    "switch" => KeywordKind::Switch,
    "case" => KeywordKind::Case,
    "default" => KeywordKind::Default,
    "break" => KeywordKind::Break,
    "continue" => KeywordKind::Continue,
    "try" => KeywordKind::Try,
    "catch" => KeywordKind::Catch,
    "finally" => KeywordKind::Finally,
    "throw" => KeywordKind::Throw,
    // Declaration keywords (continued)
    "function" => KeywordKind::Function,
    "class" => KeywordKind::Class,
    "enum" => KeywordKind::Enum,
    // Unary keyword operators
    "typeof" => KeywordKind::Typeof,
    "delete" => KeywordKind::Delete,
    // Async/await keywords
    "async" => KeywordKind::Async,
    "await" => KeywordKind::Await,
    // Class keywords
    "this" => KeywordKind::This,
    "super" => KeywordKind::Super,
    "extends" => KeywordKind::Extends,
    // Module keywords
    "export" => KeywordKind::Export,
    "import" => KeywordKind::Import,
    "from" => KeywordKind::From,
    "as" => KeywordKind::As,
    "satisfies" => KeywordKind::Satisfies,
    // Generator keywords
    "yield" => KeywordKind::Yield,
    // Debugger
    "debugger" => KeywordKind::Debugger,
    // TODO: Expand keyword list for:
    // - Type keywords: interface, type, namespace, etc.
};

/// Shortest reserved word (`as`/`in`/`if`/`do`) is 2 bytes; longest (`instanceof`) is 10.
const KEYWORD_MIN_LEN: usize = 2;
const KEYWORD_MAX_LEN: usize = 10;

/// Bit `b - b'a'` is set when some reserved word begins with lowercase-ASCII letter `b`.
/// Kept exactly in sync with `KEYWORDS` by `prefilter_admits_every_keyword`.
const KEYWORD_FIRST_LETTER_MASK: u32 = {
    const fn bit(b: u8) -> u32 {
        1 << (b - b'a')
    }
    bit(b'a')
        | bit(b'b')
        | bit(b'c')
        | bit(b'd')
        | bit(b'e')
        | bit(b'f')
        | bit(b'i')
        | bit(b'l')
        | bit(b'n')
        | bit(b'o')
        | bit(b'r')
        | bit(b's')
        | bit(b't')
        | bit(b'u')
        | bit(b'v')
        | bit(b'w')
        | bit(b'y')
};

/// O(1) keyword lookup using a perfect hash function, gated by a cheap reject pre-filter.
///
/// Every reserved word is 2–10 bytes long and begins with one of a fixed set of
/// lowercase-ASCII letters, so an identifier failing either test cannot be a keyword.
/// The gate skips the phf hash for PascalCase types, `_`/`$`-prefixed names, and any
/// identifier whose first letter starts no keyword — the overwhelming majority of
/// identifier tokens. `prefilter_admits_every_keyword` proves it never rejects a real
/// keyword, so the gate is purely a fast path and changes no behavior.
#[inline]
fn keyword_kind(s: &str) -> Option<KeywordKind> {
    let bytes = s.as_bytes();
    if !matches!(bytes.len(), KEYWORD_MIN_LEN..=KEYWORD_MAX_LEN) {
        return None;
    }
    let idx = bytes[0].wrapping_sub(b'a');
    if idx >= 26 || (KEYWORD_FIRST_LETTER_MASK >> idx) & 1 == 0 {
        return None;
    }
    KEYWORDS.get(s).copied()
}

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

/// SWAR keyword recognition for identifiers of length **2..=8**: the caller packs
/// the identifier's bytes into a little-endian `u64` (`word`, masked to `len`
/// bytes — see `read_keyword_word`) and this matches it against the keyword
/// constants of that length, retiring the `phf::get_entry` hash on the keyword
/// path. Returns `None` for non-keywords and for `len` outside 2..=8 — the caller
/// routes the three length-9/10 keywords (`undefined`/`satisfies`/`instanceof`)
/// to the `phf` `keyword_kind`.
///
/// Byte-for-byte equivalent to `keyword_kind` for `len <= 8`; proven over the
/// whole `KEYWORDS` set in `swar_matches_phf`. Dispatching on `len` first keeps
/// each per-length compare set tiny, and the `const { … }` encodings are compile-time
/// constants so this is pure integer comparison.
#[inline]
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

/// Reserved-word lookup for the identifier `bytes[start..start+len]`
/// (`len = end - start`). The lexer's single keyword entry point: it applies the
/// same cheap pre-filter as [`keyword_kind`] (length 2..=10 + keyword first-letter,
/// rejecting PascalCase / `_`/`$`-led / non-keyword-letter names without further
/// work), then recognizes the 49 keywords of length ≤ 8 via SWAR ([`keyword_swar`],
/// retiring the `phf` hash) and defers the three length-9/10 keywords
/// (`undefined`/`satisfies`/`instanceof`) to the `phf` [`keyword_kind`].
///
/// `bytes` is the lexer source and `[start, start+len)` a validated identifier, so
/// it is in bounds and valid UTF-8 (a non-ASCII identifier simply matches no
/// ASCII keyword constant and falls through to `None`).
#[inline]
pub fn keyword_at(bytes: &[u8], start: usize, len: usize) -> Option<KeywordKind> {
    if !matches!(len, KEYWORD_MIN_LEN..=KEYWORD_MAX_LEN) {
        return None;
    }
    let idx = bytes[start].wrapping_sub(b'a');
    if idx >= 26 || (KEYWORD_FIRST_LETTER_MASK >> idx) & 1 == 0 {
        return None;
    }
    if len <= 8 {
        keyword_swar(read_keyword_word(bytes, start, len), len)
    } else {
        // The 2 (len 9) + 1 (len 10) keywords: phf over the validated slice.
        std::str::from_utf8(&bytes[start..start + len])
            .ok()
            .and_then(keyword_kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gate must admit every reserved word — otherwise the lexer would misclassify
    /// a keyword as an identifier. Also re-derives the length bounds and the first-letter
    /// mask from `KEYWORDS`, so adding or removing a keyword that shifts either invariant
    /// fails here instead of silently corrupting tokenization.
    #[test]
    fn prefilter_admits_every_keyword() {
        let mut derived_mask = 0u32;
        for (&kw, &kind) in KEYWORDS.entries() {
            assert_eq!(
                keyword_kind(kw),
                Some(kind),
                "pre-filter rejected reserved word `{kw}`"
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
            derived_mask |= 1u32 << (first - b'a');
        }
        assert_eq!(
            derived_mask, KEYWORD_FIRST_LETTER_MASK,
            "KEYWORD_FIRST_LETTER_MASK is out of sync with the keyword set"
        );
    }

    /// Non-keywords the gate should reject (most without hashing): PascalCase, sigil-led
    /// and single-char names, and contextual words deliberately absent from `KEYWORDS`.
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
            assert_eq!(keyword_kind(s), None, "`{s}` should not be a keyword");
        }
    }

    /// `keyword_swar` must agree with the `phf` `keyword_kind` for every reserved
    /// word of length ≤ 8 (the SWAR-eligible set) — this proves the hand-written
    /// per-length compare set has no typo or omission. Lengths 9/10 are out of
    /// SWAR scope (the caller routes them to `keyword_kind`), so they must return
    /// `None` from `keyword_swar`.
    #[test]
    fn swar_matches_phf() {
        for (&kw, &kind) in KEYWORDS.entries() {
            if kw.len() <= 8 {
                assert_eq!(
                    keyword_swar(keyword_encode(kw), kw.len()),
                    Some(kind),
                    "SWAR misclassified reserved word `{kw}`"
                );
            } else {
                // 9/10-byte keywords are out of SWAR scope (keyword_encode is only valid
                // for ≤8 bytes); the `len` gate alone must reject them regardless of
                // the word, so `keyword_at` routes them to the phf `keyword_kind`.
                assert_eq!(
                    keyword_swar(0, kw.len()),
                    None,
                    "SWAR should defer length-{} word `{kw}` to phf",
                    kw.len()
                );
            }
        }
    }

    /// The production keyword encoder (`read_keyword_word`, a single 8-byte load + mask,
    /// or a byte-assembly near EOF) must produce the same little-endian `u64` as the
    /// compile-time `keyword_encode` the SWAR constants are built from. `swar_matches_phf`
    /// feeds `keyword_encode`, so without this a divergence in `read_keyword_word` — the byte
    /// order the lexer actually runs — would pass the unit suite and only surface in
    /// the integration gates. Covers both the in-bounds fast path (padded source) and
    /// the near-EOF assembly path (the keyword as the final bytes).
    #[test]
    fn read_keyword_word_matches_keyword_encode() {
        for (&kw, _) in KEYWORDS.entries() {
            if kw.len() > 8 {
                continue; // out of SWAR scope; routed to phf, never read_keyword_word
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
