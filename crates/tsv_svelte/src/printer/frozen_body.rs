// The frozen verbatim body — one claim, one place.
//
// A body declared as a language tsv does not format at that position is the author's own
// bytes (`internal::EmbeddedLang::is_frozen` decides which), and this module owns the shape
// they ride out in: prettier's `preformattedBody`, spelled once for the two emitters that
// need it — the top-level `<script>` / `<style>` WRITER (`script_style.rs`) and the doc path
// for a nested raw-text element or a frozen `<template>` (`nodes/element_doc.rs`).
//
// The *contrast* these two arms are decided against — the delimiter-only shape a body with
// nothing to format takes — belongs to the formattable arm and lives at its one call site,
// `nodes/element_doc.rs`.
//
// See docs/conformance_prettier_svelte.md §Foreign-language embedded bodies.

use crate::printer::Printer;
use tsv_lang::Span;
use tsv_lang::doc::arena::DocId;

/// The horizontal filler a frozen body's own delimiter line may carry — prettier's
/// `[\t\f\r ]` in [`frozen_body_span`], minus the `\r`.
///
/// Deliberately **not** `is_collapsible_ws_char`: a form feed belongs here (it is filler on
/// the delimiter's line) where the template's own text nodes treat it as content, and `\n` is
/// excluded because it is the terminator these scans are looking for, never part of the run.
///
/// So this is the crate's *other* `[\t\f ]`, and the two split on the form feed on purpose:
/// `fragment_doc`'s `text_starts_with_linebreak` drops it, because there the run is template
/// text the compiler renders. Here the run is a delimiter line tsv itself emits and no reader
/// ever sees, so the form feed stays filler.
///
/// ⚠️ **`\r` is a terminator here, never filler.** Prettier normalizes its input to LF
/// before this scan runs, so a `\r` reaching its `[\t\f\r ]` is stray filler by
/// construction and the branch that treats it so is unreachable. tsv folds the same way at
/// the same point ([`tsv_lang::printing::normalize_carriage_returns`], ahead of the parse),
/// so no format entry point brings one here either — but the library primitive
/// `format(&ast, source)` takes the caller's bytes as given, and there a `\r` is
/// overwhelmingly the delimiter's own line ending. So it is matched as a terminator by
/// [`strip_one_terminator_front`] / [`strip_one_terminator_back`] rather than eaten as
/// filler: eaten, the `\n` the scan then looked for was never there, and a CR-terminated
/// document kept its opening delimiter as a blank first body line.
const DELIMITER_LINE_FILLER: [char; 3] = ['\t', '\x0c', ' '];

/// The trailing run a rendered line never keeps — prettier's `trimIndentation`, which is
/// **space and tab only** (never `\r`, `\f` or `\n`).
///
/// The doc paths get this for free: [`tsv_lang::doc`]'s renderer trims exactly this class
/// ahead of every non-literal newline, so a frozen body emitted as a doc is trimmed by the
/// `hardline` that closes it. The **writer** path builds its own lines and so must spell it,
/// or the same body comes out two ways at its two positions.
const RENDERED_LINE_TRAILING_WS: [char; 2] = [' ', '\t'];

/// Strip ONE line terminator from the front of `s` — `\r\n`, `\n`, or `\r`, longest first
/// so a CRLF pair is never read as two. `None` when `s` does not open with one.
fn strip_one_terminator_front(s: &str) -> Option<&str> {
    s.strip_prefix("\r\n")
        .or_else(|| s.strip_prefix('\n'))
        .or_else(|| s.strip_prefix('\r'))
}

/// [`strip_one_terminator_front`]'s mirror at the end of `s`.
fn strip_one_terminator_back(s: &str) -> Option<&str> {
    s.strip_suffix("\r\n")
        .or_else(|| s.strip_suffix('\n'))
        .or_else(|| s.strip_suffix('\r'))
}

