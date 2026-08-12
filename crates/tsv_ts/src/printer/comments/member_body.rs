// The `{ … }` member-body walk, once.
//
// A class body, an interface body and both of the type literal's force-multiline
// member walks emit the same five things per member — the leading comment run, the
// blank-line separator, the member doc (with a format-ignore freeze arm), the trailing
// seam, and the deferred-line-comment flag threaded into the next member's leading run
// and into the body's end-of-run emitter — plus that end-of-run emitter itself. They
// held four copies of it, and the copy is what lets a walk drift: the interface's
// re-spelled `is_same_line` trailing filter and the block walk's emit-keyed blank scan
// were both found that way, one family at a time.
//
// The member analog of `build_statement_list_docs_into` (`expressions/blocks.rs`), and
// the differences that remain are stated as per-family FACTS ([`MemberBody`]'s enums,
// the shape [`MemberGap`] already uses) rather than guessed by the walk. An **enum**
// body is not one of these: its members are comma-separated, so it takes the
// element-comma seam like every other comma list.

use super::{CommentVec, Printer};
use crate::printer::MemberGap;
use tsv_lang::Span;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Where a member gap's blank-line scan opens.
///
/// A blank-line scan is an **in-source** question, so it must not span comment bytes —
/// yet the class body opens it at the walk's own cursor. The two readings can only
/// disagree over a comment physically in the gap that this gap does not EMIT (an owned
/// one, or one the previous member's trailing emitter already took), which is why the
/// class body has never been observed to differ; they are not the same question all the
/// same, so the family states which it asks.
///
/// TODO: the class body reads [`Self::FromCursor`] where the other three member bodies
/// read [`Self::PastComments`]. Unifying them is a behaviour change that has to be
/// argued and fixtured on its own, not folded into a walk extraction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MemberBlankScan {
    /// From the walk's cursor, whatever comment bytes lie between it and the scan's end.
    FromCursor,
    /// Past every comment physically in the gap ([`Printer::blank_scan_start`]), so a
    /// multi-line block's interior newlines aren't read as an authored blank line
    /// (`a: 1; /*⏎…⏎*/⏎b` has no blank line between the members).
    PastComments,
}

/// What an honored `prettier-ignore` in a member's leading gap emits verbatim.
///
/// The extent is a per-family fact because it must stop where the family's own emission
/// resumes: a class or interface member's doc carries its own terminator, while the
/// type-literal walk re-adds the `;` from its seam and so must not let the frozen slice
/// carry one too.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MemberFreeze {
    /// No freeze arm at all — the alignment walk (a union-member / parenthesized-
    /// intersection object type), which has never honored a directive here.
    ///
    /// TODO: this one is a KNOWN BUG rather than a family fact — it is the
    /// `UNHONORED TSTypeLiteral.members` line in
    /// `crates/tsv_debug/src/cli/commands/ignore_audit_known.txt`, and the plain type
    /// literal next door honors the same directive through [`Self::Content`]. Named here so
    /// the walk's shape does not hide it; switching this arm to `Content` is the fix, and it
    /// needs its own fixture plus a `deno task ignore:audit:update` re-pin.
    Never,
    /// The member's whole span, its terminator included.
    Span,
    /// The member's content only, stopping at the seam anchor, because the walk's own
    /// seam re-emits the separator ([`Printer::raw_source_range`]).
    Content,
}

/// Where the leads-next test ([`Printer::trailing_claim_end`]) opens in the gap after a
/// member whose doc carries its own terminator — the `floor` of [`MemberGap::Next`],
/// named per family rather than guessed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MemberFloor {
    /// The member's own end: an interface member's span already covers its separator.
    MemberEnd,
    /// Past the last stray `;` in the gap ([`Printer::class_member_gap_floor`]): a class
    /// body's stray `;` closes its slot while producing no member node to key on.
    PastStraySemicolons,
}

/// How a member's trailing run is emitted.
///
/// The two arms are not a preference. A class or interface member's doc prints its own
/// `;`, so its trailing run is emitted whole and the cursor advances past it
/// ([`Printer::member_trailing_run`]). A **type** member's `;` is the walk's, so its run
/// has to be partitioned around that separator with the member→`;` gap's deferred run
/// slotted between the halves ([`Printer::push_type_member_trailing_seam`]) — emitting
/// the two in one flush welds a second `//` onto the first and reorders a block ahead of
/// it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MemberSeam {
    /// The member doc carries its own terminator (class, interface).
    Whole { floor: MemberFloor },
    /// The walk re-emits the terminator and partitions the run around it (type literal).
    AroundSeparator,
}

