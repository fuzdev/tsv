//! The **comment census** — lex comment trivia straight off raw text, so a
//! parse-time comment drop is visible to an oracle that never consulted the
//! parser's own comment carrying.
//!
//! Every other comment gate reads the comments a format entry point REGISTERED
//! (the print-once ledger) or the list the parser produced (`parse().comments`)
//! — which inherits exactly the registration holes it should be checking: a
//! comment a parse path consumes without registering (the CSS
//! `skip_whitespace_and_comments` class) never existed as far as those
//! instruments know, so its drop is invisible and the corpus stays green **by
//! absence**. The census's whole design is independence from that channel: it
//! scans the raw INPUT and the raw OUTPUT with its own trivia scanners and
//! compares the two comment-interior **multisets**. A comment the parser lost is
//! then a plain arithmetic fact — present in the input scan, absent from the
//! output scan — no matter which internal layer lost it.
//!
//! Three scanners, one per language surface, self-contained in this module (they
//! deliberately do NOT drive the product lexers: TS comment extents depend on
//! parser context — a regex body is opaque only because the parser said "regex
//! here" — so a raw `next_token` loop mis-lexes real code and ERRORs on any
//! regex containing `\`; and an instrument that shared the product lexer's
//! extent rules would inherit its bugs):
//!
//! - **TS/JS** — `//` + `/* */` (+ the byte-0 `#!` hashbang), with strings,
//!   template literals (interpolation stack included), and regex literals
//!   opaque. Regex-vs-division uses the classic previous-significant-token
//!   heuristic; where it misreads, it misreads the input and the output with the
//!   same eyes, so the phantoms cancel in the multiset.
//! - **CSS** — `/* */`, with strings and unquoted `url()` tokens opaque, and one
//!   structural carve-out: a `<!-- ... -->` CDO/CDC span is skipped WHOLESALE,
//!   because tsv (matching Svelte's `parseCss`) discards the entire span
//!   including any CSS between the markers — the **one sanctioned whole-comment
//!   drop** in the tool, expressed here as "the census never counts those
//!   comments" rather than as an exemption downstream.
//! - **Svelte** — a lexical mode machine over the document: `<!-- -->` template
//!   comments; `<script>` / `<style>` raw-text islands (bounded by the first
//!   matching close tag, exactly Svelte's own lexical rule) handed to the TS /
//!   CSS scanners; `{...}` expressions — in text, in attribute position, and
//!   inside quoted attribute values — handed to the TS scanner in expression
//!   mode (which returns at the first unmatched `}`). A `{#if}`/`{:else}`/
//!   `{/if}`/`{@html}` sigil + keyword is stepped over before the expression
//!   scan so the `/` of a block close tag is never mistaken for a regex.
//!
//! **Normalization is a `<CR>` fold plus the line-edge trim the PRINTER is
//! licensed to make, and no more** — which is a different trim per comment kind,
//! because prettier's `printComment` is. A line comment (and the hashbang) is
//! emitted `.trimEnd()`-ed; an INDENTABLE block is reindented and its lines
//! trimmed; every other block — single-line, or multi-line and non-indentable —
//! is emitted verbatim and gets no trim at all. Anything the printer may not do
//! compares byte-exact. See `normalize_interior`.
//!
//! The consumer is `census_audit` (`deno task census:audit`), which formats each
//! pristine seed, runs `comment_census` on both sides, and ratchets the per-file
//! deltas.

use std::collections::BTreeMap;

use tsv_cli::cli::input::ParserType;

/// Which language surface a comment was lexed in. Svelte documents fan out into
/// all three; standalone files use their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CensusBucket {
    /// TS/JS: a standalone `.ts`-family file, a `<script>` island, or a template
    /// `{expression}`.
    Ts,
    /// CSS: a standalone `.css` file or a `<style>` island.
    Css,
    /// The Svelte template's own `<!-- -->` comments.
    Template,
}

impl CensusBucket {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            CensusBucket::Ts => "ts",
            CensusBucket::Css => "css",
            CensusBucket::Template => "template",
        }
    }
}

/// The comment's delimiter kind. Part of the multiset key so a (never expected)
/// delimiter rewrite reads as a drop + an add rather than silently matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CensusKind {
    /// `//` (and the byte-0 `#!` hashbang, whose content includes the `#!`).
    Line,
    /// `/* */`
    Block,
    /// `<!-- -->`
    Html,
}

impl CensusKind {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            CensusKind::Line => "line",
            CensusKind::Block => "block",
            CensusKind::Html => "html",
        }
    }
}

/// One normalized comment occurrence class: where it was lexed, its delimiter
/// kind, and its per-line-trimmed interior.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CensusEntry {
    pub(crate) bucket: CensusBucket,
    pub(crate) kind: CensusKind,
    pub(crate) content: String,
}

