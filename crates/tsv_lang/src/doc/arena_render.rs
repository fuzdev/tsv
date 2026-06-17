//! Rendering algorithm for arena-based document trees.

use crate::EmbedContext;
use crate::config::TAB_WIDTH;
use std::collections::HashMap;

use super::arena::{ArenaCommand, DocArena, DocId, DocNode};
use super::arena_fits::{arena_fits_multi, arena_fits_with_lookahead, update_pos_for_text};
use super::render_config::RenderConfig;
use super::types::{
    DocContext, GroupId, LineKind, Mode, TEXT_WIDTH_HAS_NEWLINE, TextResolver, resolve_text,
};

/// Trim trailing whitespace from only the last line of output.
/// Interior lines are already handled by `trim_trailing_whitespace()` in `render_line_break()`.
fn trim_last_line(mut s: String) -> String {
    // Find the last newline — only trim after it (the final line)
    let trim_start = s.rfind('\n').map_or(0, |i| i + 1);
    let trimmed_len = s[trim_start..].trim_end_matches([' ', '\t']).len();
    s.truncate(trim_start + trimmed_len);
    s
}

//
// Shared rendering helpers
//

/// Render text content and update position.
///
/// Uses cached width when available to skip `visual_width()` for the common
/// no-newline case. Still needs `resolve_text()` to get the actual string for output.
#[inline]
fn render_text<R: TextResolver + ?Sized>(
    text: &super::types::DocText,
    output: &mut String,
    pos: &mut usize,
    resolver: Option<&R>,
) {
    let s = resolve_text(text, resolver);
    output.push_str(s);
    match text.cached_width() {
        Some(w) if w == TEXT_WIDTH_HAS_NEWLINE => {
            // Has newline — compute position from last line
            if let Some(last_nl) = s.rfind('\n') {
                *pos = crate::printing::visual_width(&s[last_nl + 1..], TAB_WIDTH);
            }
        }
        Some(w) => *pos += w as usize, // Common path: no visual_width call
        None => update_pos_for_text(pos, s), // Symbol fallback
    }
}

/// Reserved trailing-punctuation width once the printer has crossed
/// `first_line_offset`. Embedding contexts use this to keep the suffix
/// (e.g., `}` after a Svelte template expression) on the same line.
#[inline]
fn effective_suffix_width(pos: usize, embed: &EmbedContext) -> usize {
    if pos >= embed.first_line_offset {
        embed.suffix_width
    } else {
        0
    }
}

/// Trim trailing whitespace (spaces and tabs) from the end of the output buffer.
/// Matches Prettier's `trim()` / `trimIndentation()` — called before each
/// non-literal newline to strip trailing indentation/spaces from code lines.
#[inline]
fn trim_trailing_whitespace(output: &mut String) {
    let trimmed_len = output.trim_end_matches([' ', '\t']).len();
    output.truncate(trimmed_len);
}

/// Render a line break.
#[inline]
fn render_line_break(
    kind: LineKind,
    mode: Mode,
    indent_level: usize,
    output: &mut String,
    pos: &mut usize,
    render: &RenderConfig,
    embed: &EmbedContext,
) -> bool {
    let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
    if mode == Mode::Break || is_hard {
        if kind == LineKind::Literal {
            // Literal line (template literals): preserve trailing whitespace
            output.push('\n');
            *pos = 0;
        } else {
            // Non-literal line: trim trailing whitespace before newline
            // (matches Prettier's trim() call before non-literal newlines)
            trim_trailing_whitespace(output);
            output.push('\n');
            write_indentation(output, indent_level, render, embed);
            *pos = line_start_column(indent_level, render, embed);
        }
        true
    } else if kind == LineKind::Normal {
        output.push(' ');
        *pos += 1;
        false
    } else {
        false
    }
}

/// Flush pending line suffix content.
fn flush_line_suffix<R: TextResolver + ?Sized>(
    arena: &DocArena,
    line_suffix: &mut Vec<ArenaCommand>,
    output: &mut String,
    pos: &mut usize,
    render: &RenderConfig,
    embed: &EmbedContext,
    resolver: Option<&R>,
) {
    if line_suffix.is_empty() {
        return;
    }
    for suffix_cmd in std::mem::take(line_suffix).into_iter().rev() {
        render_single_doc_inner(
            arena,
            suffix_cmd.doc,
            output,
            pos,
            suffix_cmd.indent,
            suffix_cmd.mode,
            render,
            embed,
            resolver,
            None,
        );
    }
}

