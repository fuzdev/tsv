//! The wire writer's site macros, shared by both of its mounts (`write.rs`'s module doc, §One
//! walk, two products) — `tsv_ts`'s convention (its `ast/convert/write_macros.rs`), with the
//! forms a Svelte node's head needs.
//!
//! A site states its bytes ONCE, as the JSON literal it writes. The JSON copy writes it; the tape
//! copy replays what the literal means on the tape — read from it at compile time by
//! `tsv_lang::tape::literal_ops`, in a `const` — inside the mount's `tape!`, which the JSON mount
//! expands to nothing, so the JSON copy holds no tape call. A site whose literal holds a key
//! names the entry its keys belong to (`raw!(w, "EachBlock", ",\"body\":")`; `@E` for a const
//! generic's; `@in e => "A" | "B"` for one of a few entries chosen at run time), so each key's
//! field is resolved at build time and a key the entry lacks fails the tape copy's build —
//! nothing is looked up by name at run time.
//!
//! A Svelte node's head carries what the reader does not take as written, so its forms state it:
//!
//! - **The head itself** (`head!`): the record opens on the tape as the entry the site names —
//!   a named variant (`Literal/SvelteFusedLiteral`), or `css:StyleSheet`, which no `type`
//!   string names alone — and the literal's `{"type":"T"` is checked against that entry's
//!   `type` at build time and dropped. The head's offsets are the data call after its literal
//!   (`span_start_end`), written positionally — `start` and `end` are every such entry's required
//!   offsets — so the literal's trailing `"start":` key is dropped too
//!   ([`tape_head`], [`tape_keys`]).
//! - **A key ahead of `type`** (`Root`'s `css` / `js`, every directive's `start` / `end`): the
//!   site opens its entry before the literal's first key, and a later literal's leading
//!   `,"type":"T"` is checked and dropped like a head's ([`tape_text`]).
//!
//! The JSON-only half of each macro is inside `json_only!`: the tape copy's sink has no `raw`, so
//! a literal written past these macros fails its build rather than vanishing from the tape.

/// The compile-time id of the tape entry named `$e` (`"Literal"`, `"css:StyleSheet"`).
#[cfg_attr(
    not(feature = "_tape"),
    expect(unused_macros, reason = "expanded inside `tape!` alone")
)]
macro_rules! entry {
    ($e:literal) => {{
        const E: u16 = ::tsv_lang::tape::entry_id($e);
        E
    }};
}

/// A helper's const-generic entry argument: the entry named `$e` in the tape copy, `0` in the
/// JSON copy, which reads no entry (so it instantiates the helper once).
macro_rules! at {
    ($e:literal) => {
        copy_pick!(0, entry!($e))
    };
}

/// A literal's tape ops, in a `const`: its keys entry `$e`'s (`@gen E`: a const generic's;
/// `@none`: a literal holding no key — one that holds a key fails the build). The text read is
/// [`tape_text`]'s: a leading `,"type":"T"` of the named entry is checked and dropped.
#[cfg_attr(
    not(feature = "_tape"),
    expect(unused_macros, reason = "expanded inside `tape!` alone")
)]
macro_rules! lit_ops {
    ($e:literal, $s:expr) => {{
        const LIT_ENTRY: u16 = ::tsv_lang::tape::entry_id($e);
        const LIT_TEXT: &str = $crate::ast::convert::write_macros::tape_text(LIT_ENTRY, $s);
        const LIT_OPS: [::tsv_lang::tape::LitOp; ::tsv_lang::tape::literal_len(LIT_TEXT)] =
            ::tsv_lang::tape::literal_ops(Some(LIT_ENTRY), LIT_TEXT);
        &LIT_OPS
    }};
    (@gen $e:ident, $s:expr) => {{
        const S: &str = $s;
        const N: usize = ::tsv_lang::tape::literal_len(S);
        const { &::tsv_lang::tape::literal_ops::<N>(Some($e), S) }
    }};
    (@none $s:expr) => {{
        const S: &str = $s;
        const OPS: [::tsv_lang::tape::LitOp; ::tsv_lang::tape::literal_len(S)] =
            ::tsv_lang::tape::literal_ops(None, S);
        &OPS
    }};
}

