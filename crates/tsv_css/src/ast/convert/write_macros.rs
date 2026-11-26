//! The wire writer's site macros, shared by both of its mounts (`write.rs`'s module doc, §One
//! walk, two products).
//!
//! A site states its bytes ONCE, as a token spec these macros turn into both halves: the JSON
//! literal, `concat!`'d at compile time, so the JSON copy writes exactly the bytes it wrote
//! before the tape existed; and the tape's structure calls (`t_*`), each inside the mount's
//! `tape!`, which the JSON mount expands to nothing — so the JSON copy holds no tape call at all.
//! Every entry, key and keyword a spec names is looked up in the tape's tables in a `const`
//! (`tsv_lang::tape::entry_id` and its siblings), so a name the tables lack fails the build of
//! the tape copy.
//!
//! The macros name the mount's own items unqualified (`tape!`, the `put_*` string sites, the
//! sink's methods) and resolve them where they expand, so one spec means the right thing in
//! each copy.

/// The compile-time id of the tape entry `css:$t`.
#[cfg(feature = "_tape")]
macro_rules! entry {
    ($t:ident) => {{
        const E: u16 = ::tsv_lang::tape::entry_id(concat!("css:", stringify!($t)));
        E
    }};
}

/// The compile-time id of the `type`-less tape entry `~$t`.
#[cfg(feature = "_tape")]
macro_rules! object_entry {
    ($t:ident) => {{
        const E: u16 = ::tsv_lang::tape::entry_id(concat!("~", stringify!($t)));
        E
    }};
}

/// The compile-time presence-mask bit of entry `$e`'s field `$f`.
#[cfg(feature = "_tape")]
macro_rules! bit {
    ($e:expr, $f:ident) => {{
        const B: u8 = ::tsv_lang::tape::field_bit($e, stringify!($f));
        B
    }};
}

