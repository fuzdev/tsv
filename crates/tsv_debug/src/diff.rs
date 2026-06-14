//! Diff utilities for comparing text and JSON with colored output

use std::fmt::Write;
use std::io::IsTerminal;
use std::str::FromStr;
use tsv_lang::printing::visual_width;

/// Indentation prefix for diff output lines
const INDENT: &str = "           ";

/// Default tab width for visual width calculations (matches prettier)
const TAB_WIDTH: usize = 2;

/// Only show line widths when they exceed this threshold
const LINE_WIDTH_THRESHOLD: usize = 90;

/// Number of digits needed to display `n` (minimum 1)
pub const fn digit_width(n: usize) -> usize {
    if n == 0 { 1 } else { n.ilog10() as usize + 1 }
}

/// Expand tabs to spaces for consistent display
pub fn expand_tabs(line: &str, tab_width: usize) -> String {
    let spaces: String = " ".repeat(tab_width);
    line.replace('\t', &spaces)
}

/// Labels for diff sources to clarify what +/- mean
#[derive(Debug, Clone, Copy)]
pub struct DiffLabels {
    /// Term for lines only in left/first source (shown with -)
    /// e.g., "ours-only", "missing", "original-only"
    pub left_term: &'static str,
    /// Term for lines only in right/second source (shown with +)
    /// e.g., "prettier-only", "extra", "formatted-only"
    pub right_term: &'static str,
}

impl DiffLabels {
    /// Labels for compare command (ours vs prettier)
    pub const fn compare() -> Self {
        Self {
            left_term: "ours-only",
            right_term: "prettier-only",
        }
    }

    /// Labels for ast_diff command (original vs formatted)
    pub const fn ast_diff() -> Self {
        Self {
            left_term: "original-only",
            right_term: "formatted-only",
        }
    }

    /// Labels for idempotency checks (formatted-actual vs input-expected)
    ///
    /// Uses standard testing terminology (actual vs expected):
    /// - formatted-actual: what our formatter produces
    /// - input-expected: the target output (input file must format to itself)
    pub const fn idempotency() -> Self {
        Self {
            left_term: "formatted-actual",
            right_term: "input-expected",
        }
    }

    /// Labels for freshness checks (stored file vs regenerated)
    ///
    /// Used when checking if stored files (expected.json, output_prettier.svelte)
    /// match what the canonical tools currently produce.
    pub const fn freshness() -> Self {
        Self {
            left_term: "stored-only",
            right_term: "current-only",
        }
    }

    /// Labels for prettier behavior checks
    ///
    /// Used when checking prettier's behavior (quirk preservation, variant normalization).
    pub const fn prettier_behavior() -> Self {
        Self {
            left_term: "expected-only",
            right_term: "prettier-only",
        }
    }

    /// Labels for input vs prettier checks
    ///
    /// Used when checking that input file matches prettier's output.
    pub const fn input_vs_prettier() -> Self {
        Self {
            left_term: "input-only",
            right_term: "prettier-only",
        }
    }
}

/// Color output choice
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    /// Automatically detect (use TTY detection)
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

impl ColorChoice {
    /// Determine if colors should be used
    pub fn use_color(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => supports_color(),
        }
    }
}

impl FromStr for ColorChoice {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(format!(
                "invalid color choice: {s} (expected auto, always, or never)"
            )),
        }
    }
}

/// ANSI terminal colors
#[derive(Clone, Copy)]
pub enum Color {
    Red,
    Green,
    Cyan,
}

impl Color {
    /// ANSI escape code for this color
    pub const fn code(self) -> &'static str {
        match self {
            Self::Red => "\x1b[31m",
            Self::Green => "\x1b[32m",
            Self::Cyan => "\x1b[36m",
        }
    }

    /// ANSI reset code
    pub const fn reset() -> &'static str {
        "\x1b[0m"
    }
}

