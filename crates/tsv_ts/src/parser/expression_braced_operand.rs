//! The braced-operand resolver: whether a `}` ends an operand, read as acorn reads it —
//! `tsv_lang`'s `ClosesBracedOperand` for a Svelte island's extent
//! (`crate::ACORN_ISLAND_GRAMMAR`).

use super::scan::{identifier_starts_at, skip_identifier, skip_numeric_literal, skip_whitespace};
use tsv_lang::source_scan::{
    BracedOperandWalk, OperandAnchor, OperandGrammar, ScanStart, TriviaProfile, skip_regex_literal,
    skip_trivia,
};

/// Whether the `}` at `rbrace` closes a braced operand — an object literal, or a function or
/// class expression's body that acorn reads as an expression's — so a `/` after it divides.
/// `tsv_lang`'s `ClosesBracedOperand`, for the scan that finds where a Svelte island ends.
///
/// The reading is acorn's, because acorn parses the island and so decides where it ends.
/// Its tokenizer settles a `/` by a stack of token contexts (`tokencontext.js`), and after a
/// `}` by the context that brace pushed; its parser corrects the tokenizer in places. This
/// walks `lower_bound..=rbrace` token by token keeping the part of that state an EXPRESSION
/// reaches ([`TokenContexts`]):
///
/// - A `{` opens an object literal or a block by the tokenizer's `braceIsBlock`, which reads
///   the token before it: a block after a `)` or a `=>` (a body), and after a few tokens
///   where an expression has an object literal — at the region's start, after a
///   conditional's `:`, after a class heritage's `extends`, after a `for` header's `;` or
///   `of⏎`. The parser, which knows it is reading an object literal, overrides the
///   tokenizer at each, and the walk models the first three. An object literal's `}` ends
///   an operand. A TypeScript type literal's `{` gets no override, so the tokenizer's
///   reading stands for it, block or not.
/// - A `function` or `class` keyword opens a function context, an expression's or a
///   statement's by the token before it, and its body's `}` ends an operand when the
///   context is an expression's. That is the tokenizer's heuristic, not the grammar: it
///   takes the keyword for a statement's at the first token of an island, after a name
///   (`yield`, `await`, a decorator's) and after a `:` at the island's top level (a `[`
///   pushes no context, so an array's elements read as the top level does) — and then reads
///   the `/` after the body as a regex, where the grammar has a division
///   (`{c ? x : function () {} / 2}` does not parse in Svelte). The walk reproduces it. The
///   parser's one correction here is for an `async function` it reads as an expression,
///   whose context it rewrites after the token that follows the keyword — unless that token
///   is the `(`, whose own context takes the rewrite instead. So a named or generator one
///   divides (`{async function f() {} / 2}`) and an anonymous one does not.
///
/// The parser's last correction is the one a tokenizer walk cannot model: where the grammar
/// is at the start of a STATEMENT, acorn re-reads a `/` the tokenizer called a division as
/// a regex, so a brace its tokenizer took for an expression's is still followed by one
/// (`a⏎{}⏎/re/.test(s)`, a block after a statement that ended without its `;`; `enum E {…}`
/// and a function declaration with a return type, in TypeScript). Statements begin only
/// inside a block, so a `{`, `function` or `class` directly inside one — a function, arrow
/// or class body, or any brace nested in one with no `(…)` or `${…}` between — answers
/// `false`: a block's answer, and the reading the bytes alone give
/// ([`Frame::statement_level`]). An expression inside such a block therefore still reads a
/// regex after its object literal (`{() => { x = {} / 2 }}` is rejected on one line where
/// Svelte accepts it); a `(…)` or `${…}` there holds expressions only and is answered in
/// full.
///
/// A postfix `!` glued to the `}` (TypeScript's non-null) leaves the question the brace's:
/// the scan steps over the run to the `}` ([`OperandAnchor`]) and asks about it. Before
/// such a run a function or class body ends an operand whatever acorn's tokenizer took its
/// keyword for — acorn-typescript reads the `!` as ending one — so `{function () {}! / 2}`
/// divides.
///
/// Every other `/` the walk meets is read by the scanners' own rule ([`OperandAnchor`], with
/// this crate's type-argument and statement-header answers), and one after an earlier `}` by
/// that brace's context, as it was answered for. So the walk's tokens are the ones the scan
/// asking it read, a template literal's interpolations included: it tokenizes them in line,
/// as regions of their own ([`ScanStart::Interpolation`]), rather than asking about each. A
/// walk that does not reach `rbrace` as a token of its own — the `}` inside a literal it
/// read differently — or a `}` whose `{` lies before `lower_bound` answers `false`.
///
/// A scan asks in source order, and `resume` is where the walk it began is kept between
/// its asks ([`Walk`]): each picks up where the last stopped, so a region is walked once
/// however many `}` + `/` pairs it holds.
///
/// Asked only for a `/` after a `}`, which real code almost never writes, so the walk's cost
/// is paid there alone.
// TODO: the walk reads each `>` + `/` and each `)!` + `/` it passes through
// `closes_type_arguments` / `closes_statement_header`, each a walk from the region's start,
// so a region that interleaves thousands of `f<T> /` or `(a)! /` pairs with a `} /` pays
// that quadratically — those resolvers' own worst case, met a second time. A template
// literal's interpolation is a region of its own to the scan and is tokenized again in
// line by each region around it, so nesting costs its depth. A parse entry that reads an
// expression from the island's start and stops where it ends would retire this walk, the
// type-argument one and the statement-header one together.
// TODO: tracked differences from Svelte.
// - Rejected where Svelte accepts, when the misread regex closes on its line: a braced
//   operand at statement level inside a nested body (`{() => { x = {} / 2 }}`,
//   `{(function* () { yield {} / 2; })}`, a class field's initializer); an object literal
//   the parser overrides at a position the walk does not model — a `for` header's `;`
//   (`for (;{} / 2;) {}`) or `of⏎{`, a heritage's past its first token
//   (`class extends a[{}] {}`); a `}` before a postfix `!` written with a space
//   (`{} ! / 2`), which the scan reads as a prefix operator; a function whose return type
//   is a type predicate over a type literal (`x + function (): a is {} {} / 2`,
//   `asserts a is {}`), which acorn-typescript divides after — and with them a line that
//   holds any such misread `/` beside a braced operand the walk divides after, whose two
//   misread regexes no longer pair up (`` `${{} / x}${c ! / f}` ``).
// - Accepted where Svelte rejects: a function or class body the tokenizer reads as a
//   statement's, when the regex it reads after the body ends inside the island or never
//   closes on its line (`{function () {} / 2 / 3}`) — the scan only settles where the island
//   ends, and the parse that follows reads the grammar's division; and acorn-typescript's
//   own misread of an empty object type (`x as {} / 2`, `function (a: {}) {} / 2`), which
//   this walk does not model.
pub(crate) fn closes_braced_operand(
    bytes: &[u8],
    rbrace: usize,
    lower_bound: usize,
    start: ScanStart,
    resume: &mut BracedOperandWalk,
) -> bool {
    let mut walk = resume
        .take::<Walk>()
        .filter(|walk| walk.resumes(rbrace, lower_bound, start))
        .unwrap_or_else(|| Box::new(Walk::new(lower_bound, start)));
    let closed = walk.reach(bytes, rbrace);
    if closed.is_some() {
        // A walk that missed `rbrace` read the region differently from the scan asking it,
        // and is not kept for that scan to resume.
        resume.put(walk);
    }
    closed.is_some_and(Closed::ends_operand)
}

