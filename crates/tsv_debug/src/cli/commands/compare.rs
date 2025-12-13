use crate::cli::input_parser;
use crate::diff::{ColorChoice, DiffOptions};
use crate::error;
use crate::{deno, subprocess};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::{Input, ParserType};

/// Compare command - compares our printer output with prettier
pub struct CompareCommand;

impl Command for CompareCommand {
    fn name(&self) -> &str {
        "compare"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        // Parse flags
        let quiet = args.flag("quiet");
        let json_output = args.flag("json");
        let color_choice = if let Some(color_str) = args.option("color") {
            Some(color_str.parse()?)
        } else {
            None
        };

        // Parse input and detect parser type
        let (input, parser_type) = input_parser::parse_input_and_parser_type(args)?;

        Ok(Box::new(CompareExecutable {
            input,
            parser_type,
            quiet,
            json_output,
            color_choice,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "compare <file>                                  Compare formatter output with prettier for file"
                .to_string(),
            "compare --content <string> --parser <type>      Compare formatter output (requires --parser svelte|typescript|css)"
                .to_string(),
            "compare --stdin --parser <type>                 Compare formatter output from stdin (requires --parser)"
                .to_string(),
            "compare --quiet <file>                          Only show diff if outputs differ (exit 0 if match, 1 if differ)"
                .to_string(),
            "compare --color <auto|always|never>             Control color output (default: auto)"
                .to_string(),
            "compare --json <file>                           Output machine-readable JSON"
                .to_string(),
        ]
    }
}

/// Executable instance for compare command
struct CompareExecutable {
    input: Input,
    parser_type: ParserType,
    quiet: bool,
    json_output: bool,
    color_choice: Option<ColorChoice>,
}

impl Executable for CompareExecutable {
    fn execute(&self) {
        let rt = super::create_runtime();
        let exit_code = rt.block_on(run(
            &self.input,
            self.parser_type,
            self.quiet,
            self.json_output,
            self.color_choice,
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
    json_output: bool,
    color_choice: Option<ColorChoice>,
) -> i32 {
    let content = input.content();
    let parser_name = match parser_type {
        ParserType::Svelte => "svelte",
        ParserType::TypeScript => "typescript",
        ParserType::Css => "css",
    };

    if !quiet {
        println!("=== Input ===");
        println!("{content}");
        println!();
    }

    // Run our formatter
    let our_output = match run_our_formatter(content, parser_name) {
        Ok(output) => {
            if !quiet {
                println!("=== Our Formatter ===");
                println!("{output}");
                println!();
            }
            Some(output)
        }
        Err(err) => {
            if !quiet {
                eprintln!("=== Our Formatter ===");
            }
            eprintln!("Error running our formatter: {err}");
            if !quiet {
                println!();
            }
            return 1;
        }
    };

    // Run prettier
    let prettier_output = match run_prettier(content, parser_name).await {
        Ok(output) => {
            if !quiet {
                println!("=== Prettier ===");
                println!("{output}");
                println!();
            }
            Some(output)
        }
        Err(err) => {
            if !quiet {
                eprintln!("=== Prettier ===");
            }
            eprintln!("Error running prettier: {err}");
            let hint = err.hint();
            if !hint.is_empty() {
                eprintln!("hint: {hint}");
            }
            if !quiet {
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

        // In normal mode, always show the comparison
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
    use crate::diff::{Color, diff_to_string};

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
