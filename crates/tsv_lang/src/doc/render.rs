//! Rendering algorithm for converting Doc trees to formatted strings

use crate::PrintConfig;

use super::fits::{fits_multi, fits_with_lookahead, string_width};
use super::helpers::will_break;
use super::types::{Command, Doc, DocContext, Mode, TextResolver, resolve_text};

/// Convert a Doc tree to a formatted string (starting at column 0)
///
/// Note: This function does not support Symbol text. Use `print_doc_resolved` for docs with symbols.
pub fn print_doc(doc: &Doc, config: &PrintConfig) -> String {
    print_doc_at_column(doc, config, 0)
}

/// Convert a Doc tree to a formatted string with symbol resolution
pub fn print_doc_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    resolver: &R,
) -> String {
    print_doc_with_indent_resolved(doc, config, 0, 0, resolver)
}

/// Convert a Doc tree to a formatted string, starting at a specific column
///
/// Use this when the doc is being inserted into a line that already has content.
/// The `start_column` affects the width calculation for breaking decisions.
///
/// Note: This function does not support Symbol text. Use `print_doc_at_column_resolved` for docs with symbols.
pub fn print_doc_at_column(doc: &Doc, config: &PrintConfig, start_column: usize) -> String {
    print_doc_with_indent(doc, config, start_column, 0)
}

/// Convert a Doc tree to a formatted string, starting at a specific column, with symbol resolution
pub fn print_doc_at_column_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    resolver: &R,
) -> String {
    print_doc_with_indent_resolved(doc, config, start_column, 0, resolver)
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
    let mut output = String::new();
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

    output
}

/// Convert a Doc tree to a formatted string with column, indent level, and symbol resolution
pub fn print_doc_with_indent_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let mut output = String::new();
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

    output
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
    }];

    while let Some(cmd) = commands.pop() {
        match cmd.doc {
            Doc::Text(t) => {
                let s = resolve_text(t, resolver);
                output.push_str(s);
                *pos += string_width(s);
            }

            Doc::Line {
                hard,
                soft,
                literal,
            } => {
                if cmd.mode == Mode::Break || *hard {
                    // Break: emit newline
                    output.push('\n');
                    if *literal {
                        // Literal line: just newline, no indentation (for blank lines)
                        *pos = 0;
                    } else {
                        // Normal line: newline + indentation
                        write_indentation(output, cmd.indent, config);
                        // Account for base_indent_offset (e.g., Svelte wrapper indentation)
                        let base_indent = config.base_indent_offset * config.tab_width;
                        *pos = indent_width(cmd.indent, config) + base_indent;
                    }
                } else {
                    // Flat mode: soft line disappears, regular line becomes space
                    if !*soft {
                        output.push(' ');
                        *pos += 1;
                    }
                }
            }

            Doc::Indent(inner) => {
                commands.push(Command {
                    indent: cmd.indent + 1,
                    mode: cmd.mode,
                    doc: inner,
                });
            }

            Doc::Dedent(inner) => {
                commands.push(Command {
                    indent: cmd.indent.saturating_sub(1),
                    mode: cmd.mode,
                    doc: inner,
                });
            }

            Doc::Group {
                contents,
                expanded_states,
            } => {
                // If contents will definitely break (contains hardline), use break mode
                if will_break(contents) {
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: Mode::Break,
                        doc: contents,
                    });
                } else if let Some(states) = expanded_states {
                    // conditionalGroup: try each state until one fits
                    // This is prettier's expandedStates algorithm (printer.js:288-333)
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

                    // Try each state in flat mode until one fits
                    let mut found = false;
                    for (i, state) in states.iter().enumerate() {
                        if i == states.len() - 1 {
                            // Last state: use in break mode
                            commands.push(Command {
                                indent: cmd.indent,
                                mode: Mode::Break,
                                doc: state,
                            });
                            found = true;
                            break;
                        }
                        let state_fits = fits_with_lookahead(
                            state,
                            Mode::Flat,
                            &commands,
                            remaining_width,
                            config,
                            resolver,
                        );

                        if state_fits {
                            commands.push(Command {
                                indent: cmd.indent,
                                mode: Mode::Flat,
                                doc: state,
                            });
                            found = true;
                            break;
                        }
                    }

                    // Shouldn't happen if states is non-empty, but handle gracefully
                    if !found {
                        commands.push(Command {
                            indent: cmd.indent,
                            mode: Mode::Break,
                            doc: contents,
                        });
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
                    let chosen_mode = if fits_with_lookahead(
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining_width,
                        config,
                        resolver,
                    ) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: chosen_mode,
                        doc: contents,
                    });
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
                commands.push(Command {
                    indent: cmd.indent,
                    mode: cmd.mode,
                    doc: chosen,
                });
            }

            Doc::Concat(docs) => {
                // Push in reverse order (stack is LIFO)
                for doc in docs.iter().rev() {
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: cmd.mode,
                        doc,
                    });
                }
            }

            Doc::Fill(parts) => {
                // Fill needs special handling for greedy packing
                render_fill_iterative(
                    parts,
                    output,
                    pos,
                    cmd.indent,
                    config,
                    &DocContext::default(),
                    resolver,
                );
            }

            Doc::WithContext { doc, context } => {
                // Extract context and apply to inner doc
                match doc.as_ref() {
                    Doc::Fill(parts) => {
                        // Apply context when rendering fill
                        render_fill_iterative(
                            parts, output, pos, cmd.indent, config, context, resolver,
                        );
                    }
                    _ => {
                        // For non-fill docs, just unwrap and continue
                        // (context only affects Fill rendering currently)
                        commands.push(Command {
                            indent: cmd.indent,
                            mode: cmd.mode,
                            doc: doc.as_ref(),
                        });
                    }
                }
            }
        }
    }
}

