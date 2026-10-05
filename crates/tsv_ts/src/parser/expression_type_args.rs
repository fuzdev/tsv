// Type-argument byte-scan lookahead: disambiguates `<Type, ...>` from the
// less-than operator without lexing, by scanning raw source bytes after `<`.

use super::Parser;
use super::expression_lookahead::{
    construct_type_arrow_end, generic_function_type_arrow_end, has_line_terminator_between,
    matching_angle_close, matching_delimiter_close, paren_list_arrow_end,
    paren_starts_function_type, paren_starts_modified_parameter_list,
    scan_for_closing_angle_bracket, starts_expression_after_type_args,
};
use super::scan::{
    identifier_starts_at, is_word_at, skip_identifier, skip_numeric_literal, skip_whitespace,
    skip_whitespace_and_comments,
};
use smallvec::SmallVec;
use tsv_lang::source_scan::{
    OperandAnchor, OperandGrammar, TriviaProfile, skip_regex_literal, skip_template_literal,
    skip_trivia,
};

/// Which reading of the type-argument lookahead a caller wants — the one parameter the two
/// readings are threaded through, so the walk stays a single body and they cannot drift.
///
/// Both readings ask the same structural question. They differ in WHICH TEXT they ask it of
/// and WHOSE grammar answers: [`Parse`] grades the source bytes as the author wrote them
/// against acorn-typescript, tsv's AST drop-in oracle; [`Relex`] grades the form the printer
/// would emit from them against every parser that will read tsv's output — tsc, and tsv's
/// own [`Parse`].
///
/// **What the printer MOVES**, each a place the printed form is not the source:
///
/// - **a line break before a `[`, which the printer folds away.** A break there ends the type
///   to tsc and acorn-typescript alike (the loop in each is named in
///   `docs/conformance_prettier_ts.md` §Relational chain type-argument parens), so
///   `fn<A⏎[T]>(t, u)` is a comparison chain — and every parse-then-format entry point
///   folds that break (`tsv_lang::printing::normalize_carriage_returns`, plus the printer's
///   own soft-line joins) before the output is read back. [`type_arg_head_commits`]'s `[`
///   arm, at the list's own level and inside an index alike. A break before a name's own
///   `<` is the same move (`fn<A[B⏎<C>]>(t, u)`), read by that walk's nested-list arm.
/// - **a paren shell, which the printer strips**, read wherever one can stand. Past an
///   operand, a `)` the region did not open is not in the printed form, so it neither ends
///   the operand nor ends the scan ([`skip_relex_operand_suffixes`], and
///   [`matching_angle_close`] mid-scan) —
///   without which the pair this reading justifies would erase itself on the next pass,
///   since tsv's own output is the shell-bearing spelling (`(a < b) > c`). At an operand's
///   HEAD, the shell is looked THROUGH and the head question asked of its content
///   ([`type_arg_head_grade`]'s `(` arm), so `a < (arr[b - 1]) > c` grades the arithmetic
///   its own paren-free twin does. The same shell stands in two more places, each one where
///   stripping it joins the tokens on either side: behind a prefix operator's own operand
///   (`-(1)` prints as `-1`, `typeof (b)` as `typeof b` — [`skip_stripped_shells`]), and
///   ahead of a qualified name's next segment (`(b).c` prints as `b.c` —
///   [`OperandSuffix::shell_only`]). [`Parse`] answers for one in a single place, where
///   no pair of the printer's can: a shell heading the region, ahead of a `<`
///   ([`OperandEnd::RegionHeadShell`]).
/// - **a paren pair, which the printer adds**, wherever a pair it writes decides what a
///   region reads as. An arrow function's bare-name parameter takes one
///   (`b[x => y]` prints as `b[(x) => y]`, a function type's head —
///   [`check_identifier_type_arg_pattern`]); a commented operand of a sign takes one
///   (`- /* c */ 1` prints as `-(/* c */ 1)`, no literal type — [`skip_stripped_shells`]);
///   and an operand of a comparison nested in an index takes whichever pair its position
///   asks for (`a < b[B<C>[]] > c` prints its index as `b[(B < C) > []]`), which moves the
///   delimiters a nested list is matched by — one of the two reasons this reading grades
///   an index no further than its first operand ([`Region::Nested`]).
/// - **a line break past the `>`, which the printer may or may not add.** Past one, tsv's
///   own [`Parse`] (with acorn-typescript) commits the list ahead of any expression. Which
///   followers commit, for tsc and for acorn-typescript, is stated once in the same catalog
///   entry; where tsc refuses a follower that [`Parse`] commits on, the pair is owed to
///   soundness (below), not to tsc. Whether the printer takes the break is unknowable where
///   parens are decided, so [`Relex`] asks about the REGION alone and skips the follower
///   entirely ([`scan_for_closing_angle_bracket`]).
///
///   That is the `>` which CLOSES the chain. A lone `>` the region reaches before it — tsc's
///   recovering list parse stops at the first token the type grammar cannot take, so the
///   `>` inside `x < (a > b)`, or a comma sibling's in `fn(x < q, a > b)` — is no reading's
///   to answer, since no pair can end a region ahead of a token inside it. It takes a
///   LAYOUT answer instead: the printer never lets it end a line, nor prints a pair of
///   its own directly behind it (`BinaryExpression::may_close_type_arguments`).
///
/// WHITESPACE between a numeric type's SIGN and its digits is not one of them, though the
/// printer folds a line break there too: a literal type's `-` is a token of its own to tsc
/// and acorn-typescript alike, so both readings step over it
/// ([`skip_signed_numeric_literal`]) and `fn<-⏎⏎1>(t)` is a generic call that prints as
/// `fn<-1>(t)`.
///
/// **One token the two ORACLES read differently**, no printer move involved: the non-null
/// `!`. `T!` is tsc's `JSDocNonNullableType` — prefix and postfix — so `<b!>` is a
/// type-argument list to the compiler, where acorn-typescript reads a comparison and tsv's
/// parse follows it. Admitting it at [`Parse`] would move tsv's AST off the drop-in
/// contract; refusing it at [`Relex`] would emit an output tsc reads as a different program
/// ([`skip_relex_operand_suffixes`] for the postfix, [`type_arg_head_grade`]'s `!` arm for
/// the prefix — both at the list's own level).
///
/// **Two properties bound every relaxation above, and neither is "[`Relex`] is never
/// stricter than [`Parse`] on one string".** That comparison grades both readings against
/// the SAME bytes, which is a proxy — it holds only where the source and the printed form
/// give [`Parse`] the same answer at that `<`, and the whole reason [`Relex`] exists is the
/// places they do not (a line break, a shell). The two properties name the texts apart:
///
/// - **Soundness.** For every document `D`: if tsv's own [`Parse`] reads a type-argument
///   region at the `<` of `format(D)`, the printer must have put a pair there. tsv's parser
///   is one of the readers of the output the verdict is about, so a `<` [`Relex`] leaves
///   bare and [`Parse`] then claims is an output tsv cannot reparse. **Nothing gates this
///   today**: `gaps:audit` grades the fixture tree as authored plus its injections, and no
///   fixture can hold the shapes that break it — a fixture input must format to itself, and
///   the broken form is one only the printer produces.
/// - **Independence.** [`Relex`] must agree with ITSELF across the redundant-paren
///   spellings of one program: `a < X > c` and `a < (X) > c` are one document, so they owe
///   one verdict. Gated by `deno task paren:audit` (its `< > chain` and `< > operand`
///   classes). This is soundness's dual: a `(` arm that graded nothing would give a
///   parenthesized operand a pair its bare twin lacks.
///
/// The bracketed heads are where soundness bites, and it is what splits them: a `(` head
/// grades its BODY ([`paren_type_head_close`]), a `{` or `[` head grades none. The grade
/// is the `(` head's SECOND question — a group that is a parameter list closing on `=>`
/// is a function type and is claimed ahead of it, since no comparison chain can spell
/// one — so what the grade reads is every other `(`-headed region.
///
/// **The `(` head can be graded because a region it refuses never reaches this scan bare.**
/// Where the printer KEEPS the operand's shell, the region's first printed byte is that `(`
/// and the chain takes a pair around the `>`'s left operand (`needs_parens`'s
/// `relational_region_opens_on_a_kept_shell`), so tsv's own parse meets the pair and not the
/// region; where the printer STRIPS it, the shell is not in the printed form at all and
/// [`Relex`] looks THROUGH it at the content, grading the bytes the output will hold. Either
/// way the verdict is taken on the text that is actually printed, which is a property of the
/// head's own shell rather than of its body — so no enumeration of what the printer may
/// parenthesize INSIDE the body is owed, and a wrap that moves a refusing token one level
/// deeper cannot make the grade unsound.
///
/// **The other two heads have no such shell, so they grade no body.** Neither `{` nor `[` is
/// a shell the printer strips, neither opens a region the kept-shell rule answers, and the
/// printer parenthesizes freely inside both (a spread argument, an `as` left operand, a
/// for-init `in`). So they commit on any matching `>` a committing follower stands past, and
/// tsv reads `x < { ...s } >⏎c` and `x < [b, c++] > (c, d)` as type-argument lists and then
/// rejects them — where acorn and tsc both read the comparison chain. That over-rejection is
/// [`Parse`]'s, and belongs to the parse oracles
/// (`docs/conformance_svelte.md` §TypeScript Corrections).
///
/// **An index's body is a region of its own, and the same three delimiters split the same
/// way inside it — for the same reason, one level down.** The walk grades the body of
/// `A[`…`]` operand by operand ([`Region::Nested`]), so a body the type grammar spells keeps
/// the list (`f<A[() => B]>(x)`, `f<A[keyof B | C]>(x)`) and an expression refuses it
/// (`f<A[a.b()]>(x)`). A `{` or `[` there is ungraded exactly as at the head, and commits:
/// `f<A[{ a: 1 }]>(x)` is the generic call both oracles read, and
/// `a < b[{ c: d + 1 }] > (t, u)` the same known over-rejection. A `(` there is where the
/// index parts from the head, because inside an index a shell is never REQUIRED — the index
/// takes a whole expression — so the printer strips the pair around one body and adds one
/// around another (`b[(c)]` prints as `b[c]`, `b[c = d]` as `b[(c = d)]`), and no rule of
/// the kept-shell kind can say which spelling the output holds. So both readings read a
/// shell there by its CONTENT ([`HeadGrade::Shell`]): the two spellings of one index are
/// then one verdict by construction, and the parameter-list claim the head follows tsc on
/// (`x < (a = b) > (c, d)`) is not made of `x < b[(a = c)] > (d, e)`, which stays the
/// comparison chain acorn-typescript reads. Past the shell, [`Relex`] reads an index far
/// more coarsely than [`Parse`] does, and always toward the pair ([`Region::Nested`]).
///
/// **What the `(` grade refuses is tsc's answer, not a judgment about types.** tsc parses
/// the list for real and with error recovery, claiming the region whenever the recovery still
/// reaches the `>` — so a body that is plainly no type is very often a region the compiler
/// claims and then rejects, and only the bodies it ABANDONS are comparison chains. Those are
/// the cells that may refuse, and [`grade_body_token`] states the whole table.
///
/// **The `(` arm's look-through carries soundness only where the printer STRIPS the shell,
/// so the shells it KEEPS are not this reading's to answer.** Where the printer strips, the
/// content IS the printed bytes and the two readings are asking about one text. Where it
/// keeps the shell, they are not: [`Relex`] would answer about the content while [`Parse`]
/// answers about the `(`-headed region that is actually printed, and on any content that is
/// no type they disagree. That half is settled in the printer instead, where the shell's
/// fate is known — `needs_parens`'s `relational_region_opens_on_a_kept_shell` disjunct,
/// which asks whether the region's FIRST PRINTED BYTE is a `(` the operand or its leftmost
/// printed spine keeps, so soundness holds over the pair of readings rather than over this
/// one alone.
/// The class it covers is every operand whose shell survives printing and whose head and
/// follow token are no type — assignment and compound assignment, a conditional, `||` /
/// `&&` / `??`, equality, `^`, `in` / `instanceof`, `await`, `yield`, `as` / `satisfies`. A
/// kept shell whose content happens to SPELL a type reaches the pair through this reading
/// anyway: a sequence spells the argument separator, `&` and `|` a type intersection or
/// union, and `() =>` a function type, so [`Relex`] commits and the disjunction is
/// redundant there. What remains is the two families [`Parse`] still refuses to read as a
/// chain, both of them tsc's own: a group whose content spells a PARAMETER LIST
/// (`x < (a = b) > (c, d)`), which tsc claims for a function type before reading any body,
/// and a body its error RECOVERY carries to the `>` (`x < (a << b) > (c, d)`). tsv follows
/// the compiler on both (`docs/conformance_svelte.md` §TypeScript Corrections), so the
/// disjunct is what keeps the pair standing over them — and, on every `(`-headed region the
/// grade refuses, it is what makes grading the body sound at all.
///
/// [`Parse`]: TypeArgScan::Parse
/// [`Relex`]: TypeArgScan::Relex
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TypeArgScan {
    /// What the PARSER commits to: the input's own bytes, line terminators and parens and
    /// all.
    Parse,
    /// What the PRINTED form would re-lex as, which is the printer's question. Never decides
    /// a parse; it only records whether a paren pair has to stand between two tokens (see
    /// `Printer::needs_parens_binary_operand`).
    Relex,
    /// [`Relex`](TypeArgScan::Relex) with an index body graded as finely as the parse
    /// grades it, where that reading takes the body's first operand and no more
    /// ([`Region::Nested`]). Asked behind a region [`Relex`](TypeArgScan::Relex) has
    /// committed on, for the one thing the coarse grade cannot see: a token inside the
    /// index that the two parse oracles read differently
    /// (`Parser::chain_owes_type_argument_pair`).
    RelexFine,
}

impl TypeArgScan {
    /// Whether this reading grades the source bytes AS WRITTEN, against acorn. False for
    /// [`Relex`](TypeArgScan::Relex), which grades the printed form — whose readers are
    /// named on [`TypeArgScan`], along with the sites that read each difference.
    #[inline]
    pub(super) const fn reads_source_as_written(self) -> bool {
        matches!(self, TypeArgScan::Parse)
    }

    /// Whether this reading grades an index body operand by operand, rather than on its
    /// first operand alone ([`Region::is_coarse_under`]).
    #[inline]
    const fn grades_index_finely(self) -> bool {
        !matches!(self, TypeArgScan::Relex)
    }
}

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Check if current position starts type arguments: `<Type, ...>`
    ///
    /// Uses lookahead to distinguish from the comparison operator, dispatching on the
    /// first token after `<`. [`type_keyword_at`] classifies the identifier-shaped heads
    /// ahead of the byte dispatch, since the identifier arm would otherwise claim them:
    /// - Type keywords: an atom (`<string>`, `<never>`), `this`, or an operator
    ///   (`<keyof T>`, `<typeof x>` — [`type_operator_grade`])
    /// - Identifiers: `<T>`, `<Ns.Type>`, `<T | U>`, `<T, U>`
    /// - Function types: `<(x: T) => R>`, `<() => R>`, `<<T>(v: T) => void>`
    /// - Parenthesized types: `<(A | B) & C>`, `<(() => void) | null>`
    /// - Object/tuple types: `<{ a: T }>`, `<[T, U]>`
    /// - Literal types: `<"foo">`, `` <`a${B}`> ``, `<42>`, `<-1>`, `<.5>`
    /// - A leading union/intersection bar: `<| A | B>`, `<& A & B>`
    ///
    /// Every operand-headed arm ends at the shared follow-token filter
    /// ([`type_arg_head_commits`]), which is what decides call vs comparison.
    ///
    /// `scan` names which of the two readings the caller wants — see [`TypeArgScan`]. The
    /// parser always asks [`TypeArgScan::Parse`]; the one [`TypeArgScan::Relex`] asker is a
    /// `>` reducing over a `<` left operand, recording on that `<` node whether its PRINTED
    /// form would re-lex as a type-argument list.
    pub(super) fn is_type_arguments_start(&self, scan: TypeArgScan) -> bool {
        self.is_type_arguments_start_at(self.current.start as usize, scan)
    }

    /// [`Parser::is_type_arguments_start`] asked at an EXPLICIT `<`, rather than at the
    /// token the parser is sitting on.
    ///
    /// The relaxed [`TypeArgScan::Relex`] reading is taken at a `>` — the point at which
    /// both tokens of the join it grades exist — by which time the lexer has long left the
    /// `<` behind, so that caller supplies the offset itself.
    pub(super) fn is_type_arguments_start_at(&self, lt_start: usize, scan: TypeArgScan) -> bool {
        let bytes = self.source.as_bytes();

        // Must start with '<'
        if lt_start >= bytes.len() || bytes[lt_start] != b'<' {
            return false;
        }

        // Skip whitespace AND comments after '<' - comments can appear before types
        let pos = skip_whitespace_and_comments(bytes, lt_start + 1);
        pos < bytes.len() && type_arg_head_commits(bytes, pos, scan)
    }

    /// Whether the chain whose `<` stands at `lt_start` owes a paren pair around that
    /// `<`'s node — the one asker of [`TypeArgScan::Relex`]. `source_pair` says the source
    /// already holds one there.
    ///
    /// The printed form re-lexing as a type-argument list is what owes it, with one
    /// exception. That reading grades an index body coarsely, so it commits on bodies a
    /// finer grade reads further into, and one kind it must not put a pair around: a body
    /// that grade leaves, with no claim made, at a token ONE parse oracle reads a type
    /// through ([`IndexSplit`]) — or reads to the region's close through one, a generic
    /// head's parameter default ([`type_arg_head_walk`]). The chain is what tsv parses
    /// either way; but that oracle reads the bare text as a generic call, and a pair
    /// would hand it a comparison instead — a second program where the bare form keeps
    /// the one it read. So the pair is whatever the SOURCE had there: kept where the
    /// author wrote one, which that oracle read as the comparison too, and not added
    /// where there was none.
    pub(super) fn chain_owes_type_argument_pair(&self, lt_start: usize, source_pair: bool) -> bool {
        if !self.is_type_arguments_start_at(lt_start, TypeArgScan::Relex) {
            return false;
        }
        if source_pair {
            return true;
        }
        let bytes = self.source.as_bytes();
        let pos = skip_whitespace_and_comments(bytes, lt_start + 1);
        let mut split_unclaimed = false;
        type_arg_head_walk(bytes, pos, TypeArgScan::RelexFine, &mut split_unclaimed)
            || !split_unclaimed
    }
}

/// Whether the `>` at `gt` closes a type-argument list this crate's parser would read —
/// `tsv_lang`'s `ClosesTypeArguments` for the TypeScript grammar, so a raw byte scan
/// that meets `f<T> / 2` reads the division the parser will, instead of opening a regex
/// at `/ 2` and running past the expression's end.
///
/// It asks the parser's own question from the other end. The parser tries a list at a
/// `<` that follows an operand (the subscript loop, `new C<T>`, a type reference) and
/// commits on [`type_arg_head_commits`] under [`TypeArgScan::Parse`], so this walks
/// `lower_bound..gt` for such a `<` — its position read by the same operand rule the
/// scanners use ([`OperandAnchor`]), comments and strings stepped over — and answers for
/// the one whose [`matching_angle_close`] is `gt`. At most one `<` can close there: a
/// second opener between them raises the angle depth past `gt`, and one inside a
/// bracket or paren stops at its own unbalanced closer.
///
/// A `<` after a postfix `++` / `--` opens no list — the update is no subscript base, so
/// the parser reads a comparison there — and is passed over; so is one after a statement
/// header's `)` (`if (c) <T>x`), where the parser begins a statement, not a subscript.
///
/// Asked only for a `/` right after a `>`, which real code almost never writes, so the
/// walk's cost is paid there alone.
// TODO: the walk is O(gt - lower_bound) per ask, so a region holding many `> /` pairs
// (`a > /x/ > /x/ > …`) pays it quadratically. Nothing real has the shape; a scan that
// recorded each operand-position `<` as it passed would make it linear.
pub(crate) fn closes_type_arguments(bytes: &[u8], gt: usize, lower_bound: usize) -> bool {
    // The inner ask needs no type-argument resolver — a `>` right before a `<` ends no
    // list the parser would subscript again — but it does read a `!` glued to a header's
    // `)` as the prefix not it is (`if (c)!<T>x`), whose `<` opens an assertion.
    let grammar = OperandGrammar {
        closes_type_arguments: |_, _, _| false,
        closes_statement_header,
        ..OperandGrammar::BYTES_ONLY
    };
    let mut anchor = OperandAnchor::new(lower_bound);
    let mut headers = StatementHeaders::default();
    let mut i = lower_bound;
    while i < gt {
        if let Some(past) = skip_trivia(bytes, i, gt, TriviaProfile::JS) {
            anchor.skipped_trivia(bytes, i, past, lower_bound, grammar);
            i = past;
            continue;
        }
        if bytes[i] == b'/'
            && let Some(past) =
                skip_walked_regex(bytes, i, gt, &anchor, &headers, lower_bound, grammar)
        {
            anchor.skipped_operand(past);
            headers.skipped_operand();
            i = past;
            continue;
        }
        let b = bytes[i];
        if b == b'<'
            && !headers.after_header_close
            // Past an operand: a `/` here would divide.
            && !anchor.starts_regex(bytes, i, lower_bound, grammar)
            && !follows_postfix_update(bytes, i, lower_bound)
            && matching_angle_close(bytes, i + 1, TypeArgScan::Parse) == Some(gt)
        {
            let head = skip_whitespace_and_comments(bytes, i + 1);
            return head < bytes.len() && type_arg_head_commits(bytes, head, TypeArgScan::Parse);
        }
        i = if b.is_ascii_whitespace() {
            i + 1
        } else {
            headers.step(bytes, i, gt)
        };
    }
    false
}