/// A site's token spec, in one of two modes over the same tokens:
///
/// - `spec!(@json [] …)` — the JSON bytes, one `concat!`'d `&'static str`;
/// - `spec!(@tape (w) …)` — the tape-structure calls on `w`, each inside `tape!`.
///
/// The tokens: `open T` (`{"type":"T"` — a typed record of entry `css:T` opens), `obj T` (`{` —
/// the `type`-less entry `~T`), `key T.f` (`,"f":` — entry `css:T`'s field `f`; its mask bit when
/// it has one), `okey T.f` / `okey0 T.f` (a `type`-less entry's key, with / without its comma),
/// `lopen` / `lclose` (`[` / `]`), `close` (`}`), `sep` (`,`), `nul_rec` / `nul_ref` / `nul`
/// (`null` as a record, a ref, a `null`-encoded field), `false` / `true`, and the three metadata
/// payloads `rule_meta`, `complex_meta`, `relative_meta` (the key and the object).
macro_rules! spec {
    // the metadata payloads — `parseCss()`'s constant `Default` shapes
    (@$m:ident $c:tt rule_meta $($r:tt)*) => {
        spec!(@$m $c key Rule.metadata obj CSSRuleMetadata okey0 CSSRuleMetadata.parent_rule nul
            okey CSSRuleMetadata.has_local_selectors false
            okey CSSRuleMetadata.has_global_selectors false
            okey CSSRuleMetadata.is_global_block false close $($r)*)
    };
    (@$m:ident $c:tt complex_meta $($r:tt)*) => {
        spec!(@$m $c key ComplexSelector.metadata obj CSSComplexSelectorMetadata
            okey0 CSSComplexSelectorMetadata.rule nul
            okey CSSComplexSelectorMetadata.is_global false
            okey CSSComplexSelectorMetadata.used false close $($r)*)
    };
    (@$m:ident $c:tt relative_meta $($r:tt)*) => {
        spec!(@$m $c key RelativeSelector.metadata obj CSSRelativeSelectorMetadata
            okey0 CSSRelativeSelectorMetadata.is_global false
            okey CSSRelativeSelectorMetadata.is_global_like false
            okey CSSRelativeSelectorMetadata.scoped false close $($r)*)
    };

    // JSON bytes
    (@json [$($a:expr),*]) => { concat!("" $(, $a)*) };
    (@json [$($a:expr),*] open $t:ident $($r:tt)*) => {
        spec!(@json [$($a,)* "{\"type\":\"", stringify!($t), "\""] $($r)*)
    };
    (@json [$($a:expr),*] obj $t:ident $($r:tt)*) => { spec!(@json [$($a,)* "{"] $($r)*) };
    (@json [$($a:expr),*] key $t:ident . $f:ident $($r:tt)*) => {
        spec!(@json [$($a,)* ",\"", stringify!($f), "\":"] $($r)*)
    };
    (@json [$($a:expr),*] okey $t:ident . $f:ident $($r:tt)*) => {
        spec!(@json [$($a,)* ",\"", stringify!($f), "\":"] $($r)*)
    };
    (@json [$($a:expr),*] okey0 $t:ident . $f:ident $($r:tt)*) => {
        spec!(@json [$($a,)* "\"", stringify!($f), "\":"] $($r)*)
    };
    (@json [$($a:expr),*] lopen $($r:tt)*) => { spec!(@json [$($a,)* "["] $($r)*) };
    (@json [$($a:expr),*] lclose $($r:tt)*) => { spec!(@json [$($a,)* "]"] $($r)*) };
    (@json [$($a:expr),*] close $($r:tt)*) => { spec!(@json [$($a,)* "}"] $($r)*) };
    (@json [$($a:expr),*] sep $($r:tt)*) => { spec!(@json [$($a,)* ","] $($r)*) };
    (@json [$($a:expr),*] nul_rec $($r:tt)*) => { spec!(@json [$($a,)* "null"] $($r)*) };
    (@json [$($a:expr),*] nul_ref $($r:tt)*) => { spec!(@json [$($a,)* "null"] $($r)*) };
    (@json [$($a:expr),*] nul $($r:tt)*) => { spec!(@json [$($a,)* "null"] $($r)*) };
    (@json [$($a:expr),*] false $($r:tt)*) => { spec!(@json [$($a,)* "false"] $($r)*) };
    (@json [$($a:expr),*] true $($r:tt)*) => { spec!(@json [$($a,)* "true"] $($r)*) };

    // tape-structure calls
    (@tape ($w:ident)) => {};
    (@tape ($w:ident) open $t:ident $($r:tt)*) => {
        tape! { $w.t_open(entry!($t)); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) obj $t:ident $($r:tt)*) => {
        tape! { $w.t_open_object(object_entry!($t)); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) key $t:ident . $f:ident $($r:tt)*) => {
        tape! { $w.t_bit(bit!(entry!($t), $f)); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) okey $t:ident . $f:ident $($r:tt)*) => {
        tape! { $w.t_bit(bit!(object_entry!($t), $f)); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) okey0 $t:ident . $f:ident $($r:tt)*) => {
        tape! { $w.t_bit(bit!(object_entry!($t), $f)); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) lopen $($r:tt)*) => {
        tape! { $w.t_list_open(); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) lclose $($r:tt)*) => {
        tape! { $w.t_list_close(); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) close $($r:tt)*) => {
        tape! { $w.t_close(); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) sep $($r:tt)*) => { spec!(@tape ($w) $($r)*); };
    (@tape ($w:ident) nul_rec $($r:tt)*) => {
        tape! { $w.t_null_record(); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) nul_ref $($r:tt)*) => {
        tape! { $w.t_null_ref(); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) nul $($r:tt)*) => { spec!(@tape ($w) $($r)*); };
    (@tape ($w:ident) false $($r:tt)*) => {
        tape! { $w.t_bool(false); }
        spec!(@tape ($w) $($r)*);
    };
    (@tape ($w:ident) true $($r:tt)*) => {
        tape! { $w.t_bool(true); }
        spec!(@tape ($w) $($r)*);
    };
}

/// A spec's JSON bytes as a fixed-length array, for the windows that take `&[u8; N]`.
macro_rules! lit_bytes {
    ($($s:tt)*) => {{
        const S: &str = spec!(@json [] $($s)*);
        const B: [u8; S.len()] = $crate::ast::convert::write_macros::lit_array(S);
        &B
    }};
}

