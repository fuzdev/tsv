// Comment handling for TypeScript printer
//
// This module handles all comment-related operations:
// - Building Doc representations for comments
// - Printing comments directly to buffer
// - Finding and filtering comments in ranges
// - Handling leading/trailing/inline comments
//
// ## Module Organization
//
// - **mod.rs** (this file): The `CommentSpacing` / `CommentFilter` enums and the
//   generic comment-emission primitives every other module builds on.
// - **render.rs**: Single-comment text-layout leaves (block-comment framing,
//   indentable / preserved block comments, trailing line/block comment docs).
// - **paren.rs**: Stripped-grouping-paren comment handling (promotion across `=`
//   / operators, trailing-paren comment preservation, removed-paren prepends).
// - **owned.rs**: The comment/paren binding seam — a comment glued to the token
//   after it is printed by the node that token begins, so a synthesized paren
//   can't land between the two (`Comment::owned_by_node`).
// - **scan.rs**: Pure source span-math helpers (comma/angle/blank-line scanning).
// - **declarations.rs**: Member-keyword / modifier-marker / marker→colon /
//   heritage / keyword→name comment emitters.
// - **lists.rs**: List- and body-level comment emitters (leading/trailing body
//   comments, delimiter-line prefixes, empty-container comments, comma emission).
// - **member_body.rs**: The `{ … }` member-body walk shared by the class body, the
//   interface body and both type-literal force-multiline walks — the member analog
//   of `build_statement_list_docs_into`.
// - **element_comma.rs**: The single source of the `trailingComma: 'none'`
//   comment-position contract for inline element lists (block-before / comma /
//   block-after-on-last / line-suffix), shared by the object/array pattern and
//   object-literal builders.

mod declarations;
mod element_comma;
mod lists;
mod member_body;
mod owned;
mod paren;
mod render;
mod scan;

pub(crate) use declarations::{ClassMemberModifiers, ContinuationValue, HeritageKeyword};
pub(super) use element_comma::{block_is_before_comma, next_real_element_start, run_defers_line};
pub(crate) use lists::{BlankRule, MemberGap, StandaloneGlue};
pub(crate) use member_body::{MemberBlankScan, MemberBody, MemberFloor, MemberFreeze, MemberSeam};
pub(crate) use owned::OwnedCommentEffect;
pub(crate) use paren::{ParenLeadingValue, paren_pair_keeps_leading_run, paren_shell_close_after};

// Re-export for submodules to use `super::X` instead of `super::super::X`.
pub(super) use super::{Printer, calls, layout};

use smallvec::SmallVec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::printing;
use tsv_lang::source_scan::has_newline_after_position;
use tsv_lang::{Comment, comments_to_emit_in_range};

/// Small stack-allocated vector of comment references. Inline capacity 8 keeps
/// the common comment gaps off the heap: 0–2 comments are the bulk, and a
/// stacked `//` block (3–8 lines, common in documented code) still fits inline;
/// comment-dense corpora put the p99 statement-gap run at 7 (`cargo run -p
/// tsv_debug --features buffer_stats buffer_sizes` — the histogram source for
/// this `N`). A larger run spills to a single heap alloc — exactly what a
/// `Vec` would do.
pub(crate) type CommentVec<'a> = SmallVec<[&'a Comment; 8]>;

/// Spacing style for comments in doc building
#[derive(Debug, Clone, Copy)]
pub(crate) enum CommentSpacing {
    /// Space before comment: ` /* c */`
    Leading,
    /// Space after comment: `/* c */ `
    Trailing,
    /// No spacing around the run: `/* c */`.
    ///
    /// ⚠️ This governs the run's **outer** edges only — comments *within* the run are
    /// still separated, or a multi-comment run fuses into `/* a *//* b */`. The caller
    /// picks `None` because it has already placed the anchor's space itself.
    None,
}

impl CommentSpacing {
    /// `Trailing` when followed by type params (`/* c */ <T>`),
    /// `Leading` when followed by parens (` /* c */()`).
    pub(crate) fn for_type_params(has_type_params: bool) -> Self {
        if has_type_params {
            Self::Trailing
        } else {
            Self::Leading
        }
    }
}

/// Filter for which comment types to include
#[derive(Debug, Clone, Copy)]
pub(crate) enum CommentFilter {
    /// Include all comments (block and line)
    All,
    /// Only include block comments (/* */)
    BlockOnly,
}

/// Whether an author blank line **above a comment run's first comment** survives.
///
/// The Trailing-vs-Leading question, at the one emitter two kinds of run share
/// ([`Printer::push_anchored_trailing_run`]). A **trailing** run's blank sits between the
/// value and the comment and prettier's `printTrailingComment` emits it
/// (`isPreviousLineEmpty`); a **leading** run's sits between the introducer — an operator,
/// a keyword, a delimiter — and the comment, where `printLeadingComment` has no emitter
/// for it at all, so it is dropped. tsv drops it at every other leading gap (a value's
/// `=`, an object `:`, a call argument, an array element, a `return` operand); the binary
/// operator→operand gap kept it only because it borrows the trailing emitter for its
/// anchor rule.
///
/// A blank *between* two comments is a different question, never this one: it separates
/// two distinct remarks and survives in both kinds of run.
///
/// The same split the control-flow gaps name as `GapCommentRun::blank_seed`
/// (`Some`/`None`) — one concept, stated per emitter because the two emitters take
/// different shapes of it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunLeadingBlank {
    /// A trailing run: the blank is the author's, between the value and its comment.
    Keep,
    /// A leading run: the blank sits above the run, where prettier cannot emit one.
    Drop,
}

/// The shape a **keyword→operand** gap takes — [`Printer::keyword_operand_gap`], whose
/// doc carries the rule. `new`→callee and `await`→operand are its two sites.
#[derive(Clone, Copy)]
pub(crate) enum KeywordOperandGap {
    /// A comment in the gap ENDS A LINE, so the operand cannot stay on the keyword's
    /// line: the caller emits its keyword bare and wraps its whole tail in
    /// `Printer::build_continuation_indent`.
    Continuation,
    /// Nothing in the gap ends a line: the run — `None` when the gap holds no comment to
    /// emit — trails the keyword inline, and the tail keeps the enclosing indent.
    Inline(Option<DocId>),
}

/// Who emits a retained paren SHELL's leading **line**-comment run — the one axis
/// [`Printer::push_paren_shell_leading_run`] and its callers part on.
///
/// An ownership question, so it is a named axis rather than a bare `bool`: a region two
/// emitters each believe the other owns is a DROP or a DOUBLE-PRINT, and neither is
/// visible to the print-once ledger when the two spellings sit in different modules
/// ([`comments.md`](../../../../docs/comments.md) hazards 1 and 3). One type means a call
/// site states which emitter owns the run in the same vocabulary the emitter reads it in.
///
/// The **block** arm is not on this axis: a block comment renders inline inside the shell
/// wherever the run is emitted, so every value emits it and only the `//` is claimed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellLeadingRun {
    /// An upstream emitter already placed the run — the union's own member path
    /// ([`Printer::build_union_type_doc_with_line_comments`]), the conditional check /
    /// `extends` callers (the stripped-shell relocation), and a nested-conditional branch
    /// whose run was relocated above the operator. Emitting here would double-print it.
    ///
    /// ⚠️ **Scope**: the axis reaches the parenthesized-**union** rendering only. The
    /// required pair around every other operand kind opens over its own leading run
    /// without consulting it, because the licence is granted on an upstream emitter
    /// EXISTING — true of an intersection's FIRST member and of a redundant member, false
    /// of every later retained one, where reading the flag left the run welded to the `(`
    /// ([`Printer::build_type_doc_maybe_parens_impl`]).
    Upstream,
    /// This site is the run's only emitter, so it renders inside the pair. True of an
    /// optional tuple element, which has no upstream at all: its run was simply DROPPED.
    Here,
}

/// How a leading-comment run decides whether a *block* comment hugs the token
/// that follows it (a trailing space, `/* c */ X`) rather than dropping to its
/// own line. The rest of the run is identical across sites — one
/// `build_comment_doc` per comment, and a `line`/`hardline` toward the next
/// comment (or the terminal) for every comment that doesn't hug — so only this
/// glue test varies, and [`push_leading_comment_run`](Printer::push_leading_comment_run)
/// takes it as a mode.
#[derive(Debug, Clone, Copy)]
pub(crate) enum LeadingGlue {
    /// A block hugs when it shares a source line with whatever follows it — the
    /// *next* comment, or the terminal for the last one. Prettier's rule: its
    /// `printLeadingComment` reads only the source right after the comment's `*/`
    /// (`hasNewline(text, locEnd(comment))`), never where the terminal starts, so
    /// a run the author glued together stays glued (`/* a */ /* b */⏎X` keeps the
    /// pair on one line and breaks before `X`).
    Adjacent,
    /// `Adjacent`, plus a single-line block glued to the operator hugs the value
    /// across a source newline — prettier's assignment/call pull-up
    /// ([`build_rhs_comments_glued_opt`](Printer::build_rhs_comments_glued_opt)).
    AdjacentGlued,
    /// `Adjacent`, plus a block whose only company on its line is a grouping paren the
    /// printer STRIPPED hugs across the newline that paren left behind — the nested
    /// JSDoc-cast shape `/** @type {A} */ (⏎/** @type {B} */ (expr))`, where the author
    /// glued the comment to a `(` that is not in the output, so
    /// [`Printer::comment_hugs_next`] alone reads the pair as broken and splits it.
    /// The call-argument leading run's mode, since paren stripping is what puts two casts
    /// in one gap.
    AdjacentStrippedParen,
    /// `Adjacent`, but an author **blank** line after a glued block's `*/` does not
    /// force the comment onto its own line — it yields with the soft `line` like a
    /// plain newline. The **value-gap** mode: the gap between a head (`=`, `:`, `as`,
    /// a keyword) and the value it introduces.
    ///
    /// The distinction is which break the blank belongs to. A glued block does not run
    /// to end-of-line, so nothing forces the value down and the break after `*/` is the
    /// author's — which tsv reflows at every value position
    /// ([conformance_prettier.md](../../../../docs/conformance_prettier.md) §Authored
    /// breaks in value position). A blank line is a property of a line break: collapse
    /// the break and there are no longer two lines for it to separate, so the blank
    /// yields with it. `Adjacent` keeps the opposite rule because *its* blank separates
    /// two list items, which is ordinary authoring tsv preserves (the
    /// `arrays/end_of_line_block_comment` divergence fixture pins it).
    ///
    /// Without this split the two families disagreed on whitespace *quantity*: at a
    /// value gap one newline collapsed and two hung, while ~20 peer gaps on
    /// `AdjacentGlued` collapsed both. An author who wants the blank kept writes the
    /// comment on its own line, where the break IS forced and it survives.
    AdjacentValueGap,
    /// `Adjacent`, but the run's **first** comment is emitted on the anchor's line by
    /// construction (the caller pushes a bare `" "` ahead of it, never a `line`), so the
    /// newline the author wrote *before* it is erased by this emitter's own output.
    /// Reading `is_own_line_comment` there is
    /// [rule-conditioned-on-what-its-own-output-destroys][docs]: it forces a `hardline`
    /// for a lone own-line block (`if (a)⏎/* c */⏎;`) that the very next pass collapses,
    /// because by then the comment sits on the `)` line — **non-idempotent**, and prettier
    /// needs two passes for exactly that reason. So the first comment answers "own-line"
    /// as **false** and takes the soft `line`, landing on prettier's fixed point in one
    /// pass; every later comment keeps the source reading, since the separator that put it
    /// on its own line reproduces it.
    ///
    /// The **empty-statement body** gap's mode (`) ;` / `else ;`), the one site that pins
    /// its run to the anchor's line.
    ///
    /// [docs]: ../../../../../docs/comments.md
    AdjacentAnchorLine,
}

impl LeadingGlue {
    /// Whether an author blank line after a glued block's `*/` forces it onto its own
    /// line (preserving the blank) rather than yielding with the soft `line`.
    fn blank_forces_own_line(self) -> bool {
        !matches!(self, Self::AdjacentValueGap)
    }

    /// Whether the run's FIRST comment shares the anchor's line in the output, which
    /// erases the newline the author wrote before it — see [`Self::AdjacentAnchorLine`].
    fn first_shares_anchor_line(self) -> bool {
        matches!(self, Self::AdjacentAnchorLine)
    }
}

impl<'a> Printer<'a> {
    /// Push one **block** comment with `spacing` applied to its outer edges — the single
    /// definition of what [`CommentSpacing`] means for a comment that does not end its
    /// line: ` /* c */` (`Leading`), `/* c */ ` (`Trailing`), or bare (`None`).
    ///
    /// `space_before` lets a `Leading` caller suppress the space when the run already
    /// sits at the start of a fresh (indented) line, where a space would render as a
    /// stray `\t /* c */`; a caller with nothing to suppress passes `true`.
    ///
    /// Shared by [`Self::format_block_comments`] (the chain gaps' block-only runs) and
    /// the block arm of [`Self::build_header_comment_run`]. A **line** comment has no
    /// spacing choice to make — it must end its line — so it never reaches here; that
    /// asymmetry is the whole content-loss hazard, and the one emitter that hand-rolls
    /// both halves ([`Self::build_comments_between_filtered_opt`]) documents it in place.
    pub(crate) fn push_block_comment_spaced(
        &self,
        parts: &mut DocBuf,
        comment: &Comment,
        spacing: CommentSpacing,
        space_before: bool,
    ) {
        debug_assert!(comment.is_block, "line comments have no spacing choice");
        let d = self.d();
        match spacing {
            CommentSpacing::Leading => {
                if space_before {
                    parts.push(d.text(" "));
                }
                parts.push(self.build_comment_doc(comment));
            }
            CommentSpacing::Trailing => {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            }
            CommentSpacing::None => parts.push(self.build_comment_doc(comment)),
        }
    }

    /// Whether a comment can't share a line with anything around it — any line comment
    /// (it runs to EOL), or a block comment with a newline on **both** sides in source:
    /// nothing before it on its line ([`Self::comment_follows_content_on_its_line`]) and
    /// nothing glued after it ([`Self::comment_hugs_next`]). An adjacency on either side
    /// keeps the comment inline (`a /* c */ b`), matching prettier, which collapses both
    /// `a,⏎/* c */ b` and `a /* c */,⏎b` back to the inline form. The shared rule behind
    /// the intersection member break gates, the import-attribute gaps, and the first-param
    /// leading-run collapse (`build_param_leading_comments_doc`).
    ///
    /// ⚠️ **Both halves read the SOURCE, and that is the whole predicate.** This is
    /// prettier's `printLeadingComment` hardline condition transcribed —
    /// `hasNewline(text, locEnd(comment)) && hasNewline(text, locStart(comment),
    /// {backwards: true})` — so it asks about physical newlines, never about where the
    /// neighboring *items* start and end. An item-boundary spelling
    /// (`is_same_line(prev, c.start)` / `is_same_line(c.end, next)`) is blind to every
    /// byte no item span covers — a list's comma, a stripped paren shell's `)`, an `&`
    /// operator, **and another comment in the same gap** — each of which reads as
    /// isolation that the author did not write. The last one is why the anchors could not
    /// be kept even where the gap holds no structure at all: at the import-attribute
    /// `:`→value gap, whose `prev` *is* the `:`, a glued run (`a:⏎/* c1 */ /* c2 */⏎'x'`)
    /// still mis-read. The union keeps its own one-sided rule
    /// ([`Self::is_own_line_comment`], no glue half) because prettier's union printer
    /// genuinely expands a block adjacent to its member where the intersection collapses
    /// it — see `union_has_own_line_member_comment`.
    ///
    /// The kind clause is the only thing this adds to
    /// [`Self::block_comment_owns_its_line`] (with an item following), which the
    /// list-expansion gates call directly because they filter on kind themselves.
    pub(crate) fn comment_isolated_on_its_line(&self, c: &Comment) -> bool {
        !c.is_block || self.block_comment_owns_its_line(c, true)
    }