/// Whether the `)` at `rparen` closes a statement header this crate's parser would read —
/// `if (c)`, `while (c)`, `for (…)`, `for await (…)`, `with (o)` — so a statement begins
/// after it. `tsv_lang`'s `ClosesStatementHeader` for the TypeScript grammar: a scan that
/// meets a `!` glued to a `)` asks it, and reads `if (c)!/re/` as the prefix `!` and regex
/// the parser will, where `f()! / 2` is a postfix non-null divided.
///
/// The same forward walk [`closes_type_arguments`] takes ([`StatementHeaders`]), from
/// `lower_bound` to the `)`. Asked only of a `)` a glued `!` run follows, which real code
/// almost never writes before a `/`, so the walk's cost is paid there alone.
pub(crate) fn closes_statement_header(bytes: &[u8], rparen: usize, lower_bound: usize) -> bool {
    // The walk's own anchor, only to step over a regex literal (`if (/\)/.test(s))!`), so it
    // asks no header question itself: no recursion into this walk.
    let grammar = OperandGrammar::BYTES_ONLY;
    let mut anchor = OperandAnchor::new(lower_bound);
    let mut headers = StatementHeaders::default();
    let mut i = lower_bound;
    while i < rparen {
        if let Some(past) = skip_trivia(bytes, i, rparen, TriviaProfile::JS) {
            anchor.skipped_trivia(bytes, i, past, lower_bound, grammar);
            i = past;
            continue;
        }
        if bytes[i] == b'/'
            && let Some(past) =
                skip_walked_regex(bytes, i, rparen, &anchor, &headers, lower_bound, grammar)
        {
            anchor.skipped_operand(past);
            headers.skipped_operand();
            i = past;
            continue;
        }
        i = if bytes[i].is_ascii_whitespace() {
            i + 1
        } else {
            headers.step(bytes, i, rparen)
        };
    }
    // Only a walk that landed on the `)` itself read what it closes.
    i == rparen && {
        headers.step(bytes, rparen, rparen + 1);
        headers.after_header_close
    }
}

/// Where a regex literal opening at the `/` at `i` ends, for a resolver's forward walk to step over it
/// whole — so a paren or angle in its body (`if (/\)/.test(s))`) is not tracked as code.
///
/// Only a regex the walk's `anchor` reads there — or one right after a `for` header's `of`
/// (`for (x of /\)/g.exec(s))`), a contextual keyword the anchor's word list cannot hold,
/// since an `of` elsewhere may be a name divided — closed on its line, and ending at or
/// before the walk's `bound`: the anchor asks no type-argument question (a resolver must
/// not recurse into itself), so after a list's `>` it can take a division for a regex's
/// opener, and a span reaching past the `>` or `)` being asked about would hide the very
/// token the walk is looking for.
#[cold]
#[inline(never)]
fn skip_walked_regex(
    bytes: &[u8],
    i: usize,
    bound: usize,
    anchor: &OperandAnchor,
    headers: &StatementHeaders,
    lower_bound: usize,
    grammar: OperandGrammar,
) -> Option<usize> {
    if !(headers.after_for_of || anchor.starts_regex(bytes, i, lower_bound, grammar)) {
        return None;
    }
    skip_regex_literal(bytes, i, bytes.len()).filter(|&past| past <= bound)
}

/// Which open `(` are statement headers' — `if (`, `while (`, `for (`, `for await (`,
/// `with (` — read forward, token by token, by the raw scans that must tell a header's `)`
/// from any other: a header's `)` ends no operand, since a statement begins after it
/// (`if (c) <T>x`, `if (c)!/re/`), where after any other `)` (`f()<T>`, `f()!`) an operand
/// has just ended.
///
/// Read forward, so a comment between a keyword and its `(` (`if /* c */ (`) is stepped
/// over like any other trivia by the walk that feeds it.
#[derive(Default)]
struct StatementHeaders {
    /// Whether each open `(` is a header's, innermost last.
    open: Vec<bool>,
    /// Whether the last significant token closed a header's `(`.
    after_header_close: bool,
    /// The header keyword the last significant token was, so a `(` right after it opens
    /// a header.
    keyword: HeaderKeyword,
    /// Whether the last significant byte was a `.`, which makes the next word a member
    /// name (`a.if (`), never a keyword.
    after_dot: bool,
    /// Whether the last significant token was the `of` of a `for` header (`for (x of`),
    /// after which an expression begins — so a `/` there opens a regex.
    after_for_of: bool,
}

impl StatementHeaders {
    /// A literal the walk stepped over whole, which ends an operand like any word.
    fn skipped_operand(&mut self) {
        self.after_header_close = false;
        self.keyword = HeaderKeyword::None;
        self.after_dot = false;
        self.after_for_of = false;
    }

    /// Read the significant, non-whitespace token at `i` — a whole word, or one byte —
    /// and return where the next one may begin. `end` bounds a word.
    fn step(&mut self, bytes: &[u8], i: usize, end: usize) -> usize {
        if identifier_starts_at(bytes, i) {
            let word_end = skip_identifier(bytes, i).min(end);
            if word_end > i {
                let word = &bytes[i..word_end];
                self.after_for_of =
                    !self.after_dot && word == b"of" && self.open.last() == Some(&true);
                self.keyword = if self.after_dot {
                    HeaderKeyword::None
                } else {
                    HeaderKeyword::of(word, self.keyword)
                };
                self.after_dot = false;
                self.after_header_close = false;
                return word_end;
            }
        }
        let b = bytes[i];
        self.after_for_of = false;
        match b {
            b'(' => {
                self.open.push(self.keyword != HeaderKeyword::None);
                self.after_header_close = false;
            }
            b')' => self.after_header_close = self.open.pop().unwrap_or(false),
            _ => self.after_header_close = false,
        }
        self.keyword = HeaderKeyword::None;
        self.after_dot = b == b'.';
        i + 1
    }
}

/// Whether a word opens a statement header when a `(` follows it — the keyword half of
/// [`StatementHeaders`].
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum HeaderKeyword {
    /// No header keyword: a `(` after it is a call's or a group's.
    #[default]
    None,
    /// `if`, `while`, `with`, or `for await`.
    Header,
    /// `for`, a header keyword itself and the one an `await` may follow.
    For,
}

impl HeaderKeyword {
    /// The keyword `word` is, `previous` being the one before it (`for await`).
    fn of(word: &[u8], previous: Self) -> Self {
        match word {
            b"if" | b"while" | b"with" => Self::Header,
            b"for" => Self::For,
            b"await" if previous == Self::For => Self::Header,
            _ => Self::None,
        }
    }
}

/// Whether the significant byte before `pos` closes a postfix `++` / `--`, looking back
/// over whitespace only — a comment in that gap reads as no update, the operand answer.
fn follows_postfix_update(bytes: &[u8], pos: usize, lower_bound: usize) -> bool {
    let mut j = pos;
    while j > lower_bound && bytes[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    j >= lower_bound + 2 && matches!(&bytes[j - 2..j], b"++" | b"--")
}

/// Whether the type-argument HEAD at `pos` — the first significant byte past a `<` —
/// opens a type-argument list: the whole grade, in one walk.
///
/// The walk reads the list's first argument one OPERAND at a time. Each operand's head is
/// graded by [`type_arg_head_grade`], and the loop here then reads the token past the
/// operand — the follow-token filter every operand shares, so a head whose filter drifted
/// from the others cannot answer the same source two ways. Anything outside the commit
/// set (`?`, arithmetic, a call's `(`, a second literal, …) cannot continue a type, so the
/// `<` is the less-than operator however the bytes past the would-be closing `>` read —
/// which is what keeps ``p < string ? q : r > `t` `` and `x < 1 + 2 > (t, u)` comparisons
/// (matching acorn) even though a template tag or a `(` starts no expression and would
/// otherwise let the closing-`>` scan commit.
///
/// **Three tokens CONTINUE the argument rather than settle the list**, and the walk goes on
/// past each in the same loop — so a union of any length and an index nested to any depth
/// are one stack frame deep:
///
/// - a union or intersection **member** after `|` / `&`. tsc parses every argument with
///   `parseType` (`parseTypeArgumentsInExpression`), and a member whose first token no type
///   starts with ends the union there, so the list is closed by something other than `>`
///   and read as a comparison. So the member is graded by the same head dispatch the first
///   operand was: `f<A[K] | B>(x)` and `f<A | B[K] | C>(x)` commit, while in
///   `a < b[c] | d() > (e)` and `a < b[c] | d + 1 > (e)` the member is an expression, so
///   the `<` is a comparison whatever follows the would-be closing `>`.
/// - an **index** `[`…`]`, which opens a NESTED region ([`Region::Nested`]): its body is an
///   operand run of its own, graded by this same walk until the `]` that closes it, where
///   the indexed access is a complete operand again. `T[K]`-shaped bytes are equally a
///   member access on a comparison's right operand, so only the grade of the body and of
///   what follows the `]` tells them apart: `f<A[B], C>(x)`, `f<A[K] | B>(x)` and
///   `f<A[() => B]>(x)` are instantiations, `f(a < B[c], d)`, `a < B[c] > d` and
///   `f<A[a.b()]>(x)` comparisons.
/// - a **nested argument list** `<`…`>` behind an operand that takes one
///   ([`OperandEnd::Name`]). Nothing else does: `string<C>`, `'a'<C>`, `{}<C>`, `(B)<C>` and
///   `A[K]<C>` are comparisons to tsc and acorn-typescript alike, so a `<` behind any of
///   them refuses the list (`f<A | string<C>>(x)` is `(f < A) | (string < C >> x)`).
///
///   Some operands are neither: behind them the walk keeps the region CLAIMED though tsv
///   has no list to read, because the chain the `<` would make has no printed form every
///   parser reads back as the input — a negative literal ([`OperandEnd::SignedLiteral`]),
///   a nested list's own close ([`OperandEnd::ListClose`]), and an operand heading the
///   region that the printed form spells differently: a paren shell, a name whose list
///   opens past a line break ([`broken_off_head_list_claims`]), a keyword's or a literal's
///   list with a second one behind it. The type parse then rejects the region: a loud
///   error, where the alternative is a chain one parser reads as another program.
///
/// **What ends a region is the region's own.** At the list's level ([`Region::List`]) a
/// `>`, a `,` and a conditional's `extends` are each confirmed by the closing-`>` scan
/// ([`scan_for_closing_angle_bracket`]), which reads the follower too. Inside a nested
/// region the same tokens are read where they stand — a `>` or a `,` there closes no list
/// and separates no arguments, so the body is an expression — and the walk goes on until
/// the region's closer hands it back to the level outside.
///
/// The operand's own end moves under [`TypeArgScan::Relex`]: what the printed form does not
/// hold between the operand and the next token is stepped over first
/// ([`skip_relex_operand_suffixes`]). And inside an index that reading goes no further
/// than the body's first operand — the first token that could continue a type takes the
/// rest of the index as it stands, for the reasons [`Region::Nested`] states.
fn type_arg_head_commits(bytes: &[u8], pos: usize, scan: TypeArgScan) -> bool {
    type_arg_head_walk(bytes, pos, scan, &mut false)
}

/// The walk behind [`type_arg_head_commits`]. `split_unclaimed` is set where the walk
/// ends, inside an index that makes no claim of it, on a token the two parse oracles read
/// differently ([`IndexSplit`]).
///
/// One such token ends no walk where it stands: a GENERIC function head whose parameter
/// list holds a default (`f<A[<T>(a = 1) => B]>(x, y)`, [`HeadGrade::DefaultedPrefix`]).
/// tsc reads a function type through it, leaving the default to its checker, and
/// acorn-typescript an arrow function — the split a plain head's default is
/// ([`IndexSplit`]) — and what follows the `=>` is graded like any function head's
/// return. So it parts the oracles only where the region reads as a list THROUGH it, in
/// an index that opens as no type: there the line is the comparison chain
/// acorn-typescript reads, and the walk ends unclaimed. Where the return is no type both
/// parsers read the chain already and the head decides nothing; where the index opens as
/// a type the region stays claimed, and the type parse rejects it.
fn type_arg_head_walk(
    bytes: &[u8],
    pos: usize,
    scan: TypeArgScan,
    split_unclaimed: &mut bool,
) -> bool {
    let mut through_default = false;
    let commits = type_arg_region_walk(bytes, pos, scan, split_unclaimed, &mut through_default);
    if commits && through_default {
        *split_unclaimed = true;
        return false;
    }
    commits
}

/// [`type_arg_head_walk`]'s walk of the region. `through_default` is set where it passes,
/// inside an index that opens as no type, a generic function head whose parameter list
/// holds a default ([`HeadGrade::DefaultedPrefix`]).
fn type_arg_region_walk(
    bytes: &[u8],
    pos: usize,
    scan: TypeArgScan,
    split_unclaimed: &mut bool,
    through_default: &mut bool,
) -> bool {
    // The nested regions still open, innermost last; empty at the list's own level.
    let mut frames: SmallVec<[Frame; 8]> = SmallVec::new();
    let mut head = pos;
    // Whether the operand being graded HEADS the region: the list's first, ahead of any
    // bar. The one position an operand keeps the claim its printed form would make
    // ([`OperandEnd::RegionHeadShell`], [`broken_off_head_list_claims`]).
    let mut heads_region = true;
    // Whether the walk has passed, at the list's own level, a nested list or an import
    // type — and where the outermost index opened, with that fact as it stood there. Read
    // where an index holds a token the oracles part on ([`IndexSplit`]).
    let mut passed_list_or_import = false;
    let mut outer_index = (0, false);
    // Whether that outermost index stands behind a negative literal (`-1[…]`).
    let mut outer_index_behind_literal = false;
    'operand: loop {
        let region = region_of(&frames);
        let (mut after_operand, mut end) = match type_arg_head_grade(bytes, head, region, scan) {
            HeadGrade::Decided(verdict) => {
                // A nested region is closed by its own delimiter, never by a head.
                debug_assert!(!verdict || region == Region::List);
                if verdict || frames.is_empty() || !scan.grades_index_finely() {
                    return verdict;
                }
                // Inside an index, a head the grade refuses may still be one a parse
                // oracle reads a type through ([`IndexSplit`]).
                let opens_as_type = index_opens_as_type(bytes, outer_index.0, outer_index.1);
                let claimed_from = match IndexSplit::at_head(bytes, head) {
                    Some(IndexSplit::Claimed(token)) if opens_as_type => token,
                    Some(_) => {
                        *split_unclaimed = !opens_as_type;
                        return false;
                    }
                    None => return false,
                };
                let Some(after) = close_nested_region(bytes, claimed_from, &mut frames, scan)
                else {
                    return false;
                };
                (after, OperandEnd::Indexed)
            }
            HeadGrade::Prefix(next) => {
                head = next;
                continue;
            }
            HeadGrade::DefaultedPrefix(next) => {
                // A split only in an index that opens as no type; anywhere else the
                // head is the prefix it looks like.
                if !index_opens_as_type(bytes, outer_index.0, outer_index.1) {
                    *through_default = true;
                }
                head = next;
                continue;
            }
            HeadGrade::Shell(content) => {
                frames.push(Frame::Shell);
                head = content;
                continue;
            }
            HeadGrade::Rest => {
                let Some(after) = close_nested_region(bytes, head, &mut frames, scan) else {
                    return false;
                };
                (after, OperandEnd::Indexed)
            }
            HeadGrade::Operand { end, kind } => (end, kind),
            HeadGrade::Shelled(end) => {
                let kind = if heads_region {
                    OperandEnd::RegionHeadShell
                } else {
                    OperandEnd::Other
                };
                (end, kind)
            }
        };
        if frames.is_empty() && end == OperandEnd::ImportName {
            passed_list_or_import = true;
        }
        // Whether a negative literal took a member tail glued to its digits.
        let literal_has_tail = end == OperandEnd::SignedLiteral
            && after_operand > skip_signed_numeric_literal(bytes, head, scan);
        // A negative literal that took a glued member tail, inside an index that does
        // not open as a type: the member is the postfix the oracles part on, held by
        // the operand itself, and the walk ends on it ([`IndexSplit`]).
        if literal_has_tail
            && !frames.is_empty()
            && scan.grades_index_finely()
            && !index_opens_as_type(bytes, outer_index.0, outer_index.1)
        {
            *split_unclaimed = true;
            return false;
        }

        // The operand is complete; read what follows it until a token opens the next one.
        loop {
            let suffix = skip_relex_operand_suffixes(
                bytes,
                skip_whitespace_and_comments(bytes, after_operand),
                region_of(&frames),
                scan,
            );
            let pos = suffix.token;
            let Some(&byte) = bytes.get(pos) else {
                return false;
            };

            // Whether this is the reading of the printed form, inside an index — which it
            // grades no further than the body's first operand (see above).
            let relex_index = !scan.grades_index_finely() && !frames.is_empty();

            match byte {
                // `||` and `&&` are logical operators, NOT type operators (`a || b`, not args)
                b'|' | b'&' if bytes.get(pos + 1) == Some(&byte) => return false,

                // `<=` is the relational operator, never a list's opener: `a < b <= c >⏎d`
                // is a comparison chain, and reading its `<=` as a `<` would commit the
                // list ahead of the line break and reject the statement.
                b'<' if bytes.get(pos + 1) == Some(&b'=') => return false,

                // [`TypeArgScan::Relex`], inside an index, behind a negative NUMBER: the
                // printer writes the member on a parenthesized literal (`-(1).x`), which
                // is an expression to every parser and no literal type, so nothing is
                // claimed. A BigInt's member has been taken with it
                // ([`negative_literal_end`]).
                b'.' if relex_index && end == OperandEnd::SignedLiteral => return false,

                // [`TypeArgScan::Relex`], inside an index: a token that could continue a
                // type takes the rest of the index as it stands (see above). A `<<` is the
                // one `<` that does not: a shift prints as a shift, which opens no list to
                // tsv's own parse or to acorn-typescript's. (tsc re-scans it into two `<`
                // and claims the region past a line break — one of the index's tsc-only
                // claims, with the `!` and the dynamic import there.)
                b'|' | b'&' | b'[' | b'<' | b'.' if relex_index => {
                    if byte == b'<' && bytes.get(pos + 1) == Some(&b'<') {
                        return false;
                    }
                    let Some(after) = close_nested_region(bytes, pos, &mut frames, scan) else {
                        return false;
                    };
                    after_operand = after;
                    end = OperandEnd::Indexed;
                }

                // Behind a keyword type, a literal, a group or an index the `<` is the
                // comparison operator (see above).
                //
                // One shape keeps the region claimed instead: a SECOND `<` right behind
                // that one's own list, where the operand is no indexed access
                // (`f<string<C><D>, B>(x)`). acorn-typescript reads an instantiation
                // instantiated again there, which prints in a pair of its own —
                // `(string<C>)<D>`. Where the operand heads the region, the printed region
                // then opens on a paren shell, the head the claim below takes whatever it
                // holds ([`OperandEnd::RegionHeadShell`]); and anywhere, a run of lists an
                // operand follows is the spelling tsc reads a type assertion in, whose
                // reading that pair moves ([`operand_follows_list_run`]), as it does
                // behind a name's list. Behind an indexed access at the list's own level
                // the same pair is printed and no claim is made of the source: an
                // index's `<` is no claim there. Inside an index it is one, like the
                // rest (`b[c[K]<D><E> + 1]`, `b[(c)<D><E> + 1]`).
                b'<' if matches!(
                    end,
                    OperandEnd::Keyword | OperandEnd::Other | OperandEnd::Indexed
                ) =>
                {
                    let claimed = (end != OperandEnd::Indexed || !frames.is_empty())
                        && second_list_start(bytes, pos, scan).is_some_and(|second| {
                            (heads_region && frames.is_empty())
                                || operand_follows_list_run(bytes, second, scan)
                        });
                    if !claimed {
                        return false;
                    }
                    if frames.is_empty() {
                        return scan_for_closing_angle_bracket(bytes, pos, scan);
                    }
                    let Some(after) = close_nested_region(bytes, pos, &mut frames, scan) else {
                        return false;
                    };
                    after_operand = after;
                    end = OperandEnd::Indexed;
                }

                // A paren shell heading the region ([`OperandEnd::RegionHeadShell`]): the
                // source's `<` is a comparison, and what the printed form reads there is
                // not this walk's to say, so the region keeps the claim of its own
                // closing-`>` scan, whatever follows the `<`.
                b'<' if end == OperandEnd::RegionHeadShell => {
                    return scan_for_closing_angle_bracket(bytes, pos, scan);
                }

                // A nested argument list (`<A<B>>`, `<A<B>[]>`, `<A[B<C>]>`): stepped over to
                // its own `>`, past which the operand is complete again. What follows it is
                // then read like any other operand's follow token, so `f<A | B<C>>(x)`
                // commits at the `>` that closes the outer list while `a < b | c<d>(e) > (f)`
                // — a generic CALL, which no type spells — stays the comparison both oracles
                // read. The list's own arguments are not graded, only its delimiters matched
                // ([`matching_angle_close`]).
                //
                // A `<` past a line terminator opens no list of the name's: the type
                // grammar takes a reference's arguments on the name's own line (tsc and
                // acorn-typescript alike), as it takes an index's `[` — and so does
                // [`Parser::parse_type`]. So the source's `<` is a comparison, or the
                // opener of an instantiation EXPRESSION's list, which a break does not
                // refuse (`f<A[B⏎<C>]>(x)` compares `f` with `A[B<C>]`). The printer folds
                // that break, which is why a name HEADING the region keeps a claim there,
                // its printed form being a bare name with a list
                // ([`broken_off_head_list_claims`]) — and why the reading of the printed
                // form passes the gate, except where a comment carries the break into the
                // output ([`list_opens_past_a_break`]). A SECOND list behind the first
                // takes no gate: it is the claim it is behind any other first list.
                //
                // An IMPORT type is the one name the oracles part on: acorn-typescript
                // takes its list across the break, where tsc ends the type
                // (`f<A | import('m').B⏎<C>>(x)` is a generic call to the first and a
                // chain to the second). That name takes no gate, so the region stays
                // claimed and the type parse — which holds tsc's line rule — rejects it.
                //
                // A type QUERY of an import takes a second list, its own, behind the
                // import type's ([`OperandEnd::ImportQuery`]): inside an index the
                // operand behind the first is a type name again, where any other list's
                // close is read for what a SECOND list would claim. At the list's own
                // level the closing-`>` scan answers for that second list as it stands.
                b'<' if matches!(
                    end,
                    OperandEnd::Name | OperandEnd::ImportName | OperandEnd::ImportQuery
                ) =>
                {
                    if matches!(end, OperandEnd::Name | OperandEnd::ImportName)
                        && list_opens_past_a_break(bytes, after_operand, pos, scan)
                        && second_list_start(bytes, pos, scan).is_none()
                    {
                        let word = &bytes[head..after_operand];
                        if end == OperandEnd::Name
                            && matches!(word, b"async" | b"await")
                            && !list_opens_past_a_break(
                                bytes,
                                after_operand,
                                pos,
                                TypeArgScan::Relex,
                            )
                        {
                            // No name the break stands a list off from: the printer folds
                            // a break no comment carries, and `async<C>(e)` is then a
                            // generic arrow function's head, `await <C>…` an operand of
                            // its own. The region is taken as it stands and the type
                            // parse rejects it.
                            if frames.is_empty() {
                                return scan_for_closing_angle_bracket(bytes, pos, scan);
                            }
                            let Some(after) = close_nested_region(bytes, pos, &mut frames, scan)
                            else {
                                return false;
                            };
                            after_operand = after;
                            end = OperandEnd::Indexed;
                            continue;
                        }
                        // An import type's list is acorn-typescript's across the break
                        // (above) — except inside the index of a negative literal, which
                        // that parser reads as an expression and welds no list in.
                        if end == OperandEnd::Name
                            || (outer_index_behind_literal && !frames.is_empty())
                        {
                            return heads_region
                                && frames.is_empty()
                                && broken_off_head_list_claims(bytes, pos, scan);
                        }
                    }
                    let Some(close) = matching_angle_close(bytes, pos + 1, scan) else {
                        return false;
                    };
                    if frames.is_empty() {
                        passed_list_or_import = true;
                    }
                    // An import with OPTIONS is no import type to acorn-typescript, which
                    // reads a query over one, with two lists behind it, as an expression;
                    // tsv's type parser takes both. So that spelling follows the index
                    // like the other tokens the parsers part on ([`IndexSplit`]): a type
                    // where the index opens as one, a comparison anywhere else.
                    if end == OperandEnd::ImportQuery
                        && !frames.is_empty()
                        && scan.grades_index_finely()
                        && second_list_start(bytes, pos, scan).is_some()
                        && type_query_import_takes_options(bytes, head)
                        && !index_opens_as_type(bytes, outer_index.0, outer_index.1)
                    {
                        return false;
                    }
                    after_operand = close + 1;
                    end = if end == OperandEnd::ImportQuery && !frames.is_empty() {
                        OperandEnd::Name
                    } else {
                        OperandEnd::ListClose
                    };
                }

                // A negative literal whose list would close INTO a shift (`-1<C>>`): the
                // `>>` is one token to every parser, so no list closed there and the `<` is
                // the comparison operator tsc and acorn-typescript both read.
                b'<' if end == OperandEnd::SignedLiteral
                    && matching_angle_close(bytes, pos + 1, scan)
                        .is_some_and(|close| bytes.get(close + 1) == Some(&b'>')) =>
                {
                    return false;
                }

                // A `<` behind one of the two CLAIMED operands — a negative literal
                // ([`OperandEnd::SignedLiteral`]) or a nested list's own close
                // ([`OperandEnd::ListClose`]): the region is taken as it stands, to its
                // closing `>` at the list's own level and to the index's `]` inside one,
                // and the type parse then rejects it.
                //
                // Inside an index two spellings are no claim. A `<<` is the shift it looks
                // like, which opens nothing behind either operand. And a second list is a
                // claim only where an OPERAND follows it for tsc's reading — a type
                // assertion over what follows the list — to take (`b[A<B><C>[0]]`,
                // `b[A<B><C> + 1]`): there the chain acorn-typescript reads prints with a
                // pair that moves the compiler's reading. Where none does — the list, or
                // the run of lists it opens, ends the body (`b[A<B><C>]`), or a bar or a
                // comma follows it — the compiler rejects the spelling and the
                // instantiation acorn-typescript reads is the only reading there is; and
                // ahead of a call's arguments or a template (`b[c<D><E>(e)]`,
                // `b[() => c<D><E>(e)]`) both read an expression the printer writes as
                // it stands, so no pair moves either one ([`operand_follows_list_run`]).
                //
                // Behind a NEGATIVE LITERAL the same test reads the other way. The claim
                // there guards the list acorn-typescript's expression parser takes behind
                // the digits, which it takes only where an instantiation may stand — and
                // none may ahead of an operand (`b[() => -1<C>[]]`, `b[A | -1<C> + 1]`),
                // where both parsers read the comparison `(-1 < C) > []`.
                b'<' => {
                    debug_assert!(matches!(
                        end,
                        OperandEnd::SignedLiteral | OperandEnd::ListClose
                    ));
                    if frames.is_empty() {
                        return scan_for_closing_angle_bracket(bytes, pos, scan);
                    }
                    if bytes.get(pos + 1) == Some(&b'<') {
                        return false;
                    }
                    // A negative literal's list past a line break is no list of the
                    // literal's to acorn-typescript's expression parser either, so the
                    // `<` is the comparison both parsers read — outside an index that
                    // opens as a type, which stays claimed like the rest of its kind.
                    if end == OperandEnd::SignedLiteral
                        && scan.reads_source_as_written()
                        && has_line_terminator_between(bytes, after_operand, pos)
                        && !index_opens_as_type(bytes, outer_index.0, outer_index.1)
                    {
                        return false;
                    }
                    let operand_follows = operand_follows_list_run(bytes, pos, scan);
                    if operand_follows == (end == OperandEnd::SignedLiteral) {
                        return false;
                    }
                    let Some(after) = close_nested_region(bytes, pos, &mut frames, scan) else {
                        return false;
                    };
                    after_operand = after;
                    end = OperandEnd::Indexed;
                }

                // A union or intersection member, graded as a type (see above).
                b'|' | b'&' => {
                    head = skip_whitespace_and_comments(bytes, pos + 1);
                    if head >= bytes.len() {
                        return false;
                    }
                    if frames.is_empty() {
                        heads_region = false;
                    }
                    continue 'operand;
                }

                // At the list's own level a `>` closes the list and a `,` separates its
                // arguments. Each is confirmed by scanning for the matching `>` — which
                // rejects a trailing identifier, so `a < b > c` stays a comparison. (`,` is
                // neutral to the scan, so starting at `pos` is equivalent to starting past
                // the separator.) Inside a nested region a `>` closes nothing and a `,`
                // separates nothing: the body is a comparison or a sequence, never a type.
                b'>' | b',' => {
                    return frames.is_empty() && scan_for_closing_angle_bracket(bytes, pos, scan);
                }

                // Indexed type vs array access: `T[K]` vs `arr[0]`. An empty pair is an
                // array type's suffix and completes the operand as it stands; anything else
                // opens a nested region, whose body the walk grades next.
                //
                // A `[` past a line terminator is no index at all: the type grammar takes its
                // postfix operators on the operand's own line (tsc and acorn-typescript alike —
                // the loop in each is named in `docs/conformance_prettier_ts.md` §Relational
                // chain type-argument parens), and so does [`Parser::parse_type`]. Committing to
                // type arguments here would hand that parser a `<B⏎[c]>` it stops reading at
                // `B` — `a <⏎B // c⏎[c] >⏎d` is the comparison the same bytes on one line are.
                //
                // That break is exactly what the PRINTER folds away, which is why the
                // [`TypeArgScan::Relex`] reading passes the gate: `fn<A⏎[T]>(t, u)` parses as a
                // comparison chain and prints as `fn < A[T] > (t, u)`, whose region is a
                // type-argument list. See [`TypeArgScan`].
                b'[' => {
                    if scan.reads_source_as_written()
                        && has_line_terminator_between(bytes, after_operand, pos)
                    {
                        return false;
                    }
                    let inside = skip_whitespace_and_comments(bytes, pos + 1);
                    if bytes.get(inside) == Some(&b']') {
                        after_operand = inside + 1;
                        end = OperandEnd::Indexed;
                    } else if inside < bytes.len() {
                        if frames.is_empty() {
                            outer_index = (pos, passed_list_or_import);
                            outer_index_behind_literal = end == OperandEnd::SignedLiteral;
                        }
                        frames.push(Frame::Index);
                        head = inside;
                        continue 'operand;
                    } else {
                        return false;
                    }
                }

                // The nested region's own closer: the indexed access, or the parenthesized
                // type, is a complete operand of the region outside — one that takes no
                // argument list (see above).
                b']' if frames.last() == Some(&Frame::Index) => {
                    frames.pop();
                    after_operand = pos + 1;
                    end = OperandEnd::Indexed;
                }
                b')' if frames.last() == Some(&Frame::Shell) => {
                    frames.pop();
                    after_operand = pos + 1;
                    end = OperandEnd::Indexed;
                }

                // A qualified name's next segment behind a shell the printer strips
                // ([`OperandSuffix::shell_only`], [`TypeArgScan::Relex`] alone): `(b).c`
                // prints as `b.c`, a type reference like any other.
                b'.' if suffix.shell_only && end.heads_qualified_name() => {
                    let tail = skip_qualified_tail(bytes, pos);
                    if tail == pos {
                        return false;
                    }
                    after_operand = tail;
                    end = if end == OperandEnd::Keyword {
                        OperandEnd::Name
                    } else {
                        end
                    };
                }

                // A conditional type's constraint (`T extends U ? A : B`), or a type
                // predicate's `is` (`(a) => a is T`, `this is T`). Whole-word — an
                // identifier that merely starts with one is an ordinary operand
                // (`a < b` ⏎ `extendsFoo()`, where ASI ends the statement) — and `is`, an
                // ordinary name where `extends` is reserved, only on the operand's own
                // line, as the type grammar takes it: past a break it begins the next
                // statement (`a < b` ⏎ `is > (c)`). Neither word continues an EXPRESSION
                // past an operand, so everything behind one to the region's end is the
                // type's: at the list's level the closing-`>` scan confirms it like every
                // sibling arm, and a nested region is stepped over to its own closer.
                b'e' | b'i'
                    if is_word_at(bytes, pos, b"extends")
                        || (is_word_at(bytes, pos, b"is")
                            && !has_line_terminator_between(bytes, after_operand, pos)) =>
                {
                    if frames.is_empty() {
                        return scan_for_closing_angle_bracket(bytes, pos, scan);
                    }
                    let Some(after) = close_nested_region(bytes, pos, &mut frames, scan) else {
                        return false;
                    };
                    after_operand = after;
                    end = OperandEnd::Indexed;
                }

                // A negative literal's postfix inside an index ([`IndexSplit`]): claimed
                // where the index opens as a type — the region the literal stands in is
                // taken as it stands, and the type parse rejects it — and the end of the
                // walk anywhere else, where the line is a comparison chain.
                _ if end == OperandEnd::SignedLiteral
                    && scan.grades_index_finely()
                    && !frames.is_empty()
                    && literal_postfix_starts_at(bytes, pos, literal_has_tail) =>
                {
                    if !index_opens_as_type(bytes, outer_index.0, outer_index.1) {
                        *split_unclaimed = true;
                        return false;
                    }
                    let Some(after) = close_nested_region(bytes, pos, &mut frames, scan) else {
                        return false;
                    };
                    after_operand = after;
                    end = OperandEnd::Indexed;
                }

                // A non-null `!` behind any other operand inside an index: tsc's JSDoc
                // non-nullable type, which acorn-typescript and this parse read as the
                // expression's own. No claim is made of it; the walk ends, and says so
                // where the index does not open as a type ([`IndexSplit`]).
                b'!' if scan.grades_index_finely()
                    && !frames.is_empty()
                    && bytes.get(pos + 1) != Some(&b'=') =>
                {
                    *split_unclaimed = !index_opens_as_type(bytes, outer_index.0, outer_index.1);
                    return false;
                }

                _ => return false,
            }
        }
    }
}

