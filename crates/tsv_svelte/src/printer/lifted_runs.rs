//! Lifted runs: a hoisted root section written **between** two template nodes.
//!
//! The canonical reorder takes a `<script>`, `<style>` or `<svelte:options>` out of the template
//! together with the comments that travel with it (its leading run and its region-end trail —
//! [`Printer::hoisted_comments`]), so the template nodes written on either side of that run
//! become neighbours. The compiler never saw the run between them either (a lifted section is
//! not a fragment node, and a comment renders nothing), so the neighbours join as if the run
//! had never been there. That is a fact about the whole template layout — text-edge trims, the
//! glue tests, blank-line reads, the fill, the global-element glue — and every one of those
//! readers asks it positionally, off source bytes and node spans that still have the run in
//! between. So it is answered once, ahead of the parse the layout reads: the document is
//! rewritten in the OUTPUT's own layout — every section run in canonical order, the template
//! with each such run's neighbours joined, the `<style>` run last — and the rewritten document
//! is what gets formatted, the same shape as the template-tail respell (`format_respelled_tail`).
//!
//! Writing every run where the output prints it, rather than only the runs that sat between
//! template nodes, is what keeps the comments where they were: the rewritten document holds its
//! comments in the order the output does, so it reads them the way the output's own next pass
//! reads them. A comment the original's classification and the output's disagree on (a
//! region-end trail that ends up directly above another section) therefore lands where the
//! output would put it on its next pass, never somewhere neither would.
//!
//! The gap carries three facts, read off every whitespace segment between the two neighbours —
//! the one before the run, the one after it, and the ones inside it (between a travelling
//! comment and its section, between two runs in a row):
//!
//! - **presence**: any whitespace at all separates them; none keeps them glued, which the
//!   rewrite makes literal adjacency, so the seam the section left never takes a break;
//! - **newline**: any segment holding one makes the gap a line break;
//! - **blank line**: only the segment BEFORE or AFTER the run can hold one for the neighbours.
//!   A blank inside the run belongs to the run — it travels with the comments and prints there
//!   — and the lines the section itself occupied are not a blank line either, so a newline on
//!   each side of it is one line break.
//!
//! A run at the document's start or end (template content on one side only) has no pair of
//! neighbours to join: the whitespace beside it stays where it is, at the fragment's edge, which
//! trims it. A section inside a `format-ignore` range moves too, cut out by [`range_cuts`] — the
//! cuts the range's own verbatim slice makes (`Printer::build_ignore_range_doc`), so the range
//! prints the same bytes either way. Those join the same facts as if the sections were absent, but
//! a winning run before or after the sections is kept byte for byte, since the range's bytes are
//! the author's; only a winning run between two sections is respelled — one of its line breaks,
//! or a single space when it holds none, which renders the same.
//! A document with no run between template nodes (≈ every component) is not rewritten at all.

use super::text::has_authored_blank_line;
use super::{HoistedComments, HoistedSection, Printer, push_char_reference, root_sections};
use crate::ast::internal::{self, FragmentNode};
use smallvec::SmallVec;
use tsv_lang::Span;

/// One hoisted section with the comments that travel with it, as its source pieces in order.
struct LiftedRun {
    section: HoistedSection,
    section_span: Span,
    /// From the first travelling comment (or the section) to the trail (or the section).
    extent: Span,
    /// The leading comments, the section, and the region-end trail, in source order — the
    /// whitespace between consecutive pieces is the run's INSIDE.
    pieces: SmallVec<[Span; 4]>,
    /// Whether template content sits on both sides of the run.
    between: bool,
    /// Whether the section sits inside a `format-ignore` range.
    in_range: bool,
}

/// The whitespace facts of the segments between two neighbours ([module docs](self)).
#[derive(Default)]
struct GapFacts {
    whitespace: bool,
    newline: bool,
    blank_line: bool,
}

impl GapFacts {
    /// Fold one whitespace segment in; `edge` is whether it is the segment before or after the
    /// run group, the only two a blank line counts from.
    ///
    /// An edge segment is collapsible whitespace by construction (the run's neighbours are cut
    /// at it), so a blank line there is any second newline — what
    /// [`has_authored_blank_line`] reads when no content byte can sit between the two.
    fn add(&mut self, segment: &str, edge: bool) {
        debug_assert!(
            !edge || internal::collapsible_ws_prefix_len(segment) == segment.len(),
            "an edge segment is collapsible whitespace"
        );
        self.whitespace |= !segment.is_empty();
        self.newline |= segment.contains('\n');
        self.blank_line |= edge && has_authored_blank_line(segment);
    }

