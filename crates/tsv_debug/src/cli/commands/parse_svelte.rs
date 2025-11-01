use crate::deno;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::Input;

/// parse_svelte command - parse Svelte using official Svelte parser
pub struct ParseSvelteCommand;

impl Command for ParseSvelteCommand {
    fn name(&self) -> &str {
        "parse_svelte"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let input = if let Some(content) = args.option("content") {
            Input::from_content(content)
        } else if let Some(path) = args.positional() {
            Input::from_file(&path)?
        } else {
            return Err("No input provided. Use a file path or --content".to_string());
        };

        Ok(Box::new(ParseSvelteExecutable { input }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "parse_svelte <file>           Parse Svelte file using official parser".to_string(),
            "parse_svelte --content <str>  Parse Svelte content using official parser".to_string(),
        ]
    }
}

struct ParseSvelteExecutable {
    input: Input,
}

impl Executable for ParseSvelteExecutable {
    fn execute(&self) {
        let content = self.input.content();

        match deno::parse_svelte(content) {
            Ok(json) => print!("{}", json),
            Err(err) => {
                eprintln!("Error parsing Svelte: {}", err);
                std::process::exit(1);
            }
        }
    }
}