/// The census product: normalized comment classes with their occurrence counts.
pub(crate) type CensusMultiset = BTreeMap<CensusEntry, usize>;

/// Lex the comment trivia of `source` into its census multiset.
pub(crate) fn comment_census(source: &str, parser: ParserType) -> CensusMultiset {
    let mut out = CensusMultiset::new();
    match parser {
        ParserType::TypeScript => scan_ts_file(source, &mut out),
        ParserType::Css => scan_css(source, CensusBucket::Css, &mut out),
        ParserType::Svelte => scan_svelte(source, &mut out),
    }
    out
}

/// The line-edge trim a comment interior is licensed to lose — the census's ONE
/// normalization, and it is **kind-aware**, because the printer is.
///
/// The rule is prettier's `printComment` transcribed, since that is what tsv mirrors:
///
/// | kind | what the printer emits | trimmed here |
/// | --- | --- | --- |
/// | line / hashbang | `originalText.slice(…).trimEnd()` | trailing, JS `\s` |
/// | block, INDENTABLE | reindented, each line trimmed | per line, JS `\s` |
/// | block, anything else | verbatim (`replaceEndOfLine(value)`) | nothing |
///
/// ⚠️ The class is **[`tsv_lang::is_js_whitespace`], not ASCII `[ \t]` and not Rust's
/// `White_Space`** — because the trims it models are JS `String.prototype.trim*` calls.
/// An ASCII-narrow class, on the argument that an NBSP at a line edge is CONTENT, is wrong
/// at a line comment's end and inside an indentable block (prettier deletes it there, and
/// so does tsv), where the census would read a sanctioned trim as a rewrite.
/// The narrowness that argument was protecting is kept where it is still true, by giving
/// the verbatim kinds **no trim at all** — which is stricter than the old blanket
/// `[ \t]`, not looser.
///
/// The `<CR>` fold comes FIRST, and is the FORMAT PATH's own
/// ([`tsv_lang::printing::normalize_carriage_returns`]) rather than a second copy, so the
/// two cannot drift. The formatter applies it to its input ahead of the parse, so a `<CR>`
/// the author wrote inside a comment is an `<LF>` on the other side of this diff — and this
/// side has to make the same move to compare like with like. Splitting on `\n` alone left
/// the two disagreeing about where the LINES are — the input's lone `<CR>` stayed interior
/// content while the output's `<LF>` split and took the per-line trim — and the census
/// reported a MISSING/EXTRA pair for a comment nothing had rewritten. `<LS>` / `<PS>` are
/// deliberately NOT folded: the formatter does not fold them either, so folding here would
/// make `a<LS>b` and `a<LF>b` compare equal and blind the census to a real rewrite. `\r`
/// left the trim class in the same step — after the fold there is none left to trim.
fn normalize_interior(kind: CensusKind, raw: &str) -> String {
    let raw = tsv_lang::printing::normalize_carriage_returns(raw).into_text();
    match kind {
        // Runs to end of line, so it has exactly one edge the printer touches.
        CensusKind::Line => raw.trim_end_matches(tsv_lang::is_js_whitespace).to_owned(),
        // A Svelte `<!-- … -->` is a template NODE, emitted verbatim by both formatters —
        // interior columns and trailing spaces included (measured; the only thing that moves
        // is the opener's own indent, which is outside the interior).
        CensusKind::Html => raw.into_owned(),
        // Only the `*`-aligned form reindents; every other block is copied verbatim, so a
        // line edge there is content and stays byte-exact.
        CensusKind::Block if !tsv_lang::printing::is_indentable_block_comment(raw.split('\n')) => {
            raw.into_owned()
        }
        CensusKind::Block => {
            let mut out = String::with_capacity(raw.len());
            for (i, line) in raw.split('\n').enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                out.push_str(line.trim_matches(tsv_lang::is_js_whitespace));
            }
            out
        }
    }
}

fn record(out: &mut CensusMultiset, bucket: CensusBucket, kind: CensusKind, raw: &str) {
    let entry = CensusEntry {
        bucket,
        kind,
        content: normalize_interior(kind, raw),
    };
    *out.entry(entry).or_insert(0) += 1;
}

/// `(content_end, resume_pos)` for a delimited comment whose content begins at
/// `content_start` and whose closer is `close` (`*/` / `-->`). Unterminated
/// runs to EOF — the same tolerance on both sides of the diff, so the truncated
/// interiors still compare. The one definition of the close-or-EOF rule, shared
/// by every delimited scan (TS block, CSS block, Svelte html, the CDO/CDC skip).
fn find_close(src: &str, content_start: usize, close: &str) -> (usize, usize) {
    match src[content_start..].find(close) {
        Some(i) => (content_start + i, content_start + i + close.len()),
        None => (src.len(), src.len()),
    }
}

// ---------------------------------------------------------------------------
// TS/JS
// ---------------------------------------------------------------------------

