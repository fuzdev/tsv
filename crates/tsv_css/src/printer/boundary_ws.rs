// CSS boundary-whitespace preservation — the printer half of the class named in
// `crate::whitespace::is_boundary_only_whitespace`.
//
// ## What this module is
//
// `parseCss` skips a JS-`\s` run at every `allow_whitespace()` juncture, and tsv's parser
// mirrors that skip (`CssParser::skip_boundary_whitespace`). The members at or above U+00A0
// are not ASCII whitespace, so dropping one from the OUTPUT is content loss — the corpus
// SAFETY check counts them as semantic characters. The parser decides what the AST does not
// contain; this module decides what the output must still carry, and the two are **one
// change**: a juncture added to the parser with no claim here trades a graceful
// over-rejection for content loss, the worse of the two.
//
// ## The two claims, and why they partition
//
// - [`Printer::preserved_boundary_ws`] scans BACKWARD from an anchor and takes the run
//   CONTIGUOUS with it. It needs a FLOOR wherever the previous node's span can end inside the
//   run (`read_identifier` folds a trailing run into the name it follows), or it emits a
//   character that name already carried.
// - [`Printer::boundary_ws_in_gap`] sweeps FORWARD over a gap, stepping comment interiors, and
//   takes every member in it. A backward scan alone stops at a `*/` and drops whatever a
//   comment strands behind it (`a <NBSP>/* c */ b`).
//
// [`Printer::boundary_ws_in_gap_before_anchor`] bounds the forward sweep at
// [`Printer::boundary_run`]'s offset — the backward scan's own starting point — so a gap can be
// claimed whole without either emitter printing one character twice.
// [`Printer::gap_boundary_ws`] is the composite, and the answer every emitter of a gap ending
// at a NAME should use.
//
// ## Where a claim is emitted
//
// Flush against the token that FOLLOWS the run, because there it can only re-parse as the run
// the parser skipped — but that is only half the question, and the other half is what stands
// BEHIND it:
//
// - a run emitted flush after a NAME glues INTO it (`read_identifier` takes every code point
//   at or above U+00A0 as content), so `[a <NBSP>]` comes back with the name `a<NBSP>` — an
//   AST that changed under a format, and one that is its own fixed point, so no idempotency,
//   reparse or content-count gate can see it. Every claim whose left neighbour can be a name
//   therefore asks [`Printer::name_run_separator`] (from the source position it resumes at) or
//   [`name_run_separator_after`] (from the text it is appending to) for the ASCII space that
//   ends the name first. ⚠️ That is **not** one gap: the attribute selector's `[name<HERE>]`,
//   a selector list's `,` (`a.x <NBSP>, c`), an explicit combinator's symbol
//   (`a <NBSP>> b`), a pseudo-argument list's `)` (`:is(a <NBSP>)`) and the commented
//   attribute rebuild's every interior gap all have a name on their left, and each was a live
//   glue. A claim added anywhere new owes the same question.
// - ahead of a COMBINATOR symbol the printer regenerates a space after the run, so the run's
//   own ASCII tail is trimmed or the line grows a column per pass.
//
// ## Who claims where
//
// One entry per juncture the parser steps a run at, so "a skip and a preservation are one
// change" is checkable by reading this list against `CssParser::skip_boundary_whitespace`:
//
// | juncture | claimer |
// | --- | --- |
// | a compound's first simple selector | `gap_boundary_ws` (both complex-selector builders) |
// | an explicit / anchorless combinator | `push_combinator_boundary_ws` |
// | a selector list's `,` | `pre_comma_boundary_ws` |
// | a rule's pre-`{` gap | `boundary_ws_in_gap` (`printer/rules.rs`) |
// | a pseudo-argument list's lead and its `)` | `build_pseudo_args_doc` |
// | `::part()`'s inter-name gaps | `build_part_idents_doc` |
// | every attribute-selector interior gap | `push_attribute_gap` / `push_name_tail_boundary_ws` |
// | a block child's rebuilt head (declaration / at-rule / comment) | [`Printer::write_head_boundary_ws`] |
// | a block's tail before `}` | [`Printer::write_block_tail_boundary_ws`] |
// | a condition prelude's part heads | `build_condition_query_doc` |
//
// A **rule** child needs no head claim: its selector's first compound already claims the same
// run, and a second claim would print it twice.
//
// ⚠️ [`skip_gap_trivia`] belongs to this module for the same reason: a printer scan that
// LOCATES a part by stepping trivia is retracing one of these skips, so its class must be the
// skip's. One that is narrower reports the RUN as the part's start.
//
// ## Residue
//
// Two positions still drop the run: the stylesheet's own trailing whitespace (the outermost
// gap has no following construct, and a Svelte `<style>` host trims the island's tail before
// writing it), and a run inside a COMMENT-BEARING property→colon gap, where the property name
// is reconstructed from its parts and the gap's whitespace normalizes with it. Both are
// ratcheted in
// [`tests/css_boundary_whitespace.rs`](../../../../tests/css_boundary_whitespace.rs).