/// Render a fill doc using greedy line packing (iterative version)
fn render_fill_iterative<R: TextResolver + ?Sized>(
    parts: &[Doc],
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    config: &PrintConfig,
    context: &DocContext,
    resolver: Option<&R>,
) {
    let mut offset = 0;

    while offset < parts.len() {
        let remaining = config.print_width.saturating_sub(*pos);
        let content = &parts[offset];

        // Check if current content fits in flat mode
        // Apply trailing_reserve to prevent packing to exactly printWidth
        let available = remaining.saturating_sub(context.trailing_reserve);
        let content_fits = fits_with_lookahead(
            content,
            Mode::Flat,
            &[],
            available as isize,
            config,
            resolver,
        );

        // Case 1: Last item - render it (break to new line if it doesn't fit)
        if offset + 1 >= parts.len() {
            if content_fits {
                render_single_doc(
                    content,
                    output,
                    pos,
                    indent_level,
                    Mode::Flat,
                    config,
                    resolver,
                );
            } else {
                // Doesn't fit - break to new line first
                output.push('\n');
                write_indentation(output, indent_level, config);
                let base_indent = config.base_indent_offset * config.tab_width;
                *pos = indent_width(indent_level, config) + base_indent;
                render_single_doc(
                    content,
                    output,
                    pos,
                    indent_level,
                    Mode::Flat,
                    config,
                    resolver,
                );
            }
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
            );
            break;
        }

        // Case 3: Full three-way decision
        let next_content = &parts[offset + 2];
        // Check if content + separator + next_content all fit
        // Use the same available width as content_fits check
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
            );
            render_single_doc(
                separator,
                output,
                pos,
                indent_level,
                Mode::Flat,
                config,
                resolver,
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
            );
            render_single_doc(
                separator,
                output,
                pos,
                indent_level,
                Mode::Break,
                config,
                resolver,
            );
        } else {
            // Neither fits: render content break, separator break
            render_single_doc(
                content,
                output,
                pos,
                indent_level,
                Mode::Break,
                config,
                resolver,
            );
            render_single_doc(
                separator,
                output,
                pos,
                indent_level,
                Mode::Break,
                config,
                resolver,
            );
        }

        offset += 2;
    }
}