/// The per-family facts of a `{ … }` member-body walk.
///
/// `span` is the body's own span, braces included: the walk's cursor opens just past the
/// `{` and its end-of-body run closes just before the `}`.
///
/// `has_comments` is the caller's zero-comment fast gate over that span — one binary
/// search short-circuiting every per-member comment sub-query, sound because comments
/// are disjoint + start-sorted and every sub-range lies inside the body. Blank-line
/// preservation is comment-independent and stays outside it.
///
/// ⚠️ **Either comment axis is admissible here, and that is a fact about what the gate
/// GUARDS rather than a free choice.** The class and interface bodies pass the **on-page**
/// reading and the type literal the **to-emit** one; both are sound only because every
/// sub-query under the gate is itself to-emit — the leading collect and the trailing runs
/// by construction, and the two predicates that look like exceptions are not
/// (`member_gap_frozen` recognizes a directive, which is never owned, and
/// `terminator_defers_line_comment` a line comment, which is never owned either). A new
/// sub-query asking an **on-page** question — any layout gate: break, expand, hug, paren —
/// does NOT inherit that argument, and an emit-keyed gate would blind it. See
/// `docs/comments.md` §the three axes.
///
/// `delimiter_pull_pos` is `Some(pos)` when the caller emitted the comments sharing the
/// opening `{`'s line as a prefix on that line, so the first member's leading run must
/// drop the same ones ([`Printer::first_member_leading_comments`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct MemberBody {
    pub span: Span,
    pub has_comments: bool,
    pub delimiter_pull_pos: Option<u32>,
    pub blank_scan: MemberBlankScan,
    pub freeze: MemberFreeze,
    pub seam: MemberSeam,
}

impl<'a> Printer<'a> {
    /// Fill `parts` with everything a `{ … }` member body prints between its braces: a
    /// hardline-separated member per line, each preceded by its leading comment run and
    /// any authored blank line, followed by its trailing run, and then the end-of-body
    /// comment run before the `}`.
    ///
    /// The caller supplies the braces (and any brace-line prefix), guards the EMPTY body
    /// — every family has its own empty-body emitter, and a dangling comment there is
    /// that emitter's business — and states the per-family facts ([`MemberBody`]).
    ///
    /// `get_span` is the member's own span; `seam_anchor` is where its trailing run
    /// opens, which is the span end for a member that prints its own terminator and the
    /// **content** end for a type member, whose span swallows the `;`/`,` the walk
    /// re-emits. `build_member` builds the member doc, pushing into the supplied sink any
    /// comments its member→`;` gap deferred past the terminator — that run belongs
    /// between the `;` and whatever trails it, which is the order
    /// [`Printer::build_member_with_semicolon_doc`] gives a member that prints its own.
    pub(in crate::printer) fn build_member_list_docs_into<T>(
        &self,
        parts: &mut DocBuf,
        members: &[T],
        body: MemberBody,
        get_span: impl Fn(&T) -> Span,
        seam_anchor: impl Fn(&T) -> u32,
        build_member: impl Fn(&T, &mut DocBuf) -> DocId,
    ) {
        let d = self.d();
        let mut prev_end = body.span.start + 1; // after the `{`
        // Set when the member just emitted deferred a line comment past its own `;`, so
        // its doc ends on a later line than the `;` and cannot carry that line's comments.
        let mut prev_deferred_line_comment = false;

        for (i, member) in members.iter().enumerate() {
            let member_span = get_span(member);
            let member_start = member_span.start;
            let is_first = i == 0;

            // The leading run: what the previous member's trailing emitter did NOT claim.
            // A comment whose glue chain reaches this member's start leads it even from
            // the previous member's line ([`Printer::comment_already_trailed`]'s
            // `leads_target` escape, the trailing claim's other half), and
            // `prev_deferred_line_comment` hands over a whole gap the previous member
            // could not trail — it deferred a line comment past its own `;` and so
            // trailed nothing on that line, leaving this run its only emitter.
            let leading: CommentVec<'_> = if !body.has_comments {
                CommentVec::new()
            } else {
                let all: CommentVec<'_> =
                    comments_to_emit_in_range(self.comments, prev_end, member_start).collect();
                if is_first {
                    // A first member has no previous member, so it can never be the
                    // deferring case; the only exclusion is what the `{` line pulled.
                    self.first_member_leading_comments(all, body.delimiter_pull_pos)
                } else {
                    all.iter()
                        .filter(|c| {
                            !self.comment_already_trailed(
                                Some(prev_end),
                                c,
                                prev_deferred_line_comment,
                                Some(member_start),
                            )
                        })
                        .copied()
                        .collect()
                }
            };

            // Blank-line preservation, measured up to the first comment this gap emits
            // (or the member itself) — never across it.
            if !is_first {
                let check_pos = leading.first().map_or(member_start, |c| c.span.start);
                let blank_start = match body.blank_scan {
                    MemberBlankScan::FromCursor => prev_end,
                    MemberBlankScan::PastComments => self.blank_scan_start(prev_end, check_pos),
                };
                if self.has_blank_line_between(blank_start, check_pos) {
                    parts.push(d.literalline());
                }
            }
            parts.push(d.hardline());

            self.push_leading_comments_before(parts, &leading, member_start);

            // A preceding format-ignore directive keeps the member's source verbatim, over
            // the extent the family named ([`MemberBody::freeze`]).
            let anchor = seam_anchor(member);
            let mut deferred = DocBuf::new();
            let frozen = body.has_comments
                && !matches!(body.freeze, MemberFreeze::Never)
                && self.member_gap_frozen(prev_end, member_start);
            parts.push(match (frozen, body.freeze) {
                (true, MemberFreeze::Span) => self.raw_source_doc(member_span),
                (true, MemberFreeze::Content) => self.raw_source_range(member_start, anchor),
                _ => build_member(member, &mut deferred),
            });

            // The trailing seam, and with it the deferred flag the next member's leading
            // run and the end-of-body emitter read as their `claims_trailing`.
            let next_start = members.get(i + 1).map(|next| get_span(next).start);
            (prev_end, prev_deferred_line_comment) = match body.seam {
                MemberSeam::Whole { floor } => {
                    debug_assert!(
                        deferred.is_empty(),
                        "a member printing its own `;` places its own deferred run"
                    );
                    self.push_whole_member_trailing_seam(
                        parts,
                        body,
                        member_span,
                        floor,
                        next_start,
                    )
                }
                MemberSeam::AroundSeparator => self.push_type_member_trailing_seam(
                    parts,
                    body,
                    member_span,
                    anchor,
                    next_start,
                    deferred,
                ),
            };
        }

