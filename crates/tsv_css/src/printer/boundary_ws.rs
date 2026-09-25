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
// - [`Printer::spell_gap_items`] sweeps FORWARD over a whole gap, printing its members AND its
//   comments. A backward scan alone stops at a `*/` and drops whatever a comment strands
//   behind it (`a <NBSP>/* c */ b`).
//
// A claim that sweeps a gap forward owns the gap's comments too, wherever the gap holds a
// member ([`Printer::gap_holds_member`]), and the site's own comment seam then stands down
// for that gap; a member-free gap keeps the site's comment spelling. [`Printer::claim_lead_gap`]
// lets a container (a list's comma, a pseudo-argument's `(`, an `of`) claim a selector's lead
// gap whole and stand the selector's own backward scan down ([`Printer::claimed_lead`]), so
// the two never print one character twice. [`Printer::gap_boundary_ws`] is the answer every
// emitter of a gap ending at a compound's first simple selector uses.
//
// ## How a claim is spelled
//
// Every claim emits its gap through one speller (`spell_run`), which reads the gap as a
// SEQUENCE OF ITEMS — the members, and the comments where the claim owns them — and prints
// them in source order, in place: between two items, and at each edge, the author's ASCII
// whitespace is ONE space (a tab, a line break or a double space alike) and its absence is
// none. Three readers make that the rule. `parseCss` skips the whole run, so nothing about
// the AST depends on it. css-syntax-3, whose whitespace is ASCII only, reads every member as
// identifier content and a comment as nothing at all, so the spelling IS the tokenization:
// `<NBSP><NBSP>` is one identifier where `<NBSP> <NBSP>` is two, `a<ZWNBSP>` one where
// `a <ZWNBSP>` is a type selector and a descendant, and `a <NBSP>/* c */:hover` holds the
// compound `<NBSP>:hover` where `a /* c */ <NBSP> :hover` is a descendant — a comment moved to
// the other side of a member, or spaced off one it was glued to, changes what a browser
// reads. And the doc renderer: a line break carried inside the text the printer emits is one
// it neither indents after nor counts, so the anchor lands at column 0, and under a host that
// re-indents every line of the formatted sheet (a nested `<style>`) the next pass reads that
// indentation back as part of the run and grows it by a level. It is prettier's answer at
// every juncture where prettier keeps the run.
//
// ⚠️ The rule holds at the CLAIMED junctures — the table below. An at-rule prelude the parser
// cannot structure is printed verbatim (`@layer`, `@page`, `@keyframes`' name, a condition
// prelude whose head holds ASCII after a member), and its raw text keeps whatever the author
// wrote, tabs and line breaks included; that is the raw-prelude path's, not a claim's.
//
// What varies per claim is only the two EDGES, and the claim says which ([`Edge`]):
//
// - **[`Edge::Flush`]** where the printer regenerates what stands there — a line's
//   indentation ahead of a block child's head, the space a combinator's separator doc or a
//   `::part()` join writes, the ` {` after a rule's head — or where it is a delimiter the run
//   may touch: the `(` / `[` it opens after, the `,` / `)` / `]` / `}` it closes against.
// - **[`Edge::Presence`]** beside a NAME or a TERM: one space if the author separated the
//   gap's outermost item from it, none if they glued it. `a <ZWNBSP>{` stays `a <ZWNBSP> {`,
//   `2n <NBSP>)` stays spaced, and `2n<NBSP>)` does not grow a space the author never wrote.
// - **[`Edge::Space`]** where a MEMBER is about to be emitted after a NAME the printer cannot
//   see the author's separation from (`read_identifier` takes every code point at or above
//   U+00A0 as content, so the run would glue into it — `[a <NBSP>]` came back with the name
//   `a<NBSP>`, an AST that changed under a format and its own fixed point, which no
//   idempotency, reparse or content-count gate sees). A gap that opens on a comment takes the
//   author's separation instead, since no name can take a comment in. [`Printer::name_run_edge`]
//   (from a source position) and [`name_run_separator_after`] (from built text) ask the
//   question. ⚠️ That is **not** one gap: the attribute selector's `[name<HERE>]`, a selector
//   list's `,` (`a.x <NBSP>, c`), an explicit combinator's symbol (`a <NBSP>> b`), a
//   pseudo-argument list's `)` (`:is(a <NBSP>)`), the commented attribute rebuild's every
//   interior gap, a rule's pre-`{` gap, and a condition prelude's part head behind a
//   CONNECTOR (`@supports and <NBSP>(a: b)` — the connector is an identifier like any other)
//   all have a name on their left, and each was a live glue. A claim added anywhere new owes
//   the same question. The condition one answers it structurally rather than by asking:
//   `build_condition_query_doc`'s head pieces are joined and *followed* by one space, so the
//   run can never land flush against the last of them; its tell is not a changed name in the
//   wire but a prelude that stops reading as a condition at all
//   (`a_connector_never_absorbs_the_run_behind_it`).
//
// Two emitters spell a gap's items without a claim, each for its own construct: the An+B
// normalizer (`Printer::normalize_an_plus_b`), which folds the runs around an operator into
// the term's own text (`2n<NBSP> + 1`) and keeps a comment glued to a member glued, and the
// attribute selector's tail (`Printer::push_attribute_tail`), whose flag is one more item.
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
// | a selector list's `,`, and the gap after it | `pre_comma_boundary_ws`, and `build_comma_list_doc` through `claim_lead_gap` |
// | a rule's pre-`{` gap, its comments included | `spell_gap_items` (`printer/rules.rs`) |
// | a pseudo-argument list's lead and its `)` | `build_pseudo_args_doc` (the lead through `claim_lead_gap`) |
// | an `:nth-*()` term's lead, its `)`, both sides of its `of`, the `)` after `S` | `build_pseudo_args_doc`'s `Nth` arm (`S`'s lead through `claim_lead_gap`) |
// | `::part()`'s inter-name gaps | `build_part_idents_doc` |
// | every attribute-selector interior gap, and its tail | `push_attribute_gap` / `push_attribute_tail` |
// | a declaration's property→colon gap | `extract_property_name`, through [`boundary_run_spelling`] |
// | a block child's rebuilt head (declaration / at-rule / comment) | [`Printer::write_head_boundary_ws`] |
// | a block's tail before `}` | [`Printer::write_block_tail_boundary_ws`] |
// | a condition prelude's part heads (behind the name, a connector, or nothing) | `Printer::condition_part_head_ws` |
//
// A **rule** child needs no head claim: its selector's first compound already claims the same
// run, and a second claim would print it twice.
//
// ⚠️ [`skip_gap_trivia`] belongs to this module for the same reason: a printer scan that
// LOCATES a part by stepping trivia is retracing one of these skips, so its class must be the
// skip's. One that is narrower reports the RUN as the part's start.
//
// ## The document precondition
//
// Every claim above is a per-site question — asked once per selector, per combinator, per
// at-rule, per comment, per selector-list comma and per DECLARATION — and on real code every
// one of them answers "nothing". [`source_holds_boundary_ws`] settles that for the whole
// document ONCE, and [`Printer::holds_boundary_ws`] carries the answer: a source with no
// member of the class anywhere cannot have one inside any gap of it, so the three claim entry
// points ([`Printer::gap_boundary_ws`], [`Printer::preserved_boundary_ws`] and
// [`Printer::gap_may_hold_boundary_ws`], which every other claim routes through) return their
// empty answer before scanning anything. It is a strict SUPERSET of what any claim can read —
// the whole source, not a region — so it can only turn a claim off where the claim would have
// come back empty on its own, and the two claim shapes stay the sole deciders of what the run
// actually is.
//
// ## Where the bytes are the claim
//
// Two readers disagree about a run in a way no spacing can reconcile: to css-syntax-3 the
// run is identifier content and the ASCII space beside it the token separator, to `parseCss`
// the run is the separator. An attribute selector's tail and a declaration's property→colon
// gap are where that bites — `[a=b<NBSP>]` is the value `b` to Svelte and the ident `b<NBSP>`
// to a browser — so those two claims keep the author's bytes: a bare value glued to a run
// stays bare and unquoted, a flag glued to a run stays glued, and ASCII whitespace beside a
// run keeps its presence as one space ([`Printer::push_attribute_tail`],
// [`boundary_run_spelling`]). Re-quoting the value or hoisting the run would emit a selector
// the browser drops where it matched the input.
//
// ## Residue
//
// Known positions that drop the run: the stylesheet's own trailing whitespace (the outermost
// gap has no following construct, and a Svelte `<style>` host trims the island's tail before
// writing it), ratcheted in
// [`tests/css_boundary_whitespace.rs`](../../../../tests/css_boundary_whitespace.rs); and the
// at-rule preludes whose selector list the printer structures with no claim in the table
// above (`@scope`, `@custom-selector`), where the parser steps a run no emitter puts back.

