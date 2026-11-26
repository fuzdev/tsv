//! The wire writer's site macros, shared by both of its mounts (`write/mod.rs`'s module doc, §One
//! walk, two products).
//!
//! A site states its bytes ONCE, as the JSON literal it writes. The JSON copy writes it; the tape
//! copy replays what the literal means on the tape — read from it at compile time by
//! `tsv_lang::tape::literal_ops`, in a `const` — inside the mount's `tape!`, which the JSON mount
//! expands to nothing, so the JSON copy holds no tape call. A site whose literal holds a key
//! names the entry its keys belong to, so each key's field is resolved at build time and a key
//! the entry lacks fails the tape copy's build — nothing is looked up by name at run time:
//!
//! - `raw!(w, "CallExpression", ",\"optional\":false")`: the entry by name;
//! - `raw!(w, @E, …)`: a helper shared by several entries takes its caller's entry as a const
//!   generic (`fn write_x<const E: u16>`, called `write_x::<{ at!("T") }>(…)`), so each of the
//!   tape copy's instances resolves its keys at build time (the JSON copy has one instance,
//!   `at!` being `0` there);
//! - `raw!(w, @in e => "A" | "B", …)`: a site whose record is one of a few entries chosen at run
//!   time resolves its keys against each, and the run-time entry `e` picks one.
//!
//! The open record must be the named entry's, which the sink checks as it writes the key, in
//! every build. A key's value is written as its field lays it down (`tsv_lang::tape::
//! TapeWriter`'s keyed values), whether the literal holds it or a data call after it writes it;
//! a keyword field's value is its index, resolved at build time — the literal's own word by the
//! reader, a word chosen at run time by `keyword!`, an exhaustive match over its enum's variants.
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

/// A helper's const-generic entry argument: the entry named `$e` in the tape copy, `0` in the
/// JSON copy, which reads no entry (so it instantiates the helper once).
macro_rules! at {
    ($e:literal) => {
        copy_pick!(0, entry!($e))
    };
}

/// A run-time entry argument (`Ent`, the mount's type): the entry named `$e` in the tape copy,
/// nothing in the JSON copy (`()`, which a call passes no register for).
macro_rules! ent {
    ($e:literal) => {
        copy_pick!((), entry!($e))
    };
}

