// CSS whitespace classification, shared by the value parser (separation) and the
// printer (value-text normalization) so both agree on what counts as whitespace.

/// CSS whitespace is ASCII-only — tab, line feed, form feed, carriage return, and space.
///
/// ⚠️ **Five, where css-syntax-3 §4.2 defines three.** §4.2 is *newline* (which it fixes at
/// U+000A alone), tab and space; carriage return and form feed are absent from it **because
/// §3.3's input preprocessing has already folded them to U+000A** by the time tokenization
/// runs. tsv does not run that preprocessing — `parse` takes the author's bytes so its offsets
/// stay a drop-in contract with `parseCss` — which puts the two folded code points back on this
/// predicate's plate. Reading §4.2 on its own and narrowing this to `[' ', '\t', '\n']` is
/// therefore a plausible-looking change that stops every value scanner on a `<CR>` or a form
/// feed. The rule is §4.2 **plus** §3.3, and `is_ascii_whitespace` is exactly that union.
///
/// ⚠️ **Not the class the lexer skips at a token boundary**, which is JS `\s`
/// ([`tsv_lang::is_js_whitespace`]) because that is what `parseCss`'s `allow_whitespace()`
/// is. The two answer different questions and differ at every code point at or above
/// U+00A0: this one is about value *separation* and value-text *collapsing*, where a
/// `<NBSP>` is content that must not be rewritten to a space; that one is about whether a
/// token even starts here. Both are right, and swapping either for the other is a bug.
///
/// This is deliberately **not** `char::is_whitespace`,
/// which follows the Unicode `White_Space` property and would treat NBSP
/// (U+00A0), NEL, the ideographic space, etc. as separators (it also includes
/// U+000B VT, which CSS does not). Those code points are ordinary value content —
/// both prettier and Svelte's `parseCss` keep them inside their token — so all of
/// CSS value *separation* (`ValueCursor` here, and the byte-scanning
/// `classify_separators` via the equivalent `u8::is_ascii_whitespace`) and
/// value-text whitespace *collapsing* (the printer's `normalize_css_whitespace`)
/// act only on ASCII whitespace; otherwise a non-ASCII-whitespace code point would
/// be silently rewritten to a space.
#[inline]
pub(crate) fn is_css_whitespace(c: char) -> bool {
    c.is_ascii_whitespace()
}

/// Whether `c` is whitespace to `parseCss`'s `allow_whitespace()` **and** identifier content
/// to its `read_identifier` — the ambiguous class, and the exact run a token boundary skips
/// and the printer puts back.
///
/// The two sides are one fact asked from opposite directions: the parser decides what the AST
/// does **not** contain (`CssParser::skip_boundary_whitespace`, and its comment-looping twin
/// `skip_boundary_whitespace_registering_comments`), and the printer decides what the output
/// must still carry. Spelling it twice is a drift hazard with two silent failure modes — a
/// printer that restores less than the parser skipped deletes source, and one that restores
/// more duplicates it — so every one of them asks *this* rather than re-deriving a class.
///
/// ⚠️ **A skip and a preservation are one change.** Adding a juncture to the parser without
/// the matching restore turns a graceful over-rejection into content loss, which is the worse
/// trade; both failure modes above have been live. The printer has two shapes of restore
/// because the junctures have two shapes of anchor, and which one applies is a property of
/// what follows the run:
///
/// - `Printer::preserved_boundary_ws` — a BACKWARD scan from the node that follows the run,
///   for a gap that ends at a name (a compound, an explicit combinator). It takes a FLOOR,
///   because a run glued to the previous identifier is already inside that name's span and an
///   unbounded scan emits it a second time.
/// - `Printer::spell_gap` — a FORWARD collection over a gap that ends at a
///   STRUCTURAL token instead (`,`, `{`, `)`, `]`, an attribute value), which has no node to
///   anchor on and may hold comments the printer emits separately.
///
/// Both live in `printer/boundary_ws.rs`, with the whole model — the partition between them,
/// where a claim is emitted, and the residue — in that module's own doc, for the same reason
/// this class is named once.
///
/// It is JS `\s` minus its ASCII members: an ASCII space really is a separator on both sides
/// of `parseCss`, so it is the printer's to regenerate as indentation, not to preserve
/// verbatim.
#[inline]
pub(crate) fn is_boundary_only_whitespace(c: char) -> bool {
    !c.is_ascii() && tsv_lang::is_js_whitespace(c)
}

