//! The "break after an operator/keyword, then hang-indent the continuation"
//! layout family, shared by every language printer.
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

use super::arena::{DocArena, DocId};

/// Break-after-operator hanging indent: `group(indent([line, content]))`.
///
/// Flat: ` content`. Broken: `\n\t content`. A forced break inside `content`
/// forces this group to break (the operator-line drop). Callers emit the
/// operator/keyword text (`=`, `as`, `:`, `=>`, …) as a sibling immediately
/// before this group.
pub fn hang_after_operator(d: &DocArena, content: DocId) -> DocId {
    d.group(d.indent_line(content))
}

/// Fluid break-after-operator marker: `group(indent(line), {id})` +
/// `lineSuffixBoundary` + `indentIfBreak(value, {id})`.
///
/// `value` stays outside the marker group, so its own forced breaks do not
/// force the after-operator break — object-like values hug the operator and
/// expand internally. The conditional indent is keyed on this marker instance, so a
/// fluid layout nested in `value` never answers for it.
pub fn fluid_after_operator(d: &DocArena, value: DocId) -> DocId {
    let marker = d.group_with_id(d.indent(d.line()));
    d.concat(&[
        marker.doc(),
        d.line_suffix_boundary(),
        d.indent_if_break(value, marker),
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
pub fn hang_after_operator_unindented(d: &DocArena, content: DocId) -> DocId {
    d.group(d.concat(&[d.line(), content]))
}

/// The un-indented twin of [`fluid_after_operator`] — see
/// [`hang_after_operator_unindented`] for when to reach for one. Both of the original's
/// indents go: the marker group's and the value's conditional one — so nothing reads
/// the marker's mode, and it is a plain group.
pub fn fluid_after_operator_unindented(d: &DocArena, value: DocId) -> DocId {
    d.concat(&[d.group(d.line()), d.line_suffix_boundary(), value])
}

/// The after-operator layout for a value whose only break point is a LAST RESORT —
/// one an enclosing layout never takes while a break of its own keeps the line within
/// print width (a sole nullable union type argument, which the value's doc already
/// prints unbreakable first and broken inside only past print width). The break after
/// the operator is gated on the value's HEAD: everything up to its first break point.
///
/// Five states, tried in order:
///
/// 0. the whole value flat on the operator's line — the conditional group's `contents`,
///    prettier's `conditionalGroup` contents tried flat first. It must carry no forced
///    break: [`DocArena::will_break`] reads a group's `contents` alone, and a forced break
///    there would force every group ENCLOSING this one to break (an `if` body, a call's
///    arguments, an arrow body, an object property) however short the line;
/// 1. the value on the operator's line, laid out by its own groups — taken while the
///    value's head (measured as a break-mode look-ahead does) fits there, so a call's
///    arguments break before the operator does, exactly as [`fluid_after_operator`]
///    decides;
/// 2. break after the operator, the value on the continuation line laid out by its own
///    groups — taken while the value's head fits there, measured as state 1 measures it
///    (a when-fits gate standing in for the line the break puts it on, since no fits walk
///    measures past a break): prettier's break-after-operator, so a call whose `(` fits
///    on the continuation line breaks its arguments below it;
/// 3. the value stays on the operator's line and breaks inside, where its last-resort
///    break point is (`value_broken`, the value's doc with that break point forced) —
///    taken while the line up to that break fits;
/// 4. otherwise state 2 ungated: break after the operator, the value laid out by its own
///    groups — which break it inside at its last-resort point, since its head did not fit
///    on that line (state 2's gate failed). Only a value whose last-resort break leaves a
///    line over print width by itself overflows here.
///
/// The last state is also what a look-ahead reaching this group from outside in break
/// mode reads (a breakable binding deciding whether it fits): it starts with a forced
/// break, so that look-ahead measures up to the operator alone, as prettier's break-mode
/// `line` after an operator does. Were state 3 last, a binding whose line fits up to the
/// operator but not up to the value's last-resort break would break open itself while the
/// render took state 2. A flat look-ahead reads state 0, the value whole, as prettier's
/// does. Fluid's marker answers only state 1's question, so a value that does not fit
/// after the operator either would break after it anyway and overflow there.
pub fn after_operator_head_gated(d: &DocArena, value: DocId, value_broken: DocId) -> DocId {
    let on_line =
        |value: DocId| d.concat(&[d.text(" "), d.line_suffix_boundary(), d.group_break(value)]);
    let value_open = d.group_break(value);
    let hung = d.indent(d.concat(&[d.hardline(), value_open]));
    d.conditional_group(&[
        d.concat(&[d.text(" "), d.line_suffix_boundary(), value]),
        on_line(value),
        d.gated_state_when_fits(value_open, hung),
        on_line(value_broken),
        hung,
    ])
}

/// Break-after-operator for a value that is poorly breakable only while one of its
/// breaks reads as unauthored — with that break's settled layout gated in between.
///
/// Three states: the value flat on the operator's line (state 0, the hang's flat
/// reading, so an enclosing look-ahead and [`DocArena::will_break`] read exactly what
/// [`hang_after_operator`] gives them); [`fluid_after_operator`] over `value_assumed`,
/// the value built with that break assumed authored, admitted only while `probe`
/// cannot fit on the continuation line the hang would put it on; then the hang again,
/// rendered when neither fits.
///
/// Prettier reaches the middle state on its SECOND pass: its first breaks after the
/// operator, the value breaks for width on the continuation line, and the second pass
/// reads that break as authored, which makes the value breakable and keeps it on the
/// operator's line. `probe` stands for the continuation line — what of the value the
/// first pass puts there (a [`DocArena::line_probe`] where that line ends before the
/// value does) — so the state is taken only where that first pass breaks inside the
/// value.
///
/// The middle state's contents are a `group_break`, so the gate alone admits the state.
/// Measured flat, as a conditional group measures a state, the value's own conditional
/// groups read only their first state — the value whole on the operator's line, up to
/// the assumed break — so where that line does not fit but a later state of the value's
/// does (a root call that breaks its own argument list and keeps the hug,
/// `w(⏎\t1⏎).f…<{`), a flat measurement would fall through to the hang. Measured in break
/// mode, the contents end at their first line break, and the value's groups choose their
/// states at render, as prettier's second pass chooses them.
pub fn after_operator_assumed_fluid(
    d: &DocArena,
    value: DocId,
    probe: DocId,
    value_assumed: DocId,
) -> DocId {
    let hung = hang_after_operator(d, value);
    d.conditional_group(&[
        hung,
        d.gated_state(probe, d.group_break(fluid_after_operator(d, value_assumed))),
        hung,
    ])
}
