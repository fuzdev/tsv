// Member-keyword / modifier-marker / heritage comment emitters.
//
// These preserve comments in the gaps of a declaration header: between member
// keywords (`static` / `readonly` / `get` / `set`), around optional/definite
// markers (`?` / `!`), in the marker→`:` and keyword→name gaps, and within
// heritage clauses (`extends` / `implements`).

use super::layout::hang_after_operator;
use super::{CommentFilter, CommentSpacing, CommentVec, LeadingGlue, Printer};
use crate::ast::internal;
use crate::printer::analysis;
use smallvec::{SmallVec, smallvec};
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::{TriviaProfile, find_char, find_char_skipping_comments};

/// What follows the operator in a
/// [`Printer::build_operator_value_continuation`] tail — everything that seam needs to
/// answer its separator question, in one value so a caller cannot supply half of it.
///
/// The gap's far bound and "does the value's own doc lead with an own-line JSDoc cast"
/// are two readings of the same value, and passing the bound alone is what let the cast
/// go unseen: the gap's to-emit lookup skips an owned comment, so an empty answer there
/// means "nothing *the gap* prints", never "nothing prints".
#[derive(Clone, Copy)]
pub(crate) enum ContinuationValue<'e, 'ast> {
    /// An ordinary expression value. Its span start bounds the gap, and its doc may lead
    /// with a comment it OWNS — which no gap lookup can see.
    Expression(&'e internal::Expression<'ast>),
    /// A value that owns no comment and is not an expression — a type alias's RHS, whose
    /// gap is bounded by the start alone.
    Opaque(u32),
}

impl ContinuationValue<'_, '_> {
    /// The operator→value gap's far bound: where the value's own first token begins.
    fn gap_end(self) -> u32 {
        match self {
            Self::Expression(expr) => expr.span().start,
            Self::Opaque(start) => start,
        }
    }

    /// Whether the value's own doc opens with a JSDoc cast's comment on a line of its own —
    /// the one owned shape that prints its own hardline
    /// ([`Printer::is_own_line_jsdoc_cast`]).
    fn leads_with_own_line_jsdoc_cast(self, printer: &Printer<'_>) -> bool {
        match self {
            Self::Expression(expr) => printer.is_own_line_jsdoc_cast(expr),
            Self::Opaque(_) => false,
        }
    }
}

/// How one heritage inter-item gap splits between the preceding item's doc and the
/// join separator. The gap holds a comma, any comments, and the break to the next
/// item; which of those the item's doc already emitted decides what the separator
/// must add, so the two are read off one value instead of re-derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeritageGap {
    /// The whole gap is baked into the preceding item's doc. A line comment in the
    /// gap forces the break, so the gap emitter owns the comma, the comments, *and*
    /// the break (it must, to let a block glued to the next item hug it). The
    /// separator emits nothing.
    ///
    /// Why the *preceding* item, when a block-only gap instead leads the **next** one
    /// (the leading branch in `build_heritage_clause_doc`)? Because this gap's split —
    /// which comments trail the comma vs lead the next item — is one derivation, and
    /// `push_inter_item_line_comment_gap` makes it once. Handing the leading half to
    /// the next item's doc would force *it* to re-derive where the previous item's tail
    /// stopped, and two derivations of one boundary drift apart (a
    /// block-only gap escapes this only because both sides call
    /// `is_stranded_after_comma_block` with identical arguments).
    Baked,
    /// The comma is baked — a **stranded** after-comma block trails it on its line
    /// (`A, /* c */⏎B`) — but the break is the separator's.
    CommaBaked,
    /// Nothing is baked; the separator emits comma + break.
    Open,
}

/// A heritage-clause keyword (`extends` / `implements`). Carried as an enum
/// rather than `&str` so the keyword text and its spaced form are total — no
/// stringly-typed fallback that needs an unreachable arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeritageKeyword {
    Extends,
    Implements,
}

impl HeritageKeyword {
    /// The keyword text (`"extends"` / `"implements"`).
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Extends => "extends",
            Self::Implements => "implements",
        }
    }

    /// The keyword text with a trailing space (`"extends "` / `"implements "`).
    pub(crate) fn with_space(self) -> &'static str {
        match self {
            Self::Extends => "extends ",
            Self::Implements => "implements ",
        }
    }
}

/// The class-member modifiers that print as one canonical run, shared by properties
/// and methods — see [`Printer::push_class_member_modifiers_doc`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClassMemberModifiers {
    pub accessibility: Option<internal::Accessibility>,
    pub is_static: bool,
    pub r#abstract: bool,
    pub r#override: bool,
}

impl<'a> Printer<'a> {
    /// Emit a member keyword (modifier like `static ` / `readonly `, or
    /// accessor `get ` / `set `) preserving comments BEFORE it: the range
    /// `(cursor, keyword_pos)` is emitted ahead of the keyword text, so a chain
    /// of calls keeps each comment at the user's position
    /// (`static /* c */ readonly p`). Advances `cursor` past the keyword.
    ///
    /// Callers finish the chain with [`Self::push_pre_name_comments_doc`] for
    /// the final `(cursor, name_start)` range.
    pub(crate) fn push_member_keyword_doc(
        &self,
        parts: &mut DocBuf,
        kind_text: &'static str,
        cursor: &mut u32,
        bound: u32,
    ) {
        let keyword = kind_text.trim_end();
        if let Some(kw_pos) = self.find_keyword_in_range(*cursor, bound, keyword) {
            if self.has_comments_to_emit_between(*cursor, kw_pos) {
                parts.push(self.build_trailing_comments_hang_next(*cursor, kw_pos));
            }
            *cursor = kw_pos + keyword.len() as u32;
        }
        parts.push(self.d().text(kind_text));
    }

