//! How a printed path is spelled ([`quote_path`] and its byte and owned forms), and
//! which characters an ignore-file line a warning offers can spell at all
//! ([`line_can_spell`]) — the one text rule every path either `tsv` bin prints reads.

use std::borrow::Cow;

/// A path as a diagnostic or a listing spells it: verbatim, unless it holds a character
/// no line can carry — a control character (U+0000–U+001F, U+007F) or a double quote — in
/// which case the whole path is wrapped in double quotes and C-escaped, the shape git
/// prints such a path in (`core.quotePath=false`): `\a` `\b` `\t` `\n` `\v` `\f` `\r`
/// `\"` `\\` by name, any other control character as three octal digits (`\033`), and
/// every other character as itself. A backslash escapes inside a quoted path but does not
/// trigger the quoting on its own, since on Windows every path holds one — so a quoted
/// path unquotes exactly as git's does, while a plain path prints as it is.
///
/// One rule for every printed path, in both `tsv` bins, so a warning that names a path
/// stays one line (a line feed in the name split it, a carriage return garbled the
/// terminal) and a `--list` or changed-path line stays readable by the routine that reads
/// `git ls-files`: a line beginning with `"` is quoted, any other names the file exactly.
/// The re-include *patterns* a warning offers are not paths and stay literal — they are
/// meant to be pasted into an ignore file, which reads no escapes — so one that would hold
/// a control character other than a tab is not offered at all ([`line_can_spell`]); the
/// ignore-file names in prose (`the repo-root .gitignore`) hold nothing to quote.
#[must_use]
pub fn quote_path(path: &str) -> Cow<'_, str> {
    match quote_path_bytes(path.as_bytes()) {
        Cow::Borrowed(_) => Cow::Borrowed(path),
        // only ASCII bytes are rewritten, each into ASCII, so the quoted form of UTF-8 is
        // UTF-8 and the lossy conversion is a plain copy
        Cow::Owned(bytes) => Cow::Owned(String::from_utf8_lossy(&bytes).into_owned()),
    }
}

/// [`quote_path`] for a caller that already owns the path: handed back untouched when
/// nothing in it needs quoting, so the common case moves the `String` rather than
/// copying it.
#[must_use]
pub fn quote_path_owned(path: String) -> String {
    if path.bytes().any(path_byte_needs_quoting) {
        quote_path(&path).into_owned()
    } else {
        path
    }
}

/// [`quote_path`] over a path's own bytes — how a listing names a non-UTF-8 file on unix.
/// A byte outside ASCII prints as itself in either form, as git prints it.
#[must_use]
pub fn quote_path_bytes(path: &[u8]) -> Cow<'_, [u8]> {
    if !path.iter().any(|&byte| path_byte_needs_quoting(byte)) {
        return Cow::Borrowed(path);
    }
    let mut out = Vec::with_capacity(path.len() + 2);
    out.push(b'"');
    for &byte in path {
        let named = match byte {
            0x07 => b'a',
            0x08 => b'b',
            b'\t' => b't',
            b'\n' => b'n',
            0x0b => b'v',
            0x0c => b'f',
            b'\r' => b'r',
            b'"' | b'\\' => byte,
            _ if path_byte_needs_quoting(byte) => {
                out.extend_from_slice(&[
                    b'\\',
                    b'0' + (byte >> 6),
                    b'0' + ((byte >> 3) & 7),
                    b'0' + (byte & 7),
                ]);
                continue;
            }
            _ => {
                out.push(byte);
                continue;
            }
        };
        out.extend_from_slice(&[b'\\', named]);
    }
    out.push(b'"');
    Cow::Owned(out)
}

/// The bytes that make [`quote_path`] quote: the ASCII controls, and the double quote
/// that would otherwise be read as a quoted path's opening.
const fn path_byte_needs_quoting(byte: u8) -> bool {
    byte.is_ascii_control() || byte == b'"'
}

/// What a warning says in place of the re-include lines it cannot offer — a path or a
/// rule holding a control character no ignore-file line can spell ([`line_can_spell`]).
/// One spelling for both warnings; each composes its own remedy clause around it.
pub(crate) const UNSPELLABLE_REASON: &str =
    "no ignore-file line can spell the control character in its path";

/// Whether an ignore-file line a warning offers can spell `text` — a path segment, or a
/// rule's own pattern: no ASCII control character but a tab. A line feed splits any line
/// holding it and a carriage return at a line's end is stripped with the line ending, so
/// neither is a line's to hold; every other control character an ignore file could hold
/// only raw, since gitignore(5) has no escape for one — and raw is the one way a warning
/// will not print it, the very byte [`quote_path`] exists to keep off the terminal. A tab
/// is the one exception, spelled raw (`!/build/n<TAB>o.ts`), as both CLIs pin: it renders
/// as the whitespace it is, and the pasted line reads it back as the file holds it.
pub(crate) fn line_can_spell(text: &str) -> bool {
    !text
        .bytes()
        .any(|byte| byte.is_ascii_control() && byte != b'\t')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn quote_path_leaves_a_plain_path_alone() {
        for path in [
            "src/a.ts",
            "k l.ts",
            "mé.ts",
            "i\\j.ts",
            "C:\\src\\a.ts",
            "",
            "'a'.ts",
            "a`b.ts",
            ".",
        ] {
            assert!(
                matches!(quote_path(path), Cow::Borrowed(p) if p == path),
                "{path:?}"
            );
            assert!(
                matches!(quote_path_bytes(path.as_bytes()), Cow::Borrowed(_)),
                "{path:?}"
            );
        }
        // a byte outside ASCII prints as itself, as git prints it under
        // `core.quotePath=false`
        assert!(matches!(quote_path_bytes(b"t\xffu.ts"), Cow::Borrowed(_)));
        assert_eq!(quote_path_owned("k l.ts".to_owned()), "k l.ts");
        assert_eq!(quote_path_owned("a\nb.ts".to_owned()), r#""a\nb.ts""#);
    }

    #[test]
    fn quote_path_c_quotes_a_control_character_or_a_double_quote() {
        // git's own spellings (`git ls-files` under `core.quotePath=false`, git 2.47)
        for (path, quoted) in [
            ("a\nb.ts", r#""a\nb.ts""#),
            ("c\rd.ts", r#""c\rd.ts""#),
            ("e\tf.ts", r#""e\tf.ts""#),
            ("g\"h.ts", r#""g\"h.ts""#),
            ("n\u{1}o.ts", r#""n\001o.ts""#),
            ("p\u{7f}q.ts", r#""p\177q.ts""#),
            ("r\u{1b}s.ts", r#""r\033s.ts""#),
            ("x\u{b}y\u{c}z\u{8}\u{7}.ts", r#""x\vy\fz\b\a.ts""#),
            // a backslash escapes inside a quoted path, though it never triggers the quoting
            ("i\\j\n.ts", r#""i\\j\n.ts""#),
            // a character outside ASCII prints as itself even inside the quotes
            ("v\né.ts", "\"v\\né.ts\""),
            ("build/a\nb.ts", r#""build/a\nb.ts""#),
            ("\"", r#""\"""#),
        ] {
            assert_eq!(&*quote_path(path), quoted, "{path:?}");
            assert_eq!(
                &*quote_path_bytes(path.as_bytes()),
                quoted.as_bytes(),
                "{path:?}"
            );
        }
        assert_eq!(&*quote_path_bytes(b"v\n\xffw.ts"), b"\"v\\n\xffw.ts\"");
    }
}