/// The offset of the first ECMAScript LineTerminator at or after the start of
/// `s` — `\n`, `\r`, U+2028, or U+2029 — or `s.len()`. A line comment (and the
/// hashbang) ends at any of the four, and the formatter agrees, so an
/// instrument that only honored `\n` would read the separator and the code
/// after it as comment content on the input side.
fn line_terminator_offset(s: &str) -> usize {
    s.char_indices()
        .find(|&(_, c)| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
        .map_or(s.len(), |(i, _)| i)
}

/// Scan a whole TS/JS file: the byte-0 `#!` hashbang (content includes the `#!`,
/// mirroring `tsv_lang::Comment`), then ordinary code.
fn scan_ts_file(source: &str, out: &mut CensusMultiset) {
    let mut start = 0;
    // A BOM may precede the hashbang check in neither grammar; tsv treats the
    // hashbang as strictly byte-0, so the census does too.
    if source.starts_with("#!") {
        let end = line_terminator_offset(source);
        record(out, CensusBucket::Ts, CensusKind::Line, &source[..end]);
        start = end;
    }
    let mut scanner = TsScanner::new(&source[start..], CensusBucket::Ts, out);
    scanner.scan(false);
}

/// Scan a Svelte `{`-delimited expression slice (everything after the `{`).
/// Returns the offset of the unmatched `}` relative to `slice` (not consumed),
/// or `slice.len()` when the document ends first.
fn scan_ts_expression(slice: &str, out: &mut CensusMultiset) -> usize {
    let mut scanner = TsScanner::new(slice, CensusBucket::Ts, out);
    scanner.scan(true);
    scanner.pos
}

/// The words after which a `/` begins a regex literal rather than division —
/// the classic previous-significant-token heuristic's keyword half.
fn is_regex_preceding_keyword(word: &str) -> bool {
    matches!(
        word,
        "return"
            | "typeof"
            | "instanceof"
            | "in"
            | "of"
            | "new"
            | "delete"
            | "void"
            | "throw"
            | "case"
            | "do"
            | "else"
            | "yield"
            | "await"
    )
}

struct TsScanner<'a, 'o> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    /// Whether a `/` at the cursor would start a REGEX (true after an operator,
    /// an opening bracket, a regex-preceding keyword, or at entry) or DIVISION
    /// (after a value: identifier, literal, `)`, `]`, `}`, postfix `++`/`--`).
    regex_allowed: bool,
    /// Brace depth of the current code context (relative to the scan entry or
    /// the innermost template interpolation).
    depth: u32,
    /// Saved brace depths of enclosing template interpolations: entering
    /// `` `...${ `` pushes the current depth; the matching `}` pops it and
    /// resumes the template's content.
    frames: Vec<u32>,
    bucket: CensusBucket,
    out: &'o mut CensusMultiset,
}

