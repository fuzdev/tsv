//! Diff utilities for comparing text and JSON with colored output

use std::fmt::Write;
use std::io::IsTerminal;
use std::str::FromStr;

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
    /// Show summary line (e.g., "5 insertions(+), 3 deletions(-)")
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
        }
    }
}

impl DiffOptions {
    /// Create options suitable for printing to stderr in validation errors
    pub fn validation() -> Self {
        let color_choice = ColorChoice::Auto;
        Self {
            context_lines: None, // Show all lines for debugging
            show_summary: true,
            show_header: false,
            inline_diff: true,     // Show exact character changes
            show_json_paths: true, // Show JSON paths for AST diffs
            color_choice,
            color: color_choice.use_color(),
        }
    }

    /// Create options suitable for the compare command
    pub fn compare() -> Self {
        let color_choice = ColorChoice::Auto;
        Self {
            context_lines: Some(3),
            show_summary: true,
            show_header: true,
            inline_diff: true,      // Show exact character changes
            show_json_paths: false, // Less useful for full file comparison
            color_choice,
            color: color_choice.use_color(),
        }
    }

    /// Set color choice and update color flag
    pub fn with_color_choice(mut self, choice: ColorChoice) -> Self {
        self.color_choice = choice;
        self.color = choice.use_color();
        self
    }
}

/// Print a colored diff between two strings to stderr
///
/// If both strings are valid JSON, pretty-prints them first.
/// Otherwise, diffs raw strings.
pub fn print_diff(label: &str, expected: &str, actual: &str) {
    print_diff_with_options(label, expected, actual, &DiffOptions::validation());
}

/// Print a colored diff with custom options
pub fn print_diff_with_options(label: &str, expected: &str, actual: &str, options: &DiffOptions) {
    eprintln!("\n           {label}:");
    eprint!("{}", diff_to_string(expected, actual, options));
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
        if options.color {
            let green = Color::Green.code();
            let red = Color::Red.code();
            let reset = Color::reset();
            let _ = writeln!(
                output,
                "           {green}{insertions} insertions(+){reset}, {red}{deletions} deletions(-){reset}"
            );
        } else {
            let _ = writeln!(
                output,
                "           {insertions} insertions(+), {deletions} deletions(-)"
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
                let _ = writeln!(output, "           {cyan}{path}{reset}");
            } else {
                let _ = writeln!(output, "           {path}");
            }
        }

        match &filtered_lines[i] {
            DiffLine::Gap => {
                if options.color {
                    let _ = writeln!(output, "           {cyan}...{reset}");
                } else {
                    let _ = writeln!(output, "           ...");
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
                        "           {cyan}@@ -{old_start},{old_count} +{new_start},{new_count} @@{reset}"
                    );
                } else {
                    let _ = writeln!(
                        output,
                        "           @@ -{old_start},{old_count} +{new_start},{new_count} @@"
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

                if options.color {
                    let code = color.map_or("", Color::code);
                    let _ = writeln!(output, "           {code}{sign}{change}{reset}");
                } else {
                    let _ = write!(output, "           {sign}{change}");
                    if !change.value().ends_with('\n') {
                        output.push('\n');
                    }
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
fn write_inline_diff(output: &mut String, old_line: &str, new_line: &str, options: &DiffOptions) {
    let reset = Color::reset();
    let red = Color::Red.code();
    let green = Color::Green.code();
    let red_bg = "\x1b[41m"; // Red background for deleted chars
    let green_bg = "\x1b[42m"; // Green background for inserted chars

    // Use inline diff to highlight exact character changes
    let char_diff = similar::TextDiff::from_chars(old_line.trim_end(), new_line.trim_end());

    // Build the old line with highlights
    let mut old_highlighted = String::from("           -");
    if options.color {
        old_highlighted.push_str(red);
    }

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

    if options.color {
        old_highlighted.push_str(reset);
    }
    let _ = writeln!(output, "{old_highlighted}");

    // Build the new line with highlights
    let mut new_highlighted = String::from("           +");
    if options.color {
        new_highlighted.push_str(green);
    }

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
    let mut hunk_start_idx: Option<usize> = None;

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
                if !hunk_changes.is_empty()
                    && show_headers
                    && let Some(start_idx) = hunk_start_idx
                {
                    let header = calculate_hunk_header(&hunk_changes, start_idx);
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
                hunk_start_idx = Some(idx);
            }

            hunk_changes.push(*change);
            result.push(DiffLine::Change(*change));
            last_change_idx = Some(idx);
        }
    }

    // Flush final hunk with header
    if !hunk_changes.is_empty()
        && show_headers
        && let Some(start_idx) = hunk_start_idx
    {
        let header = calculate_hunk_header(&hunk_changes, start_idx);
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
fn calculate_hunk_header(
    hunk_changes: &[similar::Change<&str>],
    _start_idx: usize,
) -> (usize, usize, usize, usize) {
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

/// Print a side-by-side comparison (for compare command)
#[allow(dead_code)] // Replaced by print_comparison_with_options in compare.rs
pub fn print_comparison(label: &str, our_output: &str, prettier_output: &str) {
    let options = DiffOptions::compare();
    let cyan = Color::Cyan.code();
    let reset = Color::reset();

    if our_output == prettier_output {
        if options.color {
            println!("{cyan}{label} ✓ Outputs match{reset}");
        } else {
            println!("{label} ✓ Outputs match");
        }
    } else {
        if options.color {
            println!("{cyan}{label} ✗ Outputs differ{reset}");
        } else {
            println!("{label} ✗ Outputs differ");
        }
        print!("{}", diff_to_string(our_output, prettier_output, &options));
    }
}
