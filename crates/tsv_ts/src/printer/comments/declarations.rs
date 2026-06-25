// Member-keyword / modifier-marker / heritage comment emitters.
//
// These preserve comments in the gaps of a declaration header: between member
// keywords (`static` / `readonly` / `get` / `set`), around optional/definite
// markers (`?` / `!`), in the marker→`:` and keyword→name gaps, and within
// heritage clauses (`extends` / `implements`).

use super::layout::hang_after_operator;
use super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal;
use smallvec::smallvec;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;
use tsv_lang::source_scan::{TriviaProfile, find_char, find_char_skipping_comments};

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
            if self.has_comments_between(*cursor, kw_pos) {
                parts.push(self.build_trailing_comments_break_for_line(*cursor, kw_pos));
            }
            *cursor = kw_pos + keyword.len() as u32;
        }
        parts.push(self.d().text(kind_text));
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
        if self.has_comments_between(cursor, name_start) {
            parts.push(self.build_trailing_comments_break_for_line(cursor, name_start));
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
        #[allow(clippy::expect_used)] // Parser guarantees the marker exists when the flag is set
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
        // such comment — a `_prettier_divergence` (conformance_prettier.md §Comment
        // relocation). The marker is the continuation `tail`; later pushes continue
        // mid-line after it.
        if self.has_line_comments_between(after, pos) {
            parts.push(self.build_continuation_indent(after, pos, marker_doc));
        } else {
            if self.has_comments_between(after, pos) {
                parts.push(self.build_inline_comments_between_doc(after, pos));
            }
            parts.push(marker_doc);
        }
        pos + 1
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
    /// ([conformance_prettier.md](../../../../docs/conformance_prettier.md)
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
    /// comments via `build_trailing_comments_break_for_line` (each line comment
    /// terminated at end-of-line so a `//` can't swallow what follows), then `tail` —
    /// all wrapped in one `indent`, so only the first comment stays flush on the head
    /// line and everything after (remaining comments and `tail`) drops one level and
    /// reads as part of the construct, not a sibling.
    ///
    /// `start`/`end` bound the comment gap; `tail` is the continued content (a type,
    /// a `: type` annotation, …) the caller has already built. Used by the `:`→type
    /// annotation (`build_type_annotation_doc`), the marker→`:` before-colon gap
    /// (`build_marker_colon_line_continuation`), and the index-signature `]`→value-`:`
    /// gap (`build_index_signature_member_doc`). See conformance_prettier.md
    /// §Uniform Forced-Continuation Indent.
    pub(crate) fn build_continuation_indent(&self, start: u32, end: u32, tail: DocId) -> DocId {
        let d = self.d();
        d.indent(d.concat(&[
            d.text(" "),
            self.build_trailing_comments_break_for_line(start, end),
            tail,
        ]))
    }

    /// When a **line** comment sits in the marker→`:` gap of a key/binding's type
    /// annotation, build the indented continuation: the first comment trails the
    /// marker on its line, then any remaining comments and the `: type` (`type_doc`,
    /// built by the caller) drop to a continuation line indented one level — the
    /// uniform forced-continuation indent (`build_continuation_indent`), so the
    /// annotation reads as part of its key/binding rather than a sibling. Returns
    /// `None` when the gap has no line comment, leaving the caller's block /
    /// no-comment handling in place.
    ///
    /// `marker_end` is the offset just past the key (and any `?`/`!`); `colon_pos`
    /// is the type annotation's `:` (its span start). Callers gate on
    /// `has_comments_between` first, so the common (no-comment) path never reaches
    /// the `has_line_comments_between` probe here.
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
        self.has_line_comments_between(marker_end, colon_pos)
            .then(|| self.build_continuation_indent(marker_end, colon_pos, type_doc))
    }

    /// When a **line** comment sits in the name→`=` gap of an initializer, build the
    /// indented continuation: the comment trails the name on its line, then the `=`
    /// and value (`value_doc`, built by the caller — the bare value, no leading
    /// `= `) drop to a continuation line indented one level (the uniform
    /// forced-continuation indent, `build_continuation_indent`). Returns `None` when
    /// the gap has no line comment, leaving the caller's block / no-comment /
    /// assignment-layout handling in place.
    ///
    /// `name_end` is the offset just before the `=` gap (past the binding name and
    /// any `?`/`!`/type annotation); `eq_pos` is the `=`. `build_value` lazily builds
    /// the bare value doc — only invoked on the (rare) line-comment path, so the
    /// common no-comment path never builds the value twice. Unlike the `:` twin
    /// (`build_marker_colon_line_continuation`, where prettier keeps the continuation
    /// flush), prettier here *relocates* the comment past the value to
    /// end-of-statement — which is **lossy when a second comment already trails the
    /// construct** (prettier merges them onto one line, the second `//` becoming text;
    /// tsv keeps both comments distinct). Shared by the initializer `=` sites: enum
    /// members, class properties, variable declarators. See conformance_prettier.md
    /// §Comment relocation.
    pub(crate) fn build_initializer_line_continuation(
        &self,
        name_end: u32,
        eq_pos: u32,
        build_value: impl FnOnce() -> DocId,
    ) -> Option<DocId> {
        let d = self.d();
        self.has_line_comments_between(name_end, eq_pos).then(|| {
            let tail = d.concat(&[d.text("= "), build_value()]);
            self.build_continuation_indent(name_end, eq_pos, tail)
        })
    }

    /// Build a binding/identifier `: type` annotation including any before-`:`
    /// comment. A **line** comment keeps the comment after the marker and indents the
    /// `: type` continuation one level (`build_marker_colon_line_continuation`); a
    /// **block** stays inline with a space before `:` (` /* c */ : T`); no comment is
    /// just `: T`. `marker_end` is the offset past the name and any `!`/`?`; `wrap`
    /// selects the width-aware annotation builder (generics / wrapping type args).
    ///
    /// Gates on `has_comments_between` once, so the common no-comment path is a single
    /// binary search. Shared by every before-`:` site whose block form keeps the space
    /// before `:`: index-signature keys, class properties, variable bindings, and
    /// function parameters/identifiers. (Property signatures handle the gap inline:
    /// their non-optional block form omits that space.)
    pub(crate) fn build_binding_type_annotation_doc(
        &self,
        marker_end: u32,
        type_ann: &internal::TSTypeAnnotation,
        wrap: bool,
    ) -> DocId {
        let d = self.d();
        let colon_pos = type_ann.span.start;
        let type_doc = if wrap {
            self.build_type_annotation_doc_wrapping(type_ann)
        } else {
            self.build_type_annotation_doc(type_ann)
        };
        if !self.has_comments_between(marker_end, colon_pos) {
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

    /// Build a declaration header's keyword→name gap comment followed by the rest
    /// of the declaration (`continuation`), indenting that continuation one level
    /// when a *line* comment forces the break.
    ///
    /// `keyword_end` bounds the start of the comment scan and `name_start` its end
    /// (the start of the name, or first declarator). Usually `keyword_end` is just
    /// past the final keyword before the name (`function`/`*`, `class`, `const`, …),
    /// but the `enum` and `declare function` printers pass the declaration start, so
    /// a comment in an earlier inter-keyword gap (`const /* c */ enum`,
    /// `declare /* c */ function`) is captured here too and relocated after the
    /// keyword — matching the pre-refactor behavior. The preceding keyword token must
    /// be emitted **without** a trailing space; the leading space is supplied here.
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
        let has_line = self.has_line_comments_between(keyword_end, name_start);
        let comment_doc = if has_line {
            self.build_name_to_type_params_comments(
                keyword_end,
                name_start,
                CommentSpacing::Leading,
            )
        } else if let Some(c) = self.build_inline_comments_between_doc_opt(keyword_end, name_start)
        {
            c
        } else {
            d.empty()
        };
        // After a line comment the hardline provides separation; otherwise a space.
        let space_after = if has_line { d.empty() } else { d.text(" ") };
        let body = d.concat(&[comment_doc, space_after, continuation]);
        if has_line { d.indent(body) } else { body }
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
        let d = self.d();
        let first_idx = tsv_lang::find_first_comment_from(self.comments, start);
        let mut parts = DocBuf::new();
        // After a line comment's hardline the next comment starts a fresh (indented)
        // line, so it must not get a leading space — otherwise a 2nd+ own-line comment
        // renders as `\t // c` (stray leading space).
        let mut at_line_start = false;
        for comment in self.comments[first_idx..]
            .iter()
            .take_while(|c| c.span.end <= end)
        {
            if comment.is_block {
                // Block comment: use caller-specified spacing
                match block_spacing {
                    CommentSpacing::Leading => {
                        if !at_line_start {
                            parts.push(d.text(" "));
                        }
                        parts.push(self.build_comment_doc(comment));
                    }
                    CommentSpacing::Trailing => {
                        parts.push(self.build_comment_doc(comment));
                        parts.push(d.text(" "));
                    }
                    CommentSpacing::None => {
                        parts.push(self.build_comment_doc(comment));
                    }
                }
                at_line_start = false;
            } else {
                // Line comment: leading space (unless already at line start) +
                // hardline after — `class A // c\n<T> {}`
                if !at_line_start {
                    parts.push(d.text(" "));
                }
                parts.push(self.build_comment_doc(comment));
                parts.push(d.hardline());
                at_line_start = true;
            }
        }
        d.concat(&parts)
    }

    /// Like `build_name_to_type_params_comments`, but returns `None` when there
    /// are no comments in the range (avoids the separate `has_comments_between` check).
    pub(crate) fn build_name_to_type_params_comments_opt(
        &self,
        start: u32,
        end: u32,
        block_spacing: CommentSpacing,
    ) -> Option<DocId> {
        if self.has_comments_between(start, end) {
            Some(self.build_name_to_type_params_comments(start, end, block_spacing))
        } else {
            None
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
        for comment in comments_in_range(self.comments, start, end) {
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

    /// Build a heritage clause doc: `keyword` + indented, comma-separated heritage items.
    ///
    /// Handles line comments between items (SAFETY): when a line comment appears after
    /// a heritage item, the comma is placed before the comment to prevent the comment
    /// from absorbing subsequent items. Block comments keep the comma after.
    ///
    /// Used by both class `implements` and interface `extends` clauses.
    pub(crate) fn build_heritage_clause_doc(
        &self,
        keyword: &'static str,
        items: &[internal::TSInterfaceHeritage],
        group_mode: bool,
        keyword_start: Option<u32>,
    ) -> DocId {
        let d = self.d();

        // Track which items have trailing line comments (between this item and the next).
        // Line comments consume the rest of the line, so the comma must go before them.
        let has_trailing_line_comment: Vec<bool> = items
            .windows(2)
            .map(|pair| {
                self.has_line_comments_between(heritage_item_end(&pair[0]), pair[1].span.start)
            })
            .collect();
        let has_any_item_line_comments = has_trailing_line_comment.iter().any(|&v| v);

        let item_docs: Vec<_> = items
            .iter()
            .enumerate()
            .map(|(i, heritage)| {
                let mut h_parts: DocBuf =
                    smallvec![self.build_entity_name_doc(&heritage.expression)];
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
                    h_parts.push(self.build_type_arguments_doc_wrapping(type_args));
                }
                if let Some(next) = items.get(i + 1) {
                    let item_end = heritage_item_end(heritage);
                    let comments: Vec<_> =
                        comments_in_range(self.comments, item_end, next.span.start).collect();

                    if has_trailing_line_comment[i] {
                        // Has line comment(s): comma must go before the first line comment.
                        // Block comments before the first line comment go before the comma.
                        // e.g. `I /* c1 */,\n// c2\nJ` or `I, // c1\n// c2\nJ`
                        let first_line_idx = comments.iter().position(|c| !c.is_block).unwrap_or(0);

                        // Block comments before the first line comment
                        for comment in &comments[..first_line_idx] {
                            h_parts.push(d.text(" "));
                            h_parts.push(self.build_comment_doc(comment));
                        }

                        // Comma before the first line comment
                        h_parts.push(d.text(","));

                        // Remaining comments (starting with the first line comment)
                        // `needs_hardline` starts true when block comments precede
                        // (comma sits between block and line, needs newline after)
                        let mut needs_hardline = first_line_idx > 0;
                        for comment in &comments[first_line_idx..] {
                            if needs_hardline {
                                h_parts.push(d.hardline());
                            } else {
                                h_parts.push(d.text(" "));
                            }
                            h_parts.push(self.build_comment_doc(comment));
                            needs_hardline = !comment.is_block;
                        }
                    } else {
                        // No line comments: emit block comments inline with leading space
                        for comment in &comments {
                            h_parts.push(d.text(" "));
                            h_parts.push(self.build_comment_doc(comment));
                        }
                    }
                }
                d.concat(&h_parts)
            })
            .collect();

        // Optional comments between keyword and first item: `extends /* c */ Item`
        let kw_comments = keyword_start
            .and_then(|kw_start| {
                let kw_end = kw_start + keyword.len() as u32;
                self.build_comments_between_filtered_opt(
                    kw_end,
                    items[0].span.start,
                    CommentSpacing::Trailing,
                    CommentFilter::All,
                )
            })
            .unwrap_or_else(|| d.empty());

        // A line comment between the keyword and the first item is kept trailing
        // the keyword (preserve-in-place; prettier relocates it before the
        // keyword), with the items pushed onto the next line — mirroring the
        // as/satisfies + type-param keyword→value handling. The keyword stays
        // inline; only the items are pushed down (no whole-heritage break).
        if let Some(kw_start) = keyword_start {
            let kw_end = kw_start + keyword.len() as u32;
            if self.has_line_comments_between(kw_end, items[0].span.start) {
                let value_doc = d.join(item_docs, ", ");
                let mut parts = smallvec![d.text(keyword)];
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
                // Line comments force hardline breaks. Items with line comments have
                // commas baked in; others get commas from the separator.
                let comma_hardline = d.concat(&[d.text(","), d.hardline()]);
                let hardline = d.hardline();
                let mut joined_parts: DocBuf = smallvec![item_docs[0]];
                for (idx, &item_doc) in item_docs.iter().enumerate().skip(1) {
                    // Previous item had baked-in comma + line comment → just hardline
                    // Otherwise → comma + hardline
                    joined_parts.push(if has_trailing_line_comment[idx - 1] {
                        hardline
                    } else {
                        comma_hardline
                    });
                    joined_parts.push(item_doc);
                }
                let types_joined = d.concat(&joined_parts);
                let inner = d.indent(d.concat(&[d.hardline(), kw_comments, types_joined]));
                d.concat(&[d.text(keyword), inner])
            } else {
                let comma_line = d.concat(&[d.text(","), d.line()]);
                let types_joined = d.join_doc(item_docs, comma_line);
                d.concat(&[
                    d.text(keyword),
                    hang_after_operator(d, d.concat(&[kw_comments, types_joined])),
                ])
            }
        } else {
            let keyword_space = match keyword {
                "implements" => "implements ",
                "extends" => "extends ",
                _ => unreachable!(),
            };
            d.concat(&[d.text(keyword_space), kw_comments, d.join(item_docs, ", ")])
        }
    }

    /// Append comments between type params `>` and `(` to parts.
    ///
    /// Block comments are emitted inline with a leading space. Line comments
    /// use `line_suffix` so they're deferred to end of the rendered line
    /// (avoids corruption where `// c` would swallow `(x: T)`).
    pub(crate) fn append_type_params_to_paren_comments(
        &self,
        parts: &mut DocBuf,
        type_params_end: u32,
        paren_pos: u32,
    ) {
        for comment in comments_in_range(self.comments, type_params_end, paren_pos) {
            parts.push(self.build_trailing_comment_doc(comment));
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
        for comment in comments_in_range(self.comments, search_start, star.unwrap_or(key_start)) {
            parts.push(self.build_comment_doc(comment));
            parts.push(d.text(" "));
        }
        parts.push(d.text("*"));
        // Comments between the `*` and the key trail it (bounded at `[` for a
        // computed key, whose in-bracket comments the bracket builder owns).
        if let Some(star) = star {
            let name_bound = self.computed_key_name_bound(star + 1, key_start, computed);
            for comment in comments_in_range(self.comments, star + 1, name_bound) {
                parts.push(self.build_comment_doc(comment));
                parts.push(d.text(" "));
            }
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
    /// delimiter — a boundary loss). Shared by type-parameter constraint/default
    /// values (`= `/`extends`) and class-property initializers (`= `).
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
        for comment in comments_in_range(self.comments, keyword_end, value_start) {
            let same_line = !on_own_line && self.is_same_line(keyword_end, comment.span.start);
            if same_line {
                if comment.is_block {
                    parts.push(d.text(" "));
                    parts.push(self.build_comment_doc(comment));
                } else {
                    parts.push(self.build_trailing_line_comment_doc(comment));
                    on_own_line = true; // a line comment ends its line
                }
            } else {
                on_own_line = true;
                value_block.push(self.build_comment_doc(comment));
                value_block.push(d.hardline());
            }
        }
        value_block.push(value_doc);
        parts.push(d.indent(d.concat(&value_block)));
    }
}

/// End position of a heritage item (after type arguments if present).
fn heritage_item_end(item: &internal::TSInterfaceHeritage) -> u32 {
    item.type_arguments
        .as_ref()
        .map_or_else(|| item.expression.span().end, |ta| ta.span.end)
}