/// Process an IndentIfBreak node.
#[inline]
fn process_indent_if_break(
    contents: DocId,
    group_id: GroupId,
    negate: bool,
    group_mode_map: Option<&HashMap<GroupId, Mode>>,
    cmd: &ArenaCommand,
) -> ArenaCommand {
    let group_mode = group_mode_map
        .and_then(|map| map.get(&group_id).copied())
        .unwrap_or(Mode::Flat);

    let should_indent = if negate {
        group_mode == Mode::Flat
    } else {
        group_mode == Mode::Break
    };

    if should_indent {
        cmd.indented(contents)
    } else {
        cmd.with_doc(contents)
    }
}

//
// Public API
//

/// Convert an arena doc tree to a formatted string (starting at column 0).
pub fn arena_print_doc(arena: &DocArena, doc: DocId, embed: &EmbedContext) -> String {
    arena_print_doc_at_column(arena, doc, embed, 0)
}

/// Render with effectively infinite print width — every group flattens.
///
/// Used by callers that need to measure a doc's flat-layout width
/// (e.g., template literal type sizing). The renderer still uses
/// [`crate::TAB_WIDTH`] / [`crate::INDENT`].
pub fn arena_print_doc_flat_resolved<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    resolver: &R,
) -> String {
    let render = RenderConfig {
        print_width: usize::MAX / 2,
        ..RenderConfig::default()
    };
    let mut output = String::with_capacity(arena.estimated_output_capacity());
    let mut pos: usize = 0;

    render_doc_iterative(
        arena,
        doc,
        &mut output,
        &mut pos,
        0,
        &render,
        embed,
        Some(resolver),
    );

    trim_last_line(output)
}

/// Convert an arena doc tree to a formatted string, starting at a specific column.
pub fn arena_print_doc_at_column(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
) -> String {
    arena_print_doc_with_indent(arena, doc, embed, start_column, 0)
}

/// Convert an arena doc tree to a formatted string with column and indent level.
pub fn arena_print_doc_with_indent(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
) -> String {
    arena_print_doc_with_indent_and_render(
        arena,
        doc,
        embed,
        start_column,
        start_indent_level,
        &RenderConfig::default(),
    )
}

/// Convert an arena doc tree to a formatted string with column, indent, and symbol resolution.
pub fn arena_print_doc_with_indent_resolved<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let render = RenderConfig::default();
    let mut output = String::with_capacity(arena.estimated_output_capacity());
    let mut pos: usize = start_column;

    render_doc_iterative(
        arena,
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        &render,
        embed,
        Some(resolver),
    );

    trim_last_line(output)
}

/// Convert an arena doc tree, preserving trailing whitespace on the last line
/// (for HTML `<pre>`, `<textarea>`, etc.). Interior non-literal lines are still
/// trimmed inline by `render_line_break`; only the final-line trim is skipped.
pub fn arena_print_doc_with_indent_resolved_preserve_whitespace<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let render = RenderConfig::default();
    let mut output = String::with_capacity(arena.estimated_output_capacity());
    let mut pos: usize = start_column;

    render_doc_iterative(
        arena,
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        &render,
        embed,
        Some(resolver),
    );

    output
}

/// Test-only entry point: render with explicit width/indent overrides.
///
/// Production callers should use [`arena_print_doc`] (which uses
/// [`crate::PRINT_WIDTH`] / [`crate::TAB_WIDTH`] / [`crate::INDENT`]).
pub(crate) fn arena_print_doc_with_indent_and_render(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    render: &RenderConfig,
) -> String {
    let mut output = String::with_capacity(arena.estimated_output_capacity());
    let mut pos: usize = start_column;

    render_doc_iterative::<dyn TextResolver>(
        arena,
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        render,
        embed,
        None,
    );

    trim_last_line(output)
}

//
// Core rendering
//

