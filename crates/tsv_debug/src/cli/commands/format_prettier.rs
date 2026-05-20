use crate::deno::{PrettierParser, run_prettier};
use crate::diff::{digit_width, expand_tabs};
use argh::FromArgs;
use tsv_cli::cli::input::{Input, InputArgs, ParserType};

/// Default tab width for visual width calculations (matches prettier)
const TAB_WIDTH: usize = 2;

/// Format code using prettier (with line width annotations by default).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "format_prettier")]
pub struct FormatPrettierCommand {
    /// suppress line width annotations
    #[argh(switch)]
    no_line_widths: bool,

    /// content to format (requires --parser)
    #[argh(option)]
    content: Option<String>,

    /// read from stdin (requires --parser)
    #[argh(switch)]
    stdin: bool,

    /// parser type: svelte | typescript | css
    #[argh(option)]
    parser: Option<ParserType>,

    /// file path (parser auto-detected from extension)
    #[argh(positional)]
    file: Option<String>,
}

impl FormatPrettierCommand {
    pub fn run(self) {
        let show_line_widths = !self.no_line_widths;
        let input_args = InputArgs {
            content: self.content,
            stdin: self.stdin,
            parser: self.parser,
            file: self.file,
        };
        let (input, parser_type) = match input_args.resolve() {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        };

        let rt = super::create_runtime();
        rt.block_on(run(&input, parser_type, show_line_widths));
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
