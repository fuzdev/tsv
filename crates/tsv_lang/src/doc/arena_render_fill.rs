//! Greedy line-packing for `Fill` doc nodes.
//!
//! `render_fill_iterative` is the fill layout algorithm, mutually recursive
//! with `render_single_doc` in `arena_render.rs`.

use smallvec::SmallVec;

use super::arena::{ArenaCommand, DocId, RenderIndent};
use super::arena_fits::{arena_fits_multi, arena_fits_with_lookahead};
use super::arena_render::{
    RenderCtx, line_start_column, render_single_doc, trim_trailing_whitespace, write_indentation,
};
use super::types::{DocContext, Mode};

/// Render a fill doc using greedy line packing (iterative version).
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

        let available = if is_final_segment {
            remaining.saturating_sub(context.trailing_reserve)
        } else {
            remaining
        };

        // A text-run fill glued to a following tag (`… ~{ratio}`) measures its last word ALONE:
        // prettier keeps the tag outside the fill, so the fill never breaks before the word it is
        // glued to (the word stays put, the tag rides past printWidth after it). Suppress the
        // last-item look-ahead so the glued tag doesn't fold into the word's fit check.
        let lookahead_rest: &[ArenaCommand] = if context.trailing_glued_tag && is_final_segment {
            &[]
        } else {
            rest_commands
        };
        let content_fits = if is_final_segment && !lookahead_rest.is_empty() {
            arena_fits_with_lookahead(
                arena,
                content,
                Mode::Flat,
                lookahead_rest,
                remaining as isize,
                embed,
                source,
            )
        } else {
            arena_fits_with_lookahead(
                arena,
                content,
                Mode::Flat,
                &[],
                available as isize,
                embed,
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
        // Case 1 is deliberately excluded (`offset + 1 < parts.len()`): there the `line` is the
        // fill's last item, a boundary separator to whatever FOLLOWS the fill, and its existing
        // `rest_commands` measurement already asks the right question.
        let content_fits = if offset + 1 < parts.len() && arena.is_collapsible_line(content) {
            let mut with_sep: SmallVec<[ArenaCommand; 8]> =
                SmallVec::from_slice(if is_final_segment {
                    lookahead_rest
                } else {
                    &[]
                });
            with_sep.push(ArenaCommand {
                indent,
                mode: Mode::Flat,
                doc: parts[offset + 1],
            });
            let budget = if is_final_segment && !lookahead_rest.is_empty() {
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
                embed,
                source,
            )
        } else {
            content_fits
        };

        // A short inline element (its own content fits flat) that dropped to its own line — whether
        // pushed there by a preceding break (already at line start) or dropped mid-fill below — no
        // longer isolates its trailing text: it packs like every other fill word so the run flows
        // after it (conformance_prettier.md §Svelte: Inline content block-style, "a text run flows as
        // one fill"). The at-line-start case falls through to Case 3's `both_fit` flow; the mid-fill
        // case flows via the `hug_terminal_after_break`-gated arm in Case 3. A *wide* element that
        // wraps still hugs the dangled `>` (`hug_terminal_after_break`) / owns its line.

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
            // (the `next_node_is_flow` / after-element-fold boundary — the only fills that reach
            // Case 2, since they alone end in a separator) under-measures by the separator's
            // width and lets the following node overshoot printWidth by a column. Re-measure with
            // the separator counted just before the look-ahead so the boundary breaks (next node
            // to its own line) exactly when it should.
            let sep_fits = if context.break_before_wide_flow
                && is_final_segment
                && rest_commands
                    .last()
                    .is_some_and(|c| arena.will_break(c.doc))
            {
                // Flow boundary, forced-break element: the following inline element is already
                // multiline (multiline attributes, a block-body event handler, …). Prettier's
                // `group([line, element])` breaks on that forced break and drops the element, so the
                // separator must break here too — a flat-width measurement would short-circuit at the
                // element's hardline and wrongly report a fit (hugging it onto the text line).
                false
            } else if is_final_segment && !rest_commands.is_empty() {
                // Inline-backed copy of the look-ahead stack plus the separator —
                // matches the render work-list's `N = 8` so the common case stays
                // off the heap (this rare Case-2 flow boundary still cloned a `Vec`).
                let mut rest_with_sep: SmallVec<[ArenaCommand; 8]> =
                    SmallVec::from_slice(rest_commands);
                // Flow boundary (Svelte text→inline-element/component): measure the immediately
                // following node — the top of the rest stack, the inline element — as a WHOLE flat
                // unit (force Flat mode), so the separator breaks (dropping the element to its own
                // line whole) exactly when prettier's `group([line, element])` would: when the
                // element doesn't fit flat after the last word + the separator space. Without this,
                // the element's inherited Break mode lets `arena_fits` short-circuit at its first
                // internal line, so the element packs onto the text line and breaks its own tag in
                // place. Scoped by the context flag to the in-flow (`!is_first`) text→element
                // boundary; a first-child text leaves the element bare, which keeps hugging.
                if context.break_before_wide_flow
                    && let Some(next) = rest_with_sep.last_mut()
                {
                    // When the following node is an after-element fold (an inline element + its
                    // trailing text), measure only the fold's LEAD element — the trailing text
                    // can wrap, so a short element should pack after the last word instead of the
                    // whole element+tail unit forcing a drop (prettier's fill is pairwise: last
                    // word, separator, element — never the tail). A bare following element
                    // (`after_element_fold_lead` → `None`) keeps the whole-flat measurement.
                    next.doc = arena.after_element_fold_lead(next.doc).unwrap_or(next.doc);
                    next.mode = Mode::Flat;
                }
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
                    embed,
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
            embed,
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
                    embed,
                    source,
                );

                if context.hug_wide_first && !content_fits_at_start {
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
                    // word (conformance_prettier.md §Svelte: Inline content block-style, "a text run
                    // flows as one fill"), and the same-line-authored drop converges with the
                    // newline-authored one instead of one flowing and the other isolating (an F1
                    // break).
                    let sep_mode = hug_terminal_sep_mode(ctx, context, next_content, *pos);
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
                let sep_mode = hug_terminal_sep_mode(ctx, context, next_content, *pos);
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

/// Terminal-tail separator mode for the Svelte after-element fold, shared by Case 3's two drop
/// arms (the mid-fill drop and the at-line-start wrapped drop). After the fold's lead element has
/// rendered on its own line, the trailing text hugs the dangled `>` — separator rendered Flat, the
/// one space it stands for — when the next item actually fits at the resulting column (`+ 1` for
/// that space), and takes its own line (Break) otherwise. Gated on the fold via
/// [`DocContext::hug_terminal_after_break`]; every non-fold fill keeps the isolating Break, where a
/// wrapped item never lets the next hug its last line.
#[inline]
fn hug_terminal_sep_mode(
    ctx: &RenderCtx<'_>,
    context: &DocContext,
    next_content: DocId,
    pos: usize,
) -> Mode {
    if context.hug_terminal_after_break
        && arena_fits_with_lookahead(
            ctx.arena,
            next_content,
            Mode::Flat,
            &[],
            ctx.render.print_width.saturating_sub(pos + 1) as isize,
            ctx.embed,
            ctx.source,
        )
    {
        Mode::Flat
    } else {
        Mode::Break
    }
}
