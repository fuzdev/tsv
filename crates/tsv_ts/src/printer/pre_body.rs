// The head→body `{` seam: one gap, resolved once, for every braced-body declaration.
//
// `interface` / `enum` / `namespace` / `module`, the class header, and every value-level
// function definition (declaration and expression, class method, getter/setter,
// constructor, object method) all reach their body brace across the same gap, and the
// answer is the same question at each: which comments the gap holds, whether the brace
// leaves the line, and whether an honored format-ignore directive freezes the body. What
// lives here is that question's run emitter, its resolution, the two shapes callers take
// the resolution in — a finished doc, or the doc plus the freeze verdict — and the body
// appender the value-level function definitions reach it through.
//
// The callers vary only in the SEPARATOR they want for a given verdict (a plain header
// hugs the brace with `" "`; the class header's group uses a collapsible `line()`), which
// is why the resolution is exposed as a pair as well as a finished doc. Deriving the run
// or the verdict separately is what let the class header drift.

use crate::ast::internal;
use crate::printer::Printer;
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    /// Emit the comments in `[start, end)` between a declaration header (after the last
    /// heritage item, type params, or the signature's `)`) and the body `{`, preserving
    /// each comment where the author wrote it. The separator after each comment is
    /// [`Printer::comment_hangs_next`], the one predicate for this question: a line
    /// comment ends its line, so anything following one is pushed to its own line —
    /// otherwise it would be absorbed into the line comment's text (`// c1 // c2`
    /// reparses as a single comment, a content/boundary loss) — and a **multiline** block
    /// the author broke after keeps that break, the same rule the pre-separator and value
    /// gaps apply. The first comment, and a comment following one that hangs nothing,
    /// keep a leading space, matching the single-comment heritage form `J // c`.
    ///
    /// Returns `None` when the range has no comments. The caller appends the pre-`{`
    /// separator itself (`hardline` when the gap hangs, space/`line` otherwise —
    /// [`Printer::comments_force_own_line_between`] is the gate).
    /// `own_line_first` breaks before the FIRST comment instead of spacing it onto the
    /// header's line — set when the gap holds an honored format-ignore directive, whose own
    /// line is what makes it honored (a header-trailing placement is inert, so the relocated
    /// form would lose the freeze on the second pass).
    fn build_pre_body_comments_doc(
        &self,
        start: u32,
        end: u32,
        own_line_first: bool,
    ) -> Option<DocId> {
        let d = self.d();
        let mut parts = DocBuf::new();
        let mut prev_hangs = own_line_first;
        let mut comments = comments_to_emit_in_range(self.comments, start, end).peekable();
        while let Some(comment) = comments.next() {
            if prev_hangs {
                parts.push(d.hardline());
            } else {
                parts.push(d.text(" "));
            }
            parts.push(self.build_comment_doc(comment));
            // **in source**: the hang question anchors on the physically next comment, so
            // an owned comment sitting between two emitted ones can't desync this emitter
            // from the gate that selected it (both walk the same range).
            let emit_next = comments.peek().map_or(end, |n| n.span.start);
            let next = self.blank_scan_end(comment.span.end, emit_next);
            prev_hangs = self.comment_hangs_next(comment, next);
        }
        if parts.is_empty() {
            None
        } else {
            Some(d.concat(&parts))
        }
    }

    /// The header→body `{` gap resolved once: the gap's comment run (`None` when it holds
    /// none) and whether the gap forces the brace onto its own line.
    ///
    /// The brace leaves the line when a `//` is in the gap (it would otherwise swallow the
    /// brace), when a **multiline** block the author broke after keeps that break, or when
    /// the body is FROZEN — a directive must keep the own line that makes it honored, since
    /// a header-trailing placement is inert and would lose the freeze on the second pass.
    ///
    /// ⚠️ The comment half is [`Printer::comments_force_own_line_between`] — the shared
    /// gate, not a line-vs-block reading of its own. Asking `has_line_comments_between` here
    /// made this gap the one place a broke-after multiline block collapsed, against the rule
    /// every pre-separator and value gap applies; the run
    /// ([`Printer::build_pre_body_comments_doc`]) asks the same per-comment predicate, so
    /// gate and emitter cannot answer differently.
    ///
    /// Returned as a pair rather than a finished doc because the two header layouts want
    /// **different separators** for the same verdict: a plain header hugs with `" "`, while
    /// the class header's group uses a collapsible `line()` (and `" "` for an empty body).
    /// That difference is the only thing they may vary — deriving the run or the verdict
    /// separately is what let the class header drift.
    pub(in crate::printer) fn pre_body_gap(
        &self,
        header_end: u32,
        body_start: u32,
        body_frozen: bool,
    ) -> (Option<DocId>, bool) {
        let comments = self.build_pre_body_comments_doc(header_end, body_start, body_frozen);
        let breaks = body_frozen || self.comments_force_own_line_between(header_end, body_start);
        (comments, breaks)
    }

    /// The header→body `{` gap as a finished doc, for the header layouts that hug the brace
    /// with a plain space: `interface B /* c */ {`, `function a() // c⏎{`. A bare `" "` when
    /// the gap holds no comments. The class header takes [`Self::pre_body_gap`] directly,
    /// because its separator is group-relative.
    pub(in crate::printer) fn build_header_pre_body_doc(
        &self,
        header_end: u32,
        body_start: u32,
        body_frozen: bool,
    ) -> DocId {
        let d = self.d();
        match self.pre_body_gap(header_end, body_start, body_frozen) {
            (Some(comments), true) => d.concat(&[comments, d.hardline()]),
            (Some(comments), false) => d.concat(&[comments, d.text(" ")]),
            (None, _) => d.text(" "),
        }
    }

    /// A braced-body declaration's header→`{` gap, answered in one call: the pre-`{` doc
    /// (the gap's comments plus the separator) and the format-ignore verdict for the body.
    ///
    /// ⚠️ The two answers must come from **one** resolution of the gap. The header needs
    /// it to keep an honored directive on its own line — a header-trailing placement is
    /// inert, so following the ordinary relocation would lose the freeze on tsv's own
    /// second pass — and the body needs the span to emit verbatim. Resolving it twice is
    /// two sources of truth for one question.
    ///
    /// `interface`, `enum`, `namespace`/`module` and every value-level function definition
    /// (declaration and expression, class method, getter/setter, constructor, object method
    /// — via [`Printer::append_body_with_sig_comments`]) take this. The **class** resolves
    /// the same pair by hand, because its header is assembled inside a group
    /// ([`Printer::build_class_header_doc`]) and the verdict has to travel there as
    /// `ClassHeaderOptions::body_frozen`.
    ///
    /// A `body` span whose `start` precedes `header_end` — no `{` where one was expected —
    /// yields the bare `" "` and no freeze: both lookups read the gap as a source range,
    /// and an inverted range holds nothing.
    pub(in crate::printer) fn build_declaration_pre_body_doc(
        &self,
        header_end: u32,
        body: Span,
    ) -> (DocId, Option<Span>) {
        let frozen = self.gap_frozen_span(header_end, body);
        let pre_body = self.build_header_pre_body_doc(header_end, body.start, frozen.is_some());
        (pre_body, frozen)
    }

    /// Append a value-level function definition's body, with the comments the author left
    /// in the signature→body `{` gap (function declarations and expressions, class methods,
    /// getters/setters, constructors, object methods).
    ///
    /// The gap keeps its comments — a single-line block stays inline with `{` hugging it
    /// (`gen() /* c */ {}`), while a **line** comment or a **multiline** block the author
    /// broke after drops `{` to its own line, flush with the head
    /// (`gen() // c⏎{}`). That is the class/interface/enum/namespace answer to the same gap
    /// ([`Printer::build_header_pre_body_doc`]), reached through the same gate
    /// ([`Printer::comments_force_own_line_between`]) and the same per-comment separator
    /// ([`Printer::comment_hangs_next`]), so every braced-body declaration answers it
    /// identically.
    ///
    /// ⚠️ The brace **must** leave the line when a `//` is in the gap: emitted inline it
    /// would be swallowed into the comment's text (`gen() // c {`), output that does not
    /// reparse. This path used to relocate such a comment *into the body* instead — the
    /// one move across `{` tsv declines everywhere else, and the reason the catalog's
    /// "consistent with tsv's handling of line comments before block bodies across all
    /// statement types" was false of exactly this family.
    ///
    /// Sharing the emitter is also what gives this gap its **format-ignore** opt-in: a
    /// directive alone on a line here freezes the body, and keeping it on its own line is
    /// what keeps it honored on the second pass. While the gap relocated its line comments
    /// into the body, no directive could reach that state, so the freeze had nothing to
    /// hook — which is why only the class/interface/enum/namespace half ever had it.
    pub(crate) fn append_body_with_sig_comments(
        &self,
        parts: &mut DocBuf,
        sig_end: u32,
        body: &internal::BlockStatement<'_>,
    ) {
        let (pre_body, frozen) = self.build_declaration_pre_body_doc(sig_end, body.span);
        parts.push(pre_body);
        if let Some(frozen) = frozen {
            parts.push(self.build_frozen_span_doc(frozen));
        } else {
            parts.push(self.build_block_statement_doc(body));
        }
    }
}