    /// Whether a *block* comment is glued to what follows it (`/* c */ X` — nothing but
    /// spaces after its `*/`), so it leads that token inline instead of taking its own
    /// line. Prettier's leading-comment rule, asked exactly as `printLeadingComment` asks
    /// it: of the **source right after the comment** (`hasNewline(text, locEnd(comment))`),
    /// never of where the item it leads happens to start. So a run the author glued
    /// together stays glued (`/* a */ /* b */⏎X` → the pair shares a line, `X` starts a new
    /// one).
    ///
    /// ⚠️ **The two readings differ by exactly the structure a list re-emits.** With a
    /// comma between the comment and its item (`<T⏎/* c */, U // x>`), an
    /// `is_same_line(comment.end, item_start)` approximation says "the item is on the next
    /// line, so break" and drops the comment onto a line of its own — while the author's
    /// own line, and prettier's, ends *after* the comma. The array literal reached the same
    /// output by collapsing a soft `line` in its per-element group; the params family has
    /// no such group (`docs/comments.md` §Array family vs params family), so the glue test
    /// is the whole answer there and the two families disagreed about one authoring.
    ///
    /// The single statement of the rule. [`push_leading_comment_run`](Self::push_leading_comment_run)
    /// is the emitter for the sites whose surrounding loop is the shared one; a site
    /// whose separator policy genuinely differs (the union member's own-line run,
    /// which brackets the `| ` separator and preserves blanks in different positions)
    /// calls this directly rather than re-deriving it.
    pub(crate) fn comment_hugs_next(&self, comment: &Comment) -> bool {
        comment.is_block && !has_newline_after_position(self.source, comment.span.end)
    }

    /// Whether a **block** comment OWNS its line — the classification every list-EXPANSION
    /// gate applies, stated once. Four families spell the surrounding scan differently
    /// (what bounds the gap, what counts as an item, whether a multi-line block is in
    /// scope), and the one thing they must not spell differently is this.
    ///
    /// Both halves read the SOURCE, per comment: nothing before the `/*` on its line
    /// ([`Self::comment_follows_content_on_its_line`]) and nothing after the `*/` there
    /// ([`Self::comment_hugs_next`]) — prettier's `printLeadingComment` hardline condition,
    /// which is the only separator of its three that ends the line unconditionally. Every
    /// list here flattens when it fits, so the author's line break *around* the comma is
    /// layout, not own-line-ness — an item-boundary reading of either half calls a
    /// comma-glued comment own-line and reaches a third fixed point neither the bare
    /// authoring nor prettier produces (`docs/comments.md` §Own-line-ness is a SOURCE
    /// question).
    ///
    /// ⚠️ **A glued RUN given its own line does not own it.** In `<A,⏎/* c1 */ /* c2 */⏎B>`
    /// only `c1` has a newline before it and only `c2` has one after, so neither takes the
    /// hardline: `c1` glues to `c2` and `c2` takes the **soft `line`** that collapses when
    /// the list fits and breaks when it doesn't. Asking the RUN instead — "does the object
    /// the author glued end the line?" — is the reading this gate used to take, and it
    /// forces open a list prettier keeps flat at every one of these families (params, type
    /// params, type args, function-type params, call args, tuples, intersections, all
    /// measured). The soft `line` is what makes the two authorings one fixed point, and it
    /// lives in the emitters ([`Self::push_leading_comment_run`]); a gate that pre-empts it
    /// with a forced break is answering a question the emitter already answers better.
    ///
    /// `item_follows` is the caller's fact about the range it scanned, not a default: with
    /// no item left to lead, the comment is DANGLING and the container's closer sharing its
    /// line is not glue (`{ a: 1⏎/* c */ }`, `[a,⏎/* c */ ]` — prettier expands both). A
    /// gap bounded by the next item passes `true`, and the families whose trailing position
    /// belongs to a separate predicate ([`Self::has_own_line_block_comment_before_closer`])
    /// always do.
    ///
    /// The caller filters on kind first: a **line** comment forces every one of these lists
    /// open on its own, and each family says so in its own clause rather than here.
    /// [`Self::comment_isolated_on_its_line`] is this same question with that clause folded
    /// in, for the callers that don't pre-filter.
    pub(crate) fn block_comment_owns_its_line(
        &self,
        comment: &Comment,
        item_follows: bool,
    ) -> bool {
        !self.comment_follows_content_on_its_line(comment)
            && (!item_follows || !self.comment_hugs_next(comment))
    }

    /// Split a comment run into the ones that stay in the RUN and the ones
    /// **glued** to what follows ([`Self::comment_hugs_next`]) — for a site whose run and
    /// glued suffix take different separators, so it cannot hand the whole run to
    /// [`Self::push_leading_comment_run`] (which decides per comment) and must emit the two
    /// halves itself.
    ///
    /// The glued suffix leads the following token inline (`/* c */ for (…)`), where the
    /// author put it; the rest each take a line. The split is invisible wherever the
    /// following token heads an **expression** — `bind_leading_comment` is the only general
    /// binder, so the comment is then owned by that node, rides inside its doc and never
    /// reaches the gap — leaving only the **keyword** heads (`for` / `if` / `{`, a `for`
    /// clause's own head; a statement keyword binds nothing) and the positions where a
    /// freeze replaced the doc with a verbatim slice starting past the comment
    /// (docs/comments.md hazard 1).
    ///
    /// The caller owns the **break** before the suffix — this only says which comments it
    /// holds. A glued comment is still authored somewhere, and gluing it to what follows
    /// must not also move it to another line: at a header→body gap
    /// ([`Printer::push_header_to_body_gap`]) everything reaching the suffix sat below the
    /// anchor, so it takes a `hardline` even when the run before it is empty.
    ///
    /// ⚠️ **The glued set is a SUFFIX**, taken by walking back from the end while each
    /// comment still hugs. Per-comment it is not: `/* a1 */ /* a2 */⏎x` has a1 hugging a2
    /// and a2 hugging nothing, so a per-comment test puts a1 in the glued half and a2 in
    /// the run — two buckets emitted run-first, which prints the authored pair REVERSED.
    /// A comment glued to one that takes its own line takes that line with it.
    pub(crate) fn split_glued_comments(
        &self,
        comments: impl IntoIterator<Item = &'a Comment>,
    ) -> (CommentVec<'a>, CommentVec<'a>) {
        let mut run: CommentVec<'a> = comments.into_iter().collect();
        let mut split = run.len();
        while split > 0 && self.comment_hugs_next(run[split - 1]) {
            split -= 1;
        }
        let glued: CommentVec<'a> = run.drain(split..).collect();
        (run, glued)
    }