/// The walk's reading of every `/` that no `}` settles: this crate's answers for a `>` and
/// a `)`, and the bytes' for a `}` — which the walk never asks, a `/` after a `}` being its
/// own to answer ([`Region::divides_after_brace`]).
const WALK_GRAMMAR: OperandGrammar = crate::OPERAND_GRAMMAR;

/// [`WALK_GRAMMAR`] with every `}` ending an operand: the probe that tells whether the
/// operand before a `/` ends on a `}` ([`Region::divides_after_brace`]).
const BRACE_ENDS_OPERAND: OperandGrammar = OperandGrammar {
    closes_braced_operand: |_, _, _, _, _| true,
    ..WALK_GRAMMAR
};

/// A forward walk of one scanned region, token by token, kept between a scan's asks.
struct Walk {
    lower_bound: usize,
    start: ScanStart,
    /// The next byte to read. Everything before it has been tokenized.
    at: usize,
    /// The region `at` is in: the walk's whole range, or the interpolation it is inside.
    region: Region,
    /// The regions around `region`, outermost first.
    outer: Vec<Region>,
    contexts: TokenContexts,
}

impl Walk {
    fn new(lower_bound: usize, start: ScanStart) -> Self {
        Self {
            lower_bound,
            start,
            at: lower_bound,
            region: Region::new(lower_bound),
            outer: Vec::new(),
            contexts: TokenContexts::new(start),
        }
    }

    /// Whether this walk, begun for an earlier ask, is the walk an ask about `rbrace` would
    /// begin: the same region, and not yet past the brace.
    fn resumes(&self, rbrace: usize, lower_bound: usize, start: ScanStart) -> bool {
        self.lower_bound == lower_bound && self.start == start && self.at <= rbrace
    }

    /// Walk on through the `}` at `rbrace`, and say what it closed — `None` when the walk
    /// does not meet it as a token of its own.
    fn reach(&mut self, bytes: &[u8], rbrace: usize) -> Option<Closed> {
        while self.at <= rbrace {
            let i = self.at;
            let past_whitespace = skip_whitespace(bytes, i);
            if past_whitespace > i {
                self.contexts.newline |= holds_line_terminator(&bytes[i..past_whitespace]);
                self.at = past_whitespace;
                continue;
            }
            let b = *bytes.get(i)?;
            if b == b'`' {
                // Template text holds no token; the walk resumes at an interpolation or
                // past the closing backtick.
                self.at = self.template_text(bytes, i + 1, rbrace)?;
                continue;
            }
            if let Some(past) = skip_trivia(bytes, i, bytes.len(), TriviaProfile::JS) {
                self.region.anchor.skipped_trivia(
                    bytes,
                    i,
                    past,
                    self.region.lower_bound,
                    WALK_GRAMMAR,
                );
                if b == b'/' {
                    // A comment: transparent, except that a line comment's terminator — or
                    // one inside a block comment — is a line break between the tokens
                    // around it.
                    self.contexts.newline |=
                        bytes[i + 1] == b'/' || holds_line_terminator(&bytes[i..past]);
                } else {
                    self.contexts.token(Token::Operand);
                }
                self.at = past;
                continue;
            }
            if b == b'/' {
                let regex = self
                    .region
                    .divides_after_brace(bytes, i, &self.contexts)
                    .map_or_else(
                        || {
                            self.region.anchor.starts_regex(
                                bytes,
                                i,
                                self.region.lower_bound,
                                WALK_GRAMMAR,
                            )
                        },
                        |divides| !divides,
                    );
                if regex && let Some(past) = skip_regex_literal(bytes, i, bytes.len()) {
                    self.region.anchor.skipped_operand(past);
                    self.contexts.token(Token::Operand);
                    self.at = past;
                    continue;
                }
                self.contexts.token(Token::Operator);
                self.at = i + 1;
                continue;
            }
            if identifier_starts_at(bytes, i) || b == b'#' {
                let word_start = if b == b'#' { i + 1 } else { i };
                let word_end = skip_identifier(bytes, word_start).max(i + 1);
                self.contexts.word(&bytes[i..word_end]);
                self.at = word_end;
                continue;
            }
            if b.is_ascii_digit() || (b == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
            {
                self.at = skip_numeric_literal(bytes, i).max(i + 1);
                self.contexts.token(Token::Operand);
                continue;
            }
            if b == b'}' && self.contexts.top() == Context::Interpolation {
                // The interpolation's own `}`: template text follows, in the region around
                // it. One the walk began inside is the scan's own closer, which it never
                // asks about.
                self.region = self.outer.pop()?;
                self.contexts.stack.pop();
                self.at = self.template_text(bytes, i + 1, rbrace)?;
                continue;
            }
            let (token, len) = punctuator_at(bytes, i);
            self.contexts.token(token);
            self.at = i + len;
            if token == Token::BraceR {
                let closed = self.contexts.closed.as_mut()?;
                closed.bang = non_null_follows(bytes, i + 1);
                if i == rbrace {
                    return Some(*closed);
                }
            }
        }
        // The walk stepped over `rbrace` inside a literal or a comment.
        None
    }

    /// Read the template text beginning at `i` and return where tokens resume: inside an
    /// interpolation, which becomes the walk's region, or past the closing backtick. `None`
    /// when the text runs to `limit` — the position the walk is asked about, which is then
    /// text and no token.
    fn template_text(&mut self, bytes: &[u8], mut i: usize, limit: usize) -> Option<usize> {
        while i < limit {
            match bytes[i] {
                b'\\' => i += 2,
                b'`' => {
                    self.region.anchor.skipped_operand(i + 1);
                    self.contexts.token(Token::Operand);
                    return Some(i + 1);
                }
                b'$' if bytes.get(i + 1) == Some(&b'{') => {
                    self.contexts.open_interpolation();
                    self.outer
                        .push(std::mem::replace(&mut self.region, Region::new(i + 2)));
                    return Some(i + 2);
                }
                _ => i += 1,
            }
        }
        None
    }
}

/// One region the scan reads with an anchor of its own: the walk's whole range, or a
/// template literal's interpolation inside it.
struct Region {
    anchor: OperandAnchor,
    lower_bound: usize,
}

impl Region {
    fn new(lower_bound: usize) -> Self {
        Self {
            anchor: OperandAnchor::new(lower_bound),
            lower_bound,
        }
    }