    /// Emit the modifier keywords a class **property** and a class **method** share,
    /// in tsc's canonical order: `accessibility → static → abstract → override`.
    ///
    /// One emitter rather than two parallel chains, so the ordering rule and the hazard
    /// below have a single home; the two member printers diverge only after this run
    /// (`readonly`/`accessor` on a property, `async`/`*` on a method), and `declare` sits
    /// ahead of it on the property alone — a method has no `declare` form.
    ///
    /// ⚠️ **`abstract` precedes `override`** — tsc's canonical order, the only one it
    /// accepts (TS1029 otherwise), and what prettier emits. tsv's parser deliberately
    /// takes the reversed spelling too, so this run *normalizes* it.
    ///
    /// TODO: [`Self::push_member_keyword_doc`] scans FORWARD from `cursor`, so on that
    /// reversed `override abstract` source the `abstract` lookup runs first and the
    /// `override` one then finds nothing. Harmless for the keywords themselves, but a
    /// comment between the two relocates (`override /* c */ abstract x` →
    /// `/* c */ abstract override x`, where prettier gives `abstract override /* c */ x`).
    /// Fixing it needs this emitter to walk the modifiers in SOURCE order while printing
    /// them in canonical order.
    pub(crate) fn push_class_member_modifiers_doc(
        &self,
        parts: &mut DocBuf,
        modifiers: ClassMemberModifiers,
        cursor: &mut u32,
        bound: u32,
    ) {
        if let Some(accessibility) = modifiers.accessibility {
            self.push_member_keyword_doc(parts, accessibility.as_keyword(), cursor, bound);
        }
        if modifiers.is_static {
            self.push_member_keyword_doc(parts, "static ", cursor, bound);
        }
        if modifiers.r#abstract {
            self.push_member_keyword_doc(parts, "abstract ", cursor, bound);
        }
        if modifiers.r#override {
            self.push_member_keyword_doc(parts, "override ", cursor, bound);
        }
    }

    /// Emit comments between the last member keyword and the member name
    /// (e.g., `get /* c */ a()`); block comments get a trailing space, line
    /// comments a hardline.
    pub(crate) fn push_pre_name_comments_doc(
        &self,
        parts: &mut DocBuf,
        cursor: u32,
        name_start: u32,
    ) {
        if self.has_comments_to_emit_between(cursor, name_start) {
            parts.push(self.build_trailing_comments_hang_next(cursor, name_start));
        }
    }

    /// The end bound for a member's pre-name comment scan: the computed key's
    /// `[` (via [`Self::find_opening_bracket_after`]) when `computed`, else the
    /// key's start `key_start`.
    ///
    /// Comments *inside* `[ … ]` belong to the computed-key bracket builder
    /// (`build_computed_key_bracket_doc`), so a keyword/marker emitter that
    /// scanned all the way to the key expression's start (which lies past `[`)
    /// would emit them a second time — duplicating the comment onto the keyword
    /// (`get /* c */ [/* c */ a]`, `*/* c */ [/* c */ a]`). A comment the author
    /// wrote *before* `[` (`get /* c */ [a]`) still falls in the bounded range
    /// and stays with the keyword. Shared by the accessor-keyword, generator-`*`,
    /// and async-method pre-name emitters; the class member path inlines the same
    /// `[`-bound directly.
    pub(in crate::printer) fn computed_key_name_bound(
        &self,
        from: u32,
        key_start: u32,
        computed: bool,
    ) -> u32 {
        if computed {
            self.find_opening_bracket_after(from, key_start)
        } else {
            key_start
        }
    }

    /// Emit an accessor keyword (`get ` / `set `) preserving comments between
    /// the keyword and the key (e.g., `get /* c */ a()`).
    ///
    /// Single-keyword convenience over [`Self::push_member_keyword_doc`] +
    /// [`Self::push_pre_name_comments_doc`]; `search_from` is the member's start.
    /// The pre-name scan is bounded at `[` for a computed key
    /// ([`Self::computed_key_name_bound`]) so an in-bracket comment isn't emitted
    /// twice.
    pub(crate) fn push_accessor_keyword_doc(
        &self,
        parts: &mut DocBuf,
        kind_text: &'static str,
        search_from: u32,
        key_start: u32,
        computed: bool,
    ) {
        let mut cursor = search_from;
        self.push_member_keyword_doc(parts, kind_text, &mut cursor, key_start);
        let name_bound = self.computed_key_name_bound(cursor, key_start, computed);
        self.push_pre_name_comments_doc(parts, cursor, name_bound);
    }

    /// Emit an optional/definite modifier marker (`?` or `!`) that follows a key
    /// or name, preserving comments between the name and the marker
    /// (e.g., `a /* c */?: number`). Returns the position after the marker.
    ///
    /// Scans for the first `marker` byte outside comments, unbounded to the end
    /// of source: the AST flag is only set when the parser consumed the marker
    /// directly after the name (whitespace and comments only in between), so the
    /// first non-comment occurrence is always the right one. Callers must NOT
    /// derive a search bound from spans — spans exclude the marker in some shapes
    /// (`let a! = x`, `interface I { a? }`), which is how past panics happened.
    pub(crate) fn push_modifier_marker_doc(
        &self,
        parts: &mut DocBuf,
        after: u32,
        marker: u8,
    ) -> u32 {
        let d = self.d();
        #[expect(clippy::expect_used)] // Parser guarantees the marker exists when the flag is set
        let pos = find_char_skipping_comments(
            self.source.as_bytes(),
            after as usize,
            self.source.len(),
            marker,
        )
        .expect("modifier marker (`?`/`!`) not found") as u32;
        let marker_doc = d.text(if marker == b'?' { "?" } else { "!" });
        // A line comment between the name and the marker keeps the comment after
        // the name and drops the marker (and whatever the caller appends next — the
        // `: type` / `(params)`) to a continuation line indented one level
        // (`a // c⏎\t?: T`). Block stays inline (`a /* c */?`). Prettier relocates a
        // such comment — a `_prettier_divergence` (conformance_prettier_ts_comments.md
        // §Comment relocation). The marker is the continuation `tail`; later pushes continue
        // mid-line after it.
        if self.has_line_comments_between(after, pos) {
            parts.push(self.build_continuation_indent(after, pos, marker_doc));
        } else {
            if self.has_comments_to_emit_between(after, pos) {
                parts.push(self.build_inline_comments_between_doc(after, pos));
            }
            parts.push(marker_doc);
        }
        pos + 1
    }

    /// Emit a binding identifier's HEAD — its name plus any optional `?` marker — and
    /// return the offset just past it: the left edge of the annotation's marker→`:` gap.
    ///
    /// The name is the leading `name_len` bytes of the span, but the span runs on over
    /// `?`/`!`/`: T`, so the head's own end has to be scanned for
    /// ([`analysis::skip_identifier_at`]) rather than read off the node — and the scan is
    /// bounded by the annotation's `:` when there is one, so it can never run past the head
    /// into the type. That derivation plus the marker push is the whole head, and the two
    /// belong together because their *product* is the gap edge every annotation emitter
    /// then keys on.
    ///
    /// Stated once because respelling it is what the function-type hug did — it pushed the
    /// name and a bare `d.text("?")`, which silently skipped both gaps
    /// [`Self::push_modifier_marker_doc`] owns (`docs/comments.md` hazard 4, the head
    /// variant). A caller that needs an unusual annotation BODY passes it to
    /// [`Self::build_binding_type_annotation_doc_with`]; nobody needs an unusual head.
    pub(crate) fn push_identifier_head_doc(
        &self,
        parts: &mut DocBuf,
        id: &internal::Identifier<'_>,
    ) -> u32 {
        parts.push(self.identifier_name_doc(id));
        let search_end = id.type_annotation().map_or(id.span.end, |ta| ta.span.start);
        let name_end = analysis::skip_identifier_at(
            self.source.as_bytes(),
            id.span.start as usize,
            search_end as usize,
        ) as u32;
        if id.optional {
            self.push_modifier_marker_doc(parts, name_end, b'?')
        } else {
            name_end
        }
    }

    /// Emit comments in the gap between an optional `?`/`!` marker and a member's
    /// type annotation `:`, preserving the user's placement *after* the marker.
    ///
    /// A block comment stays inline with a trailing space before `:`
    /// (`a? /* c */ : T`); a line comment forces a hardline so the `: T`
    /// annotation drops to the next line instead of being swallowed as comment
    /// text (`a? // c⏎: T`) — a content-loss / non-idempotency fix. Prettier
    /// instead relocates such comments (a block before `?`, a line after the
    /// member `;`), so the preserved forms are `_prettier_divergence`s
    /// ([conformance_prettier_ts_comments.md](../../../../docs/conformance_prettier_ts_comments.md)
    /// §Comment relocation).
    ///
    /// Shared by the three type-element property arms (type-literal, interface,
    /// class) and the index-signature key→`:` gap (where the "marker" is the key
    /// name). Returns `None` when the range has no comments.
    pub(crate) fn build_marker_to_colon_comments_doc(
        &self,
        after: u32,
        colon_start: u32,
    ) -> Option<DocId> {
        let comments = self.build_name_to_type_params_comments_opt(
            after,
            colon_start,
            CommentSpacing::Leading,
        )?;
        let d = self.d();
        if self.has_line_comments_between(after, colon_start) {
            // A line comment already ended its line with a hardline; `:` follows
            // on the next line, so no extra space.
            Some(comments)
        } else {
            // Block-only: single space before `:` (matches bare `?:` spacing).
            Some(d.concat(&[comments, d.text(" ")]))
        }
    }

    /// The uniform forced-continuation indent shape, the single definition shared by
    /// every `head // c⏎ tail` continuation. Emits a leading space, then the gap's
    /// comments via `build_trailing_comments_hang_next` (each line comment
    /// terminated at end-of-line so a `//` can't swallow what follows), then `tail` —
    /// all wrapped in one `indent`, so only the first comment stays flush on the head
    /// line and everything after (remaining comments and `tail`) drops one level and
    /// reads as part of the construct, not a sibling.
    ///
    /// `start`/`end` bound the comment gap; `tail` is the continued content (a type,
    /// a `: type` annotation, an argument list, …) the caller has already built.
    ///
    /// **Every** site where a line comment splits a construct's head from its tail
    /// routes here, so the enumeration is the rule's coverage, not a sample: the
    /// `:`→type annotation and its union arm (`build_type_annotation_doc`), the
    /// marker→`:` before-colon gap (`build_marker_colon_line_continuation`), the
    /// name→`=` initializer gap (`build_initializer_line_continuation`), the
    /// type-parameter pre-keyword gaps — name→`extends` and pre-`=`
    /// (`push_keyword_value_or_continuation`, `types/type_params.rs`) — the
    /// name→marker gap and the dotted-name gap (`build_dot_gap_doc`), the
    /// object-property and import-attribute key→`:` gaps, the pattern-family
    /// pre-separator gaps — a pattern property's key→`:` and every binding default's
    /// head→`=` ([`Self::route_pre_separator_gap`], `expressions/patterns.rs`) — the
    /// index-signature `]`→value-`:` gap (`build_index_signature_member_doc`), the two
    /// callee gaps that share [`Self::build_line_split_gap_doc`] — the callee→empty
    /// argument list gap (`build_empty_args_parens_doc`) and an optional call's callee→`?.` half
    /// (`calls::optional_callee_gap_doc`) — and the switch case label's head→`:` gap
    /// (`build_switch_case_doc_inner`, gated on line comments only), where the tail is
    /// the bare `:`. Adding a site means calling this, never re-deriving
    /// `indent(" " + hang_next + tail)`. See conformance_prettier.md
    /// §Uniform Forced-Continuation Indent.
    ///
    /// ⚠️ Every site here is a gap whose comment **trails a head**, which is what licenses
    /// pulling the first one up onto the head's line: own-line-ness is authoring signal for
    /// a *leading* position, not a trailing one (conformance_prettier.md §Comment Position
    /// Philosophy). A gap whose comment leads a VALUE — the keyword→value family, the
    /// switch label's own `case`→test gap included — takes
    /// [`Self::append_keyword_value_line_comments`] instead, which keeps the author's line.
    ///
    /// ⚠️ The leading separator is a **hardline** where the run opens with an honored
    /// format-ignore directive ([`Printer::leading_comment_is_honored_directive`]) — the
    /// leading half of the rule [`Printer::comment_hangs_next`] states for the separators
    /// *inside* the run. Beside the preceding token a directive shares its line, which the
    /// placement floor classifies as inert, so the freeze it earns would be lost on the next
    /// pass.
    pub(crate) fn build_continuation_indent(&self, start: u32, end: u32, tail: DocId) -> DocId {
        let d = self.d();
        let lead = if self.leading_comment_is_honored_directive(start, end) {
            d.hardline()
        } else {
            d.text(" ")
        };
        d.indent(d.concat(&[
            lead,
            self.build_trailing_comments_hang_next(start, end),
            tail,
        ]))
    }

    /// [`Self::build_continuation_indent`] for a caller sitting in a **value slot** —
    /// one whose separator, and the space after it, is already emitted by whoever handed
    /// it the slot.
    ///
    /// The keyword form owns that space because its callers emit their keyword bare; a
    /// value slot's caller does not, and calling the keyword form there renders the
    /// separator's space twice. Same hanging run, same indent, one seam
    /// ([`Self::build_trailing_comments_hang_next`]) — only the separator differs, which
    /// is why this is a sibling entry rather than a `bool` on the other.
    ///
    /// Its one site is a preserved grouping pair's `(`→inner run
    /// (`build_expression_doc`'s `ParenthesizedExpression` arm): the pair is erased, so
    /// the reparse reads that run through the enclosing value gap and this must land
    /// where that gap's own continuation does.
    pub(crate) fn build_value_slot_continuation_indent(
        &self,
        start: u32,
        end: u32,
        tail: DocId,
    ) -> DocId {
        let d = self.d();
        d.indent(d.concat(&[self.build_trailing_comments_hang_next(start, end), tail]))
    }

    /// A gap whose comment run precedes `tail`, routed on the **line**-comment question
    /// alone.
    ///
    /// A `//` runs to end of line, so `tail` cannot stay on that line: the run keeps the
    /// position the author gave it and `tail` drops to the uniform forced-continuation
    /// indent ([`Self::build_continuation_indent`]). Anything else — a block comment in any
    /// authored position, glued or own-line, single-line or multiline — forces nothing,
    /// reflows inline, and glues in front of `tail` behind prettier's own space. An empty
    /// gap is `tail` alone.
    ///
    /// The two sites are one gap under two spellings, which is why the shape is stated
    /// once rather than hand-rolled at each: the callee→`(` gap ahead of an EMPTY argument
    /// list (`build_empty_args_parens_doc`, where `tail` is the `()` an inline `//` would swallow) and
    /// the callee→`?.` half an optional call with arguments splits off
    /// (`calls::optional_callee_gap_doc`, where it is the `?.`). Both are the same
    /// position — the region between a callee and what opens its argument list — so a
    /// caller that answered them differently would make the `?.` change how the gap reads.
    ///
    /// ⚠️ Not [`Self::keyword_operand_gap`], whose gate is
    /// [`Printer::comments_force_own_line_between`] and therefore also hangs a multiline
    /// block the author broke after. These two gaps reflow that block inline instead, so
    /// the narrower gate is deliberate and the two routers are siblings rather than
    /// duplicates.
    pub(crate) fn build_line_split_gap_doc(&self, start: u32, end: u32, tail: DocId) -> DocId {
        if self.has_line_comments_between(start, end) {
            return self.build_continuation_indent(start, end, tail);
        }
        match self.build_comments_between_filtered_opt(
            start,
            end,
            CommentSpacing::Leading,
            CommentFilter::All,
        ) {
            Some(run) => self.d().concat(&[run, tail]),
            None => tail,
        }
    }

    /// Route a **pre-keyword** gap — the head→keyword half of a `<head> <keyword>
    /// <value>` construct: a type parameter's name→`extends` / pre-`=`
    /// (`types/type_params.rs`) and a mapped type's key→`in` / constraint→`as`
    /// (`types/composite.rs`). A **line** comment defers the whole `<keyword> <value>`
    /// tail to a continuation line (returns `Some(gap)` having pushed nothing — the
    /// caller wraps its tail in [`Self::build_continuation_indent`]), else the gap's
    /// inline block comments trail the head here (`<T /* c */ extends A>`,
    /// `[K /* c */ in T]`) and the keyword stays inline (`None`). Pushing the keyword
    /// inline after a `//` run would swallow it into the comment (content loss).
    ///
    /// Deliberately keyed on `has_line_comments_between` rather than
    /// [`Printer::comments_force_own_line_between`]: at both sites prettier glues a
    /// broke-after multiline block exactly as it glues a not-broke-after one, so —
    /// unlike the pre-separator `:`/`=` gaps, where prettier distinguishes the two —
    /// there is no authoring distinction to carry and tsv matches by gluing (the
    /// switch-case head→`:` precedent). See conformance_prettier.md §Uniform
    /// Forced-Continuation Indent.
    ///
    /// A caller whose keyword text carries its own separating space must split it in
    /// two — `"in"`/`" in"` — since the continuation arm's leading space is
    /// `build_continuation_indent`'s, and a second one after a hardline would be a
    /// stray leading space on the continuation line.
    pub(crate) fn route_pre_keyword_gap(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        keyword_pos: u32,
    ) -> Option<(u32, u32)> {
        if self.has_line_comments_between(gap_start, keyword_pos) {
            return Some((gap_start, keyword_pos));
        }
        if let Some(pre) = self.build_inline_comments_between_doc_opt(gap_start, keyword_pos) {
            parts.push(pre);
        }
        None
    }

    /// Route a **pre-separator** gap — the head→separator half of a `<head><sep>
    /// <value>` construct in a *flattening* container: a pattern property's key→`:`
    /// and every binding default's head→`=` (`expressions/patterns.rs`). A comment
    /// that hangs what follows defers the whole `<sep> <value>` tail to a
    /// continuation line (returns `Some(gap)` having pushed nothing — the caller
    /// wraps its tail in [`Self::build_continuation_indent`]), else the gap's
    /// comments trail the head here (`{ a /* c */ = 1 }`) and the separator stays
    /// inline (`None`). Pushing the separator inline after a `//` run would swallow
    /// it into the comment (content loss).
    ///
    /// The pre-separator twin of [`Self::route_pre_keyword_gap`], and the gate is
    /// the whole difference between them: [`Printer::comments_force_own_line_between`]
    /// here, so a multiline block the author **broke after** hangs too. A separator
    /// gap has a value on the far side whose own gap already honors that break, so
    /// collapsing it on the head side alone would answer one question two ways
    /// within a single construct; a keyword gap has no such counterpart and matches
    /// prettier by gluing. See conformance_prettier.md §Uniform Forced-Continuation
    /// Indent and the §Comment relocation "Pre-separator multiline block" entry.
    ///
    /// Callers hold a zero-comment fast gate over the whole `head <sep> value` span
    /// and skip this call on the (overwhelmingly common) comment-free path, so the
    /// two probes here never run there. A caller whose separator text carries its own
    /// separating space must split it in two — `"= "`/`" = "` — since the
    /// continuation arm's leading space is `build_continuation_indent`'s.
    pub(crate) fn route_pre_separator_gap(
        &self,
        parts: &mut DocBuf,
        gap_start: u32,
        separator_pos: u32,
    ) -> Option<(u32, u32)> {
        if self.comments_force_own_line_between(gap_start, separator_pos) {
            return Some((gap_start, separator_pos));
        }
        if let Some(pre) = self.build_inline_comments_between_doc_opt(gap_start, separator_pos) {
            parts.push(pre);
        }
        None
    }

    /// When a **line** comment — or a multiline block the author broke after —
    /// sits in the marker→`:` gap of a key/binding's type annotation, build the
    /// indented continuation: the first comment trails the marker on its line,
    /// then any remaining comments and the `: type` (`type_doc`, built by the
    /// caller) drop to a continuation line indented one level — the uniform
    /// forced-continuation indent (`build_continuation_indent`), so the
    /// annotation reads as part of its key/binding rather than a sibling. Returns
    /// `None` when no comment in the gap hangs what follows
    /// (`comments_force_own_line_between`), leaving the caller's block /
    /// no-comment handling in place — a glued single-line or glued multiline
    /// block stays inline.
    ///
    /// `marker_end` is the offset just past the key (and any `?`/`!`); `colon_pos`
    /// is the type annotation's `:` (its span start). Callers gate on
    /// `has_comments_to_emit_between` first, so the common (no-comment) path never reaches
    /// the hang probe here.
    ///
    /// Shared by the before-`:` sites: index/property signatures, class properties,
    /// variable bindings, and function parameters (`build_identifier_doc_inner`).
    /// Prettier keeps the continuation flush — and for property signatures / class
    /// properties relocates the comment to end-of-line — see conformance_prettier.md
    /// §Uniform Forced-Continuation Indent.
    pub(crate) fn build_marker_colon_line_continuation(
        &self,
        marker_end: u32,
        colon_pos: u32,
        type_doc: DocId,
    ) -> Option<DocId> {
        self.comments_force_own_line_between(marker_end, colon_pos)
            .then(|| self.build_continuation_indent(marker_end, colon_pos, type_doc))
    }

    /// When a **line** comment — or a multiline block the author broke after —
    /// sits in the name→`=` gap of an initializer, build the indented
    /// continuation: the comment trails the name on its line, then the `=`
    /// and value (`value_doc`, built by the caller — the bare value, no leading
    /// `= `) drop to a continuation line indented one level (the uniform
    /// forced-continuation indent, `build_continuation_indent`). Returns `None`
    /// when no comment in the gap hangs what follows
    /// (`comments_force_own_line_between`) — a glued single-line or glued
    /// multiline block stays inline — leaving the caller's block / no-comment /
    /// assignment-layout handling in place.
    ///
    /// `name_end` is the offset just before the `=` gap (past the binding name and
    /// any `?`/`!`/type annotation); `eq_pos` is the `=`. `build_value` lazily builds
    /// the bare value doc — only invoked on the (rare) hang path, so the
    /// common no-comment path never builds the value twice. Unlike the `:` twin
    /// (`build_marker_colon_line_continuation`, where prettier keeps the continuation
    /// flush), prettier here *relocates* the comment past the value to
    /// end-of-statement — which is **lossy when a second comment already trails the
    /// construct** (prettier merges them onto one line, the second `//` becoming text;
    /// tsv keeps both comments distinct). Shared by the initializer `=` sites: enum
    /// members, class properties, variable declarators, and a for-header's init
    /// declarators. See conformance_prettier_ts_comments.md §Comment relocation.
    pub(crate) fn build_initializer_line_continuation(
        &self,
        name_end: u32,
        eq_pos: u32,
        value: ContinuationValue<'_, '_>,
        build_value: impl FnOnce() -> DocId,
    ) -> Option<DocId> {
        self.build_operator_value_continuation(name_end, eq_pos, "=", value, build_value)
    }

    /// [`Self::build_initializer_line_continuation`] for a gap whose separator is not
    /// a bare `=`: the assignment expression, whose operator is whatever the author
    /// wrote (`=`, `+=`, `??=`, …). `op_text` is emitted verbatim ahead of the value,
    /// so it must be the operator alone — the separating space before it is
    /// [`Self::build_continuation_indent`]'s, and the one after it is added here.
    ///
    /// The two spellings share one seam because they are one rule: a comment the
    /// author put *before* a separator stays there, and the separator's tail takes
    /// the continuation line. Keeping the `=` sites on the named wrapper keeps their
    /// call sites reading as the initializer rule they are.
    ///
    /// ⚠️ **The operator→value gap keeps its OWN rule inside this tail.** `value` is there
    /// for one question: does what `build_value` prints begin on the operator's line, or on
    /// a line of its own? A comment the author gave its own line keeps it here exactly as it
    /// does when nothing precedes the operator
    /// ([`Self::build_eq_comment_break_rhs`]'s partition, the same gap's other emitter) —
    /// gluing it to the operator with an unconditional space made this arm the family's
    /// lone dissenter, and for an honored `prettier-ignore` it was worse than a layout
    /// difference: a directive trailing the operator is **inert** under tsv's own floor,
    /// so the next pass read no freeze, normalized the value, and the directive's whole
    /// effect vanished with no gate firing.
    ///
    /// ⚠️ **The tail is GROUPED, and that is what keeps the rest of the gap's rule intact.**
    /// The hanging run this indents ends in a hardline, so an ungrouped tail hands every
    /// soft `line` in it a broken group and renders it as a break — which turned the value
    /// gap's own pull-up off inside the continuation alone (`= /* c */⏎v` stayed broken here
    /// while the ordinary arm collapsed it, as prettier does). Grouping scopes the width
    /// decision to the tail, where the gap's `line`s are the only ones that answer to it.
    pub(crate) fn build_operator_value_continuation(
        &self,
        head_end: u32,
        op_pos: u32,
        op_text: &'static str,
        value: ContinuationValue<'_, '_>,
        build_value: impl FnOnce() -> DocId,
    ) -> Option<DocId> {
        let d = self.d();
        self.comments_force_own_line_between(head_end, op_pos)
            .then(|| {
                let separator = if self.operator_gap_run_is_own_line(op_pos, value) {
                    d.hardline()
                } else {
                    d.text(" ")
                };
                let tail = d.group(d.concat(&[d.text(op_text), separator, build_value()]));
                self.build_continuation_indent(head_end, op_pos, tail)
            })
    }

    /// Whether what the operator→value gap prints first sits on a line of its own rather
    /// than on the operator's — the separator question
    /// [`Self::build_operator_value_continuation`] asks, and the same partition test
    /// [`Self::build_eq_comment_break_rhs`] applies per comment.
    ///
    /// Only the FIRST comment is asked: everything after it takes the leading run's own
    /// separators (`build_value_gap_comments_opt`), which already read each comment's own
    /// neighbours.
    ///
    /// ⚠️ **An empty gap on the to-emit axis is not an empty gap.** An owned comment prints
    /// from inside the value's own doc, so it is skipped here — and for the one owned shape
    /// that prints on a line of its own, the **JSDoc cast** ([`Self::is_own_line_jsdoc_cast`],
    /// which supplies the hardline between its comment and its `(`), a space separator leaves
    /// that `(` stranded at the continuation's indent with the annotation welded to the
    /// operator. Every other owned comment is glued to its token and prints inline
    /// (`= /* c */ v`, the ordinary arm's answer too), so the emit-axis reading is right for
    /// it — which is why this asks the cast by name rather than widening to the on-page axis.
    fn operator_gap_run_is_own_line(&self, op_pos: u32, value: ContinuationValue<'_, '_>) -> bool {
        match comments_to_emit_in_range(self.comments, op_pos, value.gap_end()).next() {
            Some(first) => !self.is_same_line(op_pos, first.span.start),
            None => value.leads_with_own_line_jsdoc_cast(self),
        }
    }

    /// Build the value side of a pattern's `=`/`:` gap that holds comments: the
    /// gap's comments (hang-aware — `build_trailing_comments_hang_next`) followed by
    /// the value, the whole indented one level when a comment hangs the value
    /// (`comments_force_own_line_between`) so the continuation reads as this
    /// binding's value, not a sibling element — the value-side mirror of
    /// `build_continuation_indent`. A glued block collapses inline unindented
    /// (`= /* c */ v`), so a breaking value never gains a spurious extra level.
    ///
    /// The pattern-family counterpart of `build_eq_comment_break_rhs`: a
    /// destructuring pattern or parameter list has no multiline preservation, so an
    /// own-line authoring collapses instead of holding its line, and prettier's
    /// destinations differ per site (float past the value, hoist before the
    /// binding — see conformance_prettier_ts_comments.md §Comment relocation). Callers — the
    /// param/array default (`build_assignment_pattern_doc`), the shorthand default,
    /// and the rename/computed-key after-`:` gap
    /// (`build_object_pattern_property_doc`) — gate on `has_comments_to_emit_between`
    /// first, so the no-comment path never reaches the run.
    ///
    /// That gate is the caller's *whole* question: this seam owns the hang-vs-inline
    /// choice, so a caller must not re-ask [`Printer::comments_force_own_line_between`]
    /// and hand-roll the inline branch. The two branches are not interchangeable by
    /// accident — they coincide only because a non-hanging run is all glued blocks,
    /// where `build_trailing_comments_hang_next` and the `Trailing`-spaced inline run
    /// emit the same bytes.
    pub(crate) fn build_pattern_value_gap_doc(
        &self,
        start: u32,
        end: u32,
        value_doc: DocId,
    ) -> DocId {
        let d = self.d();
        let doc = d.concat(&[
            self.build_trailing_comments_hang_next(start, end),
            value_doc,
        ]);
        if self.comments_force_own_line_between(start, end) {
            d.indent(doc)
        } else {
            doc
        }
    }

    /// Build a binding/identifier `: type` annotation including any before-`:`
    /// comment. A **line** comment keeps the comment after the marker and indents the
    /// `: type` continuation one level (`build_marker_colon_line_continuation`); a
    /// **block** stays inline with a space before `:` (` /* c */ : T`); no comment is
    /// just `: T`. `marker_end` is the offset past the name and any `!`/`?`; `wrap`
    /// selects the width-aware annotation builder (generics / wrapping type args).
    ///
    /// Gates on `has_comments_to_emit_between` once, so the common no-comment path is a single
    /// binary search. Shared by every before-`:` site whose block form keeps the space
    /// before `:`: index-signature keys **and** their `]`→value-`:` gap, class properties,
    /// variable bindings, and function parameters/identifiers — the last of those including
    /// the function-type hug, which reaches the seam through
    /// [`Self::build_binding_type_annotation_doc_with`] because only its annotation *body*
    /// is unusual. (Property signatures handle the gap inline: their non-optional block form
    /// omits that space, which is the one real split in the family and the reason this is a
    /// shared seam rather than a shared flag.)
    ///
    /// An alone-on-line format-ignore directive in the gap freezes the whole `: type`
    /// annotation and keeps its own line (`build_frozen_annotation_head_doc`), asked
    /// before the annotation doc is built at all — the frozen route emits a verbatim
    /// slice, so the built doc would only be discarded.
    pub(crate) fn build_binding_type_annotation_doc(
        &self,
        marker_end: u32,
        type_ann: &internal::TSTypeAnnotation<'_>,
        wrap: bool,
    ) -> DocId {
        self.build_binding_type_annotation_doc_with(marker_end, type_ann, || {
            if wrap {
                self.build_type_annotation_doc_wrapping(type_ann)
            } else {
                self.build_type_annotation_doc(type_ann)
            }
        })
    }

    /// [`Self::build_binding_type_annotation_doc`] for a caller that builds the
    /// `: type` doc itself — the marker→`:` gap is the same question wherever a
    /// binding carries an annotation, but the annotation's own body is not.
    ///
    /// `build_annotation` returns the whole `: type` (leading `:` included), and is
    /// invoked lazily: the frozen-head route emits a verbatim slice, so on that path
    /// the doc would only be discarded.
    ///
    /// The one caller that needs this rather than the `wrap` flag is the
    /// function/constructor-type **hug** (`build_function_params_doc`), whose single
    /// object-typed parameter takes a type literal with no group of its own so the
    /// literal's softlines join the function type's group. Before this seam existed it
    /// respelled the whole head inline — name, `?`, `: ` — and so had no emitter for
    /// either gap behind it, silently dropping any comment there (`docs/comments.md`
    /// hazard 4); its sibling hug on the signature path never did, because it routes
    /// through `build_function_type_param_expression_doc`.
    pub(crate) fn build_binding_type_annotation_doc_with(
        &self,
        marker_end: u32,
        type_ann: &internal::TSTypeAnnotation<'_>,
        build_annotation: impl FnOnce() -> DocId,
    ) -> DocId {
        let d = self.d();
        let colon_pos = type_ann.span.start;
        if let Some(frozen) = self.build_frozen_annotation_head_doc(marker_end, type_ann) {
            return frozen;
        }
        let type_doc = build_annotation();
        if !self.has_comments_to_emit_between(marker_end, colon_pos) {
            return type_doc;
        }
        if let Some(doc) =
            self.build_marker_colon_line_continuation(marker_end, colon_pos, type_doc)
        {
            return doc;
        }
        d.concat(&[
            self.build_inline_comments_between_doc(marker_end, colon_pos),
            d.text(" "),
            type_doc,
        ])
    }

    /// Emit a rest element's tail — its optional `?` marker and its `: type`
    /// annotation — after the caller has pushed `...` and the binding.
    ///
    /// The `?` is carried on the `RestElement`, not on `argument` (see `RestElement`),
    /// so both gaps it creates — binding→`?` and `?`→`:` — sit between two SIBLING
    /// nodes: no other emitter can see either one, and without these landings a comment
    /// in them is silently dropped (`(...a /* c */?: T[])`, the marker-less
    /// `(...a /* c */: T[])`, and destructuring rest bindings `...[a] /* c */: T[]`
    /// alike). They are the same landings the plain identifier parameter takes
    /// ([`Self::push_modifier_marker_doc`] and
    /// [`Self::build_binding_type_annotation_doc`]).
    ///
    /// `wrap` selects the width-aware annotation builder, matching the sibling
    /// non-rest arm at each call site: `false` on the value side
    /// (`build_rest_element_doc`, mirroring `build_identifier_doc_inner`), `true` on
    /// the type side (`build_function_type_param_expression_doc`, whose `Identifier`
    /// arm breaks its annotation's generic arguments at print width).
    pub(crate) fn push_rest_element_tail_doc(
        &self,
        parts: &mut DocBuf,
        rest: &internal::RestElement<'_>,
        wrap: bool,
    ) {
        let binding_end = rest.argument.span().end;
        let after_modifier = if rest.optional {
            self.push_modifier_marker_doc(parts, binding_end, b'?')
        } else {
            binding_end
        };
        if let Some(ta) = &rest.type_annotation {
            parts.push(self.build_binding_type_annotation_doc(after_modifier, ta, wrap));
        }
    }

    /// A whole multi-word header: the keyword (see
    /// [`build_keyword_words_doc`](Self::build_keyword_words_doc)) plus the
    /// keyword→`continuation` gap. The shape every caller wants that has no other use
    /// for the keyword's end — bounding the word search by `continuation_start` also
    /// keeps a word from ever matching inside the continuation.
    pub(crate) fn build_keyword_header_doc(
        &self,
        words: &[&'static str],
        start: u32,
        continuation_start: u32,
        continuation: DocId,
    ) -> DocId {
        self.build_keyword_header_doc_with(words, start, continuation_start, |_| continuation)
    }

    /// [`Self::build_keyword_header_doc`] for a caller whose continuation depends on where
    /// the keyword ENDS — `build_continuation` is handed the offset just past its final
    /// word, the start of the keyword→continuation gap.
    ///
    /// That gap is a **freeze head**: an own-line format-ignore directive in it claims the
    /// whole continuation ([`Printer::value_head_frozen_span`]), which is a question about
    /// the continuation's own doc and so has to be answered before it is built. Without
    /// this the header's shape gets respelled at the call site just to hold `keyword_end`
    /// — which is how `export =` drifted from its twin `export default`.
    pub(crate) fn build_keyword_header_doc_with(
        &self,
        words: &[&'static str],
        start: u32,
        continuation_start: u32,
        build_continuation: impl FnOnce(u32) -> DocId,
    ) -> DocId {
        let d = self.d();
        let (keyword_doc, keyword_end) =
            self.build_keyword_words_doc(words, start, continuation_start);
        d.concat(&[
            keyword_doc,
            self.build_keyword_to_name_continuation(
                keyword_end,
                continuation_start,
                build_continuation(keyword_end),
            ),
        ])
    }

    /// A dotted pair of names, with **both** gaps around the `.` emitted: a meta property
    /// (`new` `.` `target`, `import` `.` `meta`) or a qualified name (`ns` `.` `Type`).
    ///
    /// Concatenating `left` + `"."` + `right` scans neither gap and drops whatever an
    /// author wrote in one. That is the punctuator-joined member of the multi-word-keyword
    /// class (see [`build_keyword_words_doc`](Self::build_keyword_words_doc)) — and the
    /// case that shows why the class's usual detector, a `d.text` literal with an
    /// *interior* space, is only a proxy: the joining literal is `"."`, which has no space
    /// to find. Both shapes route here so neither can regrow the hole independently.
    ///
    /// Each side stays where it was authored, which is what prettier prints for a **block**
    /// comment: it hugs the `.` and keeps its space on the identifier's side. A **line**
    /// comment ends its line, so the tail continues one level down — that half is a
    /// divergence (prettier relocates it out of the construct, past the `;`).
    ///
    /// `gap_start` is `left`'s source end; `gap_end` is `right`'s source start.
    pub(crate) fn build_dotted_pair_doc(
        &self,
        left: DocId,
        right: DocId,
        gap_start: u32,
        gap_end: u32,
    ) -> DocId {
        let d = self.d();
        // Both gaps empty — every ordinary occurrence, and the only one that is hot.
        if !self.has_comments_to_emit_between(gap_start, gap_end) {
            return d.concat(&[left, d.text("."), right]);
        }
        let Some(dot) = self.find_char_outside_comments(gap_start, gap_end, b'.') else {
            debug_assert!(
                false,
                "a dotted pair always spells a `.` between its two names"
            );
            return d.concat(&[left, d.text("."), right]);
        };
        // `.`→right, then left→`.` wrapping it: the tail is built first so a line comment
        // in the left gap takes the whole `.right` down with it.
        let tail = d.concat(&[
            d.text("."),
            self.build_dot_gap_doc(dot + 1, gap_end, right, CommentSpacing::Trailing),
        ]);
        d.concat(&[
            left,
            self.build_dot_gap_doc(gap_start, dot, tail, CommentSpacing::Leading),
        ])
    }

    /// One of the two gaps around a dotted pair's `.`: the comments authored in
    /// `[start, end)`, then `tail`.
    ///
    /// Both sides obey the same rule, so both call this — a *line* comment ends its line
    /// and `tail` continues one level down; a block comment stays inline ahead of it. Only
    /// the spacing differs, since a gap's comment sits after the `.` on one side and
    /// before it on the other.
    fn build_dot_gap_doc(
        &self,
        start: u32,
        end: u32,
        tail: DocId,
        spacing: CommentSpacing,
    ) -> DocId {
        // The caller established that *some* comment lies between the two names, but not
        // which side of the `.` — so each gap still gates, and an empty one adds nothing.
        if !self.has_comments_to_emit_between(start, end) {
            return tail;
        }
        if self.has_line_comments_between(start, end) {
            return self.build_continuation_indent(start, end, tail);
        }
        self.d()
            .concat(&[self.build_comments_between(start, end, spacing), tail])
    }

    /// The keyword's words joined by one space, with its end *measured* rather than
    /// located — the shape used where the words cannot be located: the source does not
    /// hold them, or there is no window of source to hold them in.
    ///
    /// It assumes exactly one space per interior gap, so it scans no gap and can emit
    /// no interior comment. Every caller must therefore have established that there is
    /// none to emit — which an empty window proves, and which the located path below
    /// makes unnecessary.
    fn measured_keyword_doc(&self, words: &[&'static str], start: u32) -> (DocId, u32) {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        for (i, w) in words.iter().enumerate() {
            if i > 0 {
                parts.push(d.text(" "));
            }
            parts.push(d.text(w));
        }
        let width: u32 = words.iter().map(|w| w.len() as u32).sum();
        let measured = start + width + words.len() as u32 - 1;
        (d.concat(&parts), measured)
    }

    /// Build a **multi-word keyword** (`export default`, `await using`, `declare
    /// const`, `export as namespace`), preserving a comment authored in one of its
    /// interior gaps.
    ///
    /// Returns the keyword's doc (no trailing space) and the source offset just past
    /// its final word — the caller's own keyword→value gap starts there.
    ///
    /// A keyword spanning two or more words has a gap *between* them that is a real
    /// source position an author can write a comment in. Deriving the keyword's extent
    /// by measuring its text (`span.start + "export default".len()`) never locates that
    /// gap, so nothing scans it and the comment is silently dropped. Locating each word
    /// instead makes every interior gap emittable, through the same emitter the
    /// keyword→name gap uses: a block comment stays inline, a line comment indents the
    /// continuation.
    ///
    /// `words` must occur in order within `source[start..search_end]`; each is matched
    /// whole-word and comment-aware, so a word appearing inside an interior comment
    /// (`export /* default */ default 1`) never matches.
    ///
    /// A word may be a punctuator (`=`). Note what makes that safe: the whole-word test
    /// rejects a match flanked by *identifier* bytes, and a punctuator has none — so it
    /// does **not** rule out matching the `=` inside `=>` or `==`. Only `start` and
    /// `search_end` do: every caller bounds the search at the token before the
    /// continuation, and no operator can occur in that gap. A caller that widens those
    /// bounds must re-check that itself.
    pub(crate) fn build_keyword_words_doc(
        &self,
        words: &[&'static str],
        start: u32,
        search_end: u32,
    ) -> (DocId, u32) {
        let d = self.d();
        debug_assert!(!words.is_empty(), "a keyword has at least one word");

        // Does any source lie between `start` and what follows the keyword? A real
        // parsed node always says yes — the keyword's own bytes are inside the window,
        // so it cannot be empty. An empty one therefore means the span is not source-
        // backed at all, which is `tsv_svelte_compile`'s generated AST: its synthetic
        // nodes carry spans whose only job is steering these very windows, placed to
        // come out empty/inverted precisely so a minted node claims no comment out of
        // the host document (see that crate's `build.rs`).
        //
        // That is the condition under which everything below is *meaningful*, not a
        // special case for one caller. An empty window holds no comment, so the drop
        // this function exists to prevent is impossible in it; the words cannot be
        // located because there is no source to find them in; and both answers below
        // reduce to the same measured text either way.
        let has_window = search_end > start;

        // A one-word keyword has no interior gap, so there is nothing to locate: it
        // begins at `start` and its end is arithmetic. This is the hot path — the
        // single-word kinds (`const`/`let`/`var`/`using`) run through here for **every**
        // declaration in every file, and the gap printer they feed is already the
        // hottest of them (see the internal perf notes on `build_keyword_to_name_continuation`).
        // Only a genuinely multi-word keyword pays for a scan.
        if let [word] = words {
            // That shortcut rests on an invariant nothing else enforces: a single-word
            // caller's `start` IS the keyword. Locating it would cost the hot path a
            // scan to prove what every caller already knows, so assert it in debug
            // instead — a caller passing a wider span (one that leads with `export `,
            // say) would otherwise silently mis-place `keyword_end` and drop the gap's
            // comment, which is the very bug this function exists to prevent. With no
            // window there is no such comment, and no source to read the word from.
            debug_assert!(
                !has_window
                    || self
                        .source
                        .as_bytes()
                        .get(start as usize..)
                        .is_some_and(|rest| rest.starts_with(word.as_bytes())),
                "single-word keyword `{word}` must begin at `start` ({start})"
            );
            return (d.text(word), start + word.len() as u32);
        }

        if !has_window {
            return self.measured_keyword_doc(words, start);
        }

        // Left-to-right and FLAT: every gap emits its comments where the author wrote
        // them, but none of them indents on its own — the whole tail is wrapped once,
        // below. Indenting per gap would compound, and the staircase it builds is not
        // just deep, it is wrong: the caller emits the keyword→value gap at the header's
        // own level, so a two-broken-gap keyword would leave its last word sitting a
        // level *below* the value that follows it.
        //
        // Locating and emitting ride one pass: a word's gap runs from the previous
        // word's end (`cursor`) to this word's start, both of which this loop already
        // holds — so nothing needs to remember where the earlier words landed.
        let mut tail: DocBuf = DocBuf::new();
        let mut any_line = false;
        let mut cursor = start;
        let mut in_gap = false;
        for word in words {
            let Some(pos) = self.find_keyword_in_range(cursor, search_end, word) else {
                // A non-empty window that does not hold the shape the caller named:
                // a `search_end` past the words, or a span that isn't the keyword's.
                // Assert it in debug: this arm's measured end is the very arithmetic
                // this function exists to replace (it assumes one space per gap), so a
                // caller that lands here silently drops the comment it came for. Prod
                // still degrades gracefully rather than panicking — a formatter must
                // not crash on input it parsed.
                debug_assert!(
                    false,
                    "keyword word `{word}` not found in source[{cursor}..{search_end}] \
                     — caller passed a bad search_end"
                );
                return self.measured_keyword_doc(words, start);
            };
            // The first word is emitted by the caller below, outside the indent — it
            // leads the header, so no gap precedes it.
            if in_gap {
                let (gap_doc, has_line) = self.build_keyword_gap_doc(cursor, pos);
                any_line |= has_line;
                tail.push(gap_doc);
                tail.push(d.text(word));
            }
            in_gap = true;
            cursor = pos + word.len() as u32;
        }
        let keyword_end = cursor;
        let tail_doc = d.concat(&tail);
        // One level for the whole header — the same thing the single-gap rule says: a
        // broken gap reads as one statement continuation, never as N nested ones. With
        // no line comment there is no break to indent, so the wrapper is skipped and a
        // comment-free keyword stays byte-identical to `words.join(" ")`.
        let doc = d.concat(&[
            d.text(words[0]),
            if any_line {
                d.indent(tail_doc)
            } else {
                tail_doc
            },
        ]);
        (doc, keyword_end)
    }

    /// Build a **declaration header's** keyword run — an optional `declare` modifier
    /// ahead of `head` (`["namespace"]`, `["interface"]`, `["const", "enum"]`, …) —
    /// returning it with the offset just past its last word, where the caller's
    /// keyword→name gap opens.
    ///
    /// The one shape every ambient-capable header shares, over
    /// [`Self::build_keyword_words_doc`]: `start` is the declaration's span start (which
    /// `declare` begins when present) and `name_start` bounds the word search.
    ///
    /// ⚠️ **Every declaration header must build its keyword through here rather than
    /// pushing the words as text.** Measured text locates no interior gap, so the
    /// header's keyword→name emitter is left scanning from the declaration's start and
    /// prints whatever sits between the words *after* the last one — collapsing
    /// `declare /* a */ namespace /* b */ N` into `declare namespace /* a */ /* b */ N`,
    /// where "annotates the ambient-ness" and "annotates the name" become one position.
    /// That is the relocation `docs/conformance_prettier_ts_comments.md` §Comments inside
    /// a multi-word keyword catalogs prettier for and tsv preserves against; it was tsv's
    /// own behavior at `namespace`, `module`, `interface`, `enum`, `type` and
    /// `const enum` until each was routed here, and no fixture saw it.
    pub(crate) fn build_declaration_head_doc(
        &self,
        declare: bool,
        head: &[&'static str],
        start: u32,
        name_start: u32,
    ) -> (DocId, u32) {
        if !declare {
            // The head is already the slice the emitter takes; a buffer exists only to
            // put `declare` ahead of it.
            return self.build_keyword_words_doc(head, start, name_start);
        }
        let mut words: SmallVec<[&'static str; 3]> = smallvec!["declare"];
        words.extend_from_slice(head);
        self.build_keyword_words_doc(&words, start, name_start)
    }

    /// Build a declaration header's keyword→name gap comment followed by the rest
    /// of the declaration (`continuation`), indenting that continuation one level
    /// when a *line* comment forces the break.
    ///
    /// `keyword_end` bounds the start of the comment scan and `name_start` its end
    /// (the start of the name, or first declarator). It is **just past the final
    /// keyword word** before the name (`function`/`*`, `class`, `const`, `enum`, …) —
    /// never the declaration's start. ⚠️ Widening it to swallow an *earlier*
    /// inter-keyword gap (`declare /* c */ enum`, `const /* c */ enum`) does not merely
    /// scan more: it prints that comment **after** the keyword, collapsing two authored
    /// positions into one. That is the relocation
    /// `docs/conformance_prettier_ts_comments.md` §Comments inside a multi-word
    /// keyword catalogs prettier for, and it was tsv's own behavior at five heads until
    /// each was routed through [`Printer::build_keyword_words_doc`], which locates the
    /// words and emits their gaps in place. The preceding keyword token must be emitted
    /// **without** a trailing space; the leading space is supplied here.
    ///
    /// - **Line comment**: ends its line with a hardline, so the whole continuation
    ///   is wrapped in `indent` to read as a statement continuation rather than a
    ///   second statement (the uniform declaration-header rule): `function // c⏎\tf()`.
    /// - **Block comment**: trails inline (` /* c */ ` + continuation), no break.
    /// - **No comment**: just a leading space before the continuation.
    ///
    /// Block and no-comment output is byte-identical to the prior
    /// `build_keyword_to_name_comments(...)` form (which already supplies the leading
    /// space). Shared by the `function`/`class`/`enum`/`declare function`/variable
    /// declaration printers and the `export` / `export default`→declaration printers
    /// in `statements/modules.rs`.
    ///
    /// Declaration-side twin of `gap_comment_indented_continuation` (modules.rs):
    /// both supply a leading space and indent the continuation on a line comment, but
    /// they use different comment emitters (`build_name_to_type_params_comments` /
    /// `build_inline_comments_between_doc_opt` here vs `build_rhs_comments_opt`
    /// there), so a multi-line block comment stays inline here but breaks there. Keep
    /// the two separate — don't merge.
    pub(crate) fn build_keyword_to_name_continuation(
        &self,
        keyword_end: u32,
        name_start: u32,
        continuation: DocId,
    ) -> DocId {
        let d = self.d();
        let (gap_doc, has_line) = self.build_keyword_gap_doc(keyword_end, name_start);
        let body = d.concat(&[gap_doc, continuation]);
        if has_line { d.indent(body) } else { body }
    }

    /// Whether a header gap ENDS ON ITS OWN LINE — the one predicate behind
    /// [`Self::build_keyword_gap_doc`]'s broken-and-indented layout, any caller that must
    /// stop supplying its own continuation indent because this gap's `indent` already covers
    /// it (the multi-declarator variable list), and any caller that must choose a *breaking*
    /// header layout because this gap's comment needs a line of its own (the for-in/for-of
    /// header's `(`→binding region).
    ///
    /// A line comment breaks the gap, and so does an honored format-ignore directive in
    /// either spelling: [`Self::build_header_comment_run`] ends it on its own line, because
    /// flush against the keyword the directive would be inert and the freeze it earns would
    /// be lost on the second pass.
    #[inline]
    pub(crate) fn keyword_gap_breaks(&self, start: u32, end: u32) -> bool {
        self.has_line_comments_between(start, end) || self.member_gap_frozen(start, end)
    }

    /// One header gap — the comments authored in it plus the separator that follows —
    /// with **no** `indent` applied. Also reports whether a *line* comment ended the
    /// line, which is the caller's cue that a break happened.
    ///
    /// Split out from [`build_keyword_to_name_continuation`](Self::build_keyword_to_name_continuation)
    /// so a caller with *several* gaps can emit each one and then decide **once** what
    /// to indent. Indenting per gap compounds: two broken gaps would put the keyword's
    /// last word two levels deep, below the value that follows it at one.
    ///
    /// Two callers: [`build_keyword_words_doc`](Self::build_keyword_words_doc) for a
    /// keyword's interior gaps, and the import-equals header — the one multi-gap header
    /// whose words aren't contiguous (its name sits between `import` and `=`), so it
    /// drives this directly instead.
    #[inline]
    pub(crate) fn build_keyword_gap_doc(&self, start: u32, end: u32) -> (DocId, bool) {
        let d = self.d();
        // One search settles the gap. With no comment there is nothing to emit but the
        // separator — no empty child, and neither of the per-shape searches below runs.
        // Every declaration in every file passes through here, so this is the hottest of
        // the gap printers.
        if !self.has_comments_to_emit_between(start, end) {
            return (d.text(" "), false);
        }
        let has_line = self.keyword_gap_breaks(start, end);
        let comment_doc = if has_line {
            self.build_name_to_type_params_comments(start, end, CommentSpacing::Leading)
        } else if let Some(c) = self.build_inline_comments_between_doc_opt(start, end) {
            c
        } else {
            d.empty()
        };
        // After a line comment the hardline provides separation; otherwise a space.
        let space_after = if has_line { d.empty() } else { d.text(" ") };
        (d.concat(&[comment_doc, space_after]), has_line)
    }

    /// Build a Doc for comments between a keyword and the following name/token.
    ///
    /// Handles line comments safely: emits hardline after line comments to prevent
    /// absorbing following code. Block comments get a leading space + trailing space.
    /// Returns `" // c" + hardline` for line comments, or `" /* c */ "` for block.
    ///
    /// Used for: `function // c\nname`, `class // c\nname`, `export // c\ndecl`,
    /// `enum // c\nname`, etc. — any keyword-to-name/code gap.
    pub(crate) fn build_keyword_to_name_comments(&self, start: u32, end: u32) -> DocId {
        let d = self.d();
        // A comment-free gap is just the leading space — emitting it as a bare text
        // saves both the empty child and the concat node that would wrap it.
        if !self.has_comments_to_emit_between(start, end) {
            return d.text(" ");
        }
        if self.has_line_comments_between(start, end) {
            self.build_name_to_type_params_comments(start, end, CommentSpacing::Trailing)
        } else {
            let comments = self.build_inline_comments_between_doc_trailing_space(start, end);
            d.concat(&[d.text(" "), comments])
        }
    }

    /// Build a Doc for inline comments between a name/key and type params or parens.
    ///
    /// Like `build_comments_between` but handles line comments safely:
    /// block comments use the given `block_spacing`, line comments get a leading
    /// space and a hardline after (to prevent absorbing following code). The leading
    /// space is skipped for any comment that follows a line comment's hardline — it
    /// starts a fresh line, so a leading space would render as a stray `\t // c`.
    ///
    /// Used for: declaration name → type params, method key → type params/parens,
    /// getter/setter key → parens.
    ///
    /// Example: `class A // c\n<T> {}` stays multi-line instead of collapsing to
    /// `class A// c <T> {}` where `<T> {}` would be lost in the comment.
    pub(crate) fn build_name_to_type_params_comments(
        &self,
        start: u32,
        end: u32,
        block_spacing: CommentSpacing,
    ) -> DocId {
        self.build_header_comment_run(start, end, block_spacing, false)
    }

    /// [`Self::build_name_to_type_params_comments`] with the run's leading separator made
    /// explicit: `preceded_by_space` is `true` for a caller whose preceding token already ends
    /// in one (the module headers' `import `/`export `/`type `), so the first comment doesn't
    /// get a second. It is deliberately NOT the same question as "on a fresh line" — the run
    /// still starts on the preceding token's line, which is what the directive rule below
    /// keys on.
    pub(crate) fn build_header_comment_run(
        &self,
        start: u32,
        end: u32,
        block_spacing: CommentSpacing,
        preceded_by_space: bool,
    ) -> DocId {
        let d = self.d();
        let mut parts = DocBuf::new();
        // After a line comment's hardline the next comment starts a fresh (indented)
        // line, so it must not get a leading space — otherwise a 2nd+ own-line comment
        // renders as `\t // c` (stray leading space).
        let mut at_line_start = false;
        // The caller's token already supplied the run's first separator, if it had one.
        let mut owed_space = !preceded_by_space;
        let mut comments = comments_to_emit_in_range(self.comments, start, end).peekable();
        while let Some(comment) = comments.next() {
            let space_before = !at_line_start && owed_space;
            owed_space = true;
            // An HONORED format-ignore directive keeps the line the author gave it, in both
            // spellings: pulled up flush against the keyword — or left with the following
            // token on its line — it would share that line, which the placement floor
            // classifies as inert, and the freeze it earns would be lost on the second pass.
            // Every other comment in the run keeps the flush-first layout.
            //
            // Deliberately NOT conditioned on this gap being a freeze position: the rule is
            // about the directive, not the target. A gap that doesn't freeze today (a
            // `function`/`class` head) can only start honoring one if the placement survives
            // to be read, so the emitter never relocates a directive anywhere — one rule
            // instead of a flag threaded through nine header call sites, and no position can
            // be silently disqualified before its cluster reaches it. The layout cost is
            // fixtured (`keyword_gap_prettier_ignore_own_line_prettier_divergence`).
            if self.is_honored_directive(comment) {
                if !at_line_start {
                    parts.push(d.hardline());
                }
                parts.push(self.build_comment_doc(comment));
                parts.push(d.hardline());
                at_line_start = true;
                continue;
            }
            if comment.is_block {
                // Block comment: use caller-specified spacing
                self.push_block_comment_spaced(&mut parts, comment, block_spacing, space_before);
                at_line_start = false;
            } else {
                // Line comment: leading space (unless already at line start) +
                // hardline after — `class A // c\n<T> {}`. The break is forced, so an
                // author blank line after the comment survives with it — the same
                // answer the forced-continuation emitter
                // (`build_trailing_comments_hang_next`) gives. Anchored on the
                // physically next comment (`blank_scan_end`) so an owned comment
                // between two emitted ones can't desync the measure.
                if space_before {
                    parts.push(d.text(" "));
                }
                parts.push(self.build_comment_doc(comment));
                let emit_next = comments.peek().map_or(end, |n| n.span.start);
                let next = self.blank_scan_end_after(comment, emit_next);
                self.push_blank_preserving_hardline(&mut parts, comment.span.end, next);
                at_line_start = true;
            }
        }
        d.concat(&parts)
    }

    /// Like `build_name_to_type_params_comments`, but returns `None` when there
    /// are no comments in the range (avoids the separate `has_comments_to_emit_between` check).
    pub(crate) fn build_name_to_type_params_comments_opt(
        &self,
        start: u32,
        end: u32,
        block_spacing: CommentSpacing,
    ) -> Option<DocId> {
        if self.has_comments_to_emit_between(start, end) {
            Some(self.build_name_to_type_params_comments(start, end, block_spacing))
        } else {
            None
        }
    }

    /// Append the name→type-params/parens gap comments to `parts`, appending nothing
    /// when the gap is comment-free.
    ///
    /// That is the overwhelmingly common case on a gap every function, method, class and
    /// interface member emits, and pushing the builder's `empty()` unconditionally would
    /// leave a child slot for the renderer and every `fits` pass to walk.
    pub(crate) fn push_name_to_type_params_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        end: u32,
        block_spacing: CommentSpacing,
    ) {
        if let Some(comments) =
            self.build_name_to_type_params_comments_opt(start, end, block_spacing)
        {
            parts.push(comments);
        }
    }

    /// Build the name→signature-head gap's comment run — the type-params-aware form of
    /// [`Self::build_name_to_type_params_comments_opt`], returning `None` on a comment-free gap.
    ///
    /// See [`Self::push_signature_head_comments`] for why the bound and the spacing are
    /// one argument rather than two.
    pub(crate) fn build_signature_head_comments_opt(
        &self,
        start: u32,
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
        no_type_params_end: u32,
    ) -> Option<DocId> {
        self.build_name_to_type_params_comments_opt(
            start,
            type_parameters.map_or(no_type_params_end, |tp| tp.span.start),
            CommentSpacing::for_type_params(type_parameters.is_some()),
        )
    }

    /// Push the name→signature-head gap's comment run: the comments between a
    /// function/method/type name and whatever opens its signature.
    ///
    /// ⚠️ **The end bound and the run's spacing are ONE decision**, which is why this takes
    /// the type parameters rather than the two derived values. A gap that stops at `<`
    /// is followed by the type params and so spaces *after* the run (`f/* c */ <T>()`);
    /// one that runs to the parens is followed by `(` and spaces *before* it
    /// (`f /* c */()`). Deriving them at the call site let the object-literal accessor
    /// get both halves wrong — it passed `params_start` unconditionally, so its window
    /// swallowed the whole `<T>` region the signature doc also prints and every comment
    /// in there was emitted twice.
    ///
    /// `no_type_params_end` is where the gap ends when there are no type parameters —
    /// the `(` for anything with a parameter list, the name's own end for a construct
    /// with no parens at all (a type alias), which closes the gap to nothing.
    pub(crate) fn push_signature_head_comments(
        &self,
        parts: &mut DocBuf,
        start: u32,
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
        no_type_params_end: u32,
    ) {
        if let Some(comments) =
            self.build_signature_head_comments_opt(start, type_parameters, no_type_params_end)
        {
            parts.push(comments);
        }
    }

    /// Split heritage-preceding comments into inline and indented parts.
    ///
    /// For comments between a declaration name/type-params and a heritage keyword
    /// (extends/implements), comments before the first line comment stay inline at the
    /// declaration level, while comments after a line comment go into the heritage indent.
    ///
    /// Returns `(inline_parts, indent_parts)`:
    /// - `inline_parts`: `[" ", comment, " ", comment, ...]` at declaration level
    /// - `indent_parts`: `[hardline, comment, hardline, comment, ...]` for heritage indent
    pub(crate) fn build_heritage_leading_comment_parts(
        &self,
        start: u32,
        end: u32,
    ) -> (DocBuf, DocBuf) {
        let d = self.d();
        let mut inline_parts = DocBuf::new();
        let mut indent_parts = DocBuf::new();
        let mut saw_line_comment = false;
        for comment in comments_to_emit_in_range(self.comments, start, end) {
            if saw_line_comment {
                indent_parts.push(d.hardline());
                indent_parts.push(self.build_comment_doc(comment));
            } else {
                inline_parts.push(d.text(" "));
                inline_parts.push(self.build_comment_doc(comment));
                if !comment.is_block {
                    saw_line_comment = true;
                }
            }
        }
        (inline_parts, indent_parts)
    }

    /// Join heritage item docs, emitting per gap only what that gap's item doc did
    /// not already write (see [`HeritageGap`]) — so the separator and the item doc
    /// can't disagree about who owns the comma or the break. `break_doc` is the
    /// separator's break: a `hardline` where a line comment forces one, a `line`
    /// for the width-based clause. With every gap `Open` this is exactly
    /// `d.join(item_docs, "," + break_doc)`.
    fn join_heritage_items(
        &self,
        item_docs: &[DocId],
        gaps: &[HeritageGap],
        break_doc: DocId,
    ) -> DocId {
        let d = self.d();
        let comma_break = d.concat(&[d.text(","), break_doc]);
        let mut joined: DocBuf = smallvec![item_docs[0]];
        for (idx, &item_doc) in item_docs.iter().enumerate().skip(1) {
            match gaps[idx - 1] {
                // The gap emitter already wrote the comma, the comments, and the break.
                HeritageGap::Baked => {}
                HeritageGap::CommaBaked => joined.push(break_doc),
                HeritageGap::Open => joined.push(comma_break),
            }
            joined.push(item_doc);
        }
        d.concat(&joined)
    }

    /// Build a heritage clause doc: `keyword` + indented, comma-separated heritage items.
    ///
    /// See [`HeritageGap`] for how each inter-item gap splits between the preceding
    /// item's doc and the join separator.
    ///
    /// Handles line comments between items (SAFETY): when a line comment appears after
    /// a heritage item, the comma is placed before the comment to prevent the comment
    /// from absorbing subsequent items. Block comments keep the comma after.
    ///
    /// Used by both class `implements` and interface `extends` clauses.
    pub(crate) fn build_heritage_clause_doc(
        &self,
        keyword: HeritageKeyword,
        items: &[internal::TSInterfaceHeritage<'_>],
        group_mode: bool,
        keyword_start: Option<u32>,
    ) -> DocId {
        let d = self.d();

        // Track which items have trailing line comments (between this item and the next).
        // Line comments consume the rest of the line, so the comma must go before them.
        let has_trailing_line_comment: SmallVec<[bool; 8]> = items
            .windows(2)
            .map(|pair| {
                self.has_line_comments_between(heritage_item_end(&pair[0]), pair[1].span.start)
            })
            .collect();
        let has_any_item_line_comments = has_trailing_line_comment.iter().any(|&v| v);

        // How each gap's pieces split between the preceding item's doc and the join
        // separator. Inline (non-group) heritage keeps every after-comma block leading
        // the next item, so nothing is baked there (the `", "` join owns the comma).
        // Mirrors the declarator/for-init stranded rule; prettier relocates the block
        // before the comma.
        let gaps: SmallVec<[HeritageGap; 8]> = has_trailing_line_comment
            .iter()
            .enumerate()
            .map(|(i, &has_line)| {
                if has_line {
                    HeritageGap::Baked
                } else if group_mode && {
                    let next_start = items[i + 1].span.start;
                    let comma_pos = self.comma_between(heritage_item_end(&items[i]), next_start);
                    self.comments_on_page_between(comma_pos, next_start)
                        .any(|c| self.is_stranded_after_comma_block(c, comma_pos, next_start))
                } {
                    HeritageGap::CommaBaked
                } else {
                    HeritageGap::Open
                }
            })
            .collect();

        let item_docs: DocBuf = items
            .iter()
            .enumerate()
            .map(|(i, heritage)| {
                let mut h_parts: DocBuf = DocBuf::new();

                // After-comma block(s) from the previous (block-only) gap that **hug**
                // this item lead it, preserving the author's side of the comma:
                // prettier keeps a block hugging the next item after the comma (leading
                // it), and so does tsv.
                // A **stranded** after-comma block was instead baked onto the previous
                // item (trailing its comma) in group mode, so skip it here. When the
                // previous gap has a line comment, its after-comma comments were baked
                // into that item's doc, so skip the lead there entirely.
                if i > 0 && gaps[i - 1] != HeritageGap::Baked {
                    let prev_end = heritage_item_end(&items[i - 1]);
                    let comma_pos = self.comma_between(prev_end, heritage.span.start);
                    let leading: CommentVec<'_> =
                        comments_to_emit_in_range(self.comments, comma_pos, heritage.span.start)
                            .filter(|c| {
                                !(group_mode
                                    && self.is_stranded_after_comma_block(
                                        c,
                                        comma_pos,
                                        heritage.span.start,
                                    ))
                            })
                            .collect();
                    self.push_leading_comment_run(
                        &mut h_parts,
                        leading.iter().copied(),
                        heritage.span.start,
                        LeadingGlue::Adjacent,
                        None,
                    );
                }

                h_parts.push(self.build_entity_name_doc(&heritage.expression));
                if let Some(type_args) = &heritage.type_arguments {
                    // Preserve comments: `implements Foo/* c */ <T>`
                    let gap_start = heritage.expression.span().end;
                    let gap_end = type_args.span.start;
                    if let Some(doc) = self.build_name_to_type_params_comments_opt(
                        gap_start,
                        gap_end,
                        CommentSpacing::Trailing,
                    ) {
                        h_parts.push(doc);
                    }
                    h_parts.push(self.build_type_arguments_doc(type_args));
                }
                if let Some(next) = items.get(i + 1) {
                    let item_end = heritage_item_end(heritage);

                    if gaps[i] == HeritageGap::Baked {
                        // Line comment(s) in the gap: before-comma blocks trail this
                        // item, then the comma, then the first line comment trails it
                        // (on the comma's line) or drops below — the same rule as the
                        // declarator/for-init gaps, so route through the shared helper.
                        // e.g. `I /* c1 */,\n// c2\nJ` or `I, // c1\n// c2\nJ`. The whole
                        // gap is baked into this item's doc, break included (the join adds
                        // nothing — `HeritageGap::Baked`); the run sits inside the clause's
                        // `d.indent()`, so continuation indent is empty.
                        let comma_pos = self.comma_between(item_end, next.span.start);
                        self.push_inter_item_line_comment_gap(
                            &mut h_parts,
                            item_end,
                            comma_pos,
                            next.span.start,
                            None,
                        );
                    } else {
                        // Before-comma block(s) trail this item; a **hugging** after-comma
                        // block leads the NEXT item (its leading branch above). A
                        // **stranded** after-comma block stays on the comma's line: when
                        // this gap's comma is baked (`HeritageGap::CommaBaked`, group mode
                        // only) the comma is emitted here with the stranded block trailing
                        // it, and the join uses a bare break. Otherwise the comma comes
                        // from the join separator. Preserves the author's side of the comma.
                        let comma_pos = self.comma_between(item_end, next.span.start);
                        self.push_before_comma_blocks(&mut h_parts, item_end, comma_pos);
                        if gaps[i] == HeritageGap::CommaBaked {
                            h_parts.push(d.text(","));
                            self.push_stranded_after_comma_blocks(
                                &mut h_parts,
                                comma_pos,
                                next.span.start,
                            );
                        }
                    }
                }
                d.concat(&h_parts)
            })
            .collect();

        // Optional comments between keyword and first item: `extends /* c */ Item`.
        // Kept as an `Option` so the comment-free heritage clause — every plain
        // `class X extends Y` / `interface I extends J` — pushes no empty child.
        let kw_comments = keyword_start.and_then(|kw_start| {
            let kw_end = kw_start + keyword.as_str().len() as u32;
            self.build_inline_comments_between_doc_trailing_space_opt(kw_end, items[0].span.start)
        });

        // A line comment or multiline block between the keyword and the first item
        // hangs the items on the next line — mirroring the as/satisfies + type-param
        // keyword→value handling. The keyword stays inline; only the items are pushed
        // down (no whole-heritage break). A single-line block comment (own-line,
        // trailing, or glued) collapses inline (the fall-through below); prettier
        // relocates the collapsed comment before the keyword.
        if let Some(kw_start) = keyword_start {
            let kw_end = kw_start + keyword.as_str().len() as u32;
            if self.comments_force_own_line_between(kw_end, items[0].span.start) {
                // Items carrying their own line comments must join with the
                // gap-aware separators — mirroring the group-mode line-comment join
                // below. A plain `", "` join would let a per-item line comment swallow
                // the next item (`// c1, B` — non-reparseable content loss).
                let value_doc = if has_any_item_line_comments {
                    self.join_heritage_items(&item_docs, &gaps, d.hardline())
                } else {
                    d.join(item_docs, ", ")
                };
                let mut parts = smallvec![d.text(keyword.as_str())];
                self.append_keyword_value_line_comments(
                    &mut parts,
                    kw_end,
                    items[0].span.start,
                    value_doc,
                );
                return d.concat(&parts);
            }
        }

        if group_mode {
            if has_any_item_line_comments {
                // Line comments force hardline breaks.
                let types_joined = self.join_heritage_items(&item_docs, &gaps, d.hardline());
                let inner = d.indent(match kw_comments {
                    Some(c) => d.concat(&[d.hardline(), c, types_joined]),
                    None => d.concat(&[d.hardline(), types_joined]),
                });
                d.concat(&[d.text(keyword.as_str()), inner])
            } else {
                // Width-based breaks. No gap is `Baked` here — that needs a line
                // comment, which this branch excludes.
                let types_joined = self.join_heritage_items(&item_docs, &gaps, d.line());
                let hung = match kw_comments {
                    Some(c) => d.concat(&[c, types_joined]),
                    None => types_joined,
                };
                d.concat(&[d.text(keyword.as_str()), hang_after_operator(d, hung)])
            }
        } else {
            let keyword_space = keyword.with_space();
            match kw_comments {
                Some(c) => d.concat(&[d.text(keyword_space), c, d.join(item_docs, ", ")]),
                None => d.concat(&[d.text(keyword_space), d.join(item_docs, ", ")]),
            }
        }
    }

    /// The `>`→`(` gap of a signature carrying `type_parameters`: the type-parameter list's
    /// end paired with the opening paren that follows it, ready for
    /// [`Self::append_signature_head_gap_comments`].
    ///
    /// `None` when there are no type parameters, or when no `(` follows them — the scan skips
    /// comments, so a `(` written *inside* one (`<T> /* c(d) */ (p: T)`) is not mistaken for
    /// the parameter list's.
    pub(crate) fn type_params_paren_gap(
        &self,
        type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
    ) -> Option<(u32, u32)> {
        let tp = type_parameters?;
        find_char_skipping_comments(
            self.source.as_bytes(),
            tp.span.end as usize,
            self.source.len(),
            b'(',
        )
        .map(|paren| (tp.span.end, paren as u32))
    }

    /// Emit the comments in a signature's HEAD→`(` gap and append the signature `tail`
    /// (the parameter list, plus whatever the caller groups with it) behind them.
    ///
    /// Two gap families share this seam, because they answer the same question — the
    /// type-parameter list's `>`→`(` gap in all eight of its spellings (arrow, function
    /// expression and declaration, class and object method, `declare function`, method and
    /// construct signature, function type), and the constructor type's `abstract`→`new` and
    /// `new`→next keyword gaps.
    ///
    /// ⚠️ **A `//` here takes a real break, never a `line_suffix`.** A deferred run does not
    /// leave the construct it was written in: deferring carries the comment past the
    /// parameters, the `=>` and into the body, where a second `//` **welds** onto its line
    /// (`// c1 // c2`, the second ceasing to be a comment) and an inline block **reorders**
    /// ahead of it. Both outputs reparse and are their own fixed point, so the ledger, the
    /// census, F1 and the fuzzer are all blind — this seam is the guard. So the line-comment
    /// gap routes to [`Self::append_keyword_value_line_comments`], which keeps every comment
    /// on the line the author gave it and drops the tail to a continuation line indented one
    /// level ([`docs/conformance_prettier.md`](../../../../../docs/conformance_prettier.md)
    /// §Uniform Forced-Continuation Indent) — the same emitter every sibling keyword→value
    /// gap uses, rather than a rule of its own.
    ///
    /// A gap holding **only blocks** forces nothing and stays inline, which is what
    /// [`Self::push_trailing_comments_in_range`] already renders for it: with no line comment
    /// in the run nothing is deferred, so every block takes the inline arm and an authored
    /// own-line break collapses, per §Comment Position Philosophy's single-line-block rule.
    ///
    /// `flat_separator` is what stands between the gap and `tail` when nothing breaks — empty
    /// for a `>`→`(` gap (`<T>(p: T)` is glued), a space for the constructor type's keyword
    /// gaps (`abstract new`). It is deliberately **not** emitted on the forced-break arm,
    /// where the continuation's own hardline is the separator and a space would open the
    /// line with stray indentation.
    ///
    /// `gap` is `None` where the head has no such gap at all — no type parameters, or a `(`
    /// the scan cannot find in a partial signature — and the tail is then appended behind the
    /// flat separator alone. Taking the `Option` here rather than at each caller is what keeps
    /// the ten sites from restating the no-gap arm ten ways.
    pub(crate) fn append_signature_head_gap_comments(
        &self,
        parts: &mut DocBuf,
        gap: Option<(u32, u32)>,
        flat_separator: Option<DocId>,
        tail: DocId,
    ) {
        let Some((gap_start, gap_end)) = gap else {
            parts.extend(flat_separator);
            parts.push(tail);
            return;
        };
        if self.has_line_comments_between(gap_start, gap_end) {
            // ⚠️ The tail is GROUPED before it goes under the continuation. The break this
            // gap forces belongs to the gap, not to the parameter list — but the run's
            // hardline propagates into any tail that carries no group of its own, which
            // tore `(p: T)` open on exactly the two spellings whose tail is ungrouped by
            // construction (the arrow's `build_arrow_params_doc_ungrouped`, and the function
            // type built from the same parts). Every other spelling arrives already grouped
            // by `build_callable_signature_doc` / `build_signature_params_return_group` and
            // is unaffected, so grouping here is what makes the eight agree — the same
            // boundary prettier draws by emitting `print("typeParameters")` beside
            // `group([parametersDoc, returnTypeDoc])` rather than inside it.
            let tail = self.d().group(tail);
            self.append_keyword_value_line_comments(parts, gap_start, gap_end, tail);
        } else {
            self.push_trailing_comments_in_range(parts, gap_start, gap_end);
            parts.extend(flat_separator);
            parts.push(tail);
        }
    }

    /// Emit a generator `*` marker together with any comments around it,
    /// preserving the author's position relative to the star.
    ///
    /// Comments authored between `search_start` and the `*` lead it
    /// (`async /* c */ *m`); comments between the `*` and the key trail it
    /// (`*/* c */ m`). The `*` is located with the comment/string-skipping scan,
    /// so a `*` inside a comment (`/* a * b */`) is not mistaken for the marker.
    /// This pushes the `*` itself — call it instead of pushing `d.text("*")`.
    ///
    /// For a **computed** key the after-`*` scan is bounded at `[`
    /// ([`Self::computed_key_name_bound`]): comments inside the brackets belong
    /// to the computed-key bracket builder, so scanning to the key expression's
    /// start (past `[`) would duplicate them onto the `*` (`*/* c */ [/* c */ a]`).
    pub(crate) fn push_generator_star_doc(
        &self,
        parts: &mut DocBuf,
        search_start: u32,
        key_start: u32,
        computed: bool,
    ) {
        let d = self.d();
        let star = find_char(
            self.source.as_bytes(),
            search_start as usize,
            key_start as usize,
            b'*',
            TriviaProfile::JS,
        )
        .map(|i| i as u32);

        // Comments before the `*` lead it, at the author's position. A generator
        // always has a real `*`; if (defensively) none is found, treat the whole
        // gap as "before" so no comment is ever dropped.
        //
        // Both runs go through the single leading-comment emitter rather than a
        // separator of their own (`docs/comments.md` §Leading comments): what each
        // run leads happens to be a token rather than a node, but the question —
        // does this comment stay glued to what follows? — is the same one, and its
        // line-comment answer is load-bearing here. A `//` runs to end of line, so
        // an inline separator swallows the `*`, the key and the whole method body
        // into it, output that does not reparse.
        let star_pos = star.unwrap_or(key_start);
        self.push_leading_comment_run(
            parts,
            comments_to_emit_in_range(self.comments, search_start, star_pos),
            star_pos,
            LeadingGlue::Adjacent,
            None,
        );
        parts.push(d.text("*"));
        // Comments between the `*` and the key trail it (bounded at `[` for a
        // computed key, whose in-bracket comments the bracket builder owns).
        if let Some(star) = star {
            let name_bound = self.computed_key_name_bound(star + 1, key_start, computed);
            self.push_leading_comment_run(
                parts,
                comments_to_emit_in_range(self.comments, star + 1, name_bound),
                name_bound,
                LeadingGlue::Adjacent,
                None,
            );
        }
    }

    /// Emit leading comments in `[keyword_end, value_start)` followed by
    /// `value_doc` broken onto its own indented line. Use when at least one line
    /// comment sits in the gap (a line comment forces the value down). The caller
    /// pushes the keyword/operator itself first, **without** a trailing space.
    ///
    /// A comment on the **same source line** as `keyword_end` trails the keyword
    /// inline — a block as ` /* c */`, a line comment via `line_suffix` (zero
    /// width, so a long trailing comment never forces a *preceding* group, e.g. a
    /// constraint/annotation union, to break — matching prettier's `lineSuffix`).
    /// Each **own-line** comment goes on its own line before the value; they are
    /// never joined onto one line (which would make a following `//` stop being a
    /// delimiter — a boundary loss).
    ///
    /// That own-line preservation is the whole reason this seam is distinct from
    /// [`Self::build_continuation_indent`], which pulls the first comment up onto the
    /// head's line: the comments here **lead a value**, and own-line-ness is authoring
    /// signal for a leading position (conformance_prettier.md §Comment Position
    /// Philosophy). Shared by type-parameter constraint/default values (`= `/`extends`),
    /// class-property initializers (`= `), the prefix type operators (`keyof`/`typeof`),
    /// `infer`, the predicate `is` and cast keyword→type gaps, the function-type
    /// `=>`→return gap, a switch label's `case`→test gap, and the two expression-level
    /// keyword→operand gaps — `new`→callee (`build_new_doc_with_wrapping`) and
    /// `await`→operand (`build_await_doc`), whose tail is the WHOLE operand so a broken
    /// argument list renders at the hang's indent.
    ///
    /// ⚠️ The caller pushes its keyword **bare** — this seam owns every separator after
    /// it, so a keyword text carrying its own trailing space renders two.
    pub(crate) fn append_keyword_value_line_comments(
        &self,
        parts: &mut DocBuf,
        keyword_end: u32,
        value_start: u32,
        value_doc: DocId,
    ) {
        let d = self.d();
        let mut value_block: DocBuf = smallvec![d.hardline()];
        let mut on_own_line = false;
        let comments: CommentVec<'_> =
            comments_to_emit_in_range(self.comments, keyword_end, value_start).collect();
        for (i, comment) in comments.iter().enumerate() {
            let same_line = !on_own_line && self.is_same_line(keyword_end, comment.span.start);
            if same_line {
                if comment.is_block {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                } else {
                    parts.push(self.build_trailing_line_comment_doc(comment));
                    on_own_line = true; // a line comment ends its line
                    // That comment ended the keyword's line and `value_block` opens with
                    // the hardline onto the next one, so an author blank between them has
                    // no other emitter — it belongs to this seam. The break is forced (a
                    // `//`), so the blank survives with it, as at every other forced
                    // continuation.
                    let next = self.blank_scan_end_after(
                        comment,
                        comments.get(i + 1).map_or(value_start, |c| c.span.start),
                    );
                    // Prepending is well-defined: this arm needs `!on_own_line`, which
                    // only the arm itself clears, and the same-line *block* arm above
                    // doesn't touch `value_block` — so it still holds nothing but its
                    // seed hardline, and the blank belongs before that.
                    debug_assert_eq!(value_block.len(), 1, "value_block is still its seed");
                    if self.has_blank_line_between_strict(comment.span.end, next) {
                        value_block.insert(0, d.literalline());
                    }
                }
            } else {
                on_own_line = true;
                value_block.push(self.build_comment_doc(comment));
                self.push_leading_run_separator(
                    &mut value_block,
                    comment,
                    comments.get(i + 1).map_or(value_start, |c| c.span.start),
                );
            }
        }
        value_block.push(value_doc);
        parts.push(d.indent(d.concat(&value_block)));
    }
}

/// End position of a heritage item (after type arguments if present).
fn heritage_item_end(item: &internal::TSInterfaceHeritage<'_>) -> u32 {
    item.type_arguments
        .as_ref()
        .map_or_else(|| item.expression.span().end, |ta| ta.span.end)
}