use super::Printer;
use crate::ast::internal::CssBlockChild;
use tsv_lang::Span;

impl<'a> Printer<'a> {
    /// Every boundary member of the gap that ends at `anchor`, in source order: the ones a
    /// comment or a symbol strands earlier in the gap, then the contiguous run against the
    /// anchor itself.
    ///
    /// The single answer both complex-selector builders give at every anchor, so that the
    /// comment-bearing path cannot quietly restore less than its twin — the two are emitters
    /// of one run, and a claim on only one of them is the bug spelled once. In a comment-free
    /// gap the first half is always empty (nothing but whitespace stands between the floor and
    /// the run), which is why this reads as a no-op there and as the fix on the other path.
    ///
    /// ⚠️ `floor` is `None` where nothing printed so far bounds the gap — a complex selector's
    /// FIRST compound, whose leading run sits outside its own span. The backward scan still
    /// reaches the contiguous run there, but a forward sweep with no floor would rake in every
    /// member earlier in the document, so it is not attempted: the enclosing container claims
    /// that side instead, from the open delimiter it alone knows (`build_pseudo_args_doc`'s
    /// lead gap). At the stylesheet level there is no such container, which is where the
    /// block-level residue lives — see
    /// [`tests/css_boundary_whitespace.rs`](../../../../tests/css_boundary_whitespace.rs).
    pub(super) fn gap_boundary_ws(&self, floor: Option<u32>, anchor: u32) -> String {
        let mut out = match floor {
            Some(floor) => self.boundary_ws_in_gap_before_anchor(floor, anchor),
            None => String::new(),
        };
        out.push_str(self.preserved_boundary_ws(floor.unwrap_or(0), anchor));
        out
    }

    /// Every boundary-whitespace member the parser skipped in the gap `[from, to)`, in
    /// source order, with comment interiors stepped over.
    ///
    /// The counterpart of [`Self::preserved_boundary_ws`] for a gap that ends at a
    /// STRUCTURAL token rather than at a name — a selector list's `,`, a rule's `{`, a
    /// pseudo-argument's `)`. Those have no node to anchor a backward scan on, and the gap
    /// may hold comments the printer emits separately, so the run is collected forward and
    /// re-emitted flush against the terminator. Flush is what keeps it safe: a member left
    /// against the *name* side would glue to it (`0%<NBSP>` reads as one identifier), where
    /// against a `,`/`{`/`)` it can only re-parse as the same skipped run.
    ///
    /// Returns an owned `String` because the members need not be contiguous in source; the
    /// common no-run case returns an empty one and costs a scan of a gap that is a handful
    /// of bytes.
    pub(super) fn boundary_ws_in_gap(&self, from: u32, to: u32) -> String {
        let mut out = String::new();
        if !self.gap_may_hold_boundary_ws(from, to) {
            return out;
        }
        let (from, to) = (from as usize, to as usize);
        let bytes = self.source.as_bytes();
        let mut i = from;
        while i < to {
            if crate::comments::is_comment_start(bytes, i) {
                i = crate::comments::comment_end(bytes, i).min(to);
                continue;
            }
            let Some(c) = self.source[i..].chars().next() else {
                break;
            };
            // A byte-order mark is excluded here for the same reason `boundary_run`
            // excludes it: `U+FEFF` is in JS `\s`, so it reaches this walk like any other
            // member, and tsv strips BOMs by policy. Anchored at offset 0, which is what
            // makes one a BOM — anywhere else it is an ordinary character and is kept.
            let is_bom = i == 0 && c == '\u{feff}';
            if !is_bom && crate::whitespace::is_boundary_only_whitespace(c) {
                out.push(c);
            }
            i += c.len_utf8();
        }
        out
    }