use super::{Printer, SourceRole};
use crate::ast::internal::CssBlockChild;
use tsv_lang::Span;

/// How a spelled boundary gap meets the output on one side of it.
///
/// The gap's own interior is not a choice: its items (members, and comments where the claim
/// prints them) stay in source order, and each ASCII whitespace stretch between two is one
/// space, whoever claims it. What differs per claim is only what stands BESIDE the gap,
/// which the claim knows and this module does not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Edge {
    /// Nothing on this side: the printer regenerates what stands there (a line's indentation,
    /// a combinator's separator, the ` {` after a rule's head), or it is a delimiter the run
    /// may touch (the `(` it opens after, the `,` / `)` / `]` / `}` it closes against).
    Flush,
    /// One space if the author put ASCII whitespace between the gap's outermost item and this
    /// side, and nothing if they glued it. The answer beside a NAME or a TERM, where the
    /// author's presence is the claim: `a <ZWNBSP>{` must not come back as `a<ZWNBSP> {`,
    /// and `2n<NBSP>)` must not grow a space the author never wrote.
    Presence,
    /// One space whatever the author wrote: a name the run is about to be emitted after
    /// would otherwise take it in (`read_identifier` treats every code point at or above
    /// U+00A0 as content) — see [`Printer::name_run_edge`].
    Space,
}

