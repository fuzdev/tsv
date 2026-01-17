use crate::cli::input_parser;
use crate::deno::{PrettierParser, run_prettier};
use crate::diff::{digit_width, expand_tabs};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::{Input, ParserType};

/// Default tab width for visual width calculations (matches prettier)
const TAB_WIDTH: usize = 2;

/// format_prettier command - format code using prettier
pub struct FormatPrettierCommand;

impl Command for FormatPrettierCommand {
    fn name(&self) -> &str {
        "format_prettier"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let no_line_widths = args.flag("no-line-widths");
        let (input, parser_type) = input_parser::parse_input_and_parser_type(args)?;
        Ok(Box::new(FormatPrettierExecutable {
            input,
            parser_type,
            show_line_widths: !no_line_widths,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "format_prettier <file>                                Format file using prettier (shows line widths)"
                .to_string(),
            "format_prettier --no-line-widths <file>               Format without line width annotations"
                .to_string(),
            "format_prettier --content <str> --parser <type>        Format content using prettier (requires --parser)"
                .to_string(),
            "format_prettier --stdin --parser <type>                Format from stdin using prettier (requires --parser)"
                .to_string(),
        ]
    }
}

struct FormatPrettierExecutable {
    input: Input,
    parser_type: ParserType,
    show_line_widths: bool,
}

impl Executable for FormatPrettierExecutable {
    fn execute(&self) {
        let rt = super::create_runtime();
        rt.block_on(run(&self.input, self.parser_type, self.show_line_widths));
    }
}

async fn run(input: &Input, parser_type: ParserType, show_line_widths: bool) {
    let content = input.content();
    let parser = match parser_type {
        ParserType::Svelte => PrettierParser::Parser("svelte"),
        ParserType::TypeScript => PrettierParser::Parser("typescript"),
        ParserType::Css => PrettierParser::Parser("css"),
    };

    match run_prettier(content, parser).await {
        Ok(formatted) => {
            if show_line_widths {
                print_with_line_widths(&formatted);
            } else {
                print!("{formatted}");
            }
        }
        Err(err) => {
            eprintln!("Error formatting with prettier: {err}");
            std::process::exit(1);
        }
    }
}

/// Print content with line width annotations (right-aligned suffix, elide 0)
fn print_with_line_widths(content: &str) {
    // Expand tabs for consistent display
    let expanded_lines: Vec<String> = content.lines().map(|l| expand_tabs(l, TAB_WIDTH)).collect();
    let max_width = expanded_lines.iter().map(String::len).max().unwrap_or(0);
    let num_width = digit_width(max_width);

    for line in &expanded_lines {
        let width = line.len();
        if width == 0 {
            // Elide 0 for empty lines
            println!();
        } else {
            // Right-align width suffix with at least 2 spaces padding
            let padding = max_width.saturating_sub(width) + 2;
            println!("{line}{:padding$}{width:>num_width$}", "");
        }
    }
}