    /// Whether the `/` at `slash` divides because the operand before it ends on the `}`
    /// last closed — `None` when it ends on anything else, and the scanners' own rule
    /// decides. This is the answer the scan was given when it asked about that brace.
    ///
    /// With no token since the brace, the operand ends on it: only whitespace and comments
    /// lie between, which the scan reads past too. A `!` run since is the anchor's to
    /// place, since the scan steps over one only where it is glued to the operand before
    /// it: the anchor is asked twice, with a `}` ending no operand and with a `}` ending
    /// every one, and the two disagree exactly when it landed on a `}` — the one last
    /// closed, since any token after that brace other than a `!` would be what the anchor
    /// landed on instead.
    fn divides_after_brace(
        &self,
        bytes: &[u8],
        slash: usize,
        contexts: &TokenContexts,
    ) -> Option<bool> {
        let closed = contexts.closed?;
        let ends_on_brace = contexts.prev == Token::BraceR
            || (self
                .anchor
                .starts_regex(bytes, slash, self.lower_bound, WALK_GRAMMAR)
                && !self
                    .anchor
                    .starts_regex(bytes, slash, self.lower_bound, BRACE_ENDS_OPERAND));
        ends_on_brace.then(|| closed.ends_operand())
    }
}

/// Whether a postfix `!` run glued to the `}` just before `after` follows it — the run the
/// scan steps over to ask about the brace ([`OperandAnchor`]'s rule for a glued run): one
/// right after the brace, or right after a block comment that sits on one line, with only
/// such comments and whitespace before it.
fn non_null_follows(bytes: &[u8], after: usize) -> bool {
    let mut i = after;
    loop {
        let past_whitespace = skip_whitespace(bytes, i);
        match bytes.get(past_whitespace) {
            Some(b'!') => return past_whitespace == i,
            Some(b'/') if bytes.get(past_whitespace + 1) == Some(&b'*') => {}
            _ => return false,
        }
        let Some(past) = skip_trivia(bytes, past_whitespace, bytes.len(), TriviaProfile::JS) else {
            return false;
        };
        if holds_line_terminator(&bytes[past_whitespace..past]) {
            return false;
        }
        i = past;
    }
}

/// Whether `bytes` holds an ECMAScript line terminator — LF, CR, LS or PS.
fn holds_line_terminator(bytes: &[u8]) -> bool {
    bytes.iter().enumerate().any(|(k, &b)| {
        matches!(b, b'\n' | b'\r')
            || (b == 0xE2 && matches!(bytes.get(k + 1..k + 3), Some([0x80, 0xA8 | 0xA9])))
    })
}

/// The punctuator token at `i` and its length — the classes [`TokenContexts`] tells apart.
/// Every other operator byte is a one-byte [`Token::Operator`]: an operator run reads the same
/// whether it is one token or several, since each leaves an expression expected after it.
fn punctuator_at(bytes: &[u8], i: usize) -> (Token, usize) {
    let next = bytes.get(i + 1).copied();
    match bytes[i] {
        b'(' => (Token::ParenL, 1),
        b')' => (Token::ParenR, 1),
        b'{' => (Token::BraceL, 1),
        b'}' => (Token::BraceR, 1),
        b']' | b'@' => (Token::Operand, 1),
        b';' => (Token::Semi, 1),
        b':' => (Token::Colon, 1),
        b'!' => (Token::Bang, 1),
        b'.' if next == Some(b'.') && bytes.get(i + 2) == Some(&b'.') => (Token::Operator, 3),
        b'.' => (Token::Dot, 1),
        b'?' if next == Some(b'.') && !bytes.get(i + 2).is_some_and(u8::is_ascii_digit) => {
            (Token::Dot, 2)
        }
        b'?' if next == Some(b'?') => (Token::Operator, 2),
        b'?' => (Token::Question, 1),
        b'=' if next == Some(b'>') => (Token::Arrow, 2),
        b'+' | b'-' if next == Some(bytes[i]) => (Token::Operand, 2),
        _ => (Token::Operator, 1),
    }
}

/// The token classes the context rules tell apart — a token's `beforeExpr` flag in acorn,
/// plus the types its `updateContext` and `braceIsBlock` name that an expression reaches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Token {
    /// Nothing yet: the start of a [`ScanStart::Expression`] region (acorn's `eof`).
    Start,
    /// An identifier — or a keyword read as a property name (after `.`), or one with no
    /// `beforeExpr` and no rule here (`this`, `const`, `await`, …).
    Name,
    /// A literal, or an operand-ending punctuator with no rule (`]`, `@`, `++`).
    Operand,
    /// A `beforeExpr` operator, punctuator or keyword with no rule of its own (`,`, `=`,
    /// `+`, `...`, `[`, `typeof`, `new`, `return`, …) — and the `${` a
    /// [`ScanStart::Interpolation`] region opens after.
    Operator,
    /// A `!`: an [`Self::Operator`] to acorn, told apart because a run of them glued to a
    /// `}` is TypeScript's postfix non-null, which leaves the `/` after it the brace's
    /// question ([`Closed`]).
    Bang,
    ParenL,
    ParenR,
    BraceL,
    BraceR,
    Semi,
    Colon,
    Question,
    Dot,
    Arrow,
    Extends,
    Function,
    Class,
}

impl Token {
    /// acorn's `beforeExpr` — whether an expression may begin after the token.
    fn before_expr(self) -> bool {
        matches!(
            self,
            Token::Operator
                | Token::Bang
                | Token::ParenL
                | Token::BraceL
                | Token::Semi
                | Token::Colon
                | Token::Question
                | Token::Arrow
                | Token::Extends
        )
    }
}

