//! Rendering algorithm for arena-based document trees.

use crate::PrintConfig;
use std::collections::HashMap;

use super::arena::{ArenaCommand, DocArena, DocId, DocNode};
use super::arena_fits::{arena_fits_multi, arena_fits_with_lookahead, update_pos_for_text};
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
    tab_width: usize,
    resolver: Option<&R>,
) {
    let s = resolve_text(text, resolver);
    output.push_str(s);
    match text.cached_width() {
        Some(w) if w == TEXT_WIDTH_HAS_NEWLINE => {
            // Has newline — compute position from last line
            if let Some(last_nl) = s.rfind('\n') {
                *pos = crate::printing::visual_width(&s[last_nl + 1..], tab_width);
            }
        }
        Some(w) => *pos += w as usize, // Common path: no visual_width call
        None => update_pos_for_text(pos, s, tab_width), // Symbol fallback
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
#[allow(clippy::too_many_arguments)]
fn render_line_break(
    kind: LineKind,
    mode: Mode,
    indent_level: usize,
    align_spaces: usize,
    base_indent_override: Option<usize>,
    output: &mut String,
    pos: &mut usize,
    config: &PrintConfig,
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
            write_indentation(output, indent_level, align_spaces, config);
            *pos = line_start_column(indent_level, align_spaces, config, base_indent_override);
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
    config: &PrintConfig,
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
            config,
            resolver,
            None,
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
pub fn arena_print_doc(arena: &DocArena, doc: DocId, config: &PrintConfig) -> String {
    arena_print_doc_at_column(arena, doc, config, 0)
}

/// Convert an arena doc tree to a formatted string with symbol resolution.
pub fn arena_print_doc_resolved<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    config: &PrintConfig,
    resolver: &R,
) -> String {
    arena_print_doc_with_indent_resolved(arena, doc, config, 0, 0, resolver)
}

/// Convert an arena doc tree to a formatted string, starting at a specific column.
pub fn arena_print_doc_at_column(
    arena: &DocArena,
    doc: DocId,
    config: &PrintConfig,
    start_column: usize,
) -> String {
    arena_print_doc_with_indent(arena, doc, config, start_column, 0)
}

/// Convert an arena doc tree to a formatted string at a specific column, with symbol resolution.
pub fn arena_print_doc_at_column_resolved<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    config: &PrintConfig,
    start_column: usize,
    resolver: &R,
) -> String {
    arena_print_doc_with_indent_resolved(arena, doc, config, start_column, 0, resolver)
}

/// Convert an arena doc tree to a formatted string with column and indent level.
pub fn arena_print_doc_with_indent(
    arena: &DocArena,
    doc: DocId,
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
) -> String {
    let mut output = String::with_capacity(256);
    let mut pos: usize = start_column;

    render_doc_iterative::<dyn TextResolver>(
        arena,
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        config,
        None,
    );

    trim_last_line(output)
}

/// Convert an arena doc tree to a formatted string with column, indent, and symbol resolution.
pub fn arena_print_doc_with_indent_resolved<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let mut output = String::with_capacity(256);
    let mut pos: usize = start_column;

    render_doc_iterative(
        arena,
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        config,
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
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let mut output = String::with_capacity(256);
    let mut pos: usize = start_column;

    render_doc_iterative(
        arena,
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        config,
        Some(resolver),
    );

    output
}

//
// Core rendering
//

/// Command-stack-based rendering implementation with look-ahead.
fn render_doc_iterative<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    start_indent_level: usize,
    config: &PrintConfig,
    resolver: Option<&R>,
) {
    let mut commands: Vec<ArenaCommand> = vec![ArenaCommand {
        indent: start_indent_level,
        mode: Mode::Break,
        doc,
        base_indent_override: None,
        align_spaces: 0,
    }];

    let mut line_suffix: Vec<ArenaCommand> = Vec::new();
    let mut group_mode_map: HashMap<GroupId, Mode> = HashMap::new();

    while let Some(cmd) = commands.pop() {
        let nodes = arena.borrow_nodes();
        let children_vec = arena.borrow_children();

        match &nodes[cmd.doc.index()] {
            DocNode::Text(t) => {
                render_text(t, output, pos, config.tab_width, resolver);
            }

            DocNode::Line(kind) => {
                let kind = *kind;
                let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
                drop(nodes);
                drop(children_vec);
                if cmd.mode == Mode::Break || is_hard {
                    flush_line_suffix(arena, &mut line_suffix, output, pos, config, resolver);
                }
                render_line_break(
                    kind,
                    cmd.mode,
                    cmd.indent,
                    cmd.align_spaces,
                    cmd.base_indent_override,
                    output,
                    pos,
                    config,
                );
            }

            DocNode::Indent(inner) => {
                let inner = *inner;
                drop(nodes);
                drop(children_vec);
                commands.push(cmd.indented(inner));
            }

            DocNode::Dedent(inner) => {
                let inner = *inner;
                drop(nodes);
                drop(children_vec);
                commands.push(cmd.dedented(inner));
            }

            DocNode::Align { n, contents } => {
                let n = *n;
                let contents = *contents;
                drop(nodes);
                drop(children_vec);
                commands.push(cmd.with_indent(n, contents));
            }

            DocNode::AlignSpaces { spaces, contents } => {
                let spaces = *spaces;
                let contents = *contents;
                drop(nodes);
                drop(children_vec);
                commands.push(cmd.with_align_spaces(spaces, contents));
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
                drop(nodes);
                drop(children_vec);

                if !expanded_states.is_empty() {
                    // conditionalGroup: try each state until one fits.
                    // Prettier: only use most expanded when group's OWN should_break is true.
                    // Parent mode being Break does NOT skip the fits check — conditional
                    // groups always try flat first, even inside a MODE_BREAK parent.
                    if should_break {
                        // Prettier: if (doc.break) → use most expanded in break mode
                        let children_vec = arena.borrow_children();
                        let states = expanded_states.resolve(&children_vec).to_vec();
                        drop(children_vec);
                        let most_expanded = states.last().copied().unwrap_or(contents);
                        let chosen_mode = Mode::Break;
                        commands.push(cmd.with_mode(chosen_mode, most_expanded));
                        if let Some(group_id) = id {
                            group_mode_map.insert(group_id, chosen_mode);
                        }
                    } else {
                        // Fits check regardless of parent mode — matches Prettier
                        let suffix = if *pos >= config.first_line_offset {
                            config.suffix_width
                        } else {
                            0
                        };
                        let effective_width = config.print_width.saturating_sub(suffix);
                        let remaining_width = effective_width.saturating_sub(*pos) as isize;

                        let contents_fit = arena_fits_with_lookahead(
                            arena,
                            contents,
                            Mode::Flat,
                            &commands,
                            remaining_width,
                            config,
                            resolver,
                        );

                        let mut chosen_mode: Mode = Mode::Break;

                        if contents_fit {
                            chosen_mode = Mode::Flat;
                            commands.push(cmd.with_mode(chosen_mode, contents));
                        } else {
                            let children_vec = arena.borrow_children();
                            let states = expanded_states.resolve(&children_vec).to_vec();
                            drop(children_vec);

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
                                    config,
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
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining_width = effective_width.saturating_sub(*pos) as isize;
                    let fits = arena_fits_with_lookahead(
                        arena,
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining_width,
                        config,
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
                drop(nodes);
                drop(children_vec);

                let suffix = if *pos >= config.first_line_offset {
                    config.suffix_width
                } else {
                    0
                };
                let effective_width = config.print_width.saturating_sub(suffix);
                let remaining_width = effective_width.saturating_sub(*pos) as isize;
                let fits = arena_fits_with_lookahead(
                    arena,
                    contents,
                    Mode::Flat,
                    &commands,
                    remaining_width,
                    config,
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
                drop(nodes);
                drop(children_vec);
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
                drop(nodes);
                drop(children_vec);
                commands.push(process_indent_if_break(
                    contents,
                    group_id,
                    negate,
                    Some(&group_mode_map),
                    &cmd,
                ));
            }

            DocNode::Concat(range) => {
                let kids = range.resolve(&children_vec);
                for &child in kids.iter().rev() {
                    commands.push(cmd.with_doc(child));
                }
                drop(nodes);
                drop(children_vec);
            }

            DocNode::Fill(range) => {
                let parts: Vec<DocId> = range.resolve(&children_vec).to_vec();
                drop(nodes);
                drop(children_vec);
                render_fill_iterative(
                    arena,
                    &parts,
                    output,
                    pos,
                    cmd.indent,
                    config,
                    &DocContext::default(),
                    &commands,
                    resolver,
                );
            }

            DocNode::WithContext { doc, context } => {
                let inner_doc = *doc;
                let context = context.clone();
                let merged_override = context.base_indent_override.or(cmd.base_indent_override);

                // Check if inner is a Fill
                let is_fill = matches!(&nodes[inner_doc.index()], DocNode::Fill(_));

                if is_fill {
                    let fill_range = match &nodes[inner_doc.index()] {
                        DocNode::Fill(range) => *range,
                        _ => unreachable!(),
                    };
                    let parts: Vec<DocId> = fill_range.resolve(&children_vec).to_vec();
                    drop(nodes);
                    drop(children_vec);
                    render_fill_iterative(
                        arena, &parts, output, pos, cmd.indent, config, &context, &commands,
                        resolver,
                    );
                } else {
                    drop(nodes);
                    drop(children_vec);
                    commands.push(cmd.with_base_override(merged_override, inner_doc));
                }
            }

            DocNode::LineSuffix(inner) => {
                let inner = *inner;
                drop(nodes);
                drop(children_vec);
                line_suffix.push(cmd.with_doc(inner));
            }

            DocNode::LineSuffixBoundary => {
                drop(nodes);
                drop(children_vec);
                flush_line_suffix(arena, &mut line_suffix, output, pos, config, resolver);
            }

            DocNode::BreakParent => {
                // No-op during rendering
            }
        }
    }

    flush_line_suffix(arena, &mut line_suffix, output, pos, config, resolver);
}

/// Render a fill doc using greedy line packing (iterative version).
#[allow(clippy::too_many_arguments)]
fn render_fill_iterative<R: TextResolver + ?Sized>(
    arena: &DocArena,
    parts: &[DocId],
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    config: &PrintConfig,
    context: &DocContext,
    rest_commands: &[ArenaCommand],
    resolver: Option<&R>,
) {
    let mut offset = 0;

    while offset < parts.len() {
        let remaining = config.print_width.saturating_sub(*pos);
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
                config,
                resolver,
            )
        } else {
            arena_fits_with_lookahead(
                arena,
                content,
                Mode::Flat,
                &[],
                available as isize,
                config,
                resolver,
            )
        };

        // Case 1: Last item
        if offset + 1 >= parts.len() {
            if !content_fits {
                let line_start_pos =
                    line_start_column(indent_level, 0, config, context.base_indent_override);
                if *pos != line_start_pos {
                    trim_trailing_whitespace(output);
                    output.push('\n');
                    write_indentation(output, indent_level, 0, config);
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
                config,
                resolver,
                context.base_indent_override,
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
                config,
                resolver,
                context.base_indent_override,
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
                config,
                resolver,
                context.base_indent_override,
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
            config,
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
                config,
                resolver,
                context.base_indent_override,
            );
            render_single_doc(
                arena,
                separator,
                output,
                pos,
                indent_level,
                Mode::Flat,
                config,
                resolver,
                context.base_indent_override,
            );
        } else if content_fits {
            render_single_doc(
                arena,
                content,
                output,
                pos,
                indent_level,
                Mode::Flat,
                config,
                resolver,
                context.base_indent_override,
            );
            render_single_doc(
                arena,
                separator,
                output,
                pos,
                indent_level,
                Mode::Break,
                config,
                resolver,
                context.base_indent_override,
            );
        } else {
            let line_start_pos =
                line_start_column(indent_level, 0, config, context.base_indent_override);
            let at_line_start = *pos == line_start_pos;

            if !at_line_start {
                let remaining_at_start = config.print_width.saturating_sub(line_start_pos);
                let content_fits_at_start = arena_fits_with_lookahead(
                    arena,
                    content,
                    Mode::Flat,
                    &[],
                    remaining_at_start as isize,
                    config,
                    resolver,
                );

                trim_trailing_whitespace(output);
                output.push('\n');
                write_indentation(output, indent_level, 0, config);
                *pos = line_start_pos;

                if content_fits_at_start {
                    render_single_doc(
                        arena,
                        content,
                        output,
                        pos,
                        indent_level,
                        Mode::Flat,
                        config,
                        resolver,
                        context.base_indent_override,
                    );
                    render_single_doc(
                        arena,
                        separator,
                        output,
                        pos,
                        indent_level,
                        Mode::Break,
                        config,
                        resolver,
                        context.base_indent_override,
                    );
                } else {
                    render_single_doc(
                        arena,
                        content,
                        output,
                        pos,
                        indent_level,
                        Mode::Break,
                        config,
                        resolver,
                        context.base_indent_override,
                    );
                    render_single_doc(
                        arena,
                        separator,
                        output,
                        pos,
                        indent_level,
                        Mode::Break,
                        config,
                        resolver,
                        context.base_indent_override,
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
                    config,
                    resolver,
                    context.base_indent_override,
                );
                render_single_doc(
                    arena,
                    separator,
                    output,
                    pos,
                    indent_level,
                    Mode::Break,
                    config,
                    resolver,
                    context.base_indent_override,
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
    config: &PrintConfig,
    resolver: Option<&R>,
    base_indent_override: Option<usize>,
) {
    let mut line_suffix: Vec<ArenaCommand> = Vec::new();
    render_single_doc_inner(
        arena,
        doc,
        output,
        pos,
        indent_level,
        mode,
        config,
        resolver,
        Some(&mut line_suffix),
        base_indent_override,
    );
    flush_line_suffix(arena, &mut line_suffix, output, pos, config, resolver);
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
    config: &PrintConfig,
    resolver: Option<&R>,
    suffix_buffer: Option<&mut Vec<ArenaCommand>>,
    base_indent_override: Option<usize>,
) {
    let mut commands: Vec<ArenaCommand> = vec![ArenaCommand {
        indent: indent_level,
        mode,
        doc,
        base_indent_override,
        align_spaces: 0,
    }];

    let tracking_suffix = suffix_buffer.is_some();
    let mut dummy_suffix: Vec<ArenaCommand> = Vec::new();
    let line_suffix = suffix_buffer.unwrap_or(&mut dummy_suffix);

    while let Some(cmd) = commands.pop() {
        let nodes = arena.borrow_nodes();
        let children_vec = arena.borrow_children();

        match &nodes[cmd.doc.index()] {
            DocNode::Text(t) => {
                render_text(t, output, pos, config.tab_width, resolver);
            }

            DocNode::Line(kind) => {
                let kind = *kind;
                drop(nodes);
                drop(children_vec);
                if tracking_suffix {
                    let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
                    if cmd.mode == Mode::Break || is_hard {
                        flush_line_suffix(arena, line_suffix, output, pos, config, resolver);
                    }
                }
                render_line_break(
                    kind,
                    cmd.mode,
                    cmd.indent,
                    cmd.align_spaces,
                    cmd.base_indent_override,
                    output,
                    pos,
                    config,
                );
            }

            DocNode::Indent(inner) => {
                let inner = *inner;
                drop(nodes);
                drop(children_vec);
                commands.push(cmd.indented(inner));
            }

            DocNode::Dedent(inner) => {
                let inner = *inner;
                drop(nodes);
                drop(children_vec);
                commands.push(cmd.dedented(inner));
            }

            DocNode::Align { n, contents } => {
                let n = *n;
                let contents = *contents;
                drop(nodes);
                drop(children_vec);
                commands.push(cmd.with_indent(n, contents));
            }

            DocNode::AlignSpaces { spaces, contents } => {
                let spaces = *spaces;
                let contents = *contents;
                drop(nodes);
                drop(children_vec);
                commands.push(cmd.with_align_spaces(spaces, contents));
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
                drop(nodes);
                drop(children_vec);

                if !tracking_suffix {
                    commands.push(cmd.with_doc(contents));
                } else if !expanded_states.is_empty() {
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining = effective_width.saturating_sub(*pos) as isize;

                    if arena_fits_with_lookahead(
                        arena,
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining,
                        config,
                        resolver,
                    ) {
                        commands.push(cmd.with_mode(Mode::Flat, contents));
                    } else {
                        let children_vec = arena.borrow_children();
                        let states = expanded_states.resolve(&children_vec).to_vec();
                        drop(children_vec);

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
                                config,
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
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining = effective_width.saturating_sub(*pos) as isize;
                    let chosen_mode = if arena_fits_with_lookahead(
                        arena,
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining,
                        config,
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
                drop(nodes);
                drop(children_vec);

                if !tracking_suffix {
                    commands.push(cmd.with_doc(contents));
                } else {
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining = effective_width.saturating_sub(*pos) as isize;
                    let chosen_mode = if arena_fits_with_lookahead(
                        arena,
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining,
                        config,
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
                drop(nodes);
                drop(children_vec);
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
                drop(nodes);
                drop(children_vec);
                commands.push(process_indent_if_break(
                    contents, group_id, negate, None, &cmd,
                ));
            }

            DocNode::Concat(range) => {
                let kids = range.resolve(&children_vec);
                for &child in kids.iter().rev() {
                    commands.push(cmd.with_doc(child));
                }
                drop(nodes);
                drop(children_vec);
            }

            DocNode::Fill(range) => {
                let parts: Vec<DocId> = range.resolve(&children_vec).to_vec();
                drop(nodes);
                drop(children_vec);
                render_fill_iterative(
                    arena,
                    &parts,
                    output,
                    pos,
                    cmd.indent,
                    config,
                    &DocContext::default(),
                    &[],
                    resolver,
                );
            }

            DocNode::WithContext { doc, context } => {
                let inner_doc = *doc;
                let context = context.clone();
                drop(nodes);
                drop(children_vec);

                if tracking_suffix {
                    let nodes = arena.borrow_nodes();
                    let is_fill = matches!(&nodes[inner_doc.index()], DocNode::Fill(_));
                    if is_fill {
                        let fill_range = match &nodes[inner_doc.index()] {
                            DocNode::Fill(range) => *range,
                            _ => unreachable!(),
                        };
                        let children_vec = arena.borrow_children();
                        let parts: Vec<DocId> = fill_range.resolve(&children_vec).to_vec();
                        drop(children_vec);
                        drop(nodes);
                        render_fill_iterative(
                            arena,
                            &parts,
                            output,
                            pos,
                            cmd.indent,
                            config,
                            &context,
                            &[],
                            resolver,
                        );
                    } else {
                        let merged_override =
                            context.base_indent_override.or(cmd.base_indent_override);
                        drop(nodes);
                        commands.push(cmd.with_base_override(merged_override, inner_doc));
                    }
                } else {
                    let merged_override = context.base_indent_override.or(cmd.base_indent_override);
                    commands.push(cmd.with_base_override(merged_override, inner_doc));
                }
            }

            DocNode::LineSuffix(inner) => {
                let inner = *inner;
                drop(nodes);
                drop(children_vec);
                if tracking_suffix {
                    line_suffix.push(cmd.with_doc(inner));
                } else {
                    commands.push(cmd.with_doc(inner));
                }
            }

            DocNode::LineSuffixBoundary => {
                drop(nodes);
                drop(children_vec);
                if tracking_suffix {
                    flush_line_suffix(arena, line_suffix, output, pos, config, resolver);
                }
            }

            DocNode::BreakParent => {}
        }
    }
}

//
// Utilities
//

fn write_indentation(output: &mut String, level: usize, align_spaces: usize, config: &PrintConfig) {
    let extra = if config.first_line_offset > 0 {
        config.base_indent_offset
    } else {
        0
    };
    for _ in 0..(level + extra) {
        output.push_str(config.indent);
    }
    for _ in 0..align_spaces {
        output.push(' ');
    }
}

fn indent_width(level: usize, config: &PrintConfig) -> usize {
    level * indent_str_width(config.indent, config.tab_width)
}

fn line_start_column(
    indent_level: usize,
    align_spaces: usize,
    config: &PrintConfig,
    base_override: Option<usize>,
) -> usize {
    let base = base_override.unwrap_or(config.base_indent_offset);
    indent_width(indent_level, config) + base * config.tab_width + align_spaces
}

fn indent_str_width(indent: &str, tab_width: usize) -> usize {
    indent
        .chars()
        .map(|ch| if ch == '\t' { tab_width } else { 1 })
        .sum()
}
