//! Greedy line-packing for `Fill` doc nodes.
//!
//! `render_fill_iterative` is the fill layout algorithm, mutually recursive
//! with `render_single_doc` in `arena_render.rs`.

use smallvec::SmallVec;

use super::arena::{ArenaCommand, DocArena, DocId, RenderIndent, WeldedEntry};
use super::arena_fits::{arena_fits_multi, arena_fits_with_lookahead};
use super::arena_render::{
    RenderCtx, line_start_column, render_single_doc, trim_trailing_whitespace, write_indentation,
};
use super::types::{DocContext, Mode};

/// Render a fill doc using greedy line packing (iterative version).
///
/// `has_line_suffix` is the render loop's pending-suffix state at fill entry, threaded
/// verbatim into every `fits` call below (Prettier passes `lineSuffix.length > 0` from
/// its own fill arm): a `LineSuffixBoundary` reached with a comment pending doesn't
/// fit, because the flush will end the line — see [`arena_fits_with_lookahead`].
// Remaining args are the MUTABLE render state (`output`/`pos`/`should_remeasure`, plus the
// work buffers). Deliberately not bundled: a struct would take their address and sink them out
// of registers in the hot loop — see `RenderCtx`, which carries only the shared context.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_fill_iterative(
    ctx: &RenderCtx<'_>,
    parts: &[DocId],
    output: &mut String,
    pos: &mut usize,
    indent: RenderIndent,
    context: &DocContext,
    rest_commands: &[ArenaCommand],
    has_line_suffix: bool,
    should_remeasure: &mut bool,
) {
    let &RenderCtx {
        arena,
        render,
        embed,
        source,
    } = ctx;
    let mut offset = 0;

    while offset < parts.len() {
        let remaining = render.print_width.saturating_sub(*pos);
        let content = parts[offset];

        let is_final_segment = offset + 2 >= parts.len();
        // Final segment with following render-stack content — the boundary to whatever comes
        // after the fill, where every look-ahead measurement below applies.
        let final_with_rest = is_final_segment && !rest_commands.is_empty();

        let available = if is_final_segment {
            remaining.saturating_sub(context.trailing_reserve() as usize)
        } else {
            remaining
        };

        // Flow boundary, forced-break follower — ONE condition shared by Case 1's `content_fits`
        // and Case 2's `sep_fits` (the same boundary rule, differing only in where the separator
        // sits): the node after this fill is already multiline (wrapped attributes, a block-body
        // handler), so the welded unit can't stay on the line — never "fits". Prettier's
        // `group([line, element])` breaks on that forced break and drops the element; a flat
        // measurement here would instead short-circuit at the follower's own hardline and
        // wrongly report a fit, hugging it onto the text line.
        let flow_forced_break = context.break_before_wide_flow()
            && is_final_segment
            && rest_commands
                .last()
                .is_some_and(|c| arena.will_break(c.doc));

        // `break_before_wide_flow`, Case-1 half: a GLUED text→element boundary (`… glued<a…>`) has
        // no trailing separator, so the glued last word is the fill's last item and the element
        // follows on the render stack — the whole-flat measurement lands here (the space-separated
        // half lands in Case 2's `sep_fits`). A ws-fill also reaches this at `is_final_segment`, but
        // its content there is a bare word whose `content_fits` only feeds `should_remeasure` (inert
        // for a groupless leaf), so keying the stack on the shared flag is contamination-free.
        let content_fits = if flow_forced_break {
            false
        } else if final_with_rest {
            let flow_stack;
            let lookahead: &[ArenaCommand] = if context.break_before_wide_flow() {
                // Measure the following element as a WHOLE flat unit so the fill breaks at the
                // whitespace boundary BEFORE the glued last word when (word + element) don't fit.
                // The element's inherited Break mode would otherwise let `arena_fits`
                // short-circuit at its first internal line and wrongly report "fits", welding the
                // word and breaking the element's own content in place. Pairwise like Case 2's
                // `sep_fits` — the same boundary rule, only the separator differs — so it takes
                // the same truncated stack (see [`flow_lookahead`]).
                flow_stack = flow_lookahead(arena, rest_commands);
                &flow_stack
            } else {
                rest_commands
            };
            arena_fits_with_lookahead(
                arena,
                content,
                Mode::Flat,
                lookahead,
                remaining as isize,
                has_line_suffix,
                source,
            )
        } else {
            arena_fits_with_lookahead(
                arena,
                content,
                Mode::Flat,
                &[],
                available as isize,
                has_line_suffix,
                source,
            )
        };

        // A collapsible `line` in the CONTENT slot is 1 column flat, so measuring it ALONE is
        // meaningless — it always "fits" and could never force a break. A `line` lands there
        // whenever the fill was built with a LEADING separator (`leading_line` — Svelte text
        // after an expression tag), which shifts the content/separator parity by one: every
        // `line` occupies a content slot and every word a separator.
        //
        // The fit that matters is the line PLUS the word it separates, so fold the separator
        // into the measurement (top of the look-ahead stack is what comes next). Without this
        // the pair renders flat past printWidth — print width is a hard limit in tsv — and the
        // break lands one separator too late, which is also non-idempotent: the next pass
        // measures from a different column and moves it.
        //
        // At a flow boundary the folded measurement must grade the SAME pairwise welded unit
        // the primary `content_fits` above graded (`[line, word]` glued to a following tag —
        // the word is the separator here, so the unit is word + tag): raw `rest_commands`
        // would measure the tag in its inherited Break mode, short-circuit at its first
        // internal line, report a false fit, and the tag would then break in place mid-line —
        // the tear the welded walk exists to prevent (`fill_multi_expr_travel_long`).
        //
        // Case 1 is deliberately excluded (`offset + 1 < parts.len()`): there the `line` is the
        // fill's last item, a boundary separator to whatever FOLLOWS the fill, and its existing
        // `rest_commands` measurement already asks the right question.
        let content_fits = if offset + 1 < parts.len() && arena.is_collapsible_line(content) {
            let mut with_sep: SmallVec<[ArenaCommand; 8]> = if is_final_segment {
                if context.break_before_wide_flow() {
                    flow_lookahead(arena, rest_commands)
                } else {
                    SmallVec::from_slice(rest_commands)
                }
            } else {
                SmallVec::new()
            };
            with_sep.push(ArenaCommand {
                indent,
                mode: Mode::Flat,
                doc: parts[offset + 1],
            });
            let budget = if final_with_rest {
                remaining
            } else {
                available
            };
            arena_fits_with_lookahead(
                arena,
                content,
                Mode::Flat,
                &with_sep,
                budget as isize,
                has_line_suffix,
                source,
            )
        } else {
            content_fits
        };

        // A short inline element (its own content fits flat) that dropped to its own line — whether
        // pushed there by a preceding break (already at line start) or dropped mid-fill below — no
        // longer isolates its trailing text: it packs like every other fill word so the run flows
        // after it (conformance_prettier_svelte.md §Svelte: Inline content block-style, "a text run flows as
        // one fill"). The at-line-start case falls through to Case 3's `both_fit` flow; the mid-fill
        // case flows via the `after_element_fold`-gated arm in Case 3. A *wide* element that wraps
        // still hugs the dangled `>` (the fold's terminal-tail hug) / owns its line.

        // Case 1: Last item
        if offset + 1 >= parts.len() {
            // A fill built with a LEADING separator (a `leading_line` — Svelte text after an
            // expression tag) shifts the content/separator parity by one, so a fill that also
            // ends in a separator (a `trailing_line` — text before an expression tag) lands its
            // trailing `line` HERE, in the last-item slot, instead of as Case 2's separator. It
            // is a boundary separator to whatever follows the fill, not content: render it by fit
            // exactly as Case 2 does (Flat → the space it stands for when the next node fits,
            // Break → the newline when it doesn't). The generic content path below would instead
            // emit a manual newline+indent AND THEN render the `Line` in Flat mode — a space —
            // stranding a stray leading space at the head of the continuation line (the
            // fill-break-before-an-expression-tag non-idempotency).
            if arena.is_collapsible_line(content) {
                let sep_mode = if content_fits {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                render_single_doc(
                    ctx,
                    content,
                    output,
                    pos,
                    indent,
                    sep_mode,
                    should_remeasure,
                );
                break;
            }
            if !content_fits {
                let line_start_pos = line_start_column(indent, render, embed);
                if *pos != line_start_pos {
                    trim_trailing_whitespace(output);
                    output.push('\n');
                    write_indentation(output, indent, render, embed);
                    *pos = line_start_pos;
                }
                // Unmeasured flat render (tsv shape: prettier uses Break mode
                // here) — the nested groups must measure for themselves, so
                // poison the fits-skip flag for this subtree.
                *should_remeasure = true;
            }
            render_single_doc(
                ctx,
                content,
                output,
                pos,
                indent,
                Mode::Flat,
                should_remeasure,
            );
            break;
        }

        let separator = parts[offset + 1];

        // Case 2: Only content + separator left
        if offset + 2 >= parts.len() {
            // A `line` here is the parity-shifted separator between the last two items (see the
            // `content_fits` correction above, which measured it together with its word). Render
            // it by fit exactly as Case 1 does — Flat is the space it stands for, Break the
            // newline — rather than unconditionally Flat, which would let the tail word overflow.
            let content_is_line = arena.is_collapsible_line(content);
            let content_mode = if content_is_line && !content_fits {
                Mode::Break
            } else {
                Mode::Flat
            };
            if !content_is_line && !content_fits {
                // Unmeasured flat render (see Case 1) — poison the fits-skip. A `line` rendered in
                // Break mode is not an unmeasured flat render and has no nested groups to
                // re-measure, so it does not poison (matching Case 1's guard).
                *should_remeasure = true;
            }
            render_single_doc(
                ctx,
                content,
                output,
                pos,
                indent,
                content_mode,
                should_remeasure,
            );
            // The separator (the last fill item) is rendered between `content` and whatever
            // follows the fill (`rest_commands`). The generic `content_fits` above measures
            // `content` + `rest_commands` but NOT this separator, so a trailing-`line` fill
            // (the `next_is_flow` / after-element-fold boundary — the only fills that reach
            // Case 2, since they alone end in a separator) under-measures by the separator's
            // width and lets the following node overshoot printWidth by a column. Re-measure with
            // the separator counted just before the look-ahead so the boundary breaks (next node
            // to its own line) exactly when it should.
            // The forced-break short-circuit is [`flow_forced_break`], hoisted above Case 1 —
            // the separator must break exactly where Case 1's content would.
            let sep_fits = if flow_forced_break {
                false
            } else if final_with_rest {
                // Inline-backed look-ahead stack plus the separator — matches the render
                // work-list's `N = 8` so the common case stays off the heap (this rare Case-2
                // flow boundary still cloned a `Vec`).
                //
                // At a flow boundary (Svelte text→inline-element/component) the stack is the
                // PAIRWISE one — last word, separator, element (see [`flow_lookahead`]). Scoped by
                // the context flag to the in-flow (`!is_first`) text→element boundary; a
                // first-child text leaves the element bare, which keeps hugging.
                let mut rest_with_sep: SmallVec<[ArenaCommand; 8]> =
                    if context.break_before_wide_flow() {
                        flow_lookahead(arena, rest_commands)
                    } else {
                        SmallVec::from_slice(rest_commands)
                    };
                rest_with_sep.push(ArenaCommand {
                    indent,
                    mode: Mode::Flat,
                    doc: separator,
                });
                arena_fits_with_lookahead(
                    arena,
                    content,
                    Mode::Flat,
                    &rest_with_sep,
                    remaining as isize,
                    has_line_suffix,
                    source,
                )
            } else {
                content_fits
            };
            let sep_mode = if sep_fits { Mode::Flat } else { Mode::Break };
            render_single_doc(
                ctx,
                separator,
                output,
                pos,
                indent,
                sep_mode,
                should_remeasure,
            );
            break;
        }

        // Case 3: Full three-way decision
        let next_content = parts[offset + 2];
        let both_fit = arena_fits_multi(
            arena,
            &[content, separator, next_content],
            available,
            Mode::Flat,
            has_line_suffix,
            source,
        );

        if both_fit {
            render_single_doc(
                ctx,
                content,
                output,
                pos,
                indent,
                Mode::Flat,
                should_remeasure,
            );
            render_single_doc(
                ctx,
                separator,
                output,
                pos,
                indent,
                Mode::Flat,
                should_remeasure,
            );
        } else if content_fits {
            render_single_doc(
                ctx,
                content,
                output,
                pos,
                indent,
                Mode::Flat,
                should_remeasure,
            );
            render_single_doc(
                ctx,
                separator,
                output,
                pos,
                indent,
                Mode::Break,
                should_remeasure,
            );
        } else {
            let line_start_pos = line_start_column(indent, render, embed);
            let at_line_start = *pos == line_start_pos;

            if !at_line_start {
                let remaining_at_start = render.print_width.saturating_sub(line_start_pos);
                let content_fits_at_start = arena_fits_with_lookahead(
                    arena,
                    content,
                    Mode::Flat,
                    &[],
                    remaining_at_start as isize,
                    has_line_suffix,
                    source,
                );

                if context.after_element_fold() && !content_fits_at_start {
                    // The first fill item is a breakable inline element (the after-element fold's
                    // element) sitting mid-line right after a small prefix — the parent inline
                    // element's `>`. It does not fit flat here, and it would not fit on its own line
                    // either (it is wider than printWidth even at line start). Dropping it to the
                    // next line therefore wouldn't help — it would only strand a spurious break
                    // before it (`>⏎<child`, which the next pass collapses → non-idempotent).
                    // Render it in place (it breaks its own attributes/content internally) and break
                    // the separator so the trailing text takes its own line. This keeps the child
                    // hugging the parent's `>`, the same shape the newline-authored boundary lands
                    // on, so both authorings converge.
                    render_single_doc(
                        ctx,
                        content,
                        output,
                        pos,
                        indent,
                        Mode::Break,
                        should_remeasure,
                    );
                    render_single_doc(
                        ctx,
                        separator,
                        output,
                        pos,
                        indent,
                        Mode::Break,
                        should_remeasure,
                    );
                    offset += 2;
                    continue;
                }

                // A collapsible `line` sitting in a CONTENT slot **is** the break — so let it
                // render itself in Break mode rather than emitting a manual newline and then
                // rendering it Flat, which writes the space it stands for at the head of the
                // continuation line (`~{expr}⏎\t ccccc`). The next pass reads that space as
                // indentation and drops it, so the format has no fixed point: the
                // fill-break-before-an-expression-tag non-idempotency Case 1 already guards
                // for the last-item slot, reached here through the generic path instead.
                //
                // A `line` lands in a content slot whenever the fill was built with a LEADING
                // separator (`leading_line` — Svelte text after an expression tag), which
                // shifts the content/separator parity by one, making every `line` a content
                // and every word a separator. Rendering the separator Flat is then just
                // "write the word", the same thing every other arm does with it.
                if arena.is_collapsible_line(content) {
                    render_single_doc(
                        ctx,
                        content,
                        output,
                        pos,
                        indent,
                        Mode::Break,
                        should_remeasure,
                    );
                    render_single_doc(
                        ctx,
                        separator,
                        output,
                        pos,
                        indent,
                        Mode::Flat,
                        should_remeasure,
                    );
                    offset += 2;
                    continue;
                }

                // `glued_lead`: the fill's FIRST item is byte-glued to what precedes it, so the
                // boundary before it carries no whitespace and the fresh-line drop below would
                // INJECT a rendered space (and, since the mangled form is itself a fixed point, F1
                // could never see it). Render it in place — prettier's shape — and break the
                // separator, so the run splits at the first whitespace boundary INSIDE it instead,
                // even when the glued head overruns printWidth. Head only: every later item is
                // separated by real whitespace and keeps the ordinary drop.
                if context.glued_lead() && offset == 0 {
                    render_single_doc(
                        ctx,
                        content,
                        output,
                        pos,
                        indent,
                        Mode::Break,
                        should_remeasure,
                    );
                    render_single_doc(
                        ctx,
                        separator,
                        output,
                        pos,
                        indent,
                        Mode::Break,
                        should_remeasure,
                    );
                    offset += 2;
                    continue;
                }

                trim_trailing_whitespace(output);
                output.push('\n');
                write_indentation(output, indent, render, embed);
                *pos = line_start_pos;

                if content_fits_at_start {
                    render_single_doc(
                        ctx,
                        content,
                        output,
                        pos,
                        indent,
                        Mode::Flat,
                        should_remeasure,
                    );
                    // The Svelte after-element fold's lead element dropped to its own line from
                    // mid-fill (a preceding word pushed it). It fits intact here, so let the trailing
                    // text flow greedily after it — the short inline element packs like any other fill
                    // word (conformance_prettier_svelte.md §Svelte: Inline content block-style, "a text run
                    // flows as one fill"), and the same-line-authored drop converges with the
                    // newline-authored one instead of one flowing and the other isolating (an F1
                    // break).
                    let sep_mode =
                        hug_terminal_sep_mode(ctx, context, next_content, *pos, has_line_suffix);
                    render_single_doc(
                        ctx,
                        separator,
                        output,
                        pos,
                        indent,
                        sep_mode,
                        should_remeasure,
                    );
                } else {
                    render_single_doc(
                        ctx,
                        content,
                        output,
                        pos,
                        indent,
                        Mode::Break,
                        should_remeasure,
                    );
                    render_single_doc(
                        ctx,
                        separator,
                        output,
                        pos,
                        indent,
                        Mode::Break,
                        should_remeasure,
                    );
                }
            } else {
                // Content didn't fit flat at line start; render it (it may break
                // internally) and break the separator so the next item takes its own
                // line. Default across every fill — list-shaped (CSS value lists) and the
                // inline after-element fold alike: a wrapped item does not let the
                // following item hug onto its last line.
                render_single_doc(
                    ctx,
                    content,
                    output,
                    pos,
                    indent,
                    Mode::Break,
                    should_remeasure,
                );
                // Exception (Svelte after-element fold, terminal trailing text): hug the dangled `>`
                // when the tail fits there, else own line — see `hug_terminal_sep_mode`.
                // `next_content` (= `parts[offset + 2]`) is in bounds here: this is the at-line-start
                // arm of Case 3, which Case 2 (`offset + 2 >= parts.len()`) has already excluded.
                let sep_mode =
                    hug_terminal_sep_mode(ctx, context, next_content, *pos, has_line_suffix);
                render_single_doc(
                    ctx,
                    separator,
                    output,
                    pos,
                    indent,
                    sep_mode,
                    should_remeasure,
                );
            }
        }

        offset += 2;
    }
}

/// The look-ahead stack a [`DocContext::break_before_wide_flow`] measurement grades: `rest_commands`
/// reduced to the **pairwise** unit — the immediately following node (the top of the stack, which is
/// consumed back-to-front) plus any welded run behind it.
///
/// Prettier's fill is pairwise — last word, separator, element — so the measurement both starts AND
/// ends at that following node. Two things follow from "ends", and both are load-bearing.
///
/// Measure the node as a WHOLE flat unit (force Flat mode), so the boundary breaks — dropping the
/// element to its own line whole — exactly when prettier's `group([line, element])` would: when the
/// element doesn't fit flat after the last word. Without it the element's inherited Break mode lets
/// `arena_fits` short-circuit at its first internal line, so the element packs onto the text line
/// and breaks its own tag in place.
///
/// And TRUNCATE the stack at the end of that unit. A later sibling this fill reaches only across a
/// break point of its own does not belong in the element's fit check — but it is not always
/// *separated* from the element by one, so the cut is at the first genuine break opportunity, not
/// blindly after the element:
///
/// - A run with a whitespace boundary of its own is EXCLUDED. Its fill leads with a WORD, not a
///   `line`, so a full-stack look-ahead counts that word and breaks BEFORE the element when the real
///   overflow lands after it — isolating the element on its own line while the word it was measured
///   against wraps anyway. Pinned by `tests/fixtures/svelte/elements/fill_inline_pairwise_long`,
///   whose four `<p>`s are the two boundary spellings at 100 and 101.
/// - A **welded** run ([`DocArena::welded_entry`] — a `.` fused to `</a>`, a word held by a
///   non-breaking space) is INCLUDED, and so is any welded run behind it. No break may land in front
///   of it, so it rides the element's line whichever way the boundary resolves and shares its width
///   by construction. Pinned by `inline_break_before_wrap_long`,
///   `inline_break_before_comment_glued_long` and `inline_nbsp_boundary_long`. A welded run reaches
///   through **every glued member** — element, glued text, element, however long the weld runs
///   (`<code>a</code>/<code>b</code>`, `.w<b>yy</b>.z<i>q</i>`): a mid-run element takes the top
///   element's treatment (forced flat) and the walk continues while the next entry is still glued;
///   the run's last element arrives bare, as a sibling join, or as a welded after-element fold,
///   and ends the unit. Pinned by `fill_inline_pairwise_welded_long`,
///   `inline_fold_glued_head_long_prettier_divergence` and
///   `inline_welded_run_travel_long_prettier_divergence`.
///
/// When the following node is an after-element fold (an inline element + its trailing text), measure
/// only the fold's LEAD element — the same rule, since that trailing text is separated by the fold's
/// own `line` and can wrap. A bare following element ([`DocArena::welded_atom`] → `None`) is
/// already the whole unit.
///
/// Both halves of the boundary rule share this, since they differ only in where the separator sits:
/// the space-authored half measures it as Case 2's `sep_fits` (separator counted after this stack),
/// the glued half as Case 1's `content_fits` (no separator at all). The parity-shifted glued half —
/// a `leading_line` fill whose last CONTENT is the `line` and whose word is the separator — reaches
/// it through the collapsible-line `content_fits` correction, which folds that word on top of this
/// stack; measuring the word against raw `rest_commands` instead would let the welded tag's
/// inherited Break mode short-circuit the check and tear the tag mid-line.
fn flow_lookahead(arena: &DocArena, rest_commands: &[ArenaCommand]) -> SmallVec<[ArenaCommand; 8]> {
    let mut out: SmallVec<[ArenaCommand; 8]> = SmallVec::new();
    let Some((&el_cmd, deeper)) = rest_commands.split_last() else {
        return out;
    };
    // The stack is consumed back-to-front: the element is its last entry, and the welded unit
    // extends DOWNWARD from there through `deeper`.
    // A welded TEXT run rides in its OWN mode, deliberately: its head alone is pinned, so its
    // internal whitespace boundaries are ordinary break points and the inherited Break mode
    // stopping at the first of them is the right answer. A welded **atom**
    // ([`DocArena::welded_entry`] — a bare glued element, a glued element run, an after-element
    // fold's lead, or a sibling join's element) is the exception: measured in Break mode it would
    // short-circuit inside the element's own group and report a fit after the open tag alone, so
    // it takes the top element's treatment (forced flat). The walk continues past either kind
    // while the next entry is still glued — a weld can run element, glued text, element
    // (`.w<b>yy</b>.z<i>q</i>`), and ending the unit at the first atom would let its earlier
    // members report a fit that strands the later wide one — and the unit ends at the first
    // entry that is not glued, which sits behind a break opportunity of its own.
    let mut first = deeper.len();
    while first > 0 {
        match arena.welded_entry(deeper[first - 1].doc) {
            WeldedEntry::NotGlued => {
                // Burial tripwire: a marker sitting as this entry's first structural child
                // means a wrapping builder hid it from `welded_entry` (debug builds only).
                #[cfg(debug_assertions)]
                arena.debug_check_buried_welded_marker(deeper[first - 1].doc);
                break;
            }
            WeldedEntry::TextRun | WeldedEntry::Atom(_) => first -= 1,
        }
    }
    for cmd in &deeper[first..] {
        match arena.welded_entry(cmd.doc) {
            WeldedEntry::Atom(a) => out.push(ArenaCommand {
                doc: a,
                mode: Mode::Flat,
                ..*cmd
            }),
            _ => out.push(*cmd),
        }
    }
    out.push(ArenaCommand {
        doc: arena.welded_atom(el_cmd.doc).unwrap_or(el_cmd.doc),
        mode: Mode::Flat,
        ..el_cmd
    });
    out
}

/// Terminal-tail separator mode for the Svelte after-element fold, shared by Case 3's two drop
/// arms (the mid-fill drop and the at-line-start wrapped drop). After the fold's lead element has
/// rendered on its own line, the trailing text hugs the dangled `>` — separator rendered Flat, the
/// one space it stands for — when the next item actually fits at the resulting column (`+ 1` for
/// that space), and takes its own line (Break) otherwise. Gated on the fold via
/// [`DocContext::after_element_fold`]; every non-fold fill keeps the isolating Break, where a
/// wrapped item never lets the next hug its last line.
#[inline]
fn hug_terminal_sep_mode(
    ctx: &RenderCtx<'_>,
    context: &DocContext,
    next_content: DocId,
    pos: usize,
    has_line_suffix: bool,
) -> Mode {
    if context.after_element_fold()
        && arena_fits_with_lookahead(
            ctx.arena,
            next_content,
            Mode::Flat,
            &[],
            ctx.render.print_width.saturating_sub(pos + 1) as isize,
            has_line_suffix,
            ctx.source,
        )
    {
        Mode::Flat
    } else {
        Mode::Break
    }
}
