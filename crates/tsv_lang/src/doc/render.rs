//! Rendering algorithm for converting Doc trees to formatted strings

use crate::PrintConfig;
use smallvec::SmallVec;
use std::collections::HashMap;

use super::fits::{fits_multi, fits_with_lookahead, update_pos_for_text};
use super::helpers::will_break;
use super::types::{Command, Doc, DocContext, GroupId, LineKind, Mode, TextResolver, resolve_text};

/// Strip trailing whitespace from lines with content (matches Prettier's behavior)
///
/// - Lines with non-whitespace content → strip trailing whitespace
/// - Whitespace-only lines IN THE MIDDLE → preserve (template literal blank lines, etc.)
/// - Whitespace-only lines AT THE END → strip (trailing indentation noise)
fn strip_trailing_whitespace(s: String) -> String {
    // Fast path: no trailing whitespace to process
    if !s.lines().any(|line| line.len() != line.trim_end().len()) {
        return s;
    }

    // Single-pass: buffer whitespace-only lines, flush when followed by content
    let mut result = String::with_capacity(s.len());
    let mut pending: SmallVec<[&str; 2]> = SmallVec::new();

    for line in s.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            // Buffer whitespace-only line (might be at end)
            pending.push(line);
        } else {
            // Content line: flush pending ws-only lines (they're in the middle)
            for ws_line in pending.drain(..) {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(ws_line);
            }
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(trimmed);
        }
    }

    // Add trailing newline if: original ended with \n, OR there were trailing ws-only lines
    // (the ws-only lines themselves are discarded, but the newline before them is preserved)
    if s.ends_with('\n') || !pending.is_empty() {
        result.push('\n');
    }
    result
}

//
// Shared rendering helpers
//

/// Render text content and update position
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
    update_pos_for_text(pos, s, tab_width);
}

/// Render a line break (shared logic for all rendering contexts)
///
/// Handles the line break itself and indentation. Does NOT flush line suffix -
/// that must be done by the caller before calling this if needed.
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
        output.push('\n');
        if kind == LineKind::Literal {
            // Literal line: just newline, no indentation (for blank lines)
            *pos = 0;
        } else {
            // Normal line: newline + indentation + alignment spaces
            write_indentation(output, indent_level, align_spaces, config);
            *pos = line_start_column(indent_level, align_spaces, config, base_indent_override);
        }
        true // Did break
    } else {
        // Flat mode: soft line disappears, normal line becomes space
        if kind == LineKind::Normal {
            output.push(' ');
            *pos += 1;
        }
        false // Did not break
    }
}

/// Flush pending line suffix content
///
/// Renders all buffered suffix commands and clears the buffer.
fn flush_line_suffix<R: TextResolver + ?Sized>(
    line_suffix: &mut Vec<Command>,
    output: &mut String,
    pos: &mut usize,
    config: &PrintConfig,
    resolver: Option<&R>,
) {
    if line_suffix.is_empty() {
        return;
    }
    // Process suffix content (reverse to get correct order)
    // Use None for suffix_buffer to avoid infinite recursion
    for suffix_cmd in std::mem::take(line_suffix).into_iter().rev() {
        render_single_doc_inner(
            suffix_cmd.doc,
            output,
            pos,
            suffix_cmd.indent,
            suffix_cmd.mode,
            config,
            resolver,
            None, // No suffix collection when rendering suffix content
            None, // No base indent override
        );
    }
}

/// Convert a Doc tree to a formatted string (starting at column 0)
///
/// Note: This function does not support Symbol text. Use `print_doc_resolved` for docs with symbols.
pub fn print_doc(doc: &Doc, config: &PrintConfig) -> String {
    strip_trailing_whitespace(print_doc_at_column(doc, config, 0))
}

/// Convert a Doc tree to a formatted string with symbol resolution
pub fn print_doc_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    resolver: &R,
) -> String {
    strip_trailing_whitespace(print_doc_with_indent_resolved(doc, config, 0, 0, resolver))
}

/// Convert a Doc tree to a formatted string, starting at a specific column
///
/// Use this when the doc is being inserted into a line that already has content.
/// The `start_column` affects the width calculation for breaking decisions.
///
/// Note: This function does not support Symbol text. Use `print_doc_at_column_resolved` for docs with symbols.
pub fn print_doc_at_column(doc: &Doc, config: &PrintConfig, start_column: usize) -> String {
    strip_trailing_whitespace(print_doc_with_indent(doc, config, start_column, 0))
}