/// The tape half of a literal site: the literal's ops under the site's naming of its entry
/// (`raw!`'s forms).
#[cfg_attr(
    not(feature = "_tape"),
    expect(unused_macros, reason = "expanded inside `tape!` alone")
)]
macro_rules! t_lit {
    ($w:ident, [$e:literal], $s:expr) => {
        $w.t_lit(lit_ops!($e, $s))
    };
    ($w:ident, [@ $g:ident], $s:expr) => {
        $w.t_lit(lit_ops!(@gen $g, $s))
    };
    ($w:ident, [@in $t:expr => $($c:literal)|+], $s:expr) => {{
        let t: u16 = $t;
        $(if t == entry!($c) {
            $w.t_lit(lit_ops!($c, $s));
        } else)+ {
            $w.t_wrong_entry();
        }
    }};
    ($w:ident, [], $s:expr) => {
        $w.t_lit(lit_ops!(@none $s))
    };
}

/// A literal site (`JsonWriter::raw`): its keys entry `$e`'s (`@E`: a const generic's; `@in t =>
/// "A" | "B"`: whichever of those entries the run-time entry `t` is), or none (a literal with no
/// key).
macro_rules! raw {
    ($w:ident, $e:literal, $s:literal) => {{
        tape! { t_lit!($w, [$e], $s); }
        json_only! { $w.raw($s); }
    }};
    ($w:ident, @ $g:ident, $s:literal) => {{
        tape! { t_lit!($w, [@ $g], $s); }
        json_only! { $w.raw($s); }
    }};
    ($w:ident, @in $t:expr => $($c:literal)|+, $s:literal) => {{
        tape! { t_lit!($w, [@in $t => $($c)|+], $s); }
        json_only! { $w.raw($s); }
    }};
    ($w:ident, $s:literal) => {{
        tape! { t_lit!($w, [], $s); }
        json_only! { $w.raw($s); }
    }};
}

/// The tape half of a stated-key site (`keyed!`): the key's presence bit, resolved at build
/// time for entry `$e` (`tsv_lang::tape::stated_key`, which fails the build for a literal that
/// is not one key whose value its data call writes in its own encoding — a keyword key's
/// included), with no field pending — and the field (`t_stated_debug`), which a debug build of
/// the sink checks the data call after it against. `keyword [$e]` is a keyword key's
/// (`keyword!`'s): resolved through `tsv_lang::tape::stated_keyword`, the one path that accepts
/// one, so a keyword key is stated only beside an index whose vocabulary `stated_vocab` reads from
/// the same literal.
#[cfg_attr(
    not(feature = "_tape"),
    expect(unused_macros, reason = "expanded inside `tape!` alone")
)]
macro_rules! t_stated {
    ($w:ident, [$e:literal], $s:literal) => {{
        const E: u16 = ::tsv_lang::tape::entry_id($e);
        const F: ::tsv_lang::tape::Field = ::tsv_lang::tape::stated_key(E, $s);
        $w.t_key_stated(E, F.bit);
        $w.t_stated_debug(F);
    }};
    ($w:ident, [@ $g:ident], $s:literal) => {{
        let f: ::tsv_lang::tape::Field = const { ::tsv_lang::tape::stated_key($g, $s) };
        $w.t_key_stated($g, f.bit);
        $w.t_stated_debug(f);
    }};
    ($w:ident, keyword [$e:literal], $s:literal) => {{
        const E: u16 = ::tsv_lang::tape::entry_id($e);
        const F: ::tsv_lang::tape::Field = ::tsv_lang::tape::stated_keyword(E, $s);
        $w.t_key_stated(E, F.bit);
        $w.t_stated_debug(F);
    }};
}

