//! `tape` — the span-only wire and its tape (`docs/tape_format.md`) for every fixture document,
//! or on request.
//!
//! The Rust half of `npm run check:tape` (`scripts/check_tape.ts`): one process that parses every
//! fixture document `loc_wires` grades, writes its span-only wire, transcodes the wire to a tape
//! (`tsv_arena::tape::Transcoder`), and streams one NDJSON record per document to stdout —
//! `{"path", "language", "source", "span", "tape", "direct"?}`, the wire embedded verbatim, the
//! tapes in base64 — so the JS half can encode the same wire with the offline encoder, compare
//! the two tapes byte for byte, and decode them. `direct` is the document's tape as its wire
//! writer writes it itself (`tsv_debug::tape::grade_css_direct_tape` and its TypeScript and
//! Svelte twins, which grade it against the wire first: a failure there fails the run), for
//! the JS half to decode too.
//! `source` is the exact string parsed, a leading BOM included; the tape is written against the
//! text the wire's offsets index (`tsv_debug::tape::wire_source`). A prettier-side document tsv
//! rejects is left out (it has no wire; `check:loc` owns that ledger), and one the fixture tree
//! requires tsv to parse fails the run.
//!
//! ## `--stdin`: a tape on request
//!
//! The corpus leg (`npm run tape:corpus`) and the decoder bench ask this process for tsv's side:
//! one NDJSON request per line on stdin, `{"language", "goal"?, "source", "repeat"?}` (`goal`
//! `"script"` / `"module"`, TypeScript only; `repeat` a count of timed transcodes), answered
//! by exactly one line on stdout, flushed before the next request is read:
//!
//! - `{"span": <wire>, "tape": "<base64>", "direct"?: "<base64>", "transcode_ns"?}` — the
//!   span-only wire and its tape, its source's direct tape (as above), and
//!   with `repeat` the fastest of that many transcodes of the wire on a warm transcoder, in
//!   nanoseconds;
//! - `{"error": "<message>"}` — tsv rejected the source, the transcoder refused the wire, or a
//!   source's direct tape failed its grade (`direct: …`);
//! - `{"panic": "<payload>"}` — tsv panicked (each request runs under `catch_unwind`).
//!
//! A malformed request is a harness bug, not a tsv verdict: it ends the process with an error.

use crate::cli::CliError;
use crate::tape::{
    base64, fixture_wires, grade_css_direct_tape, grade_svelte_direct_tape, grade_ts_direct_tape,
    wire_source,
};
use argh::FromArgs;
use std::io::Write;
use std::path::PathBuf;
use tsv_arena::tape::Transcoder;
use tsv_lang::tape::TapeSink;

/// Stream every fixture document's span-only wire and tape as NDJSON (`check:tape`'s input), or
/// answer tape requests over stdin (`--stdin`).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "tape")]
pub struct TapeCommand {
    /// answer NDJSON tape requests read from stdin instead of walking a tree
    #[argh(switch)]
    pub stdin: bool,

    /// fixture tree to walk (default: tests/fixtures)
    #[argh(positional, default = "PathBuf::from(\"tests/fixtures\")")]
    pub root: PathBuf,
}

impl TapeCommand {
    /// Run the command.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the tree can't be walked, a required document can't be
    /// read, parsed or transcoded, a `--stdin` request is malformed, or stdin / stdout fails.
    pub fn run(self) -> Result<(), CliError> {
        if self.stdin {
            return super::serve_ndjson("tape", answer);
        }
        let stdout = std::io::stdout();
        let mut out = std::io::BufWriter::new(stdout.lock());
        let mut transcoder = Transcoder::new();
        let mut sink = TapeSink::new();
        let mut failure: Option<String> = None;
        let walked = fixture_wires(&self.root, |doc| {
            if failure.is_some() {
                return;
            }
            let tape = match transcoder.transcode(&doc.span, wire_source(doc.language, &doc.source))
            {
                Ok(tape) => base64(tape),
                Err(e) => {
                    failure = Some(format!("{}: transcode: {e}", doc.path));
                    return;
                }
            };
            let graded = match doc.language {
                "css" => Some(grade_css_direct_tape(
                    &doc.source,
                    &doc.span,
                    &mut transcoder,
                    &mut sink,
                )),
                "typescript" => Some(grade_ts_direct_tape(
                    &doc.source,
                    doc.goal,
                    &doc.span,
                    &mut transcoder,
                    &mut sink,
                )),
                "svelte" => Some(grade_svelte_direct_tape(
                    &doc.source,
                    &doc.span,
                    &mut transcoder,
                    &mut sink,
                )),
                _ => None,
            };
            let direct = match graded {
                Some(Ok(direct)) => Some(base64(&direct)),
                Some(Err(e)) => {
                    failure = Some(format!("{}: direct: {e}", doc.path));
                    return;
                }
                None => None,
            };
            let mut line = Vec::with_capacity(doc.span.len() * 2 + doc.source.len() + 64);
            line.extend_from_slice(b"{\"path\":");
            line.extend_from_slice(json_string(&doc.path).as_bytes());
            line.extend_from_slice(b",\"language\":\"");
            line.extend_from_slice(doc.language.as_bytes());
            line.extend_from_slice(b"\",\"source\":");
            line.extend_from_slice(json_string(&doc.source).as_bytes());
            line.extend_from_slice(b",\"span\":");
            line.extend_from_slice(&doc.span);
            line.extend_from_slice(b",\"tape\":\"");
            line.extend_from_slice(tape.as_bytes());
            if let Some(direct) = direct {
                line.extend_from_slice(b"\",\"direct\":\"");
                line.extend_from_slice(direct.as_bytes());
            }
            line.extend_from_slice(b"\"}\n");
            if let Err(e) = out.write_all(&line) {
                failure = Some(format!("writing stdout: {e}"));
            }
        });
        if let Err(e) = walked {
            eprintln!("Error: {e}");
            return Err(CliError::Failed);
        }
        if let Some(e) = failure {
            eprintln!("Error: {e}");
            return Err(CliError::Failed);
        }
        out.flush().map_err(|e| {
            eprintln!("Error: writing stdout: {e}");
            CliError::Failed
        })
    }
}