/// The whole run `CssParser::skip_boundary_whitespace` steps: the lexer's own whitespace
/// (`is_ascii_css_whitespace` plus the sub-U+00A0 non-ASCII whitespace its dispatch admits)
/// **and** the members hiding inside an identifier token.
///
/// Named once because the printer scans that run **backwards** out of the source, where the
/// parser met it as a token stream, and the two must agree on where it starts — a printer
/// that stops early deletes the members behind its stopping point. Both terms are load-
/// bearing at that seam: `<NBSP><VT>div` needs the ASCII half to reach the `<NBSP>`, and
/// `<NBSP><NEL>div` needs the `White_Space` half.
///
/// ⚠️ Neither term alone is this set, and neither is [`is_css_whitespace`] — that one is the
/// css-syntax-3 *tokenization* class (ASCII, no `<VT>`), the right answer for value
/// separation and the wrong one here. This is JS `\s` ∪ Unicode `White_Space`, which is JS
/// `\s` plus `<NEL>` (U+0085): the lexer reads a `<NEL>` as whitespace where `parseCss`
/// rejects it, so the run the parser really steps includes one, and the printer must scan
/// past it to reach anything behind it. That over-acceptance is the tracked `<NEL>` gap — see
/// [`tests/css_boundary_whitespace.rs`](../../../tests/css_boundary_whitespace.rs) — and
/// mirroring it here is what keeps the printer preserving exactly what the parser skipped,
/// gap included, rather than inventing a second answer.
#[inline]
pub(crate) fn is_boundary_whitespace(c: char) -> bool {
    tsv_lang::is_js_whitespace(c) || c.is_whitespace()
}

/// The ASCII members of [`is_boundary_whitespace`] — the six code points below U+0080 that
/// JS `\s` and `White_Space` share: `<TAB>`, `<LF>`, `<VT>`, `<FF>`, `<CR>` and space.
///
/// The half of a boundary run the printer REGENERATES (as indentation, or as the one space
/// it puts around an operator) rather than preserves — so a trim over text that may hold both
/// halves asks this and leaves the non-ASCII members standing, where `str::trim` (Unicode
/// `White_Space`) would take the `<NBSP>` with them and `str::trim_ascii` would stop on the
/// `<VT>`. The byte spelling is `Printer::boundary_run`'s inner loop
/// (`u8::is_ascii_whitespace() || b == 0x0b`); this is the char one.
#[inline]
pub(crate) const fn is_ascii_boundary_whitespace(c: char) -> bool {
    c.is_ascii() && tsv_lang::is_js_whitespace(c)
}

/// Could the char that begins or ends at this byte be [`is_boundary_whitespace`]? `true` for
/// the six ASCII members — the two classes agree there: `<TAB>`, `<LF>`, `<VT>`, `<FF>`,
/// `<CR>` and space — and for every non-ASCII byte, which may belong to a multi-byte member.
/// `false` is a proof and `true` is a question for the char predicate: an ASCII byte outside
/// the six is a whole char of its own, and not one either class holds.
///
/// The gate ahead of a trim over that class. A property name settles both of its ends here in
/// four compares, where the char-predicate searchers each decode a char from one end to find
/// nothing to trim — and a searcher's construction, not its walk, was the declaration
/// printer's largest single cost.
#[inline]
pub(crate) const fn byte_may_be_boundary_whitespace(b: u8) -> bool {
    b >= 0x80 || matches!(b, b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b' ')
}