    /// [`Self::boundary_ws_in_gap`] over the part of a gap its far ANCHOR will not claim.
    ///
    /// The two restores partition every gap that ends at a name: the anchor's own
    /// [`Self::preserved_boundary_ws`] claims the contiguous run directly behind it, and this
    /// claims everything before that — the members a comment, a combinator symbol or an ASCII
    /// gap separates from the anchor, which a backward scan stops short of and drops
    /// (`a <NBSP>/* c */ b`). Bounding at [`Self::boundary_run`]'s offset rather than at the
    /// anchor is what keeps the two from both claiming the same character.
    pub(super) fn boundary_ws_in_gap_before_anchor(&self, floor: u32, anchor: u32) -> String {
        self.boundary_ws_in_gap(floor, self.boundary_run(floor, anchor).0)
    }

    /// Whether `[from, to)` is a well-formed, non-empty gap that could hold a member of the
    /// boundary class at all.
    ///
    /// Every member is non-ASCII by construction (`is_boundary_only_whitespace` is JS `\s`
    /// MINUS its ASCII members), so an all-ASCII gap — which is every gap in every real
    /// stylesheet — settles here, before any comment walk or UTF-8 decode. This is asked once
    /// per comma, per rule brace, per combinator and per attribute-selector gap, so the guard
    /// is what keeps a whole-corpus cost off a case no corpus contains.
    ///
    /// It is also the bounds check for the slices behind it: the offsets reaching these
    /// helpers are span arithmetic (`span.end - 1`, `m_start + text.len()`), and one that
    /// lands mid-character used to reach a `str` slice and PANIC. The `debug_assert` keeps
    /// that an upstream bug rather than a crash, and the release path declines instead.
    pub(super) fn gap_may_hold_boundary_ws(&self, from: u32, to: u32) -> bool {
        let (from, to) = (from as usize, to as usize);
        if from >= to || to > self.source.len() {
            return false;
        }
        debug_assert!(
            self.source.is_char_boundary(from) && self.source.is_char_boundary(to),
            "boundary-whitespace gap bounds must be char boundaries"
        );
        if !self.source.is_char_boundary(from) || !self.source.is_char_boundary(to) {
            return false;
        }
        !self.source.as_bytes()[from..to].is_ascii()
    }

    /// [`Self::boundary_ws_in_gap`] over the gap that ends at a construct's CLOSING delimiter.
    ///
    /// Exists so the boundary claim states the bound as a name rather than as arithmetic; the
    /// arithmetic itself is [`closer_pos`], which every reader of that position shares.
    pub(super) fn boundary_ws_before_closer(&self, from: u32, span: Span) -> String {
        self.boundary_ws_in_gap(from, closer_pos(span))
    }