/// The span a frozen body contributes — the whole content run minus the two delimiter lines'
/// own terminators, prettier's `.replace(/^[\t\f\r ]*\n/, '').replace(/\n[\t\f\r ]*$/, '')`
/// with `\r` read as a terminator rather than as filler (see [`DELIMITER_LINE_FILLER`]).
/// Empty when the body is nothing but those two lines.
///
/// **Every** freeze goes through it — a frozen `<template>`, `<script>` or `<style>` body,
/// at each of its positions: one shape, because the claim they all make is the same one
/// (these bytes are the author's).
///
/// Only the FIRST terminator goes at each end — a body that opens or closes with blank lines
/// keeps every one of them but the delimiter's own. The strips are sequential in prettier, so
/// they are here too: a lone `"\n"` body loses its newline to the leading strip and has
/// nothing left for the trailing one.
///
/// The slice's own **last line** still owes [`RENDERED_LINE_TRAILING_WS`]; this returns the
/// range the freeze *covers* (what the comment ledger is told rode out), not the bytes a
/// writer emits, so the trim belongs to the emitter rather than to the bounds.
pub(in crate::printer) fn frozen_body_span(source: &str, content: Span) -> Span {
    let raw = content.extract(source);
    let rest = raw.trim_start_matches(DELIMITER_LINE_FILLER);
    let start = match strip_one_terminator_front(rest) {
        Some(after) => raw.len() - after.len(),
        None => 0,
    };
    let head = raw[start..].trim_end_matches(DELIMITER_LINE_FILLER);
    let end = match strip_one_terminator_back(head) {
        Some(shorter) => start + shorter.len(),
        None => raw.len(),
    };
    Span::new(content.start + start as u32, content.start + end as u32)
}

impl<'a> Printer<'a> {
    /// Write everything a frozen top-level section owes after its opening tag: one delimiter
    /// newline, the author's bytes, one delimiter newline, then the closing tag. A body with
    /// no bytes between the tags gets neither newline, so the pair collapses onto one line.
    ///
    /// The single verbatim-freeze emitter of the two top-level writers, shared by `<script>`
    /// and `<style>` because the claim is one claim — *this body is in a language tsv does
    /// not format, so these are the author's bytes*. [`Printer::frozen_body_doc`] is its
    /// doc-path twin, and [`frozen_body_span`] the shape both take.
    pub(in crate::printer) fn write_frozen_section(&mut self, tag: &str, content: Span) {
        if content.start != content.end {
            // The range the freeze COVERS, untrimmed: a comment ending in the trailing run
            // below is inside it, and a ledger range that stopped short of one would read as
            // a DROP.
            let body = frozen_body_span(self.source, content);
            #[cfg(feature = "comment_check")]
            tsv_lang::comment_ledger::record_verbatim_range(self.source, body.start, body.end);
            // The `&'a str` field, not `self.source()` — that accessor's return borrows
            // `&self`, which the `write` calls below would conflict with.
            let bytes = body.extract(self.source);
            self.write("\n");
            // The slice's last line owes the trailing-whitespace trim every rendered line
            // owes — the doc-path freeze gets it from its closing `hardline`, and prettier's
            // `preformattedBody` from the same `hardline`. Interior lines keep theirs on both
            // sides: only the run at the very end is on a line the break is about to close.
            self.write(bytes.trim_end_matches(RENDERED_LINE_TRAILING_WS));
            self.write("\n");
        }
        self.write("</");
        self.write(tag);
        self.write(">\n");
    }