/// A stated-key site, `tsv_ts`'s form (its `write_macros.rs`): a literal holding one key whose
/// value the data call after it writes — a record or a list it opens, a ref — so the tape states
/// the key (`t_stated!`) rather than replaying the literal through the reader. Its entry as
/// `raw!`'s. A record field that may be `null` writes it through `write_or_null` (the tape's
/// `null_or_record`). A keyword key is not one: it goes through `keyword!`, and a `keyed!`
/// naming one fails the build.
macro_rules! keyed {
    ($w:ident, $e:literal, $s:literal) => {{
        tape! { t_stated!($w, [$e], $s); }
        json_only! { $w.raw($s); }
    }};
    ($w:ident, @ $g:ident, $s:literal) => {{
        tape! { t_stated!($w, [@ $g], $s); }
        json_only! { $w.raw($s); }
    }};
    ($w:ident, @in $t:expr => $($c:literal)|+, $s:literal) => {{
        tape! {
            let t: u16 = $t;
            $(if t == entry!($c) {
                t_stated!($w, [$c], $s);
            } else)+ {
                $w.t_wrong_entry();
            }
        }
        json_only! { $w.raw($s); }
    }};
}

/// A node's head: the literal `$s` — `{"type":"T"`, any constant keys, `,"start":` — then the
/// node's `start` and `end` (`span_start_end`). On the tape the record of entry `$e` opens, the
/// literal's keys between its `type` and its `start` replay ([`tape_head`]), and the two offsets
/// are written as they lie (`pick $b, $t, $f`: one literal per value of the condition, through
/// `JsonWriter::raw_pick`).
macro_rules! head {
    ($w:ident, $e:literal, $s:literal, $start:expr, $end:expr, $ctx:expr) => {{
        tape! {
            const _: () = $crate::ast::convert::write_macros::assert_positional_span(entry!($e));
            $w.t_open(entry!($e));
            head!(@keys $w, $e, $crate::ast::convert::write_macros::tape_head(entry!($e), $s));
        }
        json_only! { $w.raw($s); }
        $w.span_start_end($ctx.positions, $start, $end);
    }};
    ($w:ident, $e:literal, pick $b:expr, $t:literal, $f:literal, $start:expr, $end:expr,
        $ctx:expr) => {{
        let b: bool = $b;
        tape! {
            const _: () = $crate::ast::convert::write_macros::assert_positional_span(entry!($e));
            $w.t_open(entry!($e));
            if b {
                head!(@keys $w, $e, $crate::ast::convert::write_macros::tape_head(entry!($e), $t));
            } else {
                head!(@keys $w, $e, $crate::ast::convert::write_macros::tape_head(entry!($e), $f));
            }
        }
        json_only! { $w.raw_pick(b, lit_bytes!($t), lit_bytes!($f)); }
        $w.span_start_end($ctx.positions, $start, $end);
    }};
    // the keys a head's literal holds between its `type` and its `start`, when it holds any
    (@keys $w:ident, $e:literal, $s:expr) => {{
        const HEAD_KEYS: &str = $s;
        if const { ::tsv_lang::tape::literal_len(HEAD_KEYS) != 0 } {
            $w.t_lit(lit_ops!($e, HEAD_KEYS));
        }
    }};
}

/// Constant keys ending in `"start":` — `$s`, entry `$e`'s, or a record `$s` itself opens —
/// then the `start` and `end` of the record open at the literal's end (`span_start_end`): on
/// the tape the keys replay without that trailing `start` ([`tape_keys`]) and the two offsets
/// are written as they lie.
macro_rules! keys_span {
    ($w:ident, $e:literal, $s:literal, $start:expr, $end:expr, $ctx:expr) => {{
        tape! {
            let ops = lit_ops!($e, $crate::ast::convert::write_macros::tape_keys($s));
            // the offsets are the record the literal leaves open, positionally
            const _: () = $crate::ast::convert::write_macros::assert_positional_span($crate::ast::convert::write_macros::open_entry(
                entry!($e),
                lit_ops!($e, $crate::ast::convert::write_macros::tape_keys($s)),
            ));
            $w.t_lit(ops);
        }
        json_only! { $w.raw($s); }
        $w.span_start_end($ctx.positions, $start, $end);
    }};
}