/// acorn's token contexts (`types` in `tokencontext.js`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Context {
    /// `b_stat` — a block or a body, and the base every parse starts in.
    BraceStatement,
    /// `b_expr` — an object literal.
    BraceExpression,
    /// `b_tmpl` — a template literal's `${…}`.
    Interpolation,
    /// `p_expr` / `p_stat` — a paren. What sets a statement header's apart is read only at
    /// statement level, where every brace answers alike.
    Paren,
    /// `f_stat` — a function or class read as a declaration.
    FunctionStatement,
    /// `f_expr` — a function or class read as an expression.
    FunctionExpression,
}

impl Context {
    /// acorn's `isExpr`.
    fn is_expr(self) -> bool {
        matches!(
            self,
            Context::BraceExpression | Context::Paren | Context::FunctionExpression
        )
    }

    fn is_function(self) -> bool {
        matches!(
            self,
            Context::FunctionStatement | Context::FunctionExpression
        )
    }
}

/// One entry of the context stack.
#[derive(Clone, Copy, Debug)]
struct Frame {
    context: Context,
    /// How many conditional `?`s are still open in this context, so a `:` can be told for a
    /// conditional's (where acorn's parser reads the `{` after it as an object literal)
    /// rather than a type annotation's or a property's.
    conditionals: u32,
    /// Whether the frame was pushed where a statement may begin: directly inside a block
    /// other than the region's base — or inside a brace or a function that itself was, since
    /// acorn's tokenizer takes some blocks there for object literals. Its `}` answers
    /// `false` ([`closes_braced_operand`]); a `(…)` or a `${…}` holds expressions only and
    /// starts over.
    statement_level: bool,
}

/// What the last `}` closed, for the `/` after it — kept while only `!`s follow the brace.
#[derive(Clone, Copy, Debug)]
struct Closed {
    /// Whether the brace ends an operand: an object literal's, or the body of a function or
    /// class acorn's tokenizer reads as an expression — and not at statement level.
    operand: bool,
    /// Whether it does when a postfix `!` run follows: any function or class body as well,
    /// which acorn-typescript's non-null reads as an operand whatever the tokenizer took
    /// the keyword for.
    operand_before_bang: bool,
    /// Whether a postfix `!` run glued to the brace follows it ([`non_null_follows`]).
    bang: bool,
}

impl Closed {
    fn of(popped: Option<Frame>) -> Self {
        let frame = popped.filter(|frame| !frame.statement_level);
        let operand = frame.is_some_and(|frame| {
            matches!(
                frame.context,
                Context::BraceExpression | Context::FunctionExpression
            )
        });
        Self {
            operand,
            operand_before_bang: operand || frame.is_some_and(|frame| frame.context.is_function()),
            bang: false,
        }
    }

    fn ends_operand(self) -> bool {
        if self.bang {
            self.operand_before_bang
        } else {
            self.operand
        }
    }
}

/// What the context rules read off the token before the current one beyond its class.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    None,
    /// The name `async`.
    AsyncName,
    /// A `class` keyword, or the name right after one — so an `extends` next is that
    /// class's heritage clause.
    ClassHead,
    /// A class heritage's `extends`: acorn's parser reads a `{` after it as an object
    /// literal.
    HeritageExtends,
    /// A `:` that closed a conditional opened by a `?` in the same context: acorn's parser
    /// reads a `{` after it as an object literal.
    ConditionalColon,
    /// The `function` of an `async function` acorn parses as an expression. Its parser
    /// rewrites the top context to an expression function's once the NEXT token is read:
    /// the function's own, when that token pushed none (a name, a `*`, a type parameter's
    /// `<`). A `(` there pushes the parameter list's context, which takes the rewrite
    /// instead — so an anonymous one's own context stays a statement's.
    AsyncFunction,
}

/// The part of acorn's tokenizer state an expression reaches, over a walk: the context
/// stack, `exprAllowed`, and the token before the current one.
///
/// A rule acorn states only for a token that begins or ends a STATEMENT — `else`, `return`,
/// a statement header's parens, `yield` in a generator — is not kept: such a token stands
/// directly inside a block, where every brace answers alike ([`Frame::statement_level`]).
struct TokenContexts {
    stack: Vec<Frame>,
    expr_allowed: bool,
    prev: Token,
    /// What else the rules read off `prev` ([`Role`]).
    prev_role: Role,
    /// Whether a line terminator sits between `prev` and the current token.
    newline: bool,
    /// What the last `}` closed ([`Closed`]).
    closed: Option<Closed>,
}

impl TokenContexts {
    fn new(start: ScanStart) -> Self {
        let base = Frame {
            context: Context::BraceStatement,
            conditionals: 0,
            statement_level: false,
        };
        let mut contexts = Self {
            stack: vec![base],
            expr_allowed: true,
            prev: Token::Start,
            prev_role: Role::None,
            newline: false,
            closed: None,
        };
        if start == ScanStart::Interpolation {
            contexts.open_interpolation();
        }
        contexts
    }

    fn top(&self) -> Context {
        self.stack
            .last()
            .map_or(Context::BraceStatement, |frame| frame.context)
    }

    /// Push `context`, at statement level when the frame it lands in is a block other than
    /// the region's base, or a brace or function that is itself at statement level
    /// ([`Frame::statement_level`]).
    fn push(&mut self, context: Context) {
        let statement_level = self.stack.last().is_some_and(|top| match top.context {
            Context::BraceStatement => self.stack.len() > 1,
            Context::BraceExpression | Context::FunctionStatement | Context::FunctionExpression => {
                top.statement_level
            }
            Context::Interpolation | Context::Paren => false,
        });
        self.stack.push(Frame {
            context,
            conditionals: 0,
            statement_level,
        });
    }

    /// acorn's `braceR.updateContext` / `parenR.updateContext`. A close whose opener lies
    /// before the walk pops what the region began inside — the base every parse starts in,
    /// an interpolation's `${` — which is no operand's context.
    fn pop_close(&mut self) -> Option<Frame> {
        let mut out = self.stack.pop()?;
        if out.context == Context::BraceStatement && self.top().is_function() {
            out = self.stack.pop()?;
        }
        Some(out)
    }

    /// acorn's `dollarBraceL.updateContext`: a template literal's `${` opens a context of
    /// its own, and an expression may begin after it.
    fn open_interpolation(&mut self) {
        self.push(Context::Interpolation);
        self.expr_allowed = true;
        self.prev = Token::Operator;
        self.prev_role = Role::None;
        self.newline = false;
        self.closed = None;
    }

