use std::fs;
use std::io::{self, Read as _};
use std::path::Path;
use std::str::FromStr;

/// Input source for parsing or formatting (just the content string)
#[derive(Debug)]
pub struct Input(String);

impl Input {
    pub fn content(&self) -> &str {
        &self.0
    }

    /// Read from file path
    pub fn from_file(path: &str) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|e| {
            // the `<path>: detail` shape every other path diagnostic takes, with no
            // quotes of its own: a name `quote_path` quotes would otherwise read `'"…"'`
            format!("Error reading file {}: {e}", tsv_discover::quote_path(path))
        })?;
        Ok(Input(content))
    }

    /// Read from stdin
    pub fn from_stdin() -> Result<Self, String> {
        read_stdin_to_string()
            .map(Input)
            .map_err(|e| format!("Error reading from stdin: {e}"))
    }

    /// Direct string content
    pub fn from_content(content: String) -> Self {
        Input(content)
    }
}

/// Read stdin to EOF as strict UTF-8, waiting out `WouldBlock` the way
/// `cli::out::write_or_stop` does. Whether fd 0 blocks belongs to the open file
/// description this process shares with its parent, and a Node parent that opens its
/// own piped `process.stdin` flips it to non-blocking under the child — where
/// `read_to_string` answers the first momentarily empty pipe with `EAGAIN`, reporting
/// a slow writer as a read error. Any other error, and invalid UTF-8, stay errors; the
/// UTF-8 one keeps `read_to_string`'s own wording, which `cli.js` mirrors.
fn read_stdin_to_string() -> io::Result<String> {
    use crate::cli::out::{WOULD_BLOCK_MAX_WAIT, WOULD_BLOCK_MIN_WAIT};
    let mut stdin = io::stdin().lock();
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; 64 * 1024];
    let mut wait = WOULD_BLOCK_MIN_WAIT;
    loop {
        match stdin.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                bytes.extend_from_slice(&chunk[..n]);
                wait = WOULD_BLOCK_MIN_WAIT;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(wait);
                wait = (wait * 2).min(WOULD_BLOCK_MAX_WAIT);
            }
            Err(e) => return Err(e),
        }
    }
    String::from_utf8(bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        )
    })
}

/// Parser/formatter type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserType {
    Svelte,
    TypeScript,
    Css,
}

impl ParserType {
    /// Canonical lowercase name — the `--parser` value and the
    /// `tsv_debug` sidecar tool key.
    pub const fn name(self) -> &'static str {
        match self {
            ParserType::Svelte => "svelte",
            ParserType::TypeScript => "typescript",
            ParserType::Css => "css",
        }
    }

    /// The parser a path's extension picks, read without regard to ASCII case as
    /// `tsv_discover::is_formattable` reads it (`App.SVELTE` is a Svelte file): `.svelte`,
    /// `.css`, and TypeScript for everything else — the dispatch has no unknown arm, which
    /// is why a named file is held to the extension check first.
    pub fn from_extension(path: &str) -> Self {
        if has_extension_ignoring_case(path, "svelte") {
            ParserType::Svelte
        } else if has_extension_ignoring_case(path, "css") {
            ParserType::Css
        } else {
            ParserType::TypeScript
        }
    }
}

/// Whether `path`'s extension is `ext`, ignoring ASCII case — read by `Path::extension`,
/// the one reading every dispatch takes (`tsv_discover::is_formattable`,
/// `tsv_ts::Goal::from_extension`), so a bare dotfile like `.svelte` is a stem with no
/// extension here as it is there.
fn has_extension_ignoring_case(path: &str, ext: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(ext))
}

impl FromStr for ParserType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "svelte" => Ok(ParserType::Svelte),
            "typescript" | "ts" => Ok(ParserType::TypeScript),
            "css" => Ok(ParserType::Css),
            _ => Err(format!(
                "Unknown parser type: '{s}'. Valid types: svelte, typescript, css"
            )),
        }
    }
}

/// Shared input arguments for commands that accept a file path, `--content`, or `--stdin`.
///
/// Each command declares the four argh fields on its own struct and assembles an
/// `InputArgs` to call [`InputArgs::resolve`] — or [`InputArgs::resolve_with_source_type`]
/// where the command also takes `--source-type` (`tsv parse`, `tsv format --content`), which
/// grades that flag against the parser ahead of the read. argh has no struct-flattening
/// attribute, so the field declarations are repeated per command.
#[derive(Debug)]
pub struct InputArgs {
    pub content: Option<String>,
    pub stdin: bool,
    pub parser: Option<ParserType>,
    pub file: Option<String>,
}

/// What [`InputArgs::resolve_with_source_type`] settles: the input, the parser the
/// arguments named, and the goal a `--source-type` named — `None` where none was, which
/// each command reads its own way (`parse` as `Module`, `format` as the module-then-script
/// fallback).
#[derive(Debug)]
pub struct ResolvedInput {
    pub input: Input,
    pub parser_type: ParserType,
    pub goal: Option<tsv_ts::Goal>,
}

/// The refusal when no input arm is named — the same line from [`InputArgs::parser_type`]
/// and [`InputArgs::read`], since both walk the arms.
const NO_INPUT: &str = "No input provided. Use a file path, --content, or --stdin";

