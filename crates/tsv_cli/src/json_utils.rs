//! JSON output utilities — the CLI's `--pretty` re-indenter.
//!
//! The parse wire is emitted compact by the writer (the sole emission path) and
//! `--pretty` re-indents those bytes **without reading them back**: a linear
//! byte walk with a depth counter, no deserializer and no tree. That is what
//! keeps `--pretty`'s depth ceiling the parser's own — a `serde_json::Value`
//! round trip recursed once per JSON level (~0.6 KiB of stack a level, measured)
//! and, at serde_json's default recursion limit of 128, refused to read a wire
//! its own writer had just emitted past ~60 nested arrays or ~40 nested objects,
//! while the compact form of the same input succeeded. `tsv_debug`, which needs
//! the `Value` tree, reads the wire through its own `json` module instead.

/// The bytes a `PrettyFormatter::with_indent(b"\t")` serialization of `compact`
/// would produce, without deserializing.
///
/// `compact` is a valid JSON document with no whitespace outside strings — the
/// writer's wire, or any `serde_json::to_vec` output. Whitespace outside strings
/// is skipped rather than assumed absent, so a hand-written document re-indents
/// too. The output is byte-identical to serializing the parsed `Value` with the
/// tab formatter (the fixture tree's `expected.json` shape): `,` becomes `,` +
/// newline + indent, `:` becomes `: `, an opening bracket newlines and indents
/// before its first member, a closing bracket newlines and dedents after its
/// last, and an empty container stays `[]` / `{}`. Strings are copied verbatim,
/// escapes included — the walk only has to find their closing quote.
///
/// Linear in the input, recursion-free, and the only state is the nesting
/// depth, so there is no depth at which this can fail: a document the parser
/// could emit is a document this can indent.
pub fn indent_json_with_tabs(compact: &[u8]) -> Vec<u8> {
    // Each member costs a newline plus its indent; a generous guess that avoids
    // the early doublings on the ~15×-source-sized wire.
    let mut out = Vec::with_capacity(compact.len() + compact.len() / 2);
    let mut depth = 0usize;
    // Set on an opening bracket and cleared by the first member (which writes the
    // newline + indent it owed) or by an immediately following closer (which
    // writes nothing — an empty container stays `[]` / `{}`).
    let mut just_opened = false;
    let mut i = 0;
    while i < compact.len() {
        let b = compact[i];
        match b {
            b'[' | b'{' => {
                if just_opened {
                    push_newline_indent(&mut out, depth);
                }
                out.push(b);
                depth += 1;
                just_opened = true;
            }
            b']' | b'}' => {
                depth = depth.saturating_sub(1);
                if !just_opened {
                    push_newline_indent(&mut out, depth);
                }
                out.push(b);
                just_opened = false;
            }
            b',' => {
                out.push(b',');
                push_newline_indent(&mut out, depth);
            }
            b':' => out.extend_from_slice(b": "),
            b' ' | b'\t' | b'\n' | b'\r' => {}
            b'"' => {
                if just_opened {
                    push_newline_indent(&mut out, depth);
                    just_opened = false;
                }
                // Copy the string through its closing quote; a backslash escapes
                // the byte after it, which is the only way a `"` appears inside.
                let start = i;
                i += 1;
                while i < compact.len() {
                    match compact[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                let end = i.min(compact.len());
                out.extend_from_slice(&compact[start..end]);
                continue;
            }
            _ => {
                // A scalar token byte (number, `true` / `false` / `null`).
                if just_opened {
                    push_newline_indent(&mut out, depth);
                    just_opened = false;
                }
                out.push(b);
            }
        }
        i += 1;
    }
    out
}

fn push_newline_indent(out: &mut Vec<u8>, depth: usize) {
    out.push(b'\n');
    out.extend(std::iter::repeat_n(b'\t', depth));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indent(compact: &str) -> String {
        String::from_utf8(indent_json_with_tabs(compact.as_bytes())).expect("utf-8 in, utf-8 out")
    }

    /// The exact `PrettyFormatter::with_indent(b"\t")` shape: newline + indent
    /// before every member, `: ` after a key, dedent before a closer.
    #[test]
    fn nested_containers_take_the_tab_formatter_shape() {
        assert_eq!(
            indent(r#"{"type":"Program","body":[{"a":1,"b":[2,3]}],"n":null}"#),
            "{\n\t\"type\": \"Program\",\n\t\"body\": [\n\t\t{\n\t\t\t\"a\": 1,\n\t\t\t\"b\": [\n\t\t\t\t2,\n\t\t\t\t3\n\t\t\t]\n\t\t}\n\t],\n\t\"n\": null\n}"
        );
    }

    /// An empty container prints as `[]` / `{}` with nothing between the
    /// brackets, at the root and as a member.
    #[test]
    fn empty_containers_stay_closed() {
        assert_eq!(indent("[]"), "[]");
        assert_eq!(indent("{}"), "{}");
        assert_eq!(
            indent(r#"{"a":[],"b":{},"c":[[]],"d":[{}]}"#),
            "{\n\t\"a\": [],\n\t\"b\": {},\n\t\"c\": [\n\t\t[]\n\t],\n\t\"d\": [\n\t\t{}\n\t]\n}"
        );
    }

    /// String content is copied verbatim: brackets, commas, colons and escaped
    /// quotes inside a string are text, not structure.
    #[test]
    fn strings_are_opaque() {
        assert_eq!(
            indent(r#"{"raw":"[{,:}]","q":"a\"b\\","u":"\u00e9 é 😀"}"#),
            "{\n\t\"raw\": \"[{,:}]\",\n\t\"q\": \"a\\\"b\\\\\",\n\t\"u\": \"\\u00e9 é 😀\"\n}"
        );
    }

    /// Scalars other than strings — numbers in every spelling the writer's
    /// `f64` emitter produces, and the keyword literals — pass through unchanged.
    #[test]
    fn scalars_pass_through() {
        assert_eq!(
            indent(r"[1,-2.5,1e21,1.7976931348623157e308,true,false,null]"),
            "[\n\t1,\n\t-2.5,\n\t1e21,\n\t1.7976931348623157e308,\n\ttrue,\n\tfalse,\n\tnull\n]"
        );
    }

    /// Whitespace outside strings is dropped, so an already-indented document
    /// re-indents to the same bytes as its compact form.
    #[test]
    fn is_idempotent_over_its_own_output() {
        let compact = r#"{"a":[1,{"b":"x y"}],"c":{}}"#;
        let once = indent(compact);
        assert_eq!(indent(&once), once);
    }

    /// Depth costs a counter, not a frame: nesting far past any deserializer's
    /// recursion limit indents in one pass. (The output is quadratic in the depth —
    /// every level's closer carries its full indent — so the depth stays modest.)
    #[test]
    fn depth_is_not_bounded() {
        let n = 3_000;
        let compact = format!("{}{}", "[".repeat(n), "]".repeat(n));
        let out = indent_json_with_tabs(compact.as_bytes());
        // n opens, then `[]` at the bottom, then n-1 closers each on its own line.
        assert!(out.starts_with(b"[\n\t[\n\t\t["));
        assert!(out.ends_with(b"\n\t]\n]"));
        let newlines = out.split(|&b| b == b'\n').count() - 1;
        assert_eq!(newlines, 2 * (n - 1));
    }
}