/// A literal's tape ops, in a `const`: its keys entry `$e`'s (`@gen E`: a const generic's; `@none`:
/// a literal holding no key — one that holds a key fails the build).
#[cfg_attr(
    not(feature = "_tape"),
    expect(unused_macros, reason = "expanded inside `tape!` alone")
)]
macro_rules! lit_ops {
    ($e:literal, $s:expr) => {{
        const S: &str = $s;
        const E: Option<u16> = Some(::tsv_lang::tape::entry_id($e));
        const OPS: [::tsv_lang::tape::LitOp; ::tsv_lang::tape::literal_len(S)] =
            ::tsv_lang::tape::literal_ops(E, S);
        &OPS
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
    ($w:ident, [$e:literal], $s:literal) => {
        $w.t_lit(lit_ops!($e, $s))
    };
    ($w:ident, [@ $g:ident], $s:literal) => {
        $w.t_lit(lit_ops!(@gen $g, $s))
    };
    ($w:ident, [@in $t:expr => $($c:literal)|+], $s:literal) => {{
        let t: u16 = $t;
        $(if t == entry!($c) {
            $w.t_lit(lit_ops!($c, $s));
        } else)+ {
            $w.t_wrong_entry();
        }
    }};
    ($w:ident, [], $s:literal) => {
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

/// A literal site through `JsonWriter::raw_fixed` (a fixed-width append); its entry as `raw!`'s.
macro_rules! fixed {
    ($w:ident, $e:literal, $s:literal) => {{
        tape! { t_lit!($w, [$e], $s); }
        json_only! { $w.raw_fixed(lit_bytes!($s)); }
    }};
}

/// A `JsonWriter::raw_pick` site: one fused literal per value of the condition; its entry as
/// `raw!`'s.
macro_rules! pick {
    ($w:ident, $e:literal, $b:expr, $t:literal, $f:literal) => {
        pick!(@ $w, [$e], $b, $t, $f)
    };
    ($w:ident, @ $g:ident, $b:expr, $t:literal, $f:literal) => {
        pick!(@ $w, [@ $g], $b, $t, $f)
    };
    (@ $w:ident, [$($e:tt)*], $b:expr, $t:literal, $f:literal) => {{
        let b: bool = $b;
        tape! {
            if b {
                t_lit!($w, [$($e)*], $t);
            } else {
                t_lit!($w, [$($e)*], $f);
            }
        }
        json_only! { $w.raw_pick(b, lit_bytes!($t), lit_bytes!($f)); }
    }};
}

/// A run-time enum value's build-time answer: an exhaustive match over the variants named, each
/// arm `$f` of that variant (bound to `$x`) evaluated in a `const` — a name the tables lack
/// fails the build, and a variant left out fails the match (`else $d`: the arm of every other
/// variant, for a value the caller has narrowed).
#[cfg_attr(
    not(feature = "_tape"),
    expect(unused_macros, reason = "expanded inside `tape!` alone")
)]
macro_rules! by_variant {
    ($v:expr, $($t:ident)::+ => [$($var:ident),+ $(,)?], |$x:ident| -> $r:ty { $f:expr }
        $(else $d:expr)?) => {{
        type T = $($t)::+;
        match $v {
            $(T::$var => {
                const R: $r = {
                    let $x = T::$var;
                    $f
                };
                R
            })+
            $(_ => $d,)?
        }
    }};
}

/// A keyword field's value chosen at run time — `$v.$m()`, a word of enum `$t` — after the key
/// a literal wrote: the word on the JSON wire (`quoted`: between the quotes the literals hold,
/// `token`: an escape-free token), its index in the field's vocabulary on the tape, resolved at
/// build time per variant (`by_variant!`). The field is entry `$e`'s field `$f`; `|$x| $e` names
/// the entry per variant.
macro_rules! keyword {
    ($w:ident, $mode:ident, $v:expr => $m:ident, $($t:ident)::+ => [$($var:ident),+ $(,)?],
        $e:literal . $f:literal) => {
        keyword!($w, $mode, $v => $m, $($t)::+ => [$($var),+], |x| $e, $f)
    };
    ($w:ident, $mode:ident, $v:expr => $m:ident, $($t:ident)::+ => [$($var:ident),+ $(,)?],
        |$x:ident| $e:expr, $f:literal) => {{
        let v = $v;
        tape! {
            let (vocab, index) = by_variant!(v, $($t)::+ => [$($var),+], |$x| -> (u16, u32) {{
                let vocab = ::tsv_lang::tape::field_aux(::tsv_lang::tape::entry_id($e), $f);
                (vocab, ::tsv_lang::tape::keyword_id(vocab, $x.$m()))
            }});
            $w.t_keyword_id(vocab, index);
        }
        json_only! { keyword!(@json $mode $w, v.$m()); }
    }};
    (@json quoted $w:ident, $s:expr) => {
        $w.raw($s)
    };
    (@json token $w:ident, $s:expr) => {
        $w.token($s)
    };
}

/// A literal's bytes as a fixed-length array, for the appends that take `&[u8; N]`.
macro_rules! lit_bytes {
    ($s:literal) => {{
        const S: &str = $s;
        const B: [u8; S.len()] = $crate::ast::convert::write_macros::lit_array(S);
        &B
    }};
}

/// An import's `phase` keyword (`$p`, a `crate::imports::ImportPhase` other than `None`), of
/// entry `$e`'s `phase` field, by its build-time index — `keyword!` for a word `as_str` returns
/// as an `Option`.
#[cfg_attr(
    not(feature = "_tape"),
    expect(unused_macros, reason = "expanded inside `tape!` alone")
)]
macro_rules! phase_keyword {
    ($w:ident, $p:expr, $e:literal) => {{
        type P = crate::imports::ImportPhase;
        const VOCAB: u16 = ::tsv_lang::tape::field_aux(entry!($e), "phase");
        const fn index(p: P) -> u32 {
            match p.as_str() {
                Some(word) => ::tsv_lang::tape::keyword_id(VOCAB, word),
                None => u32::MAX,
            }
        }
        match $p {
            P::Source => $w.t_keyword_id(VOCAB, const { index(P::Source) }),
            P::Defer => $w.t_keyword_id(VOCAB, const { index(P::Defer) }),
            P::None => $w.t_wrong_entry(),
        }
    }};
}

/// A node's header (`node_header`): the record of entry `$t` opens on the tape (`$t @ $e`: the
/// wire type `$t`, the tape entry `$e` — a named variant; `@dyn t => e`: a type chosen at run
/// time, `t` its name on the JSON wire and `e` its entry on the tape, an `Ent` or a `u16` resolved
/// at build time).
macro_rules! node_header {
    ($w:ident, $t:literal, $span:expr, $ctx:expr) => {{
        tape! { $w.t_open(entry!($t)); }
        node_header($w, $t, $span, $ctx);
    }};
    ($w:ident, $t:literal @ $e:literal, $span:expr, $ctx:expr) => {{
        tape! { $w.t_open(entry!($e)); }
        node_header($w, $t, $span, $ctx);
    }};
    ($w:ident, @dyn $t:expr => $e:expr, $span:expr, $ctx:expr) => {{
        let t: &'static str = $t;
        tape! {
            let e: u16 = $e;
            #[cfg(debug_assertions)]
            assert_eq!(
                ::tsv_lang::tape::shapes::ENTRIES[usize::from(e)].ty,
                Some(t),
                "a header's tape entry is its JSON type's"
            );
            $w.t_open(e);
        }
        node_header($w, t, $span, $ctx);
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

/// A key chosen at run time (a spine link's child key — `ChildKey`, the mount's type, made by
/// `child_key!`): the literal on the JSON wire, its ops on the tape.
macro_rules! key_raw {
    ($w:ident, $k:expr) => {{
        let k: ChildKey = $k;
        tape! { $w.t_lit(k.1); }
        json_only! { $w.raw(k); }
    }};
}

/// A spine link's child key (`ChildKey`): the literal `$s`, entry `$e`'s, on the JSON wire; the
/// entry and the literal's ops on the tape, resolved at build time.
macro_rules! child_key {
    ($e:literal, $s:literal) => {
        copy_pick!($s, (entry!($e), lit_ops!($e, $s)))
    };
}