    /// The tail of the boundary whitespace run before `start` that this printer must emit
    /// verbatim: everything from its first **non-ASCII** member on.
    ///
    /// The parser skipped the whole run because `parseCss` skips it (JS `\s` at every
    /// `allow_whitespace()` juncture — see `tsv_lang::is_js_whitespace`), but that is a
    /// statement about the AST, not a licence to drop source. Deleting one of those
    /// characters is a content difference the corpus SAFETY check reads as `content_lost` —
    /// its semantic-character count excludes only ASCII whitespace, so a `<NBSP>` or `<LS>` is
    /// a character like any other. Same call the escaped-selector names make: the AST mirrors
    /// Svelte, the printer emits the author's bytes.
    ///
    /// ⚠️ The run is scanned back over **both** classes and emitted from the first non-ASCII
    /// member — not scanned back over the non-ASCII class alone. The two differ exactly on a
    /// mixed run, and only the first is prettier's answer: `<NBSP><SP>div` keeps `<NBSP><SP>`
    /// there, where a non-ASCII-only scan stops on the space, finds nothing, and DELETES the
    /// `<NBSP>`. What the ASCII *head* of the run contributes is indentation, which the
    /// printer regenerates — so it is dropped, while an interior one rides along inside the
    /// slice as the author spelled it. prettier respells that interior run as a single space
    /// (`<NBSP><TAB><NBSP>div` → `<NBSP><SP><NBSP>div`); tsv keeps the bytes, which is a
    /// whitespace-SPELLING difference with no content at stake, in a run no authored
    /// stylesheet contains. Pinned in `tests/css_boundary_whitespace.rs`.
    /// ⚠️ The scan takes a FLOOR — the end of the node printed just before this anchor — and
    /// there is no unfloored spelling on purpose.
    ///
    /// It is load-bearing wherever the previous node's own span can end **inside** the run,
    /// which is exactly what a boundary run glued to an identifier does: `read_identifier`
    /// takes `a<NBSP>` as one name, so the printer emits the `<NBSP>` as part of the name and
    /// an unbounded scan back from the next anchor finds it again and emits it TWICE
    /// (`a<NBSP>> b` → `a<NBSP><NBSP> > b`). The floor makes each character belong to exactly
    /// one emitter, the same partition the element-comma seam keeps — and BOTH complex-selector
    /// builders need it, since a comment anywhere in the selector routes to the other one.
    /// Pass `0` only where the run genuinely precedes everything printed so far: a complex
    /// selector's first compound, whose leading run sits *before* `complex.span.start`.
    pub(super) fn preserved_boundary_ws(&self, floor: u32, start: u32) -> &'a str {
        let (run_start, holds_member) = self.boundary_run(floor, start);
        // The common answer, settled by the scan that just ran rather than by a second pass:
        // every member of this class is non-ASCII, so an all-ASCII run carries nothing to
        // preserve. This is asked once per selector, per combinator, per at-rule, per comment
        // and per DECLARATION — the densest construct in a stylesheet — so the empty answer
        // has to cost one branch.
        if !holds_member {
            return "";
        }
        let (run_start, start) = (run_start as usize, start as usize);
        let kept = self.source[run_start..start]
            .char_indices()
            .find(|(_, c)| crate::whitespace::is_boundary_only_whitespace(*c))
            .map_or(start, |(i, _)| run_start + i);
        &self.source[kept..start]
    }

    /// The contiguous boundary-whitespace run ending at `anchor`: where it begins (floored),
    /// and whether it holds a member of this class at all.
    ///
    /// Two answers from one scan, because the two callers want opposite halves of it and
    /// neither should pay for a second pass:
    ///
    /// - the offset is the **partition point** every forward collector over the same gap
    ///   needs — what it excludes is exactly what the backward scan is about to claim, so
    ///   [`Self::boundary_ws_in_gap_before_anchor`] can sweep the rest of the gap without
    ///   either emitter printing a character twice;
    /// - the flag is [`Self::preserved_boundary_ws`]'s early-out, and it is what keeps a claim
    ///   asked per DECLARATION down to a few byte compares on the path every real stylesheet
    ///   takes.
    ///
    /// The flag may over-report by exactly one case — a run whose only member is the
    /// document's leading BOM, which the offset then steps past — so it is a fast *gate*, not
    /// a fact: the caller still asks what the run actually holds.
    pub(super) fn boundary_run(&self, floor: u32, anchor: u32) -> (u32, bool) {
        let floor = floor as usize;
        let mut run_start = anchor as usize;
        // Every member of the class is non-ASCII, so a purely ASCII run behind the anchor
        // carries nothing to preserve. Step it with byte compares and settle there — this is
        // asked once per selector, per combinator, per at-rule and per DECLARATION, and the
        // backward `chars()` decode below is far more than a byte compare. The set is the six
        // ASCII members of `is_boundary_whitespace` (JS `\s` ∪ `White_Space`), the vertical
        // tab included — `u8::is_ascii_whitespace` alone stops on one and hides the member
        // behind it.
        let bytes = self.source.as_bytes();
        while run_start > floor {
            let b = bytes[run_start - 1];
            if !b.is_ascii() {
                break;
            }
            if !(b.is_ascii_whitespace() || b == b'\x0b') {
                return (run_start as u32, false);
            }
            run_start -= 1;
        }
        // Only this half can meet a member of the class — the fast loop above stops on the
        // first non-ASCII byte, whether or not it turns out to be whitespace.
        let mut holds_member = false;
        while run_start > floor {
            let prev = self.source[..run_start]
                .chars()
                .next_back()
                .filter(|c| crate::whitespace::is_boundary_whitespace(*c));
            match prev {
                Some(c) => {
                    holds_member |= !c.is_ascii();
                    run_start -= c.len_utf8();
                }
                None => break,
            }
        }
        // ⚠️ A leading BOM is NOT preserved. `U+FEFF` is in JS `\s`, so a byte-order mark
        // reaches this scan like any other member — but tsv strips BOMs by policy (a
        // cataloged prettier divergence, `docs/conformance_prettier.md` §Whitespace: BOM
        // Handling), and re-emitting it here would quietly undo that. The exclusion is
        // anchored at offset 0 because that is what makes a `U+FEFF` a byte-order mark; one
        // anywhere else is an ordinary character and is preserved with the rest — including a
        // second one later in this same run, which the forward scan in `preserved_boundary_ws`
        // still reaches.
        if run_start == 0 && self.source.starts_with('\u{feff}') {
            run_start = '\u{feff}'.len_utf8();
        }
        (run_start as u32, holds_member)
    }

    /// The boundary run of a gap whose PREVIOUS token is a bare name, with the ASCII space
    /// that keeps the name's end where the author put it — the attribute selector's
    /// `[name<HERE>]`.
    ///
    /// A thin spelling of the rule in this module's §Where a claim is emitted: build the run,
    /// then ask [`name_run_separator_after`] of what it is being appended to. The gap this
    /// serves always has a name on its left, so the separator is always the space — asking
    /// anyway is what keeps one rule with one implementation rather than a second literal.
    pub(super) fn push_name_tail_boundary_ws(&self, result: &mut String, from: u32, to: u32) {
        let kept = self.boundary_ws_in_gap(from, to);
        self.push_boundary_ws_after_name(result, &kept);
    }

    /// Append `kept` to `result` behind the separator its left neighbour requires.
    ///
    /// The one appender for every claim that builds a rebuilt construct's text — the
    /// attribute selector's interior gaps and its tail — so none of them can restore the run
    /// and still move the name it sits behind. A no-op on an empty run.
    pub(super) fn push_boundary_ws_after_name(&self, result: &mut String, kept: &str) {
        if kept.is_empty() {
            return;
        }
        let separator = name_run_separator_after(result);
        result.push_str(separator);
        result.push_str(kept);
    }

    /// The ASCII space a boundary run resumed at `at` needs, or `""`.
    ///
    /// The SOURCE-position face of [`name_run_separator_after`], for the doc-builder claims
    /// that have no accumulated string to look at — a selector list's `,`, an explicit
    /// combinator's symbol, a pseudo-argument list's `)`. `at` is the offset the claim
    /// resumes from (the previous node's `end`), which is where the printer's own last
    /// character came from at every one of those seams: nothing between a compound's end and
    /// the gap is rewritten, so the source byte and the emitted one are the same character.
    /// A claim over a REBUILT construct must ask the other face instead, since there they
    /// are not.
    pub(super) fn name_run_separator(&self, at: u32) -> &'static str {
        let at = at as usize;
        if at == 0 || at > self.source.len() || !self.source.is_char_boundary(at) {
            return "";
        }
        name_run_separator_after(&self.source[..at])
    }

    /// Emit the boundary run the parser skipped ahead of a node whose head this printer
    /// REBUILDS — a declaration's property, an at-rule's `@`, a comment's `/*`.
    ///
    /// Each of those is a block-child (or stylesheet-child) `allow_comment_or_whitespace`
    /// juncture, and the rebuilt head regenerates the gap, so the run has nothing else to
    /// ride out on. A **rule** child is the exception that needs no call: its selector's first
    /// compound already claims the same run through `preserved_boundary_ws`, and a second
    /// claim here would print it twice.
    ///
    /// Unfloored on purpose — a backward scan settles on the first non-whitespace byte, which
    /// at every one of these positions is the `{`, the previous child's `;` / `}` / `*/`, or
    /// the `<style>`'s `>`. Emitted after the indent this printer regenerates, flush against
    /// the head.
    pub(crate) fn write_head_boundary_ws(&mut self, start: u32) {
        let kept = self.preserved_boundary_ws(0, start);
        if !kept.is_empty() {
            self.write(kept);
        }
    }

    /// Emit the boundary run a block's own TAIL juncture skipped, flush against the `}`
    /// (`a { color: red;<NBSP>}`).
    ///
    /// The block-child loop asks `allow_comment_or_whitespace` once more after its last
    /// child, and that run has no child to ride out on — the `}` is the only thing left in
    /// the gap, and both block emitters regenerate it. Floored on the last child's end so the
    /// sweep can never reach into a declaration's own value, where the same code point is
    /// CONTENT (`color: a<NBSP>b`); with no child at all the floor is the `{`.
    pub(crate) fn write_block_tail_boundary_ws(
        &mut self,
        children: &[CssBlockChild<'_>],
        block_span: Span,
    ) {
        let from = children
            .last()
            .map_or(block_span.start, |child| child.span().end);
        let kept = self.boundary_ws_in_gap(from, closer_pos(block_span));
        if !kept.is_empty() {
            self.write(&kept);
        }
    }
}