    /// Emit the glued suffix [`Self::split_glued_comments`] held back, immediately before
    /// the doc the caller pushes next: each comment plus the space that keeps it glued.
    pub(crate) fn push_glued_comment_run(&self, parts: &mut DocBuf, glued: &[&'a Comment]) {
        let d = self.d();
        for comment in glued {
            parts.push(self.build_comment_doc(comment));
            parts.push(d.text(" "));
        }
    }

    /// The separator after one comment in a per-line comment list, where an author
    /// blank line between `comment_end` and `next` (the following own-line comment, or
    /// the element the comments lead) **forces a `hardline`** and carries the blank as a
    /// leading `literalline`; without one the caller's `sep` stands.
    ///
    /// The single statement of "does an author blank survive here?" — always asked with
    /// [`Self::has_blank_line_between_strict`], never the table-only
    /// [`Self::has_blank_line_between`], whose newline count can be met by a delimiter
    /// the printer emits between two spans (see that method's rustdoc: reading one as a
    /// blank is a one-shot non-idempotency). A site that re-derives this pays that bug.
    pub(crate) fn push_blank_preserving_separator(
        &self,
        parts: &mut DocBuf,
        comment_end: u32,
        next: u32,
        sep: DocId,
    ) {
        let d = self.d();
        if self.has_blank_line_between_strict(comment_end, next) {
            parts.push(d.literalline());
            parts.push(d.hardline());
        } else {
            parts.push(sep);
        }
    }

    /// [`Self::push_blank_preserving_separator`] with the `hardline` separator — the
    /// blank-preserving counterpart to a bare `hardline`, for a run whose comments each
    /// take their own line unconditionally. The common case; a run whose non-blank
    /// separator is collapsible (a conditional branch's soft `line`) calls the general
    /// form.
    pub(crate) fn push_blank_preserving_hardline(
        &self,
        parts: &mut DocBuf,
        comment_end: u32,
        next: u32,
    ) {
        self.push_blank_preserving_separator(parts, comment_end, next, self.d().hardline());
    }

    /// The **list-element** sibling of [`Self::push_blank_preserving_hardline`]: same
    /// emission (`literalline` + `hardline` for an author blank, a bare `hardline`
    /// otherwise), different blank question.
    ///
    /// Here it is prettier's `isNextLineEmpty` ([`Self::is_next_line_empty`]), measured
    /// from the **element's own end** — which is what makes the *separator* and the
    /// element's same-line trailing comments trivia. Two consequences the range-based
    /// question cannot express, and both are authored shapes: a blank the author left
    /// before a comma they pushed onto its own line still counts (`a: 1⏎⏎// c⏎, b`), and
    /// one *after* such a comma does not (`a: 1⏎,⏎⏎b` — the rule
    /// `property_own_line_comma_blank` pins). Callers pass `content_start` = where this
    /// element's printed content begins (its first leading comment, else the element), so
    /// a blank ahead of that content is inside the measured range.
    ///
    /// The function-parameter list calls it directly, deriving its own `content_start`
    /// (decorators, the `(`-line pull); every other list goes through
    /// [`Self::push_item_blank_separator`], which derives that position once. A further
    /// list that needs the same rule should call one of the two rather than re-derive the
    /// two-line emission around a bare `is_next_line_empty`.
    ///
    /// The literal and the pattern are one entry, not two: prettier prints
    /// `ObjectExpression` and `ObjectPattern` through the same `printObject`, so a site
    /// that answers this question differently for one of them is a divergence by
    /// omission. Arrays are the deliberate exception — array literals and array patterns
    /// take prettier's *other* helper (`isLineAfterElementEmpty`, [`Self::has_blank_line_after_comma`]),
    /// which advances to the comma before measuring; see [`Self::is_next_line_empty`] for
    /// the table of where the two disagree.
    pub(crate) fn push_next_line_empty_hardline(
        &self,
        parts: &mut DocBuf,
        elem_end: u32,
        content_start: u32,
    ) {
        let d = self.d();
        if self.is_next_line_empty(elem_end, content_start) {
            parts.push(d.literalline());
        }
        parts.push(d.hardline());
    }

    /// [`Self::push_next_line_empty_hardline`] with the `content_start` derived here: the
    /// separator between two list items, asked once for every list that shares the
    /// element-comma contract (object literal, object pattern, import/export specifiers,
    /// enum members).
    ///
    /// `prev_end` is the previous item's trailing-run end, `item_start` this item's node
    /// start; the content between them is this item's leading comment run, so the scan
    /// stops at the first comment.
    ///
    /// ⚠️ **in source** — the first comment *physically* in the gap, not the first one
    /// this caller will emit. A blank-line scan reads bytes, so an OWNED comment (glued
    /// to the item's first token and printed from inside its doc) bounds it just the
    /// same: the author's blank sits *before* that comment, and a bound past it would put
    /// the blank outside the measured range. Deriving this per site is how the literal
    /// and the pattern came to ask it on two different axes — the one thing `printObject`
    /// printing both through one path says they must not do.
    ///
    /// ⚠️ The scan starts at the previous item's SHELL end ([`Self::element_shell_end`]),
    /// not at `prev_end` itself. This is the seam's **distance** half: `prev_end` is a
    /// CLAIM anchor and stops where the item's doc stops printing, which for an item whose
    /// span was extended over a stripped paren is *inside* the erased shell — and a
    /// distance measured from there reads the shell's own line breaks as an author blank
    /// (`{ k: (1⏎⏎), b }` grew one, in the literal and the pattern alike). The peel steps
    /// over `)` and whitespace only and **stops at the first comment**, so a blank the
    /// author wrote ahead of a comment in that shell is still measured — the two answers
    /// this function must give differ by exactly that, and one anchor gives both.
    pub(crate) fn push_item_blank_separator(
        &self,
        parts: &mut DocBuf,
        prev_end: u32,
        item_start: u32,
    ) {
        let (from, bound) = self.item_gap_blank_scan(prev_end, item_start);
        self.push_next_line_empty_hardline(parts, from, bound);
    }

    /// Whether the author left a blank line in an item gap — the predicate half of
    /// [`Self::push_item_blank_separator`], for a caller that must *decide* on the blank
    /// before it can pick a separator at all.
    ///
    /// A soft `line` cannot carry a blank, so a list whose separator is one has to know:
    /// the blank is authorship the list preserves, and preserving it means taking the
    /// hardline separator (which forces the break) rather than the `line`. Both halves read
    /// the same range, so the decision and the emission cannot disagree.
    pub(crate) fn item_gap_has_blank_line(&self, prev_end: u32, item_start: u32) -> bool {
        let (from, bound) = self.item_gap_blank_scan(prev_end, item_start);
        self.is_next_line_empty(from, bound)
    }

    /// The range an item gap's blank-line scan reads: from the previous item's SHELL end
    /// to the first comment **in source** (else the item itself). Both ends in one place
    /// because both halves above need both — deriving either separately is how a predicate
    /// and its emitter drift into disagreeing about the same gap.
    ///
    /// See [`Self::push_item_blank_separator`] for why the start is peeled and why the
    /// bound is physical.
    pub(in crate::printer) fn item_gap_blank_scan(
        &self,
        prev_end: u32,
        item_start: u32,
    ) -> (u32, u32) {
        let from = self.element_shell_end(prev_end, item_start);
        let bound = self
            .comments_in_source_between(from, item_start)
            .next()
            .map_or(item_start, |c| c.span.start);
        (from, bound)
    }

    /// Whether `[from, next)` holds a **truly blank line** — two newlines with nothing
    /// but horizontal whitespace between them.
    ///
    /// [`Self::has_blank_line_between`] reads the line-break table and counts newlines
    /// without looking at what sits between them, and either endpoint of this gap can be
    /// a node span that excludes a delimiter the printer still emits — a grouping paren
    /// the value's span starts inside (`const y =⏎// c⏎(⏎  a = b // c2⏎);`). The newline
    /// before that `(` and the one after it then read as an author blank line, one is
    /// emitted, and the next pass reads it back as real: a one-shot non-idempotency.
    ///
    /// The table lookup is kept as the fast reject — fewer than two newlines is
    /// conclusive — so only the rare positive pays the byte scan
    /// ([`printing::has_blank_line_between_strict`], the shared statement of the
    /// intervening-line rule), over a gap that is whitespace and at most a delimiter or
    /// two.
    pub(in crate::printer) fn has_blank_line_between_strict(&self, from: u32, next: u32) -> bool {
        self.has_blank_line_between(from, next)
            && printing::has_blank_line_between_strict(self.source, from, next)
    }

    /// Emit the separator after one comment in a leading run, toward the **physical**
    /// next comment rather than `emit_next` (the start of the next *emitted* comment,
    /// or the value/argument when this is the last). An owned comment — glued to the
    /// token after it, printed by that token's node — is skipped by every emit
    /// iterator yet still occupies the source gap, so both decisions here must anchor
    /// past it: [`blank_scan_end`](Self::blank_scan_end) finds the first physical
    /// comment in `(comment.end, emit_next)`, then a same-line block hugs it with a
    /// space ([`comment_hugs_next`](Self::comment_hugs_next)) and everything else takes
    /// the blank-preserving hardline. The single statement of that rule for the two
    /// hand-rolled leading-run emitters whose surrounding loop can't route through
    /// [`push_leading_comment_run`](Self::push_leading_comment_run)
    /// (`build_eq_comment_break_rhs`, `append_keyword_value_line_comments`) — so a run the
    /// author glued stays glued and a multiline owned comment's own newline is never read
    /// as an author blank line.
    ///
    /// ⚠️ **Two states, so it belongs only where the break is already FORCED.** Both
    /// callers sit past a `//` that has taken the line, inside an `indent_hardline` /
    /// forced continuation, where prettier's third separator — the soft `line` — would
    /// render as the hardline anyway. A site whose enclosing group can still be flat needs
    /// [`push_leading_comment_run`](Self::push_leading_comment_run) instead: reaching for
    /// this one there forces the group open around a run prettier keeps inline, which is
    /// exactly what the call family's leading emitter did before it was converged.
    /// The separator after a comment in a **single-slot gap** — a keyword/operator and
    /// the one token that follows it (`if /* c */ (`, `new /* c */ (`, a template
    /// interpolation's `${ /* c */ expr`, a type-literal member's leading run): a block
    /// glues with a space, a line comment takes a hardline because a `//` would
    /// otherwise swallow that token.
    ///
    /// ⚠️ **Keyed on the comment's KIND, not on the source** — that is the whole
    /// distinction from [`push_leading_run_separator`](Self::push_leading_run_separator),
    /// which asks whether the *author* glued the pair. Both are correct where they sit:
    /// a single-slot gap collapses a block comment to the inline form whenever it can do
    /// so losslessly, so the authored break carries no signal to preserve
    /// (`docs/conformance_prettier.md` §Comment Position Philosophy, the keyword→value
    /// rule), while a leading run inside a list keeps the lines the author gave it. Pick
    /// by which of those two a new site is, never by which is nearer.
    ///
    /// ⚠️ One caller deliberately does NOT route through here: the expression-statement
    /// leading gap additionally forces the hardline for an **honored directive**
    /// (`printer/expressions/mod.rs`), since a `prettier-ignore` must start its frozen
    /// node on a fresh line. Whether the other single-slot gaps owe that carve-out is
    /// open — none of them freezes a node, so no fixture reaches it today.
    pub(crate) fn push_comment_kind_separator(&self, parts: &mut DocBuf, comment: &Comment) {
        let d = self.d();
        if comment.is_block {
            parts.push(d.text(" "));
        } else {
            parts.push(d.hardline());
        }
    }

    pub(crate) fn push_leading_run_separator(
        &self,
        parts: &mut DocBuf,
        comment: &Comment,
        emit_next: u32,
    ) {
        let next = self.blank_scan_end(comment.span.end, emit_next);
        if self.comment_hugs_next(comment) {
            parts.push(self.d().text(" "));
        } else {
            self.push_blank_preserving_hardline(parts, comment.span.end, next);
        }
    }

    /// Whether the comment a TRAILING run is about to emit keeps the previous one's line,
    /// because the author GLUED the pair (`/* c1 */ /* c2 */`, `/* c */ // t`).
    ///
    /// The single statement of that question — the trailing-side counterpart of
    /// [`Self::comment_hugs_next`](Self::comment_hugs_next): every end-of-container run asks
    /// it, and a run that answered differently would be the drifted copy `docs/comments.md`
    /// §Trailing and dangling runs is about. `None` — the run's first comment — is never
    /// glued: it has the anchor behind it, not a comment.
    ///
    /// ⚠️ **Asked BETWEEN the two comments, never of the source right after the `*/`** —
    /// and the two spellings are not equivalent, which is the whole reason this is not
    /// `comment_hugs_next(prev)`. They part on exactly the byte these runs exist for: the
    /// list's own **comma**, deleted under `trailingComma: 'none'`. Asking what follows the
    /// `*/` stops at that comma, reports "no newline", and WELDS a comment the author gave
    /// its own line onto the previous one (`[A /* c1 */,⏎// c2]` → `A /* c1 */ // c2`, with
    /// the `//` then swallowing anything printed behind it). Asking whether the author put a
    /// newline *between* the two reads past the comma, because a deleted separator is not a
    /// line. The other trailing-run sites never saw the difference: everywhere else the
    /// comma coincides with a predecessor an earlier emitter already claimed, so `prev` is
    /// `None` there — only the one walk that covers a whole gap in a single pass
    /// ([`Self::build_trailing_gap_comments`]) reaches it.
    ///
    /// The `is_block` half is belt-and-braces rather than load-bearing: a `//` runs to end of
    /// line, so a following comment always has a newline between.
    ///
    /// The space it licenses is not the weld the separator-before rule exists to prevent
    /// ([`Self::build_trailing_body_comments_doc`]): both comments stay distinct, and a line
    /// comment never hugs, so nothing can land behind a `//`. A **dangling** run — the
    /// container's only content — does not ask it at all, because prettier splits a glued
    /// pair there ([`Self::push_dangling_comment_run`]).
    pub(crate) fn trailing_run_hugs_previous(
        &self,
        prev: Option<&Comment>,
        next_start: u32,
    ) -> bool {
        prev.is_some_and(|prev| {
            prev.is_block && !self.has_newline_between(prev.span.end, next_start)
        })
    }

    /// Emit the separator before one comment in a TRAILING run: a space when the author
    /// glued it to `prev` ([`Self::trailing_run_hugs_previous`]), otherwise the
    /// blank-preserving `hardline` that gives it its own line.
    ///
    /// The trailing-side counterpart of
    /// [`push_leading_run_separator`](Self::push_leading_run_separator), for the runs whose
    /// own-line arm is that unconditional break — the last-item→closer walk
    /// ([`Self::build_trailing_gap_comments`]), the array literal's end-of-array scan,
    /// and the call family's dangling emitter. A run whose non-glue separator is the
    /// caller's ([`Self::build_trailing_closer_comments_doc`], whose container may still
    /// collapse) asks the predicate directly and keeps its own arm.
    ///
    /// ⚠️ **The blank question is `isPreviousLineEmpty`, asked of the COMMENT** — the
    /// own-line arm is [`Self::push_adjacent_blank_hardline`], which carries that rule and
    /// its `blank_scan_start` floor. Both are the recurring bug here: a gap-wide reading
    /// preserves a blank prettier has no way to emit, and an unfloored scan reads a
    /// multi-line block's own newlines as one (`[1 /* x⏎⏎y */⏎// c]`). Either way the
    /// fabricated form is a fixed point both formatters then agree on — so F1, the ledger
    /// and the census are all blind to it, and only `fabrication:audit` on the pristine
    /// seed, or a prettier `compare`, shows it.
    pub(crate) fn push_trailing_run_separator(
        &self,
        parts: &mut DocBuf,
        prev: Option<&Comment>,
        scan_from: u32,
        next_start: u32,
    ) {
        if self.trailing_run_hugs_previous(prev, next_start) {
            parts.push(self.d().text(" "));
        } else {
            self.push_adjacent_blank_hardline(parts, scan_from, next_start);
        }
    }

    /// The own-line arm of a TRAILING run's separator: the comment's own `hardline`,
    /// preceded by a `literalline` where the author left a blank line **directly above
    /// it**.
    ///
    /// ⚠️ `isPreviousLineEmpty`, asked of the COMMENT ([`Self::previous_line_is_empty`])
    /// — not "does this gap hold a blank". Prettier's `printTrailingComment` emits its
    /// extra `hardline` from `isPreviousLineEmpty(locStart(comment))`, so the blank has to
    /// be adjacent to the comment; the gap-wide scan
    /// ([`Self::push_blank_preserving_hardline`], which is prettier's *leading* reading)
    /// additionally fires when re-emitted structure separates the two. Two places that
    /// bites: the specifier list's own comma (`import {a⏎⏎,⏎// c⏎b}` put the author's blank
    /// above the COMMA, which `printModuleSpecifiers` — a bare `join([",", line], …)` — has
    /// no way to emit), and a stripped grouping paren in a condition's trailing gap
    /// (`if (⏎(⏎x⏎⏎)⏎/* c */⏎)`, where the blank is the shell's and is erased with it).
    ///
    /// ⚠️ **The scan is still an IN-SOURCE question, and `blank_scan_start` is its FLOOR**
    /// rather than `scan_from` itself: a comment physically in the gap that this run did
    /// not emit — one an earlier emitter claimed, or an OWNED one printed from inside a
    /// node's doc — still occupies those bytes, and the backward walk must not read its
    /// newlines as the author's.
    ///
    /// The single statement of that arm, for the trailing-run separator above and for the
    /// control-flow gap builder's trailing run
    /// (`Printer::build_comments_between_parts`), whose glue policy differs per gap but
    /// whose blank question is this same one.
    pub(crate) fn push_adjacent_blank_hardline(
        &self,
        parts: &mut DocBuf,
        scan_from: u32,
        next_start: u32,
    ) {
        let from = self.blank_scan_start(scan_from, next_start);
        if self.previous_line_is_empty(from, next_start) {
            parts.push(self.d().literalline());
        }
        parts.push(self.d().hardline());
    }

    /// Emit the whole trailing comment run in `[anchor, end)` for a gap whose layout is
    /// **already vertical** — the run's first comment trails `anchor` on its line if the
    /// author put it there, every later one takes
    /// [`push_trailing_run_separator`](Self::push_trailing_run_separator). Returns the
    /// end of the last comment emitted (`anchor` when the gap holds none), which is the
    /// cursor a caller needs for whatever it emits next.
    ///
    /// The anchor question is asked **once**, of the first comment only: past it the
    /// thing behind a comment is another comment, so the separator's own glue test
    /// answers it. A loop that re-asks the anchor per comment reads the second half of a
    /// pair the author glued *across the first* and splits it — the bug this shape
    /// exists to prevent, which is why the four gaps that share it (a binary chain's
    /// operand→operator and operator→operand gaps, a unary operand's stripped-paren
    /// trailing gap, a retained non-null paren shell's operand→`)!` gap) call this
    /// rather than each carrying the loop.
    ///
    /// For a gap that may still collapse — where the run has to DEFER past a closer
    /// instead of taking real breaks — the emitter is
    /// [`push_trailing_comments_in_range`](Self::push_trailing_comments_in_range) or, at
    /// a stripped operand paren, `append_trailing_paren_comments`. See
    /// [docs/comments.md](../../../../../docs/comments.md) §Trailing and dangling runs.
    ///
    /// ⚠️ `leading_blank` is the one thing the three gaps do NOT share: whether an author
    /// blank ABOVE the run's first comment survives ([`RunLeadingBlank`]). Two of them are
    /// genuinely trailing runs and keep it; the operator→operand gap is a **leading** run
    /// of the right operand that borrows this emitter for its anchor rule, and prettier's
    /// `printLeadingComment` has no emitter for such a blank at all.
    ///
    /// ⚠️ **Caller invariant: an unconditional break must IMMEDIATELY follow the run.**
    /// The own-line arms emit each comment as REAL text, so a run-final `//` leaves its
    /// line open until the caller's next break — and a later gap's `line_suffix` flushing
    /// at that break lands welded onto it, invisible to the render-time run separator
    /// (`doc/arena_render_suffix.rs`), which compares suffix against suffix, never
    /// suffix against emitted text. Every current caller closes the line at once (a
    /// `hardline`, or a `group_break`-forced `softline`) with nothing between that could
    /// queue a suffix; a new call site that cannot guarantee that must defer the run
    /// through the suffix machinery instead (the clause-body statement tail does —
    /// `GapDeferral::LineSuffix` in `Printer::push_gap_comments`).
    pub(crate) fn push_anchored_trailing_run(
        &self,
        parts: &mut DocBuf,
        anchor: u32,
        end: u32,
        leading_blank: RunLeadingBlank,
    ) -> u32 {
        let mut pos = anchor;
        let mut prev_comment: Option<&Comment> = None;
        for comment in comments_to_emit_in_range(self.comments, anchor, end) {
            if prev_comment.is_none() && !self.comment_has_newline_between(pos, comment.span.start)
            {
                // On the anchor's own line (`x /* c */`, `a && // c`): it trails there —
                // a line comment via `line_suffix` (zero width, so a long trailing
                // comment never forces the preceding group to break, matching prettier's
                // `lineSuffix`), a block inline with its width counted.
                parts.push(self.build_trailing_comment_doc(comment));
            } else if prev_comment.is_none() && leading_blank == RunLeadingBlank::Drop {
                // The run's first comment in a gap that drops the blank above it: the
                // separator is the bare break, with no scan to make.
                parts.push(self.d().hardline());
                parts.push(self.build_comment_doc(comment));
            } else {
                // On its own line — unless the author GLUED it to the previous comment,
                // which keeps that line; otherwise an author blank above it is preserved.
                self.push_trailing_run_separator(parts, prev_comment, pos, comment.span.start);
                parts.push(self.build_comment_doc(comment));
            }
            pos = comment.span.end;
            prev_comment = Some(comment);
        }
        pos
    }

    /// Emit the whole gap between two comma-separated items when the gap contains a
    /// **line** comment (the forced-break case): the comma, the comments, and the
    /// break to the next item, leaving `parts` positioned to emit that item.
    ///
    /// The gap decomposes at the comma. Block comments before the first line comment
    /// trail the previous item inline (`= 0 /* c */`) and the comma is placed *before*
    /// the first line comment — a line comment runs to EOL, so a comma after it would
    /// be commented out. The first line comment then trails the comma iff it was
    /// authored on the comma's line (`comma_pos` → no intervening newline). Everything
    /// from there is the next item's **leading run**, emitted by the shared
    /// [`push_leading_comment_run`](Self::push_leading_comment_run) toward
    /// `next_start`, which also owns the final break: a block glued to the next item
    /// hugs it (`/* c */ b`), anything else drops to its own line. The break between
    /// the comma's line and the leading run is a bare `hardline` — prettier drops an
    /// author blank line there (it belongs to the item join, not to the run).
    ///
    /// `continuation` is emitted after each own-line break: the variable-declaration
    /// site passes `INDENT` text (its declarators aren't wrapped in `d.indent()`), the
    /// for-init and heritage sites pass an empty doc (their runs are). Shared by the
    /// variable-declarator, for-init, and heritage inter-item sites.
    ///
    /// Callers gate on the gap holding a line comment (`has_line_comments_between`) —
    /// a block-only gap has no forced break and belongs to their own path.
    pub(crate) fn push_inter_item_line_comment_gap(
        &self,
        parts: &mut DocBuf,
        prev_end: u32,
        comma_pos: u32,
        next_start: u32,
        continuation: DocId,
    ) {
        let d = self.d();
        let comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, prev_end, next_start).collect();
        // Everything before the first line comment trails the previous item, and the
        // comma is placed there rather than at its authored offset — a `//` runs to
        // EOL, so a comma after it would be commented out, and any block between the
        // two rides left with it (`a, /* c */ // x` → `a /* c */, // x`, matching
        // prettier). With no line comment (the callers' gate makes that unreachable)
        // this is 0 and the whole run simply leads the next item.
        let first_line_idx = comments.iter().position(|c| !c.is_block).unwrap_or(0);
        for comment in &comments[..first_line_idx] {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
        parts.push(d.text(","));
        // The first line comment trails the comma when authored on the comma's line
        // (no newline between); an own-line one starts the leading run below. The
        // `is_block` test keeps this honest without leaning on the callers' gate:
        // only a *line* comment can trail the comma, since a block there would be
        // the caller's block-only path.
        //
        // The comma was just emitted AHEAD of this comment rather than at its authored
        // offset, so a comment written before `comma_pos` still trails the printed comma
        // — the side `comment_on_comma_line` deliberately answers `true` for.
        let trails_comma = comments
            .get(first_line_idx)
            .is_some_and(|c| !c.is_block && self.comment_on_comma_line(comma_pos, c));
        let run_start = if trails_comma {
            parts.push(self.build_trailing_comment_doc(comments[first_line_idx]));
            first_line_idx + 1
        } else {
            first_line_idx
        };
        parts.push(d.hardline());
        parts.push(continuation);
        self.push_leading_comment_run(
            parts,
            comments[run_start..].iter().copied(),
            next_start,
            LeadingGlue::Adjacent,
            continuation,
        );
    }

    /// Whether a gap comment sits on the **comma's** line — the anchor every question about
    /// that gap reads, since the comma is re-emitted structure that the printer pulls back
    /// onto the previous item's line whatever the author did with it.
    ///
    /// Deliberately not the *item*'s line: an author who pushed the comma onto a line of
    /// its own (`a⏎, /* c */⏎ b`) wrote the comment against the comma, and an item-anchored
    /// reading calls it own-line and hands it to the next item's leading run — re-binding it
    /// to an item it was never written against.
    ///
    /// ⚠️ **A comment-classification read, so it takes the COMMENT line-break table**
    /// ([`Printer::comment_has_newline_between`], not the layout one). It decides whether a
    /// `//` is emitted as a trailing `line_suffix` or falls to the next item's leading run,
    /// and that role must stay real in the canonical reprint — where `layout_line_breaks`
    /// is empty and every comment in the gap would read as on the comma's line, re-binding
    /// an own-line comment the author wrote against the NEXT item onto the comma. Outside
    /// [`Printer::set_canonical`] the two tables are the same slice, so the normal path is
    /// byte-identical either way.
    ///
    /// ⚠️ **A comment BEFORE `comma_pos` reads as on the comma's line**, and that is the
    /// answer rather than a short-circuit to guard against: a caller in that position emits
    /// the comma *ahead* of the comment instead of at its authored offset, so the comment
    /// does trail the printed comma ([`Self::push_inter_item_line_comment_gap`] asks exactly
    /// that). A caller that instead needs the **side of the authored comma** must test
    /// `span.start >= comma_pos` itself — the parameter list does, at both ends of its
    /// partition, because an own-line comment before the comma there belongs to a different
    /// emitter entirely.
    pub(crate) fn comment_on_comma_line(&self, comma_pos: u32, comment: &Comment) -> bool {
        !self.comment_has_newline_between(comma_pos, comment.span.start)
    }

    /// Whether the gap opening at `gap_start` has already put a `//` on the output line by
    /// the time it reaches `pos` — the point at which a claim past the separator must stop.
    ///
    /// A `//` ends its output line whichever way the gap emitted it (an anchor-line one
    /// leaves as a `line_suffix`, an own-line one as a deferred hardline run), so a second
    /// comment claimed onto that line welds onto the first — the second delimiter becoming
    /// text, the comment ceasing to exist and the code behind it swallowed — and an inline
    /// block claimed after one comes out REORDERED ahead of it. Everything past that point
    /// falls to the next item's leading run, on its own line, which is where prettier puts
    /// it too.
    ///
    /// ⚠️ **The question is the comment's KIND, never its anchor line.** Asking it with an
    /// anchor reading misses an own-line `//` (`a /* x⏎y */ // c1⏎, // c2⏎ b`, where the
    /// block's `*/` pushes `// c1` off the anchor's line) and welds the pair.
    ///
    /// One spelling for both emitters that claim past a comma —
    /// [`Printer::emit_multiline_comma_with_comments`] and
    /// [`Printer::param_trailing_line_comment`] — since a drift between them is a weld at
    /// whichever one drifted, and the weld is lossless-looking to every structural guard.
    pub(crate) fn gap_emitted_line_comment_before(&self, gap_start: u32, pos: u32) -> bool {
        comments_to_emit_in_range(self.comments, gap_start, pos).any(|c| !c.is_block)
    }

    /// A block comment after the comma that sits on the comma's own line (no
    /// newline between the comma and the comment) while a newline separates it
    /// from the next item — a **stranded** comment. It trails the comma,
    /// preserving the author's placement, rather than dropping to its own line;
    /// prettier relocates it *before* the comma. Mirrors the call-argument
    /// stranded rule (`calls/arg_comments.rs`). See conformance_prettier_ts_comments.md
    /// §Comment relocation. (A block that instead *hugs* the next item — no
    /// newline before it — leads that item and matches prettier, so it is not
    /// stranded.)
    pub(crate) fn is_stranded_after_comma_block(
        &self,
        comment: &Comment,
        comma_pos: u32,
        next_start: u32,
    ) -> bool {
        comment.is_block
            && self.comment_on_comma_line(comma_pos, comment)
            && !self.is_same_line(comment.span.end, next_start)
    }

    /// Emit the **before-comma** block comments in `[start, comma_pos)` trailing
    /// the preceding item (` /* c */`), preserving the author's side of the comma.
    /// The caller pushes the comma after this. Shared by the variable-declarator,
    /// for-init, and heritage inter-item sites; the after-comma counterparts are
    /// [`Self::push_stranded_after_comma_blocks`] (stranded, trails the comma) and
    /// the site's leading run (a block hugging the next item leads it).
    pub(crate) fn push_before_comma_blocks(&self, parts: &mut DocBuf, start: u32, comma_pos: u32) {
        let d = self.d();
        for comment in comments_to_emit_in_range(self.comments, start, comma_pos) {
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
        }
    }

    /// Emit every comment in `[start, end)` trailing what precedes it
    /// ([`Self::build_trailing_comment_doc`] — a block inline, a line comment via
    /// `line_suffix` so it flushes at end of the rendered line rather than swallowing
    /// the following token). For a gap whose comments all keep their position and need
    /// no per-comment routing; a caller that must split the run by position (around a
    /// separator, by own-line-ness) iterates itself.
    ///
    /// Returns whether the run held a **line** comment — i.e. whether the caller must
    /// break. That is the same walk, so asking it here spares the caller a second
    /// binary search over the range it just scanned, and keeps the break question
    /// keyed on what was actually EMITTED (an owned comment is skipped by both).
    /// Callers whose following token can't be swallowed ignore it.
    ///
    /// ⚠️ **The separator goes BEFORE each comment, and it is asked of the SOURCE.**
    /// A comment the author left on its own line takes
    /// [`Self::build_trailing_comment_doc_own_line`] instead, so a run of them stays a
    /// run. Emitting them back to back with nothing between welds the run into a single
    /// comment — the second `//` becomes text of the first (`// c1 // c2`), so the
    /// second comment stops existing — and a block following a line comment additionally
    /// **reorders** ahead of it, the inline form jumping the deferred one. Both are
    /// stable, lossless-looking outputs that reparse, which is why the ledger, F1, the
    /// round-trip and the fuzzer are all blind to them; only the census and a prettier
    /// `compare` see it. This is the trailing-gap face of the rule in
    /// [`Self::build_trailing_body_comments_doc`] / [`Self::push_dangling_comment_run`]
    /// — see [docs/comments.md](../../../../../docs/comments.md) §Trailing and dangling
    /// runs.
    ///
    /// Asking the source ("did the author give this comment its own line?") rather than
    /// the previous comment's kind is what keeps the two glued shapes intact: a run the
    /// author wrote on one line (`/* c1 */ /* c2 */`) stays on one line, because a block
    /// comment can share it. The kind test is the trap that formulation avoids.
    pub(crate) fn push_trailing_comments_in_range(
        &self,
        parts: &mut DocBuf,
        start: u32,
        end: u32,
    ) -> bool {
        let mut has_line_comment = false;
        // Cursor over what physically precedes each comment — an **in-source** question
        // (docs/comments.md §the three axes), so it advances over every comment emitted
        // here, not just the ones that broke.
        let mut prev_end = start;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            // The *comment* line-break table, never the layout one: this decides whether
            // a `//` is followed by a break, so it must stay real under the canonical
            // reprint, where an erased read would re-weld the run.
            let own_line = self.comment_has_newline_between(prev_end, comment.span.start);
            let is_line = !comment.is_block;
            // A **line** comment is deferred by construction (it runs to EOL). A **block**
            // one is deferred only to stay BEHIND a line comment already in this run:
            // deferring is what carries a comment out past the construct's closer, so a
            // block that could simply sit inline must, or it leaves the parens it was
            // written inside. Once the run is deferred, though, an inline block would
            // render *before* the deferred text and the pair would come out reordered.
            let deferred = is_line || has_line_comment;
            parts.push(if deferred && own_line {
                self.build_trailing_comment_doc_own_line(comment)
            } else {
                self.build_trailing_comment_doc(comment)
            });
            has_line_comment |= is_line;
            prev_end = comment.span.end;
        }
        has_line_comment
    }