/// Render a single doc with specified mode (helper for Fill)
fn render_single_doc<R: TextResolver + ?Sized>(
    doc: &Doc,
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    mode: Mode,
    config: &PrintConfig,
    resolver: Option<&R>,
) {
    // Use a small command stack for this single doc
    let mut commands: Vec<Command> = vec![Command {
        indent: indent_level,
        mode,
        doc,
    }];

    while let Some(cmd) = commands.pop() {
        match cmd.doc {
            Doc::Text(t) => {
                let s = resolve_text(t, resolver);
                output.push_str(s);
                *pos += string_width(s);
            }

            Doc::Line {
                hard,
                soft,
                literal,
            } => {
                if cmd.mode == Mode::Break || *hard {
                    output.push('\n');
                    if *literal {
                        *pos = 0;
                    } else {
                        write_indentation(output, cmd.indent, config);
                        let base_indent = config.base_indent_offset * config.tab_width;
                        *pos = indent_width(cmd.indent, config) + base_indent;
                    }
                } else if !*soft {
                    output.push(' ');
                    *pos += 1;
                }
            }

            Doc::Indent(inner) => {
                commands.push(Command {
                    indent: cmd.indent + 1,
                    mode: cmd.mode,
                    doc: inner,
                });
            }

            Doc::Dedent(inner) => {
                commands.push(Command {
                    indent: cmd.indent.saturating_sub(1),
                    mode: cmd.mode,
                    doc: inner,
                });
            }

            Doc::Group {
                contents,
                expanded_states,
            } => {
                // Within Fill, groups still make their own decisions
                // but we pass the outer mode context
                if will_break(contents) {
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: Mode::Break,
                        doc: contents,
                    });
                } else if let Some(states) = expanded_states {
                    // conditionalGroup within Fill - try each state
                    // Only apply suffix_width on the "first line" of the expression.
                    let suffix = if *pos >= config.first_line_offset {
                        config.suffix_width
                    } else {
                        0
                    };
                    let effective_width = config.print_width.saturating_sub(suffix);
                    let remaining = effective_width.saturating_sub(*pos) as isize;
                    let mut found = false;
                    for (i, state) in states.iter().enumerate() {
                        if i == states.len() - 1 {
                            commands.push(Command {
                                indent: cmd.indent,
                                mode: Mode::Break,
                                doc: state,
                            });
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
                            commands.push(Command {
                                indent: cmd.indent,
                                mode: Mode::Flat,
                                doc: state,
                            });
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        commands.push(Command {
                            indent: cmd.indent,
                            mode: Mode::Break,
                            doc: contents,
                        });
                    }
                } else {
                    // Only apply suffix_width on the "first line" of the expression.
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
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: chosen_mode,
                        doc: contents,
                    });
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
                commands.push(Command {
                    indent: cmd.indent,
                    mode: cmd.mode,
                    doc: chosen,
                });
            }

            Doc::Concat(docs) => {
                for doc in docs.iter().rev() {
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: cmd.mode,
                        doc,
                    });
                }
            }

            Doc::Fill(parts) => {
                // Nested fill - recurse with default context
                render_fill_iterative(
                    parts,
                    output,
                    pos,
                    cmd.indent,
                    config,
                    &DocContext::default(),
                    resolver,
                );
            }

            Doc::WithContext { doc, context } => {
                // Extract context and apply to inner doc
                match doc.as_ref() {
                    Doc::Fill(parts) => {
                        // Apply context when rendering fill
                        render_fill_iterative(
                            parts, output, pos, cmd.indent, config, context, resolver,
                        );
                    }
                    _ => {
                        // For non-fill docs, just unwrap and continue
                        commands.push(Command {
                            indent: cmd.indent,
                            mode: cmd.mode,
                            doc: doc.as_ref(),
                        });
                    }
                }
            }
        }
    }
}

// =============================================================================
// Utilities
// =============================================================================

/// Write indentation to output
///
/// When `first_line_offset > 0`, we're embedding an expression inline in a larger context
/// (e.g., TypeScript expression inside Svelte block). In this case, wrapped lines need
/// `base_indent_offset` extra indentation to align with the surrounding context.
///
/// When `first_line_offset == 0`, we're formatting a standalone block where the outer
/// context handles indentation (e.g., TypeScript in `<script>`).
fn write_indentation(output: &mut String, level: usize, config: &PrintConfig) {
    // Add base_indent_offset only for inline-embedded expressions (first_line_offset > 0)
    let extra = if config.first_line_offset > 0 {
        config.base_indent_offset
    } else {
        0
    };
    for _ in 0..(level + extra) {
        output.push_str(config.indent);
    }
}

/// Calculate width of indentation
fn indent_width(level: usize, config: &PrintConfig) -> usize {
    level * indent_str_width(config.indent, config.tab_width)
}

/// Calculate visual width of indentation string
fn indent_str_width(indent: &str, tab_width: usize) -> usize {
    indent
        .chars()
        .map(|ch| if ch == '\t' { tab_width } else { 1 })
        .sum()
}
