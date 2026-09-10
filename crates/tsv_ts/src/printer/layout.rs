//! Shared doc shapes: the bracketed list body, and the "break after an
//! operator/keyword, then hang-indent the continuation" layout family.
//!
//! Prettier expresses this family with two distinct mechanisms, and the
//! difference between them is load-bearing — they are NOT interchangeable:
//!
//! - **break-after-operator** (`hang_after_operator`): `group(indent([line, x]))`.
//!   The continuation `x` sits inside the group, so a forced break inside `x`
//!   (e.g. a multiline object type) propagates and forces the break after the
//!   operator. Used where the continuation should drop to the next line when it
//!   breaks — union cast/annotation/return types, type-alias RHS.
//!   Mirrors Prettier's `printAssignment` `break-after-operator` and
//!   `printUnionType` + `shouldIndentUnionType`.
//!
//! - **fluid** (`fluid_after_operator`): `group(indent(line), {id})` +
//!   `lineSuffixBoundary` + `indentIfBreak(value, {id})`. The value sits OUTSIDE
//!   the marker group, so its forced breaks do NOT force the operator break —
//!   an object-like type hugs `= {` / `extends {` and expands internally.
//!   Mirrors Prettier's `printAssignment` `fluid` and `printTypeParameter`.
//!
//! See `prettier/src/language-js/print/assignment.js` (`chooseLayout`),
//! `type-parameters.js` (`printTypeParameter`), and `union-type.js`
//! (`shouldIndentUnionType`).

use tsv_lang::doc::GroupId;
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

/// Break-after-operator hanging indent: `group(indent([line, content]))`.
///
/// Flat: ` content`. Broken: `\n\t content`. A forced break inside `content`
/// forces this group to break (the operator-line drop). Callers emit the
/// operator/keyword text (`=`, `as`, `:`, `=>`, …) as a sibling immediately
/// before this group.
pub(in crate::printer) fn hang_after_operator(d: &DocArena, content: DocId) -> DocId {
    d.group(d.indent_line(content))
}

/// Fluid break-after-operator marker: `group(indent(line), {id})` +
/// `lineSuffixBoundary` + `indentIfBreak(value, {id})`.
///
/// `value` stays outside the marker group, so its own forced breaks do not
/// force the after-operator break — object-like values hug the operator and
/// expand internally. `group_id` ties the conditional indent to this specific
/// marker; it must stay distinct across nested contexts (assignment vs type
/// parameter), so it is always a parameter.
pub(in crate::printer) fn fluid_after_operator(
    d: &DocArena,
    value: DocId,
    group_id: GroupId,
) -> DocId {
    d.concat(&[
        d.group_with_id(d.indent(d.line()), group_id),
        d.line_suffix_boundary(),
        d.indent_if_break(value, group_id),
    ])
}

/// The **un-indented twins** of the two markers above, for a caller that has already
/// spent the continuation level itself.
///
/// A seam that wraps its whole operator→value region in one `d.indent` — the pre-`=`
/// comment path of a type alias, via `Printer::build_continuation_indent` — has put the
/// operator one level in already. A marker's own indent on top of that is a DOUBLE indent:
/// the value's lines land one level deeper than the operator they hang from, and deeper
/// than prettier puts them once it has relocated the comment. These yield the same shapes
/// with that level dropped, so the value sits at the operator's own level.
///
/// They live here rather than inline at the seam so the four markers read as two pairs:
/// the only difference within a pair is the indent, which is the whole question, and an
/// arm reaching for the wrong half is visible on one screen. Pinned by
/// `types/comments/type_alias_line_pre_equals_hang_prettier_divergence` (and its union
/// sibling), whose null controls are the arms that HUG the operator — those spend no level
/// of their own and ask nothing here.
pub(in crate::printer) fn hang_after_operator_unindented(d: &DocArena, content: DocId) -> DocId {
    d.group(d.concat(&[d.line(), content]))
}

/// The un-indented twin of [`fluid_after_operator`] — see
/// [`hang_after_operator_unindented`] for when to reach for one. Both of the original's
/// indents go: the marker group's and the value's conditional one.
pub(in crate::printer) fn fluid_after_operator_unindented(
    d: &DocArena,
    value: DocId,
    group_id: GroupId,
) -> DocId {
    d.concat(&[
        d.group_with_id(d.line(), group_id),
        d.line_suffix_boundary(),
        value,
    ])
}