/// Check if stdout/stderr supports colors
///
/// Respects standard environment variables:
/// - NO_COLOR: When set (any value), disables colors
/// - FORCE_COLOR: When set (any value), forces colors even if not a TTY
fn supports_color() -> bool {
    // Respect NO_COLOR (https://no-color.org/)
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Respect FORCE_COLOR
    if std::env::var("FORCE_COLOR").is_ok() {
        return true;
    }

    // Default: check if stderr is a TTY
    std::io::stderr().is_terminal()
}

/// Diff configuration options
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // Configuration struct with clear field names
pub struct DiffOptions {
    /// Number of context lines to show around changes (None = show all)
    pub context_lines: Option<usize>,
    /// Show summary line (e.g., "-3 ours-only, +2 prettier-only")
    pub show_summary: bool,
    /// Show unified diff header (e.g., "@@ -10,7 +10,8 @@")
    pub show_header: bool,
    /// Show inline/word-level diffs within changed lines
    pub inline_diff: bool,
    /// Show JSON paths for changes (e.g., "$.children[0].name")
    pub show_json_paths: bool,
    /// Color choice (auto, always, never)
    pub color_choice: ColorChoice,
    /// Enable colored output (computed from color_choice)
    pub color: bool,
    /// Labels for diff sources (None = use generic "deletions"/"insertions")
    pub labels: Option<DiffLabels>,
}

impl Default for DiffOptions {
    fn default() -> Self {
        let color_choice = ColorChoice::Auto;
        Self {
            context_lines: None,
            show_summary: false,
            show_header: false,
            inline_diff: false,
            show_json_paths: false,
            color_choice,
            color: color_choice.use_color(),
            labels: None,
        }
    }
}

impl DiffOptions {
    /// Base verbose diff options (all lines, inline diff, JSON paths)
    fn verbose(labels: DiffLabels) -> Self {
        let color_choice = ColorChoice::Auto;
        Self {
            context_lines: None,
            show_summary: true,
            show_header: false,
            inline_diff: true,
            show_json_paths: true,
            color_choice,
            color: color_choice.use_color(),
            labels: Some(labels),
        }
    }

    /// Compact diff options (limited context, inline diff, no JSON paths)
    fn compact(labels: DiffLabels) -> Self {
        let color_choice = ColorChoice::Auto;
        Self {
            context_lines: Some(3),
            show_summary: true,
            show_header: false,
            inline_diff: true,
            show_json_paths: false,
            color_choice,
            color: color_choice.use_color(),
            labels: Some(labels),
        }
    }

    /// Create options suitable for the compare command
    pub fn compare() -> Self {
        Self::compact(DiffLabels::compare()).with_header()
    }

    /// Create options suitable for ast_diff command
    pub fn ast_diff() -> Self {
        Self::verbose(DiffLabels::ast_diff())
    }

    /// Create options for idempotency checks (formatted vs input file)
    pub fn idempotency() -> Self {
        Self::compact(DiffLabels::idempotency())
    }

    /// Create options for freshness checks (stored file vs regenerated)
    pub fn freshness() -> Self {
        Self::verbose(DiffLabels::freshness())
    }

    /// Create options for prettier behavior checks
    pub fn prettier_behavior() -> Self {
        Self::verbose(DiffLabels::prettier_behavior()).without_json_paths()
    }

    /// Create options for input vs prettier checks
    pub fn input_vs_prettier() -> Self {
        Self::verbose(DiffLabels::input_vs_prettier()).without_json_paths()
    }

    /// Disable JSON path annotations
    fn without_json_paths(mut self) -> Self {
        self.show_json_paths = false;
        self
    }

    /// Enable hunk headers (e.g., "@@ -10,7 +10,8 @@")
    fn with_header(mut self) -> Self {
        self.show_header = true;
        self
    }

    /// Set color choice and update color flag
    pub fn with_color_choice(mut self, choice: ColorChoice) -> Self {
        self.color_choice = choice;
        self.color = choice.use_color();
        self
    }
}

/// Print a colored diff with custom options
pub fn print_diff_with_options(label: &str, expected: &str, actual: &str, options: &DiffOptions) {
    eprint!(
        "{}",
        render_diff_with_options(label, expected, actual, options)
    );
}

