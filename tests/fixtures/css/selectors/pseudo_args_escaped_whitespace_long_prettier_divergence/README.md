# pseudo_args_escaped_whitespace_long_prettier_divergence

An escaped whitespace ending the **last argument of a functional pseudo-class** (`.ccccc\ `, and an
escaped tab as the whole argument) whose selector list wraps one-per-line, at the 100/101-column
print-width boundary.

`\ ` and `\⇥` are valid escapes whose escaped code point is that whitespace (CSS Syntax 3
§4.3.4/§4.3.7), so it is the selector's last character, not padding in front of the break. The
line therefore ends in the escape: trimming trailing whitespace there would leave `\` before a
newline, which is **not** a valid escape (§4.3.4), and the output would not re-parse at all.

- the **100-char** selector fits exactly and stays inline;
- the **over-width** selector wraps its arguments one-per-line, and the escaped whitespace stays
  on the last argument's line, ahead of the break before `)`.

**Prettier mangles the escape**: from the wrapped form its first pass keeps `.ccccc\ ` but adds a
blank line before `)`, and it collapses the escaped-tab case back onto one over-width line with
the tab dropped (`\)`), escaping the closing paren, so that output no longer re-parses.

See [conformance_prettier_css.md §CSS: Selectors](../../../../../docs/conformance_prettier_css.md#css-selectors)
and [§CSS: Values](../../../../../docs/conformance_prettier_css.md#css-values).
