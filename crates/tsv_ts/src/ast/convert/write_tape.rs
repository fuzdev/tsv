//! The wire writer's tape copy: `write/` mounted a second time, writing the tape
//! (`docs/tape_format.md`) into a `tsv_lang::tape::TapeWriter` — `write/mod.rs`'s module doc,
//! §One walk, two products. Here are what the copy needs that the JSON copy takes from
//! elsewhere: its `tape!` / `json_only!`, its list helpers, its own copy of the BigInt helper
//! (`bigint.rs` says why), and its forms of the functions each copy writes its own way
//! (`write_json.rs` holds the JSON copy's).

use crate::ast;
use tsv_lang::Span;
use tsv_lang::tape::TapeWriter;
use write::Ctx;
use write::expressions::{ExprFlags, write_chain_general};

/// The tape copy compiles its tape code.
macro_rules! tape {
    ($($t:tt)*) => { $($t)* };
}

/// The tape copy drops the JSON copy's own code.
macro_rules! json_only {
    ($($t:tt)*) => {};
}

/// The tape copy's half of an expression each copy spells its own way.
macro_rules! copy_pick {
    ($json:expr, $tape:expr) => {
        $tape
    };
}

/// A run-time entry argument (`ent!`): the entry's id.
type Ent = u16;

/// A spine link's child key (`child_key!`): the link's entry and its stated key's presence bit
/// (the key's field is a record's).
type ChildKey = (u16, u8);

#[path = "bigint.rs"]
#[expect(
    clippy::duplicate_mod,
    reason = "a per-copy helper, mounted once per copy of the writer (`bigint.rs`)"
)]
mod bigint;
use bigint::bigint_to_decimal;

#[path = "write/mod.rs"]
#[expect(
    clippy::duplicate_mod,
    reason = "the writer's tape copy, mounted beside its JSON copy (`write/mod.rs`'s module doc)"
)]
mod write;
// The embedded entry points `tsv_svelte`'s tape copy composes (`__private`'s `*_tape` names).
pub(crate) use write::write_program_tape;
pub use write::{
    write_expression_embedded as write_expression_embedded_tape,
    write_identifier_expression_with_character as write_identifier_expression_with_character_tape,
    write_pattern_embedded as write_pattern_embedded_tape,
    write_program_embedded as write_program_embedded_tape,
    write_variable_declaration_embedded as write_variable_declaration_embedded_tape,
};

/// A list: its count, then its items — `tsv_lang::write_array` on the tape.
#[inline]
fn write_array<T>(
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

/// A nullable child: the item through `f`, or the `null` its field holds — `tsv_lang::
/// write_or_null` on the tape, after a `raw!` key or a stated one (`keyed!`).
#[inline]
fn write_or_null<T>(
    w: &mut TapeWriter<'_>,
    item: Option<&T>,
    f: impl FnOnce(&mut TapeWriter<'_>, &T),
) {
    match item {
        Some(v) => f(w, v),
        None => w.null_or_record(),
    }
}

/// A node header's tape half: the record is open (the site's `node_header!`), so its two
/// offsets. `CHARACTER` lives in `loc`, which no tape carries.
#[inline]
fn header_run<const CHARACTER: bool>(
    w: &mut TapeWriter<'_>,
    _: &'static str,
    span: Span,
    ctx: &Ctx<'_>,
) {
    w.span_start_end(ctx.positions, span.start, span.end);
}

/// An identifier's `name` field: the source slice it occupies, or its decoded pool text. The
/// field is a required ref in every entry that holds an identifier's name, so it declares no
/// key: a key would set no bit and leave the value a ref, at a run-time lookup per name — the
/// tape copy's hottest.
#[inline]
fn write_name_field(w: &mut TapeWriter<'_>, name: ast::IdentName, name_start: u32, ctx: &Ctx<'_>) {
    w.t_top_field_debug("name", |field| {
        assert!(
            field.is_some_and(
                |f| f.enc == tsv_lang::tape::Enc::Ref && f.bit == tsv_lang::tape::NO_BIT
            ),
            "an identifier's name is a required ref of the open record"
        );
    });
    match name.escaped {
        Some(id) => w.string(ctx.view.escaped_text(id)),
        None => {
            let start = name_start as usize;
            w.string(&ctx.source[start..start + name.raw_len as usize]);
        }
    }
}

/// A numeric literal's tagged value: the `f64` the JSON text spells (a `-0` reads back as
/// `0`), or `null` for one past the `f64` range.
#[inline]
fn write_number_value(w: &mut TapeWriter<'_>, n: f64) {
    if n.is_finite() {
        w.number(n + 0.0);
    } else {
        w.null();
    }
}

/// A `Literal`'s fields, field by field (the JSON copy's fused write has no tape form).
#[inline]
fn write_literal_fields_any(w: &mut TapeWriter<'_>, lit: &ast::Literal, ctx: &Ctx<'_>) {
    write::write_literal_fields(w, lit, ctx);
}

/// A plain `Identifier` (the JSON copy stages its common case as one run).
#[inline]
fn write_identifier_plain(w: &mut TapeWriter<'_>, id: &ast::Identifier, ctx: &Ctx<'_>) {
    write::write_identifier_parts(w, id.span, id.ident_name(), false, None, None, ctx);
}

/// A call/member/non-null spine (the JSON copy stages its ordinary case).
#[inline]
fn write_chain_any(
    w: &mut TapeWriter<'_>,
    expr: &ast::Expression,
    ctx: &Ctx<'_>,
    flags: ExprFlags,
) {
    write_chain_general(w, expr, ctx, flags);
}

/// The tape of an embedded expression island written under `env` (a Svelte template island's
/// shape — its comment attach included), for the tape gates, which grade the comment emitter
/// through it apart from a Svelte host.
pub(crate) fn write_expression_island_tape(
    expr: &ast::Expression,
    env: super::EmbedWriter<'_>,
    sink: &mut tsv_lang::tape::TapeSink,
) {
    let mut w = sink.writer(env.source, env.positions, crate::WIRE_COORDINATES.bom);
    write::write_expression_embedded(&mut w, expr, env);
}