/// Byte length of the [`is_boundary_only_whitespace`] run at the head of `text`, or `0`.
///
/// The one measurement of "how much of this identifier token is really the run
/// `allow_whitespace()` would have stepped", asked by the parser three ways: to decide
/// whether a run stands here (`CssParser::boundary_run_len`), to step past one
/// (`CssParser::skip_boundary_whitespace`'s lookaheads), and to tell a glued compound
/// continuation from a boundary (`compound_continues_across_comments`). They are one
/// question, so they are one function — a `starts_with` spelling beside a `take_while` one
/// is how two of the three come to disagree about a mixed run.
#[inline]
pub(crate) fn boundary_prefix_len(text: &str) -> usize {
    text.chars()
        .take_while(|c| is_boundary_only_whitespace(*c))
        .map(char::len_utf8)
        .sum()
}

/// Byte offset of the first [`is_boundary_only_whitespace`] member INSIDE `text`, past its
/// head, or `None`.
///
/// The twin of [`boundary_prefix_len`] for a reader whose own class is JS `\s` where the
/// lexer's is "everything at or above U+00A0": `read_attribute_value` ends a bare value at the
/// first `\s` (`[a=b<NBSP>]` is the value `b`, `[a=b<NBSP>i]` the value `b` and the flag `i`),
/// where the lexer handed `parse_attribute_value` one identifier token — this is where that
/// token really ends. The case flag beside it is a narrower reader still
/// (`REGEX_ATTRIBUTE_FLAGS` reads letters only, so `[a=b i<NBSP>]` is the flag `i`) and takes
/// its own ASCII-letter prefix rather than asking this.
///
/// A `\` escapes the code point after it on both sides — `read_attribute_value` appends `\`
/// plus the char, the lexer reads the escape — so an escaped member is content and is stepped
/// over (`[a=b\<NBSP>]` is the value `b\<NBSP>` to both).
pub(crate) fn boundary_split_offset(text: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if is_boundary_only_whitespace(c) {
            return Some(i);
        }
    }
    None
}

/// Whether `[from, to)` of `source` holds a boundary MEMBER outside its comments — the test
/// that hands a gap to the speller. A non-ASCII character inside a comment (`/* é */`) is not
/// a member, and neither is a `U+FEFF` at offset 0, the byte-order mark tsv strips by policy.
///
/// The one statement of the question for the printer's claims (`Printer::gap_holds_member`,
/// which asks it behind the document precondition) and the parser's prelude readers, which
/// spell a gap into text a node carries.
pub(crate) fn holds_boundary_member(source: &str, from: usize, to: usize) -> bool {
    let bytes = source.as_bytes();
    if from >= to || to > bytes.len() || bytes[from..to].is_ascii() {
        return false;
    }
    let mut i = from;
    while i < to {
        if crate::comments::is_comment_start(bytes, i) {
            i = crate::comments::comment_end(bytes, i);
            continue;
        }
        let Some(c) = source[i..].chars().next() else {
            break;
        };
        if !(i == 0 && c == tsv_lang::BOM) && is_boundary_only_whitespace(c) {
            return true;
        }
        i += c.len_utf8();
    }
    false
}

// ## Spelling a boundary gap
//
// The one spelling every boundary claim emits, and its edge vocabulary. Named here rather than
// in the printer because the parser spells three kinds of gap into text a node carries — a
// condition part's `not` gap and an operator run's gaps (`parser/atrules/preludes.rs`), and a
// verbatim prelude's two ends (`parser/atrules/raw.rs`, `PreludeEnds`) — and a second speller
// is the drift this module exists to prevent; see `printer/boundary_ws.rs` §How a claim is
// spelled for the model.

