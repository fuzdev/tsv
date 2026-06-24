use crate::cli::CliError;
use argh::FromArgs;
use tsv_cli::cli::input::Input;
use tsv_lang::printing::visual_width;
use tsv_lang::{PRINT_WIDTH, TAB_WIDTH};

/// Measure visual line widths (accounts for tab width).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "line_width")]
pub struct LineWidthCommand {
    /// only measure this line number
    #[argh(option)]
    line: Option<usize>,

    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// content to measure
    #[argh(option)]
    content: Option<String>,

    /// read from stdin
    #[argh(switch)]
    stdin: bool,

    /// file path
    #[argh(positional)]
    file: Option<String>,
}

impl LineWidthCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let input = if let Some(content) = self.content {
            Input::from_content(content)
        } else if self.stdin {
            match Input::from_stdin() {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("Error: {e}");
                    return Err(CliError::Failed);
                }
            }
        } else if let Some(path) = self.file {
            match Input::from_file(&path) {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("Error: {e}");
                    return Err(CliError::Failed);
                }
            }
        } else {
            eprintln!("Error: No input provided. Use a file path, --content, or --stdin");
            return Err(CliError::Failed);
        };

        let content = input.content();
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            if self.json {
                println!(r#"{{"lines": []}}"#);
            } else {
                println!("No lines to measure");
            }
            return Ok(());
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
            return Err(CliError::Failed);
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
            let total = visual_width(line, TAB_WIDTH);
            let tab_count = line.chars().filter(|&c| c == '\t').count();
            let tab_width_total = tab_count * TAB_WIDTH;
            let content_width = total - tab_width_total;

            let exceeds = total > PRINT_WIDTH;
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
                let status = if total > PRINT_WIDTH {
                    format!("✗ EXCEEDS print_width ({PRINT_WIDTH})")
                } else if total == PRINT_WIDTH {
                    format!("⚠️  EXACTLY print_width ({PRINT_WIDTH})")
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
                PRINT_WIDTH
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

        Ok(())
    }
}
