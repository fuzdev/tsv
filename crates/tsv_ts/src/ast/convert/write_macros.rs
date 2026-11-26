//! The wire writer's site macros, shared by both of its mounts (`write/mod.rs`'s module doc, §One
//! walk, two products).
//!
//! A site states its bytes ONCE, as the JSON literal it writes. The JSON copy writes it; the tape
//! copy replays what the literal means on the tape — read from it at compile time by
//! `tsv_lang::tape::literal_ops`, in a `const` — inside the mount's `tape!`, which the JSON mount
//! expands to nothing, so the JSON copy holds no tape call. A site names the entry its keys
//! belong to (`raw!(w, "CallExpression", ",\"optional\":false")`), so each key's field is looked
//! up at build time and a key the entry lacks fails the tape copy's build; a site shared by
//! several entries names none, and its keys are read against whatever record is open at run
//! time. A key's value is written as its field lays it down (`tsv_lang::tape::TapeWriter`'s keyed
//! values), whether the literal holds it or a data call after it writes it.
//!
//! The JSON-only half of each macro is inside `json_only!`: the tape copy's sink has no `raw`, so
//! a literal written past these macros fails its build rather than vanishing from the tape.

/// The compile-time id of the tape entry named `$e` (`"Literal"`, `"Literal/RegexLiteral"`).
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

/// A literal's tape ops, in a `const`: its keys entry `$e`'s, or (`@dyn`) the open record's.
#[cfg_attr(
    not(feature = "_tape"),
    expect(unused_macros, reason = "expanded inside `tape!` alone")
)]
macro_rules! lit_ops {
    ($e:literal, $s:expr) => {{
        const S: &str = $s;
        const E: Option<u16> = Some(::tsv_lang::tape::entry_id($e));
        const OPS: [::tsv_lang::tape::LitOp; ::tsv_lang::tape::literal_len(E, S)] =
            ::tsv_lang::tape::literal_ops(E, S);
        &OPS
    }};
    (@dyn $s:expr) => {{
        const S: &str = $s;
        const OPS: [::tsv_lang::tape::LitOp; ::tsv_lang::tape::literal_len(None, S)] =
            ::tsv_lang::tape::literal_ops(None, S);
        &OPS
    }};
}

/// A literal site (`JsonWriter::raw`): its keys entry `$e`'s, or the open record's when it
/// names none.
macro_rules! raw {
    ($w:ident, $e:literal, $s:literal) => {{
        tape! { $w.t_lit(lit_ops!($e, $s)); }
        json_only! { $w.raw($s); }
    }};
    ($w:ident, $s:literal) => {{
        tape! { $w.t_lit(lit_ops!(@dyn $s)); }
        json_only! { $w.raw($s); }
    }};
}

/// A literal site through `JsonWriter::raw_fixed` (a fixed-width append).
macro_rules! fixed {
    ($w:ident, $e:literal, $s:literal) => {{
        tape! { $w.t_lit(lit_ops!($e, $s)); }
        json_only! { $w.raw_fixed(lit_bytes!($s)); }
    }};
    ($w:ident, $s:literal) => {{
        tape! { $w.t_lit(lit_ops!(@dyn $s)); }
        json_only! { $w.raw_fixed(lit_bytes!($s)); }
    }};
}

/// A `JsonWriter::raw_pick` site: one fused literal per value of the condition.
macro_rules! pick {
    ($w:ident, $e:literal, $b:expr, $t:literal, $f:literal) => {{
        let b: bool = $b;
        tape! {
            if b {
                $w.t_lit(lit_ops!($e, $t));
            } else {
                $w.t_lit(lit_ops!($e, $f));
            }
        }
        json_only! { $w.raw_pick(b, lit_bytes!($t), lit_bytes!($f)); }
    }};
    ($w:ident, $b:expr, $t:literal, $f:literal) => {{
        let b: bool = $b;
        tape! {
            if b {
                $w.t_lit(lit_ops!(@dyn $t));
            } else {
                $w.t_lit(lit_ops!(@dyn $f));
            }
        }
        json_only! { $w.raw_pick(b, lit_bytes!($t), lit_bytes!($f)); }
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

/// A node's header (`node_header`): the record of entry `$t` opens on the tape (`$t @ $e`: the
/// wire type `$t`, the tape entry `$e` — a named variant; `@dyn`: the entry of a type known
/// only at run time, by its name).
macro_rules! node_header {
    ($w:ident, $t:literal, $span:expr, $ctx:expr) => {{
        tape! { $w.t_open(entry!($t)); }
        node_header($w, $t, $span, $ctx);
    }};
    ($w:ident, $t:literal @ $e:literal, $span:expr, $ctx:expr) => {{
        tape! { $w.t_open(entry!($e)); }
        node_header($w, $t, $span, $ctx);
    }};
    ($w:ident, @dyn $t:expr, $span:expr, $ctx:expr) => {{
        let t: &'static str = $t;
        tape! { $w.t_open_named(t); }
        node_header($w, t, $span, $ctx);
    }};
}

/// A string value the JSON copy writes between the quotes its literals hold (`raw`): the
/// string its field holds on the tape (an operator's keyword index).
macro_rules! quoted {
    ($w:ident, $s:expr) => {{
        let s: &'static str = $s;
        tape! { $w.string(s); }
        json_only! { $w.raw(s); }
    }};
}

/// A `null` in a list (an array hole): the `null` record on the tape.
macro_rules! null_item {
    ($w:ident) => {{
        tape! { $w.t_null_record(); }
        json_only! { $w.null(); }
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

/// A key known only at run time (`,"callee":` / `,"object":`, a spine link's child key): the
/// open record's field of that name on the tape.
macro_rules! key_raw {
    ($w:ident, $k:expr) => {{
        let k: &'static str = $k;
        tape! { $w.t_key_named(&k[2..k.len() - 2]); }
        json_only! { $w.raw(k); }
    }};
}