impl<'a> Printer<'a> {
    /// Every boundary member of the gap that ends at `anchor`, spelled ([`spell_run`]) flush
    /// on its left — whatever stood there, the printer regenerates it — and with the author's
    /// separation kept against the anchor on its right.
    ///
    /// The single answer both complex-selector builders give at every anchor, so that the
    /// comment-bearing path cannot quietly restore less than its twin — the two are emitters
    /// of one run, and a claim on only one of them is the bug spelled once. Floored, the gap
    /// is claimed whole with its comments in place ([`Self::spell_gap_items`]), so the
    /// comment-bearing builder's separator leaves them alone wherever the gap holds a member.
    ///
    /// ⚠️ `floor` is `None` where nothing printed so far bounds the gap — a complex selector's
    /// FIRST compound, whose leading run sits outside its own span. The backward scan still
    /// reaches the contiguous run there, but a forward sweep with no floor would rake in every
    /// member earlier in the document, so it is not attempted: the enclosing container claims
    /// that side instead, from the open delimiter it alone knows (`build_pseudo_args_doc`'s
    /// lead gap). At the stylesheet level there is no such container, which is where the
    /// block-level residue lives — see
    /// [`tests/css_boundary_whitespace.rs`](../../../../tests/css_boundary_whitespace.rs).
    ///
    /// `#[inline]` for the gate alone — the scan stays out of line. Every claim is asked far
    /// more often than it answers, so what the caller needs folded in is the branch, not the
    /// work behind it: with the test at the call site the empty answer costs no call, no
    /// `String` to construct and drop, and no `is_empty` test the caller cannot see through.
    #[inline]
    pub(super) fn gap_boundary_ws(&self, floor: Option<u32>, anchor: u32) -> String {
        if !self.holds_boundary_ws {
            return String::new();
        }
        match floor {
            Some(floor) => self.spell_gap_items(floor, anchor, Edge::Flush, Edge::Presence),
            None => self.preserved_boundary_ws(0, anchor),
        }
    }

