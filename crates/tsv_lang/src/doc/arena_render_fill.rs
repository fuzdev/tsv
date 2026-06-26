//! Greedy line-packing for `Fill` doc nodes.
//!
//! `render_fill_iterative` is the fill layout algorithm, mutually recursive
//! with `render_single_doc` in `arena_render.rs`.

use crate::EmbedContext;
use smallvec::SmallVec;

use super::arena::{ArenaCommand, DocArena, DocId};
use super::arena_fits::{arena_fits_multi, arena_fits_with_lookahead};
use super::arena_render::{
    line_start_column, render_single_doc, trim_trailing_whitespace, write_indentation,
};
use super::render_config::RenderConfig;
use super::types::{DocContext, Mode, TextResolver};

/// Render a fill doc using greedy line packing (iterative version).
#[allow(clippy::too_many_arguments)]
pub(super) fn render_fill_iterative<R: TextResolver + ?Sized>(
    arena: &DocArena,
    parts: &[DocId],
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    render: &RenderConfig,
    embed: &EmbedContext,
    context: &DocContext,
    rest_commands: &[ArenaCommand],
    resolver: Option<&R>,
) {
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

        let content_fits = if is_final_segment && !rest_commands.is_empty() {
            arena_fits_with_lookahead(
                arena,
                content,
                Mode::Flat,
                rest_commands,
                remaining as isize,
                embed,
                resolver,
            )
        } else {
            arena_fits_with_lookahead(
                arena,
                content,
                Mode::Flat,
                &[],
                available as isize,
                embed,
                resolver,
            )
        };

        // Dropped-first boundary (Svelte after-element fold of a sandwiched inline child): if the
        // first fill item rendered at the start of its line, it was pushed there by a preceding
        // break — it dropped to its own line — so break the separator after it and let the rest of
        // the fill pack from there. A wide inline child that drops owns its line; trailing text
        // wraps to the next line rather than hugging the child's `>`. Scoped by the context flag so
        // greedy fills (text word-wrap, CSS value lists) are unaffected.
        if offset == 0 && context.break_after_dropped_first && offset + 1 < parts.len() {
            let line_start_pos = line_start_column(indent_level, render, embed);
            if *pos == line_start_pos {
                let content_mode = if content_fits {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                render_single_doc(
                    arena,
                    content,
                    output,
                    pos,
                    indent_level,
                    content_mode,
                    render,
                    embed,
                    resolver,
                );
                render_single_doc(
                    arena,
                    parts[offset + 1],
                    output,
                    pos,
                    indent_level,
                    Mode::Break,
                    render,
                    embed,
                    resolver,
                );
                offset += 2;
                continue;
            }
        }

        // Case 1: Last item
        if offset + 1 >= parts.len() {
            if !content_fits {
                let line_start_pos = line_start_column(indent_level, render, embed);
                if *pos != line_start_pos {
                    trim_trailing_whitespace(output);
                    output.push('\n');
                    write_indentation(output, indent_level, render, embed);
                    *pos = line_start_pos;
                }
            }
            render_single_doc(
                arena,
                content,
                output,
                pos,
                indent_level,
                Mode::Flat,
                render,
                embed,
                resolver,
            );
            break;
        }

        let separator = parts[offset + 1];

        // Case 2: Only content + separator left
        if offset + 2 >= parts.len() {
            render_single_doc(
                arena,
                content,
                output,
                pos,
                indent_level,
                Mode::Flat,
                render,
                embed,
                resolver,
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
                    next.mode = Mode::Flat;
                }
                rest_with_sep.push(ArenaCommand {
                    indent: indent_level,
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
                    resolver,
                )
            } else {
                content_fits
            };
            let sep_mode = if sep_fits { Mode::Flat } else { Mode::Break };
            render_single_doc(
                arena,
                separator,
                output,
                pos,
                indent_level,
                sep_mode,
                render,
                embed,
                resolver,
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
            resolver,
        );

        if both_fit {
            render_single_doc(
                arena,
                content,
                output,
                pos,
                indent_level,
                Mode::Flat,
                render,
                embed,
                resolver,
            );
            render_single_doc(
                arena,
                separator,
                output,
                pos,
                indent_level,
                Mode::Flat,
                render,
                embed,
                resolver,
            );
        } else if content_fits {
            render_single_doc(
                arena,
                content,
                output,
                pos,
                indent_level,
                Mode::Flat,
                render,
                embed,
                resolver,
            );
            render_single_doc(
                arena,
                separator,
                output,
                pos,
                indent_level,
                Mode::Break,
                render,
                embed,
                resolver,
            );
        } else {
            let line_start_pos = line_start_column(indent_level, render, embed);
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
                    resolver,
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
                        arena,
                        content,
                        output,
                        pos,
                        indent_level,
                        Mode::Break,
                        render,
                        embed,
                        resolver,
                    );
                    render_single_doc(
                        arena,
                        separator,
                        output,
                        pos,
                        indent_level,
                        Mode::Break,
                        render,
                        embed,
                        resolver,
                    );
                    offset += 2;
                    continue;
                }

                trim_trailing_whitespace(output);
                output.push('\n');
                write_indentation(output, indent_level, render, embed);
                *pos = line_start_pos;

                if content_fits_at_start {
                    render_single_doc(
                        arena,
                        content,
                        output,
                        pos,
                        indent_level,
                        Mode::Flat,
                        render,
                        embed,
                        resolver,
                    );
                    render_single_doc(
                        arena,
                        separator,
                        output,
                        pos,
                        indent_level,
                        Mode::Break,
                        render,
                        embed,
                        resolver,
                    );
                } else {
                    render_single_doc(
                        arena,
                        content,
                        output,
                        pos,
                        indent_level,
                        Mode::Break,
                        render,
                        embed,
                        resolver,
                    );
                    render_single_doc(
                        arena,
                        separator,
                        output,
                        pos,
                        indent_level,
                        Mode::Break,
                        render,
                        embed,
                        resolver,
                    );
                }
            } else {
                // Content didn't fit flat at line start; render it (it may break
                // internally) and break the separator so the next item takes its own
                // line. Default across every fill — list-shaped (CSS value lists) and the
                // inline after-element fold alike: a wrapped item does not let the
                // following item hug onto its last line.
                render_single_doc(
                    arena,
                    content,
                    output,
                    pos,
                    indent_level,
                    Mode::Break,
                    render,
                    embed,
                    resolver,
                );
                // Exception (Svelte after-element fold, terminal trailing text): choose the
                // separator by the *actual resulting column* after the wrapped element. If the next
                // item fits after the dangled `>` (separator rendered flat = one space), hug it
                // there — respecting the author's space boundary — instead of forcing its own line.
                // `next_content` (= `parts[offset + 2]`) is in bounds here: this is the at-line-start
                // arm of Case 3, which Case 2 (`offset + 2 >= parts.len()`) has already excluded.
                let sep_mode = if context.hug_terminal_after_break
                    && arena_fits_with_lookahead(
                        arena,
                        next_content,
                        Mode::Flat,
                        &[],
                        render.print_width.saturating_sub(*pos + 1) as isize,
                        embed,
                        resolver,
                    ) {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                render_single_doc(
                    arena,
                    separator,
                    output,
                    pos,
                    indent_level,
                    sep_mode,
                    render,
                    embed,
                    resolver,
                );
            }
        }

        offset += 2;
    }
}