/// Convert a Doc tree to a formatted string, starting at a specific column, with symbol resolution
pub fn print_doc_at_column_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    resolver: &R,
) -> String {
    strip_trailing_whitespace(print_doc_with_indent_resolved(
        doc,
        config,
        start_column,
        0,
        resolver,
    ))
}

/// Convert a Doc tree to a formatted string with both column and indent level specified
///
/// Use this when the doc is being inserted into content that already has both
/// column position and indentation context (e.g., Svelte template expressions).
///
/// Note: This function does not support Symbol text. Use `print_doc_with_indent_resolved` for docs with symbols.
pub fn print_doc_with_indent(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
) -> String {
    // Pre-allocate to avoid early reallocations; most formatted output exceeds 256 chars
    let mut output = String::with_capacity(256);
    let mut pos: usize = start_column;

    // Use command-stack-based rendering with look-ahead (no resolver)
    render_doc_iterative::<dyn TextResolver>(
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        config,
        None,
    );

    strip_trailing_whitespace(output)
}

/// Convert a Doc tree to a formatted string with column, indent level, and symbol resolution
pub fn print_doc_with_indent_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let mut output = String::with_capacity(256);
    let mut pos: usize = start_column;

    // Use command-stack-based rendering with look-ahead
    render_doc_iterative(
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        config,
        Some(resolver),
    );

    strip_trailing_whitespace(output)
}

/// Convert a Doc tree to a formatted string with column, indent level, and symbol resolution,
/// preserving trailing whitespace (for HTML <pre>, <textarea>, etc.)
///
/// Same as `print_doc_with_indent_resolved` but skips trailing whitespace stripping.
/// Use this for whitespace-sensitive contexts like HTML <pre> elements where trailing
/// spaces and tabs are semantically significant.
pub fn print_doc_with_indent_resolved_preserve_whitespace<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let mut output = String::with_capacity(256);
    let mut pos: usize = start_column;

    // Use command-stack-based rendering with look-ahead
    render_doc_iterative(
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        config,
        Some(resolver),
    );

    // Return output WITHOUT stripping trailing whitespace
    output
}

/// Process an IndentIfBreak node by checking group mode and returning the appropriate command
///
/// This helper deduplicates the IndentIfBreak logic used in all rendering functions.
/// Matches Prettier's indentIfBreak handling (printer.js lines 459-482).
///
/// `group_mode_map` is optional - when None (e.g., in Fill rendering), defaults to Flat mode.
#[inline]
fn process_indent_if_break<'a>(
    contents: &'a Doc,
    group_id: GroupId,
    negate: bool,
    group_mode_map: Option<&HashMap<GroupId, Mode>>,
    cmd: &Command<'a>,
) -> Command<'a> {
    // Check if the referenced group broke (default to Flat if not tracked or no map)
    let group_mode = group_mode_map
        .and_then(|map| map.get(&group_id).copied())
        .unwrap_or(Mode::Flat);

    // Determine if we should indent based on group mode and negate flag
    let should_indent = if negate {
        // Negate: indent if group stayed flat
        group_mode == Mode::Flat
    } else {
        // Normal: indent if group broke
        group_mode == Mode::Break
    };

    // Return command with conditional indentation
    if should_indent {
        cmd.indented(contents)
    } else {
        cmd.with_doc(contents)
    }
}

