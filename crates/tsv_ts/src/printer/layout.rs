//! The bracketed list body shared by the TypeScript printer's list builders. The
//! operator→value hang family lives in `tsv_lang::doc::after_operator`.

use tsv_lang::doc::arena::{DocArena, DocId};

/// Bracketed list body: `open` + indented `inner` + `close`. Width-decided
/// (softlines, UNGROUPED — the caller supplies the group if it wants one)
/// unless `force_break` — a Rule A multi-line frozen member — where hardlines
/// substitute: they render identically to the broken group AND propagate the
/// break to every enclosing group (a `verbatim_source_span` is
/// `will_break`-opaque, so the forcing is explicit rather than propagated
/// from the slice). `open` / `close` are the delimiters' docs, built by the caller where
/// their spelling is a literal.
pub(in crate::printer) fn bracketed_list_body(
    d: &DocArena,
    open: DocId,
    close: DocId,
    inner: DocId,
    force_break: bool,
) -> DocId {
    if force_break {
        let body = d.concat(&[d.hardline(), inner]);
        return d.concat(&[open, d.indent(body), d.hardline(), close]);
    }
    d.concat(&[open, d.indent_softline(inner), d.softline(), close])
}
