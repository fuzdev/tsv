# escaped_whitespace_break_long_prettier_divergence

An escaped whitespace (`ccccc\ `) that **ends a token at a line break** — the last argument of a
function whose arguments wrap one-per-line, and a word the value fill breaks after — at the
100/101-column print-width boundary.

`\ ` is a valid escape whose escaped code point is that space (CSS Syntax 3 §4.3.4/§4.3.7), so the
space is the token's last character, not padding in front of the break. The line therefore ends
in `\ `: trimming trailing whitespace there would leave `\` before a newline, which is **not** a
valid escape (§4.3.4), and the output would not re-parse at all.

Both boundaries are pinned for both shapes:

- the **100-char** line fits exactly and stays inline;
- the **over-width** value breaks — after the last function argument, and at the fill separator
  after the escaped word — and the escaped space stays on its line, ahead of the break.

**Prettier drops the escape's payload**: `ccccc\ )` → `ccccc\)`, escaping the closing paren, so
its output no longer re-parses; and in the fill, `bbbb\  cccc` → `bbbb\cccc`, one ident with a
different value. It also never breaks either line (its usual print-width behavior).

tsv preserves the escape and treats the print width as a hard limit.

See [conformance_prettier_css.md §CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values).