/// Render a labeled diff to a string — the buffered variant of
/// `print_diff_with_options`, for callers that run concurrently (fixture
/// validation) and must not interleave output from different fixtures.
pub fn render_diff_with_options(
    label: &str,
    expected: &str,
    actual: &str,
    options: &DiffOptions,
) -> String {
    format!(
        "\n{INDENT}{label}:\n{}",
        diff_to_string(expected, actual, options)
    )
}

/// Generate a diff string (returned, not printed)
pub fn diff_to_string(expected: &str, actual: &str, options: &DiffOptions) -> String {
    // Try to parse as JSON and add path annotations if requested
    let (expected_formatted, actual_formatted, is_json) = match (
        serde_json::from_str::<serde_json::Value>(expected),
        serde_json::from_str::<serde_json::Value>(actual),
    ) {
        (Ok(exp_json), Ok(act_json)) => (
            serde_json::to_string_pretty(&exp_json).unwrap_or_else(|_| expected.to_string()),
            serde_json::to_string_pretty(&act_json).unwrap_or_else(|_| actual.to_string()),
            true,
        ),
        _ => (expected.to_string(), actual.to_string(), false),
    };

    let diff = similar::TextDiff::from_lines(&expected_formatted, &actual_formatted);
    let mut output = String::new();

    // Collect changes with line numbers
    let changes: Vec<_> = diff.iter_all_changes().collect();

    // Count insertions and deletions for summary
    let mut insertions = 0;
    let mut deletions = 0;
    for change in &changes {
        match change.tag() {
            similar::ChangeTag::Delete => deletions += 1,
            similar::ChangeTag::Insert => insertions += 1,
            similar::ChangeTag::Equal => {}
        }
    }

    // Show summary at top if requested
    if options.show_summary && (insertions > 0 || deletions > 0) {
        let (left_term, right_term) = if let Some(labels) = options.labels {
            (labels.left_term, labels.right_term)
        } else {
            ("deletions", "insertions")
        };

        if options.color {
            let green = Color::Green.code();
            let red = Color::Red.code();
            let reset = Color::reset();
            let _ = writeln!(
                output,
                "{INDENT}{red}-{deletions} {left_term}{reset}, {green}+{insertions} {right_term}{reset}"
            );
        } else {
            let _ = writeln!(
                output,
                "{INDENT}-{deletions} {left_term}, +{insertions} {right_term}"
            );
        }
        output.push('\n');
    }

    // Apply context filtering if requested
    let filtered_lines: Vec<DiffLine> = if let Some(context) = options.context_lines {
        apply_context_filter(&changes, context, options.show_header)
    } else {
        changes.iter().map(|c| DiffLine::Change(*c)).collect()
    };

    // Build JSON path map if enabled
    let path_map = if is_json && options.show_json_paths {
        build_json_path_map(&expected_formatted)
    } else {
        std::collections::HashMap::new()
    };

    // First pass: find max visual line width among changed lines exceeding threshold (for suffix alignment)
    let mut max_visual_width = 0usize;
    for line in &filtered_lines {
        if let DiffLine::Change(change) = line
            && !matches!(change.tag(), similar::ChangeTag::Equal)
        {
            let line_content = change.value().trim_end_matches('\n');
            let width = visual_width(line_content, TAB_WIDTH);
            if width > LINE_WIDTH_THRESHOLD {
                max_visual_width = max_visual_width.max(width);
            }
        }
    }
    let num_width = digit_width(max_visual_width);

    // Generate diff output with inline diffs if enabled
    let reset = Color::reset();
    let cyan = Color::Cyan.code();

    let mut i = 0;
    while i < filtered_lines.len() {
        // Show JSON path for changed lines if enabled
        if let DiffLine::Change(change) = &filtered_lines[i]
            && is_json
            && options.show_json_paths
            && !matches!(change.tag(), similar::ChangeTag::Equal)
            && let Some(line_num) = change.old_index().or_else(|| change.new_index())
            && let Some(path) = path_map.get(&line_num)
        {
            if options.color {
                let _ = writeln!(output, "{INDENT}{cyan}{path}{reset}");
            } else {
                let _ = writeln!(output, "{INDENT}{path}");
            }
        }

        match &filtered_lines[i] {
            DiffLine::Gap => {
                if options.color {
                    let _ = writeln!(output, "{INDENT}{cyan}...{reset}");
                } else {
                    let _ = writeln!(output, "{INDENT}...");
                }
                i += 1;
            }
            DiffLine::HunkHeader {
                old_start,
                old_count,
                new_start,
                new_count,
            } => {
                if options.color {
                    let _ = writeln!(
                        output,
                        "{INDENT}{cyan}@@ -{old_start},{old_count} +{new_start},{new_count} @@{reset}"
                    );
                } else {
                    let _ = writeln!(
                        output,
                        "{INDENT}@@ -{old_start},{old_count} +{new_start},{new_count} @@"
                    );
                }
                i += 1;
            }
            DiffLine::Change(change) => {
                // Check if this is a delete followed by an insert (line replacement)
                let is_replacement = if let similar::ChangeTag::Delete = change.tag() {
                    i + 1 < filtered_lines.len()
                        && matches!(
                            filtered_lines[i + 1],
                            DiffLine::Change(c) if matches!(c.tag(), similar::ChangeTag::Insert)
                        )
                } else {
                    false
                };

                if options.inline_diff && is_replacement {
                    // Show inline diff for the replacement
                    if let DiffLine::Change(insert_change) = &filtered_lines[i + 1] {
                        write_inline_diff(
                            &mut output,
                            change.value(),
                            insert_change.value(),
                            options,
                            max_visual_width,
                            num_width,
                        );
                        i += 2; // Skip both delete and insert
                        continue;
                    }
                }

                // Regular line output
                let (sign, color) = match change.tag() {
                    similar::ChangeTag::Delete => ("-", Some(Color::Red)),
                    similar::ChangeTag::Insert => ("+", Some(Color::Green)),
                    similar::ChangeTag::Equal => (" ", None),
                };

                let line_content = change.value().trim_end_matches('\n');
                let width = visual_width(line_content, TAB_WIDTH);
                // Expand tabs so terminal display matches our width calculation
                let display_content = expand_tabs(line_content, TAB_WIDTH);

                // For changed lines exceeding threshold, show visual width as right-aligned suffix
                if let Some(c) = color {
                    let code = c.code();
                    if width > LINE_WIDTH_THRESHOLD {
                        // Right-aligned suffix with at least 2 spaces padding
                        let padding = max_visual_width.saturating_sub(width) + 2;
                        if options.color {
                            let _ = writeln!(
                                output,
                                "{INDENT}{code}{sign}{display_content}{:padding$}{width:>num_width$}{reset}",
                                ""
                            );
                        } else {
                            let _ = writeln!(
                                output,
                                "{INDENT}{sign}{display_content}{:padding$}{width:>num_width$}",
                                ""
                            );
                        }
                    } else {
                        // No width suffix for lines at or below threshold
                        if options.color {
                            let _ =
                                writeln!(output, "{INDENT}{code}{sign}{display_content}{reset}");
                        } else {
                            let _ = writeln!(output, "{INDENT}{sign}{display_content}");
                        }
                    }
                } else {
                    // Unchanged line: no width suffix
                    let _ = writeln!(output, "{INDENT} {display_content}");
                }
                i += 1;
            }
        }
    }

    output
}