impl<'a, 'o> TsScanner<'a, 'o> {
    fn new(src: &'a str, bucket: CensusBucket, out: &'o mut CensusMultiset) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            regex_allowed: true,
            depth: 0,
            frames: Vec::new(),
            bucket,
            out,
        }
    }

    fn peek(&self, ahead: usize) -> Option<u8> {
        self.bytes.get(self.pos + ahead).copied()
    }

    /// The main code loop. With `expression_mode`, returns (cursor ON the brace)
    /// at the first `}` that closes nothing opened inside the scan.
    fn scan(&mut self, expression_mode: bool) {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            match b {
                b'/' if self.peek(1) == Some(b'/') => self.line_comment(),
                b'/' if self.peek(1) == Some(b'*') => self.block_comment(),
                b'/' => {
                    if self.regex_allowed {
                        self.regex();
                    } else {
                        // Division (or `/=`): the next `/` position is regex again.
                        self.pos += 1;
                        self.regex_allowed = true;
                    }
                }
                b'\'' | b'"' => self.string(b),
                b'`' => {
                    self.pos += 1;
                    self.template_content();
                }
                b'{' => {
                    self.depth += 1;
                    self.pos += 1;
                    self.regex_allowed = true;
                }
                b'}' => {
                    if self.depth == 0 {
                        if let Some(depth) = self.frames.pop() {
                            // The `}` closes a template interpolation: resume
                            // the template's content.
                            self.pos += 1;
                            self.depth = depth;
                            self.template_content();
                        } else if expression_mode {
                            return;
                        } else {
                            // A stray close brace in a standalone file —
                            // tolerate; the file wouldn't parse anyway.
                            self.pos += 1;
                            self.regex_allowed = false;
                        }
                    } else {
                        self.depth -= 1;
                        self.pos += 1;
                        self.regex_allowed = false;
                    }
                }
                b'(' | b'[' => {
                    self.pos += 1;
                    self.regex_allowed = true;
                }
                b')' | b']' => {
                    self.pos += 1;
                    self.regex_allowed = false;
                }
                // Postfix `++` / `--` leave a value, so division follows
                // (`x++ / 2`). The prefix form is never directly followed by
                // `/`, so classifying both as value-leaving is safe.
                b'+' if self.peek(1) == Some(b'+') => {
                    self.pos += 2;
                    self.regex_allowed = false;
                }
                b'-' if self.peek(1) == Some(b'-') => {
                    self.pos += 2;
                    self.regex_allowed = false;
                }
                b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c => self.pos += 1,
                _ if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'#') => self.word(),
                _ if b < 0x80 => {
                    // Any other ASCII punctuator: an operator, so regex position.
                    self.pos += 1;
                    self.regex_allowed = true;
                }
                _ => {
                    // Non-ASCII: whitespace is skipped, anything else is
                    // identifier-ish (a value). Defensive `get` so a cursor that
                    // somehow landed mid-char escapes by one byte instead of
                    // panicking the instrument.
                    match self.src.get(self.pos..).and_then(|s| s.chars().next()) {
                        Some(c) if c.is_whitespace() => self.pos += c.len_utf8(),
                        Some(_) => self.word(),
                        None => self.pos += 1,
                    }
                }
            }
        }
    }

    fn line_comment(&mut self) {
        let content_start = self.pos + 2;
        let end = content_start + line_terminator_offset(&self.src[content_start..]);
        record(
            self.out,
            self.bucket,
            CensusKind::Line,
            &self.src[content_start..end],
        );
        self.pos = end;
    }

    fn block_comment(&mut self) {
        let content_start = self.pos + 2;
        let (content_end, resume) = find_close(self.src, content_start, "*/");
        record(
            self.out,
            self.bucket,
            CensusKind::Block,
            &self.src[content_start..content_end],
        );
        self.pos = resume;
        // Trivia: `regex_allowed` deliberately unchanged (`a = /* c */ /re/`).
    }

    fn string(&mut self, quote: u8) {
        self.pos += 1;
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b'\\' => self.pos = (self.pos + 2).min(self.bytes.len()),
                // Unterminated — tolerate, leave the terminator. (U+2028/2029
                // are legal INSIDE a string since ES2019, so only these two.)
                b'\n' | b'\r' => break,
                b if b == quote => {
                    self.pos += 1;
                    break;
                }
                _ => self.pos += 1,
            }
        }
        self.regex_allowed = false;
    }

    /// Template content after a `` ` `` or a `${...}`-closing `}`. Returns to the
    /// code loop on `${` (frame pushed) or after the closing backtick.
    fn template_content(&mut self) {
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b'\\' => self.pos = (self.pos + 2).min(self.bytes.len()),
                b'`' => {
                    self.pos += 1;
                    self.regex_allowed = false;
                    return;
                }
                b'$' if self.peek(1) == Some(b'{') => {
                    self.pos += 2;
                    self.frames.push(self.depth);
                    self.depth = 0;
                    self.regex_allowed = true;
                    return;
                }
                _ => self.pos += 1,
            }
        }
    }

    fn regex(&mut self) {
        let slash = self.pos;
        self.pos += 1;
        let mut in_class = false;
        loop {
            let Some(b) = self.bytes.get(self.pos).copied() else {
                return; // unterminated at EOF
            };
            match b {
                b'\\' => self.pos = (self.pos + 2).min(self.bytes.len()),
                b'[' => {
                    in_class = true;
                    self.pos += 1;
                }
                b']' => {
                    in_class = false;
                    self.pos += 1;
                }
                b'/' if !in_class => {
                    self.pos += 1;
                    break;
                }
                b'\n' | b'\r' => {
                    // A regex body can't span a line — the heuristic misjudged;
                    // re-read the original `/` as division.
                    self.pos = slash + 1;
                    self.regex_allowed = true;
                    return;
                }
                _ => self.pos += 1,
            }
        }
        // Flags.
        while self
            .bytes
            .get(self.pos)
            .is_some_and(|&b| b.is_ascii_alphabetic())
        {
            self.pos += 1;
        }
        self.regex_allowed = false;
    }

    /// An identifier / keyword / number / private name — a value, unless it is
    /// one of the regex-preceding keywords.
    fn word(&mut self) {
        let start = self.pos;
        while self
            .bytes
            .get(self.pos)
            .is_some_and(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$') || b >= 0x80)
        {
            self.pos += 1;
        }
        if self.pos == start {
            // A lone `#` (word-start byte that doesn't continue): consume it.
            self.pos += 1;
            self.regex_allowed = false;
            return;
        }
        self.regex_allowed = is_regex_preceding_keyword(&self.src[start..self.pos]);
    }
}

// ---------------------------------------------------------------------------
// CSS
// ---------------------------------------------------------------------------

/// Whether `b` can end a CSS identifier-ish run — the `5url(` guard's notion of
/// "the `url` is glued to something", mirroring the lexer's token-start test.
fn is_css_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-') || b >= 0x80
}