/// A record whose keys lead its `type` opens (`{"css":`): on the tape the record of entry `$e`
/// opens and the literal's keys after its `{` replay ([`tape_open`]).
macro_rules! open_keys {
    ($w:ident, $e:literal, $s:literal) => {{
        tape! {
            $w.t_open(entry!($e));
            $w.t_lit(lit_ops!($e, $crate::ast::convert::write_macros::tape_open($s)));
        }
        json_only! { $w.raw($s); }
    }};
}

/// A `type`-less or `type`-after-offsets node's head (`JsonWriter::span_start_end_object`:
/// `{"start":N,"end":M`): on the tape the record of entry `$e` (or `@E`, a const
/// generic's) opens, then its two offsets.
macro_rules! head_object {
    ($w:ident, $e:literal, $start:expr, $end:expr, $ctx:expr) => {{
        tape! {
            const _: () = $crate::ast::convert::write_macros::assert_positional_span(entry!($e));
            $w.t_open(entry!($e));
        }
        head_object!(@span $w, $start, $end, $ctx);
    }};
    ($w:ident, @ $g:ident, $start:expr, $end:expr, $ctx:expr) => {{
        tape! {
            // `$g` is a const generic, so this assert is evaluated where the generic is
            // instantiated for codegen (any build or test), not under `cargo check` or clippy
            const { $crate::ast::convert::write_macros::assert_positional_span($g) };
            $w.t_open($g);
        }
        head_object!(@span $w, $start, $end, $ctx);
    }};
    (@span $w:ident, $start:expr, $end:expr, $ctx:expr) => {{
        tape! { $w.span_start_end($ctx.positions, $start, $end); }
        json_only! { $w.span_start_end_object($ctx.positions, $start, $end); }
    }};
}

/// A record's `}`: on the tape the record's close, called directly (`tsv_ts`'s `close_node`
/// says why not through the literal reader).
macro_rules! close {
    ($w:ident) => {{
        tape! { $w.t_close(); }
        json_only! { $w.raw("}"); }
    }};
}

/// A JSON list's `,` between two items, which the tape does not spell.
macro_rules! comma {
    ($w:ident) => {{
        json_only! { $w.raw(","); }
    }};
}

/// A run-time enum value's build-time answer: an exhaustive match over the variants named, each
/// arm `$f` of that variant (bound to `$x`) evaluated in a `const` — a name the tables lack
/// fails the build, and a variant left out fails the match.
#[cfg_attr(
    not(feature = "_tape"),
    expect(unused_macros, reason = "expanded inside `tape!` alone")
)]
macro_rules! by_variant {
    ($v:expr, $($t:ident)::+ => [$($var:ident),+ $(,)?], |$x:ident| -> $r:ty { $f:expr }) => {{
        type T = $($t)::+;
        match $v {
            $(T::$var => {
                const R: $r = {
                    let $x = T::$var;
                    $f
                };
                R
            })+
        }
    }};
}

/// A keyword field: its key, stated through `t_stated!`'s `keyword` form (entry `$e`), then its
/// value chosen at run time — `$v.$m()`, a word of enum `$t` — the word on the JSON wire (an
/// escape-free token), its index on the tape, resolved at build time per variant (`by_variant!`)
/// in the vocabulary of the field the key literal `$s` names (`tsv_lang::tape::stated_vocab`,
/// which fails the build for a key that is not a keyword field's). The index's vocabulary is the
/// stated key's, so the two cannot disagree.
macro_rules! keyword {
    ($w:ident, $e:literal, $s:literal, $v:expr => $m:ident,
        $($t:ident)::+ => [$($var:ident),+ $(,)?]) => {{
        tape! { t_stated!($w, keyword [$e], $s); }
        json_only! { $w.raw($s); }
        let v = $v;
        tape! {
            let (vocab, index) = by_variant!(v, $($t)::+ => [$($var),+], |x| -> (u16, u32) {{
                let vocab = ::tsv_lang::tape::stated_vocab(::tsv_lang::tape::entry_id($e), $s);
                (vocab, ::tsv_lang::tape::keyword_id(vocab, x.$m()))
            }});
            $w.t_keyword_id(vocab, index);
        }
        json_only! { $w.token(v.$m()); }
    }};
}

