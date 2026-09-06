//! The one place tsv_debug reads JSON into a tree — the wire tsv's writers emit,
//! the canonical AST the sidecar returns, and the `expected*.json` a fixture
//! stores — plus the tab-indented serialization every fixture file takes.
//!
//! Every read here runs with serde_json's recursion limit **disabled**. The
//! default limit is 128 JSON levels, and a nested array costs two of those
//! (the node object and its `elements` array), a nested object literal three
//! (`ObjectExpression` → `properties` → `Property` → `value`) — so the default
//! refused a wire past ~60 nested arrays or ~40 nested objects while `tsv parse`
//! itself reaches ~25,000, and the deepest file in the tsc corpus (208 wire
//! levels of minified asm.js) was unreadable by every audit built on the
//! `Value` tree. No oracle bounds depth by choice: `JSON.parse` in V8 and JSC is
//! iterative and takes a million levels, and acorn, Svelte's parser and prettier
//! each stop only at V8's stack (acorn + acorn-typescript at 1,023 nested
//! arrays, prettier's `typescript` parser at 767), so a tsv reader bounded at 60
//! was the outlier by an order of magnitude.
//!
//! What makes an unbounded read sound is where it runs: every tsv_debug thread
//! reserves `STACK_SIZE` (`main` through `run_on_sized_stack`, the tokio
//! runtime through `thread_stack_size`, the audit pools through
//! `sized_thread`), and a `Value` read costs ~0.6 KiB of stack per JSON level
//! (measured: 0.595, the same for the deserialize, drop, `==` and pretty-print
//! walks). Per SOURCE level that is ~1.2 KiB on a nested array against the
//! parser's ~1.14 and ~1.8 KiB on a nested object against the parser's ~2.4, so
//! on the 32 MiB reservation the read reaches ~27,500 nested arrays where the
//! parse reaches ~28,000: a 2% band, on input 400× deeper than any corpus, in
//! which a dev tool would overflow instead of erroring. Everywhere else the
//! parser is the binding ceiling, which is the ceiling tsv documents
//! (`docs/cli.md` §Recursion Depth). The shipped CLI's `--pretty` does not read
//! at all — it re-indents the wire bytes in one pass (`tsv_cli::json_utils`) —
//! and no other shipped artifact deserializes.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Deserialize `bytes` with no recursion limit — the reader for every wire tsv
/// or an oracle emitted. See the module docs for why the limit is off and why
/// that is safe on tsv_debug's threads.
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> serde_json::Result<T> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    de.disable_recursion_limit();
    let value = T::deserialize(&mut de)?;
    de.end()?;
    Ok(value)
}

/// [`from_slice`] over a `&str`.
pub fn from_str<T: DeserializeOwned>(s: &str) -> serde_json::Result<T> {
    from_slice(s.as_bytes())
}

/// The `Value` tree of a wire tsv's own writer emitted (`convert_ast_json_bytes`
/// and its siblings).
///
/// This is the one read that may `expect`: with the recursion limit off, the
/// only way the parse of those bytes fails is malformed JSON, and the writer
/// emitting malformed JSON is a bug in the sole emission path that every
/// fixture would also catch — an invariant, not an input. (The wrappers this
/// replaces, one `convert_ast_json` per language crate, carried the same
/// message over a *bounded* read, and blamed the writer for the reader's
/// limit whenever a deep-enough file reached them.)
#[expect(
    clippy::expect_used,
    reason = "an unbounded read of the writer's own bytes fails only on a writer bug"
)]
pub fn wire_value(bytes: &[u8]) -> Value {
    from_slice(bytes).expect("writer emits valid JSON")
}

/// Serialize to JSON with tab indentation — the `expected*.json` shape.
///
/// `serde_json::to_string_pretty` uses 2 spaces by default; this uses tabs to
/// match the workspace's formatting conventions. The CLI's `--pretty` produces
/// the same bytes from the compact wire without a tree
/// (`tsv_cli::json_utils::indent_json_with_tabs`), and the equivalence of the
/// two is pinned by the `cli_reindent_matches_the_tree_serialization` test
/// below over every fixture input.
pub fn to_json_with_tabs<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut ser)?;
    // SAFETY: serde_json always produces valid UTF-8
    #[expect(clippy::unwrap_used)]
    Ok(String::from_utf8(buf).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tsv_cli::cli::input::ParserType;

    /// serde_json's default limit refuses a 128-level document; this reader
    /// takes it — and takes one an order of magnitude deeper on the stack every
    /// tsv_debug thread reserves (a bare test thread is 2 MiB, and a debug-profile
    /// frame is many times a release one, so the deep case runs where the tool does).
    #[test]
    fn reads_past_the_default_recursion_limit() {
        let n = 200;
        let deep = format!("{}{}", "[".repeat(n), "]".repeat(n));
        assert!(
            serde_json::from_str::<Value>(&deep).is_err(),
            "the default reader must refuse this, or the test proves nothing"
        );
        let value: Value = from_str(&deep).expect("unbounded read");
        assert!(value.is_array());

        let deep = tsv_cli::cli::stack::run_on_sized_stack(|| {
            let n = 1_000;
            let deep = format!("{}{}", "[".repeat(n), "]".repeat(n));
            let value: Value = from_str(&deep).expect("unbounded read");
            value.is_array()
        });
        assert!(deep);
    }

    /// Trailing bytes after the document are still an error — the read is
    /// unbounded in depth, not lax in syntax.
    #[test]
    fn rejects_trailing_garbage() {
        assert!(from_str::<Value>("{} x").is_err());
        assert!(from_str::<Value>("{").is_err());
    }

    /// The CLI's recursion-free re-indenter and the tree serialization agree
    /// byte for byte over every fixture input, in every language, both wires.
    #[test]
    fn cli_reindent_matches_the_tree_serialization() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let mut checked = 0usize;
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("fixtures dir") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with("input.") || name.starts_with("input_invalid") {
                    continue;
                }
                let Ok(source) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let parser = ParserType::from_extension(&path.to_string_lossy());
                let arena = bumpalo::Bump::new();
                let wires: Vec<Vec<u8>> = match parser {
                    ParserType::TypeScript => {
                        let Ok(ast) = tsv_ts::parse(&source, &arena) else {
                            continue;
                        };
                        vec![
                            tsv_ts::convert_ast_json_bytes(&ast, &source),
                            tsv_ts::convert_ast_json_bytes_no_locations(&ast, &source),
                        ]
                    }
                    ParserType::Svelte => {
                        let Ok(ast) = tsv_svelte::parse(&source, &arena) else {
                            continue;
                        };
                        vec![
                            tsv_svelte::convert_ast_json_bytes(&ast, &source),
                            tsv_svelte::convert_ast_json_bytes_no_locations(&ast, &source),
                        ]
                    }
                    ParserType::Css => {
                        let Ok(ast) = tsv_css::parse(&source, &arena) else {
                            continue;
                        };
                        vec![tsv_css::convert_ast_json_bytes(&ast, &source)]
                    }
                };
                for wire in wires {
                    let via_tree = to_json_with_tabs(&wire_value(&wire)).expect("serialize");
                    let via_bytes = tsv_cli::json_utils::indent_json_with_tabs(&wire);
                    assert_eq!(
                        via_bytes,
                        via_tree.as_bytes(),
                        "re-indent diverges from the tree serialization on {}",
                        path.display()
                    );
                    checked += 1;
                }
            }
        }
        assert!(
            checked > 4_000,
            "walked only {checked} wires — the fixture tree moved?"
        );
    }
}