    /// Whether the `{` being read opens a block: acorn's `braceIsBlock`, with its parser's
    /// `overrideContext(b_expr)` for a `{` the parser reads as an object literal where the
    /// tokenizer has a block — at a region's start, after a conditional's `:` and after a
    /// class heritage's `extends` (`role`, the one `prev` had).
    fn brace_is_block(&self, role: Role) -> bool {
        let parent = self.top();
        if parent.is_function() {
            // A function's or class's body — or, to the tokenizer, anything braced before it.
            return role != Role::HeritageExtends;
        }
        match self.prev {
            // An arrow's body; and after a `;` — in an expression, a `for` header's — a
            // block to the tokenizer.
            Token::Arrow | Token::Semi => true,
            // After `of`, a block when a line break intervenes.
            Token::Name if self.expr_allowed => self.newline,
            Token::Name => false,
            // A type annotation's `:` at the top of a block, the region's base included; in
            // an object literal a property's.
            Token::Colon
                if matches!(parent, Context::BraceStatement | Context::BraceExpression) =>
            {
                parent == Context::BraceStatement && role != Role::ConditionalColon
            }
            // An object literal where an expression may begin — the region's start
            // included, by the parser's override — and a block after an operand: a
            // method's body follows its `)`.
            _ => !self.expr_allowed,
        }
    }

    /// Read a word: a keyword's token, or a name.
    fn word(&mut self, word: &[u8]) {
        if self.prev == Token::Dot || word.first() == Some(&b'#') {
            // A property name, keyword or not: acorn's `type.keyword && prevType === tt.dot`.
            self.token(Token::Name);
            return;
        }
        let token = match word {
            b"function" => Token::Function,
            b"class" => Token::Class,
            b"extends" => Token::Extends,
            b"case" | b"default" | b"do" | b"else" | b"return" | b"throw" | b"new" | b"in"
            | b"instanceof" | b"typeof" | b"void" | b"delete" => Token::Operator,
            _ => {
                // acorn's `name.updateContext`: an expression may follow `of` where none was
                // expected — a `for` header's.
                let allowed = word == b"of" && !self.expr_allowed;
                let names_class = self.prev_role == Role::ClassHead;
                self.token(Token::Name);
                self.expr_allowed = allowed;
                self.prev_role = if word == b"async" {
                    Role::AsyncName
                } else if names_class {
                    Role::ClassHead
                } else {
                    Role::None
                };
                return;
            }
        };
        self.token(token);
    }

    /// Apply a token's `updateContext` and make it `prev`.
    fn token(&mut self, token: Token) {
        let role = std::mem::replace(&mut self.prev_role, Role::None);
        let closed = self.closed.take();
        match token {
            Token::ParenL => {
                self.push(Context::Paren);
                self.expr_allowed = true;
            }
            Token::ParenR => {
                self.expr_allowed = self.pop_close().is_none_or(|out| !out.context.is_expr());
            }
            Token::BraceR => {
                let out = self.pop_close();
                self.expr_allowed = out.is_none_or(|out| !out.context.is_expr());
                self.closed = Some(Closed::of(out));
            }
            Token::BraceL => {
                self.push(if self.brace_is_block(role) {
                    Context::BraceStatement
                } else {
                    Context::BraceExpression
                });
                self.expr_allowed = true;
            }
            Token::Function | Token::Class => {
                // A statement's keyword after a token no expression follows — and after a
                // `:` at the top of a block, the region's base included.
                let expression = self.prev.before_expr()
                    && !(self.prev == Token::Colon && self.top() == Context::BraceStatement);
                self.push(if expression {
                    Context::FunctionExpression
                } else {
                    Context::FunctionStatement
                });
                self.expr_allowed = false;
                self.prev_role = if token == Token::Class {
                    Role::ClassHead
                } else if role == Role::AsyncName && !self.newline {
                    Role::AsyncFunction
                } else {
                    Role::None
                };
            }
            Token::Extends => {
                // A heritage's `extends` follows the class's keyword, its name, or its type
                // parameters' `>`; one after any other name is a type parameter's constraint.
                if self.top().is_function() && (role == Role::ClassHead || self.prev != Token::Name)
                {
                    self.prev_role = Role::HeritageExtends;
                }
                self.expr_allowed = true;
            }
            Token::Colon => {
                if self.top().is_function() {
                    // A return type's `:` — or the one after a property named `function` or
                    // `class` — ends the function's context, and closes no conditional.
                    self.stack.pop();
                } else if let Some(frame) = self.stack.last_mut()
                    && frame.conditionals > 0
                {
                    frame.conditionals -= 1;
                    self.prev_role = Role::ConditionalColon;
                }
                self.expr_allowed = true;
            }
            Token::Question => {
                if let Some(frame) = self.stack.last_mut() {
                    frame.conditionals += 1;
                }
                self.expr_allowed = true;
            }
            Token::Bang => {
                self.closed = closed;
                self.expr_allowed = true;
            }
            _ => self.expr_allowed = token.before_expr(),
        }
        // acorn's `overrideContext(f_expr)`, one token late: its parser has already read the
        // token after `function` when it sees the `async` before it.
        if role == Role::AsyncFunction
            && let Some(top) = self.stack.last_mut()
            && top.context == Context::FunctionStatement
        {
            top.context = Context::FunctionExpression;
        }
        self.prev = token;
        self.newline = false;
    }
}

#[cfg(test)]
mod tests {
    use super::closes_braced_operand;
    use tsv_lang::source_scan::{BracedOperandWalk, ScanStart};

    /// One ask, with no walk to resume.
    fn closes(src: &[u8], rbrace: usize, lower_bound: usize, start: ScanStart) -> bool {
        closes_braced_operand(
            src,
            rbrace,
            lower_bound,
            start,
            &mut BracedOperandWalk::default(),
        )
    }

    /// The `}`s a `/` follows, past whitespace alone — the ones a scan asks about.
    fn braces_before_a_slash(src: &str) -> impl DoubleEndedIterator<Item = usize> {
        src.bytes()
            .enumerate()
            .filter(|&(at, b)| b == b'}' && src[at + 1..].trim_start().starts_with('/'))
            .map(|(at, _)| at)
    }

    /// Whether the first `}` a `/` follows ends an operand, the walk starting at byte 0 as
    /// `start` says.
    fn divides(src: &str, start: ScanStart) -> bool {
        let rbrace = braces_before_a_slash(src)
            .next()
            .expect("a `}` before a `/` to ask about");
        closes(src.as_bytes(), rbrace, 0, start)
    }

    /// [`divides`] for a whole island.
    fn island_divides(src: &str) -> bool {
        divides(src, ScanStart::Expression)
    }

