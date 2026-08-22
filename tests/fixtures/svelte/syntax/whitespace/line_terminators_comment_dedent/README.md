# line_terminators_comment_dedent

Svelte's comment dedent reads line terminators with **two different classes**, and
the wire `value` is where the difference shows. `onComment`
(`svelte/packages/svelte/src/compiler/phases/1-parse/acorn.js`) dedents a
multi-line block comment by the indentation of the line it opens on, in two steps:

- it finds that line with `while (a > 0 && source[a - 1] !== '\n') a -= 1` — `\n`
  and nothing else, so a `<LS>` / `<PS>` ahead of the comment is ordinary text and
  the indentation taken is still the *line's* own;
- it strips that indentation with
  `value.replace(new RegExp('^' + indentation, 'gm'), '')`, and an `m`-mode `^`
  opens a line after every ECMAScript terminator.

`a1` and `b1` are the walk-back arm in both directions — `a1`'s terminator is
followed by no indent at all, `b1`'s by a wider one than its line opens with, so a
mis-read line start under-dedents at one and over-dedents at the other. An indent
that happened to match the line's own would hide the bug outright, which is why
neither shape alone is the case. `c1` and `d1` are the strip arm.

`e1` is the null control: an `<LS>` whose two readings agree, which is what makes
the other four a claim about the *class* rather than about the character.

Because a `.svelte` fixture pins the whole wire, this also pins the `loc` half of
the same class — every position after one of these terminators is acorn's line
count, not `locate-character`'s. The region-by-region claims about that live in
[line_terminators_acorn_regions](../line_terminators_acorn_regions/).

The `<CR>` spelling belongs to both claims and can be a fixture input for neither:
every parse-then-format entry point folds it to `<LF>` before parsing, so such a
document is not the fixed point F1 requires. It is pinned by
[`tests/comment_dedent_line_terminators.rs`](../../../../../comment_dedent_line_terminators.rs)
and [`tests/acorn_loc_line_terminators.rs`](../../../../../acorn_loc_line_terminators.rs).