    /// Split an OPENING DELIMITER's leading run at the author's GLUE: a **line** comment
    /// written on the delimiter's own line is returned separately, for the caller to emit
    /// right after it (`( // c`), and the rest of the run resumes below, inside the
    /// construct. Returns `(glued, resume)`, where `resume` is where the remaining run
    /// starts — `start` unchanged when nothing was glued.
    ///
    /// This is the opening-delimiter rule, in one place: what the author put on the
    /// `(`/`[`/`{`/`<` line stays there, and what they put on its own line keeps its own
    /// line, so both authorings are fixed points
    /// (`Printer::push_leading_comment_run`'s glue test, asked here of the delimiter
    /// rather than of the item). Only a `//` can claim the line and only one can — it runs
    /// to end of line — and only a comment the author actually glued; a **block** is
    /// untouched, since it already renders inline inside the construct.
    ///
    /// The retained type paren shells were the first callers and a **statement header's**
    /// `(` the last — `if` / `while` / do-while / `switch` / `catch` and all three `for`
    /// spellings, which followed prettier here long after every other delimiter in the
    /// printer had stopped. Prettier is no coherent oracle for the position: it un-glues at
    /// `if`-like heads and the C-style `for`, **glues** at for-in / for-of (emitting the
    /// rest of the head unindented at column zero), and relocates the comment out of the
    /// parens entirely at do-while. See
    /// [conformance_prettier_ts_comments.md §Comment relocation](../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
    ///
    /// The leading `" "` is the PRINTER's and rides in the returned doc, never the
    /// author's — `fn(// c` and `fn(   // c` both normalize to one space
    /// ([`comments.md`](../../../../docs/comments.md) §The delimiter-line question). A
    /// caller therefore emits its bare delimiter text and concatenates this; writing
    /// `"( "` itself is the rule spelled a second time, which is how the two shells that
    /// used to own a copy of this function came to disagree with it.
    ///
    /// The author blank BELOW the pulled comment rides with it, for every caller — see
    /// [`Self::push_delimiter_glued_blank`], which this pushes and the list / call families
    /// push for their own pull.
    pub(crate) fn split_open_delimiter_glued_run(
        &self,
        start: u32,
        end: u32,
    ) -> (Option<DocId>, u32) {
        let d = self.d();
        let Some(comment) = comments_to_emit_in_range(self.comments, start, end).next() else {
            return (None, start);
        };
        // ⚠️ The COMMENT line-break table, never the layout one: this is a comment-adjacency
        // classification, so it must stay real under `Printer::set_canonical` — read against
        // the emptied layout table it calls every own-line comment glued and relocates it
        // onto the delimiter's line in the canonical reprint.
        if comment.is_block || self.comment_has_newline_between(start, comment.span.start) {
            return (None, start);
        }
        let mut parts = DocBuf::new();
        parts.push(d.text(" "));
        parts.push(self.build_comment_doc(comment));
        // The run emitter below never sees this blank, because the comment left its range.
        self.push_delimiter_glued_blank(&mut parts, comment.span.end, end);
        (Some(d.concat(&parts)), comment.span.end)
    }

    /// The author blank BELOW the comment(s) a pull moved onto an opening delimiter's line
    /// — one `hardline` on top of the caller's own break, which is the blank itself.
    ///
    /// **One rule for the whole family**, and the second half of the opening-delimiter rule
    /// ([`comments.md`](../../../../docs/comments.md) §The delimiter-line question): a blank
    /// between a comment and what follows it is authorship, and the pull moves the comment's
    /// LINE, not its membership. Every leading-run emitter in the printer already keeps this
    /// blank ([`Self::push_leading_comment_run`],
    /// [`Self::push_paren_shell_leading_run`]) and prettier keeps it at every construct —
    /// call, `new`, `[`, `{`, `if`, `return (`, `yield` — from its own un-glued placement.
    ///
    /// ⚠️ It used to be a two-valued axis (`DelimiterGluedBlank`, now gone), the list and call
    /// families dropping the blank on the reading that a blank directly under a container's
    /// opening line is that container's LEADING-GAP blank. Two things sank it. The null
    /// control it rested on (`fn(⏎⏎a)`, no comment) measures a different gap — delimiter→
    /// content, which both formatters clear here too — while the blank in question is
    /// comment→content, which tsv itself keeps from the un-glued authoring of the same
    /// comment. And the premise was true only *after* tsv's own pull put the comment on that
    /// line, so the blank's fate turned on whether the author had glued the comment to the
    /// delimiter: `fn(⏎// c⏎⏎a)` kept it and `fn( // c⏎⏎a)` did not, one authored blank with
    /// two fates.
    ///
    /// A blank between the delimiter and the FIRST comment is not this question and stays
    /// erased: it sits against the delimiter, where tsv and prettier drop it at every
    /// bracket alike.
    ///
    /// `pulled_end` is the end of the last comment the pull emitted; `gap_end` the first
    /// item's start. The scan stops at the next comment, so a blank further down the run
    /// belongs to that comment's own leading rule rather than to the pull.
    pub(crate) fn push_delimiter_glued_blank(
        &self,
        parts: &mut DocBuf,
        pulled_end: u32,
        gap_end: u32,
    ) {
        let next_start = comments_to_emit_in_range(self.comments, pulled_end, gap_end)
            .next()
            .map_or(gap_end, |c| c.span.start);
        // ⚠️ The scan measures a DISTANCE, so it must not span a delimiter glyph the
        // author put on a line of its own. At most sites the delimiter precedes the
        // comment (`fn( // c`), but the unary comment-holder's gap opens at the OPERATOR
        // (`!! // c⏎(⏎x`), so the `(`'s own line break lands inside the range and the two
        // newlines read as an author blank — a fabricated blank line, which is its own
        // fixed point and reparses, so only a prettier `compare` or the fabrication audit
        // over real code finds it (`docs/comments.md` hazard 5). Ending the scan at the
        // glyph also gives the right answer for a blank BELOW it: that one sits against
        // the delimiter, where every bracket in both formatters drops it.
        // `blank_scan_end` first, because the scan must not span comment BYTES either —
        // an owned comment in this gap is skipped by the emit axis but still occupies its
        // interior newlines (hazard 5's other half). It also makes the byte scan below
        // comment-free by construction.
        let scan_end = self.blank_scan_end(pulled_end, next_start);
        let scan_end = self.first_open_delimiter_in(pulled_end, scan_end);
        if self.has_blank_line_between(pulled_end, scan_end) {
            parts.push(self.d().hardline());
        }
    }