    // Each verdict is Svelte's (`svelte.parse` of the island, which hands acorn the
    // expression as a parse of its own): whether the `/` after the `}` continues the
    // expression. The rows marked as a block's answer are the ones the walk answers `false`
    // by construction, where Svelte divides.

    #[test]
    fn an_object_literal_ends_an_operand() {
        for src in [
            "{} / 2",
            "{ a: 1 } / 2",
            "x + {} / 2",
            "c ? {} / 2 : d",
            "c ? d : {} / 2",
            "[{} / 2]",
            "{ a: function () {} } / 2",
            "{ class: 1 } / 2",
            "{ function: 1 } / 2",
            "`${a}` + {} / 2",
            "a.function + {} / 2",
            "f(c ? d : {} / 2)",
            // a paren inside a nested body holds expressions only
            "() => { f({} / 2) }",
            "(function* () { f(yield {} / 2); })",
            "[async function (a = {} / 2) {}]",
            // a `?` before a number's `.` opens a conditional
            "c ?.5 : {} / 2",
        ] {
            assert!(island_divides(src), "{src:?}");
            assert!(
                divides(src, ScanStart::Interpolation),
                "{src:?} interpolated"
            );
        }
    }

    #[test]
    fn a_function_or_class_acorn_reads_as_an_expression_ends_an_operand() {
        for src in [
            "x + function () {} / 2",
            "c ? function () {} / 2 : d",
            "f(c ? d : function () {} / 2)",
            "!function () {} / 2",
            "a = function () {} / 2",
            "x + class {} / 2",
            "x + class A extends f() { m() {} } / 2",
            "new class {} / 2",
            "x + function* () {} / 2",
            "[...function () {} / 2]",
            "typeof function () {} / 2",
            "void function () {} / 2",
            "delete function () {} / 2",
            "x in function () {} / 2",
            "x instanceof class {} / 2",
            // a `for` header's `;` inside a nested body: the parens hold expressions only
            "() => { for (;function () {} / 2;) {} }",
            "() => { for (;;function () {} / 2) {} }",
        ] {
            assert!(island_divides(src), "{src:?}");
        }
    }

    #[test]
    fn a_type_literal_is_braced_as_the_tokenizer_reads_it() {
        // acorn-typescript's parser overrides nothing for a `{` in a type, so the tokenizer's
        // reading stands: a return type's at the top level is a block, after whose `}` the
        // body's `{` reads as an object literal — and in a paren or an object literal it is
        // the other way round.
        for src in [
            "x + function (): {} {} / 2",
            "x + function (): { a: 1 } {} / 2",
            "x + function (): string {} / 2",
            // the return type's `:` closes no conditional
            "c ? x + function (): {} {} / 2 : d",
            // `?.` opens none
            "[a?.b, x + function (): {} {} / 2]",
        ] {
            assert!(island_divides(src), "{src:?}");
        }
        for src in [
            "(x + function (): {} {} / 2)",
            "{ k: x + function (): {} {} / 2 }",
        ] {
            assert!(!island_divides(src), "{src:?}");
        }
    }

    #[test]
    fn a_function_or_class_acorn_reads_as_a_statement_ends_none() {
        for src in [
            "function () {} / 2",
            "class {} / 2",
            "class extends B {} / 2",
            "c ? d : function () {} / 2",
            "[c ? d : function () {} / 2]",
            "x ? y : z ? {} : function () {} / 2",
            "c ? d : class {} / 2",
            // `yield` and `await` are names to the tokenizer, which no expression follows.
            "await function () {} / 2",
            "x + await class {} / 2",
            "yield function () {} / 2",
            "(function* () { f(yield function () {} / 2); })",
            // a decorator's name is one too
            "x + @dec class {} / 2",
            // a `:` pops a function context, so a property named `function` or `class`
            // leaves none behind to change what the conditional's `:` reads
            "[{function: 1}, c ? d : function () {} / 2]",
            "[{class: 1}, c ? d : class {} / 2]",
            // `?.` opens no conditional
            "[a?.b, c ? d : function () {} / 2]",
        ] {
            assert!(!island_divides(src), "{src:?}");
        }
    }

    #[test]
    fn an_async_function_is_an_expression_unless_anonymous() {
        // acorn's parser rewrites the context once it has read the token after `function`:
        // the function's own when that is a name, a `*` or a type parameter's `<`, and the
        // parameter list's when it is the `(`.
        for src in [
            "async function f() {} / 2",
            "x + async function f() {} / 2",
            "async function* () {} / 2",
            "async function* f() {} / 2",
            "c ? x : async function f() {} / 2",
            "[async function f() {} / 2]",
            "async function <T>() {} / 2",
            "async function f(a = {}) {} / 2",
            "x + async /* c */ function f() {} / 2",
        ] {
            assert!(island_divides(src), "{src:?}");
            assert!(
                divides(src, ScanStart::Interpolation),
                "{src:?} interpolated"
            );
        }
        for src in [
            "async function () {} / 2",
            "x + async function () {} / 2",
            "f(async function () {} / 2)",
            "async function (a = {}) {} / 2",
            // a line break after `async` ends the statement it is, a comment's included
            "async\nfunction f() {} / 2",
            "async // c\nfunction f() {} / 2",
            // a property named `async` is no keyword
            "x.async function f() {} / 2",
        ] {
            assert!(!island_divides(src), "{src:?}");
            assert!(
                !divides(src, ScanStart::Interpolation),
                "{src:?} interpolated"
            );
        }
    }

    #[test]
    fn a_class_heritage_object_is_an_object_literal() {
        // The heritage's `{` is an object literal, so the class's own context is still
        // open when its body closes.
        assert!(!island_divides("class extends {}.x {} / 2"));
        assert!(!island_divides("class A extends {}.x {} / 2"));
        assert!(island_divides("x + class extends {}.x {} / 2"));
        assert!(island_divides("x + class A extends {}.x {} / 2"));
        assert!(island_divides("x + class extends {} {} / 2"));
        assert!(!island_divides("class A<T> extends {}.x {} / 2"));
        assert!(island_divides("x + class A<T> extends {}.x {} / 2"));
        // A type parameter's constraint is no heritage: acorn-typescript's tokenizer takes
        // that `{` for a block, which pops the function's context with it.
        assert!(!island_divides("x + function <T extends {}>() {} / 2"));
        assert!(island_divides("x + class A<T extends {}> {} / 2"));
    }

