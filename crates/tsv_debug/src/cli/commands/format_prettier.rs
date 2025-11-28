use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::{Input, ParserType};

/// format_prettier command - format code using prettier
pub struct FormatPrettierCommand;

impl Command for FormatPrettierCommand {
    fn name(&self) -> &str {
        "format_prettier"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let (input, parser_type) = if let Some(content) = args.option("content") {
            // --content requires --parser
            let parser = args
                .option("parser")
                .ok_or("Error: --parser required when using --content")?
                .parse()?;
            (Input::from_content(content), parser)
        } else if args.flag("stdin") {
            // --stdin requires --parser
            let parser = args
                .option("parser")
                .ok_or("Error: --parser required when using --stdin")?
                .parse()?;
            (Input::from_stdin()?, parser)
        } else if let Some(path) = args.positional() {
            let parser = ParserType::from_extension(&path);
            (Input::from_file(&path)?, parser)
        } else {
            return Err("No input provided. Use a file path, --content, or --stdin".to_string());
        };

        Ok(Box::new(FormatPrettierExecutable { input, parser_type }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "format_prettier <file>                                Format file using prettier"
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
}

impl Executable for FormatPrettierExecutable {
    fn execute(&self) {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(run(&self.input, self.parser_type));
    }
}

async fn run(input: &Input, parser_type: ParserType) {
    let content = input.content();
    let filepath = match parser_type {
        ParserType::Svelte => "temp.svelte",
        ParserType::TypeScript => "temp.ts",
        ParserType::Css => "temp.css",
    };

    match fuz_client::run_prettier(content, filepath).await {
        Ok(formatted) => print!("{}", formatted),
        Err(err) => {
            eprintln!("Error formatting with prettier: {}", err);
            std::process::exit(1);
        }
    }
}