/// A token inside an index that ONE parse oracle reads a type through, where the other —
/// and tsv's own parse — reads an expression:
///
/// - **a negative literal's postfix** — a call's arguments, a template, a member (glued to
///   the digits or spaced off), a non-null `!` (`-1(e)`, ``-1`t` ``, `-1 .x`, `-1..x`,
///   `-1!`). acorn-typescript reads a negative literal type with its EXPRESSION parser,
///   which takes each behind the digits, so the index is a type to it. The printer's own
///   spellings of the same postfix are read with it: the member it writes on a
///   parenthesized literal (`-(1).x`), and the literal it makes by stripping a shell
///   (`-(1)(e)` prints as `-1(e)`);
/// - **a meta-property**, `new.target`, which acorn-typescript reads a type reference
///   named so in, and tsc rejects;
/// - **an arrow function whose parameter list holds a default** of a parameter's own
///   (`(a = 1) => 0`, [`parameter_list_holds_default`]), which tsc reads a function type
///   in, leaving the default to its checker, and acorn-typescript an arrow function. A
///   GENERIC head's default is the same split, read where the region runs on as a list
///   through it ([`type_arg_head_walk`]);
/// - **a non-null `!`** on any other operand, tsc's JSDoc non-nullable type; and a
///   **commented sign** (`- /* c */ 1`, printed as `-(/* c */ 1)`), a negative literal type
///   in the source and an expression in the printed form.
///
/// tsv has neither tree to give for the first two, and what it does turns on the INDEX
/// ([`index_opens_as_type`]): where the body opens as a type, the region the token stands
/// in is taken as it stands and the type parse rejects it; anywhere else the walk ends
/// there and the line is the comparison chain, printed with no pair the source did not
/// have — a pair reads as a comparison to the oracle that read a generic call
/// (`Parser::chain_owes_type_argument_pair`). The last kind is never claimed, and marks
/// the same end.
///
/// A member tail GLUED to the digits is taken with the literal where the index opens as
/// a type or the literal stands at the list's own level ([`skip_glued_member_tail`]), so
/// a region that reaches its close there stays claimed without this.
// TODO: one rule should answer for a negative literal's postfix wherever it stands — the
// question [`skip_glued_member_tail`] leaves open. Until it is settled the claim keeps
// to the indexes [`index_opens_as_type`] names and reaches no further.
#[derive(Clone, Copy)]
enum IndexSplit {
    /// Claimed where the index opens as a type, from the token at this offset.
    Claimed(usize),
    /// Never claimed: the walk ends on it.
    Unclaimed,
}

impl IndexSplit {
    /// The split a HEAD the grade refused spells, if any.
    fn at_head(bytes: &[u8], head: usize) -> Option<Self> {
        match bytes.get(head)? {
            b'-' => match shelled_negative_number_end(bytes, head) {
                // A stripped shell makes the literal: a type where nothing the oracles
                // part on follows it, a claim ahead of a call or a template, and — the
                // shell the printer keeps — an expression ahead of a member.
                Some(shell_end) => {
                    let token = skip_whitespace_and_comments(bytes, shell_end);
                    literal_postfix_starts_at(bytes, token, false).then(|| {
                        if matches!(bytes[token], b'.' | b'?') {
                            Self::Unclaimed
                        } else {
                            Self::Claimed(token)
                        }
                    })
                }
                // A commented sign, in the source's spelling or in the pair the printer
                // writes it in (`- /* c */ 1`, `-(/* c */ 1)`).
                None => {
                    let inner = skip_stripped_shells(bytes, head + 1);
                    let digits = skip_whitespace_and_comments(bytes, inner);
                    (digits > inner && numeric_literal_starts_at(bytes, digits))
                        .then_some(Self::Unclaimed)
                }
            },
            // An arrow function whose parameter list holds a default.
            b'(' if paren_list_arrow_end(bytes, head).is_some()
                && parameter_list_holds_default(bytes, head) =>
            {
                Some(Self::Claimed(head))
            }
            b'!' if bytes.get(head + 1) != Some(&b'=') => Some(Self::Unclaimed),
            b'n' if is_word_at(bytes, head, b"new")
                && bytes.get(skip_whitespace_and_comments(bytes, head + 3)) == Some(&b'.') =>
            {
                Some(Self::Claimed(head))
            }
            _ => None,
        }
    }
}

/// Whether a postfix acorn-typescript's expression parser takes behind a negative literal
/// starts at `pos`, the first token past the literal: a call's arguments, a template, a
/// member's `.` or `?.`, a non-null `!` — or, behind a literal that took a glued member
/// tail (`has_tail`), the NAME that tail's byte run stops short of (`-1..$x`,
/// [`skip_glued_member_tail`]). A conditional's `?` and a `!=` are operators, not
/// postfixes.
fn literal_postfix_starts_at(bytes: &[u8], pos: usize, has_tail: bool) -> bool {
    match bytes.get(pos) {
        Some(b'(' | b'`' | b'.') => true,
        Some(b'?') => {
            bytes.get(pos + 1) == Some(&b'.') && !bytes.get(pos + 2).is_some_and(u8::is_ascii_digit)
        }
        Some(b'!') => bytes.get(pos + 1) != Some(&b'='),
        Some(_) => has_tail && bytes[..pos].ends_with(b".") && identifier_starts_at(bytes, pos),
        None => false,
    }
}

/// The end of a negative number that stands inside a paren shell of its own at `head` —
/// `-(1)`, `-((1))` — just past the last `)`; `None` for anything else, a shell that
/// holds a comment included (the printer keeps that pair).
fn shelled_negative_number_end(bytes: &[u8], head: usize) -> Option<usize> {
    if bytes.get(head) != Some(&b'-') {
        return None;
    }
    let mut pos = skip_whitespace(bytes, head + 1);
    let mut shells = 0usize;
    while bytes.get(pos) == Some(&b'(') {
        shells += 1;
        pos = skip_whitespace(bytes, pos + 1);
    }
    let end = skip_numeric_literal(bytes, pos);
    if shells == 0 || end == pos {
        return None;
    }
    pos = end;
    for _ in 0..shells {
        pos = skip_whitespace(bytes, pos);
        if bytes.get(pos) != Some(&b')') {
            return None;
        }
        pos += 1;
    }
    Some(pos)
}

/// Whether the index that opens at `open` — the outermost one a walk is inside of —
/// OPENS AS A TYPE: read on the body's first operand and the one token behind it
/// ([`index_operand_opens_type`]), or outright where `behind_list_or_import` says the
/// region holds, ahead of that index and at the list's own level, a nested argument list
/// or an import type — a region the closing-`>` scan answers for whatever it holds.
///
/// It decides what a token the oracles part on does inside the index ([`IndexSplit`]):
/// `f<A[B | -1(e)]>(x)`, ``f<A[A[-1`t`]]>(x)`` and `f<A[-1 .x]>(x)` are rejected, and
/// `f<A[-1(e)]>(x)`, `f<A[() => B | -1(e)]>(x)` and `f<A[{} | -1(e)]>(x)` are comparison
/// chains. Which half a spelling falls in is the body's first operand and nothing about
/// the token.
fn index_opens_as_type(bytes: &[u8], open: usize, behind_list_or_import: bool) -> bool {
    behind_list_or_import
        || index_operand_opens_type(bytes, skip_whitespace_and_comments(bytes, open + 1))
}