fn scan_css(src: &str, bucket: CensusBucket, out: &mut CensusMultiset) {
    let bytes = src.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        match bytes[pos] {
            b'/' if bytes.get(pos + 1) == Some(&b'*') => {
                let (content_end, resume) = find_close(src, pos + 2, "*/");
                record(out, bucket, CensusKind::Block, &src[pos + 2..content_end]);
                pos = resume;
            }
            q @ (b'\'' | b'"') => {
                pos += 1;
                while pos < bytes.len() {
                    match bytes[pos] {
                        b'\\' => pos = (pos + 2).min(bytes.len()),
                        b'\n' => break, // bad-string ends at the newline
                        b if b == q => {
                            pos += 1;
                            break;
                        }
                        _ => pos += 1,
                    }
                }
            }
            // An escape is opaque: `\*` must not open (or close) anything. The
            // byte after may be a multibyte char's lead; its continuation bytes
            // match no arm below, so skipping 2 is safe.
            b'\\' => pos = (pos + 2).min(bytes.len()),
            // The ONE sanctioned whole-comment drop: tsv (matching `parseCss`)
            // discards a `<!-- ... -->` CDO/CDC span WHOLESALE, any CSS (and any
            // comment) between the markers included. The census skips the same
            // span so those comments never enter the input multiset.
            b'<' if src[pos..].starts_with("<!--") => {
                pos = find_close(src, pos + 4, "-->").1;
            }
            b'u' | b'U'
                if src[pos..].len() >= 4
                    && src.as_bytes()[pos..pos + 4].eq_ignore_ascii_case(b"url(")
                    && (pos == 0 || !is_css_ident_byte(bytes[pos - 1])) =>
            {
                // Peek past the `(` and any whitespace: a quote means an
                // ordinary function with a string argument (scanned by the
                // arms above); anything else is an opaque `<url-token>` whose
                // interior is literal content, comment lookalikes included.
                let mut j = pos + 4;
                while bytes
                    .get(j)
                    .is_some_and(|&b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c))
                {
                    j += 1;
                }
                if matches!(bytes.get(j), Some(b'\'' | b'"')) {
                    pos += 4;
                } else {
                    pos = j;
                    while pos < bytes.len() {
                        match bytes[pos] {
                            b'\\' => pos = (pos + 2).min(bytes.len()),
                            b')' => {
                                pos += 1;
                                break;
                            }
                            _ => pos += 1,
                        }
                    }
                }
            }
            _ => pos += 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Svelte
// ---------------------------------------------------------------------------

fn scan_svelte(src: &str, out: &mut CensusMultiset) {
    let bytes = src.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        match bytes[pos] {
            b'{' => pos = scan_svelte_brace(src, pos, out),
            b'<' => {
                if src[pos..].starts_with("<!--") {
                    let (content_end, resume) = find_close(src, pos + 4, "-->");
                    record(
                        out,
                        CensusBucket::Template,
                        CensusKind::Html,
                        &src[pos + 4..content_end],
                    );
                    pos = resume;
                } else if src[pos..].starts_with("</") {
                    // A close tag holds no attributes; scan to its `>`.
                    pos = src[pos..].find('>').map_or(bytes.len(), |i| pos + i + 1);
                } else if bytes
                    .get(pos + 1)
                    .is_some_and(|&b| b.is_ascii_alphabetic() || b == b'!')
                {
                    pos = scan_svelte_tag(src, pos, out);
                } else {
                    // A literal `<` in text (`a < b`).
                    pos += 1;
                }
            }
            _ => pos += 1,
        }
    }
}

/// A `{...}` in template or attribute position: step over a block/tag sigil and
/// its keyword (`{#if`, `{:else`, `{/if`, `{@html` — so the `/` of a block close
/// is never read as a regex head), then TS-scan to the matching `}`.
/// Returns the position after the closing `}`.
///
/// The sigil test is deliberately narrow, because a `{` may open an ordinary
/// expression whose first token *looks* like a sigil: `{/* c */ x}` (leading
/// block comment) and `{/re/.test(x)}` (leading regex) both begin `{/`. So `/`
/// counts as a sigil only when its word is one of Svelte's five block-close
/// keywords, and `#`/`:`/`@` only when a letter follows (a JS expression can
/// start with none of those in template position).
fn scan_svelte_brace(src: &str, brace: usize, out: &mut CensusMultiset) -> usize {
    let bytes = src.as_bytes();
    let mut i = brace + 1;
    let sigil = bytes.get(i).copied().filter(|b| {
        matches!(b, b'#' | b':' | b'/' | b'@')
            && bytes.get(i + 1).is_some_and(u8::is_ascii_alphabetic)
    });
    if let Some(sigil) = sigil {
        let word_start = i + 1;
        let mut word_end = word_start;
        while bytes.get(word_end).is_some_and(u8::is_ascii_alphanumeric) {
            word_end += 1;
        }
        let is_sigil = sigil != b'/'
            || matches!(
                &src[word_start..word_end],
                "if" | "each" | "await" | "key" | "snippet"
            );
        if is_sigil {
            i = word_end;
        }
    }
    let rel = scan_ts_expression(&src[i..], out);
    (i + rel + 1).min(src.len())
}

