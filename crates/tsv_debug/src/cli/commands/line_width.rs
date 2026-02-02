use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::Input;
use tsv_lang::printing::visual_width;

/// Line width measurement command - measures line widths accounting for tab width
pub struct LineWidthCommand;

impl Command for LineWidthCommand {
    fn name(&self) -> &str {
        "line_width"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        // Parse input - no parser type needed, just measure raw text
        let input = if let Some(content) = args.option("content") {
            Input::from_content(content)
        } else if args.flag("stdin") {
            Input::from_stdin()?
        } else if let Some(path) = args.positional() {
            Input::from_file(&path)?
        } else {
            return Err("No input provided. Use a file path, --content, or --stdin".to_string());
        };

        // Parse optional flags
        let line = args
            .option("line")
            .map(|s| {
                s.parse::<usize>()
                    .map_err(|_| format!("Invalid line number: {s}"))
            })
            .transpose()?;

        let tab_width = args
            .option("tab-width")
            .map(|s| {
                s.parse::<usize>()
                    .map_err(|_| format!("Invalid tab width: {s}"))
            })
            .transpose()?
            .unwrap_or(2); // Default to prettier's tabWidth: 2

        let print_width = 100; // Prettier default
        let json = args.flag("json");

        Ok(Box::new(LineWidthExecutable {
            input,
            line,
            tab_width,
            print_width,
            json,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "line_width <file>                           Measure line widths for all lines in file"
                .to_string(),
            "line_width <file> --line <N>                Measure specific line number".to_string(),
            "line_width --content <string>               Measure line widths for content string"
                .to_string(),
            "line_width --stdin                          Measure line widths from stdin"
                .to_string(),
            "line_width <file> --tab-width <N>           Use custom tab width (default: 2)"
                .to_string(),
            "line_width <file> --json                    Output in JSON format".to_string(),
        ]
    }
}

/// Executable instance for line_width command
struct LineWidthExecutable {
    input: Input,
    line: Option<usize>,
    tab_width: usize,
    print_width: usize,
    json: bool,
}

impl Executable for LineWidthExecutable {
    fn execute(&self) {
        let content = self.input.content();
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            if self.json {
                println!(r#"{{"lines": []}}"#);
            } else {
                println!("No lines to measure");
            }
            return;
        }

        // Check if specific line exists
        if let Some(line_num) = self.line
            && (line_num == 0 || line_num > lines.len())
        {
            eprintln!(
                "Error: Line {} does not exist (file has {} lines)",
                line_num,
                lines.len()
            );
            std::process::exit(1);
        }

        let mut exceeds_count = 0;
        let mut json_results = Vec::new();

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;

            // Skip if measuring specific line
            if let Some(target) = self.line
                && line_num != target
            {
                continue;
            }

            // Calculate visual width using Unicode Standard Annex #11
            let total = visual_width(line, self.tab_width);
            let tab_count = line.chars().filter(|&c| c == '\t').count();
            let tab_width_total = tab_count * self.tab_width;
            let content_width = total - tab_width_total;

            let exceeds = total > self.print_width;
            if exceeds {
                exceeds_count += 1;
            }

            if self.json {
                json_results.push(serde_json::json!({
                    "line": line_num,
                    "total": total,
                    "tabs": tab_count,
                    "tab_width_total": tab_width_total,
                    "content_width": content_width,
                    "exceeds": exceeds,
                }));
            } else {
                let status = if total > self.print_width {
                    format!("✗ EXCEEDS print_width ({})", self.print_width)
                } else if total == self.print_width {
                    format!("⚠️  EXACTLY print_width ({})", self.print_width)
                } else {
                    "✓".to_string()
                };

                println!(
                    "Line {line_num}: {total} chars ({tab_count} tabs = {tab_width_total}, content = {content_width}) {status}"
                );

                // Show line preview for specific line queries
                if self.line.is_some() {
                    println!("  {line}");
                }
            }
        }

        // Print summary for non-JSON, non-specific-line output
        if !self.json && self.line.is_none() {
            println!(
                "\nSummary: {}/{} lines exceed print_width ({})",
                exceeds_count,
                lines.len(),
                self.print_width
            );
        }

        // Print JSON output
        if self.json {
            let output = serde_json::json!({"lines": json_results});
            // SAFETY: serde_json Value types always serialize successfully
            #[allow(clippy::unwrap_used)]
            let json_str = serde_json::to_string_pretty(&output).unwrap();
            println!("{json_str}");
        }
    }
}