    #[test]
    fn an_interpolation_reads_its_first_function_as_an_expression() {
        // Inside a template's `${`, the token before the keyword is the `${`.
        assert!(divides("function () {} / 2", ScanStart::Interpolation));
        assert!(divides(
            "c ? d : function () {} / 2",
            ScanStart::Interpolation
        ));
    }

    #[test]
    fn a_block_ends_none() {
        for src in [
            "() => { if (c) {} /}/ }",
            "() => { function f() {} /}/ }",
            "() => { a; {} /}/ }",
            "() => { a; function f() {} /}/.test(s) }",
            "() => { return\nfunction f() {}\n/}/.test(s) }",
            "() => { switch (a) { case 1: {} } /}/ }",
            "() => { switch (a) { case 1: {} /}/.test(s) } }",
            "() => { l: {} /}/.test(s) }",
            "() => { a?.b; l: {} /}/.test(s) }",
            "() => { try {} finally {} /}/ }",
            "() => { return // c\n{} /}/.test(s) }",
            "() => { return /* c\n */ {} /}/.test(s) }",
            "(function* () { yield\n{}\n/}/.test(s) })",
            "(a, b) => {} /}/",
        ] {
            assert!(!island_divides(src), "{src:?}");
        }
    }

    #[test]
    fn a_brace_at_statement_level_ends_none() {
        // Where a statement may begin, acorn's parser re-reads the tokenizer's division as a
        // regex, so a brace the tokenizer takes for an expression's is still followed by one.
        for src in [
            // a block after a statement that ended without its `;`
            "() => { a\n{} /}/.test(s) }",
            "() => { a = b\n{} /}/.test(s) }",
            "function () { a\n{} /}/.test(s) }",
            // TypeScript: a return type's `:` pops the function's context, and a declaration's
            // body follows a name or an `=`
            "() => { function f(): void {}\n/}/.test(s) }",
            "() => { function f(): string {}\n/[{]/.test(s) }",
            "() => { enum E { A }\n/}/.test(s) }",
            "() => { interface I {}\n/}/.test(s) }",
            "() => { namespace N {}\n/}/.test(s) }",
            "() => { type T = {}\n/}/.test(s) }",
            // inside a brace the tokenizer took for an object literal, and a class body
            "() => { namespace N { function f(): void {}\n/}/.test(s) } }",
            "x + class { static { a\n{} /}/.test(s) } }",
        ] {
            assert!(!island_divides(src), "{src:?}");
        }
        // The same answer where the grammar has a division, and Svelte divides.
        for src in [
            "() => { x = {} / 2 }",
            "() => { x = function () {} / 2 }",
            "() => { x = async function f() {} / 2 }",
            "(function* () { yield {} / 2; })",
            "(async function () { await {} / 2; })",
            "(function* () { yield x + function () {} / 2; })",
            "(async function () { await x + function () {} / 2; })",
            "x + class { a = {} / 2 }",
        ] {
            assert!(!island_divides(src), "{src:?}");
        }
        // A class's heritage is at the level its keyword is.
        assert!(!island_divides("() => { class A extends {} / 2 {} }"));
    }

    #[test]
    fn a_for_headers_object_literal_reads_as_the_tokenizers_block() {
        // acorn's parser overrides its tokenizer for an object literal after a `for`
        // header's `;` or its `of` and a line break; the walk keeps the tokenizer's block,
        // where Svelte divides.
        for src in [
            "() => { for (;{} / 2;) {} }",
            "() => { for (const x of\n{} / 2) {} }",
        ] {
            assert!(!island_divides(src), "{src:?}");
        }
        // On `of`'s own line the tokenizer has an object literal too.
        assert!(island_divides("() => { for (const x of {} / 2) {} }"));
    }

    #[test]
    fn a_brace_opened_before_the_walk_ends_none() {
        // The walk cannot see the `{`: the scan began inside it.
        assert!(!closes(b"a } / 2", 2, 0, ScanStart::Interpolation));
        assert!(!closes(b"a } / 2", 2, 0, ScanStart::Expression));
    }

    #[test]
    fn a_brace_inside_a_literal_is_no_token_of_the_walk() {
        // The asked offset is a `}` inside a string, or template text; the walk steps over
        // both whole.
        assert!(!closes(b"'{}' / 2", 2, 0, ScanStart::Expression));
        assert!(!closes(b"`{}` / 2", 2, 0, ScanStart::Expression));
        assert!(!closes(b"`${a}{}` / 2", 6, 0, ScanStart::Expression));
    }

    #[test]
    fn comments_and_line_breaks_are_transparent() {
        assert!(island_divides("x + {} /* c */ / 2"));
        assert!(island_divides("x + /* { */ {} / 2"));
        assert!(island_divides("x + // {\n{} / 2"));
        assert!(!island_divides("/* c */ function () {} / 2"));
        // A comment between the `}` and the `/` asked about.
        let src = b"x + {} // c\n/ 2";
        assert!(closes(src, 5, 0, ScanStart::Expression));
    }

    /// [`island_divides`] for the LAST `}` a `/` follows.
    fn last_divides(src: &str) -> bool {
        let rbrace = braces_before_a_slash(src)
            .next_back()
            .expect("a `}` before a `/` to ask about");
        closes(src.as_bytes(), rbrace, 0, ScanStart::Expression)
    }

    #[test]
    fn the_walk_reads_its_own_earlier_slashes_as_it_answers() {
        // An earlier `} /` in the region is read by the same contexts, so the tokens after
        // it are the ones the scan read.
        for src in [
            "[{} / 1, {} / 2]",
            "{} / {} / 2",
            "x + function () {} / {} / 2",
            "[/a/ / 1, {} / 2]",
            "`${a}` + f<T> / {} / 2",
        ] {
            assert!(last_divides(src), "{src:?}");
        }
        // One that opens a regex swallows what follows it.
        assert!(!last_divides("function () {} / {} / 2"));
    }

    #[test]
    fn a_template_literal_is_tokenized_in_line() {
        for src in [
            "`${{} / 1}` + {} / 2",
            "[`${{} / 1}`, {} / 2]",
            "`${`${{} / 1}`}` + {} / 2",
            "`a${b}c${{} / 1}d` + {} / 2",
            "`\\${` + {} / 2",
            "`${function () {} / 1}` + {} / 2",
            "[() => { x = `${{} / 1}` }, {} / 2]",
        ] {
            assert!(last_divides(src), "{src:?}");
        }
        // The answer inside an interpolation is the one a walk beginning there gives.
        let src = "x + `${function () {} / 1}`";
        let rbrace = braces_before_a_slash(src)
            .next()
            .expect("a `}` before a `/`");
        let interpolation = src.bytes().position(|b| b == b'$').expect("a `${`") + 2;
        assert!(closes(src.as_bytes(), rbrace, 0, ScanStart::Expression));
        assert!(closes(
            src.as_bytes(),
            rbrace,
            interpolation,
            ScanStart::Interpolation
        ));
    }