/// Build a map from line numbers to JSON paths
#[allow(clippy::expect_used)] // path_stack always has root "$", empty is a bug
fn build_json_path_map(json_str: &str) -> std::collections::HashMap<usize, String> {
    let mut map = std::collections::HashMap::new();
    let mut path_stack: Vec<String> = vec!["$".to_string()];
    let mut in_array = Vec::new();
    let mut array_indices = Vec::new();

    for (line_num, line) in json_str.lines().enumerate() {
        let trimmed = line.trim();

        // Detect array start
        if trimmed.ends_with('[') {
            in_array.push(true);
            array_indices.push(0);
        }

        // Detect object start
        if trimmed.ends_with('{') && !in_array.last().copied().unwrap_or(false) {
            in_array.push(false);
        }

        // Extract key from lines like '"key":' or '"key": {'
        if let Some(key) = extract_json_key(trimmed) {
            let current_path = path_stack.last().expect("path_stack initialized with root");
            let new_path = format!("{current_path}.{key}");
            map.insert(line_num, new_path.clone());

            // If this line ends with { or [, push to stack
            if trimmed.ends_with('{') || trimmed.ends_with('[') {
                path_stack.push(new_path);
            }
        } else if in_array.last().copied().unwrap_or(false)
            && !trimmed.starts_with('}')
            && !trimmed.starts_with(']')
        {
            // Array element
            let current_path = path_stack.last().expect("path_stack initialized with root");
            let idx = array_indices.last().copied().unwrap_or(0);
            let new_path = format!("{current_path}[{idx}]");
            map.insert(line_num, new_path.clone());

            if trimmed.ends_with('{') || trimmed.ends_with('[') {
                path_stack.push(new_path);
            }

            if let Some(last_idx) = array_indices.last_mut()
                && trimmed.ends_with(',')
            {
                *last_idx += 1;
            }
        }

        // Handle closing brackets
        if trimmed == "}" || trimmed == "}," || trimmed == "]" || trimmed == "]," {
            if path_stack.len() > 1 {
                path_stack.pop();
            }
            if trimmed.starts_with(']') {
                in_array.pop();
                array_indices.pop();
            } else if !in_array.last().copied().unwrap_or(false) {
                in_array.pop();
            }
        }
    }

    map
}