    /// The first `(` / `[` / `{` physically in `[start, end)`, or `end` — the bound a
    /// blank-line scan across a delimiter-glued gap takes
    /// ([`Self::push_delimiter_glued_blank`]).
    ///
    /// A raw byte scan is sound because the caller hands a comment-free range
    /// ([`Self::blank_scan_end`]), so no `(` inside a comment's text can be reached.
    fn first_open_delimiter_in(&self, start: u32, end: u32) -> u32 {
        self.source.as_bytes()[start as usize..end as usize]
            .iter()
            .position(|b| matches!(b, b'(' | b'[' | b'{'))
            .map_or(end, |i| start + i as u32)
    }

    /// [`Self::split_open_delimiter_glued_run`] for a shell whose `(` is **located** by a
    /// scan rather than known from the AST — the restricted-production hang
    /// ([`Printer::build_restricted_production_paren_doc`]) and the ASI-gap operand shell
    /// ([`Self::build_asi_operand_shell_doc`]). A gap with no `(` in it has no delimiter
    /// line for a comment to claim, so the whole gap stays the leading run's.
    ///
    /// The guard is all this adds: the rule itself is the one function above, so the two
    /// shells cannot answer differently about the same authoring — and no longer differ
    /// from every other delimiter either, which they did on all three of the space, the
    /// blank and the line-break table.
    ///
    /// ⚠️ `gap_start` is the gap's own start, NOT `open_paren + 1`, and it is both the scan
    /// start and the glue anchor. The two coincide, but for different reasons per caller and
    /// neither is a free choice:
    /// - Scanning from `gap_start` is required. The restricted-production gap opens at the
    ///   KEYWORD, so a comment before the `(` (`return /* c */ (x)`) is in it and has no
    ///   other emitter; starting the scan at the `(` would drop it, and gluing past it would
    ///   reorder it.
    /// - Measuring from `gap_start` is safe. It differs from `open_paren + 1` only if a
    ///   newline sits between the two, and all three restricted productions forbid one
    ///   (`return` / `throw` / `yield` + a line terminator is ASI, so the argument is gone);
    ///   the ASI shell's `gap_start` IS `open + 1`. A multi-line *block* in that region
    ///   cannot reach the measurement either — it is the gap's first comment, and a block
    ///   declines the glue outright.
    pub(crate) fn split_located_paren_glued_run(
        &self,
        gap_start: u32,
        open_paren: Option<u32>,
        value_start: u32,
    ) -> (Option<DocId>, u32) {
        if open_paren.is_none() {
            return (None, gap_start);
        }
        self.split_open_delimiter_glued_run(gap_start, value_start)
    }