/// A literal's bytes as a fixed-length array, for the appends that take `&[u8; N]`.
macro_rules! lit_bytes {
    ($s:literal) => {{
        const S: &str = $s;
        const B: [u8; S.len()] = $crate::ast::convert::write_macros::lit_array(S);
        &B
    }};
}

/// `s`'s bytes as an array of its own length (`lit_bytes!`'s const helper).
pub(super) const fn lit_array<const N: usize>(s: &str) -> [u8; N] {
    let b = s.as_bytes();
    let mut a = [0u8; N];
    let mut i = 0;
    while i < N {
        a[i] = b[i];
        i += 1;
    }
    a
}

// ── the tape's reading of a literal: compile-time slices of the site's JSON text ──────────

/// The record of entry `e` takes its `start` and `end` positionally — the offsets a head's data
/// call writes with no key declared: both must be offsets every record of the entry carries,
/// `end` the field right after `start`. A schema edit that breaks this fails the tape copy's
/// build rather than shifting the record's words. Each site asserts it in a `const _` item, so
/// `cargo check` and clippy evaluate it too; only the const-generic form (`head_object!`'s
/// `@ E`) asserts in an inline `const` block, which only a codegen build evaluates.
#[cfg(feature = "_tape")]
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
pub(super) const fn assert_positional_span(e: u16) {
    let fields = ::tsv_lang::tape::shapes::ENTRIES[e as usize].fields;
    let mut i = 0;
    while i + 1 < fields.len() {
        if is_key(fields[i].key, "start") {
            assert_positional_field(e, "start", ::tsv_lang::tape::Enc::Offset);
            assert_positional_field(e, "end", ::tsv_lang::tape::Enc::Offset);
            assert!(
                is_key(fields[i + 1].key, "end"),
                "a positional `start` is followed by its `end`"
            );
            return;
        }
        i += 1;
    }
    panic!("a positional span's record has a `start` and an `end`");
}

/// Entry `e`'s field `key` is written positionally (no key declared): a field every record of
/// the entry carries, laid down as `enc`.
#[cfg(feature = "_tape")]
pub(super) const fn assert_positional_field(e: u16, key: &str, enc: ::tsv_lang::tape::Enc) {
    let f = ::tsv_lang::tape::field_of(e, key);
    assert!(
        f.enc as u8 == enc as u8 && f.bit == ::tsv_lang::tape::NO_BIT,
        "a positional field is a required one of the encoding its writer lays down"
    );
}

/// Whether a field's key is `key`.
#[cfg(feature = "_tape")]
const fn is_key(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && holds_at(a, 0, b)
}

/// The record a key literal's ops leave open at their end: the last record they open (none
/// closing after it), else the site's named entry `named`.
#[cfg(feature = "_tape")]
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
pub(super) const fn open_entry(named: u16, ops: &[::tsv_lang::tape::LitOp]) -> u16 {
    use ::tsv_lang::tape::LitOp;
    let mut open = named;
    let mut i = 0;
    while i < ops.len() {
        match ops[i] {
            LitOp::Open(e) | LitOp::OpenObject(e) => open = e,
            LitOp::Close => panic!("a span's literal closes no record"),
            _ => {}
        }
        i += 1;
    }
    open
}

