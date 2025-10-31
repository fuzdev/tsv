use std::{
    env, fs,
    io::{self, Read},
    process,
};
use tsvr::{
    format_css, format_svelte, format_typescript, parse_css_ast, parse_svelte_ast,
    parse_to_json_with_options, parse_typescript_ast,
};

/// Input source for parsing or formatting
enum Input {
    File(String),    // File path
    Content(String), // Direct string content
    Stdin(String),   // Content read from stdin
}

impl Input {
    fn content(&self) -> &str {
        match self {
            Input::File(s) | Input::Content(s) | Input::Stdin(s) => s,
        }
    }
}

/// Parser/formatter type
#[derive(Clone, Copy)]
enum ParserType {
    Svelte,
    TypeScript,
    Css,
}

impl ParserType {
    fn from_extension(path: &str) -> Self {
        if path.ends_with(".svelte") {
            ParserType::Svelte
        } else if path.ends_with(".css") {
            ParserType::Css
        } else {
            ParserType::TypeScript
        }
    }

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "svelte" => Ok(ParserType::Svelte),
            "typescript" | "ts" => Ok(ParserType::TypeScript),
            "css" => Ok(ParserType::Css),
            _ => Err(format!(
                "Unknown parser type: '{}'. Valid types: svelte, typescript, css",
                s
            )),
        }
    }
}

/// CLI command
enum Command {
    Parse {
        input: Input,
        pretty: bool,
    },
    Format {
        input: Input,
        parser_type: ParserType,
    },
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        process::exit(1);
    }

    let command = match parse_args(&args) {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!();
            print_usage(&args[0]);
            process::exit(1);
        }
    };

    execute_command(command);
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("No command provided".to_string());
    }

    match args[1].as_str() {
        "parse" => parse_parse_command(args),
        "format" => parse_format_command(args),
        // TODO: Future commands for development workflow:
        // "tokens" => parse_tokens_command(args)  - Show lexer token stream (debugging)
        // "check" => parse_check_command(args)     - Validate syntax only (fast check)
        // "bench" => parse_bench_command(args)     - Parse N times, show timing
        cmd => Err(format!("Unknown command: '{}'", cmd)),
    }
}

fn parse_parse_command(args: &[String]) -> Result<Command, String> {
    let pretty = args.iter().any(|arg| arg == "--pretty");

    let input = if args.len() >= 4 && args[2] == "--content" {
        // Parse from --content string argument
        Input::Content(args[3].clone())
    } else if args.len() >= 3 && args[2] == "--stdin" {
        // Read from stdin (discouraged for agent usage)
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|e| format!("Error reading from stdin: {}", e))?;
        Input::Stdin(buffer)
    } else if args.len() >= 3 && args.len() <= 4 {
        // Read from file
        let path = &args[2];
        let source = fs::read_to_string(path)
            .map_err(|e| format!("Error reading file '{}': {}", path, e))?;
        Input::File(source)
    } else {
        return Err("Invalid parse command syntax".to_string());
    };

    Ok(Command::Parse { input, pretty })
}

fn parse_format_command(args: &[String]) -> Result<Command, String> {
    let (input, parser_type) = if args.len() >= 5 && args[2] == "--content" && args[4] == "--parser"
    {
        // Format from --content string argument
        let content = args[3].clone();
        let parser_type = ParserType::from_str(&args[5])?;
        (Input::Content(content), parser_type)
    } else if args.len() >= 3 && args[2] == "--stdin" {
        // Read from stdin (discouraged for agent usage)
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|e| format!("Error reading from stdin: {}", e))?;

        // Find --parser argument
        let parser_pos = args
            .iter()
            .position(|arg| arg == "--parser")
            .ok_or("--stdin requires --parser argument")?;

        if parser_pos + 1 >= args.len() {
            return Err("--parser requires a value".to_string());
        }

        let parser_type = ParserType::from_str(&args[parser_pos + 1])?;
        (Input::Stdin(buffer), parser_type)
    } else if args.len() == 3 {
        // Read from file, detect type from extension
        let path = &args[2];
        let source = fs::read_to_string(path)
            .map_err(|e| format!("Error reading file '{}': {}", path, e))?;
        let parser_type = ParserType::from_extension(path);
        (Input::File(source), parser_type)
    } else {
        return Err("Invalid format command syntax".to_string());
    };

    Ok(Command::Format { input, parser_type })
}

fn execute_command(command: Command) {
    match command {
        Command::Parse { input, pretty } => {
            match parse_to_json_with_options(input.content(), pretty) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    process::exit(1);
                }
            }
        }
        Command::Format { input, parser_type } => {
            let source = input.content();
            match parser_type {
                ParserType::Svelte => match parse_svelte_ast(source) {
                    Ok(ast) => {
                        let formatted = format_svelte(&ast, source);
                        println!("{}", formatted);
                    }
                    Err(e) => {
                        eprintln!("Parse error: {}", e);
                        process::exit(1);
                    }
                },
                ParserType::Css => match parse_css_ast(source) {
                    Ok(ast) => {
                        let formatted = format_css(&ast);
                        println!("{}", formatted);
                    }
                    Err(e) => {
                        eprintln!("Parse error: {}", e);
                        process::exit(1);
                    }
                },
                ParserType::TypeScript => match parse_typescript_ast(source) {
                    Ok(ast) => {
                        let formatted = format_typescript(&ast);
                        println!("{}", formatted);
                    }
                    Err(e) => {
                        eprintln!("Parse error: {}", e);
                        process::exit(1);
                    }
                },
            }
        }
    }
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <command> [options]", program);
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  parse <file> [--pretty]              Parse file and output AST as JSON");
    eprintln!("  parse --content <string> [--pretty]  Parse string and output AST as JSON");
    eprintln!("  parse --stdin [--pretty]             Parse stdin (not preferred for agents)");
    eprintln!("  format <file>                        Format file and output formatted code");
    eprintln!("  format --content <string> --parser <type>  Format string (preferred)");
    eprintln!("  format --stdin --parser <type>       Format stdin (not preferred for agents)");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} parse input.ts", program);
    eprintln!("  {} parse input.ts --pretty", program);
    eprintln!("  {} parse --content '<div>test</div>' --pretty", program);
    eprintln!("  {} format input.ts", program);
    eprintln!(
        "  {} format --content '<div>test</div>' --parser svelte",
        program
    );
}