/// Whether the operand at `inside` — the first of an index's body, or of a paren shell
/// heading one — opens a type, on the source as written:
///
/// - a string or template key, a `keyof` or a `typeof`: outright;
/// - a name or a numeric literal: where a type-continuation token follows it
///   ([`type_continues_past`]) — the literal read with any member tail glued to its
///   digits ([`skip_glued_member_tail`]), so `A[-1..x()]` opens as none;
/// - a paren shell: where its content does by this rule and a type continues past its
///   `)`, which an arrow function's parameter list does not (`A[(a) => -1(e)]`);
/// - anything else — an object or array literal, a unary operator — never.
///
/// Asked only of an index the walk in [`type_arg_head_commits`] has graded as far as a
/// negative literal, which is why it reads so little: a token that walk refuses on the
/// way (a `||`, a `[` past a line break), or takes the rest of the index at (a leading
/// bar, an `extends`), needs no arm here.
fn index_operand_opens_type(bytes: &[u8], inside: usize) -> bool {
    // The run of shells at the head, and the operand inside the innermost.
    let mut content = inside;
    let mut shells = 0usize;
    while bytes.get(content) == Some(&b'(') {
        shells += 1;
        content = skip_whitespace_and_comments(bytes, content + 1);
    }
    let Some(&first) = bytes.get(content) else {
        return false;
    };
    let opens = match first {
        b'\'' | b'"' | b'`' => true,
        _ if numeric_literal_starts_at(bytes, content) => {
            let after = skip_signed_numeric_literal(bytes, content, TypeArgScan::Parse);
            after != content && type_continues_past(bytes, skip_glued_member_tail(bytes, after))
        }
        _ if identifier_starts_at(bytes, content) => {
            let after = skip_identifier(bytes, content);
            matches!(&bytes[content..after], b"keyof" | b"typeof")
                || type_continues_past(bytes, after)
        }
        _ => false,
    };
    if !opens || shells == 0 {
        return opens;
    }
    // Each shell's own `)`, innermost first, in one forward pass: a type has to continue
    // past every one of them.
    let mut pos = content;
    while pos < bytes.len() {
        if let Some(past) = skip_trivia(bytes, pos, bytes.len(), TriviaProfile::JS) {
            pos = past;
            continue;
        }
        match bytes[pos] {
            b'(' | b'[' | b'{' => match matching_delimiter_close(bytes, pos) {
                Some(close) => pos = close + 1,
                None => return false,
            },
            b')' => {
                if !type_continues_past(bytes, pos + 1) {
                    return false;
                }
                shells -= 1;
                if shells == 0 {
                    return true;
                }
                pos += 1;
            }
            b']' | b'}' => return false,
            _ => pos += 1,
        }
    }
    false
}

/// Whether a type continues past the operand that ends at `operand_end`: on the closer of
/// the region it stands in, a `|` or `&`, a `.`, a `[`, or a `<` that is no shift or `<=`.
fn type_continues_past(bytes: &[u8], operand_end: usize) -> bool {
    let pos = skip_whitespace_and_comments(bytes, operand_end);
    match bytes.get(pos) {
        Some(b']' | b')' | b'|' | b'&' | b'.' | b'[') => true,
        Some(b'<') => !matches!(bytes.get(pos + 1), Some(b'<' | b'=')),
        _ => false,
    }
}

/// Step over the rest of the innermost nested region as it stands — from `pos`, a position
/// inside it, to its closer ([`nested_region_close`]) — and close it: the offset past the
/// closer, where the region is a complete operand of the one outside. `None` where there is
/// no such region, or no closer.
fn close_nested_region(
    bytes: &[u8],
    pos: usize,
    frames: &mut SmallVec<[Frame; 8]>,
    scan: TypeArgScan,
) -> Option<usize> {
    let close = nested_region_close(bytes, pos, *frames.last()?, scan)?;
    frames.pop();
    Some(close + 1)
}

/// Whether the region keeps its claim at the `<` at `lt`, which stands behind a NAME that
/// heads the region and whose `<` the source puts past a line break. The source's `<` is
/// a comparison, or opens an instantiation expression's list, to tsc and to
/// acorn-typescript alike; the printer folds the break, and behind the bare name the
/// printed `<` opens the name's own list — so the printed region runs on to its own close
/// wherever no pair of the printer's stands in between. `f<B⏎<C>>(x)` would print as
/// `f < B < C >> x`, whose two lists close on one shift token, and `f<B⏎<C>, D>(x, y)` as
/// a chain whose first `<` reads on past the comma to the later `>` — each a generic
/// call, where the source compared. The printer's pairs stand at a `>` operator and at a
/// bar, and a region that closes on a shift or across a comma takes neither.
///
/// So the region stays claimed by its own closing-`>` scan, and the type parse rejects
/// it: a loud error, and an over-rejection of a chain both parsers read. One follower
/// ends the claim, a BAR behind the list — the chain's `<` is then the bar's left
/// operand, which always prints in a pair of its own (`f<B⏎<C> | D>(x)` prints as
/// `(f < B<C>) | (D > x)`), and no reading of the printed form runs past that `)`.
///
/// A paren SHELL heading the region has the same printed form where the printer strips
/// it (`f<(A)<C>>(x)`), and makes the same claim with no exception at all
/// ([`OperandEnd::RegionHeadShell`]): what a shell holds decides whether the printer
/// strips it and what the bare content then reads as, which is not this scan's to say.
// TODO: the claim stands in for a pair the printer lacks. A region that closes on a shift
// token, or on a later argument's `>`, takes none, so `(f < A) < C >> `t`` prints bare and
// reads back as a tagged template; a pair around the outer `<`'s left operand would end
// the region, and both claims could then read the chain both oracles do.
fn broken_off_head_list_claims(bytes: &[u8], lt: usize, scan: TypeArgScan) -> bool {
    let Some(close) = matching_angle_close(bytes, lt + 1, scan) else {
        return false;
    };
    let follower = skip_whitespace_and_comments(bytes, close + 1);
    // A single bar: `||` / `&&` and `|=` / `&=` are other operators, with no pair of
    // their own around the `<`.
    let bar_follows = matches!(bytes.get(follower), Some(b'|' | b'&'))
        && !matches!(bytes.get(follower + 1), Some(b'|' | b'&' | b'='));
    !bar_follows && scan_for_closing_angle_bracket(bytes, lt, scan)
}

/// The second `<` standing right behind the list the `<` at `lt` opens, if one does —
/// `X<A><B`, the spelling of an instantiation instantiated again.
fn second_list_start(bytes: &[u8], lt: usize, scan: TypeArgScan) -> Option<usize> {
    let close = matching_angle_close(bytes, lt + 1, scan)?;
    let follower = skip_whitespace_and_comments(bytes, close + 1);
    (bytes.get(follower) == Some(&b'<') && bytes.get(follower + 1) != Some(&b'='))
        .then_some(follower)
}

/// Whether an OPERAND follows the run of consecutive lists that opens at the `<` at `lt`
/// (`<A><B>[0]`, `<A> + 1`) — a token that can start an expression, other than a call's
/// `(` or a template. That is where tsc reads the run's last list as a type ASSERTION
/// over the operand, and where the chain acorn-typescript reads prints with a pair that
/// moves that reading; a call's arguments and a template are operands to the compiler
/// too, but the call prints as written.
fn operand_follows_list_run(bytes: &[u8], lt: usize, scan: TypeArgScan) -> bool {
    let mut lt = lt;
    loop {
        let Some(close) = matching_angle_close(bytes, lt + 1, scan) else {
            return false;
        };
        let follower = skip_whitespace_and_comments(bytes, close + 1);
        if follower >= bytes.len() {
            return false;
        }
        if bytes[follower] == b'<' && !matches!(bytes.get(follower + 1), Some(b'<' | b'=')) {
            lt = follower;
            continue;
        }
        return starts_expression_after_type_args(bytes, follower);
    }
}

/// Whether a name's `<` at `lt` stands past a line break from the name, which ends at
/// `name_end` — in the text `scan` grades.
///
/// The source's own bytes answer for [`TypeArgScan::Parse`]. The printed form folds a
/// plain break onto the name's line, so [`TypeArgScan::Relex`] reads one only where a
/// COMMENT carries it into the output: a `//` comment, which ends its line, or a block
/// comment spanning one (`B // c⏎<C>`, `B /* c⏎ */ <C>` print with the break in place).
fn list_opens_past_a_break(bytes: &[u8], name_end: usize, lt: usize, scan: TypeArgScan) -> bool {
    if scan.reads_source_as_written() {
        return has_line_terminator_between(bytes, name_end, lt);
    }
    // The gap holds whitespace, comments, and what this reading steps over with them (a
    // stripped shell's `)`), so every other byte is passed by.
    let mut pos = name_end;
    while pos < lt {
        match skip_trivia(bytes, pos, lt, TriviaProfile::JS) {
            Some(past) => {
                if has_line_terminator_between(bytes, pos, past) {
                    return true;
                }
                pos = past;
            }
            None => pos += 1,
        }
    }
    false
}

/// The region an operand at the walk's current depth stands in.
#[inline]
fn region_of(frames: &[Frame]) -> Region {
    if frames.is_empty() {
        Region::List
    } else {
        Region::Nested
    }
}

/// A region the walk in [`type_arg_head_commits`] is inside of, named by what closes it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Frame {
    /// An index's `[`…`]`.
    Index,
    /// A paren shell's `(`…`)` inside an index ([`HeadGrade::Shell`]).
    Shell,
}

/// Which kind of region an operand's head is graded in — the one fact about where the
/// operand STANDS that [`type_arg_head_grade`] turns on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Region {
    /// The type-argument list's own level. A head that runs to the end of its region — a
    /// function type's return, a `keyof`'s operand, a leading-bar union — is settled by
    /// the closing-`>` scan, which finds that end and reads the follower past it.
    List,
    /// Inside an index, or a paren shell within one. The region ends at a delimiter and
    /// not at a `>`, so no closing scan can answer for it: a head that runs to its end is
    /// a [`HeadGrade::Prefix`], and the walk grades the rest operand by operand.
    ///
    /// **A nested region is an EXPRESSION position too**, which the list's own level is
    /// not: an index takes any expression, so `b[() => c + 1]`, `b[(c, d)]` and
    /// `b[keyof(c)]` are each as well-formed as `B[() => C]`, and only the grade tells
    /// them apart. That is why no arm may commit here on its first token the way the
    /// list-level function-type head does.
    ///
    /// **The reading of the printed form grades this region COARSELY**
    /// ([`Region::is_coarse_under`]): the head of its first operand, and then the first
    /// token that could continue a type — a `.`, a `|` or `&`, a `[`, a `<`, an `extends` —
    /// takes the rest of the index as it stands ([`HeadGrade::Rest`]), as a string or
    /// template key and a `keyof` or `typeof` do outright. Two things make the finer grade
    /// the parse takes the wrong instrument for an OUTPUT:
    ///
    /// - **The printer moves the pairs it would have to read.** The operands of a
    ///   comparison in the body take whichever pair their position asks for
    ///   (`b[B<C>[]]` prints as `b[(B < C) > []]`, `b[(B)<C | d()>[]]` as
    ///   `b[(B < C) | (d() > [])]`, `b[B<C, D>[]]` as `b[(B < C, D > [])]`), and those
    ///   pairs move the delimiters a nested list is matched by — the bare reading closes
    ///   one in the last of the three alone, and no reading of the source can tell.
    /// - **tsc claims more than a type.** Its list parse is error-recovering, so a body
    ///   that merely STARTS like a type is a region it claims past a line break and then
    ///   rejects (`b[c.d!]`, `b[keyof]`, `b[import.meta]`, `b[c | (d, e)]`,
    ///   `b[c[(d, e)]]`), where acorn-typescript and tsv's own parse read the chain.
    ///
    /// A pair the chain did not need is noise; a bare chain that a reader of the output
    /// then claims is a different program, or none. So the coarse reading errs toward the
    /// pair, and — reading nothing past the first operand — reaches one verdict on both
    /// spellings of a body by construction.
    Nested,
}

impl Region {
    /// Whether an operand in this region is graded coarsely under `scan` — inside an
    /// index, by the reading of the printed form ([`Region::Nested`]).
    #[inline]
    fn is_coarse_under(self, scan: TypeArgScan) -> bool {
        self == Region::Nested && !scan.grades_index_finely()
    }
}

/// How an operand ends — the one fact of the operand BEHIND it that a follow token's
/// reading may turn on ([`type_arg_head_commits`]).
#[derive(Clone, Copy, PartialEq, Eq)]
enum OperandEnd {
    /// A type NAME: a reference (`A`), a qualified name (`Ns.A`, `string.x`), a type query
    /// (`typeof x`), or one behind `readonly`. With the two import kinds below, the only
    /// operand a `<` opens an argument list of its own behind.
    Name,
    /// An import type (`import('m').A`): a type name like any other, apart from the one
    /// place the two parse oracles read it differently — a `<` past a line break, which
    /// acorn-typescript takes for its list and tsc does not.
    ImportName,
    /// A type QUERY of an import (`typeof import('m')`, `typeof import('m').A`). It reads
    /// as [`OperandEnd::ImportName`] does at a `<`, and takes TWO lists where that takes
    /// one: the import type's own, then the query's (`typeof import('m')<C><D>`). It is
    /// no import-type HEAD, though — the region is not one the closing-`>` scan answers
    /// for outright ([`index_opens_as_type`]).
    ImportQuery,
    /// A paren shell HEADING the region (`f<(A)…`, `f<((typeof a))…`, `f<(A | B)…`).
    ///
    /// To the source it is a parenthesized type, which takes no argument list, so a `<`
    /// behind one is a comparison to tsc and to acorn-typescript alike — and behind a bar
    /// that is all it is, since the chain's own pair ends the region there
    /// (`f<A | (B)<C>>(x)` prints as `(f < A) | (B < C >> x)`). At the head no pair does.
    /// Around a name the printer strips the shell — a name it may itself make by
    /// stripping a shell inside (`(typeof (b))`, `((b).c)`) — and the printed `<` then
    /// opens the bare name's own list, which runs on to the region's close
    /// ([`broken_off_head_list_claims`] names the shapes); around other contents it keeps
    /// the shell, or strips it from a head that claims the region by itself (`(-1)`,
    /// `(keyof)`, `(import(c))`).
    ///
    /// Deciding which contents print as what would mean reading the printer's paren
    /// rules from here, and a region read wrongly is a chain one parser reads as another
    /// program. So a `<` behind the shell keeps the region claimed by its own closing-`>`
    /// scan, WHATEVER the shell holds and whatever follows the `<`, and the type parse
    /// rejects it — a loud error, and for many contents an over-rejection of a chain both
    /// parsers read.
    RegionHeadShell,
    /// A bare keyword type (`string`, `null`, `void`, …). It takes no argument list — the
    /// type grammar reads the keyword, not a reference — but as an identifier token it still
    /// heads a qualified name where a `.` follows, which one reading has to know
    /// ([`OperandEnd::heads_qualified_name`]).
    Keyword,
    /// A NEGATIVE numeric literal (`-1`), the one operand the two parse oracles read past
    /// differently. acorn-typescript parses it with its EXPRESSION parser, which takes an
    /// instantiation's argument list behind the digits wherever an instantiation may stand
    /// — ahead of a call's `(`, a template, a bar — so `f<-1<C>(e)>(x)` and
    /// `f<-1<C> | D>(x)` are generic calls over the literal type of `-(1<C>(e))` to it,
    /// while tsc reads the comparison chain a positive literal makes for both parsers
    /// (`f<1<C>(e)>(x)`). tsv has neither tree to give: its type parser stops at the `<`,
    /// and the comparison would print a chain acorn reads as a different program. So a `<`
    /// behind one keeps the region claimed, and the type parse rejects it.
    ///
    /// The claim is the region's own closing-`>` scan, which reads nothing between the
    /// `<` and that close, so it is coarser than the split it guards: `f<-1<C>[]>(x)` and
    /// `f<-1<C> + 1>(x)` are chains to both parsers and are rejected with the rest.
    ///
    /// One spelling is no parser's list: `-1<C>>`, where the `>` that would close it is
    /// the head of a shift token. `f<-1<C>>(x)` is the comparison chain
    /// `f < -1 < C >> x` to tsc and acorn-typescript alike, and so to tsv.
    ///
    /// The same parser takes a MEMBER behind the digits, so the operand reaches past a
    /// tail glued to them ([`skip_glued_member_tail`]) — and a spaced member, a call's
    /// arguments, a template or a non-null `!`, each a claim inside an index that opens
    /// as a type and a comparison chain everywhere else ([`IndexSplit`]).
    SignedLiteral,
    /// A nested argument list's own `>` (`A<B>`). A SECOND list behind one is where the
    /// oracles part again: acorn-typescript reads an instantiation instantiated
    /// (`f<A<B><C>>(x)` is a comparison over `A<B>`, `f<A<B><C> + 1>(x)` a chain through
    /// it), where tsc rejects the first and reads a chain through a type assertion in the
    /// second — and the pair the printed chain takes moves the compiler's reading of both.
    /// So a `<` behind one keeps the region claimed too, by the same closing-`>` scan —
    /// except inside an index whose body the second list ends, a spelling the compiler
    /// rejects and so has no second reading of.
    ListClose,
    /// An indexed access or an array suffix (`A[K]`, `A[]`), or any operand that closed a
    /// nested region. It takes no argument list, like [`OperandEnd::Other`]; kept apart
    /// because a `<` behind one is no claim at the list's own level, where a second list behind
    /// another operand's is.
    Indexed,
    /// Everything else: a literal, an object or tuple type, a parenthesized type,
    /// `this`, an `infer` binding.
    Other,
}

impl OperandEnd {
    /// Whether a `.Seg` behind the operand continues a qualified name. Read only where a
    /// paren shell stood between the two in the source ([`OperandSuffix::shell_only`]) —
    /// with nothing between them the head's own walk has taken the segment already.
    const fn heads_qualified_name(self) -> bool {
        matches!(
            self,
            OperandEnd::Name
                | OperandEnd::ImportName
                | OperandEnd::ImportQuery
                | OperandEnd::Keyword
        )
    }
}

/// What grading a type operand's HEAD settles: the verdict on the whole list, where the
/// operand ENDS — leaving the token after it to [`type_arg_head_commits`] — or where the
/// rest of the operand begins.
///
/// The split is what lets that walk read a union member by member, and an index body by
/// body, in a loop rather than by recursion: each head is graded by one call, and the walk
/// goes on from wherever the grade leaves it. A list of a million members, or an index
/// nested a million deep, is one frame deep.
#[derive(Clone, Copy)]
enum HeadGrade {
    /// A complete operand ends at `end` (ahead of any trivia); what follows it decides.
    Operand {
        /// The operand's own end.
        end: usize,
        /// How it ends.
        kind: OperandEnd,
    },
    /// A parenthesized operand at the list's own level, ending at this offset — past its
    /// `)`: a parenthesized type, which takes no argument list. Reported apart from any
    /// other operand because of the one position it reads differently in, the region's
    /// head ([`OperandEnd::RegionHeadShell`]).
    Shelled(usize),
    /// The head is a PREFIX of its operand — a type operator, a leading bar, a function
    /// type's parameter list and `=>`, a shell the graded form does not hold — and the
    /// rest begins at this offset, to be graded by the same dispatch.
    Prefix(usize),
    /// A [`HeadGrade::Prefix`] the two parse oracles part on, inside an index: a generic
    /// function head whose parameter list holds a default (`<T>(a = 1) =>`), which tsc
    /// reads a function type through and acorn-typescript an arrow function. The return
    /// begins at this offset and is graded like any prefix's; what the split makes of a
    /// region that reads as a list through it is the walk's to say
    /// ([`type_arg_head_walk`]).
    DefaultedPrefix(usize),
    /// A paren shell opened inside a nested region; its content begins at this offset and
    /// the walk reads it as a region of its own, closed by the shell's `)`.
    Shell(usize),
    /// The head takes the rest of its nested region as it stands, to the region's closer —
    /// the coarse reading's answer for a head it grades no operand of ([`Region`]).
    Rest,
    /// The head decided the list outright — a function type that commits, a head no type
    /// can have, or an arm whose answer is the closing-`>` scan. Only `false` inside a
    /// nested region, which nothing but its own closer ends.
    Decided(bool),
}

impl HeadGrade {
    /// [`HeadGrade::Prefix`] at the first token past `after`, or a refusal where the input
    /// ends there.
    fn prefix(bytes: &[u8], after: usize) -> Self {
        let next = skip_whitespace_and_comments(bytes, after);
        if next < bytes.len() {
            HeadGrade::Prefix(next)
        } else {
            HeadGrade::Decided(false)
        }
    }

    /// [`HeadGrade::Shell`] over the content past the `(` at `open`, or a refusal where
    /// the input ends there.
    fn shell(bytes: &[u8], open: usize) -> Self {
        let content = skip_whitespace_and_comments(bytes, open + 1);
        if content < bytes.len() {
            HeadGrade::Shell(content)
        } else {
            HeadGrade::Decided(false)
        }
    }
}

/// Find the delimiter that closes the nested region `frame`, scanning from `pos` — a
/// position inside it, at the region's own depth. `None` where a different delimiter
/// closes unbalanced first, or where the input ends.
///
/// For what reaches to the end of its region whatever stands there: a conditional type's
/// `extends`, a type predicate's `is`, and every token and head the coarse reading of an
/// index stops at ([`Region::Nested`]). Strings, templates and comments are opaque, as in
/// [`matching_delimiter_close`].
///
/// A `)` the region did not open is a shell the printer strips, absent from the form
/// [`TypeArgScan::Relex`] grades — that reading opens no [`Frame::Shell`] at all — so it
/// is stepped over there and ends the scan under [`TypeArgScan::Parse`].
fn nested_region_close(bytes: &[u8], pos: usize, frame: Frame, scan: TypeArgScan) -> Option<usize> {
    let target = match frame {
        Frame::Shell => PAREN_SLOT,
        Frame::Index => BRACKET_SLOT,
    };
    let mut depths = [0i32; 3];
    let end = bytes.len();
    let mut pos = pos;
    while pos < end {
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            pos = past;
            continue;
        }
        let byte = bytes[pos];
        if let Some(slot) = delimiter_slot(byte) {
            if matches!(byte, b'(' | b'[' | b'{') {
                depths[slot] += 1;
            } else if depths[slot] > 0 {
                depths[slot] -= 1;
            } else if slot == target {
                return Some(pos);
            } else if slot != PAREN_SLOT || scan.reads_source_as_written() {
                return None; // Unbalanced - a different group ended here
            }
        }
        pos += 1;
    }
    None
}