    /// The facts of one segment.
    fn of(segment: &str, edge: bool) -> Self {
        let mut facts = Self::default();
        facts.add(segment, edge);
        facts
    }

    /// How strongly the segments separate the neighbours: a blank line, then a line break, then
    /// any other whitespace, then none — the order [`Self::spelling`] reads.
    fn strength(&self) -> u8 {
        if self.blank_line {
            3
        } else if self.newline {
            2
        } else {
            u8::from(self.whitespace)
        }
    }

    /// The one gap that stands for the segments.
    fn spelling(&self) -> &'static str {
        match self.strength() {
            3 => "\n\n",
            2 => "\n",
            1 => " ",
            _ => "",
        }
    }
}

/// One cut a `format-ignore` range's slice makes to let the hoisted sections written in it go:
/// the source it removes, and what stands in that place ([`range_cuts`]).
pub(super) struct RangeCut {
    /// The sections and the whitespace on either side of them.
    pub(super) removed: Span,
    pub(super) seam: RangeSeam,
}

/// What joins the two neighbours of a [`RangeCut`].
pub(super) enum RangeSeam {
    /// Source bytes printed as written: the run before or after the sections, or one line break
    /// of a run between them (possibly empty, when nothing separates the neighbours).
    Kept(Span),
    /// A plain space, for a run between sections that holds no line break.
    Space,
}

impl RangeSeam {
    /// The seam's text.
    pub(super) fn text<'s>(&self, source: &'s str) -> &'s str {
        match self {
            Self::Kept(span) => span.extract(source),
            Self::Space => " ",
        }
    }
}

/// The cuts a `format-ignore` range makes for the hoisted `sections` written in it (in source
/// order, every one past `floor` — the end of the range's start marker, or wherever the caller
/// has already emitted to).
///
/// A range is frozen, but a section in it still moves to its canonical position, and the nodes on
/// either side of it become neighbours. Sections with only whitespace between them are one seam,
/// joined as if they had never been there, the way [`GapFacts`] joins a lifted run outside a
/// range: of the run before the sections, the runs between them and the run after them, the
/// STRONGEST ([`GapFacts::strength`]) stands for the whole seam, and the rest go with the
/// sections. A run before or after is kept byte for byte, since the range's bytes are the
/// author's, and wins a tie (the run after over the run before). A run between two sections is
/// respelled: one of its line breaks, or a single space when it holds none (which renders the
/// same as the tab or spaces it stands for). It never counts as a blank line — a blank line there
/// belongs to the sections, as a blank inside a lifted run does — so it wins only where neither
/// edge run holds what it does: whitespace alone, or a line break.
///
/// A run before the sections stops at `floor`, which is the start marker's end for the first
/// cut; a run after them stops at the next non-whitespace byte, which the end marker guarantees
/// comes before the range closes.
pub(super) fn range_cuts(source: &str, floor: u32, sections: &[Span]) -> SmallVec<[RangeCut; 2]> {
    let ws_only = |from: u32, to: u32| {
        let run = &source[from as usize..to as usize];
        internal::collapsible_ws_prefix_len(run) == run.len()
    };
    let mut cuts = SmallVec::new();
    let mut floor = floor;
    let mut i = 0;
    while i < sections.len() {
        let mut j = i;
        while j + 1 < sections.len() && ws_only(sections[j].end, sections[j + 1].start) {
            j += 1;
        }
        let first = sections[i].start;
        let last = sections[j].end;
        let before_len =
            internal::collapsible_ws_suffix_len(&source[floor as usize..first as usize]);
        let after_len = internal::collapsible_ws_prefix_len(&source[last as usize..]);
        let before = Span::new(first - before_len as u32, first);
        let after = Span::new(last, last + after_len as u32);

        let mut seam = RangeSeam::Kept(after);
        let mut strength = GapFacts::of(after.extract(source), true).strength();
        let before_strength = GapFacts::of(before.extract(source), true).strength();
        if before_strength > strength {
            seam = RangeSeam::Kept(before);
            strength = before_strength;
        }
        for pair in sections[i..=j].windows(2) {
            let between = Span::new(pair[0].end, pair[1].start);
            let text = between.extract(source);
            let facts = GapFacts::of(text, false);
            if facts.strength() > strength {
                strength = facts.strength();
                // The run is whitespace alone, so its first newline byte is its line break.
                seam = match text.bytes().position(|b| b == b'\n') {
                    Some(at) => {
                        let at = between.start + at as u32;
                        RangeSeam::Kept(Span::new(at, at + 1))
                    }
                    None => RangeSeam::Space,
                };
            }
        }
        cuts.push(RangeCut {
            removed: Span::new(before.start, after.end),
            seam,
        });
        floor = after.end;
        i = j + 1;
    }
    cuts
}