impl InputArgs {
    /// Resolve to an `Input` + `ParserType`: [`Self::parser_type`], then [`Self::read`].
    ///
    /// Precedence: `--content` > `--stdin` > file positional. `--content` and
    /// `--stdin` require `--parser`. On a file path, `--parser` overrides the
    /// extension-based detection when present; otherwise it's inferred from the
    /// extension — which must then be one tsv handles. The dispatch behind a path has
    /// no unknown arm (everything that isn't `.svelte` or `.css` is TypeScript), so
    /// without the check `tsv parse README.md` reports a baffling TypeScript syntax
    /// error rather than saying the file is out of scope. Same check, same message, as
    /// `tsv format <file>` (`tsv_discover::unsupported_extension_error`), where it is an
    /// argument error for the same reason.
    pub fn resolve(self) -> Result<(Input, ParserType), String> {
        let ResolvedInput {
            input, parser_type, ..
        } = self.resolve_with_source_type(None)?;
        Ok((input, parser_type))
    }

    /// [`Self::resolve`] for a command that also takes `--source-type`: the flag's value is
    /// graded first ([`parse_source_type_arg`]), then the parser is settled and the flag
    /// graded against it ([`check_source_type_language`]), and only then is the input read.
    /// The order is the point, and it is stated here so neither command restates it: a
    /// `--stdin` this turns away must not first wait on its writer (an open, empty stdin
    /// would hang the refusal), and a file arm's refusal does not turn on whether the file
    /// exists.
    pub fn resolve_with_source_type(
        self,
        source_type: Option<&str>,
    ) -> Result<ResolvedInput, String> {
        let goal = parse_source_type_arg(source_type)?;
        let parser_type = self.parser_type()?;
        check_source_type_language(source_type, parser_type)?;
        Ok(ResolvedInput {
            input: self.read()?,
            parser_type,
            goal,
        })
    }

    /// The parser the arguments settle on, decided before anything is read — so
    /// [`Self::resolve_with_source_type`] can grade a flag against it ahead of the read.
    /// Everything about the arguments themselves is graded here: a missing `--parser` on
    /// `--content`/`--stdin`, a directory named as the file, an extension tsv does not
    /// handle, no input at all.
    fn parser_type(&self) -> Result<ParserType, String> {
        if self.content.is_some() {
            self.parser
                .ok_or_else(|| "--content requires --parser <svelte|typescript|css>".to_string())
        } else if self.stdin {
            self.parser
                .ok_or_else(|| "--stdin requires --parser <svelte|typescript|css>".to_string())
        } else if let Some(path) = &self.file {
            // a directory is refused by name ahead of the parser choice: every command
            // resolving through here reads one file (`tsv parse`, the `tsv_debug` commands),
            // and the read would report `Is a directory` under a message that calls it
            // one. Neutral wording for that reason; `cli.js` mirrors it word for word
            if fs::metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
                return Err(format!(
                    "{}: is a directory (one file is expected)",
                    tsv_discover::quote_path(path)
                ));
            }
            match self.parser {
                Some(parser_type) => Ok(parser_type),
                None => match tsv_discover::unsupported_extension_error(path) {
                    Some(error) => Err(error),
                    None => Ok(ParserType::from_extension(path)),
                },
            }
        } else {
            Err(NO_INPUT.to_string())
        }
    }

    /// The input itself, by the same precedence [`Self::parser_type`] graded the
    /// arguments under — called after it, since this reads without re-grading them.
    fn read(self) -> Result<Input, String> {
        if let Some(content) = self.content {
            Ok(Input::from_content(content))
        } else if self.stdin {
            Input::from_stdin()
        } else if let Some(path) = self.file {
            Input::from_file(&path)
        } else {
            Err(NO_INPUT.to_string())
        }
    }
}

/// Parse the `--source-type` argument into a [`tsv_ts::Goal`]. Absent stays
/// **absent** — `module`/`script` map to the goals, and what an unnamed source
/// type means is the caller's to decide: `parse` reads it as `Module` (the wire's
/// `Program.sourceType` is a claim, so one grammar produces it), `format` as the
/// module-then-script fallback. Shared by both.
pub fn parse_source_type_arg(source_type: Option<&str>) -> Result<Option<tsv_ts::Goal>, String> {
    match source_type {
        None => Ok(None),
        Some(s) => tsv_ts::Goal::from_source_type(s)
            .map(Some)
            .ok_or_else(|| format!("invalid --source-type '{s}' (expected 'script' or 'module')")),
    }
}

/// Refuse a `--source-type` on a language that has no goal axis. Svelte hard-wires
/// `Module` and CSS has no goal, so a caller naming one there asked for something
/// that cannot be honored and must be told — the stance every binding takes
/// (`tsv_wasm`'s `read_options`, `tsv_ffi`'s `ffi_source_type`, `tsv_napi`), so the
/// CLI is not the one surface where the flag is silently dropped — the JS mirror
/// (`crates/tsv_wasm/npm/cli.js`, the bin of both npm CLIs) carries the same
/// refusal word for word, so the two shipped `tsv` bins cannot drift. Shared by
/// `parse` and `format`; the flag's *value* is validated ahead of this by
/// `parse_source_type_arg`, so this only asks whether it was named at all.
pub fn check_source_type_language(
    source_type: Option<&str>,
    parser_type: ParserType,
) -> Result<(), String> {
    if source_type.is_some() && parser_type != ParserType::TypeScript {
        return Err(format!(
            "--source-type is only supported for typescript (the {} parser has no source type)",
            parser_type.name()
        ));
    }
    Ok(())
}