/// A literal site: the spec's JSON bytes through `raw`, its tape calls beside them.
macro_rules! raw {
    ($w:ident; $($s:tt)*) => {{
        spec!(@tape ($w) $($s)*);
        $w.raw(spec!(@json [] $($s)*));
    }};
}

/// An opening burst (`Ctx::head`): the lead's spec, the two offsets, the close's spec.
macro_rules! head {
    ($ctx:expr, $w:ident; [$($l:tt)*] $start:expr, $end:expr; [$($c:tt)*]) => {{
        spec!(@tape ($w) $($l)*);
        $ctx.head($w, lit_bytes!($($l)*), $start, $end, lit_bytes!($($c)*));
        spec!(@tape ($w) $($c)*);
    }};
}

/// A closing burst (`Ctx::tail`): the lead's spec, the two offsets, the close's spec.
macro_rules! tail {
    ($ctx:expr, $w:ident; [$($l:tt)*] $start:expr, $end:expr; [$($c:tt)*]) => {{
        spec!(@tape ($w) $($l)*);
        $ctx.tail($w, lit_bytes!($($l)*), $start, $end, lit_bytes!($($c)*));
        spec!(@tape ($w) $($c)*);
    }};
}

/// [`head!`] from shared span words — the JSON copy's path alone: the tape makes no words
/// (`tsv_lang::WireSink::span_words`).
macro_rules! head_words {
    ($w:ident; [$($l:tt)*] $words:expr; [$($c:tt)*]) => {{
        spec!(@tape ($w) $($l)*);
        $w.start_end_head_words(lit_bytes!($($l)*), $words, lit_bytes!($($c)*));
        spec!(@tape ($w) $($c)*);
    }};
}

/// [`tail!`] from shared span words — the JSON copy's path alone, as [`head_words!`].
macro_rules! tail_words {
    ($w:ident; [$($l:tt)*] $words:expr; [$($c:tt)*]) => {{
        spec!(@tape ($w) $($l)*);
        $w.start_end_tail_words(lit_bytes!($($l)*), $words, lit_bytes!($($c)*));
        spec!(@tape ($w) $($c)*);
    }};
}

/// A string behind its key, in one window on the JSON wire (the mount's `put_keyed_string`).
macro_rules! keyed {
    ($w:ident; [$($l:tt)*] $bytes:expr) => {{
        spec!(@tape ($w) $($l)*);
        put_keyed_string($w, lit_bytes!($($l)*), $bytes);
    }};
}

/// A keyword field's value: an escape-free token `$s` on the JSON wire, its vocabulary index `$i`
/// on the tape (`keyword_index!`, resolved at build time).
macro_rules! keyword {
    ($w:ident; $t:ident . $f:ident, $s:expr, $i:expr) => {{
        let s: &'static str = $s;
        tape! {
            $w.t_keyword_id(
                {
                    const A: u16 = ::tsv_lang::tape::field_aux(entry!($t), stringify!($f));
                    A
                },
                $i,
            );
        }
        json_only! { let () = $i; }
        $w.token(s);
    }};
}

/// The build-time index of `$v`'s word (`$v.as_str()`, `$v` of enum `$e`, its variants listed —
/// a variant left out fails the match) in entry `$t`'s keyword field `$f`: a `u32` in the tape
/// copy, nothing (`()`) in the JSON copy, which writes the word.
macro_rules! keyword_index {
    ($t:ident . $f:ident, $v:expr, $e:path => [$($var:ident),+ $(,)?]) => {
        copy_pick!((), {
            use $e as T;
            const A: u16 = ::tsv_lang::tape::field_aux(entry!($t), stringify!($f));
            match $v {
                $(T::$var => {
                    const I: u32 = ::tsv_lang::tape::keyword_id(A, T::$var.as_str());
                    I
                })+
            }
        })
    };
}

/// A static escape-free string in a ref position: a token on the JSON wire, a ref on the tape.
macro_rules! token_ref {
    ($w:ident; $s:expr) => {{
        let s: &'static str = $s;
        tape! { $w.t_ref(s.as_bytes()); }
        $w.token(s);
    }};
}

/// A `null` in a record position.
macro_rules! null_record {
    ($w:ident) => {{
        tape! { $w.t_null_record(); }
        $w.null();
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