/// `s` as a JSON string literal.
#[expect(
    clippy::expect_used,
    reason = "serializing a `str` cannot fail, and an empty fallback would be invalid JSON"
)]
fn json_string(s: &str) -> String {
    serde_json::to_string(s).expect("a `str` always serializes")
}

/// One `--stdin` request (module doc).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TapeRequest {
    language: String,
    #[serde(default)]
    goal: Option<String>,
    source: String,
    /// Transcode this many times and answer the fastest one's nanoseconds too (the decoder
    /// bench's transcoder column).
    #[serde(default)]
    repeat: Option<u32>,
}

/// One request's product: the span-only wire, its tape, a direct tape (base64
/// both), and the timed transcode's nanoseconds.
type Produced = (Vec<u8>, String, Option<String>, Option<u128>);

/// The reply line for one request — `Err` only for a request no binding would accept.
fn answer(request: &TapeRequest) -> Result<Vec<u8>, String> {
    let language = request.language.as_str();
    let goal = match (request.goal.as_deref(), language) {
        (None, _) => tsv_ts::Goal::Module,
        (Some(goal), "typescript") => goal
            .parse::<tsv_ts::Goal>()
            .map_err(|_| format!("unknown goal {goal:?} in a tape request"))?,
        (Some(goal), _) => return Err(format!("goal {goal:?} on {language:?}, which has none")),
    };
    if !matches!(language, "svelte" | "typescript" | "css") {
        return Err(format!("unknown language {language:?} in a tape request"));
    }
    let source = request.source.as_str();
    let produced = super::catch_request_panic(|| -> Result<Produced, String> {
        let span = match language {
            "svelte" => {
                let ast = tsv_svelte::__private::parse(source).map_err(|e| format!("{e:#}"))?;
                tsv_svelte::__private::convert_ast_json_bytes(&ast, source)
            }
            "css" => {
                let ast = tsv_css::__private::parse(source).map_err(|e| format!("{e:#}"))?;
                tsv_css::__private::convert_ast_json_bytes(&ast, source)
            }
            _ => {
                let ast = tsv_ts::__private::parse_with_goal(source, goal)
                    .map_err(|e| format!("{e:#}"))?;
                tsv_ts::__private::convert_ast_json_bytes(&ast, source)
            }
        };
        let mut transcoder = Transcoder::new();
        let wire_text = wire_source(language, source);
        let tape = transcoder
            .transcode(&span, wire_text)
            .map(base64)
            .map_err(|e| format!("transcode: {e}"))?;
        // warm: the first transcode above grew the buffers
        let fastest = request.repeat.map(|n| {
            (0..n.max(1))
                .map(|_| {
                    let started = std::time::Instant::now();
                    let _ = std::hint::black_box(transcoder.transcode(&span, wire_text));
                    started.elapsed().as_nanos()
                })
                .min()
                .unwrap_or(0)
        });
        let mut sink = TapeSink::new();
        let direct = match language {
            "css" => Some(grade_css_direct_tape(
                source,
                &span,
                &mut transcoder,
                &mut sink,
            )),
            "typescript" => Some(grade_ts_direct_tape(
                source,
                goal,
                &span,
                &mut transcoder,
                &mut sink,
            )),
            "svelte" => Some(grade_svelte_direct_tape(
                source,
                &span,
                &mut transcoder,
                &mut sink,
            )),
            _ => None,
        };
        let direct = match direct {
            Some(graded) => Some(base64(&graded.map_err(|e| format!("direct: {e}"))?)),
            None => None,
        };
        Ok((span, tape, direct, fastest))
    });
    Ok(match produced {
        Ok(Ok((span, tape, direct, fastest))) => {
            let mut reply = Vec::with_capacity(span.len() + tape.len() + 48);
            reply.extend_from_slice(b"{\"span\":");
            reply.extend_from_slice(&span);
            reply.extend_from_slice(b",\"tape\":\"");
            reply.extend_from_slice(tape.as_bytes());
            reply.push(b'"');
            if let Some(direct) = direct {
                reply.extend_from_slice(b",\"direct\":\"");
                reply.extend_from_slice(direct.as_bytes());
                reply.push(b'"');
            }
            if let Some(ns) = fastest {
                reply.extend_from_slice(format!(",\"transcode_ns\":{ns}").as_bytes());
            }
            reply.push(b'}');
            reply
        }
        Ok(Err(message)) => format!("{{\"error\":{}}}", json_string(&message)).into_bytes(),
        Err(message) => format!("{{\"panic\":{}}}", json_string(&message)).into_bytes(),
    })
}