/// An open tag from its `<`. Scans the attribute list (expressions and quoted
/// values included), then — for `<script>` / `<style>` — the raw-text island up
/// to the first matching close tag, exactly Svelte's own lexical rule (a
/// `</script>` inside a JS string DOES end the island, so scanning the bounded
/// slice matches what the real parser sees). Returns the position after the
/// tag (or after the island's close tag).
fn scan_svelte_tag(src: &str, lt: usize, out: &mut CensusMultiset) -> usize {
    let bytes = src.as_bytes();
    let mut i = lt + 1;
    let name_start = i;
    while bytes.get(i).is_some_and(|&b| {
        b.is_ascii_alphanumeric() || matches!(b, b'-' | b':' | b'.' | b'_' | b'!')
    }) {
        i += 1;
    }
    let name = &src[name_start..i];

    // Attribute list.
    let mut self_closing = false;
    while i < bytes.len() {
        match bytes[i] {
            b'>' => {
                i += 1;
                break;
            }
            b'/' if bytes.get(i + 1) == Some(&b'>') => {
                self_closing = true;
                i += 2;
                break;
            }
            b'{' => i = scan_svelte_brace(src, i, out),
            q @ (b'"' | b'\'') => i = scan_quoted_attr_value(src, i, q, out),
            b'=' => {
                i += 1;
                while bytes
                    .get(i)
                    .is_some_and(|&b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
                {
                    i += 1;
                }
                match bytes.get(i) {
                    Some(&q @ (b'"' | b'\'')) => i = scan_quoted_attr_value(src, i, q, out),
                    Some(b'{') => i = scan_svelte_brace(src, i, out),
                    _ => {
                        // Unquoted value: to whitespace, `>`, or a `{` opening
                        // an embedded expression.
                        while bytes.get(i).is_some_and(|&b| {
                            !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'{')
                        }) {
                            i += 1;
                        }
                    }
                }
            }
            _ => i += 1,
        }
    }

    if self_closing {
        return i;
    }
    match name {
        "script" => scan_raw_island(src, i, "</script", ScanIsland::Ts, out),
        "style" => scan_raw_island(src, i, "</style", ScanIsland::Css, out),
        _ => i,
    }
}

/// A quoted attribute value from its opening quote. `{...}` inside is a live
/// expression (Svelte's mixed text-and-expression values), handed to the
/// expression scanner — whose own string handling is what keeps a quote INSIDE
/// the expression from closing the attribute. No backslash escapes (HTML has
/// none; entities are content). Returns the position after the closing quote.
fn scan_quoted_attr_value(src: &str, qpos: usize, quote: u8, out: &mut CensusMultiset) -> usize {
    let bytes = src.as_bytes();
    let mut i = qpos + 1;
    while i < bytes.len() {
        match bytes[i] {
            b if b == quote => return i + 1,
            b'{' => i = scan_svelte_brace(src, i, out),
            _ => i += 1,
        }
    }
    i
}

enum ScanIsland {
    Ts,
    Css,
}