/// Extract JSON key from a line like '"key": value' or '"key": {'
fn extract_json_key(line: &str) -> Option<String> {
    let line = line.trim();
    if line.starts_with('"')
        && let Some(end_quote) = line[1..].find('"')
    {
        let key = &line[1..=end_quote];
        return Some(key.to_string());
    }
    None
}

/// Write an inline diff showing character-level changes between two lines
fn write_inline_diff(
    output: &mut String,
    old_line: &str,
    new_line: &str,
    options: &DiffOptions,
    max_visual_width: usize,
    num_width: usize,
) {
    let reset = Color::reset();
    let red = Color::Red.code();
    let green = Color::Green.code();
    let red_bg = "\x1b[41m"; // Red background for deleted chars
    let green_bg = "\x1b[42m"; // Green background for inserted chars

    // Expand tabs for consistent display, then compute visual width and diff
    let old_trimmed = old_line.trim_end();
    let new_trimmed = new_line.trim_end();
    let old_expanded = expand_tabs(old_trimmed, TAB_WIDTH);
    let new_expanded = expand_tabs(new_trimmed, TAB_WIDTH);
    let old_width = old_expanded.len(); // After expansion, len == visual width
    let new_width = new_expanded.len();
    let char_diff = similar::TextDiff::from_chars(&old_expanded, &new_expanded);

    // Build the old line with highlights
    let mut old_highlighted = INDENT.to_string();
    if options.color {
        old_highlighted.push_str(red);
    }
    old_highlighted.push('-');

    for change in char_diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Delete => {
                if options.color {
                    old_highlighted.push_str(red_bg);
                    old_highlighted.push_str(change.value());
                    old_highlighted.push_str(red);
                } else {
                    old_highlighted.push_str(change.value());
                }
            }
            similar::ChangeTag::Equal => {
                old_highlighted.push_str(change.value());
            }
            similar::ChangeTag::Insert => {} // Skip insertions in old line
        }
    }

    // Add right-aligned suffix only if exceeds threshold
    if old_width > LINE_WIDTH_THRESHOLD {
        let padding = max_visual_width.saturating_sub(old_width) + 2;
        for _ in 0..padding {
            old_highlighted.push(' ');
        }
        let _ = write!(old_highlighted, "{old_width:>num_width$}");
    }

    if options.color {
        old_highlighted.push_str(reset);
    }
    let _ = writeln!(output, "{old_highlighted}");

    // Build the new line with highlights
    let mut new_highlighted = INDENT.to_string();
    if options.color {
        new_highlighted.push_str(green);
    }
    new_highlighted.push('+');

    for change in char_diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => {
                if options.color {
                    new_highlighted.push_str(green_bg);
                    new_highlighted.push_str(change.value());
                    new_highlighted.push_str(green);
                } else {
                    new_highlighted.push_str(change.value());
                }
            }
            similar::ChangeTag::Equal => {
                new_highlighted.push_str(change.value());
            }
            similar::ChangeTag::Delete => {} // Skip deletions in new line
        }
    }

    // Add right-aligned suffix only if exceeds threshold
    if new_width > LINE_WIDTH_THRESHOLD {
        let padding = max_visual_width.saturating_sub(new_width) + 2;
        for _ in 0..padding {
            new_highlighted.push(' ');
        }
        let _ = write!(new_highlighted, "{new_width:>num_width$}");
    }

    if options.color {
        new_highlighted.push_str(reset);
    }
    let _ = writeln!(output, "{new_highlighted}");
}

