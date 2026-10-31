use crate::cli::input::{InputArgs, ParserType, parse_source_type_arg, refuse_directory};
use crate::cli::out::{exit_with_error, exit_with_parse_error, write_stdout};
use argh::FromArgs;

/// Strip the types from TypeScript, keeping every character's line and column.
///
/// Prints the JS to stdout byte for byte: every type-only range blanked to whitespace, no
/// trailing newline added, line endings kept. One input: a .ts/.mts/.cts file, --content,
/// or --stdin. Read as a module, retried as a script if that fails, as format reads it;
/// a .mts file is a module by name. Exit codes: 0 stripped, 1 refused (TypeScript that
/// type blanking cannot erase, such as an enum), 2 a parse error or a usage error.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "strip")]
pub struct StripCommand {
    /// content to strip, printed to stdout
    #[argh(option)]
    content: Option<String>,

    /// read from stdin, print to stdout
    #[argh(switch)]
    stdin: bool,

    /// parse goal: script | module. Unset: parsed as a module, retried as a script if that
    /// fails. --content/--stdin only.
    #[argh(option)]
    source_type: Option<String>,

    /// print nothing: exit 1 on a refusal, 2 on a parse error
    #[argh(switch)]
    check: bool,

    /// file path (.ts/.mts/.cts)
    #[argh(positional)]
    file: Option<String>,
}

impl StripCommand {
    /// Strip one input. Every argument is graded before anything is read, so a refused
    /// `--stdin` never waits on its writer: a path beside `--content` / `--stdin` or
    /// `--source-type`, a directory, an extension the strip does not take, a
    /// `--source-type` naming neither goal.
    pub fn run(self) {
        if let Some(path) = &self.file {
            if self.content.is_some() || self.stdin {
                exit_with_error(
                    2,
                    "Error: --content/--stdin cannot be combined with a file path",
                );
            }
            // a path's goal is its extension's, as `format` reads it, so a set
            // `--source-type` beside one is refused in `format`'s words
            if self.source_type.is_some() {
                exit_with_error(
                    2,
                    "Error: --source-type applies to --content/--stdin; a file path's goal is its extension's (.mjs/.mts a module, any other JS/TS extension the module grammar retried as a script)",
                );
            }
            if let Err(e) = refuse_directory(path) {
                exit_with_error(2, format_args!("Error: {e}"));
            }
            if let Some(e) = tsv_discover::unstrippable_extension_error(path) {
                exit_with_error(2, format_args!("Error: {e}"));
            }
        }
        let goal = parse_source_type_arg(self.source_type.as_deref())
            .unwrap_or_else(|e| exit_with_error(2, format_args!("Error: {e}")));
        // a `.mts` is a module by name and takes no retry (`tsv_ts::Goal::from_extension`)
        let goal = goal.or_else(|| self.file.as_deref().and_then(tsv_ts::Goal::from_extension));
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
        match tsv_ts::__private::strip_in(source, goal, &arena) {
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