    /// Every boundary-whitespace member the parser skipped in the gap `[from, to)`, spelled
    /// by [`spell_run`] with `lead` and `trail` deciding the two edges.
    ///
    /// The forward sweep: for a gap whose far end is a STRUCTURAL token (a selector list's
    /// `,`, a rule's `{`, a pseudo-argument's `)`), or any gap a claim can bound on both
    /// sides. The gap may hold comments the printer emits separately, so they are stepped —
    /// and count as separation, since the members either side of one are two tokens to every
    /// reader. The run is re-emitted flush against a delimiter it can only re-parse beside as
    /// the run the parser skipped; against a NAME it keeps the author's separation, or it
    /// glues into the name (`0%<NBSP>` reads as one identifier).
    ///
    /// Returns an owned `String` because the spelling need not be contiguous in source; the
    /// common no-run case returns an empty one and costs a scan of a gap that is a handful
    /// of bytes.
    ///
    /// Gated inline, scanned out of line — see [`Self::gap_boundary_ws`] for why the split is
    /// where the per-site cost of this family actually lives.
    #[inline]
    pub(super) fn spell_gap(&self, from: u32, to: u32, lead: Edge, trail: Edge) -> String {
        if !self.gap_may_hold_boundary_ws(from, to) {
            return String::new();
        }
        spell_run(
            self.source,
            from as usize,
            to as usize,
            self.source_role == SourceRole::Document,
            (lead, trail),
            Comments::Elsewhere,
        )
    }

    /// [`Self::spell_gap`] for a gap whose COMMENTS this claim prints too, in place among the
    /// members: every item where the author put it, each ASCII stretch between two as one
    /// space ([`spell_run`]).
    ///
    /// The answer wherever a gap holds both a member and a comment and one emitter can take
    /// the whole gap. Printed apart — the comments by the site's comment seam, the run by its
    /// claim — the two land in whatever order the two emitters run, and a comment moved to
    /// the other side of a member changes what a browser reads: `a <NBSP>/* c */:hover` is
    /// the compound `<NBSP>:hover`, and `a /* c */ <NBSP> :hover` a descendant. A caller
    /// takes this path only where [`Self::gap_holds_member`] says so, and must then leave the
    /// gap's comments to it; a member-free gap keeps the site's own comment spelling.
    pub(super) fn spell_gap_items(&self, from: u32, to: u32, lead: Edge, trail: Edge) -> String {
        // The member test, not the fast gate: a gap whose only non-ASCII is inside a comment
        // spells to nothing here, so its comment stays with the site's own seam.
        if !self.gap_holds_member(from, to) {
            return String::new();
        }
        spell_run(
            self.source,
            from as usize,
            to as usize,
            self.source_role == SourceRole::Document,
            (lead, trail),
            Comments::InPlace,
        )
    }

    /// Whether `[from, to)` holds a boundary MEMBER outside its comments — the test that hands
    /// a gap's comments to [`Self::spell_gap_items`]. Exact where
    /// [`Self::gap_may_hold_boundary_ws`] is a fast gate: a non-ASCII character inside a
    /// comment (`/* é */`) is not a member, and a gap holding only that keeps its comments
    /// where the site's own seam prints them.
    pub(super) fn gap_holds_member(&self, from: u32, to: u32) -> bool {
        if !self.gap_may_hold_boundary_ws(from, to) {
            return false;
        }
        let bytes = self.source.as_bytes();
        let (mut i, to) = (from as usize, to as usize);
        while i < to {
            if crate::comments::is_comment_start(bytes, i) {
                i = crate::comments::comment_end(bytes, i);
                continue;
            }
            let Some(c) = self.source[i..].chars().next() else {
                break;
            };
            let is_bom = i == 0 && c == tsv_lang::BOM && self.source_role == SourceRole::Document;
            if !is_bom && crate::whitespace::is_boundary_only_whitespace(c) {
                return true;
            }
            i += c.len_utf8();
        }
        false
    }