/// A diff line - either a real change, a gap indicator, or a hunk header
#[derive(Clone)]
enum DiffLine<'a> {
    Change(similar::Change<&'a str>),
    Gap,
    HunkHeader {
        old_start: usize,
        old_count: usize,
        new_start: usize,
        new_count: usize,
    },
}

/// Apply context filtering to show only N lines around changes, with optional hunk headers
fn apply_context_filter<'a>(
    changes: &[similar::Change<&'a str>],
    context: usize,
    show_headers: bool,
) -> Vec<DiffLine<'a>> {
    let mut result = Vec::new();
    let mut last_change_idx: Option<usize> = None;
    let mut hunk_started = false;

    // Find indices of all changed lines
    let changed_indices: Vec<usize> = changes
        .iter()
        .enumerate()
        .filter_map(|(idx, change)| {
            if !matches!(change.tag(), similar::ChangeTag::Equal) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    if changed_indices.is_empty() {
        return Vec::new();
    }

    // Track lines for current hunk
    let mut hunk_changes = Vec::new();

    // Include lines within context of any change
    for (idx, change) in changes.iter().enumerate() {
        let should_include = changed_indices.iter().any(|&change_idx| {
            let distance = idx.abs_diff(change_idx);
            distance <= context
        });

        if should_include {
            // Check if we're starting a new hunk (after a gap)
            let starting_new_hunk = if let Some(last_idx) = last_change_idx {
                idx > last_idx + 1
            } else {
                true // First hunk
            };

            if starting_new_hunk {
                // Flush previous hunk with header
                if !hunk_changes.is_empty() && show_headers && hunk_started {
                    let header = calculate_hunk_header(&hunk_changes);
                    result.insert(
                        result.len() - hunk_changes.len(),
                        DiffLine::HunkHeader {
                            old_start: header.0,
                            old_count: header.1,
                            new_start: header.2,
                            new_count: header.3,
                        },
                    );
                }

                // Add gap indicator if not the first hunk
                if last_change_idx.is_some() {
                    result.push(DiffLine::Gap);
                }

                // Start tracking new hunk
                hunk_changes.clear();
                hunk_started = true;
            }

            hunk_changes.push(*change);
            result.push(DiffLine::Change(*change));
            last_change_idx = Some(idx);
        }
    }

    // Flush final hunk with header
    if !hunk_changes.is_empty() && show_headers && hunk_started {
        let header = calculate_hunk_header(&hunk_changes);
        result.insert(
            result.len() - hunk_changes.len(),
            DiffLine::HunkHeader {
                old_start: header.0,
                old_count: header.1,
                new_start: header.2,
                new_count: header.3,
            },
        );
    }

    result
}

/// Calculate hunk header info (old_start, old_count, new_start, new_count)
fn calculate_hunk_header(hunk_changes: &[similar::Change<&str>]) -> (usize, usize, usize, usize) {
    let mut old_start = usize::MAX;
    let mut new_start = usize::MAX;
    let mut old_count = 0;
    let mut new_count = 0;

    for change in hunk_changes {
        match change.tag() {
            similar::ChangeTag::Delete => {
                if let Some(idx) = change.old_index() {
                    old_start = old_start.min(idx);
                }
                old_count += 1;
            }
            similar::ChangeTag::Insert => {
                if let Some(idx) = change.new_index() {
                    new_start = new_start.min(idx);
                }
                new_count += 1;
            }
            similar::ChangeTag::Equal => {
                if let Some(idx) = change.old_index() {
                    old_start = old_start.min(idx);
                }
                if let Some(idx) = change.new_index() {
                    new_start = new_start.min(idx);
                }
                old_count += 1;
                new_count += 1;
            }
        }
    }

    // Convert 0-indexed to 1-indexed for display
    (
        if old_start == usize::MAX {
            1
        } else {
            old_start + 1
        },
        old_count,
        if new_start == usize::MAX {
            1
        } else {
            new_start + 1
        },
        new_count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_summary_with_labels() {
        // left has line2, right replaces it with line3
        let left = "line1\nline2\n";
        let right = "line1\nline3\n";

        // Test compare labels
        let mut options = DiffOptions::compare();
        options.color = false; // Disable color for easier testing
        let output = diff_to_string(left, right, &options);
        assert!(
            output.contains("-1 ours-only, +1 prettier-only"),
            "Compare mode should use 'ours-only'/'prettier-only' labels. Got: {output}"
        );

        // Test ast_diff labels
        let mut options = DiffOptions::ast_diff();
        options.color = false;
        let output = diff_to_string(left, right, &options);
        assert!(
            output.contains("-1 original-only, +1 formatted-only"),
            "AST diff mode should use 'original-only'/'formatted-only' labels. Got: {output}"
        );

        // Test idempotency labels (actual vs expected)
        let mut options = DiffOptions::idempotency();
        options.color = false;
        let output = diff_to_string(left, right, &options);
        assert!(
            output.contains("-1 formatted-actual, +1 input-expected"),
            "Idempotency mode should use 'formatted-actual'/'input-expected' labels. Got: {output}"
        );
    }

    #[test]
    fn test_diff_summary_without_labels() {
        let left = "line1\nline2\n";
        let right = "line1\nline3\n";

        // Default options (no labels)
        let options = DiffOptions {
            show_summary: true,
            color: false,
            ..Default::default()
        };
        let output = diff_to_string(left, right, &options);
        assert!(
            output.contains("-1 deletions, +1 insertions"),
            "Default mode should use 'deletions'/'insertions' labels. Got: {output}"
        );
    }

    #[test]
    fn test_digit_width() {
        assert_eq!(digit_width(0), 1);
        assert_eq!(digit_width(1), 1);
        assert_eq!(digit_width(9), 1);
        assert_eq!(digit_width(10), 2);
        assert_eq!(digit_width(99), 2);
        assert_eq!(digit_width(100), 3);
        assert_eq!(digit_width(999), 3);
        assert_eq!(digit_width(1000), 4);
    }

    #[test]
    fn test_expand_tabs() {
        assert_eq!(expand_tabs("hello", 2), "hello");
        assert_eq!(expand_tabs("\thello", 2), "  hello");
        assert_eq!(expand_tabs("\t\thello", 2), "    hello");
        assert_eq!(expand_tabs("a\tb\tc", 2), "a  b  c");
        assert_eq!(expand_tabs("\thello", 4), "    hello");
    }

    #[test]
    fn test_line_width_threshold() {
        // Short line (50 chars) vs long line (100 chars)
        let short = "x".repeat(50);
        let long = "y".repeat(100);
        let left = format!("{short}\n");
        let right = format!("{long}\n");

        let mut options = DiffOptions::compare();
        options.color = false;
        let output = diff_to_string(&left, &right, &options);

        // Short line should NOT have width suffix (50 <= 90)
        assert!(
            !output.contains("  50"),
            "Short line should not show width. Got: {output}"
        );

        // Long line SHOULD have width suffix (100 > 90)
        assert!(
            output.contains("  100"),
            "Long line should show width. Got: {output}"
        );
    }

    #[test]
    fn test_line_width_threshold_all_short() {
        // Both lines short - no widths should appear
        let left = "short line here\n";
        let right = "different short\n";

        let mut options = DiffOptions::compare();
        options.color = false;
        let output = diff_to_string(left, right, &options);

        // No numeric suffixes should appear (no lines > 90 chars)
        // Check that the output doesn't end lines with numbers
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('-') || trimmed.starts_with('+') {
                assert!(
                    !trimmed.chars().last().unwrap_or(' ').is_ascii_digit(),
                    "Short lines should not have width suffix. Line: {trimmed}"
                );
            }
        }
    }

    #[test]
    fn context_filter_emits_two_hunks_with_a_gap() {
        // Two changes far enough apart (context = 3) produce two separate hunks,
        // each with its own `@@` header, and a `...` gap between them.
        let lines: Vec<String> = (0..20).map(|i| format!("line{i}")).collect();
        let expected = lines.join("\n") + "\n";
        let mut actual_lines = lines;
        actual_lines[2] = "CHANGED_A".to_string();
        actual_lines[17] = "CHANGED_B".to_string();
        let actual = actual_lines.join("\n") + "\n";

        let mut options = DiffOptions::compare();
        options.color = false;
        let out = diff_to_string(&expected, &actual, &options);

        assert_eq!(
            out.matches("@@ -").count(),
            2,
            "expected two hunk headers:\n{out}"
        );
        assert!(out.contains("..."), "expected a gap between hunks:\n{out}");
    }

    #[test]
    fn no_changes_yields_empty_diff() {
        // Identical inputs ⇒ apply_context_filter returns nothing ⇒ empty output.
        let same = "alpha\nbeta\ngamma\n";
        let mut options = DiffOptions::compare();
        options.color = false;
        assert!(diff_to_string(same, same, &options).is_empty());
    }

    #[test]
    fn extract_json_key_basic() {
        assert_eq!(
            extract_json_key("\"type\": \"Program\""),
            Some("type".to_string())
        );
        assert_eq!(
            extract_json_key("  \"start\": 0,"),
            Some("start".to_string())
        );
        assert_eq!(
            extract_json_key("\"children\": ["),
            Some("children".to_string())
        );
        // Non-key lines.
        assert_eq!(extract_json_key("123,"), None);
        assert_eq!(extract_json_key("{"), None);
        assert_eq!(extract_json_key("}"), None);
    }

    #[test]
    fn build_json_path_map_objects_and_scalar_arrays() {
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "type": "X",
            "nums": [10, 20, 30],
        }))
        .unwrap();
        let map = build_json_path_map(&json);
        let has = |path: &str| map.values().any(|v| v == path);
        // Object keys map to `$.key`.
        assert!(has("$.type"), "map: {map:?}");
        assert!(has("$.nums"), "map: {map:?}");
        // Scalar array elements get incrementing indices.
        assert!(has("$.nums[0]"), "map: {map:?}");
        assert!(has("$.nums[1]"), "map: {map:?}");
        assert!(has("$.nums[2]"), "map: {map:?}");

        // Known limitation: the array index only advances for scalar elements
        // ending in ',', so arrays of OBJECTS all collapse to `[0]`. Pinned here
        // so a future fix is noticed (cosmetic — debug-diff path annotations only).
        let obj_array = serde_json::to_string_pretty(&serde_json::json!({
            "items": [{"v": 1}, {"v": 2}],
        }))
        .unwrap();
        let m2 = build_json_path_map(&obj_array);
        assert!(
            m2.values().all(|v| !v.contains("[1]")),
            "object-array indices unexpectedly advanced: {m2:?}"
        );
    }
}
