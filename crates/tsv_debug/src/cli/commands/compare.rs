use crate::diff::{Color, ColorChoice, DiffOptions, diff_to_string};
use crate::error;
use crate::{deno, subprocess};
use argh::FromArgs;
use tsv_cli::cli::input::{Input, InputArgs, ParserType};

/// Compare our printer output with prettier (shows diff).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compare")]
pub struct CompareCommand {
    /// only show output if outputs differ (exit 0 match, 1 differ)
    #[argh(switch)]
    quiet: bool,

    /// show full input, ours, prettier, and diff
    #[argh(switch, short = 'v')]
    verbose: bool,

    /// emit machine-readable JSON
    #[argh(switch)]
    json: bool,

    /// color output: auto | always | never (default: auto)
    #[argh(option)]
    color: Option<ColorChoice>,

    /// content to compare (requires --parser)
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

impl CompareCommand {
    pub fn run(self) {
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
        let exit_code = rt.block_on(run(
            &input,
            parser_type,
            self.quiet,
            self.verbose,
            self.json,
            self.color,
        ));
        if exit_code != 0 {
            std::process::exit(exit_code);
        }
    }
}

#[allow(clippy::expect_used)] // JSON serialization of simple types cannot fail
async fn run(
    input: &Input,
    parser_type: ParserType,
    quiet: bool,
    verbose: bool,
    json_output: bool,
    color_choice: Option<ColorChoice>,
) -> i32 {
    let content = input.content();
    let parser_name = match parser_type {
        ParserType::Svelte => "svelte",
        ParserType::TypeScript => "typescript",
        ParserType::Css => "css",
    };

    if verbose {
        println!("=== Input ===");
        println!("{content}");
        println!();
    }

    // Run our formatter
    let our_output = match run_our_formatter(content, parser_name) {
        Ok(output) => {
            if verbose {
                println!("=== Our Formatter ===");
                println!("{output}");
                println!();
            }
            Some(output)
        }
        Err(err) => {
            if verbose {
                eprintln!("=== Our Formatter ===");
            }
            eprintln!("Error running our formatter: {err}");
            if verbose {
                println!();
            }
            return 1;
        }
    };

    // Run prettier
    let prettier_output = match run_prettier(content, parser_name).await {
        Ok(output) => {
            if verbose {
                println!("=== Prettier ===");
                println!("{output}");
                println!();
            }
            Some(output)
        }
        Err(err) => {
            if verbose {
                eprintln!("=== Prettier ===");
            }
            eprintln!("Error running prettier: {err}");
            let hint = err.hint();
            if !hint.is_empty() {
                eprintln!("hint: {hint}");
            }
            if verbose {
                println!();
            }
            return 1;
        }
    };

    // Show diff if both succeeded
    if let (Some(our), Some(prettier)) = (our_output, prettier_output) {
        let outputs_match = our == prettier;

        if json_output {
            // JSON output mode
            let result = serde_json::json!({
                "match": outputs_match,
                "our_output": our,
                "prettier_output": prettier,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("JSON serialization failed")
            );
            return if outputs_match { 0 } else { 1 };
        }

        if quiet {
            // In quiet mode, only show output if there's a difference
            if !outputs_match {
                let mut options = DiffOptions::compare();
                if let Some(choice) = color_choice {
                    options = options.with_color_choice(choice);
                }
                print_comparison_with_options(
                    "=== Diff: Ours vs Prettier ===",
                    &our,
                    &prettier,
                    &options,
                );
                return 1;
            }
            return 0;
        }

        // Default mode: always show the comparison result (diff only)
        let mut options = DiffOptions::compare();
        if let Some(choice) = color_choice {
            options = options.with_color_choice(choice);
        }
        print_comparison_with_options("=== Diff: Ours vs Prettier ===", &our, &prettier, &options);
        return if outputs_match { 0 } else { 1 };
    }

    1
}

/// Print comparison with custom options (variant of diff::print_comparison)
fn print_comparison_with_options(
    label: &str,
    our_output: &str,
    prettier_output: &str,
    options: &DiffOptions,
) {
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
        print!("{}", diff_to_string(our_output, prettier_output, options));
    }
}

fn run_our_formatter(content: &str, parser: &str) -> error::Result<String> {
    subprocess::run_tsv_format(content, parser)
}

async fn run_prettier(content: &str, parser: &str) -> error::Result<String> {
    Ok(deno::run_prettier(content, deno::PrettierParser::Parser(parser)).await?)
}
