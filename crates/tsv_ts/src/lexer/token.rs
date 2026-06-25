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
        content_start: usize,
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
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
    /// Decoded value (for strings with escape sequences)
    /// None for non-string tokens or strings without escapes
    pub decoded: Option<String>,
}

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

/// O(1) keyword lookup using perfect hash function
#[inline]
pub fn keyword_kind(s: &str) -> Option<KeywordKind> {
    KEYWORDS.get(s).copied()
}