        // The run after the last member, before the closing `}` — the shared end-of-body
        // emitter every container uses, taking its separator BEFORE each comment.
        if body.has_comments {
            parts.extend(self.build_trailing_body_comments_doc(
                prev_end,
                body.span.end.saturating_sub(1),
                prev_deferred_line_comment,
            ));
        }
    }

    /// The trailing seam for a member whose doc prints its own terminator
    /// ([`MemberSeam::Whole`]) — the class and interface arm, where the type literal takes
    /// [`Printer::push_type_member_trailing_seam`].
    ///
    /// The two are siblings on purpose: one call each, returning the walk's new
    /// `(prev_end, prev_deferred_line_comment)`, so the walk states WHICH seam a family
    /// takes and neither arm's rules leak into the loop body. This one is a thin shell over
    /// [`Printer::member_trailing_run`] — the whole run is emitted and the cursor advances
    /// past it — because the member's `;` is already inside its doc.
    fn push_whole_member_trailing_seam(
        &self,
        parts: &mut DocBuf,
        body: MemberBody,
        member_span: Span,
        floor: MemberFloor,
        next_start: Option<u32>,
    ) -> (u32, bool) {
        let member_end = member_span.end;
        let deferred_line_comment =
            body.has_comments && self.terminator_defers_line_comment(member_span.start, member_end);
        if deferred_line_comment {
            // Nothing may share the deferred comment's output line, so leaving the cursor at
            // the `;` hands that line's comments to the next member's leading run; advancing
            // it past them would DROP them.
            return (member_end, true);
        }
        if !body.has_comments {
            return (member_end, false);
        }
        let gap = match next_start {
            Some(start) => MemberGap::Next {
                floor: match floor {
                    MemberFloor::MemberEnd => member_end,
                    MemberFloor::PastStraySemicolons => {
                        self.class_member_gap_floor(member_end, start)
                    }
                },
                start,
            },
            None => MemberGap::Last {
                list_end: body.span.end,
            },
        };
        let (trailing, cursor) = self.member_trailing_run(member_end, gap);
        parts.extend(trailing);
        (cursor, false)
    }
}