/// Grade the type operand head at `pos`, dispatching on its first byte, and stop short of
/// the follow token — [`type_arg_head_commits`] reads that. One dispatch for every operand
/// position: the list's first, a union or intersection member, an index's body, a function
/// type's return.
///
/// `region` is where the operand stands ([`Region`]), and it moves only the arms whose
/// operand runs to the END of its region: at the list's level those are settled by the
/// closing-`>` scan, and inside a nested region — which a delimiter closes, so no such scan
/// exists — they hand the rest back as a [`HeadGrade::Prefix`].
fn type_arg_head_grade(bytes: &[u8], pos: usize, region: Region, scan: TypeArgScan) -> HeadGrade {
    let coarse = region.is_coarse_under(scan);
    // Type keywords come ahead of the byte dispatch — every one is also
    // identifier-shaped, so the identifier arm below would otherwise claim them.
    match type_keyword_at(bytes, pos) {
        // An atom-shaped keyword (`string`, `null`, `this`, …) is as much a value
        // name as a type head, so it takes the identifier arm's follow-token
        // filter: only content that could continue a TYPE past the head commits.
        // That is what keeps ``p < string ? q : r > `t` `` a comparison (matching
        // acorn) while `f<string>()`, `f<string[]>()` and `p<string | q>(t, u)`
        // stay instantiations — the atom answers every follow-token question
        // exactly as an ordinary identifier does.
        Some(TypeKeywordKind::Atom) => {
            return check_identifier_type_arg_pattern(bytes, pos, WordHead::Keyword, region, scan);
        }

        // `this` is an atom too, minus one rule: `this.` is member access, never a
        // type (allow `this /* comment */ .`). `this` cannot head a qualified type
        // name — unlike `string`, an ordinary identifier token to the type grammar —
        // so the filter's qualified-name walk must not see the `.`.
        Some(TypeKeywordKind::This) => {
            let after_this = skip_whitespace_and_comments(bytes, pos + b"this".len());
            if bytes.get(after_this) == Some(&b'.') {
                return if coarse {
                    HeadGrade::Rest
                } else {
                    HeadGrade::Decided(false)
                };
            }
            return HeadGrade::Operand {
                end: pos + b"this".len(),
                kind: OperandEnd::Other,
            };
        }

        // A type-operator keyword is keyword-then-operand: the follow token is its
        // operand, so the follow-token filter's `_ => false` default would reject
        // `f<typeof x>()` and `f<keyof U>()`. Each operator asks its own
        // operand-shape question instead — see `type_operator_grade`.
        Some(TypeKeywordKind::Operator(op)) => {
            return type_operator_grade(bytes, op, pos, region, scan);
        }

        None => {}
    }

    // Dispatch based on first token after '<'
    match bytes[pos] {
        // Identifier: type reference like `<T>` or `<Ns.Type>`
        _ if identifier_starts_at(bytes, pos) => {
            check_identifier_type_arg_pattern(bytes, pos, WordHead::Name, region, scan)
        }

        // A type argument starting with `(`: a function type (`<(a: T) => R>`,
        // `<() => R>`, `<(...a: T[]) => R>`, `<({ a }: T) => R>`) or a parenthesized type
        // (`<(A | B) & C>`, `<(() => void) | null>`).
        //
        // A **function type** is read first, and it is the one head that asks no
        // follow-token question of its own: a PARAMETER LIST
        // ([`paren_starts_function_type`] — the single spelling of acorn-typescript's
        // `tsIsUnambiguouslyStartOfFunctionType` this parser has, shared with the
        // return-type scan and with the type parser's own `(`) whose `)` an
        // `=>` follows ([`paren_list_arrow_end`], shared with the construct and
        // generic-function heads). Behind that pair every byte to the region's own `>` is
        // the type's — its parameters, its `=>`, and a return type that may carry a
        // nested argument list closing on `>>`. A parameter list may open on tsc's
        // MODIFIERS too ([`paren_starts_modified_parameter_list`]) — a parameter property
        // is a grammar error rather than a parse failure, so the compiler and prettier
        // both read `f<(readonly a: T) => U>(x)` as a generic call, and tsv claims it and
        // lets the type parse reject it rather than printing a different program.
        //
        // **`(…) =>` after a `<` is never an expression.** An arrow function is an
        // `AssignmentExpression`, which cannot be a relational operand, so tsc and
        // acorn-typescript both reject `x < (a) => b`: there is no comparison chain for
        // the commit to take away, which is why no follower is consulted and why
        // [`TypeArgScan::Relex`] needs no protection here — no printed form can spell a
        // region this arm claims either.
        //
        // The body grade cannot answer for these heads: its follow-token filter is asked
        // past the `)`, where the `=>` opening the return type continues a type the filter
        // has no arm for, and a refusal there reads a whole generic call as a comparison
        // and REWRITES it (`f<(...a: T[]) => U>(x)` as `f < ((...a: T[]) => U > x)`) —
        // the silent direction this dispatch may never take.
        //
        // Two parameter shapes the claim reaches have no acorn-typescript AST behind them,
        // because the compiler defers their rule to the checker where acorn refuses the
        // syntax: a **parameter property** (the modifier run above) and a **parameter
        // default** (`f<(a = x) => U>(x)`, which tsc parses and acorn rejects at the `=`).
        // Claiming the region and letting the type parse refuse is the only reading that
        // is neither a different program nor an AST no oracle has; both are pinned as the
        // `input_invalid_*` files of
        // `tests/fixtures/typescript/syntax/disambiguation/less_than_function_type_head`.
        //
        // Without that `=>` the group is a parenthesized type or a value, and the ordinary
        // path decides: skip the balanced group and ask the shared follow-token
        // filter (as the `{`/`[`/literal arms do), so `x < (b)`, `x < (b) > c` and
        // `x < (a) ? q : r > (t, u)` stay comparisons while `callee<(T)>(…)` and
        // `x < (b) > (c)` are type arguments — matching acorn-typescript's follower handling
        // (quoted in `docs/conformance_prettier_ts.md` §Relational chain type-argument parens).
        //
        // Past the function-type reading, [`TypeArgScan::Relex`] LOOKS THROUGH the shell
        // instead of skipping it: a redundant pair is not in the form that reading grades
        // (`a < (arr[b - 1]) > c` prints as `a < arr[b - 1] > c`), so the head question is
        // the CONTENT's, asked with the content's own first byte. Skipping to the follow
        // filter would grade nothing at all there — the filter is the region's only
        // discriminator and [`TypeArgScan::Relex`] deletes its follow-token half, so every
        // parenthesized operand would commit, type or not. The closing side needs no rule
        // of its own: [`skip_relex_operand_suffixes`] already steps over the `)`.
        //
        // **A shell at the list's own level is reported as one** ([`HeadGrade::Shelled`]),
        // so the walk can read the single place the source and the printed form part
        // under THIS reading: a shell heading the region, which the printer may strip
        // ([`OperandEnd::RegionHeadShell`]).
        //
        // **Inside a nested region neither half is the list level's.** A function type
        // there is no verdict: an index takes any expression, an arrow function among
        // them, so `b[() => c + 1]` is as well-formed as `B[() => C]` and the RETURN is
        // what tells them apart — the head is a [`HeadGrade::Prefix`], and the walk grades
        // what stands past the `=>`. And a shell there is read by its CONTENT under both
        // readings ([`HeadGrade::Shell`]), because inside an index a pair is never the
        // operand's own: the index takes a whole expression, so the printer strips the
        // pair around one and adds one around another (`b[(c)]` prints as `b[c]`,
        // `b[c = d]` as `b[(c = d)]`), and a grade that read the `(` rather than what it
        // holds would answer the two spellings of one document differently — which, for
        // the body tsc's parameter-list claim takes (`b[(c = d)]`), means rejecting tsv's
        // own output. The list-level head can follow the compiler there only because its
        // shell is REQUIRED: `a < (b = c) > d` has no paren-free twin.
        //
        // One parameter list is an arrow FUNCTION's outright there: one that holds a
        // default (`b[(c = 1) => 0]`, [`parameter_list_holds_default`]). acorn-typescript's
        // function type takes none, so it reads the chain over an arrow function, where
        // tsc reads a function type and leaves the default to its checker. At the list's
        // own level that split has no chain to fall back on and the region stays claimed
        // (above); in an index it has, and the chain is the reading tsv gives.
        b'(' => {
            let function_type_return = (paren_starts_function_type(bytes, pos)
                || paren_starts_modified_parameter_list(bytes, pos))
            .then(|| paren_list_arrow_end(bytes, pos))
            .flatten();
            match (function_type_return, region) {
                (Some(_), Region::List) => HeadGrade::Decided(true),
                (Some(_), Region::Nested) if parameter_list_holds_default(bytes, pos) => {
                    HeadGrade::Decided(false)
                }
                (Some(return_type), Region::Nested) => HeadGrade::prefix(bytes, return_type),
                (None, _) if !scan.reads_source_as_written() => HeadGrade::prefix(bytes, pos + 1),
                (None, Region::List) => paren_type_head_close(bytes, pos)
                    .map_or(HeadGrade::Decided(false), |close| {
                        HeadGrade::Shelled(close + 1)
                    }),
                (None, Region::Nested) => HeadGrade::shell(bytes, pos),
            }
        }

        // A non-null `!` ahead of the operand — `<!T>` is tsc's JSDoc non-nullable type,
        // so `a < !b >⏎c` is an instantiation plus a statement where the flat line is a
        // comparison chain. Only [`TypeArgScan::Relex`] reads it: acorn-typescript, tsv's
        // parse oracle, has no such type, so admitting it at [`TypeArgScan::Parse`] would
        // move a PARSE off the drop-in contract. `!=` / `!==` are operators, never a mark.
        //
        // At the list's own level alone: a `!` inside an index is read by neither scan
        // ([`skip_relex_operand_suffixes`]).
        b'!' if !scan.reads_source_as_written()
            && region == Region::List
            && bytes.get(pos + 1) != Some(&b'=') =>
        {
            HeadGrade::prefix(bytes, pos + 1)
        }

        // A second `<` — the tail of a `<<` shift token, or a spaced
        // `< <` — can only open a generic function type
        // (`f<<T>(v: T) => void>()`); shift chains (`a << b > c`) never
        // match its `>`-then-`(` shape. Inside a nested region the same head is a generic
        // ARROW FUNCTION as readily (`b[<T>(v: T) => v + 1]`), so its return is graded.
        //
        // A parameter default there is the split it is behind a plain head
        // ([`parameter_list_holds_default`]), reported with the prefix
        // ([`HeadGrade::DefaultedPrefix`]) to the reading that grades an index finely.
        b'<' => match (generic_function_type_arrow_end(bytes, pos + 1), region) {
            (None, _) => HeadGrade::Decided(false),
            (Some(_), Region::List) => HeadGrade::Decided(true),
            (Some(return_type), Region::Nested) => match HeadGrade::prefix(bytes, return_type) {
                HeadGrade::Prefix(next) if !coarse && generic_head_holds_default(bytes, pos) => {
                    HeadGrade::DefaultedPrefix(next)
                }
                grade => grade,
            },
        },

        // Object/tuple literal types — but `{`/`[` equally start object and array
        // *value* literals, so `x < {a: 1}` is a comparison. Skip the balanced group, then
        // only a type-continuing follow token commits: `f<{ a: T }>()` and
        // `f<[T, U] | null>()` are type arguments, `x < [1] ? q : r > (t, u)` is a
        // comparison whatever sits past the would-be closing `>`.
        //
        // The BODY is deliberately not graded, though tsc's answer turns on it (`<[1, 2]>`
        // and `<{ x: B }>` are types to the compiler, `<[b, c++]>` and `<{ ...s }>` are
        // not). Neither delimiter is a shell the printer strips, so [`TypeArgScan::Relex`]
        // cannot look through one the way the `(` arm does, and neither opens a region the
        // printer's kept-shell rule puts a pair around — while the printer parenthesizes
        // freely INSIDE both bodies, moving a token a grade refused one level deeper. A
        // refusal here would therefore print a bare chain that tsv's own parse then claims,
        // which is the unsound direction ([`TypeArgScan`]'s soundness property). So these
        // arms commit on any matching `>` that a line terminator, a `(` or a template
        // follows, and tsv reads `x < { ...s } >⏎c` as a type-argument list and then REJECTS
        // it — where acorn and tsc both read the comparison chain.
        //
        // The same holds one level down, as an index's body: `f<A[{ a: 1 }]>(x)` and
        // `f<A[[0]] | B>(x)` are the generic calls both oracles read, and
        // `a < b[{ c: d + 1 }] > (t, u)` — a comparison chain to both — is claimed and
        // rejected like its list-level twin. A refusal there would be the SILENT direction:
        // a real generic call read as a comparison and reprinted as one.
        b'{' | b'[' => {
            matching_delimiter_close(bytes, pos).map_or(HeadGrade::Decided(false), |close| {
                HeadGrade::Operand {
                    end: close + 1,
                    kind: OperandEnd::Other,
                }
            })
        }

        // A string or template key settles an index for the coarse reading ([`Region`]).
        b'\'' | b'"' | b'`' if coarse => HeadGrade::Rest,

        // String literal types — the same follow-token question after the literal:
        // `f<'a' | 'b'>()` commits, `x < 'a' + 'b' > (t, u)` stays a comparison.
        b'\'' | b'"' => skip_trivia(bytes, pos, bytes.len(), TriviaProfile::JS).map_or(
            HeadGrade::Decided(false),
            |end| HeadGrade::Operand {
                end,
                kind: OperandEnd::Other,
            },
        ),

        // Template literal types — skipped interpolation-aware (the opaque
        // quote-to-quote trivia scan would mis-pair backticks across a nested
        // `` `${`x`}` ``), then the same follow-token question.
        b'`' => HeadGrade::Operand {
            end: skip_template_literal(bytes, pos, bytes.len(), crate::OPERAND_GRAMMAR),
            kind: OperandEnd::Other,
        },

        // Numeric literal types: `<42>`, `<-1>`, `<.5>` — but `x < 42` is a
        // comparison, so the literal alone decides nothing. Skip it (sign and a
        // missing integer part included: `DecimalLiteral :: . DecimalDigits` is a
        // literal type like any other, so `f<.5>()` is an instantiation to acorn;
        // `-b` skips nothing and is a unary negation, never a type), then only a
        // type-continuing follow token commits: `f<-1>()`, `f<.5>()` and
        // `f<0 | 1>()` are type arguments, `x < 1 + 2 > (t, u)` and
        // ``x < .5 ? q : r > `t` `` stay comparisons.
        //
        // The head class is [`numeric_literal_starts_at`], shared with the
        // `keyof`/`unique` operand site so a literal cannot be admitted at one and
        // refused at the other.
        //
        // A NEGATIVE literal ends past a member tail glued to its digits
        // ([`negative_literal_end`]).
        _ if numeric_literal_starts_at(bytes, pos) => {
            let after = skip_signed_numeric_literal(bytes, pos, scan);
            if after == pos {
                HeadGrade::Decided(false)
            } else if bytes[pos] != b'-' {
                HeadGrade::Operand {
                    end: after,
                    kind: OperandEnd::Other,
                }
            } else {
                HeadGrade::Operand {
                    end: negative_literal_end(bytes, after, scan),
                    kind: OperandEnd::SignedLiteral,
                }
            }
        }

        // A leading `|`/`&` on the first union/intersection member
        // (`f<| A | B>()`, `f<& A & B>()`) — the form prettier itself emits
        // whenever such a type argument breaks across lines. Neither byte can
        // start an expression, so a `<` followed by one is never a comparison;
        // the closing-`>` + follow-token scan still runs, as in every other arm,
        // so an unterminated `<` stays unclaimed.
        //
        // An index's body takes the same bar (`A[| B | C]`, single — `||` / `&&` are the
        // logical operators), which the union printer's own leading-pipe layout puts
        // there (`fn<⏎A[⏎| B // c⏎| C]⏎>()`), so its output has to read back as the
        // instantiation it printed; the member behind the bar is graded like any other.
        b'|' | b'&' => match region {
            Region::List => HeadGrade::Decided(scan_for_closing_angle_bracket(bytes, pos, scan)),
            Region::Nested if bytes.get(pos + 1) == Some(&bytes[pos]) => HeadGrade::Decided(false),
            Region::Nested if coarse => HeadGrade::Rest,
            Region::Nested => HeadGrade::prefix(bytes, pos + 1),
        },

        // Not a recognized type argument start
        _ => HeadGrade::Decided(false),
    }
}

/// Whether the type query whose `typeof` stands at `query` is followed by an ARRAY suffix:
/// its entity name or import head, any argument lists, then an empty `[`…`]`.
fn type_query_array_follows(bytes: &[u8], query: usize) -> bool {
    let name = skip_whitespace_and_comments(bytes, skip_identifier(bytes, query));
    if !identifier_starts_at(bytes, name) {
        return false;
    }
    let mut pos = skip_identifier(bytes, name);
    if is_word_at(bytes, name, b"import") {
        let paren = skip_whitespace_and_comments(bytes, pos);
        if bytes.get(paren) == Some(&b'(') {
            match matching_delimiter_close(bytes, paren) {
                Some(close) => pos = close + 1,
                None => return false,
            }
        }
    }
    pos = skip_whitespace_and_comments(bytes, skip_qualified_tail(bytes, pos));
    while bytes.get(pos) == Some(&b'<') {
        match matching_angle_close(bytes, pos + 1, TypeArgScan::Parse) {
            Some(close) => pos = skip_whitespace_and_comments(bytes, close + 1),
            None => return false,
        }
    }
    bytes.get(pos) == Some(&b'[')
        && bytes.get(skip_whitespace_and_comments(bytes, pos + 1)) == Some(&b']')
}

/// Whether the type query whose `typeof` stands at `query` is over an import that takes
/// OPTIONS — a second argument at the call's own level (`import('m', { with: … })`).
fn type_query_import_takes_options(bytes: &[u8], query: usize) -> bool {
    let name = skip_whitespace_and_comments(bytes, skip_identifier(bytes, query));
    let open = skip_whitespace_and_comments(bytes, skip_identifier(bytes, name));
    let Some(close) = matching_delimiter_close(bytes, open) else {
        return false;
    };
    let mut pos = open + 1;
    while pos < close {
        if let Some(past) = skip_trivia(bytes, pos, close, TriviaProfile::JS) {
            pos = past;
            continue;
        }
        match bytes[pos] {
            b',' => return true,
            b'(' | b'[' | b'{' => match matching_delimiter_close(bytes, pos) {
                Some(group_close) => pos = group_close + 1,
                None => return false,
            },
            _ => pos += 1,
        }
    }
    false
}

/// Whether the generic function head whose `<` stands at `lt` has a parameter list that
/// holds a default ([`parameter_list_holds_default`]) — behind its type-parameter list,
/// whose own defaults (`<T = 1>(a) =>`) are none of a parameter's.
fn generic_head_holds_default(bytes: &[u8], lt: usize) -> bool {
    matching_angle_close(bytes, lt + 1, TypeArgScan::Parse).is_some_and(|close| {
        parameter_list_holds_default(bytes, skip_whitespace_and_comments(bytes, close + 1))
    })
}

/// Whether the parameter list that opens at `open` holds a DEFAULT of a parameter's own:
/// a `=` that is no arrow's at the list's own level, outside every nested group,
/// type-parameter list, string and comment (`(a = 1)`, `(a: T = 1)`, `({ a } = b)`). A
/// `=` inside a destructuring pattern or a type-parameter list is no default of the
/// parameter's (`({ a = 1 })`, `([a = 1])`, `(a: <T = 1>() => T)`): acorn-typescript's
/// function type takes those, as tsc's does. The list's own level holds no other `=` — a
/// comparison or an arrow function can only stand in a default's value, behind the `=`
/// this stops at.
fn parameter_list_holds_default(bytes: &[u8], open: usize) -> bool {
    let Some(close) = matching_delimiter_close(bytes, open) else {
        return false;
    };
    let mut pos = open + 1;
    while pos < close {
        if let Some(past) = skip_trivia(bytes, pos, close, TriviaProfile::JS) {
            pos = past;
            continue;
        }
        match bytes[pos] {
            // A nested group: a destructuring pattern, or a group of an annotation's.
            b'(' | b'[' | b'{' => match matching_delimiter_close(bytes, pos) {
                Some(group_close) => pos = group_close,
                None => return false,
            },
            // A type-parameter or type-argument list of an annotation's — the only `<`
            // the list's own level holds ahead of a default.
            b'<' => {
                if let Some(angle_close) = matching_angle_close(bytes, pos + 1, TypeArgScan::Parse)
                {
                    pos = angle_close;
                }
            }
            b'=' => {
                if bytes.get(pos + 1) != Some(&b'>') {
                    return true;
                }
                pos += 1;
            }
            _ => {}
        }
        pos += 1;
    }
    false
}

/// Where a word stands when it is an OPERATOR rather than a name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WordFixity {
    /// Takes an operand AFTER it (`await b`), so it stands where no operand has ended.
    Prefix,
    /// Takes an operand on each side (`a as B`), so it stands where one has.
    Infix,
}

/// A word that can only continue an EXPRESSION, and so refuses a parenthesized type-argument
/// head's body.
struct OperatorWord {
    word: &'static [u8],
    fixity: WordFixity,
}

