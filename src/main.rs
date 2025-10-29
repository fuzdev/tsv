use std::{env, fs, process};
use tsvr::parse_to_json_with_options;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        process::exit(1);
    }

    match args[1].as_str() {
        "parse" => cmd_parse(&args),
        // TODO: Future commands for development workflow:
        // "tokens" => cmd_tokens(&args)  - Show lexer token stream (debugging)
        // "check" => cmd_check(&args)     - Validate syntax only (fast check)
        // "bench" => cmd_bench(&args)     - Parse N times, show timing
        _ => {
            eprintln!("Unknown command: '{}'", args[1]);
            eprintln!();
            print_usage(&args[0]);
            process::exit(1);
        }
    }
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <command> [options]", program);
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  parse <file> [--pretty]    Parse file and output AST as JSON");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} parse input.ts", program);
    eprintln!("  {} parse input.ts --pretty", program);
}

fn cmd_parse(args: &[String]) {
    if args.len() < 3 || args.len() > 4 {
        eprintln!("Usage: {} parse <file> [--pretty]", args[0]);
        process::exit(1);
    }

    let path = &args[2];
    let pretty = args.len() == 4 && args[3] == "--pretty";

    let source = fs::read_to_string(path)
        .unwrap_or_else(|e| {
            eprintln!("Error reading file '{}': {}", path, e);
            process::exit(1);
        });

    match parse_to_json_with_options(&source, pretty) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    }
}