/// How a spelled boundary gap meets the output on one side of it.
///
/// The gap's own interior is not a choice: its items (members, and comments where the claim
/// prints them) stay in source order, and each ASCII whitespace stretch between two is one
/// space, whoever claims it. What differs per claim is only what stands BESIDE the gap,
/// which the claim knows and the speller does not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Edge {
    /// Nothing on this side: the printer regenerates what stands there (a line's indentation,
    /// a combinator's separator, the ` {` after a rule's head), or it is a delimiter the run
    /// may touch (the `(` it opens after, the `,` / `)` / `]` / `}` it closes against).
    Flush,
    /// One space if the author put ASCII whitespace between the gap's outermost item and this
    /// side, and nothing if they glued it. The answer beside a NAME or a TERM, where the
    /// author's presence is the claim: `a <ZWNBSP>{` must not come back as `a<ZWNBSP> {`,
    /// and `2n<NBSP>)` must not grow a space the author never wrote.
    Presence,
    /// One space whatever the author wrote: a name the run is about to be emitted after
    /// would otherwise take it in (`read_identifier` treats every code point at or above
    /// U+00A0 as content) — see `Printer::name_run_edge`.
    Space,
}

/// What [`spell_run`] does with a comment in the gap it spells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Comments {
    /// Print it, in place, as an item of the run — the gap's one emitter owns it.
    InPlace,
    /// Leave it to the emitter that owns it elsewhere; to the spelling it is only
    /// separation between the members either side of it.
    Elsewhere,
    /// Print it in place, as [`Comments::InPlace`] does, into text a NODE carries rather than
    /// into the output — the parser's spellings (a condition part's `not` gap, an operator
    /// run's gaps, a verbatim prelude's two ends). This seam reports nothing to the print-once
    /// ledger, because it prints nothing: the comment's accounting stays with the printer
    /// that emits the carrying text. Where the comment is registered at all, that is already
    /// settled there — a verbatim prelude's printer declares its whole span verbatim
    /// (`record_verbatim_range`), which covers a comment an abandoned structured reader
    /// registered before the prelude fell back; a BOUND operator run's text is printed only
    /// between its operators, where the run un-registers what it carries, while a gap outside
    /// them stays registered and is claimed from source; an UNBOUND run
    /// (`ConditionQuery::trailing_operators`, through `OperatorRun::finish`) prints its whole
    /// buffer and un-registers everything it collected; and a `not` gap's comments are never
    /// registered.
    Carried,
}

/// The one spelling of a boundary gap, which every claim emits (the printer's, in
/// `printer/boundary_ws.rs`, and the parser's three that spell a gap into a node's text — see
/// [`Comments::Carried`]): its ITEMS —
/// the members and, where the claim owns them ([`Comments::InPlace`]), the comments — in
/// source order, every ASCII whitespace stretch between two items as ONE space and its
/// absence as none, and the two edges as `lead` and `trail` say ([`Edge`]).
///
/// Two readers make the rule, and they agree on it. `parseCss` skips the whole run, so
/// nothing about the AST depends on the spelling. css-syntax-3, whose whitespace is ASCII
/// only, reads each member as identifier content, each ASCII stretch as one
/// `<whitespace-token>`, and a comment as nothing at all, so the spelling IS the
/// tokenization: `<NBSP><NBSP>` is one identifier where `<NBSP> <NBSP>` is two, a stretch
/// welded to nothing merges two tokens, a comment glued to a member keeps it glued to what
/// is on the comment's other side (`<NBSP>/* c */:hover` is the compound `<NBSP>:hover`),
/// and a stretch respelled as a tab or a line break is the same token — but a line break
/// carried inside the text the printer emits is one the doc renderer neither indents after
/// nor counts. So the items stay where the author put them and each stretch between them is
/// one space: the spelling that keeps every token the author wrote and nothing else, and
/// prettier's too, at every juncture where prettier keeps the run.
///
/// Scans `[from, to)` of `source`. Any other character in the gap (the `(` a caller's range
/// opens on) is neither an item nor a separation. `bom_at_zero` excludes a `U+FEFF` at
/// offset 0: the byte-order mark of a whole document, which tsv strips by policy (see
/// `Printer::boundary_run`); it is neither a member nor a separation. An in-place comment
/// is reported to the print-once ledger here, since this is the seam that prints it.
pub(crate) fn spell_run(
    source: &str,
    from: usize,
    to: usize,
    bom_at_zero: bool,
    edges: (Edge, Edge),
    comments: Comments,
) -> String {
    let (lead, trail) = edges;
    let bytes = source.as_bytes();
    let mut out = String::new();
    // Whether ASCII whitespace (or, to a members-only spelling, a comment) stands between
    // the previous item (or the gap's start) and this position.
    let mut separated = false;
    let mut i = from;
    while i < to {
        let item_start = i;
        if crate::comments::is_comment_start(bytes, i) {
            let end = crate::comments::comment_end(bytes, i).min(to);
            i = end;
            if comments == Comments::Elsewhere {
                separated = true;
                continue;
            }
            #[cfg(feature = "comment_check")]
            if comments == Comments::InPlace {
                tsv_lang::comment_ledger::record_emitted(
                    source,
                    tsv_lang::Span {
                        start: item_start as u32,
                        end: end as u32,
                    },
                );
            }
            push_item(&mut out, &source[item_start..end], separated, lead, false);
            separated = false;
            continue;
        }
        let Some(c) = source[i..].chars().next() else {
            break;
        };
        i += c.len_utf8();
        if bom_at_zero && item_start == 0 && c == tsv_lang::BOM {
            continue;
        }
        if is_boundary_only_whitespace(c) {
            push_item(&mut out, &source[item_start..i], separated, lead, true);
            separated = false;
        } else if is_boundary_whitespace(c) {
            separated = true;
        }
    }
    if !out.is_empty() && (trail == Edge::Space || (trail == Edge::Presence && separated)) {
        out.push(' ');
    }
    out
}