/// The expression-only words tsc abandons a `(`-headed region over.
///
/// A word REFUSES only where it stands in a genuine operator position — an infix one past a
/// complete operand, a prefix one ahead of an operand and with none behind it. Everywhere
/// else the same spelling is a NAME, and every one of them is a legal one: a segment of a
/// qualified name (`(T.in)`, `(ns.await.T)`), a type operator's operand (`(typeof as)`,
/// `(typeof await.b)`), a bare type reference (`(satisfies)`). Refusing those is a silent
/// wrong tree on real generic syntax, which is the direction this grade may never take.
///
/// A mapped type's key remapping spells `in` and `as` (`({ [K in T as F<K>]: V })`), and a
/// conditional's `infer` spells `extends` — all of them inside a nested group this walk steps
/// over whole, so none of them reaches here.
const OPERATOR_WORDS: &[OperatorWord] = &[
    OperatorWord {
        word: b"as",
        fixity: WordFixity::Infix,
    },
    OperatorWord {
        word: b"await",
        fixity: WordFixity::Prefix,
    },
    OperatorWord {
        word: b"delete",
        fixity: WordFixity::Prefix,
    },
    OperatorWord {
        word: b"in",
        fixity: WordFixity::Infix,
    },
    OperatorWord {
        word: b"instanceof",
        fixity: WordFixity::Infix,
    },
    OperatorWord {
        word: b"satisfies",
        fixity: WordFixity::Infix,
    },
    OperatorWord {
        word: b"void",
        fixity: WordFixity::Prefix,
    },
    OperatorWord {
        word: b"yield",
        fixity: WordFixity::Prefix,
    },
];

/// Whether `word` is an expression-only operator, and with which fixity.
fn operator_word_fixity(word: &[u8]) -> Option<WordFixity> {
    OPERATOR_WORDS
        .iter()
        .find(|entry| entry.word == word)
        .map(|entry| entry.fixity)
}

/// Words that END NO OPERAND: the type operators, the member modifiers, and the construct
/// and import heads. Each introduces more type past itself, so the word behind it is still
/// at an OPERAND position — which is what keeps `(typeof as)` and `(typeof await.b)`
/// type-argument lists rather than refusals ([`OPERATOR_WORDS`]). The member modifiers ride
/// along because the list is a lexical fact about the words, not a claim that an object
/// type's members reach this body.
///
/// The same list answers a second question, for the same reason: an identifier glued to a
/// `(` is a CALL, which no type spells — unless the identifier is one of these, where the
/// `(` opens a construct signature's parameter list or an import type's specifier.
const TYPE_PREFIX_WORDS: &[&[u8]] = &[
    b"abstract",
    b"asserts",
    b"extends",
    b"get",
    b"import",
    b"infer",
    b"is",
    b"keyof",
    b"new",
    b"readonly",
    b"set",
    b"typeof",
    b"unique",
];

/// Find the `)` closing the parenthesized type-argument head at `open`, requiring its BODY to
/// read as a type — the graded twin of
/// [`matching_delimiter_close`],
/// which answers the same delimiter question and grades nothing.
///
/// One walk answers both: the delimiter depths locate the close, and every token at the
/// body's own level is graded on the way past. Nested groups are stepped over wholesale,
/// because a nested body's own grammar is not this one — an object type's
/// `[K in keyof T]` holds an `in` that a parenthesized type may not.
///
/// A redundant `(` SHELL the printer strips is not part of the head: `x < ((a || b)) > c`
/// grades the content its own shell-free twin does, so the two authorings of one chain reach
/// one verdict (`deno task paren:audit`'s `< > operand` class enumerates exactly that pair),
/// and [`TypeArgScan::Relex`]'s look-through has the same rule to agree with. The chain of
/// them is located by ONE descent over the head's leading `(`s and then walked once from the
/// innermost — never by re-walking the body per shell, which is quadratic in the nesting the
/// input chose (`x < ((((a)))) > (t, u)`).
///
/// The grade is a REFUSAL list, not a whitelist, and deliberately: a body wrongly refused
/// turns a real generic call into a comparison chain — a silent wrong tree on code that
/// parses either way — where a body wrongly admitted is a loud parse error on a shape no
/// generic has. So only tokens that no type carries at a body's top level refuse, and
/// everything else commits.
///
/// Both readings ask this, which is what keeps them at one verdict on one text, and the
/// grade is blind to WHITESPACE and LINE TERMINATORS for the same reason: the printer folds
/// and adds breaks and renormalizes spacing, so a grade that read either would answer the
/// source and the printed form differently.
fn paren_type_head_close(bytes: &[u8], open: usize) -> Option<usize> {
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    // The redundant-shell chain, found in one descent: a shell holds nothing but the next
    // group, so the chain is exactly the run of `(`s at the head. Whether each one really
    // holds nothing ELSE is settled by the walk itself, which keeps grading at the shell's
    // own level when it does.
    let mut inner = open;
    let mut shells = 0usize;
    while bytes.get(skip_whitespace_and_comments(bytes, inner + 1)) == Some(&b'(') {
        inner = skip_whitespace_and_comments(bytes, inner + 1);
        shells += 1;
    }
    graded_paren_close(bytes, inner, shells)
}

/// [`paren_type_head_close`]'s walk: the close of the OUTERMOST head, `shells` redundant `(`
/// levels out from `inner`. `None` where a different delimiter closes unbalanced first, where
/// the input ends, or where a body-level token refuses the type reading.
///
/// The walk grades at ONE level at a time — `level`, the depth whose tokens are the body's
/// own. Closing that level with shells left to go steps the level out by one and starts the
/// enclosing body's grade there, so a shell that turns out to hold more than the group is
/// graded like any other body and the walk still visits each byte once.
fn graded_paren_close(bytes: &[u8], inner: usize, shells: usize) -> Option<usize> {
    // paren, bracket, brace — the graded head's own slot opens at one per level, as if this
    // walk had already stepped over each `open` it starts past.
    let mut depths = [0i32; 3];
    depths[PAREN_SLOT] = shells as i32 + 1;
    // Total nesting, so "at the body's own level" is one compare rather than three.
    let mut depth = shells as i32 + 1;
    let mut level = depth;
    let mut remaining = shells;
    let mut grade = BodyGrade::new();
    let end = bytes.len();
    let mut pos = inner + 1;

    while pos < end {
        // Whitespace and comments are not tokens: neither ends an operand, so the reading
        // stays where it stood — which is also what keeps the grade blind to the line
        // breaks and the spacing the printer moves.
        let past = skip_whitespace_and_comments(bytes, pos);
        if past != pos {
            pos = past;
            continue;
        }
        // Strings and templates are opaque, as in the ungraded twin, and are literal TYPES,
        // so each ends an operand.
        if let Some(past) = skip_trivia(bytes, pos, end, TriviaProfile::JS) {
            if depth == level {
                // A template glued to a complete operand is a TAGGED template, which no
                // type spells (`` (b`c`) ``, `` (typeof b`c`) ``); behind a type operator
                // the same template is its operand (`` (keyof `c`) ``).
                if bytes[pos] == b'`' && grade.prev.ends_operand() && !grade.committed {
                    return None;
                }
                grade.prev = PrevToken::Operand;
            }
            pos = past;
            continue;
        }
        let byte = bytes[pos];
        if let Some(slot) = delimiter_slot(byte) {
            if matches!(byte, b'(' | b'[' | b'{') {
                depths[slot] += 1;
                depth += 1;
                if depth == level + 1 {
                    grade.prev = PrevToken::Other;
                }
            } else {
                depths[slot] -= 1;
                depth -= 1;
                if depths[slot] < 0 {
                    return None; // Unbalanced - a different group ended here
                }
                if depth == level - 1 {
                    if slot != PAREN_SLOT {
                        return None; // Unbalanced - a different group ended here
                    }
                    if remaining == 0 {
                        return Some(pos);
                    }
                    // A redundant shell closed over this level; grade the enclosing body
                    // from here, where the group it just held has ended an operand.
                    remaining -= 1;
                    level -= 1;
                    grade = BodyGrade::new();
                    grade.prev = PrevToken::ParenClose;
                } else if depth == level {
                    grade.prev = if byte == b')' {
                        PrevToken::ParenClose
                    } else {
                        PrevToken::Operand
                    };
                }
            }
            pos += 1;
            continue;
        }
        if depth != level || grade.committed {
            pos += 1;
            continue;
        }
        pos = grade_body_token(bytes, pos, &mut grade)?;
    }
    None
}

/// The paren slot in [`delimiter_slot`]'s triple — the graded head's own.
const PAREN_SLOT: usize = 0;

/// The bracket slot in [`delimiter_slot`]'s triple — an index's.
const BRACKET_SLOT: usize = 1;

/// The delimiter-depth slot a `(`/`)`, `[`/`]` or `{`/`}` counts in, or `None` for every
/// other byte — [`matching_delimiter_close`]'s
/// own classification, kept identical so the graded walk locates the same close.
#[inline]
const fn delimiter_slot(byte: u8) -> Option<usize> {
    match byte {
        b'(' | b')' => Some(PAREN_SLOT),
        b'[' | b']' => Some(BRACKET_SLOT),
        b'{' | b'}' => Some(2),
        _ => None,
    }
}

/// The body-level token a parenthesized head's walk read last — the one fact of the token
/// BEHIND it that a token's own grade may turn on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PrevToken {
    /// The body's start, an operator, a separator, a nested group's opener, or a word that
    /// introduces more type past itself ([`TYPE_PREFIX_WORDS`]): no operand has ended.
    Other,
    /// A name, a literal, or a nested `[…]` / `{…}` group: an operand has ENDED, which is
    /// what tells a binary `-` from a literal type's sign, an infix `as` from a type
    /// reference of the same name, and a tagged template from a template literal type.
    Operand,
    /// A `)` — a nested group's close, or a redundant shell's. An ended operand too, and the
    /// one place a type spells `=>` behind: any other `=>` is an arrow FUNCTION's
    /// (`(a => b)`, `(async a => b)`).
    ParenClose,
    /// The word `typeof`, whose operand is an entity name — the one type position that may
    /// hold a `this.`, and so the one place that spelling may not refuse
    /// (`f<(keyof typeof this.x)>(v)`).
    Typeof,
    /// A `.`, which makes the word behind it a qualified name's segment whatever it spells
    /// (`(Ns.infer.T)`, `(Ns.class)`).
    Dot,
}

impl PrevToken {
    /// Whether the token ended an operand.
    const fn ends_operand(self) -> bool {
        matches!(self, PrevToken::Operand | PrevToken::ParenClose)
    }
}

/// What the walk has read of a parenthesized head's body so far — the facts a token's own
/// grade may turn on.
#[derive(Clone, Copy)]
struct BodyGrade {
    /// The body-level token read last.
    prev: PrevToken,
    /// Whether the body has proved itself a PARAMETER LIST, which ends its grading: tsc's
    /// `isUnambiguouslyStartOfFunctionType` claims the whole group for a function type on a
    /// `:` or a bare `=` behind the first parameter, and its parse then carries every token
    /// in the group to the `>` whatever stands there. So nothing past one may refuse.
    committed: bool,
    /// How many body-level `<` are still open. A `>` past zero closes no nested argument
    /// list, and is where tsc's own list parse stops ([`grade_body_token`]).
    angle_depth: u32,
}

impl BodyGrade {
    /// A fresh grade for one body.
    const fn new() -> Self {
        BodyGrade {
            prev: PrevToken::Other,
            committed: false,
            angle_depth: 0,
        }
    }
}

/// Grade one token at a parenthesized head's own level, advancing [`BodyGrade`] and returning
/// where the token ends — or `None` where it refuses the type reading outright.
///
/// **A refusal says tsc reads a comparison chain here, and nothing weaker.** tsc does not
/// guess at a `<`: `parseTypeArgumentsInExpression` really parses the list with
/// `parseDelimitedList(TypeArguments, parseType)`, which is error-RECOVERING — a token no
/// element can start is skipped (`abortParsingListOrMoveToNextToken`) and the parse resumes
/// — and the region is claimed whenever that recovery still lands on the `>`, errors and
/// all. tsc abandons the region only where the recovery cannot get there, and that is the
/// one condition under which the `<` is a comparison operator. So this grade may refuse
/// only on the abandoning cells; on every other body it commits, and the type parse then
/// rejects the region exactly as tsc does.
///
/// | body-level token | verdict |
/// | --- | --- |
/// | `~` | refuse |
/// | `++` `--`, prefix or postfix | refuse |
/// | `?.` optional chain | refuse |
/// | `this .` | refuse, unless it is `typeof`'s operand |
/// | `\|\|` `&&` `??` `^` | refuse |
/// | `==` `!=` `===` `!==` `<=` `>=` | refuse |
/// | `+` `-` `*` `%` past an operand | refuse |
/// | a `/`, which divides or opens a regex literal | refuse |
/// | a template glued to an operand (a tagged template) | refuse |
/// | `class` ahead of a body, a name or a heritage clause | refuse |
/// | `infer` ahead of a `[`, a `.` or a `<` | refuse |
/// | an `=>` behind anything but a `)` | refuse |
/// | a `>` closing no nested list, glued to another `>` | refuse |
/// | a `>` closing no nested list, unless a `(` or a template follows it | refuse |
/// | a decorator's `@` | refuse |
/// | a unary `+`, and a `-` on anything but a numeral | refuse |
/// | `as` `satisfies` `in` `instanceof` past an operand | refuse |
/// | `await` `void` `yield` `delete` ahead of an operand | refuse |
/// | an identifier glued to `(` | refuse, unless the word opens a type |
/// | a compound assignment | refuse |
/// | a `-` on a numeral | commit |
/// | `?` … `:` conditional | commit |
/// | `...` | commit |
/// | `:`, and a bare `=` | commit, and end the grading |
/// | a `>` closing no nested list, ahead of a `(` or a template | commit |
/// | `<<`, and a `>>` `>>>` that closes nested lists | commit |
/// | `,` `!` `\|` `&`, and an `=>` behind a `)` | commit |
///
/// `<<` and `>>` are the two shifts a type grammar re-reads: the type parser re-scans a `<<`
/// into the `<` of a nested argument list, and a nested list CLOSES with `>>`, so neither
/// ends a type at all — which is why the walk counts the body's open `<`s
/// ([`BodyGrade::angle_depth`]), and a `>` past zero is the comparison operator it looks
/// like. A `!` is the non-null mark tsc's type grammar carries (`JSDocNonNullableType`), so
/// the compiler claims the region and its CHECKER rejects it; tsv has no such type and
/// rejects at the parse
/// (`tests/fixtures/typescript/expressions/binary/relational_paren_head_non_null_svelte_divergence`).
///
/// A `:` and a bare `=` are the converse of the shifts — tsc's
/// `isUnambiguouslyStartOfFunctionType` reads either behind the first parameter and claims
/// the whole group for a function type, so its parse carries every token past one to the `>`
/// ([`BodyGrade::committed`]).
///
/// Every cell is measured against tsc; where its recovery and the shape of the grammar
/// disagree, the measurement is what stands here.
fn grade_body_token(bytes: &[u8], pos: usize, grade: &mut BodyGrade) -> Option<usize> {
    // Words first, so a word-shaped refusal is matched whole and an ordinary name can
    // never be read one byte at a time.
    if identifier_starts_at(bytes, pos) {
        let word_end = skip_identifier(bytes, pos);
        let word = &bytes[pos..word_end];
        let next = skip_whitespace_and_comments(bytes, word_end);
        let prev = grade.prev;
        let after_operand = prev.ends_operand();
        // A modifier or type operator introduces more type past itself, so it ends no
        // operand and CLEARS the one behind it — the word after it stands at an operand
        // position, which is what keeps `(readonly as)` a type-argument list
        // ([`TYPE_PREFIX_WORDS`]).
        grade.prev = if word == b"typeof" {
            PrevToken::Typeof
        } else if TYPE_PREFIX_WORDS.contains(&word) {
            PrevToken::Other
        } else {
            PrevToken::Operand
        };
        // Two words that are an expression's whenever more of one follows, read ahead of
        // everything else — and never behind a `.`, where any word is a name's segment. A
        // `class` with a body, a name or a heritage clause behind it is a class EXPRESSION
        // ([`class_expression_follows`]). An `infer` binds a NAME: with a `[`, a `.` or a
        // `<` there instead it is the ordinary value it is everywhere else
        // (`(infer[b])`, `(infer.b)`, `(infer < a)`), where tsc still claims a bare
        // `(infer)` and an `(infer | b)` and reports the missing name.
        if prev != PrevToken::Dot {
            if word == b"class" && class_expression_follows(bytes, next) {
                return None;
            }
            if word == b"infer" && matches!(bytes.get(next), Some(b'[' | b'.' | b'<')) {
                return None;
            }
        }
        // `this.` is member access; `this` heads no qualified type name, which is the same
        // rule [`TypeKeywordKind::This`] answers at the head itself. `typeof`'s operand IS
        // an entity name, and is the one type position that holds one.
        if word == b"this" && bytes.get(next) == Some(&b'.') && prev != PrevToken::Typeof {
            return None;
        }
        // A call, which no type spells — unless the word is one whose own `(` opens a
        // construct signature's parameter list or an import type's specifier
        // ([`TYPE_PREFIX_WORDS`]).
        if bytes.get(next) == Some(&b'(') && !TYPE_PREFIX_WORDS.contains(&word) {
            return None;
        }
        if let Some(fixity) = operator_word_fixity(word) {
            let is_operator = match fixity {
                WordFixity::Infix => after_operand,
                WordFixity::Prefix => !after_operand && operand_starts_at(bytes, next),
            };
            if is_operator {
                return None;
            }
        }
        return Some(word_end);
    }

    let prev = grade.prev;
    let after_operand = prev.ends_operand();
    grade.prev = PrevToken::Other;
    // An assignment operator is read AHEAD of every refusal below. A COMPOUND one refuses
    // outright: no parameter default spells `+=`, so tsc reads `x < (a += b) > (t, u)` as the
    // comparison chain acorn does. A bare `=` is a parameter default, which tsc claims the
    // whole group for ([`BodyGrade::committed`]).
    if let Some(end) = assignment_operator_end(bytes, pos) {
        if bytes[pos] != b'=' {
            return None;
        }
        grade.committed = true;
        return Some(end);
    }
    let byte = bytes[pos];
    match byte {
        // A numeric literal type (`(0 | 1)`); the sign is the `-` arm's, since a `-` may
        // equally be arithmetic.
        b'0'..=b'9' => {
            grade.prev = PrevToken::Operand;
            Some(skip_numeric_literal(bytes, pos))
        }
        b'.' if matches!(bytes.get(pos + 1), Some(b'0'..=b'9')) => {
            grade.prev = PrevToken::Operand;
            Some(skip_numeric_literal(bytes, pos))
        }
        // A rest parameter (`(a: T, ...b: U[]) => V`).
        b'.' if bytes[pos..].starts_with(b"...") => Some(pos + 3),
        // A qualified name's `.` (`(Ns.T)`). An optional chain's `?.` refuses at the `?`
        // below, so a `.` here follows a name and nothing else.
        b'.' => {
            grade.prev = PrevToken::Dot;
            Some(pos + 1)
        }
        // A parameter's type annotation, which proves the group a parameter list and ends
        // the grading ([`BodyGrade::committed`]).
        b':' => {
            grade.committed = true;
            Some(pos + 1)
        }
        // `++` / `--`: no type carries an update operator.
        b'+' | b'-' if bytes.get(pos + 1) == Some(&byte) => None,
        // `||` / `&&` are the logical operators, where the single `|` / `&` are a union and
        // an intersection.
        b'|' | b'&' if bytes.get(pos + 1) == Some(&byte) => None,
        // `??` is the nullish coalescer and `?.` the optional chain; no type spells either.
        // The `.` needs a digit test of its own: a conditional type's branch may be a
        // leading-point numeric literal (`A extends B ? .5 : C`).
        b'?' if bytes.get(pos + 1) == Some(&b'?') => None,
        b'?' if bytes.get(pos + 1) == Some(&b'.')
            && !matches!(bytes.get(pos + 2), Some(b'0'..=b'9')) =>
        {
            None
        }
        // Bitwise xor and complement; no type spells either.
        b'^' | b'~' => None,
        // `!=` / `!==` and `==` / `===` are equality operators. The assignment `=` was
        // taken above, so an `=` reaching here carries a second one.
        b'!' | b'=' if bytes.get(pos + 1) == Some(&b'=') => None,
        // `=>`, read whole so its `>` closes nothing. A type spells one behind a parameter
        // list's `)` and nowhere else, so any other is an arrow FUNCTION's — a bare-name
        // parameter (`(a => b)`, `(async a => b)`, `(a, b => c)`).
        b'=' if bytes.get(pos + 1) == Some(&b'>') => {
            (prev == PrevToken::ParenClose).then_some(pos + 2)
        }
        // The relational `<=` / `>=`.
        b'<' | b'>' if bytes.get(pos + 1) == Some(&b'=') => None,
        // A nested argument list's own delimiters. A `<<` is two of them, since the type
        // parser re-scans it (`(A<<T>(v: T) => void>)`).
        b'<' => {
            grade.angle_depth += 1;
            Some(pos + 1)
        }
        b'>' if grade.angle_depth > 0 => {
            grade.angle_depth -= 1;
            Some(pos + 1)
        }
        // A `>` that closes NO nested list is where tsc's list parse stops: it reports the
        // group's missing `)`, takes this `>` for the region's own, and asks its follower
        // question HERE. Glued to another `>` the token re-scans as a shift and the region
        // is abandoned (`(a >> b)`, `(a<b>>> c)`); alone, only a `(` or a template past it
        // still claims the region (`(a > (b))`, which tsc then rejects), and every other
        // follower continues the comparison the `>` is (`(a > b)`, `(() => a > b)`).
        //
        // tsc also claims past a LINE BREAK there. This grade does not read one — it is
        // blind to the breaks the printer moves — so `(a >⏎b)` is the chain acorn reads.
        b'>' => {
            let glued = bytes.get(pos + 1) == Some(&b'>');
            let follower = skip_whitespace_and_comments(bytes, pos + 1);
            (!glued && matches!(bytes.get(follower), Some(b'(' | b'`'))).then_some(pos + 1)
        }
        // A `/` that reaches here opens a REGEX literal or divides (a comment's was skipped
        // as trivia), and no type spells either. It refuses ahead of its pattern, which may
        // hold anything — a `<`…`>` pair included (`(a, /b<c>/)`).
        b'/' => None,
        // A decorator (`(@dec class {})`).
        b'@' => None,
        // Arithmetic past a complete operand.
        b'+' | b'-' | b'*' | b'%' if after_operand => None,
        // At an operand position a `-` opens a NEGATIVE LITERAL type (`(-1 | 1)`), which is
        // the only sign a type carries — tsc's own literal type takes a minus and nothing
        // else, so `(+1)` is the comparison chain its unary `+` makes it. Anything else
        // behind either sign is the unary operator, which no type spells.
        //
        // The literal is taken with any member tail glued to its digits, as it is where
        // it stands at the list's own level ([`skip_glued_member_tail`]): `(-1..x())` is
        // the literal type of a call to acorn-typescript, so the group stays claimed.
        b'-' if numeric_literal_starts_at(bytes, skip_whitespace_and_comments(bytes, pos + 1)) => {
            let digits = skip_whitespace_and_comments(bytes, pos + 1);
            grade.prev = PrevToken::Operand;
            Some(skip_glued_member_tail(
                bytes,
                skip_numeric_literal(bytes, digits),
            ))
        }
        b'+' | b'-' => None,
        // Everything else continues a type or is inert to it: `,` separates parameters, a
        // lone `?` marks an optional one or opens a conditional type's branch, `<` `>` carry
        // a nested argument list, `!` marks a JSDoc non-nullable, `=>` an arrow's head, and a
        // shift is what a type grammar re-reads as one of those.
        _ => Some(pos + 1),
    }
}