/// Command-stack-based rendering implementation with look-ahead.
#[allow(clippy::too_many_arguments)]
fn render_doc_iterative<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    start_indent_level: usize,
    render: &RenderConfig,
    embed: &EmbedContext,
    resolver: Option<&R>,
) {
    let mut commands: Vec<ArenaCommand> = vec![ArenaCommand {
        indent: start_indent_level,
        mode: Mode::Break,
        doc,
    }];

    let mut line_suffix: Vec<ArenaCommand> = Vec::new();
    let mut group_mode_map: HashMap<GroupId, Mode> = HashMap::new();

    // Hoist arena borrows out of the loop: the arena is read-only during
    // rendering, so a single immutable borrow held for the whole render
    // avoids the per-iteration dynamic borrow-check cost.
    let nodes_outer = arena.borrow_nodes();
    let children_outer = arena.borrow_children();
    let nodes: &[DocNode] = &nodes_outer;
    let children_vec: &[DocId] = &children_outer;

    // Opt-in diagnostic (`swallow_check` feature): flag a line comment that
    // swallows the content emitted after it on the same physical line. The
    // tracker owns the state machine and is inert when the check is disabled.
    // Compiled out entirely without the feature. See `crate::doc::swallow`.
    #[cfg(feature = "swallow_check")]
    let mut swallow = crate::doc::swallow::SwallowTracker::new();

    while let Some(cmd) = commands.pop() {
        match &nodes[cmd.doc.index()] {
            DocNode::Text(t) => {
                #[cfg(feature = "swallow_check")]
                if swallow.enabled() {
                    let s = resolve_text(t, resolver);
                    swallow.on_text(arena.is_line_comment(cmd.doc), s, output);
                }
                render_text(t, output, pos, resolver);
            }

            DocNode::Line(kind) => {
                let kind = *kind;
                let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
                if cmd.mode == Mode::Break || is_hard {
                    flush_line_suffix(
                        arena,
                        &mut line_suffix,
                        output,
                        pos,
                        render,
                        embed,
                        resolver,
                    );
                }
                // A real newline ends the comment's line → clears the pending swallow.
                let emitted_newline =
                    render_line_break(kind, cmd.mode, cmd.indent, output, pos, render, embed);
                #[cfg(feature = "swallow_check")]
                swallow.on_newline(emitted_newline);
                #[cfg(not(feature = "swallow_check"))]
                let _ = emitted_newline;
            }

            DocNode::Indent(inner) => {
                let inner = *inner;
                commands.push(cmd.indented(inner));
            }

            DocNode::Dedent(inner) => {
                let inner = *inner;
                commands.push(cmd.dedented(inner));
            }

            DocNode::Align { n, contents } => {
                let n = *n;
                let contents = *contents;
                commands.push(cmd.with_indent(n, contents));
            }

            DocNode::Group {
                contents,
                expanded_states,
                id,
                should_break,
            } => {
                let contents = *contents;
                let expanded_states = *expanded_states;
                let id = *id;
                let should_break = *should_break;

                if !expanded_states.is_empty() {
                    // conditionalGroup: try each state until one fits.
                    // Prettier: only use most expanded when group's OWN should_break is true.
                    // Parent mode being Break does NOT skip the fits check — conditional
                    // groups always try flat first, even inside a MODE_BREAK parent.
                    if should_break {
                        // Prettier: if (doc.break) → use most expanded in break mode
                        let states = expanded_states.resolve(children_vec).to_vec();
                        let most_expanded = states.last().copied().unwrap_or(contents);
                        let chosen_mode = Mode::Break;
                        commands.push(cmd.with_mode(chosen_mode, most_expanded));
                        if let Some(group_id) = id {
                            group_mode_map.insert(group_id, chosen_mode);
                        }
                    } else {
                        // Fits check regardless of parent mode — matches Prettier
                        let effective_width = render
                            .print_width
                            .saturating_sub(effective_suffix_width(*pos, embed));
                        let remaining_width = effective_width.saturating_sub(*pos) as isize;

                        let contents_fit = arena_fits_with_lookahead(
                            arena,
                            contents,
                            Mode::Flat,
                            &commands,
                            remaining_width,
                            embed,
                            resolver,
                        );

                        let mut chosen_mode: Mode = Mode::Break;

                        if contents_fit {
                            chosen_mode = Mode::Flat;
                            commands.push(cmd.with_mode(chosen_mode, contents));
                        } else {
                            let states = expanded_states.resolve(children_vec).to_vec();

                            let mut found = false;
                            for i in 0..states.len() {
                                if i == states.len() - 1 {
                                    chosen_mode = Mode::Break;
                                    commands.push(cmd.with_mode(Mode::Break, states[i]));
                                    found = true;
                                    break;
                                }
                                let state_fits = arena_fits_with_lookahead(
                                    arena,
                                    states[i],
                                    Mode::Flat,
                                    &commands,
                                    remaining_width,
                                    embed,
                                    resolver,
                                );
                                if state_fits {
                                    chosen_mode = Mode::Flat;
                                    commands.push(cmd.with_mode(Mode::Flat, states[i]));
                                    found = true;
                                    break;
                                }
                            }

                            if !found {
                                chosen_mode = Mode::Break;
                                commands.push(cmd.with_mode(
                                    Mode::Break,
                                    states.last().copied().unwrap_or(contents),
                                ));
                            }
                        }

                        if let Some(group_id) = id {
                            group_mode_map.insert(group_id, chosen_mode);
                        }
                    } // close else (fits check branch)
                } else if should_break || arena.will_break(contents) {
                    let chosen_mode = Mode::Break;
                    commands.push(cmd.with_mode(chosen_mode, contents));
                    if let Some(group_id) = id {
                        group_mode_map.insert(group_id, chosen_mode);
                    }
                } else {
                    let effective_width = render
                        .print_width
                        .saturating_sub(effective_suffix_width(*pos, embed));
                    let remaining_width = effective_width.saturating_sub(*pos) as isize;
                    let fits = arena_fits_with_lookahead(
                        arena,
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining_width,
                        embed,
                        resolver,
                    );
                    let chosen_mode = if fits { Mode::Flat } else { Mode::Break };
                    commands.push(cmd.with_mode(chosen_mode, contents));
                    if let Some(group_id) = id {
                        group_mode_map.insert(group_id, chosen_mode);
                    }
                }
            }

            DocNode::IsolatedGroup { contents } => {
                let contents = *contents;

                let effective_width = render
                    .print_width
                    .saturating_sub(effective_suffix_width(*pos, embed));
                let remaining_width = effective_width.saturating_sub(*pos) as isize;
                let fits = arena_fits_with_lookahead(
                    arena,
                    contents,
                    Mode::Flat,
                    &commands,
                    remaining_width,
                    embed,
                    resolver,
                );
                let chosen_mode = if fits { Mode::Flat } else { Mode::Break };
                commands.push(cmd.with_mode(chosen_mode, contents));
            }

            DocNode::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if cmd.mode == Mode::Break {
                    *break_doc
                } else {
                    *flat_doc
                };
                commands.push(cmd.with_doc(chosen));
            }

            DocNode::IndentIfBreak {
                contents,
                group_id,
                negate,
            } => {
                let contents = *contents;
                let group_id = *group_id;
                let negate = *negate;
                commands.push(process_indent_if_break(
                    contents,
                    group_id,
                    negate,
                    Some(&group_mode_map),
                    &cmd,
                ));
            }

            DocNode::Concat(range) => {
                let kids = range.resolve(children_vec);
                for &child in kids.iter().rev() {
                    commands.push(cmd.with_doc(child));
                }
            }

            DocNode::Fill(range) => {
                let parts: Vec<DocId> = range.resolve(children_vec).to_vec();
                render_fill_iterative(
                    arena,
                    &parts,
                    output,
                    pos,
                    cmd.indent,
                    render,
                    embed,
                    &DocContext::default(),
                    &commands,
                    resolver,
                );
            }

            DocNode::WithContext { doc, context } => {
                let inner_doc = *doc;
                let context = context.clone();

                if let DocNode::Fill(fill_range) = &nodes[inner_doc.index()] {
                    let parts: Vec<DocId> = fill_range.resolve(children_vec).to_vec();
                    render_fill_iterative(
                        arena, &parts, output, pos, cmd.indent, render, embed, &context, &commands,
                        resolver,
                    );
                } else {
                    commands.push(cmd.with_doc(inner_doc));
                }
            }

            DocNode::LineSuffix(inner) => {
                let inner = *inner;
                line_suffix.push(cmd.with_doc(inner));
            }

            DocNode::LineSuffixBoundary => {
                flush_line_suffix(
                    arena,
                    &mut line_suffix,
                    output,
                    pos,
                    render,
                    embed,
                    resolver,
                );
            }

            DocNode::BreakParent => {
                // No-op during rendering
            }
        }
    }

    flush_line_suffix(
        arena,
        &mut line_suffix,
        output,
        pos,
        render,
        embed,
        resolver,
    );
}