/// A raw-text island from just past its open tag's `>`: bounded by the first
/// `close` (`</script` / `</style`) at a tag boundary, or EOF. The interior is
/// handed to the island's language scanner; the close tag is then skipped.
fn scan_raw_island(
    src: &str,
    content_start: usize,
    close: &str,
    island: ScanIsland,
    out: &mut CensusMultiset,
) -> usize {
    let bytes = src.as_bytes();
    let mut search = content_start;
    let content_end = loop {
        match src[search..].find(close) {
            Some(i) => {
                let at = search + i;
                let after = at + close.len();
                // A real close tag ends here or continues with whitespace/`>`;
                // `</scripty` is an ordinary (broken) tag, keep looking.
                if bytes
                    .get(after)
                    .is_none_or(|&b| matches!(b, b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/'))
                {
                    break at;
                }
                search = at + 1;
            }
            None => break src.len(),
        }
    };
    let content = &src[content_start..content_end];
    match island {
        ScanIsland::Ts => {
            let mut scanner = TsScanner::new(content, CensusBucket::Ts, out);
            scanner.scan(false);
        }
        ScanIsland::Css => scan_css(content, CensusBucket::Css, out),
    }
    if content_end >= src.len() {
        return src.len();
    }
    src[content_end..]
        .find('>')
        .map_or(src.len(), |i| content_end + i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn census(source: &str, parser: ParserType) -> Vec<(CensusBucket, CensusKind, String, usize)> {
        comment_census(source, parser)
            .into_iter()
            .map(|(e, n)| (e.bucket, e.kind, e.content, n))
            .collect()
    }

    fn ts(source: &str) -> Vec<(CensusBucket, CensusKind, String, usize)> {
        census(source, ParserType::TypeScript)
    }

    #[test]
    fn ts_line_and_block_comments() {
        assert_eq!(
            ts("// a\nconst x = 1; /* b */\n"),
            vec![
                (CensusBucket::Ts, CensusKind::Line, " a".into(), 1),
                (CensusBucket::Ts, CensusKind::Block, " b ".into(), 1),
            ]
        );
    }

    #[test]
    fn ts_duplicate_contents_count() {
        assert_eq!(
            ts("// a\n// a\n"),
            vec![(CensusBucket::Ts, CensusKind::Line, " a".into(), 2)]
        );
    }

    #[test]
    fn ts_strings_and_templates_are_opaque() {
        assert!(ts("const a = '// no'; const b = \"/* no */\";").is_empty());
        assert!(ts("const t = `/* no */ ${x} // no`;").is_empty());
        // Post-interpolation template text is still template, not code.
        assert_eq!(
            ts("const t = `a ${x /* yes */} b // no`; // yes\n"),
            vec![
                (CensusBucket::Ts, CensusKind::Line, " yes".into(), 1),
                (CensusBucket::Ts, CensusKind::Block, " yes ".into(), 1),
            ]
        );
        // Nested interpolations resume correctly.
        assert_eq!(
            ts("const t = `${`inner ${y /* c */}`} // no`;"),
            vec![(CensusBucket::Ts, CensusKind::Block, " c ".into(), 1)]
        );
    }

    #[test]
    fn ts_regex_bodies_are_opaque() {
        // `/a[/*]b/` holds a comment-opener lookalike inside a class.
        assert_eq!(
            ts("const re = /a[/*]b\\//; // real\n"),
            vec![(CensusBucket::Ts, CensusKind::Line, " real".into(), 1)]
        );
        // After a value, `/` is division and the comment is real.
        assert_eq!(
            ts("const x = a / b; // real\n"),
            vec![(CensusBucket::Ts, CensusKind::Line, " real".into(), 1)]
        );
        // After a regex-preceding keyword, regex; its `//` is body, not comment.
        assert!(ts("return /a\\/\\/b/;").is_empty());
    }

    #[test]
    fn ts_line_comments_end_at_every_line_terminator() {
        // U+2028 / U+2029 terminate a line comment (and the hashbang) exactly
        // as the real lexer's — the separator and the code after it are NOT
        // comment content.
        assert_eq!(
            ts("// a\u{2028}const x = 1; // b\u{2029}const y = 2;"),
            vec![
                (CensusBucket::Ts, CensusKind::Line, " a".into(), 1),
                (CensusBucket::Ts, CensusKind::Line, " b".into(), 1),
            ]
        );
        assert_eq!(
            ts("#!hb\u{2028}const a = 1;"),
            vec![(CensusBucket::Ts, CensusKind::Line, "#!hb".into(), 1)]
        );
    }

    #[test]
    fn ts_hashbang_is_a_line_comment_including_the_marker() {
        assert_eq!(
            ts("#!/usr/bin/env node\nconst a = 1;\n"),
            vec![(
                CensusBucket::Ts,
                CensusKind::Line,
                "#!/usr/bin/env node".into(),
                1
            )]
        );
    }

    /// The normalization is exactly the printer's licence, per kind — so each arm is graded
    /// by a pair that differs only in what that arm may drop, and by the neighbouring kind
    /// that may not drop the same thing.
    #[test]
    fn interior_normalization_is_the_printers_licence_per_kind() {
        let ts = |src: &str| comment_census(src, ParserType::TypeScript);

        // INDENTABLE block: reindents, so a line edge is not content…
        assert_eq!(ts("/**\n * x\n */"), ts("/**\n\t\t * x\n\t\t */"));
        // …including the non-ASCII members of the class the printer trims with.
        assert_eq!(ts("/**\n * x\u{a0}\n */"), ts("/**\n * x\n */"));

        // Any OTHER block is emitted verbatim, so every line edge IS content — the narrowness
        // the old blanket ASCII trim gave away.
        assert_ne!(ts("/* x\u{a0} */"), ts("/* x */"));
        assert_ne!(ts("/* x  */"), ts("/* x */"));
        assert_ne!(ts("/*\n a \n b */"), ts("/*\n a\n b */"));

        // LINE comment: `printComment` trims its one edge, JS `\s` and no wider — so a
        // trailing NBSP goes and a trailing NEL, which is not `\s`, stays.
        assert_eq!(ts("// x\u{a0}\n"), ts("// x\n"));
        assert_ne!(ts("// x\u{85}\n"), ts("// x\n"));
        // …and the leading edge is content there, since nothing trims it.
        assert_ne!(ts("//  x\n"), ts("// x\n"));
    }

    #[test]
    fn css_comments_strings_urls() {
        assert_eq!(
            census("a { color: red; /* c */ }", ParserType::Css),
            vec![(CensusBucket::Css, CensusKind::Block, " c ".into(), 1)]
        );
        assert!(census("a { content: '/* no */'; }", ParserType::Css).is_empty());
        // An unquoted url token is opaque…
        assert!(census("a { background: url(http://x/*y); }", ParserType::Css).is_empty());
        // …a quoted one is an ordinary function whose string is opaque and
        // whose surroundings are scanned.
        assert_eq!(
            census("a { background: url('/*a*/') /* c */; }", ParserType::Css),
            vec![(CensusBucket::Css, CensusKind::Block, " c ".into(), 1)]
        );
    }

    #[test]
    fn css_cdo_cdc_span_is_skipped_wholesale() {
        // tsv (matching parseCss) discards the whole span, comments included —
        // the census must not count them on the input side.
        assert!(census("<!-- a { color: red; /* hidden */ } -->", ParserType::Css).is_empty());
        assert_eq!(
            census("<!-- x -->\na { /* kept */ }", ParserType::Css),
            vec![(CensusBucket::Css, CensusKind::Block, " kept ".into(), 1)]
        );
    }

    #[test]
    fn svelte_template_comments_and_islands() {
        let src = "<script>// js\nlet a = 1;</script>\n<!-- tpl -->\n<style>/* css */</style>\n<p>{/* expr */ a}</p>\n";
        assert_eq!(
            census(src, ParserType::Svelte),
            vec![
                (CensusBucket::Ts, CensusKind::Line, " js".into(), 1),
                (CensusBucket::Ts, CensusKind::Block, " expr ".into(), 1),
                (CensusBucket::Css, CensusKind::Block, " css ".into(), 1),
                (CensusBucket::Template, CensusKind::Html, " tpl ".into(), 1),
            ]
        );
    }

    #[test]
    fn svelte_template_text_is_not_code() {
        // `//` in prose is text, not a comment.
        assert!(
            census(
                "<p>see https://example.com // not code</p>",
                ParserType::Svelte
            )
            .is_empty()
        );
    }

    #[test]
    fn svelte_block_close_is_not_a_regex_head() {
        // `{/if}` must not open a regex that swallows the rest of the document.
        let src = "{#if a /* c1 */}\n<p>x</p>\n{/if}\n<!-- after -->\n";
        assert_eq!(
            census(src, ParserType::Svelte),
            vec![
                (CensusBucket::Ts, CensusKind::Block, " c1 ".into(), 1),
                (
                    CensusBucket::Template,
                    CensusKind::Html,
                    " after ".into(),
                    1
                ),
            ]
        );
    }

    #[test]
    fn svelte_brace_with_leading_comment_or_regex_is_an_expression() {
        // `{/* c */ x}` is an expression tag, not a block close.
        assert_eq!(
            census("<p>{/* c */ x}</p><!-- t -->", ParserType::Svelte),
            vec![
                (CensusBucket::Ts, CensusKind::Block, " c ".into(), 1),
                (CensusBucket::Template, CensusKind::Html, " t ".into(), 1),
            ]
        );
        // `{/re/.test(x)}` heads with a regex, whose body is opaque — and the
        // scan must not swallow the document after it.
        assert_eq!(
            census("<p>{/a[/*]/.test(x)}</p><!-- t -->", ParserType::Svelte),
            vec![(CensusBucket::Template, CensusKind::Html, " t ".into(), 1)]
        );
    }

    #[test]
    fn svelte_attribute_expressions_are_code() {
        let src = "<div class={/* c */ x} on:click={() => { f(); /* d */ }} title=\"a{/* e */ b}c\">t</div>";
        assert_eq!(
            census(src, ParserType::Svelte),
            vec![
                (CensusBucket::Ts, CensusKind::Block, " c ".into(), 1),
                (CensusBucket::Ts, CensusKind::Block, " d ".into(), 1),
                (CensusBucket::Ts, CensusKind::Block, " e ".into(), 1),
            ]
        );
    }

    #[test]
    fn svelte_island_ends_at_first_close_tag_even_in_a_string() {
        // Svelte's own lexical rule: `</script>` inside a JS string ends the
        // island. The census must bound the island identically; the trailing
        // text after the phantom string is template.
        let src = "<script>const s = '</script>'; // swallowed\n</script>";
        // The island is `const s = '` — no comments in it; the rest is template
        // text (the `// swallowed` sits in text, not code).
        assert!(census(src, ParserType::Svelte).is_empty());
    }

    #[test]
    fn svelte_quoted_attr_value_shields_html_comment_lookalikes() {
        assert!(census("<div data-x=\"<!-- no -->\">t</div>", ParserType::Svelte).is_empty());
    }
}