impl Printer<'_> {
    /// The document rewritten in the output's layout ([module docs](self)) — or `None` when no
    /// section run sits between template nodes.
    ///
    /// Layout: a leading byte-order mark, then the `<svelte:options>`, module and instance runs
    /// in that order, then the template with every run taken out, then the `<style>` run. The
    /// document's end reads the way the original's did: what the original parse trimmed from it
    /// (JavaScript's `trimEnd`, [`tsv_lang::trim_end_js_whitespace`]) stays out, and a character
    /// the original rendered that the template now ends on is spelled as a reference.
    pub(super) fn lifted_runs_source(
        &self,
        root: &internal::Root<'_>,
        sections: &HoistedComments,
    ) -> Option<String> {
        let mut runs = self.section_runs(root, sections);
        if !runs.iter().any(|run| run.between) {
            return None;
        }
        // A section inside a `format-ignore` range travels with no comments (the range keeps
        // every comment in it where it stands), and leaves the range through the cuts the
        // range's own verbatim slice makes ([`range_cuts`]).
        let nodes = &root.fragment.nodes;
        let ranges: SmallVec<[Span; 2]> = self
            .ignore_ranges(&root.fragment)
            .pairs
            .iter()
            .map(|&(start, end)| Span::new(nodes[start].span().start, nodes[end].span().end))
            .collect();
        for run in &mut runs {
            run.in_range = ranges.iter().any(|range| range.contains(run.section_span));
        }
        if !runs.iter().any(|run| run.between && !run.in_range) {
            return None;
        }
        runs.sort_unstable_by_key(|run| run.extent.start);

        let source = self.source;
        let bom_len = if source.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        let mut out = String::with_capacity(source.len() + runs.len() * 2 + 2);
        out.push_str(&source[..bom_len]);
        let mut heads: SmallVec<[&LiftedRun; 4]> = runs
            .iter()
            .filter(|run| run.section != HoistedSection::Style)
            .collect();
        heads.sort_by_key(|run| run.section.slot());
        for run in heads {
            out.push_str(run.extent.extract(source));
            out.push_str("\n\n");
        }

        let ws_only_between = |a: &LiftedRun, b: &LiftedRun| {
            let between = &source[a.extent.end as usize..b.extent.start as usize];
            internal::collapsible_ws_prefix_len(between) == between.len()
        };
        let mut template = String::with_capacity(source.len());
        let mut cursor = bom_len;
        let mut i = 0;
        while i < runs.len() {
            if runs[i].in_range {
                // Cut the way the range's verbatim slice cuts it ([`range_cuts`]), so the range
                // prints the same bytes either way.
                let mut j = i;
                while j + 1 < runs.len() && runs[j + 1].in_range {
                    j += 1;
                }
                let sections: SmallVec<[Span; 4]> =
                    runs[i..=j].iter().map(|run| run.extent).collect();
                for cut in range_cuts(source, cursor as u32, &sections) {
                    template.push_str(&source[cursor..cut.removed.start as usize]);
                    template.push_str(cut.seam.text(source));
                    cursor = cut.removed.end as usize;
                }
                i = j + 1;
                continue;
            }
            // A group: consecutive runs with nothing but whitespace between them share one
            // pair of neighbours (or one fragment edge).
            let mut j = i;
            while j + 1 < runs.len()
                && !runs[j + 1].in_range
                && ws_only_between(&runs[j], &runs[j + 1])
            {
                j += 1;
            }
            let group = &runs[i..=j];
            let first = group[0].extent.start as usize;
            let last = group[group.len() - 1].extent.end as usize;
            if group[0].between {
                let before = first - internal::collapsible_ws_suffix_len(&source[..first]);
                let after = last + internal::collapsible_ws_prefix_len(&source[last..]);
                let mut facts = GapFacts::default();
                facts.add(&source[before..first], true);
                facts.add(&source[last..after], true);
                for (k, run) in group.iter().enumerate() {
                    for pair in run.pieces.windows(2) {
                        facts.add(&source[pair[0].end as usize..pair[1].start as usize], false);
                    }
                    if let Some(next) = group.get(k + 1) {
                        facts.add(
                            &source[run.extent.end as usize..next.extent.start as usize],
                            false,
                        );
                    }
                }
                template.push_str(&source[cursor..before]);
                template.push_str(facts.spelling());
                cursor = after;
            } else {
                template.push_str(&source[cursor..first]);
                cursor = last;
            }
            i = j + 1;
        }
        // What the original parse trimmed from the end of the document stays trimmed: those
        // characters never rendered, and a run written after them would make them render.
        let kept_end = tsv_lang::trim_end_js_whitespace(source).len();
        template.push_str(&source[cursor..kept_end.max(cursor)]);

        match runs.iter().find(|run| run.section == HoistedSection::Style) {
            Some(style) => {
                out.push_str(&template);
                out.push_str("\n\n");
                out.push_str(style.extent.extract(source));
                out.push('\n');
            }
            None => {
                // The template now ends the document, and a character the original rendered —
                // it stood before a run the rewrite moved away — would be trimmed there by the
                // next parse. Spelled as a reference it survives, as the template-tail respell
                // spells it (`Printer::template_tail_respell`).
                let content = internal::trim_end_collapsible_ws(&template);
                match content.chars().next_back() {
                    Some(ch) if crate::whitespace::is_end_trimmed_content(ch) => {
                        let at = content.len() - ch.len_utf8();
                        out.push_str(&template[..at]);
                        push_char_reference(&mut out, ch);
                        out.push_str(&template[at + ch.len_utf8()..]);
                    }
                    _ => out.push_str(&template),
                }
            }
        }
        Some(out)
    }

    /// Every hoisted section's run, marked with whether template content sits on both sides of
    /// it.
    fn section_runs(
        &self,
        root: &internal::Root<'_>,
        sections: &HoistedComments,
    ) -> SmallVec<[LiftedRun; 4]> {
        let nodes = &root.fragment.nodes;
        let travels = sections.travelling(nodes.len());
        let is_content =
            |i: usize, n: &FragmentNode<'_>| !(n.is_whitespace_only_text() || travels[i]);
        let content = nodes
            .iter()
            .enumerate()
            .position(|(i, n)| is_content(i, n))
            .map(|first| {
                let last = nodes
                    .iter()
                    .enumerate()
                    .rposition(|(i, n)| is_content(i, n))
                    .unwrap_or(first);
                (nodes[first].span().start, nodes[last].span().end)
            });

        let mut runs = SmallVec::new();
        for (span, section) in root_sections(root) {
            let Some(span) = span else { continue };
            let mut run = self.lifted_run(span, section, sections, &root.fragment);
            run.between = content
                .is_some_and(|(start, end)| start < run.extent.start && run.extent.end < end);
            runs.push(run);
        }
        runs
    }

    /// The run of `section`: its leading comments, the section, its trail.
    fn lifted_run(
        &self,
        section_span: Span,
        section: HoistedSection,
        sections: &HoistedComments,
        fragment: &internal::Fragment<'_>,
    ) -> LiftedRun {
        let comments = &sections[section];
        let comment_span = |i: usize| Self::fragment_comment(fragment, i).map(|c| c.span);
        let mut pieces: SmallVec<[Span; 4]> = comments
            .leading
            .iter()
            .filter_map(|&i| comment_span(i))
            .collect();
        pieces.push(section_span);
        pieces.extend(comments.trail.and_then(comment_span));
        let extent = Span::new(pieces[0].start, pieces[pieces.len() - 1].end);
        LiftedRun {
            section,
            section_span,
            extent,
            pieces,
            between: false,
            in_range: false,
        }
    }
}