/// Render a fill doc using greedy line packing (iterative version).
#[allow(clippy::too_many_arguments)]
fn render_fill_iterative<R: TextResolver + ?Sized>(
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
            let sep_mode = if content_fits {
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
        }

        offset += 2;
    }
}

/// Render a single doc with specified mode (helper for Fill).
#[allow(clippy::too_many_arguments)]
fn render_single_doc<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    mode: Mode,
    render: &RenderConfig,
    embed: &EmbedContext,
    resolver: Option<&R>,
) {
    let mut line_suffix: Vec<ArenaCommand> = Vec::new();
    render_single_doc_inner(
        arena,
        doc,
        output,
        pos,
        indent_level,
        mode,
        render,
        embed,
        resolver,
        Some(&mut line_suffix),
    );
    flush_line_suffix(
        arena,
        &mut line_suffix,
        output,
        pos,
        render,
        embed,
        resolver,
    );
}

/// Unified single-doc renderer with optional suffix handling.
#[allow(clippy::too_many_arguments)]
fn render_single_doc_inner<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    mode: Mode,
    render: &RenderConfig,
    embed: &EmbedContext,
    resolver: Option<&R>,
    suffix_buffer: Option<&mut Vec<ArenaCommand>>,
) {
    let mut commands: Vec<ArenaCommand> = vec![ArenaCommand {
        indent: indent_level,
        mode,
        doc,
    }];

    let tracking_suffix = suffix_buffer.is_some();
    let mut dummy_suffix: Vec<ArenaCommand> = Vec::new();
    let line_suffix = suffix_buffer.unwrap_or(&mut dummy_suffix);

    // Hoist arena borrows out of the loop: the arena is read-only during
    // rendering, so a single immutable borrow held for the whole render
    // avoids the per-iteration dynamic borrow-check cost.
    let nodes_outer = arena.borrow_nodes();
    let children_outer = arena.borrow_children();
    let nodes: &[DocNode] = &nodes_outer;
    let children_vec: &[DocId] = &children_outer;

    while let Some(cmd) = commands.pop() {
        match &nodes[cmd.doc.index()] {
            DocNode::Text(t) => {
                render_text(t, output, pos, resolver);
            }

            DocNode::Line(kind) => {
                let kind = *kind;
                if tracking_suffix {
                    let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
                    if cmd.mode == Mode::Break || is_hard {
                        flush_line_suffix(arena, line_suffix, output, pos, render, embed, resolver);
                    }
                }
                render_line_break(kind, cmd.mode, cmd.indent, output, pos, render, embed);
            }

            DocNode::Indent(inner) => {
                let inner = *inner;
                commands.push(cmd.indented(inner));
            }

            DocNode::Dedent(inner) => {
                let inner = *inner;
                commands.push(cmd.dedented(inner));
            }

            DocNode::Align { n, contents } => {
                let n = *n;
                let contents = *contents;
                commands.push(cmd.with_indent(n, contents));
            }

            DocNode::Group {
                contents,
                expanded_states,
                id: _,
                should_break,
            } => {
                let contents = *contents;
                let expanded_states = *expanded_states;
                let should_break = *should_break;

                if !tracking_suffix {
                    commands.push(cmd.with_doc(contents));
                } else if !expanded_states.is_empty() {
                    let effective_width = render
                        .print_width
                        .saturating_sub(effective_suffix_width(*pos, embed));
                    let remaining = effective_width.saturating_sub(*pos) as isize;

                    if arena_fits_with_lookahead(
                        arena,
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining,
                        embed,
                        resolver,
                    ) {
                        commands.push(cmd.with_mode(Mode::Flat, contents));
                    } else {
                        let states = expanded_states.resolve(children_vec).to_vec();

                        let mut found = false;
                        for (i, &state) in states.iter().enumerate() {
                            if i == states.len() - 1 {
                                commands.push(cmd.with_mode(Mode::Break, state));
                                found = true;
                                break;
                            }
                            if arena_fits_with_lookahead(
                                arena,
                                state,
                                Mode::Flat,
                                &commands,
                                remaining,
                                embed,
                                resolver,
                            ) {
                                commands.push(cmd.with_mode(Mode::Flat, state));
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            let fallback = states.last().copied().unwrap_or(contents);
                            commands.push(cmd.with_mode(Mode::Break, fallback));
                        }
                    }
                } else if should_break || arena.will_break(contents) {
                    commands.push(cmd.with_mode(Mode::Break, contents));
                } else {
                    let effective_width = render
                        .print_width
                        .saturating_sub(effective_suffix_width(*pos, embed));
                    let remaining = effective_width.saturating_sub(*pos) as isize;
                    let chosen_mode = if arena_fits_with_lookahead(
                        arena,
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining,
                        embed,
                        resolver,
                    ) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    commands.push(cmd.with_mode(chosen_mode, contents));
                }
            }

            DocNode::IsolatedGroup { contents } => {
                let contents = *contents;

                if !tracking_suffix {
                    commands.push(cmd.with_doc(contents));
                } else {
                    let effective_width = render
                        .print_width
                        .saturating_sub(effective_suffix_width(*pos, embed));
                    let remaining = effective_width.saturating_sub(*pos) as isize;
                    let chosen_mode = if arena_fits_with_lookahead(
                        arena,
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining,
                        embed,
                        resolver,
                    ) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    commands.push(cmd.with_mode(chosen_mode, contents));
                }
            }

            DocNode::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if cmd.mode == Mode::Break {
                    *break_doc
                } else {
                    *flat_doc
                };
                commands.push(cmd.with_doc(chosen));
            }

            DocNode::IndentIfBreak {
                contents,
                group_id,
                negate,
            } => {
                let contents = *contents;
                let group_id = *group_id;
                let negate = *negate;
                commands.push(process_indent_if_break(
                    contents, group_id, negate, None, &cmd,
                ));
            }

            DocNode::Concat(range) => {
                let kids = range.resolve(children_vec);
                for &child in kids.iter().rev() {
                    commands.push(cmd.with_doc(child));
                }
            }

            DocNode::Fill(range) => {
                let parts: Vec<DocId> = range.resolve(children_vec).to_vec();
                render_fill_iterative(
                    arena,
                    &parts,
                    output,
                    pos,
                    cmd.indent,
                    render,
                    embed,
                    &DocContext::default(),
                    &[],
                    resolver,
                );
            }

            DocNode::WithContext { doc, context } => {
                let inner_doc = *doc;
                let context = context.clone();

                if tracking_suffix {
                    if let DocNode::Fill(fill_range) = &nodes[inner_doc.index()] {
                        let fill_range = *fill_range;
                        let parts: Vec<DocId> = fill_range.resolve(children_vec).to_vec();
                        render_fill_iterative(
                            arena,
                            &parts,
                            output,
                            pos,
                            cmd.indent,
                            render,
                            embed,
                            &context,
                            &[],
                            resolver,
                        );
                    } else {
                        commands.push(cmd.with_doc(inner_doc));
                    }
                } else {
                    commands.push(cmd.with_doc(inner_doc));
                }
            }

            DocNode::LineSuffix(inner) => {
                let inner = *inner;
                if tracking_suffix {
                    line_suffix.push(cmd.with_doc(inner));
                } else {
                    commands.push(cmd.with_doc(inner));
                }
            }

            DocNode::LineSuffixBoundary => {
                if tracking_suffix {
                    flush_line_suffix(arena, line_suffix, output, pos, render, embed, resolver);
                }
            }

            DocNode::BreakParent => {}
        }
    }
}

//
// Utilities
//

fn write_indentation(
    output: &mut String,
    level: usize,
    render: &RenderConfig,
    embed: &EmbedContext,
) {
    let extra = if embed.first_line_offset > 0 {
        embed.base_indent_offset
    } else {
        0
    };
    for _ in 0..(level + extra) {
        output.push_str(render.indent);
    }
}

fn indent_width(level: usize, render: &RenderConfig) -> usize {
    level * indent_str_width(render.indent)
}

fn line_start_column(indent_level: usize, render: &RenderConfig, embed: &EmbedContext) -> usize {
    indent_width(indent_level, render) + embed.base_indent_offset * TAB_WIDTH
}

fn indent_str_width(indent: &str) -> usize {
    indent
        .chars()
        .map(|ch| if ch == '\t' { TAB_WIDTH } else { 1 })
        .sum()
}