    /// Claim the LEAD gap of the selector that begins at `anchor` — the gap `[from, anchor)`
    /// between a container's opener (a list's comma, a pseudo-argument's `(`, an `of`) and
    /// the selector's first anchor — whole and in place, if it holds a member; `None` when
    /// it does not, and the container's own comment spelling stands.
    ///
    /// The selector's first anchor has no floor of its own (its builder is not told what
    /// opened it), so on its own it claims only the run CONTIGUOUS with it — which stops at a
    /// comment and leaves every member ahead of it to nobody (`a, <NBSP>/* c */ b` lost the
    /// `<NBSP>`). The container knows the floor, so it takes the gap, and records the anchor
    /// in [`Printer::claimed_lead`] so the first anchor's own claim stands down; the record is
    /// keyed by position, so it can only ever silence the one anchor it names.
    pub(super) fn claim_lead_gap(&self, from: u32, anchor: u32, lead: Edge) -> Option<String> {
        if !self.gap_holds_member(from, anchor) {
            return None;
        }
        self.claimed_lead.set(Some(anchor));
        Some(self.spell_gap_items(from, anchor, lead, Edge::Presence))
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
    /// lands mid-character would reach a `str` slice and PANIC. The `debug_assert` keeps
    /// that an upstream bug rather than a crash, and the release path declines instead.
    #[inline]
    pub(super) fn gap_may_hold_boundary_ws(&self, from: u32, to: u32) -> bool {
        // The document precondition first: a source with no member anywhere has none in this
        // gap either, and the test is a field read where the one below is a scan of the gap.
        if !self.holds_boundary_ws {
            return false;
        }
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

    /// [`Self::spell_gap`] over the gap that ends at a construct's CLOSING delimiter, flush
    /// against it.
    ///
    /// Exists so the boundary claim states the bound as a name rather than as arithmetic; the
    /// arithmetic itself is [`closer_pos`], which every reader of that position shares.
    pub(super) fn spell_gap_before_closer(&self, from: u32, span: Span, lead: Edge) -> String {
        self.spell_gap_items(from, closer_pos(span), lead, Edge::Flush)
    }

    /// The boundary whitespace run contiguous with `start`, spelled ([`spell_run`]) flush on
    /// its left and with the author's separation kept against `start` on its right.
    ///
    /// The parser skipped the whole run because `parseCss` skips it (JS `\s` at every
    /// `allow_whitespace()` juncture — see `tsv_lang::is_js_whitespace`), but that is a
    /// statement about the AST, not a licence to drop source. Deleting one of those
    /// characters is a content difference the corpus SAFETY check reads as `content_lost` —
    /// its semantic-character count excludes only ASCII whitespace, so a `<NBSP>` or `<LS>` is
    /// a character like any other. Same call the escaped-selector names make: the AST mirrors
    /// Svelte, the printer emits the author's members.
    ///
    /// ⚠️ The run is scanned back over **both** classes — not over the non-ASCII class alone.
    /// The two differ exactly on a mixed run: `<NBSP><SP>div` holds a member behind the space,
    /// where a non-ASCII-only scan stops on the space, finds nothing, and DELETES the
    /// `<NBSP>`. The run's ASCII *head* is indentation, which the printer regenerates, so it
    /// is dropped ([`Edge::Flush`]); every ASCII stretch after the first member is spelled as
    /// one space — prettier's answer (`<NBSP><TAB><NBSP>div` → `<NBSP><SP><NBSP>div`,
    /// `<ZWNBSP>⏎div` → `<ZWNBSP><SP>div`), and the only one that survives the doc renderer: a
    /// line break carried inside the run's text is neither indented after nor counted, so it
    /// lands the anchor at column 0, and under a host that re-indents every line of the
    /// formatted sheet it grows a level on every pass.
    ///
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
    ///
    /// Gated inline, scanned out of line — see [`Self::gap_boundary_ws`] for why the split is
    /// where the per-site cost of this family actually lives.
    #[inline]
    pub(super) fn preserved_boundary_ws(&self, floor: u32, start: u32) -> String {
        self.contiguous_boundary_ws(floor, start, Edge::Presence)
    }

    /// [`Self::preserved_boundary_ws`] with the right-hand edge the caller's: flush where the
    /// printer emits its own separator AFTER the run (an explicit combinator's `line`), so
    /// the author's trailing space does not stack with the regenerated one.
    #[inline]
    pub(super) fn contiguous_boundary_ws(&self, floor: u32, start: u32, trail: Edge) -> String {
        if !self.holds_boundary_ws || (floor == 0 && self.claimed_lead.get() == Some(start)) {
            return String::new();
        }
        self.scan_contiguous_boundary_ws(floor, start, trail)
    }

    /// [`Self::contiguous_boundary_ws`]'s backward scan, past the document precondition.
    fn scan_contiguous_boundary_ws(&self, floor: u32, start: u32, trail: Edge) -> String {
        let (run_start, holds_member) = self.boundary_run(floor, start);
        // The common answer, settled by the scan that just ran rather than by a second pass:
        // every member of this class is non-ASCII, so an all-ASCII run carries nothing to
        // preserve. This is asked once per selector, per combinator, per at-rule, per comment
        // and per DECLARATION — the densest construct in a stylesheet — so the empty answer
        // has to cost one branch.
        if !holds_member {
            return String::new();
        }
        spell_run(
            self.source,
            run_start as usize,
            start as usize,
            self.source_role == SourceRole::Document,
            (Edge::Flush, trail),
            Comments::Elsewhere,
        )
    }

    /// The contiguous boundary-whitespace run ending at `anchor`: where it begins (floored),
    /// and whether it holds a member of this class at all.
    ///
    /// Two answers from one scan, because the two callers want opposite halves of it and
    /// neither should pay for a second pass:
    ///
    /// - the offset is the **partition point** between a backward scan and a forward sweep
    ///   over the same gap — what it excludes is exactly what the backward scan is about to
    ///   claim, so the two never print one character twice;
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
        // anchored at offset 0 of a whole document because that is what makes a `U+FEFF` a
        // byte-order mark; one anywhere else — a fragment's offset 0 included
        // ([`SourceRole::Fragment`]) — is an ordinary character and is preserved with the
        // rest, including a second one later in this same run, which the forward scan in
        // `preserved_boundary_ws` still reaches.
        if run_start == 0 && self.source_role == SourceRole::Document {
            run_start = tsv_lang::leading_bom_len(self.source);
        }
        (run_start as u32, holds_member)
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

    /// The left edge a boundary run resumed at `at` takes: [`Edge::Space`] where a NAME ends
    /// there and would take the run in, else [`Edge::Presence`] (the author's separation).
    ///
    /// The SOURCE-position face of [`name_run_separator_after`], for the doc-builder claims
    /// that have no accumulated string to look at — a selector list's `,`, an explicit
    /// combinator's symbol, a pseudo-argument list's `)`. `at` is the offset the claim
    /// resumes from (the previous node's `end`), which is where the printer's own last
    /// character came from at every one of those seams: nothing between a compound's end and
    /// the gap is rewritten, so the source byte and the emitted one are the same character.
    /// A claim over a REBUILT construct must ask the other face instead, since there they
    /// are not.
    pub(super) fn name_run_edge(&self, at: u32) -> Edge {
        let at = at as usize;
        if at == 0 || at > self.source.len() || !self.source.is_char_boundary(at) {
            return Edge::Presence;
        }
        if name_run_separator_after(&self.source[..at]).is_empty() {
            Edge::Presence
        } else {
            Edge::Space
        }
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
            self.write(&kept);
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
        let kept = self.spell_gap(from, closer_pos(block_span), Edge::Flush, Edge::Flush);
        if !kept.is_empty() {
            self.write(&kept);
        }
    }
}

/// The ASCII space a boundary run appended to `before` needs, or `""`.
///
/// The one answer to "would this run glue backwards?", asked of the text a claim is about to
/// extend — the [`Edge::Space`] question of this module's §How a claim is spelled: a run
/// against a `,`, a `{`, a `)`, a quote or a combinator symbol can only re-parse as the run
/// the parser skipped, while one against a NAME becomes part of it — so the test is exactly
/// the lexer's identifier-continuation predicate, read backwards over the last character.
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

/// A whitespace-only gap's boundary run, spelled with the author's ASCII whitespace
/// PRESENCE kept on its left and dropped on its right: [`spell_run`] with
/// [`Edge::Presence`] / [`Edge::Flush`]. Empty when the gap holds no member — which is every
/// gap in every real stylesheet — so a caller can append it unconditionally.
///
/// For a gap the printer REBUILDS between two parts whose separator it regenerates — a
/// declaration's property→colon gap, ahead of each comment in it and ahead of the colon —
/// where the run has to be put back and the ASCII beside it is not merely spelling: to
/// css-syntax-3 the run is identifier content and the space the token separator, so
/// `color<NBSP>:` is one ident and `color <NBSP>:` an ident and a stray token, while
/// `parseCss` reads the property `color` in both. Keeping the presence as one space is what
/// lets the output tokenize as the input did under both readers; the space AFTER the run is
/// the regenerated separator's to emit, which is why the trailing one is dropped.
pub(super) fn boundary_run_spelling(gap: &str) -> String {
    debug_assert!(
        gap.chars().all(crate::whitespace::is_boundary_whitespace),
        "boundary_run_spelling over a gap holding more than whitespace: {gap:?}"
    );
    spell_run(
        gap,
        0,
        gap.len(),
        false,
        (Edge::Presence, Edge::Flush),
        Comments::Elsewhere,
    )
}

/// What [`spell_run`] does with a comment in the gap it spells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Comments {
    /// Print it, in place, as an item of the run — the gap's one emitter owns it.
    InPlace,
    /// Leave it to the emitter that owns it elsewhere; to the spelling it is only
    /// separation between the members either side of it.
    Elsewhere,
}

/// The one spelling of a boundary gap, which every claim in this module emits: its ITEMS —
/// the members and, where the claim owns them ([`Comments::InPlace`]), the comments — in
/// source order, every ASCII whitespace stretch between two items as ONE space and its
/// absence as none, and the two edges as `lead` and `trail` say ([`Edge`]).
///
/// Two readers make the rule, and they agree on it. `parseCss` skips the whole run, so
/// nothing about the AST depends on the spelling. css-syntax-3, whose whitespace is ASCII
/// only, reads each member as identifier content, each ASCII stretch as one
/// `<whitespace-token>`, and a comment as nothing at all, so the spelling IS the
/// tokenization: `<NBSP><NBSP>` is one identifier where `<NBSP> <NBSP>` is two, a stretch
/// welded to nothing merges two tokens, a comment glued to a member keeps it glued to what
/// is on the comment's other side (`<NBSP>/* c */:hover` is the compound `<NBSP>:hover`),
/// and a stretch respelled as a tab or a line break is the same token — but a line break
/// carried inside the text the printer emits is one the doc renderer neither indents after
/// nor counts. So the items stay where the author put them and each stretch between them is
/// one space: the spelling that keeps every token the author wrote and nothing else, and
/// prettier's too, at every juncture where prettier keeps the run.
///
/// Scans `[from, to)` of `source`. Any other character in the gap (the `(` a caller's range
/// opens on) is neither an item nor a separation. `bom_at_zero` excludes a `U+FEFF` at
/// offset 0: the byte-order mark of a whole document, which tsv strips by policy (see
/// [`Printer::boundary_run`]); it is neither a member nor a separation. An in-place comment
/// is reported to the print-once ledger here, since this is the seam that prints it.
fn spell_run(
    source: &str,
    from: usize,
    to: usize,
    bom_at_zero: bool,
    edges: (Edge, Edge),
    comments: Comments,
) -> String {
    let (lead, trail) = edges;
    let bytes = source.as_bytes();
    let mut out = String::new();
    // Whether ASCII whitespace (or, to a members-only spelling, a comment) stands between
    // the previous item (or the gap's start) and this position.
    let mut separated = false;
    let mut i = from;
    while i < to {
        let item_start = i;
        if crate::comments::is_comment_start(bytes, i) {
            let end = crate::comments::comment_end(bytes, i).min(to);
            i = end;
            match comments {
                Comments::Elsewhere => separated = true,
                Comments::InPlace => {
                    #[cfg(feature = "comment_check")]
                    tsv_lang::comment_ledger::record_emitted(
                        source,
                        Span {
                            start: item_start as u32,
                            end: end as u32,
                        },
                    );
                    push_item(&mut out, &source[item_start..end], separated, lead, false);
                    separated = false;
                }
            }
            continue;
        }
        let Some(c) = source[i..].chars().next() else {
            break;
        };
        i += c.len_utf8();
        if bom_at_zero && item_start == 0 && c == tsv_lang::BOM {
            continue;
        }
        if crate::whitespace::is_boundary_only_whitespace(c) {
            push_item(&mut out, &source[item_start..i], separated, lead, true);
            separated = false;
        } else if crate::whitespace::is_boundary_whitespace(c) {
            separated = true;
        }
    }
    if !out.is_empty() && (trail == Edge::Space || (trail == Edge::Presence && separated)) {
        out.push(' ');
    }
    out
}

/// Append one item of a spelled gap: behind one space when `separated` says the author put
/// ASCII whitespace ahead of it — or, for the gap's first item, as `lead` says.
///
/// [`Edge::Space`] is a claim about a MEMBER, which a name ahead of it would take in; a
/// comment cannot be taken in (the name's token ends at its `/*`, and a comment tokenizes to
/// nothing), so a gap that opens on a comment keeps the author's separation there instead.
fn push_item(out: &mut String, item: &str, separated: bool, lead: Edge, is_member: bool) {
    let space = if out.is_empty() {
        match lead {
            Edge::Flush => false,
            Edge::Presence => separated,
            Edge::Space => is_member || separated,
        }
    } else {
        separated
    };
    if space {
        out.push(' ');
    }
    out.push_str(item);
}

/// The byte offset of the CLOSING delimiter a bracketed construct's `span` ends one past —
/// a pseudo-argument list's `)`, an attribute selector's `]`, a block's `}`.
///
/// Named because five readers want it and the `- 1` is not self-evident at any of them: the
/// gap those constructs skipped a boundary run in stops a byte short of `span.end`, and an
/// off-by-one there either swallows the closer into the run or misses the last member. It
/// saturates so a raw subtraction — a shape that is unreachable today
/// (something always precedes the closer) and is a panic rather than a wrong answer the day
/// it isn't.
pub(super) fn closer_pos(span: Span) -> u32 {
    span.end.saturating_sub(1)
}

/// Whether `source` holds a member of the boundary class ANYWHERE — the document-level
/// precondition named in this module's §The document precondition, asked once per
/// [`Printer`] and read by every claim.
///
/// Whole-source and unbounded on purpose. A bound would have to be argued against every claim
/// site's reach — including [`Printer::write_head_boundary_ws`], whose scan is unfloored and
/// walks *backwards* out of the first node — and an argument that is wrong anywhere silently
/// deletes the author's bytes, which is the failure mode this whole module exists to prevent.
/// Answering over more source than any claim can read is the direction that cannot be wrong:
/// it can only decline the fast path, never take it wrongly.
///
/// The scan is the class's own definition read as a byte property: every member is at or above
/// U+00A0 (`is_boundary_only_whitespace` is JS `\s` MINUS its ASCII members), so a member's
/// UTF-8 encoding always has the high bit set, and only the bytes that do need decoding at
/// all. [`next_non_ascii`] steps the ASCII a 32-byte block per branch and the decode runs on
/// what it lands on, so a stylesheet whose only non-ASCII characters are content (an arrow in
/// a `content:` string, an emoji in a comment) pays one word-wise pass and a handful of
/// decodes.
pub(super) fn source_holds_boundary_ws(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut i = 0;
    loop {
        // Resumed at a char boundary every time, so the first high-bit byte at or after `i`
        // is a UTF-8 LEAD byte and the slice below always splits cleanly.
        i = next_non_ascii(bytes, i);
        let Some(c) = source[i..].chars().next() else {
            return false;
        };
        if crate::whitespace::is_boundary_only_whitespace(c) {
            return true;
        }
        i += c.len_utf8();
    }
}

/// Index of the first byte at or after `from` with its high bit set, or `bytes.len()`.
///
/// Word-at-a-time for the same reason `tsv_lang::printing`'s line-terminator candidate scan
/// is: the needle is sparse (a real stylesheet holds a handful of non-ASCII bytes in a
/// hundred kilobytes), so a per-byte compare spends all of its work confirming misses. "Is
/// any lane non-ASCII" is a bit test rather than an equality, so four words fold into one
/// branch — and where that branch fires the byte loop below locates the hit, which costs at
/// most one block and happens at most once per non-ASCII CHARACTER in the document.
#[inline]
fn next_non_ascii(bytes: &[u8], from: usize) -> usize {
    const BLOCK: usize = 32;
    const HIGH: u64 = 0x8080_8080_8080_8080;
    let mut i = from;
    // One slice of KNOWN length per block, split into whole words with no remainder, so the
    // four loads carry no bounds check of their own — a block is one compare and one branch.
    while let Some(block) = bytes[i..].first_chunk::<BLOCK>() {
        let (words, _) = block.as_chunks::<8>();
        if words.iter().fold(0, |acc, w| acc | u64::from_le_bytes(*w)) & HIGH != 0 {
            break;
        }
        i += BLOCK;
    }
    while i < bytes.len() && bytes[i].is_ascii() {
        i += 1;
    }
    i
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