/// The ASCII space a boundary run appended to `before` needs, or `""`.
///
/// The one answer to "would this run glue backwards?", asked of the text a claim is about to
/// extend. It is the whole of the second half of this module's §Where a claim is emitted: a
/// run against a `,`, a `{`, a `)`, a quote or a combinator symbol can only re-parse as the
/// run the parser skipped, while one against a NAME becomes part of it — so the test is
/// exactly the lexer's identifier-continuation predicate, read backwards over the last
/// character.
///
/// ⚠️ The lexer's, not a re-spelling: `IDENT_CONTINUE_LUT` for ASCII and
/// `is_non_ascii_identifier_codepoint` above it, which is what `read_identifier` will do to
/// this seam on the next parse. A narrower class here silently re-opens the glue on whatever
/// it stops short of.
pub(super) fn name_run_separator_after(before: &str) -> &'static str {
    match before.chars().next_back() {
        Some(c) if continues_identifier(c) => " ",
        _ => "",
    }
}

/// `separator` ahead of `kept`, or `kept` untouched when there is no run to separate.
///
/// The doc-builder counterpart of [`Printer::push_boundary_ws_after_name`]: those claims hand
/// their run to `text_pooled` rather than to a `String` they own, so the separator has to ride
/// inside the same value. Keeping the empty case allocation-free is what lets every claim ask
/// unconditionally.
pub(super) fn prefixed_run(separator: &'static str, kept: String) -> String {
    if kept.is_empty() || separator.is_empty() {
        return kept;
    }
    let mut out = String::with_capacity(separator.len() + kept.len());
    out.push_str(separator);
    out.push_str(&kept);
    out
}

