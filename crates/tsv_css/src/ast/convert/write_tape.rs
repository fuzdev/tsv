//! The wire writer's tape copy: `write.rs` mounted a second time, writing the tape
//! (`docs/tape_format.md`) into a `tsv_lang::tape::TapeWriter` — `write.rs`'s module doc, §One
//! walk, two products. Here are what the copy needs that the JSON copy takes from elsewhere:
//! its `tape!` / `json_only!`, its string sites and list helpers, and its own copies of the two
//! helpers the writer shares with nothing else (`declaration_split.rs` says why).

use crate::ast::convert::raw_selector_name;
use tsv_lang::Span;
use tsv_lang::tape::TapeWriter;

/// The tape copy compiles its tape code.
macro_rules! tape {
    ($($t:tt)*) => { $($t)* };
}

/// The tape copy drops the JSON copy's own code.
macro_rules! json_only {
    ($($t:tt)*) => {};
}

#[path = "declaration_split.rs"]
#[expect(
    clippy::duplicate_mod,
    reason = "a per-copy helper, mounted once per copy of the writer (module doc)"
)]
mod declaration_split;
#[path = "prelude_string.rs"]
#[expect(
    clippy::duplicate_mod,
    reason = "a per-copy helper, mounted once per copy of the writer (module doc)"
)]
mod prelude_string;
use declaration_split::split_declaration_svelte_compat;
use prelude_string::convert_prelude_to_string;

#[path = "write.rs"]
#[expect(
    clippy::duplicate_mod,
    reason = "the writer's tape copy, mounted beside its JSON copy (`write.rs`'s module doc)"
)]
#[expect(
    unreachable_pub,
    reason = "`CssComments` is `pub` for the JSON copy's Svelte composition entries, which the \
              tape copy does not compile yet"
)]
mod write;
pub(crate) use write::write_stylesheet_file_tape;

/// A string value: its ref (a source slice when the writer borrowed it from there).
#[inline]
fn put_string(w: &mut TapeWriter<'_>, s: &str) {
    w.t_ref(s.as_bytes());
}

/// A string behind its key — the JSON copy's one-window write; on the tape the key is the
/// site's spec, and the string a ref.
#[inline]
fn put_keyed_string<const K: usize>(w: &mut TapeWriter<'_>, _: &[u8; K], s: &[u8]) {
    w.t_ref(s);
}

/// A selector name read from source (the JSON copy's `write_selector_name`): the source slice
/// past `prefix_len` bytes of sigil when the name holds no escape, else `raw_selector_name`'s
/// half-decoded text, pooled.
#[inline]
fn put_selector_name(w: &mut TapeWriter<'_>, source: &str, span: Span, prefix_len: usize) {
    let (start, end) = (span.start as usize + prefix_len, span.end as usize);
    let bytes = source.as_bytes();
    if start <= end
        && end <= bytes.len()
        && tsv_lang::swar::next_byte_below_or_of(bytes, start, end, 0x20, [b'\\', b'"']) == end
    {
        w.t_ref(&bytes[start..end]);
        return;
    }
    w.t_ref(raw_selector_name(source, span, prefix_len).as_bytes());
}

/// A list: its count, then its items — `tsv_lang::write_array` on the tape.
#[inline]
fn put_list<T>(
    w: &mut TapeWriter<'_>,
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(&mut TapeWriter<'_>, T),
) {
    w.t_list_open();
    for item in items {
        f(w, item);
    }
    w.t_list_close();
}

/// A nullable record: the item through `f`, or the `null` record — `tsv_lang::write_or_null`
/// on the tape.
#[inline]
fn put_record_or_null<T>(
    w: &mut TapeWriter<'_>,
    item: Option<&T>,
    f: impl FnOnce(&mut TapeWriter<'_>, &T),
) {
    match item {
        Some(v) => f(w, v),
        None => w.t_null_record(),
    }
}

/// A nullable ref: the item through `f`, or the null ref.
#[inline]
fn put_ref_or_null<T>(
    w: &mut TapeWriter<'_>,
    item: Option<&T>,
    f: impl FnOnce(&mut TapeWriter<'_>, &T),
) {
    match item {
        Some(v) => f(w, v),
        None => w.t_null_ref(),
    }
}
