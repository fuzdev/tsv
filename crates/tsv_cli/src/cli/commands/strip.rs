use crate::cli::input::{InputArgs, ParserType, refuse_directory};
use crate::cli::out::{exit_with_error, exit_with_parse_error, write_stdout};
use argh::FromArgs;

/// Strip the types from TypeScript, keeping every character's line and column.
///
/// Prints the JS to stdout byte for byte: every type-only range blanked to whitespace, no
/// trailing newline added, line endings kept. One input: a .ts/.mts/.cts/.js/.mjs/.cjs
/// file, --content, or --stdin. Exit codes: 0 stripped, 1 refused (TypeScript with
/// runtime semantics, such as an enum), 2 a parse error or a usage error.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "strip")]
pub struct StripCommand {
    /// content to strip, printed to stdout
    #[argh(option)]
    content: Option<String>,

    /// read from stdin, print to stdout
    #[argh(switch)]
    stdin: bool,

    /// print nothing: exit 1 on a refusal, 2 on a parse error
    #[argh(switch)]
    check: bool,

    /// file path (.ts/.mts/.cts/.js/.mjs/.cjs)
    #[argh(positional)]
    file: Option<String>,
}

impl StripCommand {
    /// Strip one input. Every argument is graded before anything is read, so a refused
    /// `--stdin` never waits on its writer: a path beside `--content` / `--stdin`, a
    /// directory, an extension the strip does not take.
    pub fn run(self) {
        if let Some(path) = &self.file {
            if self.content.is_some() || self.stdin {
                exit_with_error(
                    2,
                    "Error: --content/--stdin cannot be combined with a file path",
                );
            }
            if let Err(e) = refuse_directory(path) {
                exit_with_error(2, format_args!("Error: {e}"));
            }
            if let Some(e) = tsv_discover::strip_unsupported_extension_error(path) {
                exit_with_error(2, format_args!("Error: {e}"));
            }
        }
        let (input, _) = InputArgs {
            content: self.content,
            stdin: self.stdin,
            parser: Some(ParserType::TypeScript),
            file: self.file,
        }
        .resolve()
        .unwrap_or_else(|e| exit_with_error(2, format_args!("Error: {e}")));
        let source = input.content();
        let arena =
            bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
        match tsv_ts::strip(source, &arena) {
            Ok(stripped) => {
                if !self.check {
                    write_stdout(stripped.code.as_bytes());
                }
            }
            // a parse error (or the size cap) is the error channel, as `tsv parse` prints it
            Err(tsv_ts::StripError::Parse(e)) => exit_with_parse_error(2, &e),
            // a refusal is the strip's verdict, as `format --check`'s "would change" is
            Err(error) => exit_with_error(1, format_args!("Error: {}", error.message(source))),
        }
    }
}