/// The byte offset of the CLOSING delimiter a bracketed construct's `span` ends one past —
/// a pseudo-argument list's `)`, an attribute selector's `]`, a block's `}`.
///
/// Named because five readers want it and the `- 1` is not self-evident at any of them: the
/// gap those constructs skipped a boundary run in stops a byte short of `span.end`, and an
/// off-by-one there either swallows the closer into the run or misses the last member. It
/// saturates because one caller used to subtract raw — a shape that is unreachable today
/// (something always precedes the closer) and is a panic rather than a wrong answer the day
/// it isn't.
pub(super) fn closer_pos(span: Span) -> u32 {
    span.end.saturating_sub(1)
}

/// Whether `c` continues a CSS identifier — the single predicate behind both askers.
fn continues_identifier(c: char) -> bool {
    if c.is_ascii() {
        crate::lexer::IDENT_CONTINUE_LUT[c as usize]
    } else {
        crate::lexer::is_non_ascii_identifier_codepoint(c)
    }
}

/// The first position in `[from, to)` that begins the region's next real token — the
/// [`crate::comments::skip_trivia_forward`] scan in the printer's `u32` span
/// coordinates.
///
/// Two constructs are rebuilt from parts the printer has no span for, and both locate them by
/// stepping the trivia ahead: the attribute selector (the `|`, the matcher, the case flag)
/// and a condition prelude's part (whose own span opens on whatever token the lexer produced,
/// so its content may start past it). Comment-aware by construction, so no scan can read a
/// comment's content as structure.
///
/// ⚠️ **Boundary whitespace is trivia to this scan**, because it is trivia to the parser that
/// produced the spans it navigates between: every gap here is one the attribute selector's
/// `skip_boundary_whitespace_registering_comments` stepped a run at. A scan whose class is
/// narrower than the skip it retraces stops ON the run and reports it as the next part's
/// start — which put the case flag's separator on the wrong side of the run (emitting
/// `[a='b' i<NBSP>]`, which tsv's own parser then rejects) and, worse, made
/// `m_start + text.len()` an offset INSIDE a multi-byte character, where the next slice
/// panicked. Same rule as the parser's lookaheads: a class narrower than the skip it predicts
/// is the same defect as one wider than the lexer's.
pub(super) fn skip_gap_trivia(source: &str, from: u32, to: u32) -> u32 {
    debug_assert!(from <= to, "attribute-gap scan bounds inverted");
    debug_assert!(
        source.is_char_boundary(from as usize) && source.is_char_boundary(to as usize),
        "attribute-gap scan bounds must be char boundaries"
    );
    let to = to as usize;
    let mut i = from as usize;
    loop {
        i = crate::comments::skip_trivia_forward(source.as_bytes(), i, to);
        match source[i..to].chars().next() {
            Some(c) if crate::whitespace::is_boundary_only_whitespace(c) => i += c.len_utf8(),
            _ => return i as u32,
        }
    }
}
