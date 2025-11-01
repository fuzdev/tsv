use crate::deno;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::Input;

/// parse_typescript command - parse TypeScript using acorn + acorn-typescript
pub struct ParseTypeScriptCommand;

impl Command for ParseTypeScriptCommand {
    fn name(&self) -> &str {
        "parse_typescript"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let input = if let Some(content) = args.option("content") {
            Input::from_content(content)
        } else if let Some(path) = args.positional() {
            Input::from_file(&path)?
        } else {
            return Err("No input provided. Use a file path or --content".to_string());
        };

        Ok(Box::new(ParseTypeScriptExecutable { input }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "parse_typescript <file>           Parse TypeScript file using acorn".to_string(),
            "parse_typescript --content <str>  Parse TypeScript content using acorn".to_string(),
        ]
    }
}

struct ParseTypeScriptExecutable {
    input: Input,
}

impl Executable for ParseTypeScriptExecutable {
    fn execute(&self) {
        let content = self.input.content();

        match deno::parse_typescript(content) {
            Ok(json) => print!("{}", json),
            Err(err) => {
                eprintln!("Error parsing TypeScript: {}", err);
                std::process::exit(1);
            }
        }
    }
}