    /// The frozen verbatim body every doc-path freeze shares — a frozen `<template>`'s child
    /// run and a frozen nested `<script>` / `<style>`'s raw text: the span's bytes between a
    /// `literalline` (the first line keeps column 0 — the author's indentation is part of a
    /// potentially whitespace-significant language) and a `hardline` (the closing tag takes
    /// the element's own indent), with one leading blank-through-newline and one trailing
    /// newline-through-blanks stripped ([`frozen_body_span`] — prettier's `preformattedBody`
    /// exactly).
    ///
    /// `source_span_covering_comments_doc`: a `{/* c */}` in the body reaches no comment
    /// emitter — it rides out inside this slice, so the ledger is told.
    pub(in crate::printer) fn frozen_body_doc(&self, content: Span) -> DocId {
        let d = self.d();
        let body = frozen_body_span(self.source, content);
        if body.start == body.end {
            return d.concat(&[d.literalline(), d.hardline()]);
        }
        d.concat(&[
            d.literalline(),
            self.source_span_covering_comments_doc(body),
            d.hardline(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::{RENDERED_LINE_TRAILING_WS, frozen_body_span};
    use tsv_lang::Span;

    /// The slice a body contributes, as the freeze emitters take it.
    fn body(raw: &str) -> &str {
        frozen_body_span(raw, Span::new(0, raw.len() as u32)).extract(raw)
    }

    #[test]
    fn body_bounds_drop_one_delimiter_newline_per_side() {
        assert_eq!(body("\nh1 text\n"), "h1 text");
        // Horizontal filler on either delimiter line goes with that line's newline.
        assert_eq!(body("  \nh1 text\n\t"), "h1 text");
        // Only the FIRST newline: the author's own blank lines are body.
        assert_eq!(body("\n\nh1 text\n\n"), "\nh1 text\n");
        // A body with no boundary newline at all keeps every byte.
        assert_eq!(body("h1 text"), "h1 text");
        // Trailing filler is only a delimiter line when a newline precedes it.
        assert_eq!(body("\nh1 text  "), "h1 text  ");
    }

    /// The span is relative to the CONTENT's own start, not to byte 0 — the two emitters
    /// hand it a mid-document span and feed the result straight to the ledger.
    #[test]
    fn body_bounds_are_absolute_in_the_host_source() {
        let src = "<style lang=\"less\">\na\n</style>";
        let content = Span::new(19, 22);
        assert_eq!(content.extract(src), "\na\n");
        assert_eq!(frozen_body_span(src, content).extract(src), "a");
    }

    #[test]
    fn body_bounds_are_empty_when_the_two_delimiter_lines_are_all_there_is() {
        // The strips are sequential, so a lone newline is spent on the leading one and the
        // trailing strip finds nothing left.
        assert_eq!(body("\n"), "");
        assert_eq!(body("\n\n"), "");
        // But the trailing strip needs a newline of its own: filler with none in front of it
        // is body, exactly as prettier's `/\n[\t\f\r ]*$/` leaves it. The printed line is
        // still empty — every emitter owes the slice's last line
        // [`RENDERED_LINE_TRAILING_WS`] — so this is a fact about the slice, not about the
        // output.
        assert_eq!(body("  \n  "), "  ");
        // A form feed is filler on a delimiter line, unlike in rendered template text.
        assert_eq!(body("\u{c}\nh1 text\u{c}\n\u{c}"), "h1 text\u{c}");
    }

    /// The delimiter's own terminator goes whole at each end — CRLF or lone CR — while an
    /// INTERIOR line's terminator stays in the span. The span is the caller's bytes, so the
    /// two questions stay separate: every format entry point has already folded its input
    /// (`tsv_lang::printing::normalize_carriage_returns`), and these bounds answer the
    /// library primitive that has not.
    #[test]
    fn body_bounds_drop_the_delimiters_own_terminator_however_it_is_spelled() {
        assert_eq!(body("\r\nh1 text\r\n"), "h1 text");
        assert_eq!(body("\r\na\r\nb\r\n"), "a\r\nb");
        // A lone CR is a terminator too. Read as delimiter FILLER it was eaten, the `\n` the
        // scan then looked for was never there, and the opening delimiter survived as a blank
        // first body line — a whole extra line in a CR-terminated document.
        assert_eq!(body("\rh1 text\r"), "h1 text");
        assert_eq!(body("\ra\rb\r"), "a\rb");
    }

    /// The trailing run every emitter owes the slice's last line, and only its last line —
    /// the run the closing break is about to end. `\r` and `\f` are not in the class
    /// (prettier's `trimIndentation` is space and tab), so this trim never touches a
    /// terminator; the delimiter's own goes to the bounds above, and an interior one is
    /// already `<LF>` on every path that folded its input.
    #[test]
    fn a_frozen_bodys_last_line_owes_the_rendered_line_trim() {
        fn trim(raw: &str) -> &str {
            body(raw).trim_end_matches(RENDERED_LINE_TRAILING_WS)
        }
        assert_eq!(trim("  \n  "), "");
        assert_eq!(trim("\nh1 text  "), "h1 text");
        // Interior lines keep theirs.
        assert_eq!(trim("\na  \nb  \n"), "a  \nb");
        assert_eq!(trim("\r\nh1 text\r\n"), "h1 text");
    }
}