/// Whether `s` holds `p` at `at`.
#[cfg(feature = "_tape")]
const fn holds_at(s: &[u8], at: usize, p: &[u8]) -> bool {
    if at + p.len() > s.len() {
        return false;
    }
    let mut i = 0;
    while i < p.len() {
        if s[at + i] != p[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// `s[from..to]`, borrowed for `'static` (a site's literal is ASCII).
#[cfg(feature = "_tape")]
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
const fn sub(s: &'static str, from: usize, to: usize) -> &'static str {
    let (_, rest) = s.as_bytes().split_at(from);
    let (mid, _) = rest.split_at(to - from);
    match core::str::from_utf8(mid) {
        Ok(text) => text,
        Err(_) => panic!("a site's literal is ASCII"),
    }
}

/// The length of the `"type":"T"` pair at `at` in `s`, `T` the type of entry `entry` (`0`
/// where none is there); a `type` of another value fails the build.
#[cfg(feature = "_tape")]
#[expect(
    clippy::panic,
    reason = "a compile-time reader: evaluated in a `const`, the panic is a build error"
)]
const fn type_pair_len(entry: u16, s: &[u8], at: usize) -> usize {
    const KEY: &[u8] = b"\"type\":\"";
    if !holds_at(s, at, KEY) {
        return 0;
    }
    let Some(ty) = ::tsv_lang::tape::shapes::ENTRIES[entry as usize].ty else {
        panic!("a literal's leading `type` names a `type`-less entry");
    };
    let ty = ty.as_bytes();
    assert!(
        holds_at(s, at + KEY.len(), ty) && holds_at(s, at + KEY.len() + ty.len(), b"\""),
        "the literal's `type` is not the named entry's"
    );
    KEY.len() + ty.len() + 1
}

/// A literal's text as the tape reads it under entry `entry`: a leading `,"type":"T"` (a key
/// ahead of `type` came first, in an earlier literal) is the entry's constant, checked and
/// dropped.
#[cfg(feature = "_tape")]
pub(super) const fn tape_text(entry: u16, s: &'static str) -> &'static str {
    let b = s.as_bytes();
    if !b.is_empty() && b[0] == b',' {
        let n = type_pair_len(entry, b, 1);
        if n != 0 {
            return sub(s, 1 + n, b.len());
        }
    }
    s
}

/// A literal ending in `"start":` without that key (and the `,` before it): its `start` and
/// `end` are written as the record's offsets, positionally.
#[cfg(feature = "_tape")]
pub(super) const fn tape_keys(s: &'static str) -> &'static str {
    const START: &[u8] = b"\"start\":";
    let b = s.as_bytes();
    assert!(
        b.len() >= START.len() && holds_at(b, b.len() - START.len(), START),
        "a head's literal ends in its `start` key"
    );
    let mut end = b.len() - START.len();
    if end > 0 && b[end - 1] == b',' {
        end -= 1;
    }
    sub(s, 0, end)
}

/// A literal opening a record whose keys lead its `type` (`{"css":`), without its `{`: the
/// record is open on the tape before the literal's keys replay.
#[cfg(feature = "_tape")]
pub(super) const fn tape_open(s: &'static str) -> &'static str {
    let b = s.as_bytes();
    assert!(
        !b.is_empty() && b[0] == b'{' && !holds_at(b, 1, b"\"type\":"),
        "a record whose keys lead its `type` opens at its `{{`"
    );
    sub(s, 1, b.len())
}

/// A head's literal (`{"type":"T"`, keys, `,"start":`) as the tape reads it once the record of
/// entry `entry` is open: the keys between the `type` — checked against the entry's — and the
/// `start` ([`tape_keys`]).
#[cfg(feature = "_tape")]
pub(super) const fn tape_head(entry: u16, s: &'static str) -> &'static str {
    let keys = tape_keys(s);
    let b = keys.as_bytes();
    assert!(
        !b.is_empty() && b[0] == b'{',
        "a head's literal opens its record"
    );
    let n = type_pair_len(entry, b, 1);
    assert!(n != 0, "a head's literal names its `type`");
    sub(keys, 1 + n, b.len())
}