/// Whether what stands at `next`, just past the word `class`, makes it a class EXPRESSION:
/// a body, a binding name, or a heritage clause. A bare `class` is left to the caller's
/// ordinary reading, where acorn-typescript takes it for a type name (`f<(class)>(x)`).
#[inline]
fn class_expression_follows(bytes: &[u8], next: usize) -> bool {
    bytes.get(next) == Some(&b'{') || identifier_starts_at(bytes, next)
}

/// Whether an OPERAND begins at `pos` — an identifier or a literal. A prefix operator word
/// is only an operator where one does: `(await)` and `(await.T)` name a type, where
/// `(await b)` is the expression that refuses ([`OPERATOR_WORDS`]).
#[inline]
fn operand_starts_at(bytes: &[u8], pos: usize) -> bool {
    matches!(bytes.get(pos), Some(b'0'..=b'9' | b'\'' | b'"' | b'`'))
        || identifier_starts_at(bytes, pos)
}

/// The ASSIGNMENT operator spelled at `pos`, if any — a bare `=` or any compound form
/// (`+=`, `**=`, `>>>=`, `&&=`, `??=`, …) — as the byte past it.
///
/// Read ahead of every refusal in [`grade_body_token`], because the two forms part there: a
/// bare `=` is a parameter default, which tsc's `isUnambiguouslyStartOfFunctionType` claims
/// the whole group for, while no parameter default spells `+=`, so a compound one refuses.
/// Reading them together is also what keeps a compound operator from being taken one byte at
/// a time — `a &&= b` must not reach the `&&` arm, and `a >>= b` not the `>=` one.
#[inline]
fn assignment_operator_end(bytes: &[u8], pos: usize) -> Option<usize> {
    // Longest first, so `>>=` is not read as `>` + `>=` and `**=` not as `*` + `*=`.
    const COMPOUND_LEADS: &[&[u8]] = &[
        b">>>", b"**", b"<<", b">>", b"&&", b"||", b"??", b"+", b"-", b"*", b"/", b"%", b"&", b"|",
        b"^",
    ];
    let lead = COMPOUND_LEADS
        .iter()
        .find(|lead| bytes[pos..].starts_with(lead))
        .map_or(0, |lead| lead.len());
    let eq = pos + lead;
    // `==` / `===` are equality and `=>` an arrow; a relational `>=` / `<=` reaches here
    // with no lead matched and fails the `=` test on its own first byte.
    (bytes.get(eq) == Some(&b'=') && !matches!(bytes.get(eq + 1), Some(b'=' | b'>')))
        .then_some(eq + 1)
}

/// Whether the type-operator keyword `op`, spelled at `kw_start`, opens type arguments.
///
/// Each operator takes a different operand class, measured against acorn's own type
/// parser, so this dispatches on the operator's IDENTITY rather than re-deriving it from
/// the keyword's first byte — a new [`TypeOperator`] then fails to compile here instead
/// of silently inheriting whichever arm a byte match happened to fall into.
///
/// No operand ⇒ nothing commits: a bare operator keyword is never a complete type, and
/// (unlike an atom) cannot head a qualified type name either — acorn reads
/// `p < keyof.a > (t, u)` as a comparison on the VALUE `keyof.a` and `p < keyof > (t, u)`
/// as one on the value `keyof`. All four contextual operators are ordinary names there;
/// the reserved `typeof`'s no-operand shapes are errors in both readings.
fn type_operator_grade(
    bytes: &[u8],
    op: TypeOperator,
    kw_start: usize,
    region: Region,
    scan: TypeArgScan,
) -> HeadGrade {
    let after_kw = skip_whitespace_and_comments(bytes, skip_identifier(bytes, kw_start));

    // The coarse reading of an index ([`Region`]) takes `keyof` and `typeof` — an operator
    // of the expression grammar too, in the second case — for the rest of the body as it
    // stands, and reads the three contextual operators as the plain names an expression
    // spells with them (`b[infer - 1]`, `b[readonly]`).
    if region.is_coarse_under(scan) {
        return match op {
            TypeOperator::Keyof | TypeOperator::Typeof => HeadGrade::Rest,
            // `unique` takes any type operand in both parsers' grammars, so with one
            // behind it the body is a type to every reader (`b[unique - 1]`).
            TypeOperator::Unique if can_start_type_operand(bytes, after_kw) => HeadGrade::Rest,
            TypeOperator::Unique | TypeOperator::Infer | TypeOperator::Readonly => {
                check_identifier_type_arg_pattern(bytes, kw_start, WordHead::Name, region, scan)
            }
        };
    }

    match op {
        // acorn speculatively parses ANY type operand (`p < keyof - 1 > `t`` and
        // `p < unique [0] > (t, u)` are instantiations), so once an operand can start,
        // only the closing-`>` scan decides. Inside a nested region there is no such
        // scan and the keyword is as readily a callee (`b[keyof(c, d)]`), so the operand
        // is graded like any other.
        TypeOperator::Keyof | TypeOperator::Unique => {
            if !can_start_type_operand(bytes, after_kw) {
                return HeadGrade::Decided(false);
            }
            match region {
                Region::List => {
                    HeadGrade::Decided(scan_for_closing_angle_bracket(bytes, kw_start, scan))
                }
                Region::Nested => HeadGrade::Prefix(after_kw),
            }
        }

        // The operand is an entity name (`x`, `Ns.x`) or an `import('m')` head with a
        // member tail, and nothing else (`p < typeof 1 > (t, u)` and
        // `p < typeof [0] ? q : r > `t`` are comparisons — `typeof` takes any
        // *expression* operand but only an entity-name *type* operand). Skip the operand
        // and ask the shared follow filter, so `p < typeof x ? q : r > `t`` stays a
        // comparison while `f<typeof x>()` commits.
        //
        // The EXPRESSION `typeof` takes a parenthesized operand, whose shell the printer
        // strips (`typeof (b)` prints as `typeof b`, a type query's spelling), so the
        // reading of the printed form steps over one ([`skip_stripped_shells`]).
        TypeOperator::Typeof => {
            let after_kw = if scan.reads_source_as_written() {
                after_kw
            } else {
                skip_stripped_shells(bytes, skip_identifier(bytes, kw_start))
            };
            if !identifier_starts_at(bytes, after_kw) {
                return HeadGrade::Decided(false);
            }
            let head_end = skip_identifier(bytes, after_kw);
            let mut kind = OperandEnd::Name;
            let after_head = if is_word_at(bytes, after_kw, b"import") {
                let paren = skip_whitespace_and_comments(bytes, head_end);
                if bytes.get(paren) == Some(&b'(') {
                    kind = OperandEnd::ImportQuery;
                    match matching_delimiter_close(bytes, paren) {
                        Some(close) => close + 1,
                        None => return HeadGrade::Decided(false),
                    }
                } else {
                    head_end
                }
            } else {
                head_end
            };
            HeadGrade::Operand {
                end: skip_qualified_tail(bytes, after_head),
                kind,
            }
        }

        // The operand is a lone binding identifier (never qualified). Skip it and ask the
        // shared follow filter: a constraint commits through the filter's `extends` arm
        // (`f<infer T extends U ? A : B>()`), while `p < infer - 1 > `t`` — no identifier
        // at all — is a comparison on the value `infer`.
        TypeOperator::Infer => {
            if identifier_starts_at(bytes, after_kw) {
                HeadGrade::Operand {
                    end: skip_identifier(bytes, after_kw),
                    kind: OperandEnd::Other,
                }
            } else {
                HeadGrade::Decided(false)
            }
        }

        // The operand is an array/tuple type: an element type reference (`readonly T[]`,
        // `readonly Ns.T[]`), a tuple literal (`readonly [T, U]`), or a parenthesized
        // element type whose postfix run ends in an array suffix `[]` (`readonly (A | B)[]`).
        // Otherwise a parenthesized operand is no array or tuple, so acorn-typescript
        // refuses the type and reads a call in a comparison (`p < readonly (x) > (t, u)`,
        // `a < readonly (d)[e] > (f)`); a literal operand is no type at all. Skip the
        // operand and ask the shared follow filter — its indexed arm is what tells
        // `readonly zz[] ? q : r` (a comparison) from `f<readonly zz[]>()`.
        TypeOperator::Readonly => {
            // A type query heads the element type (`readonly typeof b[]`), and is graded
            // as one from there — where the array suffix follows it, which is what
            // `readonly` stands over ([`type_query_array_follows`]). The other operators
            // take the array into their own operand (`readonly keyof A[]` is
            // `readonly (keyof A[])`), which is no array or tuple.
            if is_word_at(bytes, after_kw, b"typeof") && type_query_array_follows(bytes, after_kw) {
                return HeadGrade::Prefix(after_kw);
            }
            let after_head = if identifier_starts_at(bytes, after_kw) {
                // An element type reference, which takes an argument list of its own
                // (`readonly A<B>[]`).
                return HeadGrade::Operand {
                    end: skip_qualified_tail(bytes, skip_identifier(bytes, after_kw)),
                    kind: OperandEnd::Name,
                };
            } else if bytes.get(after_kw) == Some(&b'[') {
                match matching_delimiter_close(bytes, after_kw) {
                    Some(close) => close + 1,
                    None => return HeadGrade::Decided(false),
                }
            } else if bytes.get(after_kw) == Some(&b'(') {
                let Some(close) = paren_type_head_close(bytes, after_kw) else {
                    return HeadGrade::Decided(false);
                };
                // The same-line postfix run after the `)` must END in an empty `[]`: only
                // then is the operand an array type. `readonly (d)[e]` is an indexed access,
                // which acorn-typescript refuses under `readonly`, reading the call
                // `readonly(d)[e]` in a comparison instead; `readonly (d)[e][]` is an array.
                // The run's indices are left for the follow filter to grade, from `close + 1`.
                let mut end = close + 1;
                let mut ends_in_array_suffix = false;
                loop {
                    let open = skip_whitespace_and_comments(bytes, end);
                    if bytes.get(open) != Some(&b'[')
                        || has_line_terminator_between(bytes, end, open)
                    {
                        break;
                    }
                    let Some(group_close) = matching_delimiter_close(bytes, open) else {
                        return HeadGrade::Decided(false);
                    };
                    ends_in_array_suffix =
                        skip_whitespace_and_comments(bytes, open + 1) == group_close;
                    end = group_close + 1;
                }
                if !ends_in_array_suffix {
                    return HeadGrade::Decided(false);
                }
                close + 1
            } else {
                return HeadGrade::Decided(false);
            };
            HeadGrade::Operand {
                end: after_head,
                kind: OperandEnd::Other,
            }
        }
    }
}

/// Whether a word heading an operand is a keyword TYPE or an ordinary name — the one
/// thing [`check_identifier_type_arg_pattern`] needs of the dispatch that reached it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WordHead {
    /// An atom keyword (`string`, `null`, …): a complete type by itself, which becomes a
    /// reference only where a qualified tail follows it (`string.x`).
    Keyword,
    /// Any other identifier: a type reference.
    Name,
}

/// Grade the operand headed by the identifier at `pos`.
///
/// Leading keywords that introduce a *non-reference* type are handled first:
/// - `import('m').T` — an import type: the specifier's group, then a qualified tail, and
///   a name like any other from there. An `import` with no `(` behind it is the
///   meta-property's head or nothing at all, never a type.
///
///   A specifier that is no STRING literal is where the two parse oracles part, as they
///   do on the non-null `!` ([`TypeArgScan`]): tsc's import type takes any type there and
///   leaves the string rule to its checker, while acorn-typescript's takes a string alone
///   and reads everything else as a dynamic import in a comparison: `f<import(c)>(x)`
///   and `a < b | import(c)[K] > (d)` are generic calls to the compiler and chains to
///   acorn-typescript, and no printed form keeps both readings. So at the list's own
///   level such a head keeps the region CLAIMED wherever the closing-`>` scan finds its
///   end — the meta-property's head (`import.meta`) with it — and the type parse rejects
///   the specifier; where the follower refuses the list the chain is every parser's
///   (`a < import(b) > c`). The reading of the PRINTED form, which answers for the
///   compiler too, makes the same list-level claim over every import head, so a chain
///   takes its pair (`(a < import(b)) > c`).
/// - `new (…) => R` / `abstract new (…) => R`, each also generic (`new <T>(…) => R`) — a
///   construct-signature type; the `(…) =>` shape distinguishes it from a `new Foo()`
///   value expression (which stays a comparison). At the list's level the closing-`>`
///   scan then confirms the close and the follow token; inside a nested region the
///   return is graded ([`HeadGrade::Prefix`]).
///
/// Otherwise the leading word is a type reference: the full qualified name is scanned
/// (e.g., `Ns.Type.Sub`) and the operand ends there, a name that takes an argument list
/// of its own ([`OperandEnd::Name`]) — unless the word is a bare keyword type
/// ([`OperandEnd::Keyword`]).
// TODO: inside an index the non-string import specifier is read by neither scan, like the
// `!` there ([`skip_relex_operand_suffixes`]): `a < b[import(c)] > d` prints bare, and tsc
// claims that region past a line break.
fn check_identifier_type_arg_pattern(
    bytes: &[u8],
    pos: usize,
    word: WordHead,
    region: Region,
    scan: TypeArgScan,
) -> HeadGrade {
    // The leading identifier's end is located once and reused by the keyword
    // dispatch below and by the qualified-name loop's first step.
    let end = skip_identifier(bytes, pos);

    // A member access settles an index for the coarse reading ([`Region`]), whatever the
    // word is — a name's (`b[c.d()]`) or a meta-property's (`b[import.meta]`,
    // `b[new.target]`).
    if region.is_coarse_under(scan)
        && bytes.get(skip_whitespace_and_comments(bytes, end)) == Some(&b'.')
    {
        return HeadGrade::Rest;
    }

    // Leading keyword forms that start a non-reference type. `new`/`abstract new` require
    // the construct shape so `f<new B()>(x)` and `a < new B() > (c)` stay comparisons.
    match &bytes[pos..end] {
        b"import" => {
            // The reading of the printed form answers for tsc too, whose import type takes
            // any specifier and whose list parse then claims whatever follows it: at the
            // list's own level it keeps the claim the closing-`>` scan makes.
            if !scan.reads_source_as_written() && region == Region::List {
                return HeadGrade::Decided(scan_for_closing_angle_bracket(bytes, pos, scan));
            }
            let paren = skip_whitespace_and_comments(bytes, end);
            let is_import_type = bytes.get(paren) == Some(&b'(')
                && matches!(
                    bytes.get(skip_whitespace_and_comments(bytes, paren + 1)),
                    Some(b'\'' | b'"')
                );
            if !is_import_type {
                // A dynamic import (`import(c)`), the meta-property's head (`import.meta`),
                // or nothing at all. Inside an index it is the expression it looks like; at
                // the list's own level the region stays claimed wherever it closes, as
                // tsc's import type claims it, and the type parse rejects it.
                return HeadGrade::Decided(
                    region == Region::List && scan_for_closing_angle_bracket(bytes, pos, scan),
                );
            }
            return matching_delimiter_close(bytes, paren).map_or(
                HeadGrade::Decided(false),
                |close| HeadGrade::Operand {
                    end: skip_qualified_tail(bytes, close + 1),
                    kind: OperandEnd::ImportName,
                },
            );
        }
        b"new" => {
            return match (construct_type_arrow_end(bytes, pos), region) {
                (None, _) => HeadGrade::Decided(false),
                (Some(_), Region::List) => {
                    HeadGrade::Decided(scan_for_closing_angle_bracket(bytes, pos, scan))
                }
                (Some(return_type), Region::Nested) => HeadGrade::prefix(bytes, return_type),
            };
        }
        b"abstract" => {
            let after = skip_whitespace_and_comments(bytes, end);
            match (construct_type_arrow_end(bytes, after), region) {
                (Some(_), Region::List) => {
                    if scan_for_closing_angle_bracket(bytes, pos, scan) {
                        return HeadGrade::Decided(true);
                    }
                }
                (Some(return_type), Region::Nested) => {
                    return HeadGrade::prefix(bytes, return_type);
                }
                // A bare `abstract` is an ordinary type reference — fall through.
                (None, _) => {}
            }
        }
        // A class EXPRESSION, never a type reference: without this the heritage clause's
        // `extends` reads as a constraint and commits (`a < class extends B {} > (t, u)`).
        b"class" if class_expression_follows(bytes, skip_whitespace_and_comments(bytes, end)) => {
            return HeadGrade::Decided(false);
        }
        // An assertion predicate's head (`(a) => asserts a`, `asserts this is B`), where a
        // function type's return is graded: a name on the keyword's own line is the
        // predicate's subject, which the walk grades next — and the `is` behind it, where
        // there is one. No expression spells two names in a row, so the reading takes
        // nothing from a comparison chain.
        b"asserts" if region == Region::Nested && scan.reads_source_as_written() => {
            let subject = skip_whitespace_and_comments(bytes, end);
            if identifier_starts_at(bytes, subject)
                && !has_line_terminator_between(bytes, end, subject)
            {
                // The subject ends the predicate unless an `is` follows it: anything else
                // behind it (`asserts a | B`) is no predicate the type grammar spells.
                let after = skip_whitespace_and_comments(bytes, skip_identifier(bytes, subject));
                if matches!(bytes.get(after), Some(b']' | b')')) || is_word_at(bytes, after, b"is")
                {
                    return HeadGrade::Prefix(subject);
                }
            }
        }
        _ => {}
    }

    // An arrow function's bare-name parameter, which the printer wraps: `b[x => y]` prints
    // as `b[(x) => y]`, a function type's head ([`type_arg_head_grade`]'s `(` arm). The
    // source spelling is no type — a function type's parameter list is parenthesized — so
    // only the reading of the PRINTED form takes it, and only where an arrow function can
    // stand at all.
    if region == Region::Nested && !scan.reads_source_as_written() {
        let arrow = skip_whitespace_and_comments(bytes, end);
        if bytes[arrow.min(bytes.len())..].starts_with(b"=>") {
            return HeadGrade::prefix(bytes, arrow + 2);
        }
    }

    // Skip any qualified parts past the leading identifier (already located as
    // `end`, e.g. `Namespace.Type.SubType`); the shared follow filter decides from there.
    let tail = skip_qualified_tail(bytes, end);
    HeadGrade::Operand {
        end: tail,
        kind: if word == WordHead::Keyword && tail == end {
            OperandEnd::Keyword
        } else {
            OperandEnd::Name
        },
    }
}