/// Command-stack-based rendering implementation with look-ahead.
///
/// This is the core of the prettier-compatible printer. Instead of recursive
/// calls, we use a command stack. When checking if a group fits, we pass
/// the remaining commands so `fits()` can look ahead at what comes after.
fn render_doc_iterative<R: TextResolver + ?Sized>(
    doc: &Doc,
    output: &mut String,
    pos: &mut usize,
    start_indent_level: usize,
    config: &PrintConfig,
    resolver: Option<&R>,
) {
    // Command stack - process from end (LIFO)
    let mut commands: Vec<Command> = vec![Command {
        indent: start_indent_level,
        mode: Mode::Break,
        doc,
        base_indent_override: None,
        align_spaces: 0,
    }];

    // Buffer for LineSuffix content - flushed before line breaks
    let mut line_suffix: Vec<Command> = Vec::new();

    // Track which groups broke (for indentIfBreak)
    // Matches Prettier's groupModeMap (printer.js line 460)
    let mut group_mode_map: HashMap<GroupId, Mode> = HashMap::new();

    while let Some(cmd) = commands.pop() {
        match cmd.doc {
            Doc::Text(t) => {
                render_text(t, output, pos, config.tab_width, resolver);
            }

            Doc::Line(kind) => {
                let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
                if cmd.mode == Mode::Break || is_hard {
                    flush_line_suffix(&mut line_suffix, output, pos, config, resolver);
                }
                render_line_break(
                    *kind,
                    cmd.mode,
                    cmd.indent,
                    cmd.align_spaces,
                    cmd.base_indent_override,
                    output,
                    pos,
                    config,
                );
            }

            Doc::Indent(inner) => {
                commands.push(cmd.indented(inner));
            }

            Doc::Dedent(inner) => {
                commands.push(cmd.dedented(inner));
            }

            Doc::Align { n, contents } => {
                commands.push(cmd.with_indent(*n, contents));
            }

            Doc::AlignSpaces { spaces, contents } => {
                commands.push(cmd.with_align_spaces(*spaces, contents));
            }

            Doc::Group {
                contents,
                expanded_states,
                id,
                should_break,
            } => {
                // CRITICAL: Check expanded_states BEFORE will_break()
                //
                // Conditional groups (e.g., call expressions with trailing arrow functions)
                // intentionally contain hardlines in state[0] but still need the fits() check
                // to decide between states. If we checked will_break() first, we'd skip the
                // fits() check and always use break mode, breaking the conditional logic.
                //
                // Example: fn('long...', (x) => { body })
                // - state[0]: inline args, arrow body has hardlines
                // - state[1]: break all args
                // We need to check if state[0] fits before falling back to state[1].
                if let Some(states) = expanded_states {
                    // conditionalGroup: try each state until one fits
                    // This is prettier's expandedStates algorithm (printer.js:269-333)
                    //
                    // Prettier's logic:
                    // 1. Try contents (state[0]) in flat mode with fits() check
                    // 2. If it fits → use it
                    // 3. If it doesn't fit → try states[1..n] until one fits
                    // 4. If none fit → use last state in break mode
                    //
                    // Only apply suffix_width when we're still on the "first line" of the expression
                    // (pos >= first_line_offset). Once we've broken to a new line (pos is reset to
                    // just indentation), suffix_width shouldn't affect inner groups.
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining_width = effective_width.saturating_sub(*pos) as isize;

                    // Step 1: Try state[0] (contents) in flat mode
                    let contents_fit = fits_with_lookahead(
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining_width,
                        config,
                        resolver,
                    );

                    let mut chosen_mode: Mode = Mode::Break; // Default

                    if contents_fit {
                        // State[0] fits → use it
                        chosen_mode = Mode::Flat;
                        commands.push(cmd.with_mode(chosen_mode, contents));
                    } else {
                        // contents (state[0]) doesn't fit → try remaining states
                        // Note: expanded_states now contains [state1, state2, ...] (state0 is in contents)
                        let mut found = false;
                        for i in 0..states.len() {
                            if i == states.len() - 1 {
                                // Last state: use in break mode
                                chosen_mode = Mode::Break;
                                commands.push(cmd.with_mode(Mode::Break, &states[i]));
                                found = true;
                                break;
                            }
                            let state_fits = fits_with_lookahead(
                                &states[i],
                                Mode::Flat,
                                &commands,
                                remaining_width,
                                config,
                                resolver,
                            );

                            if state_fits {
                                chosen_mode = Mode::Flat;
                                commands.push(cmd.with_mode(Mode::Flat, &states[i]));
                                found = true;
                                break;
                            }
                        }

                        // No remaining state fit: use last state in break mode (or contents if no states)
                        if !found {
                            chosen_mode = Mode::Break;
                            commands.push(
                                cmd.with_mode(Mode::Break, states.last().unwrap_or(contents)),
                            );
                        }
                    }

                    // Track this group's mode for indentIfBreak (Prettier printer.js line 344)
                    if let Some(group_id) = id {
                        group_mode_map.insert(*group_id, chosen_mode);
                    }
                } else if *should_break || will_break(contents) {
                    // Force Break mode when:
                    // - should_break is true (source prefers expanded)
                    // - contents contain hardline (will definitely break)
                    let chosen_mode = Mode::Break;
                    commands.push(cmd.with_mode(chosen_mode, contents));
                    // Track this group's mode for indentIfBreak
                    if let Some(group_id) = id {
                        group_mode_map.insert(*group_id, chosen_mode);
                    }
                } else {
                    // Regular group: check if content fits - WITH LOOK-AHEAD
                    // Only apply suffix_width when we're still on the "first line" of the expression.
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining_width = effective_width.saturating_sub(*pos) as isize;
                    let fits = fits_with_lookahead(
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining_width,
                        config,
                        resolver,
                    );
                    let chosen_mode = if fits { Mode::Flat } else { Mode::Break };
                    commands.push(cmd.with_mode(chosen_mode, contents));
                    // Track this group's mode for indentIfBreak
                    if let Some(group_id) = id {
                        group_mode_map.insert(*group_id, chosen_mode);
                    }
                }
            }

            Doc::IsolatedGroup { contents } => {
                // IsolatedGroup renders like a regular group but without will_break() check.
                // This prevents internal hardlines from forcing the group into Break mode.
                // Instead, only the fits() width check determines Flat vs Break.
                let suffix = if *pos >= config.first_line_offset {
                    config.suffix_width
                } else {
                    0
                };
                let effective_width = config.print_width.saturating_sub(suffix);
                let remaining_width = effective_width.saturating_sub(*pos) as isize;
                let fits = fits_with_lookahead(
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

            Doc::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if cmd.mode == Mode::Break {
                    break_doc
                } else {
                    flat_doc
                };
                commands.push(cmd.with_doc(chosen));
            }

            Doc::IndentIfBreak {
                contents,
                group_id,
                negate,
            } => {
                // Process using helper - matches Prettier printer.js lines 459-482
                commands.push(process_indent_if_break(
                    contents,
                    *group_id,
                    *negate,
                    Some(&group_mode_map),
                    &cmd,
                ));
            }

            Doc::Concat(docs) => {
                // Push in reverse order (stack is LIFO)
                for doc in docs.iter().rev() {
                    commands.push(cmd.with_doc(doc));
                }
            }

            Doc::Fill(parts) => {
                // Fill needs special handling for greedy packing
                // Pass rest_commands so fill can see trailing content for accurate width
                render_fill_iterative(
                    parts,
                    output,
                    pos,
                    cmd.indent,
                    config,
                    &DocContext::default(),
                    &commands,
                    resolver,
                );
            }

            Doc::WithContext { doc, context } => {
                // Extract context and apply to inner doc
                // Merge override: context's override takes precedence, else inherit from parent
                let merged_override = context.base_indent_override.or(cmd.base_indent_override);
                match doc.as_ref() {
                    Doc::Fill(parts) => {
                        // Apply context when rendering fill
                        // Pass rest_commands so fill can see trailing content
                        render_fill_iterative(
                            parts, output, pos, cmd.indent, config, context, &commands, resolver,
                        );
                    }
                    _ => {
                        // For non-fill docs, propagate the override
                        commands.push(cmd.with_base_override(merged_override, doc.as_ref()));
                    }
                }
            }

            Doc::LineSuffix(inner) => {
                // Buffer this content to be printed at end of line
                line_suffix.push(cmd.with_doc(inner));
            }

            Doc::LineSuffixBoundary => {
                flush_line_suffix(&mut line_suffix, output, pos, config, resolver);
            }

            Doc::BreakParent => {
                // BreakParent is a marker for fits() - no-op during rendering
                // The breaking decision was already made in fits()
            }
        }
    }

    // Flush any remaining line suffix content at end of document
    flush_line_suffix(&mut line_suffix, output, pos, config, resolver);
}

/// Render a fill doc using greedy line packing (iterative version)
///
/// The `rest_commands` parameter allows the fill to see content that follows
/// it in the document tree. This is important for accurate width calculation
/// when the fill is embedded in a larger structure (e.g., expression inside
/// `{#if}...{/if}</a>`).
#[allow(clippy::too_many_arguments)]
fn render_fill_iterative<'a, R: TextResolver + ?Sized>(
    parts: &[Doc],
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    config: &PrintConfig,
    context: &DocContext,
    rest_commands: &[Command<'a>],
    resolver: Option<&R>,
) {
    let mut offset = 0;

    while offset < parts.len() {
        let remaining = config.print_width.saturating_sub(*pos);
        let content = &parts[offset];

        // For intermediate segments, pack greedily to print_width.
        // For final segments (last or second-to-last), consider trailing content.
        let is_final_segment = offset + 2 >= parts.len();

        // Calculate available width for fits check
        let available = if is_final_segment {
            remaining.saturating_sub(context.trailing_reserve)
        } else {
            remaining
        };

        // Use rest_commands for fits check on final segments so the fill can see
        // content that follows (e.g., `}{/if}</a`). For intermediate segments,
        // just check if the content fits in remaining width.
        let content_fits = if is_final_segment && !rest_commands.is_empty() {
            // Check fits with actual trailing content from document tree
            fits_with_lookahead(
                content,
                Mode::Flat,
                rest_commands,
                remaining as isize,
                config,
                resolver,
            )
        } else {
            // Check fits with trailing_reserve for intermediate segments
            fits_with_lookahead(
                content,
                Mode::Flat,
                &[],
                available as isize,
                config,
                resolver,
            )
        };

        // Case 1: Last item - render it (break to new line if it doesn't fit)
        if offset + 1 >= parts.len() {
            // Break to new line first if needed (unless already at line start)
            // Note: Fill content doesn't track align_spaces, so we use 0
            if !content_fits {
                let line_start_pos =
                    line_start_column(indent_level, 0, config, context.base_indent_override);
                if *pos != line_start_pos {
                    output.push('\n');
                    write_indentation(output, indent_level, 0, config);
                    *pos = line_start_pos;
                }
            }
            render_single_doc(
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

        let separator = &parts[offset + 1];

        // Case 2: Only content + separator left (no next content)
        if offset + 2 >= parts.len() {
            render_single_doc(
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
        let next_content = &parts[offset + 2];
        // Check if content + separator + next_content all fit
        let both_fit = fits_multi(
            &[content, separator, next_content],
            available,
            Mode::Flat,
            config,
            resolver,
        );

        if both_fit {
            // Both fit: render content flat, separator flat
            render_single_doc(
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
            // First fits, next doesn't: render content flat, break after
            render_single_doc(
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
            // Neither fits at current position.
            // Check if content would fit at line start after breaking.
            // Note: Fill content doesn't track align_spaces, so we use 0
            let line_start_pos =
                line_start_column(indent_level, 0, config, context.base_indent_override);
            let at_line_start = *pos == line_start_pos;

            if !at_line_start {
                // Check if content fits after breaking to new line
                let remaining_at_start = config.print_width.saturating_sub(line_start_pos);
                let content_fits_at_start = fits_with_lookahead(
                    content,
                    Mode::Flat,
                    &[],
                    remaining_at_start as isize,
                    config,
                    resolver,
                );

                // Break to new line
                output.push('\n');
                write_indentation(output, indent_level, 0, config);
                *pos = line_start_pos;

                if content_fits_at_start {
                    // Content fits at line start - render flat
                    render_single_doc(
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
                    // Content still doesn't fit - render in break mode
                    render_single_doc(
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
                // Already at line start, content doesn't fit - render in break mode
                render_single_doc(
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

/// Render a single doc with specified mode (helper for Fill)
///
/// This is a thin wrapper around `render_single_doc_inner` that manages the suffix buffer.
#[allow(clippy::too_many_arguments)]
fn render_single_doc<R: TextResolver + ?Sized>(
    doc: &Doc,
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    mode: Mode,
    config: &PrintConfig,
    resolver: Option<&R>,
    base_indent_override: Option<usize>,
) {
    let mut line_suffix: Vec<Command> = Vec::new();
    render_single_doc_inner(
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
    // Flush remaining suffix content
    flush_line_suffix(&mut line_suffix, output, pos, config, resolver);
}

/// Unified single-doc renderer with optional suffix handling
///
/// When `suffix_buffer` is Some, collects LineSuffix content and flushes on hard lines.
/// When `suffix_buffer` is None, renders LineSuffix content directly (used for suffix rendering
/// to avoid infinite recursion).
///
/// Group handling also differs:
/// - With suffix buffer: full fits-checking for break decisions
/// - Without suffix buffer: simplified pass-through (for suffix content)
#[allow(clippy::too_many_arguments)]
fn render_single_doc_inner<'a, R: TextResolver + ?Sized>(
    doc: &'a Doc,
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    mode: Mode,
    config: &PrintConfig,
    resolver: Option<&R>,
    suffix_buffer: Option<&mut Vec<Command<'a>>>,
    base_indent_override: Option<usize>,
) {
    let mut commands: Vec<Command> = vec![Command {
        indent: indent_level,
        mode,
        doc,
        base_indent_override,
        align_spaces: 0,
    }];

    // Track whether we're doing full suffix handling
    let tracking_suffix = suffix_buffer.is_some();

    // Unwrap the suffix buffer for use in the loop (or create a dummy that won't be used)
    let mut dummy_suffix: Vec<Command> = Vec::new();
    let line_suffix = suffix_buffer.unwrap_or(&mut dummy_suffix);

    while let Some(cmd) = commands.pop() {
        match cmd.doc {
            Doc::Text(t) => {
                render_text(t, output, pos, config.tab_width, resolver);
            }

            Doc::Line(kind) => {
                // Flush suffix before hard lines only when tracking suffixes
                if tracking_suffix {
                    let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
                    if cmd.mode == Mode::Break || is_hard {
                        flush_line_suffix(line_suffix, output, pos, config, resolver);
                    }
                }
                render_line_break(
                    *kind,
                    cmd.mode,
                    cmd.indent,
                    cmd.align_spaces,
                    cmd.base_indent_override,
                    output,
                    pos,
                    config,
                );
            }

            Doc::Indent(inner) => {
                commands.push(cmd.indented(inner));
            }

            Doc::Dedent(inner) => {
                commands.push(cmd.dedented(inner));
            }

            Doc::Align { n, contents } => {
                commands.push(cmd.with_indent(*n, contents));
            }

            Doc::AlignSpaces { spaces, contents } => {
                commands.push(cmd.with_align_spaces(*spaces, contents));
            }

            Doc::Group {
                contents,
                expanded_states,
                id: _, // Group ID not used in suffix tracking path
                should_break,
            } => {
                if !tracking_suffix {
                    // Simplified Group handling for suffix content - just pass through
                    commands.push(cmd.with_doc(contents));
                } else if let Some(states) = expanded_states {
                    // conditionalGroup within Fill - try contents first, then remaining states
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining = effective_width.saturating_sub(*pos) as isize;

                    // Step 1: Try contents (state0) first
                    if fits_with_lookahead(
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining,
                        config,
                        resolver,
                    ) {
                        commands.push(cmd.with_mode(Mode::Flat, contents));
                    } else {
                        // Step 2: Try remaining states
                        let mut found = false;
                        for (i, state) in states.iter().enumerate() {
                            if i == states.len() - 1 {
                                // Last state: use in break mode
                                commands.push(cmd.with_mode(Mode::Break, state));
                                found = true;
                                break;
                            }
                            if fits_with_lookahead(
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
                        // Fallback: use last state in break mode (or contents if states is empty)
                        if !found {
                            let fallback_doc: &Doc = states.last().unwrap_or(contents);
                            commands.push(cmd.with_mode(Mode::Break, fallback_doc));
                        }
                    }
                } else if *should_break || will_break(contents) {
                    // Force Break mode (see detailed comment in render_doc_iterative)
                    commands.push(cmd.with_mode(Mode::Break, contents));
                } else {
                    // Regular group: check if content fits
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining = effective_width.saturating_sub(*pos) as isize;
                    let chosen_mode = if fits_with_lookahead(
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

            Doc::IsolatedGroup { contents } => {
                if !tracking_suffix {
                    // Simplified IsolatedGroup handling for suffix content - just pass through
                    commands.push(cmd.with_doc(contents));
                } else {
                    // IsolatedGroup renders like a regular group but without will_break() check
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining = effective_width.saturating_sub(*pos) as isize;
                    let chosen_mode = if fits_with_lookahead(
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

            Doc::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if cmd.mode == Mode::Break {
                    break_doc
                } else {
                    flat_doc
                };
                commands.push(cmd.with_doc(chosen));
            }

            Doc::IndentIfBreak {
                contents,
                group_id,
                negate,
            } => {
                // No group tracking in Fill/suffix, defaults to Flat
                commands.push(process_indent_if_break(
                    contents, *group_id, *negate, None, &cmd,
                ));
            }

            Doc::Concat(docs) => {
                for doc in docs.iter().rev() {
                    commands.push(cmd.with_doc(doc));
                }
            }

            Doc::Fill(parts) => {
                // In render_single_doc context, we don't have outer rest_commands
                render_fill_iterative(
                    parts,
                    output,
                    pos,
                    cmd.indent,
                    config,
                    &DocContext::default(),
                    &[],
                    resolver,
                );
            }

            Doc::WithContext { doc, context } => {
                // Extract context and apply to inner doc
                if tracking_suffix {
                    // Full handling: special case for Fill
                    match doc.as_ref() {
                        Doc::Fill(parts) => {
                            // In render_single_doc context, we don't have outer rest_commands
                            render_fill_iterative(
                                parts,
                                output,
                                pos,
                                cmd.indent,
                                config,
                                context,
                                &[],
                                resolver,
                            );
                        }
                        _ => {
                            let merged_override =
                                context.base_indent_override.or(cmd.base_indent_override);
                            commands.push(cmd.with_base_override(merged_override, doc.as_ref()));
                        }
                    }
                } else {
                    // Simplified: just merge override
                    let merged_override = context.base_indent_override.or(cmd.base_indent_override);
                    commands.push(cmd.with_base_override(merged_override, doc.as_ref()));
                }
            }

            Doc::LineSuffix(inner) => {
                if tracking_suffix {
                    line_suffix.push(cmd.with_doc(inner));
                } else {
                    // In suffix rendering, just render the content directly
                    commands.push(cmd.with_doc(inner));
                }
            }

            Doc::LineSuffixBoundary => {
                if tracking_suffix {
                    flush_line_suffix(line_suffix, output, pos, config, resolver);
                }
                // No-op when not tracking suffix
            }

            Doc::BreakParent => {
                // No-op during rendering
            }
        }
    }
}

//
// Utilities
//

/// Write indentation to output
///
/// When `first_line_offset > 0`, we're embedding an expression inline in a larger context
/// (e.g., TypeScript expression inside Svelte block). In this case, wrapped lines need
/// `base_indent_offset` extra indentation to align with the surrounding context.
///
/// When `first_line_offset == 0`, we're formatting a standalone block where the outer
/// context handles indentation (e.g., TypeScript in `<script>`).
fn write_indentation(output: &mut String, level: usize, align_spaces: usize, config: &PrintConfig) {
    // Add base_indent_offset only for inline-embedded expressions (first_line_offset > 0)
    let extra = if config.first_line_offset > 0 {
        config.base_indent_offset
    } else {
        0
    };
    for _ in 0..(level + extra) {
        output.push_str(config.indent);
    }
    // Add alignment spaces after tabs (Prettier-style alignment)
    for _ in 0..align_spaces {
        output.push(' ');
    }
}

/// Calculate width of indentation
fn indent_width(level: usize, config: &PrintConfig) -> usize {
    level * indent_str_width(config.indent, config.tab_width)
}

/// Calculate column position at line start (indent + base offset + align spaces)
///
/// The `base_override` parameter allows overriding `config.base_indent_offset` for specific
/// contexts (e.g., template expressions where the wrapper won't add its usual indentation).
fn line_start_column(
    indent_level: usize,
    align_spaces: usize,
    config: &PrintConfig,
    base_override: Option<usize>,
) -> usize {
    let base = base_override.unwrap_or(config.base_indent_offset);
    indent_width(indent_level, config) + base * config.tab_width + align_spaces
}

/// Calculate visual width of indentation string
fn indent_str_width(indent: &str, tab_width: usize) -> usize {
    indent
        .chars()
        .map(|ch| if ch == '\t' { tab_width } else { 1 })
        .sum()
}
