//! Shared doc shapes for the "break after an operator/keyword, then hang-indent
//! the continuation" layout family.
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
        d.indent_if_break(value, group_id, false),
    ])
}