    #[test]
    fn template_nesting_is_walked_once() {
        // Each level holds a `} /` after the template nested in it. A walk that re-scanned
        // a template it passes — asking again about each `} /` inside — would double its
        // work per level, and this depth would not finish.
        let mut src = String::from("{} / 1");
        for _ in 0..200 {
            src = format!("`${{{src}}}` + {{}} / 1");
        }
        assert!(last_divides(&src));
    }

    #[test]
    fn a_non_null_run_leaves_the_question_the_braces() {
        // TypeScript's postfix `!`, glued to the `}`.
        let bang = |src: &str| {
            let rbrace = src
                .bytes()
                .zip(src.bytes().skip(1))
                .position(|pair| pair == (b'}', b'!'))
                .expect("a `}!` to ask about");
            closes(src.as_bytes(), rbrace, 0, ScanStart::Expression)
        };
        for src in [
            "{}! / 2",
            "{}!! / 2",
            "x + function () {}! / 2",
            // a function or class body ends an operand before a `!`, whatever the tokenizer
            // took its keyword for
            "function () {}! / 2",
            "class {}! / 2",
            "x + async function () {}! / 2",
            "c ? x : function () {}! / 2",
            "[c ? x : function () {}! / 2]",
            "function () {}!! / 2",
        ] {
            assert!(bang(src), "{src:?}");
        }
        for src in [
            // an arrow's body is a block
            "() => {}! / 2",
            // a statement's `}` and a prefix `!`
            "() => { function f() {}!/}/.test(s) }",
            "() => { if (c) {}!/}/.test(s) }",
            "() => { x = function () {}! / 2 }",
        ] {
            assert!(!bang(src), "{src:?}");
        }
        // A run glued to a one-line block comment after the brace is one the scan steps
        // over too.
        assert!(closes(b"{} /* c */! / 2", 1, 0, ScanStart::Expression));
        for src in [
            "function () {} /* c */! / 2",
            "function () {}/* c */ /* d */! / 2",
        ] {
            assert!(
                closes(src.as_bytes(), 13, 0, ScanStart::Expression),
                "{src:?}"
            );
        }
        // Not one after whitespace, a comment that spans lines, or a line comment: a prefix
        // operator, whose brace the scan never asks about.
        for src in [
            "function () {} /* c */ ! / 2",
            "function () {} /* c\n */! / 2",
            "function () {} // c\n! / 2",
        ] {
            assert!(
                !closes(src.as_bytes(), 13, 0, ScanStart::Expression),
                "{src:?}"
            );
        }
        // An earlier `}!` the walk passes reads the same way.
        assert!(last_divides("{}! / {} / 2"));
        assert!(last_divides("function () {}! / {} / 2"));
        assert!(last_divides("{} /* c */! / {} / 2"));
        // A `!` with a space before it is a prefix operator to the scan, which never asks
        // about the brace before it — and to the walk, which reads a regex after it.
        assert!(!last_divides("{} ! / {} / 2"));
    }

    #[test]
    fn a_resumed_walk_answers_as_a_fresh_one() {
        // A scan asks about each `}` + `/` of a region in turn, handing back the walk the
        // last ask left: every answer is the one a walk from the region's start gives.
        for src in [
            "[{} / 1, {} / 2, x + function () {} / 3, function () {} / 4]",
            "{} / {} / 2 + c ? d : function () {} / 5",
            "`${{} / 1}` + {} / 2 + `a${`${function () {} / 3}`}b` + {} / 4",
            "() => { a\n{} /}/.test(s) } + {} / 2",
            "{}! / {} / 2 + function () {}! / {} / 3",
            "x + async function f() {} / async function () {} / 2",
            "x + class extends {} {} / {} / 2",
            "{ a: {} / 1, b: () => { x = {} / 2 }, c: {} / 3 } / 4",
            "'}' / {} / 1 + /}/ / {} / 2",
            "f<T> / {} / 1 + {} / f<T> / 2",
        ] {
            for start in [ScanStart::Expression, ScanStart::Interpolation] {
                let mut walk = BracedOperandWalk::default();
                // Twice over: the second pass begins with a walk left past its first brace,
                // which is not one to resume.
                for rbrace in braces_before_a_slash(src).chain(braces_before_a_slash(src)) {
                    let fresh = closes(src.as_bytes(), rbrace, 0, start);
                    let resumed =
                        closes_braced_operand(src.as_bytes(), rbrace, 0, start, &mut walk);
                    assert_eq!(resumed, fresh, "{src:?} at {rbrace}");
                }
            }
        }
        // Nor is a walk begun for another region: from its `function`, this one reads a
        // regex after the body, which swallows the object literal.
        let src = b"x + function () {} / {} / 2";
        let mut walk = BracedOperandWalk::default();
        assert!(closes_braced_operand(
            src,
            17,
            0,
            ScanStart::Expression,
            &mut walk
        ));
        assert!(!closes_braced_operand(
            src,
            22,
            4,
            ScanStart::Expression,
            &mut walk
        ));
        // Nor one begun as another kind of region: inside an interpolation the body ends an
        // operand, and as a whole expression the same bytes open on a statement's keyword.
        let src = b"function () {} / 2 + {} / 3";
        let mut walk = BracedOperandWalk::default();
        assert!(closes_braced_operand(
            src,
            13,
            0,
            ScanStart::Interpolation,
            &mut walk
        ));
        assert!(!closes_braced_operand(
            src,
            22,
            0,
            ScanStart::Expression,
            &mut walk
        ));
    }

    #[test]
    fn a_region_of_many_braced_operands_is_walked_once() {
        // Asked about every `}` in turn with the walk handed back, the region is tokenized
        // once. Walked from its start at each ask, this many pairs would not finish.
        let pairs = 200_000;
        let src = format!("[{}]", vec!["{} / 1"; pairs].join(", "));
        let mut walk = BracedOperandWalk::default();
        let mut divided = 0;
        for rbrace in braces_before_a_slash(&src) {
            divided += usize::from(closes_braced_operand(
                src.as_bytes(),
                rbrace,
                0,
                ScanStart::Expression,
                &mut walk,
            ));
        }
        assert_eq!(divided, pairs);
    }
}