    /// Emit a retained-paren SHELL's leading run — the comments in `[start, end)`
    /// between the shell's `(` and the type it wraps — each followed by its own
    /// separator: a space for a block (`(/* c */ a | b)`), a `hardline` for a line
    /// comment, which must end its line or it would swallow the type after it.
    /// Returns whether a line comment was emitted, i.e. whether the shell must break.
    ///
    /// `line_comments` names who owns them ([`ShellLeadingRun`]):
    /// [`ShellLeadingRun::Upstream`] where an emitter above already placed them (the
    /// later-member paren-union arms of `build_union_type_doc_with_line_comments`), so
    /// emitting here would double-print them; the block arm always emits.
    ///
    /// ⚠️ Still not [`Self::push_leading_comment_run`], the canonical leading emitter —
    /// a shell's separator is the delimiter's question, not the item's — but no longer
    /// keyed on the comment's own KIND alone, which was the whole of the difference. Both
    /// of that emitter's what-follows rules now hold here too: the **glue** test (asked
    /// one step earlier, by [`Self::split_open_delimiter_glued_run`], since a glued
    /// comment belongs to the `(` line the caller owns) and **blank-line preservation**
    /// below. The shells were the last leading-run sites deciding a separator without
    /// asking what follows, and each half was a real difference from every other opening
    /// delimiter: `fn(`, `[`, `{` and `<` keep both the glue and the blank, and prettier
    /// keeps the blank at all of them too.
    pub(crate) fn push_paren_shell_leading_run(
        &self,
        parts: &mut DocBuf,
        start: u32,
        end: u32,
        line_comments: ShellLeadingRun,
    ) -> bool {
        let d = self.d();
        let mut has_line_comment = false;
        let run: CommentVec<'_> = comments_to_emit_in_range(self.comments, start, end).collect();
        for (i, comment) in run.iter().enumerate() {
            if comment.is_block {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            } else if line_comments == ShellLeadingRun::Here {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.hardline());
                has_line_comment = true;
                // An author blank between this comment and what follows it — the next
                // comment, or the type the shell wraps — is authorship, not shell
                // structure, and every other opening delimiter keeps it (`fn(`, `{`, `[`,
                // where prettier keeps it too). Dropping it here made the shells the one
                // place tsv erased a blank line both its own siblings and prettier
                // preserve. A blank between the `(` and the FIRST comment is a different
                // question and stays erased: it sits against the delimiter, where every
                // formatter — tsv and prettier alike, at every bracket — drops it.
                let next_start = run.get(i + 1).map_or(end, |c| c.span.start);
                if self.has_blank_line_between(comment.span.end, next_start) {
                    parts.push(d.hardline());
                }
            }
        }
        has_line_comment
    }

    /// Emit the comment run in `[anchor, end)` PRESERVING each comment's own-line-ness —
    /// the emission a routed format-ignore directive needs, since the placement that
    /// earned the freeze is exactly what an ordinary hang emitter would relocate.
    /// Returns `(trailing, own_line)`: a comment still on `anchor`'s line trails it, and
    /// every own-line comment keeps its own line, each preceded by a `hardline`. Both
    /// sinks are the caller's to place — the conditional's `?` arm trails the extends
    /// line (a separate buffer) while its `:` arm and the function type's pre-arrow gap
    /// trail into the run they are already building.
    ///
    /// A gap's same-line comments are exactly the prefix of its run (position and line
    /// number are both monotonic), so splitting the run across two sinks preserves
    /// source order at every call site.
    ///
    /// Deliberately NOT [`Self::push_leading_comment_run`]: that seam measures each
    /// separator against the run's terminal, but what follows this run in the OUTPUT is
    /// an operator (`?` / `:` / `=>`), not the node the comments lead — so its glue test
    /// would hug a block comment whose `*/` merely shares a line with that node's start,
    /// pulling a directive off the own line that earned the freeze. The same-line test
    /// here is a ROUTING question (trail the previous line vs. sit above the operator's
    /// line), not a separator question.
    pub(crate) fn build_own_line_preserving_run(&self, anchor: u32, end: u32) -> (DocId, DocBuf) {
        let d = self.d();
        let mut trailing_parts = DocBuf::new();
        let mut own_line_parts = DocBuf::new();
        for comment in comments_to_emit_in_range(self.comments, anchor, end) {
            if self.is_same_line(anchor, comment.span.start) {
                trailing_parts.push(self.build_trailing_comment_doc(comment));
            } else {
                own_line_parts.push(d.hardline());
                own_line_parts.push(self.build_comment_doc(comment));
            }
        }
        (d.concat(&trailing_parts), own_line_parts)
    }

    /// Emit the **stranded** after-comma block comments in `[comma_pos, next_start)`
    /// trailing the comma (` /* c */`), preserving the author's placement. The
    /// caller pushes the comma before this and handles the remaining (non-stranded)
    /// after-comma comments as leading comments on the next item. Shared by the
    /// variable-declarator, for-init, and heritage inter-item sites; see
    /// [`Self::is_stranded_after_comma_block`].
    ///
    /// Returns `Some(end)` — where the next item's **leading** run resumes, past the
    /// stranded prefix — or `None` when nothing was stranded, leaving the caller's own
    /// anchor standing. The stranded set is a contiguous prefix of the gap (each member
    /// sits on the comma's line, and the first that doesn't ends it), so the two halves
    /// partition it: a caller that emits the run and then resumes before it DOUBLE-PRINTS
    /// the stranded blocks. Returning the anchor is what keeps the two from being written
    /// apart — the same coupling [`Self::push_item_trailing_run`] states at the other end
    /// of the gap. A caller with no leading emitter of its own ignores it.
    ///
    /// ⚠️ **`None` is not `comma_pos`.** The gap opens at the previous item's claimed
    /// trailing run, which ends BEFORE the comma, so a caller handed `comma_pos` on the
    /// empty case resumes past `[run_end, comma)` — and a comment the author put on its own
    /// line there (`x: T⏎/* c */, y: U`) is claimed by neither half and DROPPED. Measured,
    /// by `gaps:audit`, on the change that introduced it.
    pub(crate) fn push_stranded_after_comma_blocks(
        &self,
        parts: &mut DocBuf,
        comma_pos: u32,
        next_start: u32,
    ) -> Option<u32> {
        let d = self.d();
        let mut resume = None;
        for comment in comments_to_emit_in_range(self.comments, comma_pos, next_start) {
            if !self.is_stranded_after_comma_block(comment, comma_pos, next_start) {
                break;
            }
            parts.push(d.text(" "));
            parts.push(self.build_comment_doc(comment));
            resume = Some(comment.span.end);
        }
        resume
    }

    /// Build a Doc for inline comments between two positions with specified spacing and filter
    ///
    /// Returns a Doc containing all comments in the range with the specified spacing.
    /// Returns empty concat if no comments found.
    ///
    /// Uses binary search to find starting point: O(log n + k)
    pub(crate) fn build_comments_between(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
    ) -> DocId {
        self.build_comments_between_filtered(start, end, spacing, CommentFilter::All)
    }

    /// Build a Doc for inline comments with filtering
    // `track_caller` (ledger builds only): the `BlockOnly` skip annotation below records
    // the caller that held the filter licence, and this wrapper must be transparent to it.
    #[cfg_attr(feature = "comment_check", track_caller)]
    pub(crate) fn build_comments_between_filtered(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) -> DocId {
        self.build_comments_between_filtered_opt(start, end, spacing, filter)
            .unwrap_or_else(|| self.d().empty())
    }

    /// Build a Doc for inline comments with filtering, returning None if no comments.
    ///
    /// This is more efficient than `has_comments_to_emit_between` + `build_comments_between`
    /// because it uses a single binary search instead of two.
    ///
    /// ⚠️ **`Leading` emits no separator AFTER the run — the caller owns what follows.**
    /// The separators here sit *between* comments, so a run whose last comment is a `//`
    /// ends mid-line and the caller's next token is **swallowed** by it. `Trailing` has no
    /// such hole (its separator is emitted after each comment, `hardline` for a line one),
    /// which is why an identical gap can be correct under one spacing and lossy under the
    /// other — `call // c⏎<T>()` versus the callee→`(` gap that produced `call // c();`,
    /// eating the parens and the `;` (fixture
    /// `calls/callee_line_comment_empty_args_prettier_divergence`).
    ///
    /// A `Leading` caller must therefore answer "what follows this run?" itself: gate on
    /// [`Self::has_line_comments_between`] and route to
    /// [`Self::build_continuation_indent`] (or push its own break) before falling through
    /// to this builder. `push_empty_args` and `build_dot_gap_doc` are the worked examples.
    /// The swallowed form parses and is a fixed point, so idempotency, round-trip, and the
    /// print-once ledger are all blind to it; `swallow_audit` sees only shapes some fixture
    /// already carries.
    #[cfg_attr(feature = "comment_check", track_caller)]
    pub(crate) fn build_comments_between_filtered_opt(
        &self,
        start: u32,
        end: u32,
        spacing: CommentSpacing,
        filter: CommentFilter,
    ) -> Option<DocId> {
        // The `BlockOnly` licence, checked (ledger builds only): record every line comment
        // this filter passes over, with the caller that held the licence (`track_caller`).
        // The licence is a promise that a gate routed line comments to the expansion
        // builder first; nothing else enforces that the gate is the exact complement of
        // what this filtered builder can express. Annotation only — the ledger's drain
        // surfaces the site solely on a comment that ends the format DROPPED; a skip whose
        // comment another emitter prints (the routed expansion path, a winning
        // `conditional_group` sibling) stays invisible. See
        // `comment_ledger::record_filtered_skip`.
        #[cfg(feature = "comment_check")]
        if matches!(filter, CommentFilter::BlockOnly)
            && tsv_lang::comment_ledger::comment_check_enabled()
        {
            let site = std::panic::Location::caller();
            for c in comments_to_emit_in_range(self.comments, start, end) {
                if !c.is_block {
                    tsv_lang::comment_ledger::record_filtered_skip(self.source, c.span, site);
                }
            }
        }

        let d = self.d();

        // Check if any comments exist in range (considering filter)
        let has_comments = comments_to_emit_in_range(self.comments, start, end)
            .any(|c| !matches!(filter, CommentFilter::BlockOnly) || c.is_block);

        if !has_comments {
            return None;
        }

        // Build docs for matching comments.
        //
        // A line comment ends its line, so the next comment in the run must start a new
        // one — else two line comments merge onto one (`// c1 // c2` reparses as a single
        // comment: boundary loss). So a `hardline`, not the spacing separator, sits across
        // any line-comment boundary *within* the run. The run's far edge is the caller's
        // (see the ⚠️ on this function): `Leading` adds nothing after the last comment,
        // `Trailing` hardlines. A block
        // comment keeps the inline spacing.
        let mut parts = DocBuf::new();
        let mut prev_was_line = false;
        let mut prev_end: Option<u32> = None;
        let mut first = true;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            // Apply filter
            if matches!(filter, CommentFilter::BlockOnly) && !comment.is_block {
                continue;
            }

            // An authored blank line between two comments that each occupy their own
            // line separates two distinct remarks, exactly as a blank between two
            // statements does, so it survives (`conformance_prettier_ts_comments.md` §"No
            // blank above a body block's `{`"). Only meaningful where the separator is a `hardline`:
            // an inline run has no lines to separate.
            let blank_before = prev_was_line
                && prev_end
                    .is_some_and(|p| self.has_blank_line_between_strict(p, comment.span.start));

            match spacing {
                CommentSpacing::Leading => {
                    // Separator before this comment: the surrounding-indent `hardline`
                    // after a line comment (no leading space — it starts the line),
                    // else the inline leading space.
                    if !first && prev_was_line {
                        if blank_before {
                            parts.push(d.literalline());
                        }
                        parts.push(d.hardline());
                    } else {
                        parts.push(d.text(" "));
                    }
                    parts.push(self.build_comment_doc(comment));
                }
                CommentSpacing::Trailing => {
                    parts.push(self.build_comment_doc(comment));
                    self.push_comment_kind_separator(&mut parts, comment);
                }
                CommentSpacing::None => {
                    if !first {
                        if prev_was_line {
                            if blank_before {
                                parts.push(d.literalline());
                            }
                            parts.push(d.hardline());
                        } else {
                            // A block comment doesn't end its line, so the next comment
                            // still needs an explicit separator — without one the run
                            // fuses into `/* a *//* b */`. `None` suppresses the
                            // *leading* space before the run, not the separators inside
                            // it.
                            parts.push(d.text(" "));
                        }
                    }
                    parts.push(self.build_comment_doc(comment));
                }
            }
            prev_was_line = !comment.is_block;
            prev_end = Some(comment.span.end);
            first = false;
        }
        Some(d.concat(&parts))
    }

    /// Build a Doc for inline comments between two positions (leading space)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc(&self, start: u32, end: u32) -> DocId {
        self.build_comments_between(start, end, CommentSpacing::Leading)
    }

    /// Build a Doc for trailing comments where a comment that forces the following
    /// content onto its own line gets a hardline after it, and every other comment
    /// collapses inline (a space, as `build_comments_between(_, _, Trailing)` does).
    ///
    /// The separator is [`Printer::comment_hangs_next`] — the same per-comment
    /// rule as the gate that selects this builder
    /// ([`Printer::comments_force_own_line_between`]), so a gate and its emitter can't
    /// answer differently. Two shapes hang: a **line** comment (a `//` would swallow
    /// the following content) and an **own-line multiline** block (inlining it would
    /// reflow the author's break). A single-line block in any position, and a glued
    /// multiline block, collapse.
    ///
    /// Use across a gap whose following token must not be swallowed or reflowed — the
    /// type-construct delimiter/keyword gaps (`=> // leading\nT`, `: // leading\nT`,
    /// an indexed access's `[`→index, a template-literal type's `${`→type).
    pub(crate) fn build_trailing_comments_hang_next(&self, start: u32, end: u32) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut comments = comments_to_emit_in_range(self.comments, start, end).peekable();
        while let Some(comment) = comments.next() {
            parts.push(self.build_comment_doc(comment));
            let emit_next = comments.peek().map_or(end, |n| n.span.start);
            // **in source**: both the hang question and the blank measure anchor on the
            // physically next comment, not the next one *this* builder emits, so an
            // owned comment sitting between two emitted ones can't desync them — and it
            // is the same anchor the selecting gate uses
            // (`comments_force_own_line_between` walks `comments_in_source_range`), so
            // gate and emitter answer the one question identically.
            let next = self.blank_scan_end(comment.span.end, emit_next);
            // An honored directive owns its line on BOTH sides. The after-side is
            // [`Self::comment_hangs_next`], which this asks of every comment; the
            // before-side is this separator read one comment earlier — a single-line block
            // ahead of a directive (`await /* c1 */⏎// prettier-ignore⏎v`) hangs nothing on
            // its own, so without this the run renders `/* c1 */ // prettier-ignore` and the
            // directive lands in the placement the floor calls INERT.
            let next_is_directive = comments
                .peek()
                .is_some_and(|n| self.is_honored_directive(n));
            if next_is_directive || self.comment_hangs_next(comment, next) {
                // An authored blank line survives wherever the break it separates
                // survives. Here the break is FORCED — a `//` runs to end-of-line, an
                // own-line multiline block would reflow — so the blank is authoring
                // intent, not layout this continuation gets to collapse. Same answer as
                // the value side of these very constructs
                // (`build_comments_between_filtered_opt`'s blank arm); the two emitters
                // used to answer it two ways. See conformance_prettier.md §Authored
                // breaks in value position. Only meaningful across a `hardline`: an
                // inline run has no lines to separate.
                self.push_blank_preserving_hardline(&mut parts, comment.span.end, next);
            } else {
                parts.push(d.text(" "));
            }
        }
        // `concat` short-circuits the no-comments-in-range case to `empty()`.
        d.concat(&parts)
    }

    /// Leading comment run for a conditional branch arm (`?`/`:` → branch value):
    /// each comment takes a space when the next content shares its closing line
    /// (`? /* c */ v` stays glued), else `soft_sep` — the caller's collapsible
    /// line, so an authored break after the comment holds when the conditional is
    /// broken and yields when it is flat. This is prettier's `printLeadingComment`
    /// separator, except its own-line `hardline` case is deliberately not
    /// mirrored: tsv re-glues an own-line comment to the operator, so a hardline
    /// keyed on the authored newline *before* the comment would collapse on the
    /// second pass (prettier itself is non-idempotent there), and the
    /// §Authored-breaks-in-value-position rule collapses the fitting form anyway.
    /// Separator anchors ride the physical next comment
    /// ([`Self::blank_scan_end`]) so an owned comment glued to the value can't
    /// desync them.
    ///
    /// Line comments never reach this run — both conditional printers route them
    /// to their breaking layouts — and a line comment's collapsible separator
    /// would swallow the branch, so that routing is load-bearing.
    ///
    /// Returns `None` when the gap has no comments to emit.
    pub(crate) fn build_branch_comment_run(
        &self,
        start: u32,
        end: u32,
        soft_sep: DocId,
    ) -> Option<DocId> {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut comments = comments_to_emit_in_range(self.comments, start, end).peekable();
        while let Some(comment) = comments.next() {
            debug_assert!(
                comment.is_block,
                "line comments belong to the breaking layout"
            );
            parts.push(self.build_comment_doc(comment));
            let emit_next = comments.peek().map_or(end, |n| n.span.start);
            let next = self.blank_scan_end(comment.span.end, emit_next);
            if self.comment_hugs_next(comment) {
                parts.push(d.text(" "));
            } else {
                // An author blank after the comment is itself a break trigger
                // (prettier breaks the conditional on it too), so the break is
                // forced and the blank survives — the conditional-branch
                // carve-out in conformance_prettier.md §Authored breaks in
                // value position. The expression printer routes blank gaps to
                // its breaking layout before building a run
                // (`comment_followed_by_blank`), so that arm serves the
                // conditional-type branches. Without a blank the caller's
                // collapsible `soft_sep` stands — the one thing that makes this
                // run differ from `push_leading_run_separator`.
                self.push_blank_preserving_separator(&mut parts, comment.span.end, next, soft_sep);
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(d.concat(&parts))
        }
    }

    /// Prepend an optional leading doc (a comment run) to `doc`; `None` passes
    /// `doc` through untouched, keeping the comment-free path allocation-free.
    pub(crate) fn prepend_opt(&self, lead: Option<DocId>, doc: DocId) -> DocId {
        match lead {
            Some(lead) => self.d().concat(&[lead, doc]),
            None => doc,
        }
    }

    /// Build a Doc for inline comments, returning None if no comments.
    ///
    /// Use this instead of `has_comments_to_emit_between` + `build_inline_comments_between_doc`
    /// to avoid redundant binary searches.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_opt(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        self.build_comments_between_filtered_opt(
            start,
            end,
            CommentSpacing::Leading,
            CommentFilter::All,
        )
    }

    /// Build a Doc for inline comments between two positions (no spaces)
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_no_leading_space(
        &self,
        start: u32,
        end: u32,
    ) -> DocId {
        self.build_comments_between(start, end, CommentSpacing::None)
    }

    /// Build a Doc for inline comments between two positions (trailing space)
    ///
    /// Used when comments appear before an element and need a space after.
    /// Example: `{a, /* comment */ b}` - the comment needs a space after it.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_trailing_space(
        &self,
        start: u32,
        end: u32,
    ) -> DocId {
        self.build_comments_between(start, end, CommentSpacing::Trailing)
    }

    /// Build a Doc for inline comments (trailing space), returning `None` if no comments.
    ///
    /// The `_opt` sibling of `build_inline_comments_between_doc_trailing_space`, matching
    /// the ones the other two spacings already have. Callers that push into a parts
    /// buffer want this rather than the `DocId` form: the `empty()` it would otherwise
    /// return is not free — `concat` keeps it as a child slot for the renderer and every
    /// `fits` pass to walk.
    #[inline]
    pub(crate) fn build_inline_comments_between_doc_trailing_space_opt(
        &self,
        start: u32,
        end: u32,
    ) -> Option<DocId> {
        self.build_comments_between_filtered_opt(
            start,
            end,
            CommentSpacing::Trailing,
            CommentFilter::All,
        )
    }

    /// Build inline comments between two positions with line-comment-safe trailing spacing.
    ///
    /// A block comment keeps the following value (or next comment) on its `*/`
    /// line when the source did (`/* comment */ expr`), and stays on its own line
    /// when the source broke (`/* comment */\nexpr`) — the author's layout is
    /// preserved. Line comments always get a hardline (`// comment\nexpr`) so they
    /// can't absorb the value as comment text.
    /// Use for any position where a comment appears before an expression (RHS of `=`,
    /// after keywords like `return`/`await`, after operators like `!`/`...`, etc.).
    pub(crate) fn build_rhs_comments_opt(&self, start: u32, end: u32) -> Option<DocId> {
        self.build_leading_comment_run_opt(start, end, LeadingGlue::Adjacent)
    }

    /// The **keyword→operand** gap router, shared by `new`→callee
    /// (`build_new_doc_with_wrapping`) and `await`→operand (`build_await_doc`) — the two
    /// expression-level gaps of that shape.
    ///
    /// One question, one predicate ([`Printer::comments_force_own_line_between`], i.e.
    /// the shared `comment_hangs_next`), so the two sites cannot answer differently:
    ///
    /// - It **hangs** (a line comment, or a multiline block the author broke after) →
    ///   [`KeywordOperandGap::Continuation`]: the caller emits its keyword bare and hands
    ///   its WHOLE tail to [`Printer::append_keyword_value_line_comments`], the
    ///   keyword→value seam (conformance_prettier.md §Uniform Forced-Continuation Indent
    ///   for the indent, §Comment Position Philosophy for the run placement — a comment
    ///   here leads the operand, so an own-line one keeps its line rather than pulling up
    ///   to trail the keyword). The tail is the whole operand — for `new`, callee, type
    ///   arguments and argument list alike — so a tail that breaks internally renders at
    ///   the continuation's indent rather than at the outer column.
    ///
    ///   ⚠️ Only the two keyword sites take that seam. The third consumer — a preserved
    ///   grouping pair's `(`→inner run — takes
    ///   [`Printer::build_value_slot_continuation_indent`] instead, for the reason stated
    ///   there: it sits in a slot whose separator someone else already emitted, so the
    ///   seam's own leading hardline would be a second break. It shares this router only
    ///   for the split, never for the emission.
    /// - Otherwise — a **single-line block in ANY authored position** (glued, trailing the
    ///   keyword, or on its own line) → [`KeywordOperandGap::Inline`]. Nothing forces it
    ///   off the line, so it trails inline and the author's break is reflowed: the
    ///   keyword→value rule its `as`/`satisfies`, `export =`, and module-header siblings
    ///   follow. See conformance_prettier.md §Authored breaks in value position.
    ///
    /// ⚠️ The inline arm must NOT go through the continuation `indent` "for uniformity".
    /// Nothing in the gap ends a line there, so the indent is not inert — it applies to
    /// every break the tail makes on its own WIDTH, pushing a broken argument list or
    /// object a level over (`await /* c */ fn({…})`).
    ///
    /// ⚠️ Emitting the hang case through `build_rhs_comments_opt` reads as the obvious
    /// code and was the bug this replaced: that builder picks each separator from the
    /// comment's AUTHORED position, so an own-line comment kept a hardline while the
    /// concat glued it to the keyword. The result — comment pulled up, break kept — *is*
    /// the glued authoring, which reflows inline on the next pass, so the format was not
    /// idempotent on its own output.
    ///
    /// ⚠️ **Do not merge this with `gap_comment_continuation_tail`** (the module-header
    /// gap emitter) on the strength of their matching gate→{hang, inline} shape. The
    /// resemblance is structural, not semantic — the *gates differ on purpose* for a
    /// **glued multiline block** (`kw /* …⏎… */ v`): this gate collapses it inline, while
    /// the header gap's `has_multiline_block_comments_on_page_between` hangs *any*
    /// multiline block, glued or not.
    pub(crate) fn keyword_operand_gap(
        &self,
        keyword_end: u32,
        operand_start: u32,
    ) -> KeywordOperandGap {
        if self.comments_force_own_line_between(keyword_end, operand_start) {
            KeywordOperandGap::Continuation
        } else {
            KeywordOperandGap::Inline(
                self.build_inline_comments_between_doc_trailing_space_opt(
                    keyword_end,
                    operand_start,
                ),
            )
        }
    }

    /// Like `build_rhs_comments_opt`, but an author blank line after a glued block's
    /// `*/` yields with the soft `line` instead of forcing the comment onto its own
    /// line — [`LeadingGlue::AdjacentValueGap`], the head→value gap rule. Use at a
    /// value gap (`=`, `:`, `as`, a keyword); a list gap stays on
    /// [`build_rhs_comments_opt`](Self::build_rhs_comments_opt), where a blank
    /// separates two items and is preserved.
    pub(crate) fn build_value_gap_comments_opt(&self, start: u32, end: u32) -> Option<DocId> {
        self.build_leading_comment_run_opt(start, end, LeadingGlue::AdjacentValueGap)
    }

    /// Like `build_rhs_comments_opt`, but a single-line block comment glued to the
    /// operator (not on its own line) hugs the value with a space even when the
    /// value follows on the next source line — prettier pulls the value up in the
    /// assignment/call layout (`= /* c */⏎v` → `= /* c */ v`). Positions that keep
    /// the author's line break for a glued block stay on the non-gluing
    /// `build_rhs_comments_opt` — a decorator is the clear case (`@dec /* c */⏎class`),
    /// since its following declaration owns its own line regardless.
    ///
    /// ⚠️ Don't grow an example list here without probing each entry: this comment
    /// previously named `await` operands and object property values as keeping the
    /// break, and **both actually collapse** (`await /* c */ x`, `k: /* c */ 1`).
    /// The gluing/non-gluing split is a property of each call site, so the call sites
    /// are the source of truth, not a list here.
    ///
    /// `return`/`throw` arguments pull up here too, but for a stronger reason than
    /// layout: they are restricted productions, so keeping the break would be ASI and
    /// would change the program. See `build_keyword_argument_doc`.
    pub(crate) fn build_rhs_comments_glued_opt(&self, start: u32, end: u32) -> Option<DocId> {
        self.build_leading_comment_run_opt(start, end, LeadingGlue::AdjacentGlued)
    }

    /// Emit a run of leading comments before `terminal_pos` — the value, member,
    /// item, or body the comments lead. Each comment is emitted with
    /// `build_comment_doc`, followed by one of three separators — prettier's
    /// `printLeadingComment` (`src/main/comments/print.js`), which reads only the
    /// source around *this* comment, never where `terminal_pos` is:
    ///
    /// - **space** — no newline after the `*/` (per `glue`): the comment is glued to
    ///   what follows, so it leads it inline (`/* c */ X`). A run the author glued
    ///   together therefore stays glued (`/* a */ /* b */⏎X`).
    /// - **`line`** — a newline after the `*/` but none before the `/*`: soft, so what
    ///   follows pulls up onto the comment's line when the enclosing group fits and
    ///   drops below when it breaks.
    /// - **`hardline`** — a newline on *both* sides (an own-line comment), or any line
    ///   comment (it must break, or it would absorb what follows). Blank-preserving:
    ///   an author blank line before the value / next comment is kept, matching
    ///   prettier everywhere in this "comment before expression" position (RHS of
    ///   `=`/`:`, call args, `return`/`await`, unary operands, …).
    ///
    /// `continuation` is emitted after each break, for a site whose run is not already
    /// inside a `d.indent()` and so must carry explicit `INDENT` text (the
    /// variable-declarator gap); every other site passes `d.empty()`.
    ///
    /// **Returns whether the run pushed a `hardline`** — i.e. whether it ends a line
    /// unconditionally. Only the third separator does; the space and the soft `line`
    /// both leave the enclosing group free to stay flat. A site that must open its
    /// own shell around a forced break needs that answer and cannot re-derive it:
    /// tsv has no `propagateBreaks`, so a hardline buried in this run is invisible to
    /// the group that would have to break for it ([`Printer::build_paren_leading_value_doc`]
    /// is the caller, and the import parens are the shell). Re-deriving it caller-side
    /// is the same three-way rule spelled a second time, which is what
    /// `docs/comments.md` §Leading comments exists to prevent — so the emitter reports
    /// on its own output instead. Callers with no shell to open ignore it.
    ///
    /// The single leading-comment emitter: every site that puts comments before an
    /// item routes here, so the rule lives once. Behind
    /// [`build_rhs_comments_opt`](Self::build_rhs_comments_opt),
    /// [`build_rhs_comments_glued_opt`](Self::build_rhs_comments_glued_opt), the
    /// arrow-body run, the member-leading sites (interface / intersection members),
    /// the comma-separated inter-item gaps (declarators, for-init, heritage,
    /// switch cases), the breaking chain gap's leading run
    /// (`chain::push_gap_comments_and_break`), the forced-multiline lists via
    /// [`build_leading_comments_multiline`](Self::build_leading_comments_multiline)
    /// (tuples, type params/args, function-type params, the union's first member, the
    /// bracket-break shell, the broken `<T>` cast), the array literal / array pattern
    /// element runs, the **non-block** header→body gap across `if` / `while` / the
    /// C-style `for` / for-in / for-of / `do`
    /// ([`Printer::push_indented_header_to_body_gap`]), the body/member runs via
    /// [`push_leading_comments_before`](Self::push_leading_comments_before) (class,
    /// interface and enum members, statement lists, type literals, expanded object
    /// patterns), the broken for-in/for-of header's `(`→binding gap, and — for all but
    /// its last comment —
    /// [`push_orphaned_comment_run`](Self::push_orphaned_comment_run).
    ///
    /// ⚠️ **This list is a WORK-LIST, not a summary** — an unlisted loop is a bug report,
    /// and both halves of that have already been paid here (the for-clause leading gap
    /// converged onto this emitter and stayed listed as a hand-roller; the labeled
    /// statement's run was a hand-roller and was never listed). **Four** loops still emit a
    /// leading run themselves, because their surrounding separator policy genuinely
    /// differs:
    ///
    /// - the **import/export specifier list** (`specifier_list.rs`);
    /// - the **union's inter-member run** (`types/union_intersection.rs`
    ///   `push_union_member_leading_run` / `build_member_leading_block_comments`), which
    ///   brackets the `| ` separator and preserves blanks in different positions;
    /// - the **control-flow gap builder's** `GapCommentRun::Leading` arm
    ///   (`Printer::build_comments_between_parts`, the **block**-body header→body gaps and
    ///   the `else` gap) — a between-parts shape whose separators go BEFORE each comment
    ///   and whose caller owns the tail. Its **non-block** twin
    ///   (`Printer::push_indented_header_to_body_gap`) is not among them: that gap is the
    ///   body's leading run outright and routes here;
    /// - the **labeled statement's relocated name→`:` run** (`try_jump.rs`
    ///   `build_labeled_statement_doc`), whose last comment must break whatever the source
    ///   says — the run has been relocated off the label it would otherwise hug.
    ///
    /// Each calls [`comment_hugs_next`](Self::comment_hugs_next) rather than re-deriving
    /// the rule, so what differs there is the loop, never the decision.
    pub(crate) fn push_leading_comment_run<'c>(
        &self,
        parts: &mut DocBuf,
        comments: impl Iterator<Item = &'c Comment>,
        terminal_pos: u32,
        glue: LeadingGlue,
        continuation: DocId,
    ) -> bool {
        let d = self.d();
        let mut forces_break = false;
        let mut is_first = true;
        let mut comments = comments.peekable();
        while let Some(comment) = comments.next() {
            // See `LeadingGlue::AdjacentAnchorLine`: only the run's first comment can have
            // its authored newline-before erased, and only at that one site.
            let first_shares_anchor_line = is_first && glue.first_shares_anchor_line();
            is_first = false;
            parts.push(self.build_comment_doc(comment));
            // The next thing after this comment — the following comment, or the
            // terminal (value/member/item/body) for the last one. Anchored on the
            // PHYSICAL next comment, not just the emitted one: an owned comment (glued
            // to the value, so printed by the value's node and skipped by the emit
            // iterator) still occupies the gap here, and both the glue test and the
            // blank-line scan below are physical questions. Anchoring past it would
            // unglue a run the author wrote glued (`/* a */ /* b⏎*/ v` → `/* a */` on
            // its own line) and, worse, read the owned comment's own newline as an
            // author blank line — inserting one on the next pass (non-idempotent).
            // Owned comments are always the glued suffix of a leading run, so this
            // only ever differs at the last emitted comment; bounding `blank_scan_end`
            // at the emit-next keeps it from over-reaching a caller's filtered set.
            let next = self.blank_scan_end(
                comment.span.end,
                comments.peek().map_or(terminal_pos, |c| c.span.start),
            );
            let hugs = match glue {
                // `AdjacentValueGap` differs from `Adjacent` only in the blank-line
                // rule below, not in the hug test — the soft `line` is the point at a
                // value gap (it lets a value too long for the comment's line break
                // below it), so it must not become an unconditional space.
                LeadingGlue::Adjacent
                | LeadingGlue::AdjacentValueGap
                | LeadingGlue::AdjacentAnchorLine => self.comment_hugs_next(comment),
                // A glued (not own-line) single-line block hugs across a source
                // newline; the hugs-what-follows case still hugs as in `Adjacent`.
                // ⚠️ That first half asks the comment's own neighbours, never the
                // distance to `next`: for the LAST comment `next` is the value's span
                // start, which sits INSIDE any grouping paren the author wrote, so a
                // break they put after the `(` read as a break after the comment and
                // un-glued a run prettier keeps glued (`= /* c */ (⏎v)`).
                LeadingGlue::AdjacentGlued => {
                    comment.is_block
                        && (self.comment_hugs_next(comment)
                            || !self.comment_cannot_glue_to_operator(comment))
                }
                // `Adjacent`, plus the stripped grouping paren the author glued the
                // comment to (`/* c */ (⏎…`) — invisible in the output, so the newline it
                // left behind must not un-glue the pair.
                LeadingGlue::AdjacentStrippedParen => {
                    self.comment_hugs_next(comment)
                        || (comment.is_block
                            && calls::has_stripped_paren_gap(self.source, comment.span.end, next))
                }
            };
            if hugs {
                // Value (or next comment) shares the `*/` line — keep it glued.
                parts.push(d.text(" "));
            } else if comment.is_block
                // `first_shares_anchor_line ||` reads as "this comment HAS no own line to
                // keep, so don't ask" — and short-circuits the source scan when so.
                && (first_shares_anchor_line || !self.is_own_line_comment(comment))
                && !(glue.blank_forces_own_line()
                    && self.has_blank_line_between_strict(comment.span.end, next))
            {
                // A block with a newline *after* its `*/` but none before its `/*`:
                // prettier's `printLeadingComment` emits a soft `line` here, so what
                // follows pulls up onto the comment's line when the enclosing group
                // fits and drops below when it breaks. An own-line block (newline on
                // both sides) takes the `hardline` branch instead.
                //
                // Whether a **blank** line after the `*/` overrides that and forces the
                // hardline is per-site (`LeadingGlue::blank_forces_own_line`): it does
                // in a list, where a blank between items is ordinary authoring tsv
                // preserves, and does not in a value gap, where the blank sits inside a
                // break already judged unforced.
                parts.push(d.line());
                parts.push(continuation);
            } else {
                // Line comment, or an own-line block: keep them on separate lines
                // (preserve the author's layout; a line comment must break so it
                // can't absorb the value).
                self.push_blank_preserving_hardline(parts, comment.span.end, next);
                parts.push(continuation);
                forces_break = true;
            }
        }
        forces_break
    }

    /// Push a leading block-comment run whose LAST comment the author broke after
    /// (`/* c */⏎<value>`) ahead of a value whose doc **will break** — the
    /// break-materialized form of prettier's `printLeadingComment` newline-after
    /// soft `line`. Prettier gets the materialization free from `propagateBreaks`
    /// (the value's hard break marks every enclosing group broken, so the `line`
    /// renders as a break); tsv has no `propagateBreaks`, so the caller asks
    /// `will_break` of the value doc and routes here only when it holds — the
    /// final separator is a (blank-preserving) `hardline` because the break is
    /// already forced, and forced is what lets it carry an author blank. The
    /// width-driven half of the same rule — no hard break, the seam itself
    /// deciding — is [`hang_after_operator_run_doc`](Self::hang_after_operator_run_doc),
    /// whose soft `line` rides the seam's own hang group and absorbs the blank
    /// with the unforced break. A value that FITS collapses the `line` to a space
    /// in both formatters, so the caller's glued path already renders that case.
    ///
    /// The run's interior takes the single leading-run emitter
    /// ([`push_leading_comment_run`](Self::push_leading_comment_run)) — glue,
    /// soft-line, and blank rules live there, asked of each comment's own
    /// neighbour, with the last comment's start as the interior's terminal. The
    /// caller supplies the value doc after this returns; the run always ends the
    /// line, so nothing may ride after it.
    pub(crate) fn push_leading_run_before_breaking_value(
        &self,
        parts: &mut DocBuf,
        comments: &[&'a Comment],
        value_start: u32,
    ) {
        if let Some(last) = self.push_breaking_value_run_body(parts, comments) {
            self.push_blank_preserving_hardline(parts, last.span.end, value_start);
        }
    }

    /// Push a broke-after run's interior (via the single leading-run emitter) and
    /// its last comment, leaving the final separator toward the value to the
    /// caller — the shared body of
    /// [`push_leading_run_before_breaking_value`](Self::push_leading_run_before_breaking_value)
    /// (a blank-preserving hardline) and
    /// [`hang_after_operator_run_doc`](Self::hang_after_operator_run_doc) (a soft
    /// `line`). Returns the last comment so the caller can anchor that separator.
    fn push_breaking_value_run_body(
        &self,
        parts: &mut DocBuf,
        comments: &[&'a Comment],
    ) -> Option<&'a Comment> {
        let (last, interior) = comments.split_last()?;
        self.push_leading_comment_run(
            parts,
            interior.iter().copied(),
            last.span.start,
            LeadingGlue::Adjacent,
            self.d().empty(),
        );
        parts.push(self.build_comment_doc(last));
        Some(last)
    }

    /// The `=`-seam emission for a broke-after run before a value that **will
    /// break** — prettier's break-after-operator body, everything after the
    /// operator in one indent: a hardline, the run
    /// ([`Self::push_leading_run_before_breaking_value`]), then the value. The
    /// type-alias, variable-declarator, and assignment-expression `=` seams share
    /// it; the `:` seam emits the run un-indented on the caller's own line
    /// instead and does not.
    pub(crate) fn break_after_operator_run_doc(
        &self,
        run: &[&Comment],
        value_start: u32,
        value_doc: DocId,
    ) -> DocId {
        let d = self.d();
        let mut indent_content = DocBuf::new();
        indent_content.push(d.hardline());
        self.push_leading_run_before_breaking_value(&mut indent_content, run, value_start);
        indent_content.push(value_doc);
        d.indent(d.concat(&indent_content))
    }

    /// The width-keyed sibling of [`Self::break_after_operator_run_doc`], for a
    /// broke-after run before a value whose doc carries NO hard break: the run,
    /// its newline-after soft `line`, and the value inside ONE
    /// `hang_after_operator` group, so the separator materializes exactly when
    /// the operator seam itself breaks — prettier's `printLeadingComment` `line`,
    /// which rides the enclosing assignment group and breaks with it (its
    /// hard-break half comes free from `propagateBreaks`, and that half is
    /// [`Self::break_after_operator_run_doc`]'s job here). Flat, the group
    /// renders the glued bytes (`= /* c */ value`); broken, the run holds its
    /// own line and the value — its own group — re-fits below
    /// (`=⏎\t/* c */⏎\tvalue`), exploding further only when that line also
    /// overflows. An author blank after the run yields with the unforced break
    /// ([`LeadingGlue::AdjacentValueGap`]'s rule), unlike the hard sibling,
    /// whose forced break carries it.
    pub(crate) fn hang_after_operator_run_doc(
        &self,
        run: &[&'a Comment],
        value_doc: DocId,
    ) -> DocId {
        let d = self.d();
        let mut content = DocBuf::new();
        if self
            .push_breaking_value_run_body(&mut content, run)
            .is_some()
        {
            content.push(d.line());
        }
        content.push(value_doc);
        layout::hang_after_operator(d, d.concat(&content))
    }

    /// Push a broke-after run and its newline-after soft `line` — the
    /// group-riding form of the same rule
    /// ([`push_leading_run_before_breaking_value`](Self::push_leading_run_before_breaking_value)
    /// is the hard-break form, [`hang_after_operator_run_doc`](Self::hang_after_operator_run_doc)
    /// the seam-grouped one). The caller places the run inside an existing group
    /// (an argument list's wrap), so the `line` materializes exactly when that
    /// group breaks — the value's own hard break, a sibling's, or width — and
    /// collapses to the glued bytes when it stays flat. That is prettier's
    /// `printLeadingComment` `line` riding the argument group, which its
    /// `propagateBreaks` marks broken for any of those reasons.
    pub(crate) fn push_leading_run_with_soft_line(&self, parts: &mut DocBuf, run: &[&'a Comment]) {
        if self.push_breaking_value_run_body(parts, run).is_some() {
            parts.push(self.d().line());
        }
    }

    /// [`Self::broke_after_value_leading_run`] restricted to a run that TRAILS its
    /// opening delimiter — every comment on the `(`'s own line. The call-argument
    /// seams need the narrower question: an own-line-authored leading run keeps its
    /// break unconditionally there (`calls/leading_arg_block_comment_newline`'s
    /// second case), so only the run the author glued to the opener and broke
    /// after may ride a collapsible soft `line`. The keyword seams (spread dots,
    /// `await`) deliberately take the wider gate — their target pulls an own-line
    /// run up onto the keyword's line.
    pub(crate) fn opener_trailing_broke_after_run(
        &self,
        opener: u32,
        value_start: u32,
    ) -> Option<CommentVec<'a>> {
        let run = self.broke_after_value_leading_run(opener, value_start)?;
        self.run_takes_soft_separator(opener, value_start, &run)
            .then_some(run)
    }

    /// May this broke-after run ride a SOFT separator (the hang / argument-group
    /// `line`)? Only when it TRAILS its opener — every comment on the opener's
    /// line; an own-line-authored run keeps its break unconditionally at these
    /// seams — AND no author blank separates it from the value. A soft `line`
    /// cannot carry a blank (only the hard half's blank-preserving hardline can),
    /// so emitting anyway DROPS the blank, and the own-line form that leaves
    /// behind re-reads as a different authoring on the next pass — a 2-cycle the
    /// blanks:audit catches as NON-IDEMPOTENT. A blank declines to the caller's
    /// blank-aware layouts instead. Asked by
    /// [`Self::opener_trailing_broke_after_run`] and, after the shared geometry
    /// gate has already run, by the declarator / assignment `=` seams (whose HARD
    /// half takes an own-line-authored or blank-separated run too) and the
    /// arrow's soft arms.
    pub(crate) fn run_takes_soft_separator(
        &self,
        opener: u32,
        value_start: u32,
        run: &[&'a Comment],
    ) -> bool {
        run.iter().all(|c| self.is_same_line(opener, c.span.start))
            && run
                .last()
                .is_none_or(|last| !self.has_blank_line_between(last.span.end, value_start))
    }

    /// [`Self::break_after_operator_run_doc`] or
    /// [`Self::hang_after_operator_run_doc`], chosen by the value's own hard
    /// break — the two-half dispatch the operator seams share (the type-alias,
    /// declarator, and assignment `=`). The arrow's hug arm asks the hard half
    /// alone: its glued hug already renders the width case.
    pub(crate) fn break_or_hang_after_operator_run_doc(
        &self,
        run: &[&'a Comment],
        value_start: u32,
        value_doc: DocId,
    ) -> DocId {
        if self.d().will_break(value_doc) {
            self.break_after_operator_run_doc(run, value_start, value_doc)
        } else {
            self.hang_after_operator_run_doc(run, value_doc)
        }
    }

    /// The whole declarator / assignment-expression `=` broke-after arm —
    /// prettier's break-after-operator (`chooseLayout`'s `hasLeadingOwnLineComment`
    /// arm) in the type-alias seam's two-half form: the geometry gate
    /// ([`Self::broke_after_value_leading_run`]), the value built through the
    /// caller's own builder, then `Some(rhs_doc)` when either half applies —
    /// a value carrying a hard break takes the blank-preserving hardline emitter
    /// (any run authoring: the own-line emission is the preserved own-line shape),
    /// anything else one hang group whose soft `line` breaks exactly when the
    /// operator seam does, restricted to a run that may ride a soft separator
    /// ([`Self::run_takes_soft_separator`]: trails the operator, no author blank).
    /// A value that FITS collapses to the glued bytes in both formatters; a
    /// width-broken one holds the run on its own line with the value re-fitting
    /// below instead of stranding it mid-line (`⏎\t/* c */ aaa +`, a form
    /// prettier never emits). Declines — a line comment (its mandatory-break path
    /// owns it), an own-line-authored or blank-separated run before a fitting
    /// value — keep the caller's layouts, which preserve those breaks; the value
    /// doc built for a declined probe is discarded, as the combined gate's is.
    /// The returned doc begins after the operator; the caller emits the LHS and
    /// the operator text before it.
    pub(crate) fn broke_after_operator_rhs_doc(
        &self,
        gap_start: u32,
        value_start: u32,
        build_value: impl FnOnce() -> DocId,
    ) -> Option<DocId> {
        let run = self.broke_after_value_leading_run(gap_start, value_start)?;
        let value_doc = build_value();
        (self.d().will_break(value_doc)
            || self.run_takes_soft_separator(gap_start, value_start, &run))
        .then(|| self.break_or_hang_after_operator_run_doc(&run, value_start, value_doc))
    }

    /// The geometry half of the broke-after value gate: `Some(run)` when the
    /// `[gap_start, value_start)` gap holds a block-only leading run whose last
    /// comment the author broke after. Anything else — an empty gap, a line
    /// comment (those routes own their own mandatory-break layouts), a glued
    /// last comment — declines, and the caller keeps its glued path, whose flat
    /// render is the same bytes prettier's collapsed `line` produces. Asks
    /// nothing of the value doc: the caller pairs a hit with
    /// [`Self::break_after_operator_run_doc`] when the value `will_break` (the
    /// hard materialization) and with [`Self::hang_after_operator_run_doc`]
    /// otherwise (the width-keyed one); a caller whose other layouts already
    /// render the width-driven case takes the combined
    /// [`Self::breaking_value_leading_run`] instead.
    ///
    /// ⚠️ The broke-after question is asked against the **physical** next thing in
    /// the gap, not the emit-set's view of it: an OWNED comment glued to the value
    /// (`= /* c */ /* m⏎ */ v`) is printed by the value's doc and absent from the
    /// emit run, but it still occupies the gap — measured against `value_start`,
    /// the emitted run's tail reads as broken-after when the author glued the whole
    /// run, and the owned multiline block's own newlines answer `will_break` for a
    /// value prettier keeps flat (its `fits` stops at the first newline in comment
    /// text). Any owned comment between the run and the value therefore declines —
    /// via the blank scan's in-source ceiling, which stops at the owned comment, not
    /// via the glue test below.
    ///
    /// ⚠️ **The glue test asks the comment's own NEIGHBOURS, never the distance to the
    /// value** ([`Self::comment_hugs_next`], prettier's `hasNewline(text,
    /// locEnd(comment))` — spaces and tabs skipped, nothing else). The distance
    /// reading (`is_same_line(last.end, value_start)`) is blind to anything the printer
    /// erases or the value's own shell contributes: a **paren** standing between the run
    /// and the value took the author's break INSIDE those parens (`= /* c */ (⏎v)`) and
    /// reported it as a break after the comment, so the seam broke a value prettier
    /// hugs. It reached three seams off this one predicate — the arrow `=>`, the
    /// declarator/assignment `=`, and `await` — because the paren need not even be
    /// stripped: an object arrow body keeps its required parens and the inner node's
    /// span still starts past the `(`
    /// (`docs/comments.md`, and the standing rule that a bound derived from a node span
    /// is not the printed edge).
    pub(crate) fn broke_after_value_leading_run(
        &self,
        gap_start: u32,
        value_start: u32,
    ) -> Option<CommentVec<'a>> {
        let comments: CommentVec<'a> =
            comments_to_emit_in_range(self.comments, gap_start, value_start).collect();
        let last = comments.last()?;
        if !comments.iter().all(|c| c.is_block)
            || self.blank_scan_end(last.span.end, value_start) != value_start
            || self.comment_hugs_next(last)
        {
            return None;
        }
        Some(comments)
    }

    /// [`Self::broke_after_value_leading_run`] plus the hard-break question:
    /// `Some((run, value_doc))` only when the value doc (built here, via the
    /// caller's builder) **will break** — the gate for
    /// [`push_leading_run_before_breaking_value`](Self::push_leading_run_before_breaking_value)
    /// at the seams whose remaining layouts already render the width-driven case
    /// (the simple `:` annotation, the spread's dots→argument gap, the `await`
    /// keyword→argument gap). The arrow's hug arm spells the same pair out itself
    /// — its geometry half is [`Printer::arrow_gap_broke_after_run`], which the
    /// enclosing call must ask too. A value that fits declines,
    /// and the caller keeps its glued path. The declarator and
    /// assignment-expression `=` instead split the gate — geometry here, the two
    /// break halves via
    /// [`break_or_hang_after_operator_run_doc`](Self::break_or_hang_after_operator_run_doc)
    /// — because their width half is real but restricted to a run that trails
    /// the operator with no blank after it ([`Self::run_takes_soft_separator`]).
    pub(crate) fn breaking_value_leading_run(
        &self,
        gap_start: u32,
        value_start: u32,
        build_value: impl FnOnce() -> DocId,
    ) -> Option<(CommentVec<'a>, DocId)> {
        let comments = self.broke_after_value_leading_run(gap_start, value_start)?;
        let value_doc = build_value();
        if self.d().will_break(value_doc) {
            Some((comments, value_doc))
        } else {
            None
        }
    }

    /// Build a leading-comment run over `[start, end)` into a fresh `DocBuf`,
    /// returning `None` when the range holds no comments. The `Option`-returning
    /// form of [`push_leading_comment_run`](Self::push_leading_comment_run) that
    /// the RHS-comment wrappers use.
    fn build_leading_comment_run_opt(
        &self,
        start: u32,
        end: u32,
        glue: LeadingGlue,
    ) -> Option<DocId> {
        self.build_leading_comment_run_with_break(start, end, glue, None)
            .map(|(doc, _)| doc)
    }

    /// [`build_leading_comment_run_opt`](Self::build_leading_comment_run_opt) plus the
    /// run's own forced-break report — for the caller that must open a **shell** around
    /// it ([`Self::build_paren_leading_value_doc`], the `import(…)` parens).
    ///
    /// The two share one body because the shell question is not separable from the run:
    /// re-deriving "did this break" caller-side is
    /// [`push_leading_comment_run`](Self::push_leading_comment_run)'s three-way rule
    /// spelled a second time, and a *range*-shaped approximation of it is the bug the
    /// import gap had. Callers with no shell take the `_opt` form and the report is
    /// dropped here rather than at each of them.
    /// `skip_delim` drops the comments an opening delimiter already pulled onto its own
    /// line ([`Printer::comment_on_delimiter_line`]) — the exclusion every consumer of
    /// [`Printer::delimiter_line_comment_prefix`] owes, without which the pulled run
    /// prints twice.
    fn build_leading_comment_run_with_break(
        &self,
        start: u32,
        end: u32,
        glue: LeadingGlue,
        skip_delim: Option<u32>,
    ) -> Option<(DocId, bool)> {
        let mut parts = DocBuf::new();
        let forces_break = self.push_leading_comment_run(
            &mut parts,
            comments_to_emit_in_range(self.comments, start, end)
                .filter(|c| !skip_delim.is_some_and(|pos| self.comment_on_delimiter_line(pos, c))),
            end,
            glue,
            self.d().empty(),
        );
        (!parts.is_empty()).then(|| (self.d().concat(&parts), forces_break))
    }

    /// Prepend optional RHS leading comments — block comments in the gap between an
    /// `=`/`:` and the value — to an already-built `value_doc`, returning `value_doc`
    /// unchanged when the gap carries none. Centralizes the `match { Some(c) =>
    /// concat([c, v]), None => v }` idiom shared by the initializer/property value
    /// sites (variable declarators, class properties, enum members, object property
    /// values, import-attribute values).
    ///
    /// Every caller is a head→value gap, so the run is built in the value-gap mode
    /// ([`LeadingGlue::AdjacentValueGap`]) — an author blank line after a glued block
    /// yields with the break rather than forcing the comment onto its own line. A
    /// *list* gap must not route here; it wants
    /// [`build_rhs_comments_opt`](Self::build_rhs_comments_opt).
    pub(crate) fn prepend_rhs_comments(
        &self,
        value_doc: DocId,
        start: u32,
        value_start: u32,
    ) -> DocId {
        match self.build_value_gap_comments_opt(start, value_start) {
            Some(comments_doc) => self.d().concat(&[comments_doc, value_doc]),
            None => value_doc,
        }
    }

    /// Build the `= value` RHS for an initializer whose `=`→value gap
    /// (`eq_pos + 1 .. value_start`) holds a comment that forces break handling,
    /// or `None` when the caller should emit its normal inline `= value` form (no
    /// comment, or a single inline block that glues to the value). The returned doc
    /// begins at `operator` (`" ="`, or an assignment expression's `" +="` and
    /// friends); the caller emits the LHS (name/pattern) before it.
    /// `build_value` is called only when a break is forced, so a comment-free
    /// initializer never pays to build the value doc here.
    ///
    /// Shared by variable declarators, for-loop init clauses, enum members, and the
    /// assignment expression's line-comment arm so they all place a comment after the
    /// operator identically. That sharing is the point: the enum
    /// member emitted its own positional run instead, and drifted twice over — it
    /// preserved a break the others reflow, and relocated an own-line comment onto the
    /// `=` line, which is not idempotent (the moved comment reads as glued next pass).
    /// A new `=`→value gap should route here rather than re-derive the layout:
    ///
    /// - **Line comment** after `=`: mandatory break after `=`. A comment on the
    ///   `=`'s line trails it inline; a comment on its own line leads the value on
    ///   its own line (author blank lines preserved). Diverges from prettier, which
    ///   relocates the line comment to trail the whole statement — tsv preserves the
    ///   author's placement (see [`conformance_prettier_ts_comments.md` §Comment relocation]).
    /// - **Own-line block, or multiline block the author broke after**: break-after-
    ///   operator hang, the comment on its own line (matches prettier's
    ///   `hasLeadingOwnLineComment`; the broke-after multiline case is tsv's own
    ///   authored-break rule — conformance_prettier_ts_comments.md §Pre-separator
    ///   multiline block).
    /// - **Inline block (or glued run, multiline members included) reaching the
    ///   value**, or no comment: `None` — the caller keeps the value on the `=` line
    ///   (`= /* c */ value`, `= /* c */ /* x⏎y */ value`).
    pub(crate) fn build_eq_comment_break_rhs(
        &self,
        eq_pos: u32,
        value_start: u32,
        operator: &'static str,
        build_value: impl FnOnce() -> DocId,
    ) -> Option<DocId> {
        let d = self.d();
        if !self.has_comments_to_emit_between(eq_pos + 1, value_start) {
            return None;
        }
        if self.has_line_comments_between(eq_pos + 1, value_start) {
            // Line comment → mandatory break. Partition the run: a comment on the
            // `=`'s line trails it; the rest lead the value on their own lines.
            let after_eq: CommentVec<'_> =
                comments_to_emit_in_range(self.comments, eq_pos + 1, value_start).collect();
            let mut trailing = DocBuf::new();
            let mut leading = DocBuf::new();
            for (ci, comment) in after_eq.iter().enumerate() {
                if self.is_same_line(eq_pos, comment.span.start) {
                    trailing.push(d.text(" "));
                    trailing.push(self.build_comment_doc(comment));
                } else {
                    leading.push(self.build_comment_doc(comment));
                    self.push_leading_run_separator(
                        &mut leading,
                        comment,
                        after_eq.get(ci + 1).map_or(value_start, |c| c.span.start),
                    );
                }
            }
            Some(d.concat(&[
                d.text(operator),
                d.concat(&trailing),
                d.indent_hardline(d.concat(&[d.concat(&leading), build_value()])),
            ]))
        } else if self.comment_hangs_value_after_operator(eq_pos + 1, value_start) {
            // The operator→value rule — both conjuncts, and the reasoning for each,
            // live on [`Self::comment_hangs_value_after_operator`]. A glued single-line
            // block trailing the `=` (`= /* c */⏎v`) stays with the `=` (the `None`
            // arm below), and a run whose glue chain reaches the value hangs nothing.
            // Own-line / multiline block → break-after-operator hang.
            let comments_doc = self
                .build_rhs_comments_opt(eq_pos + 1, value_start)
                .unwrap_or_else(|| d.empty());
            Some(d.concat(&[
                d.text(operator),
                layout::hang_after_operator(d, d.concat(&[comments_doc, build_value()])),
            ]))
        } else {
            // Only an inline block glued to `=`: caller emits `= /* c */ value`.
            None
        }
    }
}