/// Classify the TypeScript type keyword at `pos`, if any.
///
/// Called on every `<`/`<<` disambiguation in the postfix loop, so ordinary
/// relational comparisons (`i < n`) and shifts hit it — keep it cheap. A first-byte
/// `match` dispatches to only the same-initial-letter candidate(s), so a byte that
/// can't begin any of the 20 keywords (a digit, `(`, `[`, a quote, or an identifier
/// starting with one of the other 14 letters) bails in O(1) instead of scanning all
/// 20. No keyword is a prefix of another, so at most one can match at a position.
///
/// The [`TypeKeywordKind`] split is what the caller dispatches on: an **atom** is a
/// complete type by itself and takes the identifier arm's follow-token filter (a
/// bare closing-`>` scan over-rejected ``p < string ? q : r > `t` `` — `?` cannot
/// continue a type, so the line is a conditional to acorn); an **operator** is
/// keyword-then-operand, where that filter's `_ => false` default would reject
/// `f<typeof x>()`, so its arm asks a per-operator operand question instead (see
/// [`type_operator_grade`]).
///
/// The whole-word test is [`is_word_at`], the same one every other keyword lookahead
/// asks. A hand-rolled boundary gets two character classes wrong in the same
/// direction: `$` read as *ending* the word (so `string$` matches `string`), and
/// every byte `>= 0x80` too (so `stringµ` does). Either way an
/// ordinary identifier went down the keyword arm rather than the identifier arm's
/// filter [`check_identifier_type_arg_pattern`].
///
/// ⚠️ A mis-dispatch only bites where the token past the would-be closing `>` does
/// **not** start an expression, because an expression-starting one disqualifies the
/// type-argument reading in *both* arms. So `a < string$ ? b : c > d` parsed fine
/// even with the bad boundary, while ``a < string$ ? b : c > `t` `` and
/// `a < string$ ? b : c > (d, e)` were parse errors — a case that reaches only one
/// arm is the only case a mis-dispatch can be observed through, and the fixtures pin
/// those rather than the inert form:
/// [less_than_keyword_boundary](../../../../tests/fixtures/typescript/syntax/disambiguation/less_than_keyword_boundary/)
/// (the word boundary),
/// [less_than_keyword_follow](../../../../tests/fixtures/typescript/syntax/disambiguation/less_than_keyword_follow/)
/// (the atom/operator follow-token split).
#[inline]
fn type_keyword_at(bytes: &[u8], pos: usize) -> Option<TypeKeywordKind> {
    use TypeKeywordKind::{Atom, Operator, This};
    use TypeOperator::{Infer, Keyof, Readonly, Typeof, Unique};
    // Full keyword match at `pos`, not part of a longer identifier.
    let kw = |k: &[u8]| is_word_at(bytes, pos, k);
    match bytes.get(pos) {
        Some(b'n') if kw(b"never") || kw(b"number") || kw(b"null") => Some(Atom),
        Some(b's') if kw(b"string") || kw(b"symbol") => Some(Atom),
        Some(b'b') if kw(b"boolean") || kw(b"bigint") => Some(Atom),
        Some(b'a') if kw(b"any") => Some(Atom),
        Some(b'u') if kw(b"unknown") || kw(b"undefined") => Some(Atom),
        Some(b'u') if kw(b"unique") => Some(Operator(Unique)),
        Some(b'v') if kw(b"void") => Some(Atom),
        Some(b'o') if kw(b"object") => Some(Atom),
        Some(b't') if kw(b"this") => Some(This),
        Some(b't') if kw(b"true") => Some(Atom),
        Some(b't') if kw(b"typeof") => Some(Operator(Typeof)),
        Some(b'f') if kw(b"false") => Some(Atom),
        Some(b'k') if kw(b"keyof") => Some(Operator(Keyof)),
        Some(b'i') if kw(b"infer") => Some(Operator(Infer)),
        Some(b'r') if kw(b"readonly") => Some(Operator(Readonly)),
        _ => None,
    }
}

/// How a TypeScript type keyword heads a type, which decides the follow-token
/// question [`Parser::is_type_arguments_start`] asks after matching one.
#[derive(Clone, Copy)]
enum TypeKeywordKind {
    /// A complete type by itself (`string`, `null`, `true`, …) — equally a value
    /// name, so it takes the identifier arm's follow-token filter.
    Atom,
    /// `this` — an [`Atom`](TypeKeywordKind::Atom) that additionally cannot head a
    /// qualified type name. Split out rather than re-tested at the use site: the
    /// classifier already matched the word, and asking again by prefix is the same
    /// re-derivation [`TypeOperator`] exists to avoid.
    This,
    /// A keyword-then-operand type head — the follow token is its operand, so the
    /// identifier filter's `_ => false` default cannot apply; [`type_operator_grade`]
    /// asks the operand-shape question this operator answers to.
    Operator(TypeOperator),
}

/// The five keyword-then-operand type heads. Carried by [`TypeKeywordKind::Operator`]
/// rather than re-derived from the keyword's first byte, so the operand-class dispatch in
/// [`type_operator_grade`] is exhaustive: the byte form needed `u`-initial operators to
/// be only `unique` (`unknown`/`undefined` being atoms) and gave `readonly` the catch-all
/// arm, which a sixth operator would have joined silently.
#[derive(Clone, Copy)]
enum TypeOperator {
    Keyof,
    Unique,
    Typeof,
    Infer,
    Readonly,
}

/// Skip the qualified tail of a name — the `.Seg` chain of `Ns.Type.Sub` — starting just
/// past the head identifier, returning the name's own end: the byte after its last
/// segment, ahead of any trivia (every caller hands it to [`type_arg_head_commits`]'s walk,
/// which skips the trivia itself and must see it to gate a `[` on the name's line). The
/// `typeof` and `readonly` operator arms walk their entity-name operands with the same
/// steps.
///
/// `end` advances only over a COMPLETE `.Ident` segment, so "just past a name" holds by
/// construction. A `.` with no identifier behind it ends no name — `TypeName` is
/// `IdentifierReference | NamespaceName . IdentifierReference` — so it is left where it
/// is, for the caller's follow check to read as the token it is. tsc consumes such a `.`
/// (`parseOptional(DotToken)`, then a synthesized missing identifier), but that is its
/// error RECOVERY, which a non-recovering lookahead has no use for: the same `.` ends no
/// EXPRESSION either (`MemberExpression . IdentifierName`), so every input that reaches
/// this branch is rejected on both readings and the two spellings cannot disagree.
fn skip_qualified_tail(bytes: &[u8], after_head: usize) -> usize {
    // The name's own end — never the trivia past it, which the follow filter skips
    // itself and reads for a line terminator ahead of a `[`.
    let mut end = after_head;
    loop {
        let dot = skip_whitespace_and_comments(bytes, end);
        if bytes.get(dot) != Some(&b'.') {
            return end;
        }
        let after_dot = skip_whitespace_and_comments(bytes, dot + 1);
        if !identifier_starts_at(bytes, after_dot) {
            return end;
        }
        end = skip_identifier(bytes, after_dot);
    }
}

/// Whether the token at `pos` can BEGIN a type — the same head classes
/// [`Parser::is_type_arguments_start`]'s own dispatch admits: an identifier or keyword,
/// a group or literal opener, a leading `|`/`&`, or a numeric sign or point. The
/// `keyof`/`unique` arm peeks this to tell `f<keyof U>()` (an operand follows — the
/// closing-`>` scan decides) from `p < keyof ? q : r` (no operand can start at `?`, so
/// `keyof` is an ordinary value name and nothing can commit).
///
/// ⚠️ The class must stay the dispatch's own. A head admitted there and refused here
/// reads the same literal two ways, and the second reading is silent: it was what left
/// `f<keyof .5>()` an over-rejection while `f<.5>()` and `f<keyof -1>()` — the same
/// literal one byte apart, and the same literal one operator apart — both parsed. The
/// numeric class is [`numeric_literal_starts_at`] for that reason, and its `.` gate is
/// load-bearing HERE specifically: this is the one site where a `.` has a live second
/// reading.
fn can_start_type_operand(bytes: &[u8], pos: usize) -> bool {
    match bytes.get(pos) {
        Some(b'(' | b'<' | b'{' | b'[' | b'\'' | b'"' | b'`' | b'|' | b'&') => true,
        Some(_) => numeric_literal_starts_at(bytes, pos) || identifier_starts_at(bytes, pos),
        None => false,
    }
}

/// Whether a numeric literal begins at `pos` — the head test the type-argument
/// lookahead's numeric sites share: the dispatch's own literal arm (the list's first
/// operand, a member and an index's body alike), a `keyof`/`unique` operand
/// ([`can_start_type_operand`]), and a parenthesized head's signed literal
/// ([`grade_body_token`]).
///
/// ⚠️ A `.` opens a literal only where a DIGIT follows it. Anywhere else the `.` is a
/// member tail on whatever precedes it, and at an operator keyword BOTH readings are
/// live: `f<keyof .5>()` is an instantiation over the literal type `.5`, while
/// `p < keyof.a > (t, u)` is a COMPARISON on the value `keyof.a` — an operator keyword
/// cannot head a qualified type name. Admitting a bare `.` there commits the lookahead
/// and turns that comparison into a parse error, which is why the sites ask one
/// predicate rather than a byte class each:
/// [less_than_keyword_member_prettier_divergence](../../../../tests/fixtures/typescript/syntax/disambiguation/less_than_keyword_member_prettier_divergence/)
/// pins the comparison,
/// [less_than_numeric_leading_dot](../../../../tests/fixtures/typescript/syntax/disambiguation/less_than_numeric_leading_dot/)
/// the instantiation.
///
/// The sign is admitted unconditionally, as every arm's `-` handling always has: `-b` is
/// a unary negation, and [`skip_numeric_literal`] reports that by skipping nothing, which
/// each caller checks (`after > pos`).
#[inline]
fn numeric_literal_starts_at(bytes: &[u8], pos: usize) -> bool {
    match bytes.get(pos) {
        Some(b'0'..=b'9' | b'-') => true,
        Some(b'.') => matches!(bytes.get(pos + 1), Some(b'0'..=b'9')),
        _ => false,
    }
}

/// Where the token that follows an operand stands, and what the walk stepped over to reach
/// it — [`skip_relex_operand_suffixes`]'s answer.
#[derive(Clone, Copy)]
struct OperandSuffix {
    /// The first byte the reading does not step over.
    token: usize,
    /// Whether the walk stepped over paren shells and nothing else — at least one `)`, no
    /// `!`. Behind such a run the printed operand is the shell's bare content, so a `.Seg`
    /// continues the qualified name the content ended on (`(b).c` prints as `b.c`), where
    /// behind a `!` it is a member access on a non-null value.
    shell_only: bool,
}

/// Step past what stands between an operand and the token that settles the region, but is
/// invisible to the reading that grades the PRINTED form. Two bytes qualify, and the loop
/// alternates because either can wrap the other (`(b)!`, `(b!)`, `b!!`, `((b))`):
///
/// - a **`)` closing a paren SHELL the printer strips** — not in the graded form, so a `)`
///   the region did not open neither ends an operand nor ends the region (`(a < b) > c`
///   prints as `a < b > c`, whose `<`…`>` region IS a type-argument list). Without it the
///   pair this reading justifies would erase itself on the next pass, since tsv's own
///   output is the shell-bearing spelling.
/// - a **non-null `!`** — `T!` is tsc's JSDoc non-nullable type, so `<b!>` is a
///   type-argument list to the compiler while acorn-typescript, tsv's parse oracle, reads a
///   comparison. `!=` / `!==` are operators and stop the walk. Read at the list's own
///   level alone: a `!` inside an index ends the walk, there as under the other reading.
///
/// The identity under [`TypeArgScan::Parse`], where the same `)` ends a call or a group for
/// real — the one [`Frame::Shell`] closes on, inside an index — and the same `!` is a
/// non-null assertion on a VALUE.
// TODO: the JSDoc `!` is read at the list's level and not inside an index, so
// `a < b[c!] > d` prints bare where `a < b! > d` takes a pair. Both spellings are a region
// tsc claims past a line break; the index half is an output whose tsc reading width alone
// can move.
#[inline]
fn skip_relex_operand_suffixes(
    bytes: &[u8],
    pos: usize,
    region: Region,
    scan: TypeArgScan,
) -> OperandSuffix {
    let mut suffix = OperandSuffix {
        token: pos,
        shell_only: false,
    };
    if scan.reads_source_as_written() {
        return suffix;
    }
    let mut non_null = false;
    loop {
        match bytes.get(suffix.token) {
            Some(b')') => suffix.shell_only = !non_null,
            Some(b'!') if region == Region::List && bytes.get(suffix.token + 1) != Some(&b'=') => {
                non_null = true;
                suffix.shell_only = false;
            }
            _ => return suffix,
        }
        suffix.token = skip_whitespace_and_comments(bytes, suffix.token + 1);
    }
}

/// Step from a prefix operator to its operand as the PRINTED form spells the pair: over
/// whitespace, which the printer glues away, and over the `(` of each paren shell around
/// the operand, which it strips (`-(1)` prints as `-1`, `typeof (b)` as `typeof b`). The
/// shell's `)` is [`skip_relex_operand_suffixes`]'s to step over, past the operand.
///
/// A COMMENT stops the walk, wherever it stands: the printer keeps a pair around a
/// commented operand (`- /* c */ 1` prints as `-(/* c */ 1)`), so what stands past one is
/// behind a `(` in the printed form and is no operand of the operator's own.
#[inline]
fn skip_stripped_shells(bytes: &[u8], pos: usize) -> usize {
    let mut pos = skip_whitespace(bytes, pos);
    while bytes.get(pos) == Some(&b'(') {
        pos = skip_whitespace(bytes, pos + 1);
    }
    pos
}

/// Skip a numeric literal with its optional sign — the type-argument lookahead's own
/// wrapper over [`skip_numeric_literal`], reporting "nothing skipped" (the position it was
/// given) where no literal begins, exactly as that function does.
///
/// It tolerates trivia between the sign and the digits, because the type grammar does: a
/// literal type's `-` is a token of its own in tsc and acorn-typescript alike, so
/// `f<- 1>(x)`, `f<A | -⏎1>(x)` and `f<A | - /* c */ 1>(x)` are generic calls to both.
///
/// **What [`TypeArgScan::Relex`] reads there is the printed EXPRESSION**, since that reading
/// is only ever asked of a `<` the parser took for a comparison — whose operand is a unary
/// minus, not a literal type. The printer glues whitespace away (`a < - 1 > d` prints its
/// operand as `-1`, a literal type's spelling, so the chain needs its pair on the first
/// pass) and strips a shell around the digits the same way (`-(1)`). A COMMENT in the gap is
/// the one thing it does not fold: `- /* c */ 1` prints as `-(/* c */ 1)`, where the `(`
/// behind the sign is no literal's, so that reading takes no literal across one — or the
/// pair it earned on the first pass would be gone on the second
/// ([`skip_stripped_shells`]).
///
/// A literal type takes ONE sign. What follows the first must be the digits (or a `.` that
/// opens them); a second sign is no literal, so `a < - -1 > (c)` and `f<--1>(x)` skip
/// nothing here and stay the expressions they are to the grammar.
#[inline]
fn skip_signed_numeric_literal(bytes: &[u8], pos: usize, scan: TypeArgScan) -> usize {
    let end = skip_numeric_literal(bytes, pos);
    if end > pos || bytes.get(pos) != Some(&b'-') {
        return end;
    }
    let digits = if scan.reads_source_as_written() {
        skip_whitespace_and_comments(bytes, pos + 1)
    } else {
        skip_stripped_shells(bytes, pos + 1)
    };
    if !matches!(bytes.get(digits), Some(b'0'..=b'9' | b'.')) {
        return pos;
    }
    let end = skip_numeric_literal(bytes, digits);
    if end == digits {
        return pos;
    }
    end
}

/// Step over a member tail GLUED to a negative literal's digits (`-1..x`, `-.5.x`,
/// `-1n.x`), from `end`, the literal's own end: the run of ASCII name bytes and dots that
/// opens on a `.` there. The identity where no `.` stands at `end`.
///
/// The tail is one more postfix acorn-typescript's EXPRESSION parser takes behind a
/// negative literal type ([`OperandEnd::SignedLiteral`]): `f<-1..x>(x)` is a generic call
/// over the literal type of `-(1..x)` to it and a comparison chain to tsc, and tsv has
/// neither tree to give — the chain would print as `f < -(1).x > x`, which
/// acorn-typescript reads as a comparison where it read a call. So the tail rides with
/// the literal: what follows it is read as it is behind the bare literal, a region that
/// reaches its close stays claimed, and the type parse rejects it. Inside an index that
/// holds only where the index opens as a type; in any other the walk ends on the tail
/// and the line is the comparison chain ([`IndexSplit`]).
///
/// Its readers: the literal at the list's own level or in an index
/// ([`negative_literal_end`]), the same literal inside a paren shell heading the list
/// ([`grade_body_token`]), and the grade of an index body's first operand
/// ([`index_operand_opens_type`]).
// TODO: at the list's own level the claim covers the glued spelling alone, as a byte run
// rather than a member grammar. The same literal ahead of a spaced member, a call's
// arguments or a template (`f<-1 .x>(x)`, `f<-1(e)>(x)`, ``f<-1`t`>(x)``) is read there as
// the comparison chain tsc reads, a tree acorn-typescript does not have; inside an index
// every spelling follows the index instead ([`index_opens_as_type`]). One rule should
// answer for every postfix in every position — claim them all, or follow tsc for all.
#[inline]
fn skip_glued_member_tail(bytes: &[u8], end: usize) -> usize {
    if bytes.get(end) != Some(&b'.') {
        return end;
    }
    let mut pos = end;
    while bytes
        .get(pos)
        .is_some_and(|&b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_'))
    {
        pos += 1;
    }
    pos
}

/// Where a negative literal ENDS as an operand of the walk, from `after`, the end of its
/// digits: past a member tail glued to them ([`skip_glued_member_tail`]) wherever the
/// form `scan` reads can hold one.
///
/// The source holds any. The printed form holds one behind a BigInt alone: the printer
/// writes a number's member on a parenthesized literal (`-(1).x`, no literal type to any
/// parser) and a BigInt's on the suffix itself (`-1n.x`) — from a member the source
/// spaced off too, so that reading steps over the space.
#[inline]
fn negative_literal_end(bytes: &[u8], after: usize, scan: TypeArgScan) -> usize {
    if scan.reads_source_as_written() {
        skip_glued_member_tail(bytes, after)
    } else if bytes[..after].ends_with(b"n") {
        let dot = skip_whitespace(bytes, after);
        let tail = skip_glued_member_tail(bytes, dot);
        if tail == dot { after } else { tail }
    } else {
        after
    }
}

#[cfg(test)]
mod tests {
    use super::{closes_statement_header, closes_type_arguments};

    /// Whether the LAST `)` in `src` closes a statement header, walking from byte 0.
    fn closes_header(src: &str) -> bool {
        let rparen = src.rfind(')').expect("a `)` to ask about");
        closes_statement_header(src.as_bytes(), rparen, 0)
    }

    #[test]
    fn a_statement_headers_paren_closes_a_header() {
        for src in [
            "if (c)",
            "x = () => { if (c)",
            "while (c)",
            "for (;;)",
            "for (a of b)",
            "for await (a of b)",
            "with (o)",
            "if (f(a))",
            "if /* x */ (c)",
            "if // x\n(c)",
            "if (a) if (b)",
            "if (/\\)/.test(s))",
            "if (/\\(/.test(s))",
            "if (/[)]/.test(s) && f(t))",
            "for (x of /\\)/g.exec(s))",
        ] {
            assert!(closes_header(src), "{src:?}");
        }
    }

    #[test]
    fn any_other_paren_closes_no_header() {
        for src in [
            "f(c)",
            "f()",
            "map.get(k)",
            "a.b()",
            "a.if(c)",
            "a?.while (c)",
            "iff (c)",
            "if (c) f(d)",
            "(a)",
        ] {
            assert!(!closes_header(src), "{src:?}");
        }
    }

    /// Whether the LAST `>` in `src` closes a type-argument list, scanning from byte 0.
    fn closes(src: &str) -> bool {
        let gt = src.rfind('>').expect("a `>` to ask about");
        closes_type_arguments(src.as_bytes(), gt, 0)
    }

    #[test]
    fn a_list_the_parser_takes_closes_at_its_gt() {
        for src in [
            "f<T>",
            "a.b<T>",
            "f<A<B>>",
            "f()<T>",
            "new C<T>",
            "f /* c */ <T>",
            "g(c)<T>",
            "a.if(c)<T>",
            "a.if /* c */ (c)<T>",
            // A misread regex after an earlier list's `>` hides nothing the ask needs.
            "x = f<T> / 2 + g<U>",
            "x = f<T> / 2 + g<U> / 3 + h<V>",
            "a. /* c */ if(c)<T>",
            "a?.while(c)<T>",
            "iff(c)<T>",
            "x = () => { if (c) f(d)<T>",
        ] {
            assert!(closes(src), "{src:?}");
        }
    }

    #[test]
    fn a_gt_the_parser_reads_as_an_operator_closes_nothing() {
        for src in [
            // A postfix update is no subscript base: the parser reads a comparison.
            "a++ < b >",
            "a-- < b >",
            // An operator position opens an angle-bracket assertion, not a list.
            "a && <T>",
            // A head the type grammar refuses: `&&` cannot continue a type.
            "a < b && c >",
            // A statement header's `)` ends no operand: a statement begins after it.
            "x = () => { if (c) <T>",
            "x = () => { if (c)!<T>",
            // A paren in a regex literal in the header is no header paren.
            "x = () => { if (/\\)/.test(s)) <T>",
            "x = () => { if (/\\(/.test(s)) <T>",
            "x = () => { while (c) <T>",
            "x = () => { for await (a of b) <T>",
            // Comments anywhere around the header are trivia.
            "x = () => { if /* x */ (c) /* y */ <T>",
            "x = () => { if // x\n(c) <T>",
            "x = () => { for /* a */ await /* b */ (a of b) <T>",
            "x = () => { with (o) <T>",
            "x = () => { if (a) if (b) <T>",
            // No `<` at all.
            "a >",
            "a >> b >",
        ] {
            assert!(!closes(src), "{src:?}");
        }
    }
}