/// Append one item of a spelled gap: behind one space when `separated` says the author put
/// ASCII whitespace ahead of it — or, for the gap's first item, as `lead` says.
///
/// [`Edge::Space`] is a claim about a MEMBER, which a name ahead of it would take in; a
/// comment cannot be taken in (the name's token ends at its `/*`, and a comment tokenizes to
/// nothing), so a gap that opens on a comment keeps the author's separation there instead.
fn push_item(out: &mut String, item: &str, separated: bool, lead: Edge, is_member: bool) {
    let space = if out.is_empty() {
        match lead {
            Edge::Flush => false,
            Edge::Presence => separated,
            Edge::Space => is_member || separated,
        }
    } else {
        separated
    };
    if space {
        out.push(' ');
    }
    out.push_str(item);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The split stops at the first member, steps an escaped one, and stays `None` on the
    /// ASCII names every real stylesheet holds.
    #[test]
    fn boundary_split_offset_finds_the_first_unescaped_member() {
        assert_eq!(boundary_split_offset("value"), None);
        assert_eq!(boundary_split_offset("b\u{a0}"), Some(1));
        assert_eq!(boundary_split_offset("b\u{a0}i"), Some(1));
        assert_eq!(boundary_split_offset("i\u{feff}"), Some(1));
        assert_eq!(boundary_split_offset("b\\\u{a0}"), None);
        assert_eq!(boundary_split_offset("b\\\u{a0}\u{2028}"), Some(4));
        assert_eq!(boundary_split_offset("b\\41 c"), None);
    }

    /// The byte gate against the char class it stands in front of, at every byte value: an
    /// ASCII byte is a whole char, so the two must agree exactly there; a non-ASCII byte is
    /// a question the gate must not answer `false`.
    #[test]
    fn byte_gate_agrees_with_the_boundary_class_on_every_byte() {
        for b in 0..=255u8 {
            let gate = byte_may_be_boundary_whitespace(b);
            if b < 0x80 {
                assert_eq!(gate, is_boundary_whitespace(b as char), "byte {b:#x}");
            } else {
                assert!(
                    gate,
                    "non-ASCII byte {b:#x} must fall through to the predicate"
                );
            }
        }
    }
}
